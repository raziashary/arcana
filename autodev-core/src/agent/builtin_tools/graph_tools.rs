// Graph MCP tools — all 22 graph analysis and navigation tools

use crate::errors::Result;
use crate::graph::analyze::{
    get_impact_radius, god_nodes, graph_diff, suggest_questions, surprising_connections,
};
use crate::graph::changes::detect_changes;
use crate::graph::query::{query_by_name, query_dependencies, query_dependents};
use crate::graph::store::GraphStore;
use serde_json::{json, Value};

// ============================================================================
// DISCOVERY TOOLS (5)
// ============================================================================

pub struct GetMinimalContextTool;

impl GetMinimalContextTool {
    pub fn name() -> &'static str {
        "get_graph_minimal_context"
    }

    pub fn execute(store: &GraphStore, task: String) -> Result<Value> {
        let terms: Vec<&str> = task.split_whitespace().collect();

        // Score nodes against task terms
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
        let mut scored_nodes: Vec<_> = all_nodes
            .iter()
            .map(|n| {
                let mut score = 0.0;
                for term in &terms {
                    if n.name.to_lowercase().contains(&term.to_lowercase()) {
                        score += 1.0;
                    }
                    if n.file_path.to_lowercase().contains(&term.to_lowercase()) {
                        score += 0.5;
                    }
                }
                (score, n)
            })
            .filter(|(score, _)| *score > 0.0)
            .collect();

        scored_nodes.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let top_nodes: Vec<_> = scored_nodes
            .iter()
            .take(10)
            .map(|(_, n)| {
                json!({
                    "qualified_name": n.qualified_name,
                    "name": n.name,
                    "kind": n.kind,
                    "file": n.file_path,
                    "line": n.line_start,
                })
            })
            .collect();

        Ok(json!({
            "nodes": top_nodes,
            "next_tools": ["list_callers", "list_callees", "get_impact_radius"]
        }))
    }
}

pub struct GetArchitectureOverviewTool;

impl GetArchitectureOverviewTool {
    pub fn name() -> &'static str {
        "get_graph_architecture_overview"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        let mut communities = std::collections::HashMap::new();
        for node in &all_nodes {
            communities
                .entry(node.name.clone())
                .or_insert_with(Vec::new)
                .push(node.clone());
        }

        let overview: Vec<_> = communities
            .iter()
            .take(10)
            .map(|(name, members)| {
                json!({
                    "name": name,
                    "member_count": members.len(),
                    "nodes": members.iter().take(3).map(|n| n.name.clone()).collect::<Vec<_>>()
                })
            })
            .collect();

        Ok(json!({
            "communities": overview,
            "total_edges": all_edges.len(),
            "total_nodes": all_nodes.len()
        }))
    }
}

pub struct GetGraphStatsTool;

impl GetGraphStatsTool {
    pub fn name() -> &'static str {
        "get_graph_stats"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        let mut kind_counts = std::collections::HashMap::new();
        for node in &all_nodes {
            *kind_counts.entry(node.kind.clone()).or_insert(0) += 1;
        }

        let mut language_counts = std::collections::HashMap::new();
        for node in &all_nodes {
            *language_counts.entry(node.language.clone()).or_insert(0) += 1;
        }

        Ok(json!({
            "total_nodes": all_nodes.len(),
            "total_edges": all_edges.len(),
            "by_kind": kind_counts,
            "by_language": language_counts,
            "last_updated": chrono::Local::now().to_rfc3339()
        }))
    }
}

pub struct ListCommunitiesTool;

impl ListCommunitiesTool {
    pub fn name() -> &'static str {
        "list_graph_communities"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let mut communities = std::collections::HashMap::new();
        for node in &all_nodes {
            communities
                .entry(node.name.clone())
                .or_insert_with(Vec::new)
                .push(node.clone());
        }

        let mut community_list: Vec<_> = communities
            .into_iter()
            .map(|(name, members)| (name, members.len()))
            .collect();

        community_list.sort_by(|a, b| b.1.cmp(&a.1));

        let communities_json: Vec<_> = community_list
            .iter()
            .map(|(name, count)| {
                json!({
                    "name": name,
                    "member_count": count
                })
            })
            .collect();

        Ok(json!({
            "communities": communities_json,
            "total": communities_json.len()
        }))
    }
}

pub struct GetGraphReportTool;

