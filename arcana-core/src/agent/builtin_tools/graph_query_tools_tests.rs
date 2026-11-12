// S5-04 / P5-19 proofs for the graph query tools: each tool is executed against
// a graph the shipped detector, extractor and re-index built, and the bounds,
// the no-graph behaviour and the registration scope are checked.

use super::graph_query_tools::*;
use crate::agent::tool_effect::ToolEffect;
use crate::agent::tool_registry::ToolRegistry;
use crate::agent::tool_trait::{Tool, ToolContext};
use crate::graph::context_tests::{build_workspace_graph, write_invoice_workspace};
use crate::graph::schema::{EdgeInfo, NodeInfo};
use crate::graph::store::GraphStore;
use serde_json::{json, Value};

fn context(working_dir: &str) -> ToolContext {
    ToolContext {
        session_id: "graph-tools".into(),
        working_dir: working_dir.into(),
        call_id: "call".into(),
        run_id: None,
        task_id: None,
        permission_mode: crate::agent::permission::PermissionMode::Autonomous,
        depth: 0,
        agent_role: "supervisor".into(),
        run_token_budget: None,
    }
}

fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(GraphSearchTool),
        Box::new(GraphNodeTool),
        Box::new(GraphCallersTool),
        Box::new(GraphCalleesTool),
        Box::new(GraphImpactTool),
        Box::new(GraphFileTool),
        Box::new(GraphPathTool),
        Box::new(GraphOverviewTool),
    ]
}

async fn call(tool: &dyn Tool, input: Value, working_dir: &str) -> (bool, Value) {
    let result = tool.execute(input, &context(working_dir)).await;
    let parsed: Value = serde_json::from_str(&result.output)
        .unwrap_or_else(|_| panic!("{} returned non-JSON: {}", tool.name(), result.output));
    (result.is_error, parsed)
}

