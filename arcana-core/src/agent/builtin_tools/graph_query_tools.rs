// Code-graph query tools for the interactive agent (P5-18..19, D-014).
//
// Eight read-only tools over the workspace's code graph: search, one symbol's
// details, its direct callers and callees, its impact, a file's symbols, the
// shortest dependency path between two symbols, and an overview. They replace
// the 27 unregistered structs in `graph_tools.rs`, which could not be
// registered as they stood: every one loaded the whole node and edge tables
// per call, `list_callers` was a transitive closure, and the traversal helpers
// they leaned on compared edge kinds against `"Calls"` while the extractor
// stores `"CALLS"`, so callers, callees and impact all came back empty. (No
// extractor writes TESTED_BY edges yet, so these tools do not report tests.)
//
// Properties every tool here keeps:
//   * Read-only and bounded. Queries go through the indexed per-node edge
//     lookup, results are capped (`limit`, depth and visit caps), and a cut is
//     reported as `truncated`, never silent.
//   * The graph is the workspace's own: resolved from `ToolContext.working_dir`
//     through the same canonical location the builder writes, and only when it
//     already exists. A tool never builds a graph or creates a file; with no
//     graph it says so and names the next step.
//   * A symbol may be named by simple name or qualified name. A simple name that
//     matches several symbols returns the candidates instead of guessing.
//   * Repository text only ever leaves as JSON string values, never as prose the
//     model could mistake for an instruction.

use crate::agent::tool_effect::ToolEffect;
use crate::agent::tool_registry::ToolRegistry;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::graph::store::{GraphEdge, GraphNode, GraphStats, GraphStore, OpenCurrentError};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;

/// The names this module registers, in one place so the host, the gate and the
/// tests agree on what "the graph tools" are.
pub const GRAPH_TOOL_NAMES: [&str; 8] = [
    "graph_search",
    "graph_node",
    "graph_callers",
    "graph_callees",
    "graph_impact",
    "graph_file",
    "graph_path",
    "graph_overview",
];

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 50;
const EDGES_PER_SIDE: usize = 30;
const SEARCH_CANDIDATES: usize = 100;
const NAME_CANDIDATES: usize = 200;
const AMBIGUOUS_SHOWN: usize = 10;
const MAX_IMPACT_DEPTH: usize = 5;
const DEFAULT_IMPACT_DEPTH: usize = 3;
const MAX_IMPACT_NODES: usize = 200;
const MAX_PATH_DEPTH: usize = 8;
const MAX_PATH_VISITED: usize = 5000;

/// Edge kinds as the extractor stores them (`EdgeKind`'s `Display`).
const CALLS: &str = "CALLS";
/// What makes one symbol depend on another.
const DEPENDENCY_KINDS: [&str; 6] = [
    "CALLS",
    "IMPORTS_FROM",
    "DEPENDS_ON",
    "INHERITS",
    "IMPLEMENTS",
    "REFERENCES",
];

/// Register the graph tools. Called only through
/// `AgentEngine::with_graph_tools`, so only the interactive engine has them.
pub fn register(registry: &mut ToolRegistry) {
    registry.register(Arc::new(GraphSearchTool));
    registry.register(Arc::new(GraphNodeTool));
    registry.register(Arc::new(GraphCallersTool));
    registry.register(Arc::new(GraphCalleesTool));
    registry.register(Arc::new(GraphImpactTool));
    registry.register(Arc::new(GraphFileTool));
    registry.register(Arc::new(GraphPathTool));
    registry.register(Arc::new(GraphOverviewTool));
}

// ── shared plumbing ──────────────────────────────────────────────────────────

type Outcome = std::result::Result<Value, ToolResult>;

fn invalid(message: &str) -> ToolResult {
    ToolResult::categorized_error(
        "invalid_input",
        "validation",
        message,
        "Fix the arguments and call the tool again.",
    )
}

fn not_found(message: String, next: &str) -> ToolResult {
    ToolResult::categorized_error("graph_symbol_not_found", "not_found", message, next)
}