impl GetGraphReportTool {
    pub fn name() -> &'static str {
        "get_graph_report"
    }

    pub fn execute(_store: &GraphStore) -> Result<Value> {
        // Read GRAPH_REPORT.md from disk
        let report_path = std::path::Path::new(".autodev/GRAPH_REPORT.md");
        let content = if report_path.exists() {
            std::fs::read_to_string(report_path)
                .unwrap_or_else(|_| "No report generated yet.".to_string())
        } else {
            "No report generated yet.".to_string()
        };

        Ok(json!({
            "report": content
        }))
    }
}

// ============================================================================
// NAVIGATION TOOLS (9)
// ============================================================================

pub struct QueryGraphTool;

impl QueryGraphTool {
    pub fn name() -> &'static str {
        "query_graph"
    }

    pub fn execute(store: &GraphStore, target: String, pattern: String) -> Result<Value> {
        let result = match pattern.as_str() {
            "callers_of" => query_dependents(store, &target)?,
            "callees_of" => query_dependencies(store, &target)?,
            "imports_of" => query_dependencies(store, &target)?,
            "dependencies_of" => query_dependencies(store, &target)?,
            _ => query_by_name(store, &target)?,
        };

        Ok(json!({
            "nodes": result.nodes,
            "edges": result.edges,
            "summary": result.summary
        }))
    }
}

pub struct KeywordSearchTool;

impl KeywordSearchTool {
    pub fn name() -> &'static str {
        "keyword_search_nodes"
    }

    pub fn execute(store: &GraphStore, query: String, top_k: u32) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let q = query.to_lowercase();
        let mut scored: Vec<_> = all_nodes
            .iter()
            .map(|n| {
                let mut score = 0.0;
                if n.name.to_lowercase().contains(&q) {
                    score += 2.0;
                }
                if n.file_path.to_lowercase().contains(&q) {
                    score += 1.0;
                }
                (score, n)
            })
            .filter(|(s, _)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let results: Vec<_> = scored
            .iter()
            .take(top_k as usize)
            .map(|(_, n)| {
                json!({
                    "qualified_name": n.qualified_name,
                    "name": n.name,
                    "kind": n.kind,
                    "file": n.file_path,
                    "line": n.line_start
                })
            })
            .collect();

        Ok(json!({
            "results": results,
            "count": results.len()
        }))
    }
}

pub struct GetNodeDetailsTool;

impl GetNodeDetailsTool {
    pub fn name() -> &'static str {
        "get_node_details"
    }

    pub fn execute(store: &GraphStore, qualified_name: String) -> Result<Value> {
        // `get_node` returns Option: absence is a normal answer, not an error
        // condition of the query. Report it as such rather than unwrapping —
        // this file has never compiled, so nothing here has ever been exercised.
        let node = store.get_node(&qualified_name)?.ok_or_else(|| {
            crate::errors::AutoDevError::Agent(format!(
                "No node named '{qualified_name}' exists in the graph"
            ))
        })?;
        let edges = store.get_edges_for_node(&qualified_name)?;

        let mut incoming = Vec::new();
        let mut outgoing = Vec::new();

        for edge in edges {
            if edge.target_qualified == qualified_name {
                incoming.push(json!({
                    "from": edge.source_qualified,
                    "kind": edge.kind,
                    "confidence": edge.confidence_tier
                }));
            } else {
                outgoing.push(json!({
                    "to": edge.target_qualified,
                    "kind": edge.kind,
                    "confidence": edge.confidence_tier
                }));
            }
        }

        Ok(json!({
            "node": {
                "qualified_name": node.qualified_name,
                "name": node.name,
                "kind": node.kind,
                "file": node.file_path,
                "line": node.line_start
            },
            "incoming": incoming,
            "outgoing": outgoing
        }))
    }
}

pub struct FindPathTool;

impl FindPathTool {
    pub fn name() -> &'static str {
        "find_graph_path"
    }

    pub fn execute(store: &GraphStore, source: String, target: String) -> Result<Value> {
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        // BFS — guarantees shortest-hop path between source and target
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((source.clone(), vec![source.clone()]));
        let mut visited = std::collections::HashSet::new();

        while let Some((current, path)) = queue.pop_front() {
            if current == target {
                return Ok(json!({
                    "path": path,
                    "hops": path.len() - 1
                }));
            }

            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            for edge in &all_edges {
                if edge.source_qualified == current && !visited.contains(&edge.target_qualified) {
                    let mut new_path = path.clone();
                    new_path.push(edge.target_qualified.clone());
                    queue.push_back((edge.target_qualified.clone(), new_path));
                }
            }
        }

        Ok(json!({
            "path": null,
            "error": "No path found"
        }))
    }
}

