// Tasks database — mirrors the schema in src-tauri/src/db/tasks.rs.
// Both binaries open the same ~/.autodev/tasks.db file.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{Result, SharedError};
use super::autodev_dir;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    P0, P1, P2, P3,
}
impl TaskPriority {
    pub fn as_str(&self) -> &str {
        match self { Self::P0 => "p0", Self::P1 => "p1", Self::P2 => "p2", Self::P3 => "p3" }
    }
    pub fn from_str(s: &str) -> Self {
        match s { "p0" => Self::P0, "p1" => Self::P1, "p3" => Self::P3, _ => Self::P2 }
    }
}
impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskType {
    Feature, Bug, Refactor, Docs, Chore,
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
impl std::fmt::Display for TaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Ready, Assigned, InProgress, QcReview, Done,
    BuildFailed, Failed, Cancelled,
}
impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ready => "ready", Self::Assigned => "assigned",
            Self::InProgress => "in_progress", Self::QcReview => "qc_review",
            Self::Done => "done", Self::BuildFailed => "build_failed",
            Self::Failed => "failed", Self::Cancelled => "cancelled",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "assigned" => Self::Assigned, "in_progress" => Self::InProgress,
            "qc_review" => Self::QcReview, "done" => Self::Done,
            "build_failed" => Self::BuildFailed, "failed" => Self::Failed,
            "cancelled" => Self::Cancelled, _ => Self::Ready,
        }
    }
}
impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id:                  i64,
    pub title:               String,
    pub description:         Option<String>,
    pub priority:            TaskPriority,
    pub task_type:           TaskType,
    pub status:              TaskStatus,
    pub acceptance_criteria: String,
    pub assigned_worker:     Option<String>,
    pub created_by:          String,
    pub created_at:          String,
    pub updated_at:          String,
    pub completed_at:        Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CreateTaskInput {
    pub title:               String,
    pub description:         Option<String>,
    pub priority:            Option<String>,
    pub task_type:           Option<String>,
    pub acceptance_criteria: Option<String>,
    pub created_by:          Option<String>,
}

// ─── Database ─────────────────────────────────────────────────────────────────

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
            "PRAGMA journal_mode = WAL;
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
                 created_by          TEXT    NOT NULL DEFAULT 'pa',
                 created_at          TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at          TEXT    NOT NULL DEFAULT (datetime('now')),
                 completed_at        TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_tasks_status   ON tasks(status);
             CREATE INDEX IF NOT EXISTS idx_tasks_assigned ON tasks(assigned_worker);
             CREATE TRIGGER IF NOT EXISTS tasks_updated_at
             AFTER UPDATE ON tasks
             BEGIN
                 UPDATE tasks SET updated_at = datetime('now') WHERE id = NEW.id;
             END;"
        )?;
        Ok(())
    }

    pub fn create(&self, input: CreateTaskInput) -> Result<Task> {
        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, task_type, acceptance_criteria, created_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input.title,
                input.description,
                input.priority.unwrap_or_else(|| "p2".into()),
                input.task_type.unwrap_or_else(|| "feature".into()),
                input.acceptance_criteria.unwrap_or_default(),
                input.created_by.unwrap_or_else(|| "airsctl".into()),
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get(id)?.ok_or_else(|| SharedError::Other("Task not found after insert".into()))
    }

    pub fn get(&self, id: i64) -> Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,description,priority,task_type,status,
                    acceptance_criteria,assigned_worker,created_by,
                    created_at,updated_at,completed_at
             FROM tasks WHERE id=?1"
        )?;
        Ok(stmt.query_row(params![id], row_to_task).optional()?)
    }

    pub fn list(&self, status_filter: Option<&str>, worker_filter: Option<&str>) -> Result<Vec<Task>> {
        let sql = match (status_filter, worker_filter) {
            (Some(_), Some(_)) =>
                "SELECT id,title,description,priority,task_type,status,acceptance_criteria,assigned_worker,created_by,created_at,updated_at,completed_at FROM tasks WHERE status=?1 AND assigned_worker=?2 ORDER BY created_at ASC",
            (Some(_), None) =>
                "SELECT id,title,description,priority,task_type,status,acceptance_criteria,assigned_worker,created_by,created_at,updated_at,completed_at FROM tasks WHERE status=?1 ORDER BY created_at ASC",
            (None, Some(_)) =>
                "SELECT id,title,description,priority,task_type,status,acceptance_criteria,assigned_worker,created_by,created_at,updated_at,completed_at FROM tasks WHERE assigned_worker=?1 ORDER BY created_at ASC",
            (None, None) =>
                "SELECT id,title,description,priority,task_type,status,acceptance_criteria,assigned_worker,created_by,created_at,updated_at,completed_at FROM tasks ORDER BY created_at ASC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let tasks = match (status_filter, worker_filter) {
            (Some(s), Some(w)) => stmt.query_map(params![s, w], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?,
            (Some(s), None)    => stmt.query_map(params![s], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?,
            (None, Some(w))    => stmt.query_map(params![w], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?,
            (None, None)       => stmt.query_map([], row_to_task)?.collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(tasks)
    }

    pub fn assign(&self, id: i64, worker: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET status='assigned', assigned_worker=?1 WHERE id=?2",
            params![worker, id],
        )?;
        Ok(())
    }

    /// Claim the next available task for a worker (oldest 'ready' task becomes 'in_progress').
    pub fn claim_next(&self, worker: &str) -> Result<Option<Task>> {
        let next = {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM tasks WHERE status='ready' ORDER BY created_at ASC LIMIT 1"
            )?;
            stmt.query_row([], |r| r.get::<_, i64>(0)).optional()?
        };
        if let Some(id) = next {
            self.conn.execute(
                "UPDATE tasks SET status='in_progress', assigned_worker=?1 WHERE id=?2",
                params![worker, id],
            )?;
            return self.get(id);
        }
        Ok(None)
    }

    pub fn update_status(&self, id: i64, status: &str, assigned_worker: Option<&str>) -> Result<()> {
        let completed_sql = if status == "done" { "datetime('now')" } else { "NULL" };
        match assigned_worker {
            Some(w) => {
                let sql = format!(
                    "UPDATE tasks SET status=?1, assigned_worker=?2, completed_at={completed_sql} WHERE id=?3"
                );
                self.conn.execute(&sql, params![status, w, id])?;
            }
            None => {
                let sql = format!(
                    "UPDATE tasks SET status=?1, completed_at={completed_sql} WHERE id=?2"
                );
                self.conn.execute(&sql, params![status, id])?;
            }
        }
        Ok(())
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id=?1", params![id])?;
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
        created_by:          row.get(8)?,
        created_at:          row.get(9)?,
        updated_at:          row.get(10)?,
        completed_at:        row.get(11)?,
    })
}