fn read_failed(error: impl std::fmt::Display) -> ToolResult {
    ToolResult::categorized_error(
        "graph_read_failed",
        "graph",
        format!("The code graph could not be read: {error}"),
        "Rebuild the graph from the Graph tab, or continue without it.",
    )
}

fn arg_str<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

fn arg_limit(input: &Value, default: usize, max: usize) -> usize {
    input
        .get("limit")
        .and_then(Value::as_u64)
        .map(|n| (n as usize).clamp(1, max))
        .unwrap_or(default)
}

/// Run one graph query off the async executor, against the workspace's graph if
/// it has been built.
async fn with_graph<F>(ctx: &ToolContext, query: F) -> ToolResult
where
    F: FnOnce(&GraphStore, &Path) -> Outcome + Send + 'static,
{
    let working_dir = ctx.working_dir.clone();
    let joined = tokio::task::spawn_blocking(move || {
        let location = match crate::graph::location::find_existing(&working_dir) {
            Ok(Some(location)) => location,
            Ok(None) => {
                return Err(ToolResult::categorized_error(
                    "graph_not_built",
                    "precondition",
                    "No code graph has been built for this workspace.",
                    "Continue with grep, glob and read_file, or ask the user to build the graph from the Graph tab.",
                ))
            }
            Err(error) => return Err(read_failed(error)),
        };
        let store = match GraphStore::open_current(&location.db_path) {
            Ok(store) => store,
            Err(OpenCurrentError::NotCurrent { .. }) => {
                return Err(ToolResult::categorized_error(
                    "graph_needs_rebuild",
                    "precondition",
                    "The code graph for this workspace was built by an older version and is not read until it is rebuilt.",
                    "Continue with grep, glob and read_file, or ask the user to rebuild the graph from the Graph tab.",
                ))
            }
            Err(error) => return Err(read_failed(error)),
        };
        query(&store, &location.root)
    })
    .await;
    match joined {
        Ok(Ok(value)) => ToolResult::success(value.to_string()),
        Ok(Err(result)) => result,
        Err(error) => read_failed(error),
    }
}

