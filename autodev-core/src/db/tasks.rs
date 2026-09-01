// Task Board database — SQLite CRUD for the Internal AutoDev Task Board.
// Database path: ~/.autodev/tasks.db
// Used by: PA (create tasks in Delegate Mode), Supervisor (assign/update), Workers (claim/complete)
// See: DEVELOPMENT_PLAN.md Phase 2.1 — AutoDev Panel & Internal Board

use super::autodev_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Worker→Supervisor Request Types ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRequestStatus {
    Pending,      // Waiting for Supervisor to act
    Acknowledged, // Supervisor has seen it and is processing
    Completed,    // Supervisor fulfilled the request (tasks created, reassigned, etc.)
    Rejected,     // Supervisor declined the request
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
    pub id: i64,
    pub worker_slug: String,
    pub task_id: Option<i64>,
    pub request_type: String, // "decompose_audit" | "reassign" | "needs_info" | "report_issue" | "request_review"
    pub payload: String,      // JSON blob — structured data for the supervisor
    pub status: WorkerRequestStatus,
    pub resolution_note: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// AutoDev Communication Protocol (ACP) — structured worker↔worker message.
///
/// Mediated inbox: the sender writes a row, the target reads its inbox on its
/// next tick via list_worker_messages and acknowledges via
/// acknowledge_worker_message. The Supervisor sees every message (read path
/// is just `SELECT *`) and a UI panel renders the transcript per task.
///
/// Kinds:
///   nudge      — "look at this, no answer expected"
///   question   — "please answer this; I'm waiting"
///   answer     — "here's the answer to your question (reply chain)"
///   fyi        — informational; lowest priority
///   broadcast  — supervisor-only; fan-out one row per fleet worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMessage {
    pub id: i64,
    pub from_slug: String,
    pub to_slug: String,
    pub body: String,
    pub kind: String,
    pub task_id: Option<i64>,
    pub status: String, // "pending" | "read" | "acknowledged"
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateWorkerRequestInput {
    pub worker_slug: String,
    pub task_id: Option<i64>,
    pub request_type: String,
    pub payload: String,
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
        match self {
            Self::P0 => "p0",
            Self::P1 => "p1",
            Self::P2 => "p2",
            Self::P3 => "p3",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "p0" => Self::P0,
            "p1" => Self::P1,
            "p3" => Self::P3,
            _ => Self::P2,
        }
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
    Epic, // Ruflo Feature 5 — parent grouping for phased work
}

impl TaskType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Feature => "feature",
            Self::Bug => "bug",
            Self::Refactor => "refactor",
            Self::Docs => "docs",
            Self::Chore => "chore",
            Self::Epic => "epic",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "bug" => Self::Bug,
            "refactor" => Self::Refactor,
            "docs" => Self::Docs,
            "chore" => Self::Chore,
            "epic" => Self::Epic,
            _ => Self::Feature,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Ready,       // Created, waiting for Supervisor assignment
    Assigned,    // Supervisor assigned to a worker
    InProgress,  // Worker is executing
    QcReview,    // Worker completed, QC reviewing
    Done,        // QC approved, merged
    BuildFailed, // Dependencies/build validation failed (Fix 8d)
    Failed,      // Task failed — needs attention
    Cancelled,   // Manually cancelled
    Rejected,    // QC rejected — worker must rework and resubmit
    NeedsInfo,   // Budget exceeded or blocked — awaiting Supervisor/human guidance
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Ready => "ready",
            Self::Assigned => "assigned",
            Self::InProgress => "in_progress",
            Self::QcReview => "qc_review",
            Self::Done => "done",
            Self::BuildFailed => "build_failed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Rejected => "rejected",
            Self::NeedsInfo => "needs_info",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "assigned" => Self::Assigned,
            "in_progress" => Self::InProgress,
            "qc_review" => Self::QcReview,
            "done" => Self::Done,
            "build_failed" => Self::BuildFailed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "rejected" => Self::Rejected,
            "needs_info" => Self::NeedsInfo,
            _ => Self::Ready,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub acceptance_criteria: String,
    pub assigned_worker: Option<String>,
    pub rejection_reason: Option<String>,
    pub retry_count: i64,
    pub created_by: String, // "pa" | "supervisor" | agent slug
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    // PID-based orphan detection (Ruflo Feature 2)
    pub claimed_by_pid: Option<i64>,
    pub claim_host: Option<String>,
    // Diff risk analysis result (Ruflo Feature 3) — JSON blob from DiffAnalysisResult
    pub diff_analysis: Option<String>,
    // Budget Circuit Breaker (Ruflo Feature 4) — None when no budget was set
    pub budget_max_tokens: Option<i64>,
    pub budget_max_usd: Option<f64>,
    pub budget_max_turns: Option<i64>,
    pub budget_tokens_used: i64,
    pub budget_cost_usd: f64,
    pub budget_turns_used: i64,
    pub budget_exceeded: bool,
    pub budget_exceeded_reason: Option<String>,
    // Phase-Based Task Decomposition (Ruflo Feature 5)
    pub phase: Option<String>, // e.g. "foundation" | "core" | "integration" | "qa"
    pub phase_order: Option<i32>, // 1, 2, 3, 4 — controls sequencing within an epic
    pub milestone: Option<String>, // optional milestone marker for this phase
    pub epic_id: Option<String>, // links subtask back to its parent epic task id
    pub blocked_by: Option<Vec<String>>, // task IDs that must be done before this one starts
    // Doc 04 Phase 2 — per-task QC reviewer claim. Set atomically by
    // `spawn_qc_review` to the subagent slug; cleared on terminal status by
    // the stranded-reviewer reconciler. NULL = available for the next
    // dispatcher tick to claim.
    pub under_review_by: Option<String>,
    // Doc 04 Phase 4 — running QC cost in USD for this task. Aggregated
    // across all reviewer runs (including retries) so the QC ROI panel can
    // show qc_cost / worker_cost ratios per task.
    pub task_qc_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: String,  // "p0" | "p1" | "p2" | "p3"
    pub task_type: String, // "feature" | "bug" | "refactor" | "docs" | "chore" | "epic"
    pub acceptance_criteria: String,
    pub created_by: Option<String>,
}

