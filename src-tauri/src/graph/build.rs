// Build layer — orchestrates detection, extraction, and storage

use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::store::GraphStore;
use super::extract::{ExtractionResult, NodeKind, EdgeKind, ConfidenceTier};
use super::schema::{NodeInfo as SchemaNodeInfo, EdgeInfo as SchemaEdgeInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStats {
    pub nodes_upserted: usize,
    pub edges_upserted: usize,
    pub files_parsed: usize,
    pub elapsed_ms: u64,
}

pub struct GraphBuilder;

impl GraphBuilder {
    pub fn build_graph(_repo_path: &str) -> Result<()> {
        // TODO: Implement full graph build pipeline
        Ok(())
    }
}

pub fn build(
    extractions: Vec<ExtractionResult>,
    store: &GraphStore,
    agent_author: Option<&str>,
    task_id: Option<i64>,
) -> Result<BuildStats> {
    let start = std::time::Instant::now();
    let mut nodes_upserted = 0;
    let mut edges_upserted = 0;
    let files_parsed = extractions.len();
    
    // Collect all nodes and edges
    let mut all_nodes = Vec::new();
    let mut all_edges = Vec::new();
    
    for result in extractions {
        all_nodes.extend(result.nodes);
        all_edges.extend(result.edges);
    }
    
    // Deduplicate nodes
    deduplicate_nodes(&mut all_nodes);
    
    // Build set of node IDs for reconciliation
    let node_ids: HashSet<String> = all_nodes
        .iter()
        .map(|n| n.qualified_name.clone())
        .collect();
    
    // Reconcile edges
    reconcile_edges(&mut all_edges, &node_ids);
    
    // Batch upsert nodes to SQLite using a single connection+transaction
    tracing::info!("[GraphBuild] Upserting {} nodes in batch...", all_nodes.len());
    let schema_nodes: Vec<SchemaNodeInfo> = all_nodes.into_iter().map(|node| SchemaNodeInfo {
        kind: format!("{:?}", node.kind),
        name: node.name,
        qualified_name: node.qualified_name,
        file_path: node.file_path,
        line_start: node.line_start,
        line_end: node.line_end,
        language: node.language,
        parent_name: node.parent_name,
        params: node.params,
        return_type: node.return_type,
        modifiers: node.modifiers,
        is_test: node.is_test,
        file_hash: node.file_hash,
    }).collect();
    store.upsert_nodes_batch(&schema_nodes, agent_author, task_id)?;
    nodes_upserted = schema_nodes.len();
    tracing::info!("[GraphBuild] Upserted {} nodes", nodes_upserted);
    
    // Batch upsert edges to SQLite using a single connection+transaction
    tracing::info!("[GraphBuild] Upserting {} edges in batch...", all_edges.len());
    let schema_edges: Vec<SchemaEdgeInfo> = all_edges.into_iter().map(|edge| SchemaEdgeInfo {
        kind: format!("{:?}", edge.kind),
        source_qualified: edge.source_qualified,
        target_qualified: edge.target_qualified,
        file_path: edge.file_path,
        line: edge.line,
        confidence: edge.confidence,
        confidence_tier: format!("{:?}", edge.confidence_tier),
        weight: edge.weight,
        relation: None,
    }).collect();
    store.upsert_edges_batch(&schema_edges, agent_author, task_id)?;
    edges_upserted = schema_edges.len();
    tracing::info!("[GraphBuild] Upserted {} edges", edges_upserted);
    
    // Rebuild FTS5 index
    tracing::info!("[GraphBuild] Rebuilding FTS5 index...");
    store.rebuild_fts_index()?;
    tracing::info!("[GraphBuild] FTS5 index rebuilt");
    
    // Flush all writes to disk
    tracing::info!("[GraphBuild] Flushing writes to disk...");
    store.flush()?;
    tracing::info!("[GraphBuild] Writes flushed");
    
    let elapsed_ms = start.elapsed().as_millis() as u64;
    
    Ok(BuildStats {
        nodes_upserted,
        edges_upserted,
        files_parsed,
        elapsed_ms,
    })
}

pub fn deduplicate_nodes(nodes: &mut Vec<super::extract::NodeInfo>) {
    use std::collections::HashMap;
    
    // Dedupe by qualified_name (unique per file+symbol), NOT by name alone.
    // Previous bug: deduping by `name` collapsed all files named "core.py" etc.
    let mut seen = HashMap::new();
    nodes.retain(|node| {
        let key = node.qualified_name.clone();
        if seen.contains_key(&key) {
            false
        } else {
            seen.insert(key, true);
            true
        }
    });
}

pub fn reconcile_edges(
    edges: &mut Vec<super::extract::EdgeInfo>,
    node_ids: &HashSet<String>,
) {
    edges.retain(|edge| {
        let source_exists = node_ids.contains(&edge.source_qualified)
            || node_ids.contains(&super::extract::normalize_id(&edge.source_qualified));
        let target_exists = node_ids.contains(&edge.target_qualified)
            || node_ids.contains(&super::extract::normalize_id(&edge.target_qualified));
        
        source_exists && target_exists
    });
}

pub fn prune_deleted_files(
    store: &GraphStore,
    current_files: &HashSet<String>,
) -> Result<usize> {
    let stats = store.get_stats()?;
    
    // Get all distinct file paths from nodes
    let mut all_file_paths = HashSet::new();
    let all_nodes = store.get_all_nodes_for_graph(i64::MAX, 0)?;
    
    for node in all_nodes {
        all_file_paths.insert(node.file_path);
    }
    
    // Find files to delete
    let mut total_pruned = 0;
    for file_path in all_file_paths {
        if !current_files.contains(&file_path) {
            let nodes_deleted = store.delete_file_nodes(&file_path)?;
            let _ = store.delete_file_edges(&file_path)?;
            total_pruned += nodes_deleted;
        }
    }
    
    Ok(total_pruned)
}
