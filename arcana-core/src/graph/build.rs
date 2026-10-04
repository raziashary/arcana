// Build layer — orchestrates detection, extraction, and storage

use super::extract::{ConfidenceTier, EdgeKind, ExtractionResult, NodeKind};
use super::schema::{EdgeInfo as SchemaEdgeInfo, NodeInfo as SchemaNodeInfo};
use super::store::GraphStore;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
    let files_parsed = extractions.len();
    let mut operation = crate::observability::timing::OperationTimer::start("graph_build")
        .with_database("graph")
        .with_task_id(task_id);

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
    let node_ids: HashSet<String> = all_nodes.iter().map(|n| n.qualified_name.clone()).collect();

    // Reconcile edges
    reconcile_edges(&mut all_edges, &node_ids);

    // Batch upsert nodes to SQLite using a single connection+transaction
    tracing::info!(
        "[GraphBuild] Upserting {} nodes in batch...",
        all_nodes.len()
    );
    let schema_nodes: Vec<SchemaNodeInfo> = all_nodes
        .into_iter()
        .map(|node| SchemaNodeInfo {
            kind: format!("{}", node.kind),
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
        })
        .collect();
    let nodes_upserted = schema_nodes.len();
    let mut node_write = crate::observability::timing::OperationTimer::start("graph_nodes_upsert")
        .with_database("graph")
        .with_task_id(task_id)
        .with_result_count(nodes_upserted);
    store.upsert_nodes_batch(&schema_nodes, agent_author, task_id)?;
    node_write.succeed();
    tracing::info!("[GraphBuild] Upserted {} nodes", nodes_upserted);

    // Batch upsert edges to SQLite using a single connection+transaction
    tracing::info!(
        "[GraphBuild] Upserting {} edges in batch...",
        all_edges.len()
    );
    let schema_edges: Vec<SchemaEdgeInfo> = all_edges
        .into_iter()
        .map(|edge| SchemaEdgeInfo {
            kind: format!("{}", edge.kind),
            source_qualified: edge.source_qualified,
            target_qualified: edge.target_qualified,
            file_path: edge.file_path,
            line: edge.line,
            confidence: edge.confidence,
            confidence_tier: format!("{}", edge.confidence_tier),
            weight: edge.weight,
            relation: None,
        })
        .collect();
    let edges_upserted = schema_edges.len();
    let mut edge_write = crate::observability::timing::OperationTimer::start("graph_edges_upsert")
        .with_database("graph")
        .with_task_id(task_id)
        .with_result_count(edges_upserted);
    store.upsert_edges_batch(&schema_edges, agent_author, task_id)?;
    edge_write.succeed();
    tracing::info!("[GraphBuild] Upserted {} edges", edges_upserted);

    // Rebuild FTS5 index
    tracing::info!("[GraphBuild] Rebuilding FTS5 index...");
    let mut fts_rebuild = crate::observability::timing::OperationTimer::start("graph_fts_rebuild")
        .with_database("graph")
        .with_task_id(task_id);
    store.rebuild_fts_index_for_task(task_id)?;
    fts_rebuild.succeed();
    tracing::info!("[GraphBuild] FTS5 index rebuilt");

    // Flush all writes to disk
    tracing::info!("[GraphBuild] Flushing writes to disk...");
    let mut flush = crate::observability::timing::OperationTimer::start("graph_flush")
        .with_database("graph")
        .with_task_id(task_id);
    store.flush_for_task(task_id)?;
    flush.succeed();
    tracing::info!("[GraphBuild] Writes flushed");

    let elapsed_ms = start.elapsed().as_millis() as u64;

    operation = operation.with_result_count(nodes_upserted + edges_upserted);
    operation.succeed();

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

#[cfg(test)]
mod observability_tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn production_graph_build_emits_correlated_operation_outcomes() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("arcana-graph-observability-{unique}"));
        std::fs::create_dir_all(&dir).expect("isolated graph fixture directory");
        let store = GraphStore::open(&dir.join("graph.db")).expect("open isolated graph DB");

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            build(Vec::new(), &store, Some("test-worker"), Some(42))
                .expect("empty production graph build succeeds");
        });

        let output = String::from_utf8(output.lock().unwrap().clone()).expect("JSON event bytes");
        let events = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect::<Vec<_>>();
        for operation in [
            "graph_nodes_upsert",
            "graph_edges_upsert",
            "graph_fts_rebuild",
            "graph_flush",
            "graph_build",
        ] {
            let event = events
                .iter()
                .find(|event| event["fields"]["operation"] == operation)
                .unwrap_or_else(|| panic!("missing {operation} event: {events:?}"));
            assert_eq!(event["fields"]["database"], "graph");
            assert_eq!(event["fields"]["outcome"], "success");
            assert_eq!(event["fields"]["task_id"], "42", "event: {event:?}");
            assert!(event["fields"]["duration_ns"].as_u64().is_some());
        }
        assert_eq!(
            events
                .iter()
                .find(|event| event["fields"]["operation"] == "graph_nodes_upsert")
                .unwrap()["fields"]["result_count"],
            0
        );

        drop(store);
        std::fs::remove_dir_all(&dir).expect("remove isolated graph fixture");
    }
}

pub fn reconcile_edges(edges: &mut Vec<super::extract::EdgeInfo>, node_ids: &HashSet<String>) {
    edges.retain(|edge| {
        let source_exists = node_ids.contains(&edge.source_qualified)
            || node_ids.contains(&super::extract::normalize_id(&edge.source_qualified));
        let target_exists = node_ids.contains(&edge.target_qualified)
            || node_ids.contains(&super::extract::normalize_id(&edge.target_qualified));

        source_exists && target_exists
    });
}

pub fn prune_deleted_files(store: &GraphStore, current_files: &HashSet<String>) -> Result<usize> {
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
