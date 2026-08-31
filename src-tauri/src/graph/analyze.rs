// Analysis layer — graph algorithms, metrics, and insights

use super::store::GraphStore;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNode {
    pub qualified_name: String,
    pub label: String,
    pub degree: u32,
    pub community: Option<i32>,
    pub community_name: Option<String>,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurprisingConnection {
    pub source: String,
    pub target: String,
    pub source_file: String,
    pub target_file: String,
    pub confidence_tier: String,
    pub relation: String,
    pub score: f32,
    pub why: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedQuestion {
    pub question_type: String,
    pub question: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffectedNode {
    pub qualified_name: String,
    pub label: String,
    pub file_path: String,
    pub hops: u32,
    pub risk_level: String,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRadius {
    pub changed_node: String,
    pub affected_nodes: Vec<AffectedNode>,
    pub tests_to_run: Vec<String>,
    pub test_coverage_gaps: Vec<String>,
    pub total_affected: usize,
    pub high_risk_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSummary {
    pub qualified_name: String,
    pub label: String,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSummary {
    pub source: String,
    pub target: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphDiff {
    pub new_nodes: Vec<NodeSummary>,
    pub removed_nodes: Vec<NodeSummary>,
    pub new_edges: Vec<EdgeSummary>,
    pub removed_edges: Vec<EdgeSummary>,
    pub summary: String,
}

pub fn god_nodes(store: &GraphStore, top_n: usize) -> Result<Vec<GodNode>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

    let mut degree_map: HashMap<String, u32> = HashMap::new();
    for edge in &all_edges {
        *degree_map.entry(edge.source_qualified.clone()).or_insert(0) += 1;
        *degree_map.entry(edge.target_qualified.clone()).or_insert(0) += 1;
    }

    let node_map: HashMap<String, _> = all_nodes
        .iter()
        .map(|n| (n.qualified_name.clone(), n.clone()))
        .collect();

    let mut god_nodes_list: Vec<_> = degree_map
        .iter()
        .filter_map(|(qname, &degree)| {
            if let Some(node) = node_map.get(qname) {
                // Filter out file-hub nodes and AST stubs
                if !qname.ends_with(&format!(".{}", node.name)) && !node.name.starts_with('.') {
                    return Some(GodNode {
                        qualified_name: qname.clone(),
                        label: node.name.clone(),
                        degree,
                        community: None,
                        community_name: None,
                        file_path: node.file_path.clone(),
                    });
                }
            }
            None
        })
        .collect();

    god_nodes_list.sort_by(|a, b| b.degree.cmp(&a.degree));
    god_nodes_list.truncate(top_n);
    Ok(god_nodes_list)
}

pub fn surprising_connections(
    store: &GraphStore,
    top_n: usize,
) -> Result<Vec<SurprisingConnection>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

    let node_map: HashMap<String, _> = all_nodes
        .iter()
        .map(|n| (n.qualified_name.clone(), n.clone()))
        .collect();

    let mut connections = Vec::new();

    for edge in all_edges {
        let source_file = node_map
            .get(&edge.source_qualified)
            .map(|n| n.file_path.clone())
            .unwrap_or_default();
        let target_file = node_map
            .get(&edge.target_qualified)
            .map(|n| n.file_path.clone())
            .unwrap_or_default();

        // Skip same-file and import edges
        if source_file == target_file || edge.kind.contains("Import") || edge.kind == "Contains" {
            continue;
        }

        let mut score = 0.0;
        let mut why = Vec::new();

        // Confidence bonus
        match edge.confidence_tier.as_str() {
            "Ambiguous" => {
                score += 3.0;
                why.push("ambiguous relationship".to_string());
            }
            "Inferred" => {
                score += 2.0;
                why.push("inferred relationship".to_string());
            }
            "Extracted" => score += 1.0,
            _ => {}
        }

        // Cross-type bonus
        let source_ext = source_file.split('.').last().unwrap_or("");
        let target_ext = target_file.split('.').last().unwrap_or("");
        if (source_ext == "ipynb") != (target_ext == "ipynb") {
            score += 2.0;
            why.push("cross-type connection (code ↔ notebook)".to_string());
        }

        // Cross-repo bonus
        let source_top = source_file.split('/').next().unwrap_or("");
        let target_top = target_file.split('/').next().unwrap_or("");
        if source_top != target_top {
            score += 2.0;
            why.push("cross-module connection".to_string());
        }

        if score > 0.0 {
            connections.push(SurprisingConnection {
                source: edge.source_qualified,
                target: edge.target_qualified,
                source_file,
                target_file,
                confidence_tier: edge.confidence_tier,
                relation: edge.kind,
                score,
                why,
            });
        }
    }

    connections.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    connections.truncate(top_n);
    Ok(connections)
}

pub fn suggest_questions(store: &GraphStore, top_n: usize) -> Result<Vec<SuggestedQuestion>> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

    let mut questions = Vec::new();

    // 1. Ambiguous edges
    let ambiguous: Vec<_> = all_edges
        .iter()
        .filter(|e| e.confidence_tier == "Ambiguous")
        .take(5)
        .collect();

    for edge in ambiguous {
        questions.push(SuggestedQuestion {
            question_type: "ambiguous_relationship".to_string(),
            question: format!(
                "What is the exact relationship between '{}' and '{}'?",
                edge.source_qualified, edge.target_qualified
            ),
            why: "This edge has low confidence and needs verification".to_string(),
        });
    }

    // 2. Isolated nodes
    let mut degree_map: HashMap<String, u32> = HashMap::new();
    for edge in &all_edges {
        *degree_map.entry(edge.source_qualified.clone()).or_insert(0) += 1;
        *degree_map.entry(edge.target_qualified.clone()).or_insert(0) += 1;
    }

    let isolated: Vec<_> = all_nodes
        .iter()
        .filter(|n| degree_map.get(&n.qualified_name).copied().unwrap_or(0) <= 1)
        .take(3)
        .collect();

    if !isolated.is_empty() {
        let names = isolated
            .iter()
            .map(|n| n.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        questions.push(SuggestedQuestion {
            question_type: "isolated_nodes".to_string(),
            question: format!("What connects {} to the rest of the system?", names),
            why: "These nodes have very few connections and may be unused or undiscovered"
                .to_string(),
        });
    }

    // 3. Inferred edges on god nodes
    let god = god_nodes(store, 5)?;
    for node in god.iter().take(2) {
        let inferred_count = all_edges
            .iter()
            .filter(|e| {
                (e.source_qualified == node.qualified_name
                    || e.target_qualified == node.qualified_name)
                    && e.confidence_tier == "Inferred"
            })
            .count();

        if inferred_count > 0 {
            questions.push(SuggestedQuestion {
                question_type: "inferred_god_node".to_string(),
                question: format!(
                    "Are the {} inferred relationships involving '{}' actually correct?",
                    inferred_count, node.label
                ),
                why: "This highly-connected node has many inferred edges that should be verified"
                    .to_string(),
            });
        }
    }

    questions.truncate(top_n);
    Ok(questions)
}

pub fn get_impact_radius(
    store: &GraphStore,
    qualified_name: &str,
    max_depth: u32,
    max_nodes: usize,
) -> Result<ImpactRadius> {
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;

    let node_map: HashMap<String, _> = all_nodes
        .iter()
        .map(|n| (n.qualified_name.clone(), n.clone()))
        .collect();

    let mut affected = HashMap::new();
    let mut queue = VecDeque::new();
    queue.push_back((qualified_name.to_string(), 0u32));
    affected.insert(
        qualified_name.to_string(),
        (0u32, vec![qualified_name.to_string()]),
    );

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= max_depth || affected.len() >= max_nodes {
            break;
        }

        // Find callers/dependents
        for edge in &all_edges {
            if edge.target_qualified == current
                && (edge.kind == "Calls" || edge.kind == "ImportsFrom" || edge.kind == "DependsOn")
            {
                let next = edge.source_qualified.clone();
                if !affected.contains_key(&next) {
                    let mut path = affected[&current].1.clone();
                    path.push(next.clone());
                    affected.insert(next.clone(), (depth + 1, path.clone()));
                    queue.push_back((next, depth + 1));
                }
            }
        }
    }

    let mut affected_nodes = Vec::new();
    let mut high_risk_count = 0;

    for (qname, (hops, path)) in affected.iter() {
        if qname == qualified_name {
            continue;
        }

        let risk_level = if *hops <= 2 {
            high_risk_count += 1;
            "high"
        } else if *hops <= 4 {
            "medium"
        } else {
            "low"
        };

        if let Some(node) = node_map.get(qname) {
            affected_nodes.push(AffectedNode {
                qualified_name: qname.clone(),
                label: node.name.clone(),
                file_path: node.file_path.clone(),
                hops: *hops,
                risk_level: risk_level.to_string(),
                path: path.clone(),
            });
        }
    }

    // Find tests
    let tests_to_run: Vec<String> = all_edges
        .iter()
        .filter(|e| e.kind == "TestedBy" && affected.contains_key(&e.source_qualified))
        .map(|e| e.target_qualified.clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Find coverage gaps
    let test_coverage_gaps: Vec<String> = affected_nodes
        .iter()
        .filter(|n| {
            !all_edges
                .iter()
                .any(|e| e.kind == "TestedBy" && e.source_qualified == n.qualified_name)
        })
        .map(|n| n.qualified_name.clone())
        .collect();

    Ok(ImpactRadius {
        changed_node: qualified_name.to_string(),
        affected_nodes,
        tests_to_run,
        test_coverage_gaps,
        total_affected: affected.len() - 1,
        high_risk_count,
    })
}

/// Persist a labeled snapshot of the entire graph (nodes + edges) to the
/// `snapshots` table as gzip-compressed JSON (base64-encoded for safe TEXT storage).
/// Returns the new snapshot's row id.
pub fn save_snapshot(store: &GraphStore, label: &str) -> Result<i64> {
    use base64::Engine;
    use std::io::Write;

    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    let all_edges = store.get_all_edges_for_graph(i64::MAX, 0)?;
    let node_count = all_nodes.len() as i64;
    let edge_count = all_edges.len() as i64;

    let snapshot_data = serde_json::json!({
        "label": label,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "nodes": all_nodes,
        "edges": all_edges,
    });

    // Compress the JSON to keep snapshot rows compact
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(snapshot_data.to_string().as_bytes())?;
    let compressed = encoder.finish()?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&compressed);

    let id = store.insert_snapshot(label, node_count, edge_count, &encoded)?;
    tracing::info!(
        "[save_snapshot] Saved snapshot id={} label={} nodes={} edges={} bytes={}",
        id,
        label,
        node_count,
        edge_count,
        compressed.len()
    );
    Ok(id)
}

/// Helper: load and decompress a snapshot back into (nodes, edges).
fn load_snapshot(
    store: &GraphStore,
    snapshot_id: i64,
) -> Result<(Vec<NodeSummary>, Vec<EdgeSummary>)> {
    use base64::Engine;
    use std::io::Read;

    let encoded = store
        .get_snapshot_json(snapshot_id)?
        .ok_or_else(|| anyhow::anyhow!("Snapshot {} not found", snapshot_id))?;

    let compressed = base64::engine::general_purpose::STANDARD
        .decode(&encoded)
        .map_err(|e| anyhow::anyhow!("Snapshot base64 decode failed: {}", e))?;

    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
    let mut json = String::new();
    decoder.read_to_string(&mut json)?;

    let parsed: serde_json::Value = serde_json::from_str(&json)?;

    let nodes: Vec<NodeSummary> = parsed
        .get("nodes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|n| {
                    Some(NodeSummary {
                        qualified_name: n.get("qualified_name")?.as_str()?.to_string(),
                        label: n.get("name")?.as_str()?.to_string(),
                        file_path: n.get("file_path")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let edges: Vec<EdgeSummary> = parsed
        .get("edges")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(EdgeSummary {
                        source: e.get("source_qualified")?.as_str()?.to_string(),
                        target: e.get("target_qualified")?.as_str()?.to_string(),
                        kind: e.get("kind")?.as_str()?.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok((nodes, edges))
}

/// Compute the set difference between two snapshots: what was added vs removed
/// in `snapshot_id_b` relative to `snapshot_id_a`.
pub fn graph_diff(store: &GraphStore, snapshot_id_a: i64, snapshot_id_b: i64) -> Result<GraphDiff> {
    let (nodes_a, edges_a) = load_snapshot(store, snapshot_id_a)?;
    let (nodes_b, edges_b) = load_snapshot(store, snapshot_id_b)?;

    // Index by stable identity for O(1) membership checks
    let qnames_a: HashSet<String> = nodes_a.iter().map(|n| n.qualified_name.clone()).collect();
    let qnames_b: HashSet<String> = nodes_b.iter().map(|n| n.qualified_name.clone()).collect();

    // Edges identified by (source, target, kind) triple
    let edge_key = |e: &EdgeSummary| format!("{}|{}|{}", e.source, e.target, e.kind);
    let edge_keys_a: HashSet<String> = edges_a.iter().map(edge_key).collect();
    let edge_keys_b: HashSet<String> = edges_b.iter().map(edge_key).collect();

    let new_nodes: Vec<NodeSummary> = nodes_b
        .iter()
        .filter(|n| !qnames_a.contains(&n.qualified_name))
        .cloned()
        .collect();
    let removed_nodes: Vec<NodeSummary> = nodes_a
        .iter()
        .filter(|n| !qnames_b.contains(&n.qualified_name))
        .cloned()
        .collect();
    let new_edges: Vec<EdgeSummary> = edges_b
        .iter()
        .filter(|e| !edge_keys_a.contains(&edge_key(e)))
        .cloned()
        .collect();
    let removed_edges: Vec<EdgeSummary> = edges_a
        .iter()
        .filter(|e| !edge_keys_b.contains(&edge_key(e)))
        .cloned()
        .collect();

    let summary = format!(
        "{} nodes added, {} removed; {} edges added, {} removed",
        new_nodes.len(),
        removed_nodes.len(),
        new_edges.len(),
        removed_edges.len(),
    );

    Ok(GraphDiff {
        new_nodes,
        removed_nodes,
        new_edges,
        removed_edges,
        summary,
    })
}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze_graph(_graph_data: &str) -> Result<()> {
        // TODO: Implement graph analysis
        Ok(())
    }
}