/// Quote each alphanumeric run and join with OR, so punctuation and FTS5
/// operators in a model-supplied string are data.
fn fts_query(text: &str) -> String {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(16)
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn is_stub(qualified_name: &str) -> bool {
    qualified_name.starts_with("module::")
}

fn node_json(root: &Path, node: &GraphNode) -> Value {
    json!({
        "qualified_name": node.qualified_name,
        "name": node.name,
        "kind": node.kind,
        "file": crate::graph::context::relative(root, &node.file_path),
        "line": node.line_start,
    })
}

/// A node the graph knows, or just its name when only an edge mentions it
/// (an unresolved import or call).
fn describe(store: &GraphStore, root: &Path, qualified_name: &str) -> Result<Value, ToolResult> {
    match store.get_node(qualified_name).map_err(read_failed)? {
        Some(node) => Ok(node_json(root, &node)),
        None => {
            Ok(json!({ "qualified_name": qualified_name, "external": is_stub(qualified_name) }))
        }
    }
}

enum Resolved {
    One(Box<GraphNode>),
    /// The matches, and whether the name index was cut off while collecting them.
    Ambiguous(Vec<GraphNode>, bool),
    /// No match, and whether the name index was cut off while looking.
    Missing(bool),
}

/// A symbol by qualified name, else by simple name through the search index.
fn resolve(store: &GraphStore, symbol: &str) -> Result<Resolved, ToolResult> {
    if let Some(node) = store.get_node(symbol).map_err(read_failed)? {
        return Ok(Resolved::One(Box::new(node)));
    }
    let query = fts_query(symbol);
    if query.is_empty() {
        return Ok(Resolved::Missing(false));
    }
    let candidates = store
        .search_nodes(&query, NAME_CANDIDATES)
        .map_err(read_failed)?;
    let capped = candidates.len() >= NAME_CANDIDATES;
    let mut exact: Vec<GraphNode> = candidates
        .iter()
        .filter(|n| n.name == symbol)
        .cloned()
        .collect();
    if exact.is_empty() {
        exact = candidates
            .into_iter()
            .filter(|n| n.name.eq_ignore_ascii_case(symbol))
            .collect();
    }
    exact.sort_by(|a, b| a.qualified_name.cmp(&b.qualified_name));
    exact.dedup_by(|a, b| a.qualified_name == b.qualified_name);
    Ok(match exact.len() {
        0 => Resolved::Missing(capped),
        1 => Resolved::One(Box::new(exact.remove(0))),
        _ => Resolved::Ambiguous(exact, capped),
    })
}

/// The one node a symbol names, or the response to give instead: the
/// candidates when it is ambiguous, an error when it is unknown.
fn single(
    store: &GraphStore,
    root: &Path,
    symbol: &str,
) -> Result<GraphNode, std::result::Result<Value, ToolResult>> {
    match resolve(store, symbol).map_err(Err)? {
        Resolved::One(node) => Ok(*node),
        Resolved::Ambiguous(nodes, capped) => Err(Ok(json!({
            "status": "ambiguous",
            "symbol": symbol,
            "total_candidates": nodes.len(),
            "truncated": nodes.len() > AMBIGUOUS_SHOWN || capped,
            "candidates": nodes
                .iter()
                .take(AMBIGUOUS_SHOWN)
                .map(|n| node_json(root, n))
                .collect::<Vec<_>>(),
            "next": "Call again with one candidate's exact qualified_name.",
        }))),
        Resolved::Missing(capped) => Err(Err(not_found(
            if capped {
                format!("No symbol named '{symbol}' was found among the first {NAME_CANDIDATES} index matches; the name is common, so pass its qualified_name.")
            } else {
                format!("No symbol named '{symbol}' is in the code graph.")
            },
            "Use graph_search to find the symbol's name, or rebuild the graph if the code is new.",
        ))),
    }
}

fn edges_of(store: &GraphStore, qualified_name: &str) -> Result<Vec<GraphEdge>, ToolResult> {
    let mut edges = store
        .get_edges_for_node(qualified_name)
        .map_err(read_failed)?;
    edges.sort_by(|a, b| {
        (&a.kind, &a.source_qualified, &a.target_qualified, a.line).cmp(&(
            &b.kind,
            &b.source_qualified,
            &b.target_qualified,
            b.line,
        ))
    });
    Ok(edges)
}

fn symbol_schema(extra: Value) -> Value {
    let mut properties = json!({
        "symbol": {
            "type": "string",
            "description": "A symbol's simple name (e.g. render_invoice) or its exact qualified_name from another graph tool."
        }
    });
    if let (Some(map), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            map.insert(key.clone(), value.clone());
        }
    }
    json!({ "type": "object", "properties": properties, "required": ["symbol"] })
}

fn limit_property(max: usize) -> Value {
    json!({ "limit": {
        "type": "integer",
        "description": format!("Most results to return (default {DEFAULT_LIMIT}, at most {max}).")
    }})
}

// ── graph_search ─────────────────────────────────────────────────────────────

pub struct GraphSearchTool;

#[async_trait]
impl Tool for GraphSearchTool {
    fn name(&self) -> &str {
        "graph_search"
    }

    fn description(&self) -> &str {
        "Find symbols in the workspace's code graph by name words. Returns qualified_name, kind, file and line for each match. Matches on words in names and paths, not on meaning; needs a built graph."
    }

