// Graph store — SQLite CRUD operations for nodes and edges

use super::migrations::run_migrations;
use super::schema::{EdgeInfo, NodeInfo};
use crate::errors::Result;
use crate::observability::timing::OperationTimer;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub rowid: i64,
    pub kind: String,
    pub name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub line_start: i32,
    pub line_end: i32,
    pub language: String,
    pub parent_name: Option<String>,
    pub params: Option<String>,
    pub return_type: Option<String>,
    pub modifiers: Option<String>,
    pub is_test: bool,
    pub file_hash: String,
    pub community: Option<i32>,
    pub community_name: Option<String>,
    pub agent_author: Option<String>,
    pub task_id: Option<i64>,
    pub worker_locked: bool,
    pub worktree: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub rowid: i64,
    pub kind: String,
    pub source_qualified: String,
    pub target_qualified: String,
    pub file_path: String,
    pub line: i32,
    pub confidence: f32,
    pub confidence_tier: String,
    pub weight: f32,
    pub relation: Option<String>,
    pub agent_author: Option<String>,
    pub task_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_count: i64,
    pub edge_count: i64,
    pub nodes_by_kind: std::collections::HashMap<String, i64>,
    pub edges_by_kind: std::collections::HashMap<String, i64>,
    pub languages: Vec<String>,
    pub last_updated: Option<String>,
}

pub struct GraphStore {
    db_path: PathBuf,
}

fn install_timestamp_triggers(conn: &Connection) -> Result<()> {
    let now = crate::db::timestamps::SQLITE_UTC_NOW;
    let sql = format!(
        "CREATE TRIGGER IF NOT EXISTS graph_nodes_timestamp_default AFTER INSERT ON nodes
         WHEN NEW.updated_at = datetime('now')
         BEGIN UPDATE nodes SET updated_at = {now} WHERE id = NEW.id; END;
         CREATE TRIGGER IF NOT EXISTS graph_edges_timestamp_default AFTER INSERT ON edges
         WHEN NEW.updated_at = datetime('now')
         BEGIN UPDATE edges SET updated_at = {now} WHERE id = NEW.id; END;
         CREATE TRIGGER IF NOT EXISTS graph_communities_timestamp_default AFTER INSERT ON communities
         WHEN NEW.updated_at = datetime('now')
         BEGIN UPDATE communities SET updated_at = {now} WHERE id = NEW.id; END;
         CREATE TRIGGER IF NOT EXISTS graph_flows_timestamp_default AFTER INSERT ON flows
         WHEN NEW.updated_at = datetime('now')
         BEGIN UPDATE flows SET updated_at = {now} WHERE id = NEW.id; END;
         CREATE TRIGGER IF NOT EXISTS graph_snapshots_timestamp_default AFTER INSERT ON snapshots
         WHEN NEW.created_at = datetime('now')
         BEGIN UPDATE snapshots SET created_at = {now} WHERE id = NEW.id; END;
         CREATE TRIGGER IF NOT EXISTS graph_analysis_cache_timestamp_default AFTER INSERT ON analysis_cache
         WHEN NEW.updated_at = datetime('now')
         BEGIN UPDATE analysis_cache SET updated_at = {now} WHERE key = NEW.key; END;"
    );
    conn.execute_batch(&sql)?;
    Ok(())
}

impl GraphStore {
    /// This store's schema version, as recorded by the graph migration ladder.
    ///
    /// Exposed so a migration can be verified through the same handle the
    /// application uses, rather than by reading the file separately — see
    /// `examples/migrate_check.rs`.
    pub fn schema_version(&self) -> Result<u32> {
        let operation = OperationTimer::start("graph_schema_version_read").with_database("graph");
        let conn = crate::db::open_tuned(&self.db_path)?;
        let version = super::migrations::get_schema_version(&conn)?;
        operation.with_result_count(1).succeed();
        Ok(version)
    }

    pub fn open(db_path: &Path) -> Result<Self> {
        let operation = OperationTimer::start("graph_store_open").with_database("graph");

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut conn = crate::db::open_tuned(db_path)?;

        // Enable WAL mode for concurrent access
        conn.execute_batch("PRAGMA journal_mode=WAL")?;

        conn.execute_batch("PRAGMA foreign_keys=ON")?;

        // Run migrations
        run_migrations(&mut conn)?;
        install_timestamp_triggers(&conn)?;

        operation.with_result_count(1).succeed();
        Ok(GraphStore {
            db_path: db_path.to_path_buf(),
        })
    }