pub struct ExplainNodeTool;

impl ExplainNodeTool {
    pub fn name() -> &'static str {
        "explain_node"
    }

    pub fn execute(store: &GraphStore, label: String) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let node = all_nodes
            .iter()
            .find(|n| n.name.to_lowercase().contains(&label.to_lowercase()))
            .ok_or_else(|| {
                crate::errors::AutoDevError::Agent(format!("Node not found: {}", label))
            })?;

        let edges = store.get_edges_for_node(&node.qualified_name)?;
        let mut neighbors: Vec<_> = edges.iter().map(|e| e.clone()).collect();
        neighbors.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());

        let neighbor_list: Vec<_> = neighbors
            .iter()
            .take(20)
            .map(|e| json!({
                "node": if e.source_qualified == node.qualified_name { &e.target_qualified } else { &e.source_qualified },
                "relation": e.kind
            }))
            .collect();

        Ok(json!({
            "node": {
                "name": node.name,
                "kind": node.kind,
                "file": node.file_path,
                "line": node.line_start
            },
            "neighbors": neighbor_list,
            "degree": edges.len()
        }))
    }
}

pub struct GetFileSummaryTool;

impl GetFileSummaryTool {
    pub fn name() -> &'static str {
        "get_file_summary"
    }

    pub fn execute(store: &GraphStore, file_path: String) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let file_nodes: Vec<_> = all_nodes
            .iter()
            .filter(|n| n.file_path == file_path)
            .collect();

        let mut by_kind = std::collections::HashMap::new();
        for node in &file_nodes {
            *by_kind.entry(node.kind.clone()).or_insert(0) += 1;
        }

        let nodes_list: Vec<_> = file_nodes
            .iter()
            .map(|n| {
                json!({
                    "name": n.name,
                    "kind": n.kind,
                    "qualified_name": n.qualified_name
                })
            })
            .collect();

        Ok(json!({
            "file": file_path,
            "by_kind": by_kind,
            "nodes": nodes_list,
            "total": file_nodes.len()
        }))
    }
}

pub struct ListCallersTool;

impl ListCallersTool {
    pub fn name() -> &'static str {
        "list_callers"
    }

    pub fn execute(store: &GraphStore, qualified_name: String) -> Result<Value> {
        let result = query_dependents(store, &qualified_name)?;

        Ok(json!({
            "callers": result.nodes,
            "count": result.nodes.len()
        }))
    }
}

pub struct ListCalleesTool;

impl ListCalleesTool {
    pub fn name() -> &'static str {
        "list_callees"
    }

    pub fn execute(store: &GraphStore, qualified_name: String) -> Result<Value> {
        let result = query_dependencies(store, &qualified_name)?;

        Ok(json!({
            "callees": result.nodes,
            "count": result.nodes.len()
        }))
    }
}

pub struct ListTestsTool;

impl ListTestsTool {
    pub fn name() -> &'static str {
        "list_tests"
    }

    pub fn execute(store: &GraphStore, qualified_name: String) -> Result<Value> {
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        let tests: Vec<_> = all_edges
            .iter()
            .filter(|e| e.source_qualified == qualified_name && e.kind == "TestedBy")
            .map(|e| {
                json!({
                    "test": e.target_qualified,
                    "kind": e.kind
                })
            })
            .collect();

        Ok(json!({
            "tests": tests,
            "count": tests.len()
        }))
    }
}

// ============================================================================
// IMPACT TOOLS (5)
// ============================================================================

pub struct GetImpactRadiusTool;

impl GetImpactRadiusTool {
    pub fn name() -> &'static str {
        "get_impact_radius"
    }

    pub fn execute(
        store: &GraphStore,
        qualified_name: String,
        max_depth: Option<u32>,
    ) -> Result<Value> {
        let impact = get_impact_radius(store, &qualified_name, max_depth.unwrap_or(5), 100)?;

        Ok(json!({
            "changed_node": impact.changed_node,
            "affected_nodes": impact.affected_nodes,
            "tests_to_run": impact.tests_to_run,
            "test_coverage_gaps": impact.test_coverage_gaps,
            "total_affected": impact.total_affected,
            "high_risk_count": impact.high_risk_count
        }))
    }
}