    fn input_schema(&self) -> Value {
        let mut properties = limit_property(MAX_LIMIT);
        if let Some(map) = properties.as_object_mut() {
            map.insert(
                "query".into(),
                json!({"type": "string", "description": "Words from the symbol's name or file path."}),
            );
            map.insert(
                "kind".into(),
                json!({"type": "string", "description": "Optional node kind to keep, e.g. Function, Class, File."}),
            );
        }
        json!({ "type": "object", "properties": properties, "required": ["query"] })
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(query) = arg_str(&input, "query").map(str::to_string) else {
            return invalid("`query` is required.");
        };
        let fts = fts_query(&query);
        if fts.is_empty() {
            return invalid("`query` must contain at least one letter or digit.");
        }
        let kind = arg_str(&input, "kind").map(str::to_lowercase);
        let limit = arg_limit(&input, DEFAULT_LIMIT, MAX_LIMIT);
        with_graph(ctx, move |store, root| {
            let found = store
                .search_nodes(&fts, SEARCH_CANDIDATES)
                .map_err(read_failed)?;
            let matching: Vec<&GraphNode> = found
                .iter()
                .filter(|n| kind.as_ref().is_none_or(|k| n.kind.to_lowercase() == *k))
                .collect();
            Ok(json!({
                "status": "ok",
                "count": matching.len().min(limit),
                // Cut when more matched than shown, or when the index itself was
                // capped (a kind filter is applied after that cap).
                "truncated": matching.len() > limit || found.len() >= SEARCH_CANDIDATES,
                "results": matching.iter().take(limit).map(|n| node_json(root, n)).collect::<Vec<_>>(),
            }))
        })
        .await
    }
}

// ── graph_node ───────────────────────────────────────────────────────────────

pub struct GraphNodeTool;

#[async_trait]
impl Tool for GraphNodeTool {
    fn name(&self) -> &str {
        "graph_node"
    }

    fn description(&self) -> &str {
        "Show one symbol from the code graph: where it is defined and its incoming and outgoing edges (calls, imports, containment), up to 30 each. A simple name shared by several symbols returns the candidates to choose from."
    }

    fn input_schema(&self) -> Value {
        symbol_schema(json!({}))
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(symbol) = arg_str(&input, "symbol").map(str::to_string) else {
            return invalid("`symbol` is required.");
        };
        with_graph(ctx, move |store, root| {
            let node = match single(store, root, &symbol) {
                Ok(node) => node,
                Err(response) => return response,
            };
            let edges = edges_of(store, &node.qualified_name)?;
            let side = |incoming: bool| -> Vec<&GraphEdge> {
                edges
                    .iter()
                    .filter(|e| (e.target_qualified == node.qualified_name) == incoming)
                    .collect()
            };
            let describe_edges = |list: Vec<&GraphEdge>, incoming: bool| -> Value {
                let total = list.len();
                json!({
                    "total": total,
                    "truncated": total > EDGES_PER_SIDE,
                    "edges": list.iter().take(EDGES_PER_SIDE).map(|e| {
                        let other = if incoming { &e.source_qualified } else { &e.target_qualified };
                        json!({
                            "kind": e.kind,
                            "node": other,
                            "external": is_stub(other),
                            "confidence": e.confidence_tier,
                        })
                    }).collect::<Vec<_>>(),
                })
            };
            let mut detail = node_json(root, &node);
            if let Some(map) = detail.as_object_mut() {
                map.insert("line_end".into(), json!(node.line_end));
                map.insert("language".into(), json!(node.language));
                map.insert("parent".into(), json!(node.parent_name));
                map.insert("params".into(), json!(node.params));
                map.insert("is_test".into(), json!(node.is_test));
                map.insert("community".into(), json!(node.community_name));
            }
            Ok(json!({
                "status": "ok",
                "node": detail,
                "incoming": describe_edges(side(true), true),
                "outgoing": describe_edges(side(false), false),
            }))
        })
        .await
    }
}

// ── graph_callers / graph_callees ────────────────────────────────────────────

fn direct_calls(
    store: &GraphStore,
    root: &Path,
    symbol: &str,
    limit: usize,
    callers: bool,
) -> Outcome {
    let node = match single(store, root, symbol) {
        Ok(node) => node,
        Err(response) => return response,
    };
    let others: BTreeSet<String> = edges_of(store, &node.qualified_name)?
        .into_iter()
        .filter(|e| e.kind == CALLS)
        .filter_map(|e| {
            if callers && e.target_qualified == node.qualified_name {
                Some(e.source_qualified)
            } else if !callers && e.source_qualified == node.qualified_name {
                Some(e.target_qualified)
            } else {
                None
            }
        })
        .collect();
    let total = others.len();
    let mut listed = Vec::new();
    for qualified in others.iter().take(limit) {
        listed.push(describe(store, root, qualified)?);
    }
    Ok(json!({
        "status": "ok",
        "symbol": node_json(root, &node),
        "direction": if callers { "callers" } else { "callees" },
        "total": total,
        "truncated": total > limit,
        "results": listed,
    }))
}