fn names(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The fixture graph, built by the shipped detector, extractor and re-index.
/// Returns the guards that keep the isolated home and the workspace alive.
fn invoice_graph() -> (
    crate::db::test_support::HomeGuard,
    tempfile::TempDir,
    tempfile::TempDir,
    String,
) {
    let home = tempfile::tempdir().expect("isolated home");
    let guard = crate::db::test_support::with_home(home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    write_invoice_workspace(workspace.path());
    let location = build_workspace_graph(workspace.path());
    let root = location.root.display().to_string();
    (guard, home, workspace, root)
}

/// Every tool answers with the fact the fixture contains.
#[tokio::test]
async fn every_graph_tool_answers_from_a_real_extractor_graph() {
    let (_guard, _home, _workspace, root) = invoice_graph();

    let (error, search) = call(&GraphSearchTool, json!({"query": "render invoice"}), &root).await;
    assert!(!error, "{search}");
    assert!(
        names(&search, "results").contains(&"render_invoice".to_string()),
        "{search}"
    );

    let (error, node) = call(&GraphNodeTool, json!({"symbol": "render_invoice"}), &root).await;
    assert!(!error, "{node}");
    assert_eq!(node["node"]["name"], "render_invoice");
    assert_eq!(node["node"]["file"], "invoices.py");
    assert!(
        node["incoming"]["total"].as_u64().unwrap_or(0) >= 1,
        "render_invoice has a caller and a container: {node}"
    );

    // The extractor resolves calls within one file, so the call edge in the
    // fixture is render_invoice -> format_total.
    let (error, callers) = call(&GraphCallersTool, json!({"symbol": "format_total"}), &root).await;
    assert!(!error, "{callers}");
    assert!(
        names(&callers, "results").contains(&"render_invoice".to_string()),
        "render_invoice calls format_total: {callers}"
    );

    let (error, callees) = call(
        &GraphCalleesTool,
        json!({"symbol": "render_invoice"}),
        &root,
    )
    .await;
    assert!(!error, "{callees}");
    assert!(
        names(&callees, "results").contains(&"format_total".to_string()),
        "{callees}"
    );

    // Within a file: changing format_total affects render_invoice.
    let (error, impact) = call(&GraphImpactTool, json!({"symbol": "format_total"}), &root).await;
    assert!(!error, "{impact}");
    assert!(
        names(&impact, "affected").contains(&"render_invoice".to_string()),
        "changing format_total affects render_invoice: {impact}"
    );
    // Across files: report.py imports invoices.py, so changing the file
    // affects report.py. (The file node is addressed by its name.)
    let (error, file_impact) =
        call(&GraphImpactTool, json!({"symbol": "invoices.py"}), &root).await;
    assert!(!error, "{file_impact}");
    assert!(
        names(&file_impact, "affected").contains(&"report.py".to_string()),
        "report.py imports invoices.py: {file_impact}"
    );

    for file in ["invoices.py", "./invoices.py", "sub/../invoices.py"] {
        let (error, listing) = call(&GraphFileTool, json!({"file": file}), &root).await;
        assert!(!error, "{file}: {listing}");
        assert!(
            listing["symbols"]
                .as_array()
                .is_some_and(|s| s.iter().any(|n| n["name"] == "render_invoice")),
            "{file}: {listing}"
        );
    }

    let (error, path) = call(
        &GraphPathTool,
        json!({"from": "render_invoice", "to": "format_total"}),
        &root,
    )
    .await;
    assert!(!error, "{path}");
    assert_eq!(path["status"], "ok", "{path}");
    assert_eq!(path["hops"], 1, "{path}");
    assert_eq!(path["steps"][0]["then"], "CALLS", "{path}");

    let (error, imports) = call(
        &GraphPathTool,
        json!({"from": "report.py", "to": "invoices.py"}),
        &root,
    )
    .await;
    assert!(!error, "{imports}");
    assert_eq!(imports["steps"][0]["then"], "IMPORTS_FROM", "{imports}");

    let (error, none) = call(
        &GraphPathTool,
        json!({"from": "render_invoice", "to": "parse_settings"}),
        &root,
    )
    .await;
    assert!(!error, "{none}");
    assert_eq!(none["status"], "no_path", "{none}");

    let (error, overview) = call(&GraphOverviewTool, json!({}), &root).await;
    assert!(!error, "{overview}");
    assert!(overview["nodes"].as_i64().unwrap_or(0) >= 6, "{overview}");
    assert!(
        overview["edges_by_kind"]["CALLS"].as_i64().unwrap_or(0) >= 1,
        "{overview}"
    );
}

/// The tools never build a graph or create a file, and say what to do next.
#[tokio::test]
async fn without_a_graph_every_tool_says_so_and_creates_nothing() {
    let home = tempfile::tempdir().expect("isolated home");
    let _guard = crate::db::test_support::with_home(home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    write_invoice_workspace(workspace.path());
    let root = workspace.path().display().to_string();

    let inputs = json!({"query": "render", "symbol": "render_invoice",
                        "file": "invoices.py", "from": "a", "to": "b"});
    for tool in tools() {
        let (error, body) = call(tool.as_ref(), inputs.clone(), &root).await;
        assert!(error, "{} must report an error: {body}", tool.name());
        assert_eq!(
            body["error"]["code"],
            "graph_not_built",
            "{}: {body}",
            tool.name()
        );
        assert!(
            body["error"]["next_step"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "{}: {body}",
            tool.name()
        );
    }
    let graphs = crate::db::arcana_dir().expect("arcana dir").join("graphs");
    assert_eq!(
        std::fs::read_dir(&graphs).map(|d| d.count()).unwrap_or(0),
        0,
        "a graph tool created a database"
    );
}

fn node(name: &str, qualified: &str) -> NodeInfo {
    NodeInfo {
        kind: "Function".into(),
        name: name.into(),
        qualified_name: qualified.into(),
        file_path: format!("{qualified}.py"),
        line_start: 1,
        line_end: 2,
        language: "python".into(),
        parent_name: None,
        params: None,
        return_type: None,
        modifiers: None,
        is_test: false,
        file_hash: "h".into(),
    }
}

fn call_edge(from: &str, to: &str, line: i32) -> EdgeInfo {
    EdgeInfo {
        kind: "CALLS".into(),
        source_qualified: from.into(),
        target_qualified: to.into(),
        file_path: "f.py".into(),
        line,
        confidence: 1.0,
        confidence_tier: "certain".into(),
        weight: 1.0,
        relation: None,
    }
}

fn synthetic(nodes: &[NodeInfo], edges: &[EdgeInfo]) -> (tempfile::TempDir, String) {
    let workspace = tempfile::tempdir().expect("workspace");
    let location =
        crate::graph::location::locate(&workspace.path().display().to_string()).expect("locate");
    let store = GraphStore::open(&location.db_path).expect("open");
    store.upsert_nodes_batch(nodes, None, None).expect("nodes");
    store.upsert_edges_batch(edges, None, None).expect("edges");
    store.rebuild_fts_index().expect("fts");
    let root = location.root.display().to_string();
    (workspace, root)
}

#[tokio::test]
async fn a_shared_name_returns_candidates_and_an_unknown_name_is_an_error() {
    let home = tempfile::tempdir().expect("isolated home");
    let _guard = crate::db::test_support::with_home(home.path());
    let (_workspace, root) = synthetic(
        &[node("handle", "a::handle"), node("handle", "b::handle")],
        &[],
    );

    let (error, body) = call(&GraphNodeTool, json!({"symbol": "handle"}), &root).await;
    assert!(!error, "{body}");
    assert_eq!(body["status"], "ambiguous");
    assert_eq!(body["candidates"].as_array().map(Vec::len), Some(2));
    // Choosing one by its exact name resolves it.
    let (error, body) = call(&GraphNodeTool, json!({"symbol": "b::handle"}), &root).await;
    assert!(!error && body["status"] == "ok", "{body}");

    let (error, body) = call(&GraphNodeTool, json!({"symbol": "nothing_here"}), &root).await;
    assert!(error);
    assert_eq!(body["error"]["code"], "graph_symbol_not_found", "{body}");
}

#[tokio::test]
async fn hostile_input_is_data_and_missing_arguments_are_refused() {
    let home = tempfile::tempdir().expect("isolated home");
    let _guard = crate::db::test_support::with_home(home.path());
    let (_workspace, root) = synthetic(&[node("handle", "a::handle")], &[]);

    for query in [
        "handle\" OR NEAR(",
        "name:handle",
        "*",
        "AND OR NOT",
        "'; DROP TABLE nodes;--",
    ] {
        let (error, body) = call(&GraphSearchTool, json!({"query": query}), &root).await;
        assert!(
            !error || body["error"]["code"] == "invalid_input",
            "{query:?} must be data or a clean refusal: {body}"
        );
    }
    let (error, body) = call(&GraphSearchTool, json!({"query": "handle"}), &root).await;
    assert!(!error && body["count"] == 1, "{body}");

    let missing: Vec<(Box<dyn Tool>, Value)> = vec![
        (Box::new(GraphSearchTool), json!({})),
        (Box::new(GraphNodeTool), json!({"symbol": "   "})),
        (Box::new(GraphFileTool), json!({})),
        (Box::new(GraphPathTool), json!({"from": "a"})),
    ];
    for (tool, input) in missing {
        let (error, body) = call(tool.as_ref(), input, &root).await;
        assert!(error, "{} accepted missing arguments", tool.name());
        assert_eq!(body["error"]["code"], "invalid_input", "{body}");
    }
}

/// Output is bounded however large the graph or the requested limit.
#[tokio::test]
async fn results_are_capped_and_a_cut_is_reported() {
    let home = tempfile::tempdir().expect("isolated home");
    let _guard = crate::db::test_support::with_home(home.path());
    let mut nodes = vec![node("hub", "hub")];
    let mut edges = Vec::new();
    for i in 0..300 {
        let name = format!("caller_{i:03}");
        nodes.push(node(&name, &name));
        edges.push(call_edge(&name, "hub", i));
    }
    let (_workspace, root) = synthetic(&nodes, &edges);

    let (error, callers) = call(
        &GraphCallersTool,
        json!({"symbol": "hub", "limit": 9999}),
        &root,
    )
    .await;
    assert!(!error, "{callers}");
    assert_eq!(callers["total"], 300);
    assert_eq!(callers["results"].as_array().map(Vec::len), Some(50));
    assert_eq!(callers["truncated"], true);

    let (error, impact) = call(
        &GraphImpactTool,
        json!({"symbol": "hub", "depth": 99}),
        &root,
    )
    .await;
    assert!(!error, "{impact}");
    assert!(impact["depth"].as_u64() <= Some(5), "{impact}");
    assert_eq!(impact["truncated"], true, "{impact}");
    assert!(impact["affected"].as_array().map_or(0, Vec::len) <= 50);
}

/// P5-19: the byte cost of offering these tools is measured and bounded,
/// because every request an engine makes carries its definitions.
#[test]
fn the_tool_definitions_have_a_measured_budget() {
    let mut registry = ToolRegistry::new();
    register(&mut registry);
    let definitions = registry.to_definitions();
    assert_eq!(definitions.len(), GRAPH_TOOL_NAMES.len());
    let bytes: usize = definitions
        .iter()
        .map(|d| d.name.len() + d.description.len() + d.input_schema.to_string().len())
        .sum();
    eprintln!(
        "graph tool definitions: {bytes} bytes for {} tools",
        definitions.len()
    );
    assert!(bytes <= 5 * 1024, "{bytes} bytes of tool definitions");
    for definition in &definitions {
        assert!(
            definition.description.len() <= 400,
            "{} description is {} bytes",
            definition.name,
            definition.description.len()
        );
    }
}

#[test]
fn registering_matches_the_published_names_and_no_mode_registers_them_by_default() {
    let mut registry = ToolRegistry::new();
    register(&mut registry);
    for name in GRAPH_TOOL_NAMES {
        assert!(registry.get(name).is_some(), "{name} not registered");
    }
    for tool in tools() {
        assert!(
            GRAPH_TOOL_NAMES.contains(&tool.name()),
            "{} unlisted",
            tool.name()
        );
        assert_eq!(
            tool.effect(),
            ToolEffect::Read,
            "{} must be Read",
            tool.name()
        );
    }
    for mode in [
        crate::agent::PAMode::FastDevelopment,
        crate::agent::PAMode::Review,
        crate::agent::PAMode::Delegate,
    ] {
        let mut plain = ToolRegistry::new();
        super::register_for_mode(&mut plain, mode);
        for name in GRAPH_TOOL_NAMES {
            assert!(
                plain.get(name).is_none(),
                "{name} is registered for {mode:?} without opting in"
            );
        }
    }
}
