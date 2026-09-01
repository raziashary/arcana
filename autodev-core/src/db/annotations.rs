// Annotations database — persists inline code annotations created by users or agents.
// Database path: ~/.autodev/annotations.db
// Used by: IDE CodeViewer (display), QC Agent (write), PA (write on Ask PA flow)

use super::autodev_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub file_path: String, // workspace-relative path
    pub start_line: i64,
    pub end_line: i64,
    pub selected_text: Option<String>,
    pub prompt: String,
    pub response: Option<String>,
    pub source: String,   // "user" | "qc_agent" | "worker"
    pub severity: String, // "info" | "warning" | "blocking"
    pub status: String,   // "open" | "pending" | "resolved"
    pub task_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnnotationInput {
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub selected_text: Option<String>,
    pub prompt: String,
    pub source: Option<String>,
    pub severity: Option<String>,
    pub task_id: Option<String>,
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct AnnotationsDb {
    conn: Connection,
}

impl AnnotationsDb {
    /// This database's schema version, as recorded by the migration ladder.
    ///
    /// Exposed so a migration can be *verified* rather than assumed — see
    /// `examples/migrate_check.rs`. Reading it through the same handle the
    /// application uses is the point: a version read from a fresh connection
    /// would prove the file's state, not that the shipped `open()` path put it
    /// there.
    pub fn schema_version(&self) -> Result<i32> {
        migrate::user_version(&self.conn)
    }

    pub fn open() -> Result<Self> {
        let db_path = autodev_dir()?.join("annotations.db");
        let conn = crate::db::open_tuned(&db_path)
            .map_err(|e| AutoDevError::Agent(format!("annotations.db open failed: {e}")))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// `annotations.db`'s ladder. `v1` is the schema as it stands, created idempotently,
    /// so an existing database is stamped without being altered.
    const LADDER: &[Migration] = &[Migration {
        version: 1,
        name: "annotations-baseline",
        sql: r#"
            CREATE TABLE IF NOT EXISTS annotations (
                id            TEXT PRIMARY KEY,
                file_path     TEXT NOT NULL,
                start_line    INTEGER NOT NULL,
                end_line      INTEGER NOT NULL,
                selected_text TEXT,
                prompt        TEXT NOT NULL,
                response      TEXT,
                source        TEXT NOT NULL DEFAULT 'user',
                severity      TEXT NOT NULL DEFAULT 'info',
                status        TEXT NOT NULL DEFAULT 'open',
                task_id       TEXT,
                created_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at    DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_ann_file   ON annotations(file_path);
            CREATE INDEX IF NOT EXISTS idx_ann_status ON annotations(status);
            CREATE TRIGGER IF NOT EXISTS update_annotations_timestamp
                AFTER UPDATE ON annotations
                BEGIN
                    UPDATE annotations SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                END;
        "#,
    }];

    fn migrate(&self) -> Result<()> {
        // PRAGMAs are per-connection and are not carried by a migration.
        migrate::run_ladder(&self.conn, "annotations.db", Self::LADDER)?;
        Ok(())
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn create(&self, input: &CreateAnnotationInput) -> Result<String> {
        let id = uuid_v4();
        let source = input.source.as_deref().unwrap_or("user");
        let severity = input.severity.as_deref().unwrap_or("info");
        self.conn.execute(
            "INSERT INTO annotations \
             (id, file_path, start_line, end_line, selected_text, prompt, source, severity, task_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id, input.file_path, input.start_line, input.end_line,
                input.selected_text, input.prompt, source, severity, input.task_id
            ],
        ).map_err(|e| AutoDevError::Agent(format!("create annotation: {e}")))?;
        Ok(id)
    }

    pub fn list_for_file(&self, file_path: &str) -> Result<Vec<Annotation>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, start_line, end_line, selected_text, prompt, response, \
                    source, severity, status, task_id, created_at, updated_at \
             FROM annotations WHERE file_path = ?1 AND status != 'deleted' \
             ORDER BY start_line ASC",
            )
            .map_err(|e| AutoDevError::Agent(format!("list_for_file prepare: {e}")))?;

        let rows = stmt
            .query_map(params![file_path], |row| {
                Ok(Annotation {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    start_line: row.get(2)?,
                    end_line: row.get(3)?,
                    selected_text: row.get(4)?,
                    prompt: row.get(5)?,
                    response: row.get(6)?,
                    source: row.get(7)?,
                    severity: row.get(8)?,
                    status: row.get(9)?,
                    task_id: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .map_err(|e| AutoDevError::Agent(format!("list_for_file query: {e}")))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("list_for_file collect: {e}")))
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Annotation>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, start_line, end_line, selected_text, prompt, response, \
                    source, severity, status, task_id, created_at, updated_at \
             FROM annotations WHERE id = ?1",
            )
            .map_err(|e| AutoDevError::Agent(format!("get_by_id prepare: {e}")))?;

        stmt.query_row(params![id], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                file_path: row.get(1)?,
                start_line: row.get(2)?,
                end_line: row.get(3)?,
                selected_text: row.get(4)?,
                prompt: row.get(5)?,
                response: row.get(6)?,
                source: row.get(7)?,
                severity: row.get(8)?,
                status: row.get(9)?,
                task_id: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .optional()
        .map_err(|e| AutoDevError::Agent(format!("get_by_id: {e}")))
    }

    pub fn update_response(&self, id: &str, response: Option<&str>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE annotations SET response = ?1 WHERE id = ?2",
                params![response, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_response: {e}")))?;
        Ok(())
    }

    pub fn update_task_id(&self, id: &str, task_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE annotations SET task_id = ?1 WHERE id = ?2",
                params![task_id, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_task_id: {e}")))?;
        Ok(())
    }

    pub fn update_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE annotations SET status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_status: {e}")))?;
        Ok(())
    }

    pub fn resolve(&self, id: &str, response: Option<&str>) -> Result<()> {
        self.conn
            .execute(
                "UPDATE annotations SET status = 'resolved', response = ?1 WHERE id = ?2",
                params![response, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("resolve: {e}")))?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE annotations SET status = 'deleted' WHERE id = ?1",
                params![id],
            )
            .map_err(|e| AutoDevError::Agent(format!("delete: {e}")))?;
        Ok(())
    }
}

// ─── UUID helper ──────────────────────────────────────────────────────────────

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Simple but collision-resistant enough for local annotations:
    // combine timestamp micros + thread id + nanos
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("ann-{:016x}-{:08x}", ts, nanos)
}