    fn get_conn(&self) -> Result<Connection> {
        let conn = crate::db::open_tuned(&self.db_path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        Ok(conn)
    }

    pub fn flush(&self) -> Result<()> {
        self.flush_for_task(None)
    }

    /// Flush graph writes while retaining a task correlation when the caller
    /// is running the graph build on behalf of a worker task.
    pub fn flush_for_task(&self, task_id: Option<i64>) -> Result<()> {
        let operation = OperationTimer::start("graph_wal_flush")
            .with_database("graph")
            .with_task_id(task_id);
        let conn = self.get_conn()?;
        // Force WAL checkpoint to flush all writes to main database file
        conn.execute_batch("PRAGMA wal_checkpoint(RESTART)")?;
        operation.with_result_count(1).succeed();
        Ok(())
    }

    pub fn upsert_node(
        &self,
        node: &NodeInfo,
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<i64> {
        let operation = OperationTimer::start("graph_node_upsert")
            .with_database("graph")
            .with_task_id(task_id);
        let conn = self.get_conn()?;
        let params_json = node.params.as_deref();
        let modifiers_json = node.modifiers.as_deref();

        // Start explicit transaction
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let result = conn.execute(
            "INSERT OR REPLACE INTO nodes (
                kind, name, qualified_name, file_path, line_start, line_end,
                language, parent_name, params, return_type, modifiers, is_test,
                file_hash, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                &node.kind,
                &node.name,
                &node.qualified_name,
                &node.file_path,
                node.line_start,
                node.line_end,
                &node.language,
                &node.parent_name,
                params_json,
                &node.return_type,
                modifiers_json,
                node.is_test,
                &node.file_hash,
                agent_author,
                task_id,
                crate::db::timestamps::now_rfc3339(),
            ],
        );

        match result {
            Ok(_) => {
                let rowid = conn.last_insert_rowid();
                conn.execute_batch("COMMIT")?;
                operation.with_result_count(1).succeed();
                Ok(rowid)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e.into())
            }
        }
    }