pub struct GraphCallersTool;

#[async_trait]
impl Tool for GraphCallersTool {
    fn name(&self) -> &str {
        "graph_callers"
    }

    fn description(&self) -> &str {
        "List the symbols that directly call a symbol, from the code graph. One hop only. The graph records calls only within a single file, so callers in other files are missing; graph_impact on the file lists the files that import it."
    }

    fn input_schema(&self) -> Value {
        symbol_schema(limit_property(MAX_LIMIT))
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(symbol) = arg_str(&input, "symbol").map(str::to_string) else {
            return invalid("`symbol` is required.");
        };
        let limit = arg_limit(&input, DEFAULT_LIMIT, MAX_LIMIT);
        with_graph(ctx, move |store, root| {
            direct_calls(store, root, &symbol, limit, true)
        })
        .await
    }
}

pub struct GraphCalleesTool;

#[async_trait]
impl Tool for GraphCalleesTool {
    fn name(&self) -> &str {
        "graph_callees"
    }

    fn description(&self) -> &str {
        "List the symbols a symbol directly calls, from the code graph. One hop only. The graph records calls only within a single file, so callees defined in other files are missing."
    }

    fn input_schema(&self) -> Value {
        symbol_schema(limit_property(MAX_LIMIT))
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(symbol) = arg_str(&input, "symbol").map(str::to_string) else {
            return invalid("`symbol` is required.");
        };
        let limit = arg_limit(&input, DEFAULT_LIMIT, MAX_LIMIT);
        with_graph(ctx, move |store, root| {
            direct_calls(store, root, &symbol, limit, false)
        })
        .await
    }
}

// ── graph_impact ─────────────────────────────────────────────────────────────

pub struct GraphImpactTool;

#[async_trait]
impl Tool for GraphImpactTool {
    fn name(&self) -> &str {
        "graph_impact"
    }

    fn description(&self) -> &str {
        "Find what may be affected by changing a symbol: the symbols that depend on it through calls, imports, inheritance or references, out to a chosen depth (default 3, at most 5). The graph records no test links. Calls are recorded only within a file, so pass a file name to find the files that import it. A static estimate, not proof; capped at 200 symbols."
    }

    fn input_schema(&self) -> Value {
        let mut extra = limit_property(MAX_LIMIT);
        if let Some(map) = extra.as_object_mut() {
            map.insert(
                "depth".into(),
                json!({"type": "integer", "description": "How many hops of dependents to follow (default 3, at most 5)."}),
            );
        }
        symbol_schema(extra)
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(symbol) = arg_str(&input, "symbol").map(str::to_string) else {
            return invalid("`symbol` is required.");
        };
        let limit = arg_limit(&input, DEFAULT_LIMIT, MAX_LIMIT);
        let depth = input
            .get("depth")
            .and_then(Value::as_u64)
            .map(|d| (d as usize).clamp(1, MAX_IMPACT_DEPTH))
            .unwrap_or(DEFAULT_IMPACT_DEPTH);
        with_graph(ctx, move |store, root| {
            let start = match single(store, root, &symbol) {
                Ok(node) => node,
                Err(response) => return response,
            };

            // Reverse breadth-first over dependency edges.
            let mut reached: HashMap<String, usize> = HashMap::new();
            reached.insert(start.qualified_name.clone(), 0);
            let mut queue = VecDeque::from([(start.qualified_name.clone(), 0usize)]);
            let mut node_cap_hit = false;
            while let Some((current, at)) = queue.pop_front() {
                let edges = edges_of(store, &current)?;
                if at >= depth {
                    continue;
                }
                for edge in edges {
                    if edge.target_qualified != current
                        || !DEPENDENCY_KINDS.contains(&edge.kind.as_str())
                        || is_stub(&edge.source_qualified)
                        || reached.contains_key(&edge.source_qualified)
                    {
                        continue;
                    }
                    if reached.len() > MAX_IMPACT_NODES {
                        node_cap_hit = true;
                        continue;
                    }
                    reached.insert(edge.source_qualified.clone(), at + 1);
                    queue.push_back((edge.source_qualified, at + 1));
                }
            }

            let mut affected: Vec<(usize, String)> = reached
                .into_iter()
                .filter(|(qualified, _)| *qualified != start.qualified_name)
                .map(|(qualified, at)| (at, qualified))
                .collect();
            affected.sort();
            let total = affected.len();
            let mut listed = Vec::new();
            for (at, qualified) in affected.iter().take(limit) {
                let mut entry = describe(store, root, qualified)?;
                if let Some(map) = entry.as_object_mut() {
                    map.insert("depth".into(), json!(at));
                }
                listed.push(entry);
            }
            Ok(json!({
                "status": "ok",
                "symbol": node_json(root, &start),
                "depth": depth,
                "total_affected": total,
                "truncated": total > limit || node_cap_hit,
                "affected": listed,
            }))
        })
        .await
    }
}

