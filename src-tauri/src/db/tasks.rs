// Task Board database — SQLite CRUD for the Internal AutoDev Task Board.
// Database path: ~/.autodev/tasks.db
// Used by: PA (create tasks in Delegate Mode), Supervisor (assign/update), Workers (claim/complete)
// See: DEVELOPMENT_PLAN.md Phase 2.1 — AutoDev Panel & Internal Board

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use super::autodev_dir;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    P0, // Critical
    P1, // High
    P2, // Medium
    P3, // Low
}

impl TaskPriority {
    pub fn as_str(&self) -> &str {
        match self { Self::P0 => "p0", Self::P1 => "p1", Self::P2 => "p2", Self::P3 => "p3" }
    }
    pub fn from_str(s: &str) -> Self {
        match s { "p0" => Self::P0, "p1" => Self::P1, "p3" => Self::P3, _ => Self::P2 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Feature,
    Bug,
    Refactor,
    Docs,
    Chore,
}

impl TaskType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Feature => "feature", Self::Bug => "bug",
            Self::Refactor => "refactor", Self::Docs => "docs", Self::Chore => "chore",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "bug" => Self::Bug, "refactor" => Self::Refactor,
            "docs" => Self::Docs, "chore" => Self::Chore, _ => Self::Feature,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Ready,        // Created, waiting for Supervisor assignment
    Assigned,     // Supervisor assigned to a worker
    InProgress,   // Worker is executing
    QcReview,     // Worker completed, QC reviewing
    Done,         // QC approved, merged
    BuildFailed,  // Dependencies/build validation failed (Fix 8d)
    Failed,       // Task failed — needs attention
    Cancelled,    // Manually cancelled
    Rejected,     // QC rejected — worker must rework and resubmit
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ready => "ready", Self::Assigned => "assigned",
            Self::InProgress => "in_progress", Self::QcReview => "qc_review",
            Self::Done => "done", Self::BuildFailed => "build_failed",
            Self::Failed => "failed", Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "assigned" => Self::Assigned, "in_progress" => Self::InProgress,
            "qc_review" => Self::QcReview, "done" => Self::Done,
            "build_failed" => Self::BuildFailed,
            "failed" => Self::Failed, "cancelled" => Self::Cancelled,
            "rejected" => Self::Rejected, _ => Self::Ready,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id:                   i64,
    pub title:                String,
    pub description:          Option<String>,
    pub priority:             TaskPriority,
    pub task_type:            TaskType,
    pub status:               TaskStatus,
    pub acceptance_criteria:  String,
    pub assigned_worker:      Option<String>,
    pub rejection_reason:     Option<String>,
    pub retry_count:          i64,
    pub created_by:           String,       // "pa" | "supervisor" | agent slug
    pub created_at:           String,
    pub updated_at:           String,
    pub completed_at:         Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub title:               String,
    pub description:         Option<String>,
    pub priority:            String, // "p0" | "p1" | "p2" | "p3"
    pub task_type:           String, // "feature" | "bug" | "refactor" | "docs" | "chore"
    pub acceptance_criteria: String,
    pub created_by:          Option<String>,
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct TasksDb {
    conn: Connection,
}

impl TasksDb {
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("tasks.db");
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

            CREATE TABLE IF NOT EXISTS tasks (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                title               TEXT    NOT NULL,
                description         TEXT,
                priority            TEXT    NOT NULL DEFAULT 'p2',
                task_type           TEXT    NOT NULL DEFAULT 'feature',
                status              TEXT    NOT NULL DEFAULT 'ready',
                acceptance_criteria TEXT    NOT NULL DEFAULT '',
                assigned_worker     TEXT,
                rejection_reason    TEXT,
                retry_count         INTEGER NOT NULL DEFAULT 0,
                created_by          TEXT    NOT NULL DEFAULT 'pa',
                created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at          TEXT    NOT NULL DEFAULT (datetime('now')),
                completed_at        TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status     ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_assigned   ON tasks(assigned_worker);
            CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);

            CREATE TRIGGER IF NOT EXISTS tasks_updated_at
            AFTER UPDATE ON tasks
            BEGIN
                UPDATE tasks SET updated_at = datetime('now') WHERE id = NEW.id;
            END;
            "
        )?;
        // Add columns to existing DBs that predate these migrations.
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN rejection_reason TEXT;"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;"
        );
        Ok(())
    }

    pub fn create(&self, input: CreateTaskInput) -> Result<Task> {
        let created_by = input.created_by.unwrap_or_else(|| "pa".to_string());
        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, task_type, status, acceptance_criteria, created_by)
             VALUES (?1, ?2, ?3, ?4, 'ready', ?5, ?6)",
            params![
                input.title,
                input.description,
                input.priority,
                input.task_type,
                input.acceptance_criteria,
                created_by,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_by_id(id)?.ok_or_else(|| AutoDevError::Agent("Task not found after insert".into()))
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at
             FROM tasks WHERE id = ?1"
        )?;
        let task = stmt.query_row(params![id], row_to_task).optional()?;
        Ok(task)
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<Task>> {
        let (query, values): (&str, Vec<String>) = if let Some(status) = status_filter {
            (
                "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at
                 FROM tasks WHERE status = ?1 ORDER BY created_at ASC",
                vec![status.to_string()],
            )
        } else {
            (
                "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at
                 FROM tasks ORDER BY created_at ASC",
                vec![],
            )
        };

        let mut stmt = self.conn.prepare(query)?;
        let tasks = if values.is_empty() {
            stmt.query_map([], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(tasks)
    }

    pub fn list_tasks_by_status_and_worker(&self, status: &str, worker: &str) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at
             FROM tasks WHERE status = ?1 AND assigned_worker = ?2 ORDER BY id"
        )?;
        let tasks = stmt.query_map(params![status, worker], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    pub fn update_status(&self, id: i64, status: &str, assigned_worker: Option<&str>) -> Result<()> {
        self.update_status_with_reason(id, status, assigned_worker, None)
    }

    /// Extended update that also persists a `rejection_reason` when status='rejected'.
    pub fn update_status_with_reason(
        &self,
        id: i64,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
    ) -> Result<()> {
        // completed_at is set atomically: datetime('now') on done, NULL on any other transition.
        // This ensures the "Done Today" filter stays accurate across status cycles.
        // updated_at is always refreshed so stall detection (supervisor) stays accurate.
        let completed_at_sql = if status == "done" { "datetime('now')" } else { "NULL" };
        if let Some(worker) = assigned_worker {
            let sql = format!(
                "UPDATE tasks SET status = ?1, assigned_worker = ?2, rejection_reason = ?3, \
                 updated_at = datetime('now'), completed_at = {completed_at_sql} WHERE id = ?4"
            );
            self.conn.execute(&sql, params![status, worker, rejection_reason, id])?;
        } else {
            let sql = format!(
                "UPDATE tasks SET status = ?1, rejection_reason = ?2, \
                 updated_at = datetime('now'), completed_at = {completed_at_sql} WHERE id = ?3"
            );
            self.conn.execute(&sql, params![status, rejection_reason, id])?;
        }
        Ok(())
    }

    /// Atomically increment retry_count for a task. Called by the Supervisor
    /// whenever it resets a build_failed task back to ready for another attempt.
    pub fn increment_retry(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn get_retry_count(&self, id: i64) -> Result<i64> {
        let count = self.conn.query_row(
            "SELECT retry_count FROM tasks WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete all tasks and reset the auto-increment counter.
    /// Used by the UI "Clear Board" action to start fresh.
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "DELETE FROM tasks;
             DELETE FROM sqlite_sequence WHERE name = 'tasks';"
        )?;
        Ok(())
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id:                  row.get(0)?,
        title:               row.get(1)?,
        description:         row.get(2)?,
        priority:            TaskPriority::from_str(&row.get::<_, String>(3)?),
        task_type:           TaskType::from_str(&row.get::<_, String>(4)?),
        status:              TaskStatus::from_str(&row.get::<_, String>(5)?),
        acceptance_criteria: row.get(6)?,
        assigned_worker:     row.get(7)?,
        rejection_reason:    row.get(8)?,
        retry_count:         row.get(9)?,
        created_by:          row.get(10)?,
        created_at:          row.get(11)?,
        updated_at:          row.get(12)?,
        completed_at:        row.get(13)?,
    })
}