pub struct DetectChangesTool;

impl DetectChangesTool {
    pub fn name() -> &'static str {
        "detect_graph_changes"
    }

    pub fn execute(store: &GraphStore, base: Option<String>) -> Result<Value> {
        let analysis = detect_changes(store, ".", &base.unwrap_or_else(|| "HEAD~1".to_string()))?;

        Ok(json!({
            "changed_nodes": analysis.changed_nodes,
            "affected_nodes": analysis.affected_nodes,
            "test_coverage_gaps": analysis.test_coverage_gaps,
            "total_risk_score": analysis.total_risk_score
        }))
    }
}

pub struct GetReviewContextTool;

impl GetReviewContextTool {
    pub fn name() -> &'static str {
        "get_review_context"
    }

    pub fn execute(store: &GraphStore, files: Vec<String>) -> Result<Value> {
        let mut context = String::new();

        for file in files {
            let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
            let file_nodes: Vec<_> = all_nodes.iter().filter(|n| n.file_path == file).collect();

            context.push_str(&format!("## {}\n", file));
            for node in file_nodes {
                context.push_str(&format!(
                    "- {} ({}:{})\n",
                    node.name, node.file_path, node.line_start
                ));
            }
        }

        Ok(json!({
            "context": context,
            "token_estimate": context.len() / 4
        }))
    }
}

pub struct GetAffectedFlowsTool;

impl GetAffectedFlowsTool {
    pub fn name() -> &'static str {
        "get_affected_flows"
    }

    pub fn execute(store: &GraphStore, file_path: String) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let file_nodes: Vec<_> = all_nodes
            .iter()
            .filter(|n| n.file_path == file_path)
            .map(|n| n.qualified_name.clone())
            .collect();

        Ok(json!({
            "file": file_path,
            "affected_nodes": file_nodes,
            "count": file_nodes.len()
        }))
    }
}

pub struct GetFlowCriticalityTool;

impl GetFlowCriticalityTool {
    pub fn name() -> &'static str {
        "get_flow_criticality"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        let mut flows: Vec<_> = all_edges
            .iter()
            .map(|e| {
                json!({
                    "source": e.source_qualified,
                    "target": e.target_qualified,
                    "kind": e.kind,
                    "criticality": e.weight
                })
            })
            .collect();

        // Sort by weight (criticality)
        flows.sort_by(|a, b| {
            b["criticality"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["criticality"].as_f64().unwrap_or(0.0))
                .unwrap()
        });

        Ok(json!({
            "flows": flows.iter().take(20).collect::<Vec<_>>(),
            "total": flows.len()
        }))
    }
}

// ============================================================================
// ANALYSIS TOOLS (4)
// ============================================================================

pub struct GetGodNodesTool;

impl GetGodNodesTool {
    pub fn name() -> &'static str {
        "get_god_nodes"
    }

    pub fn execute(store: &GraphStore, top_n: Option<u32>) -> Result<Value> {
        let nodes = god_nodes(store, top_n.unwrap_or(10) as usize)?;

        let nodes_json: Vec<_> = nodes
            .iter()
            .map(|n| {
                json!({
                    "name": n.label,
                    "qualified_name": n.qualified_name,
                    "degree": n.degree,
                    "file": n.file_path
                })
            })
            .collect();

        Ok(json!({
            "god_nodes": nodes_json,
            "count": nodes_json.len()
        }))
    }
}

pub struct GetSurprisingConnectionsTool;

impl GetSurprisingConnectionsTool {
    pub fn name() -> &'static str {
        "get_surprising_connections"
    }

    pub fn execute(store: &GraphStore, top_n: Option<u32>) -> Result<Value> {
        let connections = surprising_connections(store, top_n.unwrap_or(5) as usize)?;

        let connections_json: Vec<_> = connections
            .iter()
            .map(|c| {
                json!({
                    "source": c.source,
                    "target": c.target,
                    "score": c.score,
                    "why": c.why
                })
            })
            .collect();

        Ok(json!({
            "connections": connections_json,
            "count": connections_json.len()
        }))
    }
}

pub struct GetSuggestedQuestionsTool;

