// Graph Engine — SQLite-persistent code knowledge graph
// Extracts AST nodes via tree-sitter, builds dependency graph, clusters communities,
// and serves surgical queries to agents via MCP.

pub mod analyze;
pub mod build;
pub mod cache;
pub mod changes;
pub mod cluster;
pub mod detect;
pub mod export;
pub mod extract;
pub mod flows;
pub mod incremental;
pub mod migrations;
pub mod query;
pub mod registry;
pub mod report;
pub mod schema;
pub mod store;
pub mod watcher;

pub use analyze::Analyzer;
pub use build::GraphBuilder;
pub use cluster::Clusterer;
pub use detect::CodeDetector;
pub use extract::{extract, extract_all, ConfidenceTier, EdgeKind, ExtractionResult, NodeKind};
pub use query::QueryEngine;
pub use registry::GraphRegistry;
pub use schema::{EdgeInfo, NodeInfo};
pub use store::{GraphEdge, GraphNode, GraphStats, GraphStore};
pub use watcher::GraphWatcher;
