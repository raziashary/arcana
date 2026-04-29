// Graph Engine — SQLite-persistent code knowledge graph
// Extracts AST nodes via tree-sitter, builds dependency graph, clusters communities,
// and serves surgical queries to agents via MCP.

pub mod schema;
pub mod migrations;
pub mod store;
pub mod detect;
pub mod extract;
pub mod build;
pub mod cluster;
pub mod analyze;
pub mod flows;
pub mod changes;
pub mod incremental;
pub mod query;
pub mod report;
pub mod export;
pub mod registry;
pub mod cache;
pub mod watcher;

pub use store::{GraphStore, GraphNode, GraphEdge, GraphStats};
pub use schema::{NodeInfo, EdgeInfo};
pub use detect::CodeDetector;
pub use extract::{ExtractionResult, NodeKind, EdgeKind, ConfidenceTier, extract, extract_all};
pub use build::GraphBuilder;
pub use cluster::Clusterer;
pub use analyze::Analyzer;
pub use query::QueryEngine;
pub use registry::GraphRegistry;
pub use watcher::GraphWatcher;
