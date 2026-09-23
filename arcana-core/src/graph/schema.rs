// Graph schema — SQL DDL constants and data structures

use serde::{Deserialize, Serialize};

// ── Data Structures ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub kind: String,           // function, class, module, variable, etc.
    pub name: String,           // simple name
    pub qualified_name: String, // full path (e.g., module.Class.method)
    pub file_path: String,      // relative to repo root
    pub line_start: i32,
    pub line_end: i32,
    pub language: String,            // python, javascript, rust, etc.
    pub parent_name: Option<String>, // qualified name of parent scope
    pub params: Option<String>,      // JSON array of param names
    pub return_type: Option<String>, // return type annotation
    pub modifiers: Option<String>,   // public, private, async, etc. (JSON array)
    pub is_test: bool,
    pub file_hash: String, // SHA-256 of file content
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeInfo {
    pub kind: String,             // calls, imports, inherits, uses, etc.
    pub source_qualified: String, // caller/importer
    pub target_qualified: String, // callee/imported
    pub file_path: String,        // where the edge originates
    pub line: i32,                // line number of the call/import
    pub confidence: f32,          // 0.0–1.0 (1.0 = certain, <1.0 = inferred)
    pub confidence_tier: String,  // certain, inferred, heuristic
    pub weight: f32,              // call count or importance
    pub relation: Option<String>, // extra metadata (JSON)
}

// ── SQL Schema Constants ───────────────────────────────────────────────────

pub const CREATE_NODES: &str = r#"
CREATE TABLE IF NOT EXISTS nodes (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT UNIQUE NOT NULL,
    file_path TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    language TEXT NOT NULL,
    parent_name TEXT,
    params TEXT,
    return_type TEXT,
    modifiers TEXT,
    is_test BOOLEAN DEFAULT 0,
    file_hash TEXT NOT NULL,
    community INTEGER,
    community_name TEXT,
    agent_author TEXT,
    task_id INTEGER,
    worker_locked BOOLEAN DEFAULT 0,
    worktree TEXT,
    extra TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_EDGES: &str = r#"
CREATE TABLE IF NOT EXISTS edges (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    source_qualified TEXT NOT NULL,
    target_qualified TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line INTEGER NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    confidence_tier TEXT NOT NULL DEFAULT 'certain',
    weight REAL NOT NULL DEFAULT 1.0,
    relation TEXT,
    agent_author TEXT,
    task_id INTEGER,
    extra TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (source_qualified) REFERENCES nodes(qualified_name) ON DELETE CASCADE,
    FOREIGN KEY (target_qualified) REFERENCES nodes(qualified_name) ON DELETE CASCADE
)
"#;

pub const CREATE_COMMUNITIES: &str = r#"
CREATE TABLE IF NOT EXISTS communities (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    cohesion_score REAL NOT NULL,
    member_count INTEGER NOT NULL,
    dominant_language TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_FLOWS: &str = r#"
CREATE TABLE IF NOT EXISTS flows (
    id INTEGER PRIMARY KEY,
    entry_point TEXT NOT NULL,
    flow_path TEXT NOT NULL,
    criticality_score REAL NOT NULL,
    is_tested BOOLEAN DEFAULT 0,
    has_security_keyword BOOLEAN DEFAULT 0,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_SNAPSHOTS: &str = r#"
CREATE TABLE IF NOT EXISTS snapshots (
    id INTEGER PRIMARY KEY,
    label TEXT UNIQUE NOT NULL,
    node_count INTEGER NOT NULL,
    edge_count INTEGER NOT NULL,
    graph_json TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_ANALYSIS_CACHE: &str = r#"
CREATE TABLE IF NOT EXISTS analysis_cache (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    graph_hash TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_REPOS: &str = r#"
CREATE TABLE IF NOT EXISTS repos (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    alias TEXT,
    db_path TEXT NOT NULL,
    last_updated DATETIME,
    node_count INTEGER DEFAULT 0,
    edge_count INTEGER DEFAULT 0
)
"#;

pub const CREATE_NODES_FTS: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    name,
    qualified_name,
    file_path,
    params,
    return_type,
    content='nodes',
    content_rowid='id',
    tokenize='porter unicode61'
)
"#;

pub const ALL_INDEXES: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_nodes_file ON nodes(file_path)",
    "CREATE INDEX IF NOT EXISTS idx_nodes_qualified ON nodes(qualified_name)",
    "CREATE INDEX IF NOT EXISTS idx_nodes_community ON nodes(community)",
    "CREATE INDEX IF NOT EXISTS idx_nodes_task ON nodes(task_id)",
    "CREATE INDEX IF NOT EXISTS idx_nodes_author ON nodes(agent_author)",
    "CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_qualified)",
    "CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_qualified)",
    "CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges(kind)",
    "CREATE INDEX IF NOT EXISTS idx_edges_target_kind ON edges(target_qualified, kind)",
    "CREATE INDEX IF NOT EXISTS idx_edges_source_kind ON edges(source_qualified, kind)",
    "CREATE INDEX IF NOT EXISTS idx_edges_file ON edges(file_path)",
    "CREATE INDEX IF NOT EXISTS idx_edges_tier ON edges(confidence_tier)",
];