/// Input for creating a task inside an epic with phase metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePhasedTaskInput {
    pub epic_id: String,
    pub phase: String,
    pub phase_order: i32,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub task_type: String,
    pub acceptance_criteria: String,
    pub blocked_by: Vec<String>, // task IDs that must complete first
    pub milestone: Option<String>,
    pub created_by: Option<String>,
}

/// Per-phase progress summary for an epic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseSummary {
    pub phase: String,
    pub phase_order: i32,
    pub milestone: Option<String>,
    pub total_tasks: i32,
    pub done_tasks: i32,
    pub in_progress_tasks: i32,
    pub ready_tasks: i32,
    pub blocked_tasks: i32,
    pub is_complete: bool, // true when done_tasks == total_tasks
    pub is_unlocked: bool, // true when previous phase is complete (or this is phase 1)
}

// ─── Routing History ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingHistoryEntry {
    pub id: String,
    pub task_id: String,
    pub task_title: String,
    pub task_type: String,
    pub worker_slug: String,
    pub assigned_at: String,
    pub outcome: Option<String>,
    pub outcome_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub tokens_used: Option<i64>,
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct TasksDb {
    conn: Connection,
}

impl TasksDb {
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
        let path = autodev_dir()?.join("tasks.db");
        let conn = crate::db::open_tuned(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// `tasks.db`'s ladder.
    ///
    /// `v1` is the schema as it stands. The twenty columns below are **not**
    /// separate rungs: every live `tasks.db` already has them, so
    /// `ALTER TABLE … ADD COLUMN` as a `v2` would fail with *duplicate column
    /// name*. They are applied by `ensure_columns` before the ladder, which
    /// checks `PRAGMA table_info` first.
    ///
    /// Until P1 these twenty were `let _ = execute_batch("ALTER TABLE …")` —
    /// twenty swallowed results. That idiom cannot distinguish "the column is
    /// already there" from "this statement is wrong", which is precisely how
    /// the graph store stamped `user_version = 5` over six failed statements
    /// (`ADR-LOG-RESTRUCT-0013`).
    const LADDER: &[Migration] = &[Migration {
        version: 1,
        name: "tasks-baseline",
        sql: r#"
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

            -- AutoDev Communication Protocol — structured worker↔worker inbox.
            -- One row per (sender, target) pair; a broadcast inserts N rows.
            CREATE TABLE IF NOT EXISTS worker_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                from_slug   TEXT    NOT NULL,
                to_slug     TEXT    NOT NULL,
                body        TEXT    NOT NULL,
                kind        TEXT    NOT NULL DEFAULT 'nudge',
                task_id     INTEGER,
                status      TEXT    NOT NULL DEFAULT 'pending',
                created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                read_at     TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_worker_messages_to_status
                ON worker_messages(to_slug, status);
            CREATE INDEX IF NOT EXISTS idx_worker_messages_task
                ON worker_messages(task_id);
            CREATE INDEX IF NOT EXISTS idx_worker_messages_created
                ON worker_messages(created_at);
        "#,
    }];

    /// Columns added to `tasks` after its original `CREATE TABLE`.
    const ADDED_COLUMNS: &[(&str, &str)] = &[
        (
            "rejection_reason",
            "ALTER TABLE tasks ADD COLUMN rejection_reason TEXT;",
        ),
        (
            "retry_count",
            "ALTER TABLE tasks ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0;",
        ),
        (
            "claimed_by_pid",
            "ALTER TABLE tasks ADD COLUMN claimed_by_pid INTEGER;",
        ),
        (
            "claim_host",
            "ALTER TABLE tasks ADD COLUMN claim_host TEXT;",
        ),
        (
            "diff_analysis",
            "ALTER TABLE tasks ADD COLUMN diff_analysis TEXT;",
        ),
        (
            "budget_max_tokens",
            "ALTER TABLE tasks ADD COLUMN budget_max_tokens INTEGER;",
        ),
        (
            "budget_max_usd",
            "ALTER TABLE tasks ADD COLUMN budget_max_usd    REAL;",
        ),
        (
            "budget_max_turns",
            "ALTER TABLE tasks ADD COLUMN budget_max_turns  INTEGER DEFAULT 20;",
        ),
        (
            "budget_tokens_used",
            "ALTER TABLE tasks ADD COLUMN budget_tokens_used INTEGER DEFAULT 0;",
        ),
        (
            "budget_cost_usd",
            "ALTER TABLE tasks ADD COLUMN budget_cost_usd   REAL DEFAULT 0.0;",
        ),
        (
            "budget_turns_used",
            "ALTER TABLE tasks ADD COLUMN budget_turns_used INTEGER DEFAULT 0;",
        ),
        (
            "budget_exceeded",
            "ALTER TABLE tasks ADD COLUMN budget_exceeded   INTEGER DEFAULT 0;",
        ),
        (
            "budget_exceeded_reason",
            "ALTER TABLE tasks ADD COLUMN budget_exceeded_reason TEXT;",
        ),
        ("phase", "ALTER TABLE tasks ADD COLUMN phase       TEXT;"),
        (
            "phase_order",
            "ALTER TABLE tasks ADD COLUMN phase_order INTEGER;",
        ),
        (
            "milestone",
            "ALTER TABLE tasks ADD COLUMN milestone   TEXT;",
        ),
        ("epic_id", "ALTER TABLE tasks ADD COLUMN epic_id     TEXT;"),
        (
            "blocked_by",
            "ALTER TABLE tasks ADD COLUMN blocked_by  TEXT;",
        ),
        (
            "under_review_by",
            "ALTER TABLE tasks ADD COLUMN under_review_by  TEXT;",
        ),
        (
            "task_qc_cost_usd",
            "ALTER TABLE tasks ADD COLUMN task_qc_cost_usd REAL DEFAULT 0.0;",
        ),
    ];

    fn migrate(&self) -> Result<()> {
        // PRAGMAs are per-connection and are not carried by the ladder.
        self.conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;",
        )?;
        migrate::ensure_columns(&self.conn, "tasks", Self::ADDED_COLUMNS)?;
        migrate::run_ladder(&self.conn, "tasks.db", Self::LADDER)?;
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
        self.get_by_id(id)?
            .ok_or_else(|| AutoDevError::Agent("Task not found after insert".into()))
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE id = ?1",
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
                         claimed_by_pid, claim_host, diff_analysis,
                         budget_max_tokens, budget_max_usd, budget_max_turns,
                         COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                         COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                         budget_exceeded_reason,
                         phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
                 FROM tasks WHERE status = ?1 ORDER BY created_at ASC",
                vec![status.to_string()],
            )
        } else {
            (
                "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at,
                         claimed_by_pid, claim_host, diff_analysis,
                         budget_max_tokens, budget_max_usd, budget_max_turns,
                         COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                         COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                         budget_exceeded_reason,
                         phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
                 FROM tasks ORDER BY created_at ASC",
                vec![],
            )
        };

        let mut stmt = self.conn.prepare(query)?;
        let tasks = if values.is_empty() {
            stmt.query_map([], row_to_task)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_task)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(tasks)
    }

    pub fn list_tasks_by_status_and_worker(&self, status: &str, worker: &str) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE status = ?1 AND assigned_worker = ?2 ORDER BY id",
        )?;
        let tasks = stmt
            .query_map(params![status, worker], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    pub fn update_status(
        &self,
        id: i64,
        status: &str,
        assigned_worker: Option<&str>,
    ) -> Result<()> {
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
        let completed_at_sql = if status == "done" {
            "datetime('now')"
        } else {
            "NULL"
        };
        if let Some(worker) = assigned_worker {
            let sql = format!(
                "UPDATE tasks SET status = ?1, assigned_worker = ?2, rejection_reason = ?3, \
                 updated_at = datetime('now'), completed_at = {completed_at_sql} WHERE id = ?4"
            );
            self.conn
                .execute(&sql, params![status, worker, rejection_reason, id])?;
        } else {
            let sql = format!(
                "UPDATE tasks SET status = ?1, rejection_reason = ?2, \
                 updated_at = datetime('now'), completed_at = {completed_at_sql} WHERE id = ?3"
            );
            self.conn
                .execute(&sql, params![status, rejection_reason, id])?;
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
        self.conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Delete all tasks and reset the auto-increment counter.
    /// Used by the UI "Clear Board" action to start fresh.
    ///
    /// IMPORTANT: `worker_requests(task_id) REFERENCES tasks(id)` is an
    /// enforced foreign key (`PRAGMA foreign_keys = ON` set in `init`).
    /// Bare `DELETE FROM tasks` fails the moment any pending worker request
    /// references a task — which is the normal state after a run with QC
    /// activity. We therefore drain `worker_requests` first, then `tasks`,
    /// then reset both auto-increment sequences atomically so the next
    /// `INSERT` starts from id 1 again.
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute_batch(
            "BEGIN;
             DELETE FROM worker_requests;
             DELETE FROM tasks;
             DELETE FROM sqlite_sequence WHERE name IN ('tasks', 'worker_requests');
             COMMIT;",
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
        self.get_worker_request_by_id(id)?
            .ok_or_else(|| AutoDevError::Agent("Worker request not found after insert".into()))
    }

    pub fn get_worker_request_by_id(&self, id: i64) -> Result<Option<WorkerRequest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, worker_slug, task_id, request_type, payload, status,
                    resolution_note, created_at, resolved_at
             FROM worker_requests WHERE id = ?1",
        )?;
        let req = stmt
            .query_row(params![id], row_to_worker_request)
            .optional()?;
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
            stmt.query_map([], row_to_worker_request)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_worker_request)?
                .collect::<std::result::Result<Vec<_>, _>>()?
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
        self.conn
            .execute(&sql, params![status, resolution_note, id])?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // AutoDev Communication Protocol (ACP) — worker↔worker mediated inbox
    // ═══════════════════════════════════════════════════════════════════════════

    /// Insert a single worker message and return its persisted row.
    pub fn insert_worker_message(
        &self,
        from_slug: &str,
        to_slug: &str,
        body: &str,
        kind: &str,
        task_id: Option<i64>,
    ) -> Result<WorkerMessage> {
        self.conn.execute(
            "INSERT INTO worker_messages (from_slug, to_slug, body, kind, task_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from_slug, to_slug, body, kind, task_id],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_worker_message(id)?.ok_or_else(|| {
            AutoDevError::Agent("Worker message inserted but vanished on read-back".into())
        })
    }

    pub fn get_worker_message(&self, id: i64) -> Result<Option<WorkerMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages WHERE id = ?1",
        )?;
        let msg = stmt
            .query_row(params![id], row_to_worker_message)
            .optional()?;
        Ok(msg)
    }

    /// List messages addressed to `to_slug`. When `unread_only` is true, only
    /// returns messages whose status is `pending`. Listing pending messages
    /// transitions them to `read` and stamps `read_at` so the supervisor's
    /// drain can distinguish "never seen" from "seen but un-acked."
    pub fn list_worker_messages_for(
        &self,
        to_slug: &str,
        unread_only: bool,
    ) -> Result<Vec<WorkerMessage>> {
        // Step 1: snapshot the rows we're about to return so the caller sees
        // the row's `status='pending'` state as it was when they listed
        // (rather than racing the UPDATE below). The mark-read transition is
        // applied immediately afterwards under the same connection so any
        // follow-up read sees `read`.
        let sql = if unread_only {
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE to_slug = ?1 AND status = 'pending'
             ORDER BY created_at ASC"
        } else {
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE to_slug = ?1
             ORDER BY created_at ASC"
        };
        let mut stmt = self.conn.prepare(sql)?;
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![to_slug], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        // Mark the pending ones we just returned as read.
        if !messages.is_empty() {
            let pending_ids: Vec<i64> = messages
                .iter()
                .filter(|m| m.status == "pending")
                .map(|m| m.id)
                .collect();
            if !pending_ids.is_empty() {
                // Build an IN-list bind safely.
                let placeholders: Vec<String> = (0..pending_ids.len())
                    .map(|i| format!("?{}", i + 1))
                    .collect();
                let sql = format!(
                    "UPDATE worker_messages SET status='read', read_at=datetime('now')
                     WHERE id IN ({}) AND status='pending'",
                    placeholders.join(",")
                );
                let params: Vec<&dyn rusqlite::ToSql> = pending_ids
                    .iter()
                    .map(|id| id as &dyn rusqlite::ToSql)
                    .collect();
                self.conn.execute(&sql, params.as_slice())?;
            }
        }
        Ok(messages)
    }

    /// Mark a message as acknowledged. Caller must verify ownership
    /// (`to_slug` matches the calling agent) at the tool layer.
    pub fn acknowledge_worker_message(&self, id: i64) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE worker_messages
             SET status = 'acknowledged'
             WHERE id = ?1 AND status IN ('pending','read')",
            params![id],
        )?;
        if rows == 0 {
            return Err(AutoDevError::Agent(format!(
                "Worker message #{id} not found or already acknowledged."
            )));
        }
        Ok(())
    }

    /// Supervisor drain: messages still pending OR read (un-acked) for more
    /// than `older_than_minutes`. Used by the Supervisor recipe to surface
    /// stuck cross-worker conversations as escalations.
    pub fn list_stale_worker_messages(
        &self,
        older_than_minutes: i64,
    ) -> Result<Vec<WorkerMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE status IN ('pending','read')
               AND datetime(created_at) <= datetime('now', ?1)
             ORDER BY created_at ASC",
        )?;
        let offset = format!("-{older_than_minutes} minutes");
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![offset], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Fleet Chat panel — full message list for one task, in chronological
    /// order. Returns at most `limit` rows.
    pub fn list_worker_messages_for_task(
        &self,
        task_id: i64,
        limit: i64,
    ) -> Result<Vec<WorkerMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE task_id = ?1
             ORDER BY created_at ASC
             LIMIT ?2",
        )?;
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![task_id, limit], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Fleet Chat panel — all messages across all tasks, newest first.
    /// Bounded by `limit` so the UI never pulls an unbounded backlog.
    pub fn list_recent_worker_messages(&self, limit: i64) -> Result<Vec<WorkerMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![limit], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    /// Per-target unread count, used by the AgentLiveCard badge.
    pub fn count_unread_for(&self, to_slug: &str) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM worker_messages
             WHERE to_slug = ?1 AND status = 'pending'",
            params![to_slug],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // PID-Based Orphan Detection (Ruflo Feature 2)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Claim a task and record the worker's PID for orphan detection.
    /// Transitions status from `assigned` → `in_progress`.
    ///
    /// Bug J defensive: refuse the claim when the same worker already owns
    /// another task in `in_progress`. The worker recipes already say in
    /// natural language "NEVER claim a second task while one is in_progress",
    /// but that was prompt-level discipline only. This is the code-side
    /// backstop — a bug in dispatch, a glitch in the LLM, or an aggressive
    /// retry can no longer produce a double-claim.
    pub fn claim_task_with_pid(&self, id: i64, worker_slug: &str, pid: u32) -> Result<()> {
        // Pre-flight: count this worker's currently-in-progress tasks, ignoring
        // the one we're about to claim (in case of an idempotent retry that
        // somehow already transitioned the row).
        let active: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks
             WHERE assigned_worker = ?1 AND status = 'in_progress' AND id != ?2",
                params![worker_slug, id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        if active > 0 {
            return Err(AutoDevError::Agent(format!(
                "Worker '{worker_slug}' already owns {active} in_progress task(s); \
                 finish or release them before claiming task #{id}. This guard \
                 backs the worker recipe's 'NEVER claim a second task while one \
                 is in_progress' rule."
            )));
        }

        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        let rows = self.conn.execute(
            "UPDATE tasks
             SET status = 'in_progress', assigned_worker = ?1,
                 claimed_by_pid = ?2, claim_host = ?3,
                 updated_at = datetime('now')
             WHERE id = ?4 AND status = 'assigned'",
            params![worker_slug, pid as i64, host, id],
        )?;
        if rows == 0 {
            // The atomic WHERE guard refused the transition — task not in
            // 'assigned' state. Caller can fetch the row to discover why.
            return Err(AutoDevError::Agent(format!(
                "Task #{id} could not be claimed by '{worker_slug}' — it is no \
                 longer in status='assigned'. Another worker may have claimed it, \
                 the supervisor may have reassigned it, or QC may have intervened."
            )));
        }
        Ok(())
    }

    /// Returns all tasks in `assigned` or `in_progress` state — candidates for
    /// orphan checking. Both PID-tracked and non-PID (TTL fallback) tasks included.
    pub fn get_orphan_candidates(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks
             WHERE status IN ('assigned', 'in_progress')
             ORDER BY updated_at ASC",
        )?;
        let tasks = stmt
            .query_map([], row_to_task)?
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
             ORDER BY assigned_at DESC LIMIT ?1",
        )?;
        let entries = stmt
            .query_map(params![limit], |row| {
                Ok(RoutingHistoryEntry {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    task_title: row.get(2)?,
                    task_type: row.get(3)?,
                    worker_slug: row.get(4)?,
                    assigned_at: row.get(5)?,
                    outcome: row.get(6)?,
                    outcome_at: row.get(7)?,
                    duration_ms: row.get(8)?,
                    tokens_used: row.get(9)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
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
             HAVING total > 0",
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

    // ═══════════════════════════════════════════════════════════════════════════
    // Diff Risk Analysis — Ruflo Feature 3
    // ═══════════════════════════════════════════════════════════════════════════

    /// Store a serialized DiffAnalysisResult JSON on a task so the QC review
    /// outcome is available later for audit and so the AutoDev panel can render
    /// the risk card without re-running the analysis on every refresh.
    pub fn set_task_diff_analysis(&self, task_id: i64, analysis_json: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET diff_analysis = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![analysis_json, task_id],
        )?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Budget Circuit Breaker — Ruflo Feature 4
    // ═══════════════════════════════════════════════════════════════════════════

    /// Set the budget caps for a task (called by Supervisor before assigning).
    pub fn set_task_budget(
        &self,
        task_id: i64,
        max_tokens: Option<u64>,
        max_usd: Option<f64>,
        max_turns: Option<u32>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET budget_max_tokens = ?1, budget_max_usd = ?2,
             budget_max_turns = ?3, updated_at = datetime('now') WHERE id = ?4",
            rusqlite::params![
                max_tokens.map(|t| t as i64),
                max_usd,
                max_turns.map(|t| t as i64),
                task_id,
            ],
        )?;
        Ok(())
    }

    /// Persist cumulative budget usage after each agent turn.
    pub fn update_task_budget_usage(
        &self,
        task_id: i64,
        tokens_used: u64,
        cost_usd: f64,
        turns_used: u32,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET budget_tokens_used = ?1, budget_cost_usd = ?2,
             budget_turns_used = ?3, updated_at = datetime('now') WHERE id = ?4",
            rusqlite::params![tokens_used as i64, cost_usd, turns_used as i64, task_id],
        )?;
        Ok(())
    }

    /// Mark a task as budget-exceeded and transition it to needs_info.
    pub fn mark_task_budget_exceeded(&self, task_id: i64, reason: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET budget_exceeded = 1, budget_exceeded_reason = ?1,
             status = 'needs_info', updated_at = datetime('now') WHERE id = ?2",
            rusqlite::params![reason, task_id],
        )?;
        Ok(())
    }

    /// Read back the current budget usage for a task.
    pub fn get_task_budget_status(&self, task_id: i64) -> Result<crate::budget::BudgetStatus> {
        let row = self.conn.query_row(
            "SELECT budget_tokens_used, budget_cost_usd, budget_turns_used,
                    budget_exceeded, budget_exceeded_reason
             FROM tasks WHERE id = ?1",
            rusqlite::params![task_id],
            |row| {
                Ok((
                    row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                    row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                    row.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )?;
        let exceeded_reason = row.4.as_deref().map(|r| match r {
            "BUDGET_EXCEEDED_TOKENS" => crate::budget::BudgetExceededReason::TokenLimitReached,
            "BUDGET_EXCEEDED_USD" => crate::budget::BudgetExceededReason::CostLimitReached,
            _ => crate::budget::BudgetExceededReason::TurnLimitReached,
        });
        Ok(crate::budget::BudgetStatus {
            tokens_used: row.0,
            cost_usd: row.1,
            turns_used: row.2,
            exceeded: row.3,
            exceeded_reason,
        })
    }

    /// Aggregate today's budget usage across all tasks for the daily summary.
    pub fn get_daily_budget_summary(&self) -> Result<crate::budget::DailyBudgetSummary> {
        let row = self.conn.query_row(
            "SELECT
               COALESCE(SUM(CASE WHEN budget_max_tokens IS NOT NULL THEN budget_max_tokens ELSE 0 END), 0),
               COALESCE(SUM(budget_tokens_used), 0),
               COALESCE(SUM(CASE WHEN budget_max_usd IS NOT NULL THEN budget_max_usd ELSE 0.0 END), 0.0),
               COALESCE(SUM(budget_cost_usd), 0.0),
               COALESCE(SUM(CASE WHEN budget_exceeded = 1 THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN budget_max_tokens IS NOT NULL OR budget_max_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
             FROM tasks
             WHERE date(created_at) = date('now')",
            [],
            |row| Ok(crate::budget::DailyBudgetSummary {
                total_allocated_tokens: row.get::<_, i64>(0)? as u64,
                total_spent_tokens:     row.get::<_, i64>(1)? as u64,
                total_allocated_usd:    row.get(2)?,
                total_spent_usd:        row.get(3)?,
                tasks_exceeded:         row.get::<_, i64>(4)? as u32,
                tasks_with_budget:      row.get::<_, i64>(5)? as u32,
            }),
        )?;
        Ok(row)
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
             ORDER BY worker_slug, task_type",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // ─── Ruflo Feature 5 — Phase-Based Task Decomposition ─────────────────────

    /// Create an Epic task — a parent record with task_type='epic'.
    /// Epic tasks have no assigned_worker and no phase; subtasks link back via epic_id.
    pub fn create_epic(
        &self,
        title: &str,
        description: Option<&str>,
        created_by: &str,
    ) -> Result<Task> {
        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, task_type, status, acceptance_criteria, created_by)
             VALUES (?1, ?2, 'p1', 'epic', 'ready', 'All subtasks complete', ?3)",
            params![title, description, created_by],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_by_id(id)?
            .ok_or_else(|| AutoDevError::Agent("Epic not found after insert".into()))
    }

    /// Create a task with full phase metadata and optional blocker list.
    pub fn create_phased_task(&self, input: CreatePhasedTaskInput) -> Result<Task> {
        let created_by = input.created_by.unwrap_or_else(|| "supervisor".to_string());
        let blocked_by_json = if input.blocked_by.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&input.blocked_by).unwrap_or_else(|_| "[]".to_string()))
        };
        // Determine initial status: blocked if there are unresolved blockers, else ready
        let initial_status = if input.blocked_by.is_empty() {
            "ready"
        } else {
            "needs_info"
        };
        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, task_type, status,
                               acceptance_criteria, created_by,
                               epic_id, phase, phase_order, milestone, blocked_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                input.title,
                input.description,
                input.priority,
                input.task_type,
                initial_status,
                input.acceptance_criteria,
                created_by,
                input.epic_id,
                input.phase,
                input.phase_order,
                input.milestone,
                blocked_by_json,
            ],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_by_id(id)?
            .ok_or_else(|| AutoDevError::Agent("Phased task not found after insert".into()))
    }

    /// All tasks belonging to an epic, ordered by phase_order then id.
    pub fn get_epic_tasks(&self, epic_id: &str) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE epic_id = ?1
             ORDER BY COALESCE(phase_order, 999), id",
        )?;
        let tasks = stmt
            .query_map(params![epic_id], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    /// Per-phase progress summary for an epic.
    pub fn get_phase_summary(&self, epic_id: &str) -> Result<Vec<PhaseSummary>> {
        let tasks = self.get_epic_tasks(epic_id)?;

        // Group by phase
        let mut phase_map: std::collections::BTreeMap<(i32, String), Vec<&Task>> =
            std::collections::BTreeMap::new();
        for task in &tasks {
            if let (Some(phase), Some(order)) = (&task.phase, task.phase_order) {
                phase_map
                    .entry((order, phase.clone()))
                    .or_default()
                    .push(task);
            }
        }

        let all_phases: Vec<(i32, String)> = phase_map.keys().cloned().collect();
        let mut summaries: Vec<PhaseSummary> = Vec::new();

        for (idx, (order, phase)) in all_phases.iter().enumerate() {
            let phase_tasks = phase_map.get(&(*order, phase.clone())).unwrap();
            let total = phase_tasks.len() as i32;
            let done = phase_tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Done)
                .count() as i32;
            let in_progress = phase_tasks
                .iter()
                .filter(|t| t.status == TaskStatus::InProgress)
                .count() as i32;
            let ready = phase_tasks
                .iter()
                .filter(|t| t.status == TaskStatus::Ready || t.status == TaskStatus::Assigned)
                .count() as i32;
            let blocked = phase_tasks
                .iter()
                .filter(|t| {
                    t.status == TaskStatus::NeedsInfo
                        && t.blocked_by.as_ref().map_or(false, |b| !b.is_empty())
                })
                .count() as i32;
            let is_complete = total > 0 && done == total;

            // A phase is unlocked if it's the first phase or the previous phase is complete
            let is_unlocked = if idx == 0 {
                true
            } else {
                let prev = &all_phases[idx - 1];
                summaries
                    .iter()
                    .find(|s| s.phase == prev.1 && s.phase_order == prev.0)
                    .map_or(false, |s| s.is_complete)
            };

            // Milestone: take from any task in this phase that has one
            let milestone = phase_tasks.iter().find_map(|t| t.milestone.clone());

            summaries.push(PhaseSummary {
                phase: phase.clone(),
                phase_order: *order,
                milestone,
                total_tasks: total,
                done_tasks: done,
                in_progress_tasks: in_progress,
                ready_tasks: ready,
                blocked_tasks: blocked,
                is_complete,
                is_unlocked,
            });
        }

        Ok(summaries)
    }

    /// Tasks with blocked_by entries where at least one blocker is not yet done.
    pub fn get_blocked_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE blocked_by IS NOT NULL AND blocked_by != '[]'",
        )?;
        let tasks = stmt
            .query_map([], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }

    /// Called after a task is marked done. Checks all tasks blocked by `completed_task_id`
    /// — if ALL their blockers are now done, transitions them from needs_info → ready.
    /// Returns the IDs of tasks that were unblocked.
    /// Also auto-completes the parent epic if all its subtasks are done.
    pub fn unblock_ready_tasks(&self, completed_task_id: i64) -> Result<Vec<i64>> {
        let completed_str = completed_task_id.to_string();
        let blocked_tasks = self.get_blocked_tasks()?;

        let mut unblocked_ids: Vec<i64> = Vec::new();

        for task in &blocked_tasks {
            let blockers = match &task.blocked_by {
                Some(b) => b,
                None => continue,
            };
            if !blockers.contains(&completed_str) {
                continue;
            }
            // Check if ALL blockers are now done
            let all_done = blockers.iter().all(|blocker_id| {
                blocker_id
                    .parse::<i64>()
                    .ok()
                    .and_then(|bid| self.get_by_id(bid).ok().flatten())
                    .map_or(false, |t| t.status == TaskStatus::Done)
            });
            if all_done {
                // Transition needs_info → ready
                let _ = self.conn.execute(
                    "UPDATE tasks SET status = 'ready', updated_at = datetime('now') WHERE id = ?1",
                    params![task.id],
                );
                unblocked_ids.push(task.id);
                crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                    task_id: Some(task.id),
                    kind: "unblocked".into(),
                    status: Some("ready".into()),
                    assigned_worker: task.assigned_worker.clone(),
                });
            }
        }

        // Epic auto-completion: if the completed task has an epic_id,
        // check if all subtasks of that epic are now done.
        if let Ok(Some(completed_task)) = self.get_by_id(completed_task_id) {
            if let Some(ref epic_id) = completed_task.epic_id {
                let subtasks = self.get_epic_tasks(epic_id)?;
                if !subtasks.is_empty() && subtasks.iter().all(|t| t.status == TaskStatus::Done) {
                    // All subtasks done — mark the epic task itself as done
                    if let Ok(epic_id_int) = epic_id.parse::<i64>() {
                        let _ = self.conn.execute(
                            "UPDATE tasks SET status = 'done', completed_at = datetime('now'),
                                             updated_at = datetime('now')
                             WHERE id = ?1 AND task_type = 'epic'",
                            params![epic_id_int],
                        );
                        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                            task_id: Some(epic_id_int),
                            kind: "epic_complete".into(),
                            status: Some("done".into()),
                            assigned_worker: None,
                        });
                    }
                }
            }
        }

        Ok(unblocked_ids)
    }

    // ─── Doc 04 Phase 2/4 — QC reviewer claim + cost rollup ──────────────────

    /// Atomically claim a task for QC review by setting `under_review_by` to
    /// the given subagent slug **only if** the slot is currently NULL. Returns
    /// `true` if the claim succeeded, `false` if another reviewer already
    /// owns the task. This is the single anti-double-spawn primitive — every
    /// `spawn_qc_review` call must check the return value before proceeding.
    pub fn try_claim_for_qc_review(&self, task_id: i64, subagent_slug: &str) -> Result<bool> {
        let rows = self.conn.execute(
            "UPDATE tasks SET under_review_by = ?2
             WHERE id = ?1 AND under_review_by IS NULL",
            params![task_id, subagent_slug],
        )?;
        Ok(rows == 1)
    }

    /// Clear the QC claim regardless of current value. Called when a
    /// reviewer reaches a terminal state (Completed / Failed / Terminated)
    /// or when the stranded-reviewer reconciler sweeps a dead subagent.
    pub fn release_qc_claim(&self, task_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET under_review_by = NULL WHERE id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    /// Update the QC claim from the synthetic claim_token to the real
    /// subagent_id once the spawn has returned a handle. Lets the
    /// stranded-reviewer reconciler match `under_review_by` directly against
    /// `SubagentManager::list`'s live ids.
    pub fn upgrade_qc_claim(&self, task_id: i64, real_subagent_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET under_review_by = ?2 WHERE id = ?1",
            params![task_id, real_subagent_id],
        )?;
        Ok(())
    }

    /// List task IDs currently claimed for QC review along with their slug.
    /// Used by the stranded-reviewer reconciler to detect rows whose
    /// reviewer subagent is no longer alive in the SubagentManager registry.
    pub fn list_qc_claims(&self) -> Result<Vec<(i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, under_review_by FROM tasks WHERE under_review_by IS NOT NULL")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Add a USD increment to the running `task_qc_cost_usd` for a task. Called
    /// from the spawn_qc_review completion hook with the reviewer's
    /// `SubagentResult.cost_usd` so the QC ROI panel can show qc_cost /
    /// worker_cost ratios per task.
    pub fn add_qc_cost(&self, task_id: i64, cost_usd: f64) -> Result<()> {
        if cost_usd <= 0.0 {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE tasks
             SET task_qc_cost_usd = COALESCE(task_qc_cost_usd, 0.0) + ?2
             WHERE id = ?1",
            params![task_id, cost_usd],
        )?;
        Ok(())
    }

    /// Returns tasks in a specific phase that are ready to assign (status = ready | assigned).
    pub fn get_ready_tasks_in_phase(&self, epic_id: &str, phase: &str) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks
             WHERE epic_id = ?1 AND phase = ?2
               AND status IN ('ready', 'assigned')
             ORDER BY COALESCE(phase_order, 999), id",
        )?;
        let tasks = stmt
            .query_map(params![epic_id, phase], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(tasks)
    }
}

fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        priority: TaskPriority::from_str(&row.get::<_, String>(3)?),
        task_type: TaskType::from_str(&row.get::<_, String>(4)?),
        status: TaskStatus::from_str(&row.get::<_, String>(5)?),
        acceptance_criteria: row.get(6)?,
        assigned_worker: row.get(7)?,
        rejection_reason: row.get(8)?,
        retry_count: row.get(9)?,
        created_by: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
        completed_at: row.get(13)?,
        claimed_by_pid: row.get(14).unwrap_or(None),
        claim_host: row.get(15).unwrap_or(None),
        diff_analysis: row.get(16).unwrap_or(None),
        // Ruflo Feature 4 — Budget Circuit Breaker (columns 17–24)
        budget_max_tokens: row.get(17).unwrap_or(None),
        budget_max_usd: row.get(18).unwrap_or(None),
        budget_max_turns: row.get(19).unwrap_or(None),
        budget_tokens_used: row.get::<_, i64>(20).unwrap_or(0),
        budget_cost_usd: row.get::<_, f64>(21).unwrap_or(0.0),
        budget_turns_used: row.get::<_, i64>(22).unwrap_or(0),
        budget_exceeded: row.get::<_, i64>(23).unwrap_or(0) != 0,
        budget_exceeded_reason: row.get(24).unwrap_or(None),
        // Ruflo Feature 5 — Phase-Based Task Decomposition (indices 25–29)
        phase: row.get(25).unwrap_or(None),
        phase_order: row
            .get::<_, Option<i64>>(26)
            .unwrap_or(None)
            .map(|v| v as i32),
        milestone: row.get(27).unwrap_or(None),
        epic_id: row.get(28).unwrap_or(None),
        blocked_by: row
            .get::<_, Option<String>>(29)
            .unwrap_or(None)
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok()),
        // Doc 04 Phase 2 / Phase 4 — under_review_by + task_qc_cost_usd
        // (indices 30, 31). Both are NULL-safe by SQL COALESCE / Option.
        under_review_by: row.get::<_, Option<String>>(30).unwrap_or(None),
        task_qc_cost_usd: row.get::<_, f64>(31).unwrap_or(0.0),
    })
}

fn row_to_worker_request(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkerRequest> {
    Ok(WorkerRequest {
        id: row.get(0)?,
        worker_slug: row.get(1)?,
        task_id: row.get(2)?,
        request_type: row.get(3)?,
        payload: row.get(4)?,
        status: WorkerRequestStatus::from_str(&row.get::<_, String>(5)?),
        resolution_note: row.get(6)?,
        created_at: row.get(7)?,
        resolved_at: row.get(8)?,
    })
}

fn row_to_worker_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkerMessage> {
    Ok(WorkerMessage {
        id: row.get(0)?,
        from_slug: row.get(1)?,
        to_slug: row.get(2)?,
        body: row.get(3)?,
        kind: row.get(4)?,
        task_id: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        read_at: row.get(8)?,
    })
}
