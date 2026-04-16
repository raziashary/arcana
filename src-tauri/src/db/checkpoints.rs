// File Checkpointing database — SQLite storage for file snapshots.
// Database path: ~/.autodev/checkpoints.db
// Used by: AgentEngine to record file states after write/edit operations
// Purpose: Enable rollback, undo, and audit trail for file modifications
// See: DEVELOPMENT_PLAN.md Phase 3 — File Change Tracking

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use super::autodev_dir;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    Create,
    Write,
    Edit,
}

impl OperationType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Create => "create",
            Self::Write => "write",
            Self::Edit => "edit",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "create" => Self::Create,
            "write" => Self::Write,
            "edit" => Self::Edit,
            _ => Self::Write, // default
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCheckpoint {
    pub id:             i64,
    pub message_id:     String,
    pub file_path:      String,
    pub file_content:   String,
    pub timestamp:      String,
    pub operation_type: OperationType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCheckpointInput {
    pub message_id:     String,
    pub file_path:      String,
    pub file_content:   String,
    pub operation_type: String, // "create" | "write" | "edit"
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct CheckpointsDb {
    conn: Connection,
}

impl CheckpointsDb {
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("checkpoints.db");
        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS file_checkpoints (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id      TEXT    NOT NULL,
                file_path       TEXT    NOT NULL,
                file_content    TEXT    NOT NULL,
                timestamp       TEXT    NOT NULL DEFAULT (datetime('now')),
                operation_type  TEXT    NOT NULL DEFAULT 'write'
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoints_message_id ON file_checkpoints(message_id);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_file_path  ON file_checkpoints(file_path);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_timestamp  ON file_checkpoints(timestamp);
            CREATE INDEX IF NOT EXISTS idx_checkpoints_operation  ON file_checkpoints(operation_type);

            CREATE INDEX IF NOT EXISTS idx_checkpoints_message_file 
                ON file_checkpoints(message_id, file_path);
            "
        )?;
        Ok(())
    }

    /// Create a new file checkpoint entry
    pub fn create(&self, input: CreateCheckpointInput) -> Result<FileCheckpoint> {
        self.conn.execute(
            "INSERT INTO file_checkpoints (message_id, file_path, file_content, operation_type)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                input.message_id,
                input.file_path,
                input.file_content,
                input.operation_type,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_by_id(id)?
            .ok_or_else(|| AutoDevError::Agent("Checkpoint not found after insert".into()))
    }

    /// Get a checkpoint by ID
    pub fn get_by_id(&self, id: i64) -> Result<Option<FileCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints WHERE id = ?1"
        )?;
        let checkpoint = stmt.query_row(params![id], row_to_checkpoint).optional()?;
        Ok(checkpoint)
    }

    /// List all checkpoints for a given message_id
    pub fn list_by_message_id(&self, message_id: &str) -> Result<Vec<FileCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints 
             WHERE message_id = ?1 
             ORDER BY timestamp ASC"
        )?;
        let checkpoints = stmt
            .query_map(params![message_id], row_to_checkpoint)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(checkpoints)
    }

    /// List all checkpoints for a given file_path
    pub fn list_by_file_path(&self, file_path: &str) -> Result<Vec<FileCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints 
             WHERE file_path = ?1 
             ORDER BY timestamp DESC"
        )?;
        let checkpoints = stmt
            .query_map(params![file_path], row_to_checkpoint)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(checkpoints)
    }

    /// Get the most recent checkpoint for a specific file
    pub fn get_latest_by_file_path(&self, file_path: &str) -> Result<Option<FileCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints 
             WHERE file_path = ?1 
             ORDER BY timestamp DESC 
             LIMIT 1"
        )?;
        let checkpoint = stmt.query_row(params![file_path], row_to_checkpoint).optional()?;
        Ok(checkpoint)
    }

    /// List checkpoints by operation type
    pub fn list_by_operation_type(&self, operation_type: &str) -> Result<Vec<FileCheckpoint>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints 
             WHERE operation_type = ?1 
             ORDER BY timestamp DESC"
        )?;
        let checkpoints = stmt
            .query_map(params![operation_type], row_to_checkpoint)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(checkpoints)
    }

    /// List all checkpoints (with optional limit)
    pub fn list_all(&self, limit: Option<i64>) -> Result<Vec<FileCheckpoint>> {
        let query = if let Some(l) = limit {
            format!(
                "SELECT id, message_id, file_path, file_content, timestamp, operation_type
                 FROM file_checkpoints 
                 ORDER BY timestamp DESC 
                 LIMIT {}"
            , l)
        } else {
            "SELECT id, message_id, file_path, file_content, timestamp, operation_type
             FROM file_checkpoints 
             ORDER BY timestamp DESC"
                .to_string()
        };

        let mut stmt = self.conn.prepare(&query)?;
        let checkpoints = stmt
            .query_map([], row_to_checkpoint)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(checkpoints)
    }

    /// Delete a checkpoint by ID
    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM file_checkpoints WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete all checkpoints for a given message_id
    pub fn delete_by_message_id(&self, message_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM file_checkpoints WHERE message_id = ?1",
            params![message_id],
        )?;
        Ok(())
    }

    /// Delete all checkpoints for a given file_path
    pub fn delete_by_file_path(&self, file_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM file_checkpoints WHERE file_path = ?1",
            params![file_path],
        )?;
        Ok(())
    }

    /// Count checkpoints
    pub fn count(&self) -> Result<i64> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM file_checkpoints")?;
        let count: i64 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }

    /// Count checkpoints for a specific message_id
    pub fn count_by_message_id(&self, message_id: &str) -> Result<i64> {
        let mut stmt =
            self.conn.prepare("SELECT COUNT(*) FROM file_checkpoints WHERE message_id = ?1")?;
        let count: i64 = stmt.query_row(params![message_id], |row| row.get(0))?;
        Ok(count)
    }

    /// Count checkpoints for a specific file
    pub fn count_by_file_path(&self, file_path: &str) -> Result<i64> {
        let mut stmt =
            self.conn.prepare("SELECT COUNT(*) FROM file_checkpoints WHERE file_path = ?1")?;
        let count: i64 = stmt.query_row(params![file_path], |row| row.get(0))?;
        Ok(count)
    }
}

fn row_to_checkpoint(row: &rusqlite::Row<'_>) -> rusqlite::Result<FileCheckpoint> {
    Ok(FileCheckpoint {
        id:             row.get(0)?,
        message_id:     row.get(1)?,
        file_path:      row.get(2)?,
        file_content:   row.get(3)?,
        timestamp:      row.get(4)?,
        operation_type: OperationType::from_str(&row.get::<_, String>(5)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_type_conversion() {
        assert_eq!(OperationType::Create.as_str(), "create");
        assert_eq!(OperationType::Write.as_str(), "write");
        assert_eq!(OperationType::Edit.as_str(), "edit");

        assert_eq!(OperationType::from_str("create"), OperationType::Create);
        assert_eq!(OperationType::from_str("write"), OperationType::Write);
        assert_eq!(OperationType::from_str("edit"), OperationType::Edit);
        assert_eq!(OperationType::from_str("unknown"), OperationType::Write); // default
    }
}