// ── graph_file ───────────────────────────────────────────────────────────────

pub struct GraphFileTool;

#[async_trait]
impl Tool for GraphFileTool {
    fn name(&self) -> &str {
        "graph_file"
    }

    fn description(&self) -> &str {
        "List the symbols the code graph holds for one file, in line order. Takes a workspace-relative or absolute file path. Only files the graph indexed appear; a file added since the last build does not."
    }

    fn input_schema(&self) -> Value {
        let mut properties = limit_property(100);
        if let Some(map) = properties.as_object_mut() {
            map.insert(
                "file".into(),
                json!({"type": "string", "description": "File path relative to the workspace, or absolute."}),
            );
        }
        json!({ "type": "object", "properties": properties, "required": ["file"] })
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let Some(file) = arg_str(&input, "file").map(str::to_string) else {
            return invalid("`file` is required.");
        };
        let limit = arg_limit(&input, 50, 100);
        with_graph(ctx, move |store, root| {
            for key in crate::graph::location::file_key_candidates(root, &file) {
                let nodes = store.list_nodes_by_file(&key).map_err(read_failed)?;
                if nodes.is_empty() {
                    continue;
                }
                let symbols: Vec<&GraphNode> =
                    nodes.iter().filter(|n| !is_stub(&n.qualified_name)).collect();
                return Ok(json!({
                    "status": "ok",
                    "file": crate::graph::context::relative(root, &key),
                    "total": symbols.len(),
                    "truncated": symbols.len() > limit,
                    "symbols": symbols.iter().take(limit).map(|n| json!({
                        "qualified_name": n.qualified_name,
                        "name": n.name,
                        "kind": n.kind,
                        "line_start": n.line_start,
                        "line_end": n.line_end,
                    })).collect::<Vec<_>>(),
                }));
            }
            Err(not_found(
                format!("The code graph has no symbols for '{file}'."),
                "Check the path with glob; if the file is new, the graph needs a rebuild from the Graph tab.",
            ))
        })
        .await
    }
}

// ── graph_path ───────────────────────────────────────────────────────────────

pub struct GraphPathTool;

#[async_trait]
impl Tool for GraphPathTool {
    fn name(&self) -> &str {
        "graph_path"
    }

