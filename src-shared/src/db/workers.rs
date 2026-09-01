// Workers database — mirrors the schema in src-tauri/src/db/workers.rs.

use super::autodev_dir;
use crate::errors::{Result, SharedError};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub worker_type: String,
    pub status: String,
    pub model: String,
    pub last_seen: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub description: Option<String>,
    pub notes: Option<String>,
}

pub struct CreateWorkerInput {
    pub slug: String,
    pub name: String,
    pub worker_type: String,
    pub model: Option<String>,
    pub description: Option<String>,
}

pub struct WorkersDb {
    conn: Connection,
}

impl WorkersDb {
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("workers.db");
        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS workers (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 slug        TEXT    NOT NULL UNIQUE,
                 name        TEXT    NOT NULL,
                 worker_type TEXT    NOT NULL DEFAULT 'custom',
                 status      TEXT    NOT NULL DEFAULT 'created',
                 model       TEXT    NOT NULL DEFAULT 'claude-haiku-4-5',
                 last_seen   TEXT,
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 description TEXT,
                 notes       TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_workers_slug   ON workers(slug);
             CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);",
        )?;
        Ok(())
    }

    pub fn create(&self, input: CreateWorkerInput) -> Result<Worker> {
        self.conn.execute(
            "INSERT INTO workers (slug, name, worker_type, model, description)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                input.slug,
                input.name,
                input.worker_type,
                input.model.unwrap_or_else(|| "claude-haiku-4-5".into()),
                input.description,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get(id)?
            .ok_or_else(|| SharedError::Other("Worker not found after insert".into()))
    }

    pub fn get(&self, id: i64) -> Result<Option<Worker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes
             FROM workers WHERE id=?1"
        )?;
        Ok(stmt.query_row(params![id], row_to_worker).optional()?)
    }

    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Worker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes
             FROM workers WHERE slug=?1"
        )?;
        Ok(stmt.query_row(params![slug], row_to_worker).optional()?)
    }

    pub fn list(
        &self,
        type_filter: Option<&str>,
        status_filter: Option<&str>,
    ) -> Result<Vec<Worker>> {
        let sql = match (type_filter, status_filter) {
            (Some(_), Some(_)) =>
                "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes FROM workers WHERE worker_type=?1 AND status=?2 AND status!='deleted' ORDER BY name ASC",
            (Some(_), None) =>
                "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes FROM workers WHERE worker_type=?1 AND status!='deleted' ORDER BY name ASC",
            (None, Some(_)) =>
                "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes FROM workers WHERE status=?1 AND status!='deleted' ORDER BY name ASC",
            (None, None) =>
                "SELECT id,slug,name,worker_type,status,model,last_seen,created_at,updated_at,description,notes FROM workers WHERE status!='deleted' ORDER BY name ASC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let workers = match (type_filter, status_filter) {
            (Some(t), Some(s)) => stmt
                .query_map(params![t, s], row_to_worker)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (Some(t), None) => stmt
                .query_map(params![t], row_to_worker)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (None, Some(s)) => stmt
                .query_map(params![s], row_to_worker)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (None, None) => stmt
                .query_map([], row_to_worker)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(workers)
    }

    pub fn update_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET status=?1, updated_at=datetime('now') WHERE id=?2",
            params![status, id],
        )?;
        Ok(())
    }

    /// Update last_seen timestamp for heartbeat.
    pub fn heartbeat(&self, slug: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE workers SET last_seen=datetime('now'), status='active', updated_at=datetime('now') WHERE slug=?1 AND status!='deleted'",
            params![slug],
        )?;
        if rows == 0 {
            return Err(SharedError::Other(format!("Worker '{}' not found", slug)));
        }
        Ok(())
    }

    /// Soft-delete a worker.
    pub fn unregister(&self, slug: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE workers SET status='deleted', updated_at=datetime('now') WHERE slug=?1",
            params![slug],
        )?;
        if rows == 0 {
            return Err(SharedError::Other(format!("Worker '{}' not found", slug)));
        }
        Ok(())
    }
}

fn row_to_worker(row: &rusqlite::Row<'_>) -> rusqlite::Result<Worker> {
    Ok(Worker {
        id: row.get(0)?,
        slug: row.get(1)?,
        name: row.get(2)?,
        worker_type: row.get(3)?,
        status: row.get(4)?,
        model: row.get(5)?,
        last_seen: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        description: row.get(9)?,
        notes: row.get(10)?,
    })
}