    /// Batch upsert nodes using a single connection and single transaction.
    /// This is ~100x faster than calling upsert_node in a loop.
    pub fn upsert_nodes_batch(
        &self,
        nodes: &[NodeInfo],
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<usize> {
        let operation = OperationTimer::start("graph_nodes_batch_upsert")
            .with_database("graph")
            .with_task_id(task_id);
        let conn = crate::db::open_tuned(&self.db_path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO nodes (
                kind, name, qualified_name, file_path, line_start, line_end,
                language, parent_name, params, return_type, modifiers, is_test,
                file_hash, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;

        for node in nodes {
            stmt.execute(params![
                &node.kind,
                &node.name,
                &node.qualified_name,
                &node.file_path,
                node.line_start,
                node.line_end,
                &node.language,
                &node.parent_name,
                node.params.as_deref(),
                &node.return_type,
                node.modifiers.as_deref(),
                node.is_test,
                &node.file_hash,
                agent_author,
                task_id,
                crate::db::timestamps::now_rfc3339(),
            ])?;
        }
        drop(stmt);
        conn.execute_batch("COMMIT")?;
        operation.with_result_count(nodes.len()).succeed();
        Ok(nodes.len())
    }

    /// Batch upsert edges using a single connection and single transaction.
    pub fn upsert_edges_batch(
        &self,
        edges: &[EdgeInfo],
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<usize> {
        let operation = OperationTimer::start("graph_edges_batch_upsert")
            .with_database("graph")
            .with_task_id(task_id);
        let conn = crate::db::open_tuned(&self.db_path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let mut stmt = conn.prepare(
            "INSERT OR REPLACE INTO edges (
                kind, source_qualified, target_qualified, file_path, line,
                confidence, confidence_tier, weight, relation, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )?;

        for edge in edges {
            stmt.execute(params![
                &edge.kind,
                &edge.source_qualified,
                &edge.target_qualified,
                &edge.file_path,
                edge.line,
                edge.confidence,
                &edge.confidence_tier,
                edge.weight,
                &edge.relation,
                agent_author,
                task_id,
                crate::db::timestamps::now_rfc3339(),
            ])?;
        }
        drop(stmt);
        conn.execute_batch("COMMIT")?;
        operation.with_result_count(edges.len()).succeed();
        Ok(edges.len())
    }

    pub fn upsert_edge(
        &self,
        edge: &EdgeInfo,
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<i64> {
        let operation = OperationTimer::start("graph_edge_upsert")
            .with_database("graph")
            .with_task_id(task_id);
        let conn = self.get_conn()?;

        // Start explicit transaction
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let result = conn.execute(
            "INSERT OR REPLACE INTO edges (
                kind, source_qualified, target_qualified, file_path, line,
                confidence, confidence_tier, weight, relation, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                &edge.kind,
                &edge.source_qualified,
                &edge.target_qualified,
                &edge.file_path,
                edge.line,
                edge.confidence,
                &edge.confidence_tier,
                edge.weight,
                &edge.relation,
                agent_author,
                task_id,
                crate::db::timestamps::now_rfc3339(),
            ],
        );

        match result {
            Ok(_) => {
                let rowid = conn.last_insert_rowid();
                conn.execute_batch("COMMIT")?;
                operation.with_result_count(1).succeed();
                Ok(rowid)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e.into())
            }
        }
    }

    pub fn delete_file_nodes(&self, file_path: &str) -> Result<usize> {
        let operation = OperationTimer::start("graph_file_nodes_delete").with_database("graph");
        let conn = self.get_conn()?;
        let count = conn.execute("DELETE FROM nodes WHERE file_path = ?", params![file_path])?;
        operation.with_result_count(count).succeed();
        Ok(count)
    }

    pub fn delete_file_edges(&self, file_path: &str) -> Result<usize> {
        let operation = OperationTimer::start("graph_file_edges_delete").with_database("graph");
        let conn = self.get_conn()?;
        let count = conn.execute("DELETE FROM edges WHERE file_path = ?", params![file_path])?;
        operation.with_result_count(count).succeed();
        Ok(count)
    }

    pub fn get_node(&self, qualified_name: &str) -> Result<Option<GraphNode>> {
        let mut operation = OperationTimer::start("graph_node_read").with_database("graph");
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes WHERE qualified_name = ?"
        )?;

        let node = stmt
            .query_row(params![qualified_name], |row| {
                Ok(GraphNode {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    file_path: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    language: row.get(7)?,
                    parent_name: row.get(8)?,
                    params: row.get(9)?,
                    return_type: row.get(10)?,
                    modifiers: row.get(11)?,
                    is_test: row.get(12)?,
                    file_hash: row.get(13)?,
                    community: row.get(14)?,
                    community_name: row.get(15)?,
                    agent_author: row.get(16)?,
                    task_id: row.get(17)?,
                    worker_locked: row.get(18)?,
                    worktree: row.get(19)?,
                })
            })
            .optional()?;

        operation = operation.with_result_count(usize::from(node.is_some()));
        if node.is_some() {
            operation.succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(node)
    }

    pub fn get_edges_for_node(&self, qualified_name: &str) -> Result<Vec<GraphEdge>> {
        let operation = OperationTimer::start("graph_node_edges_read").with_database("graph");
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, source_qualified, target_qualified, file_path, line,
                    confidence, confidence_tier, weight, relation, agent_author, task_id
             FROM edges WHERE source_qualified = ? OR target_qualified = ?",
        )?;

        let edges = stmt
            .query_map(params![qualified_name, qualified_name], |row| {
                Ok(GraphEdge {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    source_qualified: row.get(2)?,
                    target_qualified: row.get(3)?,
                    file_path: row.get(4)?,
                    line: row.get(5)?,
                    confidence: row.get(6)?,
                    confidence_tier: row.get(7)?,
                    weight: row.get(8)?,
                    relation: row.get(9)?,
                    agent_author: row.get(10)?,
                    task_id: row.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        operation.with_result_count(edges.len()).succeed();
        Ok(edges)
    }

    pub fn get_stats(&self) -> Result<GraphStats> {
        let mut operation = crate::observability::timing::OperationTimer::start("graph_stats_read")
            .with_database("graph");
        let conn = self.get_conn()?;
        let node_count: i64 = conn.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;

        let edge_count: i64 = conn.query_row("SELECT COUNT(*) FROM edges", [], |row| row.get(0))?;

        let mut stmt = conn.prepare("SELECT kind, COUNT(*) as count FROM nodes GROUP BY kind")?;
        let mut nodes_by_kind = std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for result in rows {
            let (kind, count) = result?;
            nodes_by_kind.insert(kind, count);
        }

        let mut stmt = conn.prepare("SELECT kind, COUNT(*) as count FROM edges GROUP BY kind")?;
        let mut edges_by_kind = std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for result in rows {
            let (kind, count) = result?;
            edges_by_kind.insert(kind, count);
        }

        let mut stmt = conn.prepare("SELECT DISTINCT language FROM nodes ORDER BY language")?;
        let languages = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let last_updated: Option<String> = conn
            .query_row(
                "SELECT updated_at FROM nodes WHERE updated_at IS NOT NULL
                 ORDER BY julianday(updated_at) DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        let stats = GraphStats {
            node_count,
            edge_count,
            nodes_by_kind,
            edges_by_kind,
            languages,
            last_updated,
        };
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(stats)
    }

    pub fn list_nodes_by_file(&self, file_path: &str) -> Result<Vec<GraphNode>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("graph_file_nodes_read")
                .with_database("graph");
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes WHERE file_path = ? ORDER BY line_start"
        )?;

        let nodes = stmt
            .query_map(params![file_path], |row| {
                Ok(GraphNode {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    file_path: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    language: row.get(7)?,
                    parent_name: row.get(8)?,
                    params: row.get(9)?,
                    return_type: row.get(10)?,
                    modifiers: row.get(11)?,
                    is_test: row.get(12)?,
                    file_hash: row.get(13)?,
                    community: row.get(14)?,
                    community_name: row.get(15)?,
                    agent_author: row.get(16)?,
                    task_id: row.get(17)?,
                    worker_locked: row.get(18)?,
                    worktree: row.get(19)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        operation = operation.with_result_count(nodes.len());
        operation.succeed();
        Ok(nodes)
    }

    pub fn get_file_hash(&self, file_path: &str) -> Result<Option<String>> {
        let mut operation = OperationTimer::start("graph_file_hash_read").with_database("graph");
        let conn = self.get_conn()?;
        let hash = conn
            .query_row(
                "SELECT file_hash FROM nodes WHERE file_path = ? LIMIT 1",
                params![file_path],
                |row| row.get(0),
            )
            .optional()?;

        operation = operation.with_result_count(usize::from(hash.is_some()));
        if hash.is_some() {
            operation.succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(hash)
    }

    pub fn set_community(
        &self,
        qualified_name: &str,
        community: i32,
        community_name: &str,
    ) -> Result<()> {
        let mut operation = OperationTimer::start("graph_community_update").with_database("graph");
        let conn = self.get_conn()?;
        let updated = conn.execute(
            "UPDATE nodes SET community = ?, community_name = ? WHERE qualified_name = ?",
            params![community, community_name, qualified_name],
        )?;
        operation = operation.with_result_count(updated);
        if updated == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    /// Batch update community assignments for many nodes in a single transaction.
    /// ~100x faster than calling set_community in a loop.
    pub fn set_communities_batch(&self, assignments: &[(String, i32, String)]) -> Result<usize> {
        let operation =
            OperationTimer::start("graph_communities_batch_update").with_database("graph");
        let conn = crate::db::open_tuned(&self.db_path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        conn.execute_batch("BEGIN IMMEDIATE")?;

        let mut stmt = conn.prepare(
            "UPDATE nodes SET community = ?, community_name = ? WHERE qualified_name = ?",
        )?;

        let mut updated = 0;
        for (qualified_name, community, community_name) in assignments {
            updated += stmt.execute(params![community, community_name, qualified_name])?;
        }
        drop(stmt);
        conn.execute_batch("COMMIT")?;
        operation.with_result_count(updated).succeed();
        Ok(assignments.len())
    }

    pub fn get_all_nodes_for_graph(&self, limit: i64, offset: i64) -> Result<Vec<GraphNode>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("graph_nodes_page_read")
                .with_database("graph");
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes LIMIT ? OFFSET ?"
        )?;

        let nodes = stmt
            .query_map(params![limit, offset], |row| {
                Ok(GraphNode {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    file_path: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    language: row.get(7)?,
                    parent_name: row.get(8)?,
                    params: row.get(9)?,
                    return_type: row.get(10)?,
                    modifiers: row.get(11)?,
                    is_test: row.get(12)?,
                    file_hash: row.get(13)?,
                    community: row.get(14)?,
                    community_name: row.get(15)?,
                    agent_author: row.get(16)?,
                    task_id: row.get(17)?,
                    worker_locked: row.get(18)?,
                    worktree: row.get(19)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        operation = operation.with_result_count(nodes.len());
        operation.succeed();
        Ok(nodes)
    }

    pub fn get_all_edges_for_graph(&self, limit: i64, offset: i64) -> Result<Vec<GraphEdge>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("graph_edges_page_read")
                .with_database("graph");
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, source_qualified, target_qualified, file_path, line,
                    confidence, confidence_tier, weight, relation, agent_author, task_id
             FROM edges LIMIT ? OFFSET ?",
        )?;

        let edges = stmt
            .query_map(params![limit, offset], |row| {
                Ok(GraphEdge {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    source_qualified: row.get(2)?,
                    target_qualified: row.get(3)?,
                    file_path: row.get(4)?,
                    line: row.get(5)?,
                    confidence: row.get(6)?,
                    confidence_tier: row.get(7)?,
                    weight: row.get(8)?,
                    relation: row.get(9)?,
                    agent_author: row.get(10)?,
                    task_id: row.get(11)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        operation = operation.with_result_count(edges.len());
        operation.succeed();
        Ok(edges)
    }

    // ── Snapshots ─────────────────────────────────────────────────────────

    /// Insert a snapshot row. The blob is the gzip-compressed JSON of all
    /// nodes+edges. Returns the new snapshot's `id`.
    pub fn insert_snapshot(
        &self,
        label: &str,
        node_count: i64,
        edge_count: i64,
        graph_json: &str,
    ) -> Result<i64> {
        let operation = OperationTimer::start("graph_snapshot_insert")
            .with_database("graph")
            .with_result_bytes(graph_json.len());
        let conn = self.get_conn()?;
        // Use INSERT OR REPLACE so re-saving the same label updates instead of failing.
        conn.execute(
            "INSERT OR REPLACE INTO snapshots (label, node_count, edge_count, graph_json) VALUES (?, ?, ?, ?)",
            params![label, node_count, edge_count, graph_json],
        )?;
        operation.with_result_count(1).succeed();
        Ok(conn.last_insert_rowid())
    }

    /// Load a snapshot's `graph_json` payload by id.
    pub fn get_snapshot_json(&self, snapshot_id: i64) -> Result<Option<String>> {
        let mut operation = OperationTimer::start("graph_snapshot_read").with_database("graph");
        let conn = self.get_conn()?;
        let json: Option<String> = conn
            .query_row(
                "SELECT graph_json FROM snapshots WHERE id = ?",
                params![snapshot_id],
                |row| row.get(0),
            )
            .optional()?;
        operation = operation.with_result_count(usize::from(json.is_some()));
        if let Some(json) = &json {
            operation.with_result_bytes(json.len()).succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(json)
    }

    // ── Fast aggregate counts ─────────────────────────────────────────────────

    /// Returns (project_node_count, project_edge_count, community_count) using SQL
    /// COUNT(*) — avoids loading all rows into memory just to compute lengths.
    /// "Project" nodes/edges exclude `module::` stubs (external deps).
    pub fn get_graph_counts(&self) -> Result<(usize, usize, usize)> {
        let operation = OperationTimer::start("graph_counts_read").with_database("graph");
        let conn = self.get_conn()?;
        let nodes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE qualified_name NOT LIKE 'module::%'",
            [],
            |row| row.get(0),
        )?;
        let edges: i64 = conn.query_row(
            "SELECT COUNT(*) FROM edges \
             WHERE source_qualified NOT LIKE 'module::%' \
               AND target_qualified NOT LIKE 'module::%'",
            [],
            |row| row.get(0),
        )?;
        let communities: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT community) FROM nodes WHERE community IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        operation.with_result_count(3).succeed();
        Ok((nodes as usize, edges as usize, communities as usize))
    }

    // ── FTS5-backed node search ───────────────────────────────────────────────

    /// Full-text search over node names, qualified names, and file paths.
    /// Uses the FTS5 virtual table when available; falls back to LIKE otherwise.
    /// Excludes `module::` stub nodes.
    pub fn search_nodes(&self, query: &str, limit: usize) -> Result<Vec<GraphNode>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("graph_node_search")
                .with_database("graph");
        let conn = self.get_conn()?;

        // Check whether the FTS table exists.
        let fts_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='nodes_fts'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(false);

        // Collect into a named Vec before leaving the stmt scope — fixes the
        // "stmt does not live long enough" lifetime error (E0597) that appears
        // when the last expression in a block borrows a local.
        let nodes: Vec<GraphNode> = if fts_exists {
            let mut stmt = conn.prepare(
                "SELECT n.id, n.kind, n.name, n.qualified_name, n.file_path,
                        n.line_start, n.line_end, n.language, n.parent_name,
                        n.params, n.return_type, n.modifiers, n.is_test,
                        n.file_hash, n.community, n.community_name,
                        n.agent_author, n.task_id, n.worker_locked, n.worktree
                 FROM nodes_fts
                 JOIN nodes n ON nodes_fts.rowid = n.id
                 WHERE nodes_fts MATCH ?
                   AND n.qualified_name NOT LIKE 'module::%'
                 ORDER BY rank
                 LIMIT ?",
            )?;
            let mapped = stmt.query_map(params![query, limit as i64], |row| {
                Ok(GraphNode {
                    rowid: row.get(0)?,
                    kind: row.get(1)?,
                    name: row.get(2)?,
                    qualified_name: row.get(3)?,
                    file_path: row.get(4)?,
                    line_start: row.get(5)?,
                    line_end: row.get(6)?,
                    language: row.get(7)?,
                    parent_name: row.get(8)?,
                    params: row.get(9)?,
                    return_type: row.get(10)?,
                    modifiers: row.get(11)?,
                    is_test: row.get(12)?,
                    file_hash: row.get(13)?,
                    community: row.get(14)?,
                    community_name: row.get(15)?,
                    agent_author: row.get(16)?,
                    task_id: row.get(17)?,
                    worker_locked: row.get(18)?,
                    worktree: row.get(19)?,
                })
            })?;
            let collected: rusqlite::Result<Vec<GraphNode>> = mapped.collect();
            collected?
        } else {
            let pattern = format!("%{}%", query);
            let mut stmt = conn.prepare(
                "SELECT id, kind, name, qualified_name, file_path,
                        line_start, line_end, language, parent_name,
                        params, return_type, modifiers, is_test,
                        file_hash, community, community_name,
                        agent_author, task_id, worker_locked, worktree
                 FROM nodes
                 WHERE qualified_name NOT LIKE 'module::%'
                   AND (name LIKE ? OR qualified_name LIKE ? OR file_path LIKE ?)
                 LIMIT ?",
            )?;
            let mapped =
                stmt.query_map(params![&pattern, &pattern, &pattern, limit as i64], |row| {
                    Ok(GraphNode {
                        rowid: row.get(0)?,
                        kind: row.get(1)?,
                        name: row.get(2)?,
                        qualified_name: row.get(3)?,
                        file_path: row.get(4)?,
                        line_start: row.get(5)?,
                        line_end: row.get(6)?,
                        language: row.get(7)?,
                        parent_name: row.get(8)?,
                        params: row.get(9)?,
                        return_type: row.get(10)?,
                        modifiers: row.get(11)?,
                        is_test: row.get(12)?,
                        file_hash: row.get(13)?,
                        community: row.get(14)?,
                        community_name: row.get(15)?,
                        agent_author: row.get(16)?,
                        task_id: row.get(17)?,
                        worker_locked: row.get(18)?,
                        worktree: row.get(19)?,
                    })
                })?;
            let collected: rusqlite::Result<Vec<GraphNode>> = mapped.collect();
            collected?
        };

        operation = operation.with_result_count(nodes.len());
        operation.succeed();
        Ok(nodes)
    }

    pub fn rebuild_fts_index(&self) -> Result<usize> {
        self.rebuild_fts_index_for_task(None)
    }

    /// Rebuild graph search data while preserving task correlation for the
    /// worker-owned production build path.
    pub fn rebuild_fts_index_for_task(&self, task_id: Option<i64>) -> Result<usize> {
        let operation = OperationTimer::start("graph_fts_rebuild")
            .with_database("graph")
            .with_task_id(task_id);
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;

        tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")?;
        tx.execute_batch(
            "CREATE VIRTUAL TABLE nodes_fts USING fts5(
                name, qualified_name, file_path, params, return_type,
                content='nodes', content_rowid='id', tokenize='porter unicode61'
            )",
        )?;
        tx.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")?;

        let count: i64 = tx.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;

        tx.commit()?;
        operation.with_result_count(count as usize).succeed();
        Ok(count as usize)
    }
}

#[cfg(test)]
mod timestamp_tests {
    use super::*;
    use chrono::Utc;

    fn node(name: &str) -> NodeInfo {
        NodeInfo {
            kind: "function".into(),
            name: name.into(),
            qualified_name: format!("mod::{name}"),
            file_path: "src/lib.rs".into(),
            line_start: 1,
            line_end: 1,
            language: "rust".into(),
            parent_name: None,
            params: None,
            return_type: None,
            modifiers: None,
            is_test: false,
            file_hash: "hash".into(),
        }
    }

    fn edge(source: &str, target: &str) -> EdgeInfo {
        EdgeInfo {
            kind: "calls".into(),
            source_qualified: format!("mod::{source}"),
            target_qualified: format!("mod::{target}"),
            file_path: "private-graph-source-path-marker".into(),
            line: 12,
            confidence: 1.0,
            confidence_tier: "certain".into(),
            weight: 1.0,
            relation: Some("private-graph-relation-marker".into()),
        }
    }

    #[test]
    fn production_graph_readers_emit_private_result_spans() {
        let dir = tempfile::tempdir().expect("isolated graph database directory");
        let path = dir.path().join("graph.db");
        let store = GraphStore::open(&path).expect("production graph opener");
        store
            .upsert_node(&node("graph-private-marker"), None, None)
            .expect("seed graph node");
        store
            .rebuild_fts_index()
            .expect("build isolated search index");

        let (_, events) = crate::observability::timing::capture_json_events(|| {
            assert_eq!(store.list_nodes_by_file("src/lib.rs").unwrap().len(), 1);
            assert_eq!(store.get_all_nodes_for_graph(20, 0).unwrap().len(), 1);
            assert!(store.get_all_edges_for_graph(20, 0).unwrap().is_empty());
            assert_eq!(store.search_nodes("marker", 10).unwrap().len(), 1);
            assert_eq!(store.get_stats().unwrap().node_count, 1);
        });

        let observed: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        let expected = [
            ("graph_file_nodes_read", 1),
            ("graph_nodes_page_read", 1),
            ("graph_edges_page_read", 0),
            ("graph_node_search", 1),
            ("graph_stats_read", 1),
        ];
        assert_eq!(observed.len(), expected.len());
        for (event, (operation, count)) in observed.iter().zip(expected) {
            assert_eq!(event["fields"]["operation"], operation);
            assert_eq!(event["fields"]["outcome"], "success");
            assert_eq!(event["fields"]["database"], "graph");
            assert_eq!(event["fields"]["result_count"], count);
            assert_eq!(event["fields"]["result_count_observed"], true);
        }
        let serialized = serde_json::to_string(&events).expect("serialize events");
        assert!(!serialized.contains("graph-private-marker"));
        assert!(!serialized.contains("src/lib.rs"));
        assert!(!serialized.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn production_graph_mutations_and_point_reads_emit_private_result_spans() {
        let dir = tempfile::tempdir().expect("isolated graph database directory");
        let path = dir.path().join("graph.db");
        let first = node("graph-private-node-one-marker");
        let second = node("graph-private-node-two-marker");
        let third = node("graph-private-node-three-marker");
        let first_edge = edge(
            "graph-private-node-one-marker",
            "graph-private-node-two-marker",
        );
        let second_edge = edge(
            "graph-private-node-two-marker",
            "graph-private-node-three-marker",
        );

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let store = GraphStore::open(&path).expect("production graph opener");
            assert_eq!(store.schema_version().expect("schema version"), 6);
            store
                .upsert_node(&first, Some("private-agent-marker"), Some(42))
                .expect("insert node");
            assert_eq!(
                store
                    .upsert_nodes_batch(&[second.clone(), third.clone()], None, Some(42))
                    .expect("insert node batch"),
                2
            );
            store
                .upsert_edge(&first_edge, Some("private-agent-marker"), Some(42))
                .expect("insert edge");
            assert_eq!(
                store
                    .upsert_edges_batch(&[second_edge], None, Some(42))
                    .expect("insert edge batch"),
                1
            );
            assert!(store
                .get_node("mod::graph-private-node-one-marker")
                .expect("node point read")
                .is_some());
            assert!(store
                .get_node("private-missing-qualified-name-marker")
                .expect("missing node point read")
                .is_none());
            assert_eq!(
                store
                    .get_edges_for_node("mod::graph-private-node-two-marker")
                    .expect("edge point read")
                    .len(),
                2
            );
            assert!(store
                .get_file_hash("src/lib.rs")
                .expect("file hash reader")
                .is_some());
            assert_eq!(store.get_graph_counts().expect("graph counts").0, 3);
            store
                .set_community(
                    "mod::graph-private-node-one-marker",
                    7,
                    "private-community-marker",
                )
                .expect("set community");
            assert_eq!(
                store
                    .set_communities_batch(&[
                        (
                            "mod::graph-private-node-two-marker".into(),
                            7,
                            "private-community-marker".into()
                        ),
                        (
                            "private-missing-qualified-name-marker".into(),
                            7,
                            "private-community-marker".into()
                        ),
                    ])
                    .expect("set communities batch"),
                2
            );
            let snapshot = store
                .insert_snapshot(
                    "private-snapshot-label-marker",
                    3,
                    2,
                    "private-snapshot-json-marker",
                )
                .expect("insert snapshot");
            assert_eq!(
                store
                    .get_snapshot_json(snapshot)
                    .expect("read snapshot")
                    .as_deref(),
                Some("private-snapshot-json-marker")
            );
            assert_eq!(store.rebuild_fts_index().expect("rebuild FTS"), 3);
            assert_eq!(
                store
                    .delete_file_edges("private-graph-source-path-marker")
                    .expect("delete graph edges"),
                2
            );
            assert_eq!(
                store
                    .delete_file_nodes("src/lib.rs")
                    .expect("delete graph nodes"),
                3
            );
            store.flush().expect("flush graph WAL");
            store
                .get_conn()
                .expect("graph test connection")
                .execute_batch(
                    "CREATE TRIGGER reject_test_graph_nodes BEFORE INSERT ON nodes
                     BEGIN SELECT RAISE(ABORT, 'private-graph-trigger-error-marker'); END;",
                )
                .expect("install isolated graph failure trigger");
            let failed_batch = store
                .upsert_nodes_batch(&[node("private-failed-node-marker")], None, None)
                .is_err();
            failed_batch
        });
        assert!(observed);

        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "graph_store_open",
            "graph_schema_version_read",
            "graph_node_upsert",
            "graph_nodes_batch_upsert",
            "graph_edge_upsert",
            "graph_edges_batch_upsert",
            "graph_node_read",
            "graph_node_edges_read",
            "graph_file_hash_read",
            "graph_counts_read",
            "graph_community_update",
            "graph_communities_batch_update",
            "graph_snapshot_insert",
            "graph_snapshot_read",
            "graph_fts_rebuild",
            "graph_file_edges_delete",
            "graph_file_nodes_delete",
            "graph_wal_flush",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing graph span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "graph_node_read"
                && event["fields"]["outcome"] == "not_found"
                && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "graph_nodes_batch_upsert"
                && event["fields"]["outcome"] == "error"
        }));
        let serialized = serde_json::to_string(&events).expect("serialize graph telemetry");
        for private_value in [
            "graph-private-node-one-marker",
            "graph-private-node-two-marker",
            "private-graph-source-path-marker",
            "private-graph-relation-marker",
            "private-agent-marker",
            "private-community-marker",
            "private-snapshot-label-marker",
            "private-snapshot-json-marker",
            "private-failed-node-marker",
            "private-graph-trigger-error-marker",
            path.to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "telemetry leaked {private_value}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "graph_node_upsert"
                && event["fields"]["task_id"] == "42"
        }));
    }

    #[test]
    fn file_backed_graph_writer_default_and_mixed_timestamp_reader_are_canonicalized() {
        let dir = tempfile::tempdir().expect("isolated graph database directory");
        let path = dir.path().join("graph.db");
        let store = GraphStore::open(&path).expect("production graph opener");
        store
            .upsert_node(&node("old"), None, None)
            .expect("insert old node");
        store
            .upsert_node(&node("new"), None, None)
            .expect("insert new node");

        let conn = store.get_conn().expect("graph reader connection");
        let new_time = Utc::now() - chrono::Duration::hours(1);
        let old_time = Utc::now() - chrono::Duration::hours(2);
        conn.execute(
            "UPDATE nodes SET updated_at = ?1 WHERE qualified_name = 'mod::old'",
            [crate::db::timestamps::format_utc_timestamp(old_time)],
        )
        .expect("plant old RFC3339 row");
        conn.execute(
            "UPDATE nodes SET updated_at = ?1 WHERE qualified_name = 'mod::new'",
            [new_time.format("%Y-%m-%d %H:%M:%S").to_string()],
        )
        .expect("plant newer legacy row");
        drop(conn);

        let stats = store.get_stats().expect("production graph reader");
        let legacy_newest = new_time.format("%Y-%m-%d %H:%M:%S").to_string();
        assert_eq!(stats.last_updated.as_deref(), Some(legacy_newest.as_str()));
        drop(store);

        // Recreate one table as it existed in older graph files, retaining an
        // old row and its timestamp. The next production open must add the
        // default-normalizing trigger without rewriting that legacy row.
        let conn = Connection::open(&path).expect("open isolated graph DB");
        conn.execute_batch(
            "CREATE TABLE snapshots_legacy (
                id INTEGER PRIMARY KEY, label TEXT UNIQUE NOT NULL,
                node_count INTEGER NOT NULL, edge_count INTEGER NOT NULL,
                graph_json TEXT NOT NULL, created_at DATETIME DEFAULT CURRENT_TIMESTAMP
             );
             INSERT INTO snapshots_legacy (id, label, node_count, edge_count, graph_json, created_at)
             VALUES (1, 'legacy', 1, 0, '{}', '2020-02-03 04:05:06');
             DROP TABLE snapshots;
             ALTER TABLE snapshots_legacy RENAME TO snapshots;",
        )
        .expect("plant legacy graph table shape");
        drop(conn);
        let store = GraphStore::open(&path).expect("production reopen and schema update");
        assert_eq!(store.get_snapshot_json(1).unwrap().as_deref(), Some("{}"));
        let conn = store.get_conn().expect("read trigger result");
        let legacy_timestamp: String = conn
            .query_row("SELECT created_at FROM snapshots WHERE id = 1", [], |row| {
                row.get(0)
            })
            .expect("legacy row survives schema update");
        assert_eq!(legacy_timestamp, "2020-02-03 04:05:06");
        let snapshot_id = store
            .insert_snapshot("timestamp", 1, 0, "{}")
            .expect("production snapshot writer");
        let created_at: String = conn
            .query_row(
                "SELECT created_at FROM snapshots WHERE id = ?1",
                [snapshot_id],
                |row| row.get(0),
            )
            .expect("snapshot reader");
        assert_eq!(created_at.len(), 24);
        assert!(created_at.ends_with('Z'));
    }
}