    fn description(&self) -> &str {
        "Find the shortest chain of dependencies leading from one symbol to another in the code graph (calls, imports, inheritance, references), up to 8 hops. Reports no_path when none is found within the search bounds."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "from": {"type": "string", "description": "Starting symbol: simple name or exact qualified_name."},
                "to": {"type": "string", "description": "Target symbol: simple name or exact qualified_name."}
            },
            "required": ["from", "to"]
        })
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let (Some(from), Some(to)) = (
            arg_str(&input, "from").map(str::to_string),
            arg_str(&input, "to").map(str::to_string),
        ) else {
            return invalid("`from` and `to` are required.");
        };
        with_graph(ctx, move |store, root| {
            let start = match single(store, root, &from) {
                Ok(node) => node,
                Err(response) => return response,
            };
            let goal = match single(store, root, &to) {
                Ok(node) => node,
                Err(response) => return response,
            };

            let mut parent: HashMap<String, (String, String)> = HashMap::new();
            let mut seen: BTreeSet<String> = BTreeSet::from([start.qualified_name.clone()]);
            let mut queue = VecDeque::from([(start.qualified_name.clone(), 0usize)]);
            let mut found = start.qualified_name == goal.qualified_name;
            let mut bound_hit = false;
            while !found {
                let Some((current, at)) = queue.pop_front() else {
                    break;
                };
                if at >= MAX_PATH_DEPTH {
                    bound_hit = true;
                    continue;
                }
                for edge in edges_of(store, &current)? {
                    if edge.source_qualified != current
                        || !DEPENDENCY_KINDS.contains(&edge.kind.as_str())
                        || is_stub(&edge.target_qualified)
                        || seen.contains(&edge.target_qualified)
                    {
                        continue;
                    }
                    if seen.len() >= MAX_PATH_VISITED {
                        bound_hit = true;
                        break;
                    }
                    seen.insert(edge.target_qualified.clone());
                    parent.insert(
                        edge.target_qualified.clone(),
                        (current.clone(), edge.kind.clone()),
                    );
                    if edge.target_qualified == goal.qualified_name {
                        found = true;
                        break;
                    }
                    queue.push_back((edge.target_qualified, at + 1));
                }
            }

            if !found {
                return Ok(json!({
                    "status": "no_path",
                    "from": node_json(root, &start),
                    "to": node_json(root, &goal),
                    "search_bounded": bound_hit,
                }));
            }
            let mut chain = vec![(goal.qualified_name.clone(), None::<String>)];
            let mut at = goal.qualified_name.clone();
            while let Some((previous, kind)) = parent.get(&at) {
                chain.push((previous.clone(), Some(kind.clone())));
                at = previous.clone();
            }
            chain.reverse();
            // Each entry carries the edge that leads from it to the next one
            // (the goal, last, carries none).
            let mut steps = Vec::new();
            for (qualified, via) in &chain {
                let mut step = describe(store, root, qualified)?;
                if let (Some(map), Some(via)) = (step.as_object_mut(), via) {
                    map.insert("then".into(), json!(via));
                }
                steps.push(step);
            }
            Ok(json!({
                "status": "ok",
                "hops": steps.len() - 1,
                "steps": steps,
            }))
        })
        .await
    }
}

// ── graph_overview ───────────────────────────────────────────────────────────

pub struct GraphOverviewTool;

fn overview_json(root: &Path, stats: &GraphStats) -> Value {
    let sorted = |map: &std::collections::HashMap<String, i64>| -> BTreeMap<String, i64> {
        map.iter().map(|(k, v)| (k.clone(), *v)).collect()
    };
    json!({
        "status": "ok",
        "workspace": root.display().to_string(),
        "nodes": stats.node_count,
        "edges": stats.edge_count,
        "nodes_by_kind": sorted(&stats.nodes_by_kind),
        "edges_by_kind": sorted(&stats.edges_by_kind),
        "languages": stats.languages,
        "last_written": stats.last_updated,
    })
}

#[async_trait]
impl Tool for GraphOverviewTool {
    fn name(&self) -> &str {
        "graph_overview"
    }

    fn description(&self) -> &str {
        "Summarise the workspace's code graph: how many symbols and edges it holds, by kind and language, and when it was last written. Use it to see whether a graph exists and how current it is before relying on the other graph tools."
    }

    fn input_schema(&self) -> Value {
        json!({ "type": "object", "properties": {}, "required": [] })
    }

    fn effect(&self) -> ToolEffect {
        ToolEffect::Read
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> ToolResult {
        with_graph(ctx, |store, root| {
            let stats = store.get_stats().map_err(read_failed)?;
            Ok(overview_json(root, &stats))
        })
        .await
    }
}