impl GetSuggestedQuestionsTool {
    pub fn name() -> &'static str {
        "get_suggested_questions"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let questions = suggest_questions(store, 5)?;

        let questions_json: Vec<_> = questions
            .iter()
            .map(|q| {
                json!({
                    "question": q.question,
                    "type": q.question_type,
                    // `SuggestedQuestion` carries question_type/question/why and no
                    // confidence at all (graph/analyze.rs). The original read
                    // `q.confidence_tier`, which does not exist; emitting a
                    // "confidence" key here would invent a number the analyser
                    // never produced, so the field it does have is surfaced.
                    "why": q.why
                })
            })
            .collect();

        Ok(json!({
            "questions": questions_json,
            "count": questions_json.len()
        }))
    }
}

pub struct GetGraphDiffTool;

impl GetGraphDiffTool {
    pub fn name() -> &'static str {
        "get_graph_diff"
    }

    pub fn execute(store: &GraphStore, snapshot_id_a: i64, snapshot_id_b: i64) -> Result<Value> {
        let diff = graph_diff(store, snapshot_id_a, snapshot_id_b)?;
        Ok(json!({
            "snapshot_a": snapshot_id_a,
            "snapshot_b": snapshot_id_b,
            "new_nodes": diff.new_nodes,
            "removed_nodes": diff.removed_nodes,
            "new_edges": diff.new_edges,
            "removed_edges": diff.removed_edges,
            "summary": diff.summary,
            "counts": {
                "new_nodes": diff.new_nodes.len(),
                "removed_nodes": diff.removed_nodes.len(),
                "new_edges": diff.new_edges.len(),
                "removed_edges": diff.removed_edges.len(),
            }
        }))
    }
}

// ============================================================================
// AUTODEV-NATIVE TOOLS (4)
// ============================================================================

pub struct GetWorkerGraphTool;

impl GetWorkerGraphTool {
    pub fn name() -> &'static str {
        "get_worker_graph"
    }

    pub fn execute(store: &GraphStore, worker_name: String) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let worker_nodes: Vec<_> = all_nodes
            .iter()
            .filter(|n| {
                n.agent_author
                    .as_ref()
                    .map(|a| a == &worker_name)
                    .unwrap_or(false)
            })
            .map(|n| {
                json!({
                    "name": n.name,
                    "qualified_name": n.qualified_name,
                    "file": n.file_path
                })
            })
            .collect();

        Ok(json!({
            "worker": worker_name,
            "nodes": worker_nodes,
            "count": worker_nodes.len()
        }))
    }
}

pub struct GetTaskGraphTool;

impl GetTaskGraphTool {
    pub fn name() -> &'static str {
        "get_task_graph"
    }

    pub fn execute(store: &GraphStore, task_id: i64) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let task_nodes: Vec<_> = all_nodes
            .iter()
            .filter(|n| n.task_id.map(|t| t == task_id).unwrap_or(false))
            .map(|n| {
                json!({
                    "name": n.name,
                    "qualified_name": n.qualified_name,
                    "file": n.file_path
                })
            })
            .collect();

        Ok(json!({
            "task_id": task_id,
            "nodes": task_nodes,
            "count": task_nodes.len()
        }))
    }
}

pub struct GetLockedNodesTool;

impl GetLockedNodesTool {
    pub fn name() -> &'static str {
        "get_locked_nodes"
    }

    pub fn execute(store: &GraphStore) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

        let locked_nodes: Vec<_> = all_nodes
            .iter()
            .filter(|n| n.worker_locked)
            .map(|n| {
                json!({
                    "name": n.name,
                    "qualified_name": n.qualified_name,
                    "file": n.file_path,
                    "locked_by": n.agent_author.clone()
                })
            })
            .collect();

        Ok(json!({
            "locked_nodes": locked_nodes,
            "count": locked_nodes.len()
        }))
    }
}

pub struct GetGraphStatusTool;

impl GetGraphStatusTool {
    pub fn name() -> &'static str {
        "get_graph_status"
    }

    pub fn execute(store: &GraphStore, _repo_path: String) -> Result<Value> {
        let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
        let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;

        Ok(json!({
            "status": "success",
            "node_count": all_nodes.len(),
            "edge_count": all_edges.len(),
            "timestamp": chrono::Local::now().to_rfc3339()
        }))
    }
}
