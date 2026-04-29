// Graph store — SQLite CRUD operations for nodes and edges

use rusqlite::{Connection, params, OptionalExtension};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::schema::{NodeInfo, EdgeInfo};
use super::migrations::run_migrations;

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

impl GraphStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        tracing::debug!("[GraphStore::open] Opening database at: {:?}", db_path);
        
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
            tracing::debug!("[GraphStore::open] Parent directory created: {:?}", parent);
        }
        
        let mut conn = Connection::open(db_path)?;
        tracing::debug!("[GraphStore::open] Connection established");
        tracing::info!("[GraphStore::open] Database file will be created at: {:?}", db_path);
        
        // Enable WAL mode for concurrent access
        tracing::debug!("[GraphStore::open] Enabling WAL mode...");
        conn.execute_batch("PRAGMA journal_mode=WAL")?;
        tracing::debug!("[GraphStore::open] WAL mode enabled");
        
        tracing::debug!("[GraphStore::open] Enabling foreign keys...");
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        tracing::debug!("[GraphStore::open] Foreign keys enabled");
        
        // Run migrations
        tracing::info!("[GraphStore::open] Running migrations...");
        run_migrations(&mut conn)?;
        tracing::info!("[GraphStore::open] Migrations complete");
        
        Ok(GraphStore {
            db_path: db_path.to_path_buf(),
        })
    }
    
    fn get_conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA foreign_keys=ON")?;
        Ok(conn)
    }
    
    pub fn flush(&self) -> Result<()> {
        let conn = self.get_conn()?;
        // Force WAL checkpoint to flush all writes to main database file
        conn.execute_batch("PRAGMA wal_checkpoint(RESTART)")?;
        tracing::debug!("[GraphStore::flush] WAL checkpoint completed");
        Ok(())
    }
    
    pub fn upsert_node(
        &self,
        node: &NodeInfo,
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<i64> {
        let mut conn = self.get_conn()?;
        let params_json = node.params.as_deref();
        let modifiers_json = node.modifiers.as_deref();
        
        // Start explicit transaction
        conn.execute_batch("BEGIN IMMEDIATE")?;
        
        let result = conn.execute(
            "INSERT OR REPLACE INTO nodes (
                kind, name, qualified_name, file_path, line_start, line_end,
                language, parent_name, params, return_type, modifiers, is_test,
                file_hash, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
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
            ],
        );
        
        match result {
            Ok(_) => {
                let rowid = conn.last_insert_rowid();
                conn.execute_batch("COMMIT")?;
                Ok(rowid)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e.into())
            }
        }
    }
    
    pub fn upsert_edge(
        &self,
        edge: &EdgeInfo,
        agent_author: Option<&str>,
        task_id: Option<i64>,
    ) -> Result<i64> {
        let mut conn = self.get_conn()?;
        
        // Start explicit transaction
        conn.execute_batch("BEGIN IMMEDIATE")?;
        
        let result = conn.execute(
            "INSERT OR REPLACE INTO edges (
                kind, source_qualified, target_qualified, file_path, line,
                confidence, confidence_tier, weight, relation, agent_author, task_id, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
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
            ],
        );
        
        match result {
            Ok(_) => {
                let rowid = conn.last_insert_rowid();
                conn.execute_batch("COMMIT")?;
                Ok(rowid)
            }
            Err(e) => {
                let _ = conn.execute_batch("ROLLBACK");
                Err(e.into())
            }
        }
    }
    
    pub fn delete_file_nodes(&self, file_path: &str) -> Result<usize> {
        let conn = self.get_conn()?;
        let count = conn.execute(
            "DELETE FROM nodes WHERE file_path = ?",
            params![file_path],
        )?;
        Ok(count)
    }
    
    pub fn delete_file_edges(&self, file_path: &str) -> Result<usize> {
        let conn = self.get_conn()?;
        let count = conn.execute(
            "DELETE FROM edges WHERE file_path = ?",
            params![file_path],
        )?;
        Ok(count)
    }
    
    pub fn get_node(&self, qualified_name: &str) -> Result<Option<GraphNode>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes WHERE qualified_name = ?"
        )?;
        
        let node = stmt.query_row(params![qualified_name], |row| {
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
        }).optional()?;
        
        Ok(node)
    }
    
    pub fn get_edges_for_node(&self, qualified_name: &str) -> Result<Vec<GraphEdge>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, source_qualified, target_qualified, file_path, line,
                    confidence, confidence_tier, weight, relation, agent_author, task_id
             FROM edges WHERE source_qualified = ? OR target_qualified = ?"
        )?;
        
        let edges = stmt.query_map(params![qualified_name, qualified_name], |row| {
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
        
        Ok(edges)
    }
    
    pub fn get_stats(&self) -> Result<GraphStats> {
        let conn = self.get_conn()?;
        let node_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM nodes",
            [],
            |row| row.get(0),
        )?;
        
        let edge_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM edges",
            [],
            |row| row.get(0),
        )?;
        
        let mut stmt = conn.prepare(
            "SELECT kind, COUNT(*) as count FROM nodes GROUP BY kind"
        )?;
        let mut nodes_by_kind = std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for result in rows {
            let (kind, count) = result?;
            nodes_by_kind.insert(kind, count);
        }
        
        let mut stmt = conn.prepare(
            "SELECT kind, COUNT(*) as count FROM edges GROUP BY kind"
        )?;
        let mut edges_by_kind = std::collections::HashMap::new();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for result in rows {
            let (kind, count) = result?;
            edges_by_kind.insert(kind, count);
        }
        
        let mut stmt = conn.prepare(
            "SELECT DISTINCT language FROM nodes ORDER BY language"
        )?;
        let languages = stmt.query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        
        let last_updated: Option<String> = conn.query_row(
            "SELECT MAX(updated_at) FROM nodes",
            [],
            |row| row.get(0),
        ).ok();
        
        Ok(GraphStats {
            node_count,
            edge_count,
            nodes_by_kind,
            edges_by_kind,
            languages,
            last_updated,
        })
    }
    
    pub fn list_nodes_by_file(&self, file_path: &str) -> Result<Vec<GraphNode>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes WHERE file_path = ? ORDER BY line_start"
        )?;
        
        let nodes = stmt.query_map(params![file_path], |row| {
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
        
        Ok(nodes)
    }
    
    pub fn get_file_hash(&self, file_path: &str) -> Result<Option<String>> {
        let conn = self.get_conn()?;
        let hash = conn.query_row(
            "SELECT file_hash FROM nodes WHERE file_path = ? LIMIT 1",
            params![file_path],
            |row| row.get(0),
        ).optional()?;
        
        Ok(hash)
    }
    
    pub fn set_community(&self, qualified_name: &str, community: i32, community_name: &str) -> Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE nodes SET community = ?, community_name = ? WHERE qualified_name = ?",
            params![community, community_name, qualified_name],
        )?;
        Ok(())
    }
    
    pub fn get_all_nodes_for_graph(&self, limit: i64, offset: i64) -> Result<Vec<GraphNode>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, name, qualified_name, file_path, line_start, line_end,
                    language, parent_name, params, return_type, modifiers, is_test,
                    file_hash, community, community_name, agent_author, task_id, worker_locked, worktree
             FROM nodes LIMIT ? OFFSET ?"
        )?;
        
        let nodes = stmt.query_map(params![limit, offset], |row| {
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
        
        Ok(nodes)
    }
    
    pub fn get_all_edges_for_graph(&self, limit: i64, offset: i64) -> Result<Vec<GraphEdge>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, kind, source_qualified, target_qualified, file_path, line,
                    confidence, confidence_tier, weight, relation, agent_author, task_id
             FROM edges LIMIT ? OFFSET ?"
        )?;
        
        let edges = stmt.query_map(params![limit, offset], |row| {
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
        let conn = self.get_conn()?;
        // Use INSERT OR REPLACE so re-saving the same label updates instead of failing.
        conn.execute(
            "INSERT OR REPLACE INTO snapshots (label, node_count, edge_count, graph_json) VALUES (?, ?, ?, ?)",
            params![label, node_count, edge_count, graph_json],
        )?;
        Ok(conn.last_insert_rowid())
    }
    
    /// Load a snapshot's `graph_json` payload by id.
    pub fn get_snapshot_json(&self, snapshot_id: i64) -> Result<Option<String>> {
        let conn = self.get_conn()?;
        let json: Option<String> = conn.query_row(
            "SELECT graph_json FROM snapshots WHERE id = ?",
            params![snapshot_id],
            |row| row.get(0),
        ).optional()?;
        Ok(json)
    }
    
    pub fn rebuild_fts_index(&self) -> Result<usize> {
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        
        tx.execute_batch("DROP TABLE IF EXISTS nodes_fts")?;
        tx.execute_batch(
            "CREATE VIRTUAL TABLE nodes_fts USING fts5(
                name, qualified_name, file_path, params, return_type,
                content='nodes', content_rowid='id', tokenize='porter unicode61'
            )"
        )?;
        tx.execute_batch("INSERT INTO nodes_fts(nodes_fts) VALUES('rebuild')")?;
        
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))?;
        
        tx.commit()?;
        Ok(count as usize)
    }
}
