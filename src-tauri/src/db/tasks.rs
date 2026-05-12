// Task Board database — SQLite CRUD for the Internal AutoDev Task Board.
// Database path: ~/.autodev/tasks.db
// Used by: PA (create tasks in Delegate Mode), Supervisor (assign/update), Workers (claim/complete)
// See: DEVELOPMENT_PLAN.md Phase 2.1 — AutoDev Panel & Internal Board

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::errors::{AutoDevError, Result};
use super::autodev_dir;

// ─── Worker→Supervisor Request Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRequestStatus {
    Pending,       // Waiting for Supervisor to act
    Acknowledged,  // Supervisor has seen it and is processing
    Completed,     // Supervisor fulfilled the request (tasks created, reassigned, etc.)
    Rejected,      // Supervisor declined the request
}

impl WorkerRequestStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::Acknowledged => "acknowledged",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "acknowledged" => Self::Acknowledged,
            "completed" => Self::Completed,
            "rejected" => Self::Rejected,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRequest {
    pub id:                i64,
    pub worker_slug:       String,
    pub task_id:           Option<i64>,
    pub request_type:      String,   // "decompose_audit" | "reassign" | "needs_info" | "report_issue" | "request_review"
    pub payload:           String,   // JSON blob — structured data for the supervisor
    pub status:            WorkerRequestStatus,
    pub resolution_note:   Option<String>,
    pub created_at:        String,
    pub resolved_at:       Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkerRequestInput {
    pub worker_slug:  String,
    pub task_id:      Option<i64>,
    pub request_type: String,
    pub payload:      String,
}

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
    // PID-based orphan detection (Ruflo Feature 2)
    pub claimed_by_pid:       Option<i64>,
    pub claim_host:           Option<String>,
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

// ─── Routing History ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingHistoryEntry {
    pub id:           String,
    pub task_id:      String,
    pub task_title:   String,
    pub task_type:    String,
    pub worker_slug:  String,
    pub assigned_at:  String,
    pub outcome:      Option<String>,
    pub outcome_at:   Option<String>,
    pub duration_ms:  Option<i64>,
    pub tokens_used:  Option<i64>,
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

            CREATE TABLE IF NOT EXISTS worker_requests (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                worker_slug     TEXT    NOT NULL,
                task_id         INTEGER,
                request_type    TEXT    NOT NULL,
                payload         TEXT    NOT NULL DEFAULT '{}',
                status          TEXT    NOT NULL DEFAULT 'pending',
                resolution_note TEXT,
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                resolved_at     TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_worker_requests_status     ON worker_requests(status);
            CREATE INDEX IF NOT EXISTS idx_worker_requests_worker    ON worker_requests(worker_slug);
            CREATE INDEX IF NOT EXISTS idx_worker_requests_task      ON worker_requests(task_id);
            "
        )?;
        // Add columns to existing DBs that predate these migrations.
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN rejection_reason TEXT;"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;"
        );
        // Ruflo Feature 2 — PID-based orphan detection
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN claimed_by_pid INTEGER;"
        );
        let _ = self.conn.execute_batch(
            "ALTER TABLE tasks ADD COLUMN claim_host TEXT;"
        );
        // Routing history table (Ruflo Feature 1 — semantic task routing)
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS routing_history (
                id           TEXT PRIMARY KEY,
                task_id      TEXT NOT NULL,
                task_title   TEXT NOT NULL,
                task_type    TEXT NOT NULL,
                worker_slug  TEXT NOT NULL,
                assigned_at  TEXT NOT NULL,
                outcome      TEXT,
                outcome_at   TEXT,
                duration_ms  INTEGER,
                tokens_used  INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_routing_history_worker
                ON routing_history(worker_slug);
            CREATE INDEX IF NOT EXISTS idx_routing_history_task_type
                ON routing_history(task_type);
            CREATE INDEX IF NOT EXISTS idx_routing_history_outcome
                ON routing_history(outcome);
            "
        )?;
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
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host
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
                         created_by, created_at, updated_at, completed_at,
                         claimed_by_pid, claim_host
                 FROM tasks WHERE status = ?1 ORDER BY created_at ASC",
                vec![status.to_string()],
            )
        } else {
            (
                "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at,
                         claimed_by_pid, claim_host
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
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host
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

    // ═══════════════════════════════════════════════════════════════════════════
    // Worker Requests — worker→supervisor communication queue
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn create_worker_request(&self, input: &CreateWorkerRequestInput) -> Result<WorkerRequest> {
        self.conn.execute(
            "INSERT INTO worker_requests (worker_slug, task_id, request_type, payload, status)
             VALUES (?1, ?2, ?3, ?4, 'pending')",
            params![
                input.worker_slug,
                input.task_id,
                input.request_type,
                input.payload,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_worker_request_by_id(id)?.ok_or_else(|| AutoDevError::Agent("Worker request not found after insert".into()))
    }

    pub fn get_worker_request_by_id(&self, id: i64) -> Result<Option<WorkerRequest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, worker_slug, task_id, request_type, payload, status,
                    resolution_note, created_at, resolved_at
             FROM worker_requests WHERE id = ?1"
        )?;
        let req = stmt.query_row(params![id], row_to_worker_request).optional()?;
        Ok(req)
    }

    pub fn list_worker_requests(&self, status_filter: Option<&str>) -> Result<Vec<WorkerRequest>> {
        let (query, values): (&str, Vec<String>) = if let Some(status) = status_filter {
            (
                "SELECT id, worker_slug, task_id, request_type, payload, status,
                        resolution_note, created_at, resolved_at
                 FROM worker_requests WHERE status = ?1 ORDER BY created_at DESC",
                vec![status.to_string()],
            )
        } else {
            (
                "SELECT id, worker_slug, task_id, request_type, payload, status,
                        resolution_note, created_at, resolved_at
                 FROM worker_requests ORDER BY created_at DESC",
                vec![],
            )
        };
        let mut stmt = self.conn.prepare(query)?;
        let reqs = if values.is_empty() {
            stmt.query_map([], row_to_worker_request)?.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_worker_request)?.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(reqs)
    }

    pub fn update_worker_request_status(
        &self,
        id: i64,
        status: &str,
        resolution_note: Option<&str>,
    ) -> Result<()> {
        let resolved_at = if status == "completed" || status == "rejected" {
            "datetime('now')"
        } else {
            "NULL"
        };
        let sql = format!(
            "UPDATE worker_requests SET status = ?1, resolution_note = ?2, resolved_at = {}
             WHERE id = ?3",
            resolved_at
        );
        self.conn.execute(&sql, params![status, resolution_note, id])?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // PID-Based Orphan Detection (Ruflo Feature 2)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Claim a task and record the worker's PID for orphan detection.
    /// Transitions status from `assigned` → `in_progress`.
    pub fn claim_task_with_pid(&self, id: i64, worker_slug: &str, pid: u32) -> Result<()> {
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        self.conn.execute(
            "UPDATE tasks
             SET status = 'in_progress', assigned_worker = ?1,
                 claimed_by_pid = ?2, claim_host = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4 AND status = 'assigned'",
            params![worker_slug, pid as i64, host, id],
        )?;
        Ok(())
    }

    /// Returns all tasks in `assigned` or `in_progress` state — candidates for
    /// orphan checking. Both PID-tracked and non-PID (TTL fallback) tasks included.
    pub fn get_orphan_candidates(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host
             FROM tasks
             WHERE status IN ('assigned', 'in_progress')
             ORDER BY updated_at ASC"
        )?;
        let tasks = stmt.query_map([], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    /// Release an orphaned task back to `ready`, clearing assignment and PID data.
    pub fn release_orphaned_task(&self, id: i64, reason: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks
             SET status = 'ready', assigned_worker = NULL,
                 claimed_by_pid = NULL, claim_host = NULL,
                 rejection_reason = ?1,
                 updated_at = datetime('now')
             WHERE id = ?2",
            params![format!("[orphan] {reason}"), id],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Routing History — Supervisor assignment records for affinity learning
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_routing_history(&self, entry: &RoutingHistoryEntry) -> Result<()> {
        self.conn.execute(
            "INSERT INTO routing_history
             (id, task_id, task_title, task_type, worker_slug, assigned_at,
              outcome, outcome_at, duration_ms, tokens_used)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                entry.id,
                entry.task_id,
                entry.task_title,
                entry.task_type,
                entry.worker_slug,
                entry.assigned_at,
                entry.outcome,
                entry.outcome_at,
                entry.duration_ms,
                entry.tokens_used,
            ],
        )?;
        Ok(())
    }

    pub fn update_routing_outcome(
        &self,
        task_id: &str,
        outcome: &str,
        duration_ms: i64,
        tokens_used: i64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE routing_history
             SET outcome = ?1, outcome_at = datetime('now'),
                 duration_ms = CASE WHEN ?2 > 0 THEN ?2 ELSE duration_ms END,
                 tokens_used = CASE WHEN ?3 > 0 THEN ?3 ELSE tokens_used END
             WHERE task_id = ?4 AND outcome IS NULL",
            params![outcome, duration_ms, tokens_used, task_id],
        )?;
        Ok(())
    }

    pub fn get_routing_history(&self, limit: i64) -> Result<Vec<RoutingHistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_id, task_title, task_type, worker_slug, assigned_at,
                    outcome, outcome_at, duration_ms, tokens_used
             FROM routing_history
             ORDER BY assigned_at DESC LIMIT ?1"
        )?;
        let entries = stmt.query_map(params![limit], |row| {
            Ok(RoutingHistoryEntry {
                id:          row.get(0)?,
                task_id:     row.get(1)?,
                task_title:  row.get(2)?,
                task_type:   row.get(3)?,
                worker_slug: row.get(4)?,
                assigned_at: row.get(5)?,
                outcome:     row.get(6)?,
                outcome_at:  row.get(7)?,
                duration_ms: row.get(8)?,
                tokens_used: row.get(9)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    /// Returns per-worker success rate keyed as "{worker_slug}:{task_type}" → rate (0.0–1.0).
    /// Only workers with at least 1 completed (success or failure) entry are included.
    pub fn get_worker_success_rates(&self) -> Result<HashMap<String, f64>> {
        let mut stmt = self.conn.prepare(
            "SELECT worker_slug, task_type,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) AS successes,
                    SUM(CASE WHEN outcome IN ('success','failure') THEN 1 ELSE 0 END) AS total
             FROM routing_history
             WHERE outcome IS NOT NULL
             GROUP BY worker_slug, task_type
             HAVING total > 0"
        )?;
        let mut map = HashMap::new();
        let rows = stmt.query_map([], |row| {
            let worker: String = row.get(0)?;
            let task_type: String = row.get(1)?;
            let successes: f64 = row.get::<_, i64>(2)? as f64;
            let total: f64 = row.get::<_, i64>(3)? as f64;
            Ok((worker, task_type, successes / total))
        })?;
        for row in rows {
            let (worker, task_type, rate) = row?;
            map.insert(format!("{worker}:{task_type}"), rate);
        }
        Ok(map)
    }

    /// Aggregate routing_history into per-worker, per-task-type affinity stats.
    /// Returns rows ordered by worker_slug then task_type.
    pub fn get_affinity_stats(&self) -> Result<Vec<(String, String, i64, i64, i64, Option<i64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT worker_slug, task_type,
                    COUNT(*) AS total,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) AS successes,
                    SUM(CASE WHEN outcome = 'failure' THEN 1 ELSE 0 END) AS failures,
                    CAST(AVG(CASE WHEN duration_ms > 0 THEN duration_ms END) AS INTEGER) AS avg_dur
             FROM routing_history
             GROUP BY worker_slug, task_type
             ORDER BY worker_slug, task_type"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<i64>>(5)?,
            ))
        })?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
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
        claimed_by_pid:      row.get(14).unwrap_or(None),
        claim_host:          row.get(15).unwrap_or(None),
    })
}

fn row_to_worker_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkerRequest> {
    Ok(WorkerRequest {
        id:               row.get(0)?,
        worker_slug:      row.get(1)?,
        task_id:          row.get(2)?,
        request_type:     row.get(3)?,
        payload:          row.get(4)?,
        status:           WorkerRequestStatus::from_str(&row.get::<_, String>(5)?),
        resolution_note:  row.get(6)?,
        created_at:       row.get(7)?,
        resolved_at:      row.get(8)?,
    })
}
