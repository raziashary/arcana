// Task Board database — SQLite CRUD for the Internal Arcana Task Board.
// Database path: ~/.arcana/tasks.db
// Used by: PA (create tasks in Delegate Mode), Supervisor (assign/update), Workers (claim/complete)
// See: DEVELOPMENT_PLAN.md Phase 2.1 — Arcana Panel & Internal Board

use super::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
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

/// Arcana Communication Protocol (ACP) — structured worker↔worker message.
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
    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "ready" => Ok(Self::Ready),
            "assigned" => Ok(Self::Assigned),
            "in_progress" => Ok(Self::InProgress),
            "qc_review" => Ok(Self::QcReview),
            "done" => Ok(Self::Done),
            "build_failed" => Ok(Self::BuildFailed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "needs_info" => Ok(Self::NeedsInfo),
            _ => Err(format!("unknown task status '{s}'")),
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

#[derive(Debug, Clone)]
pub struct BuildEvidenceRow {
    pub task_id: i64,
    pub session_id: String,
    pub command: String,
    pub exit_code: i32,
    pub exit_code_attributable: bool,
    pub kind: String,
    pub created_at: String,
    pub workspace_path: Option<String>,
    pub workspace_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectBuildBatch {
    pub id: i64,
    pub task_id: i64,
    pub expected_count: usize,
    pub supervisor_retry_approved: bool,
}

// ─── Repository ───────────────────────────────────────────────────────────────

const CLAIMED_TASK_IDS_FOR_WORKER_SQL: &str = "SELECT id FROM tasks
             WHERE assigned_worker = ?1 AND status = 'in_progress'
               AND claimed_by_pid IS NOT NULL
             ORDER BY id LIMIT 2";
const TASK_RETRY_COUNT_BY_ID_SQL: &str = "SELECT retry_count FROM tasks WHERE id = ?1";
const TASK_STATUS_AND_OWNER_BY_ID_SQL: &str =
    "SELECT status, assigned_worker FROM tasks WHERE id = ?1";
const ROUTING_LATEST_PENDING_FOR_WORKER_SQL: &str = "SELECT id, assigned_at FROM routing_history
             WHERE task_id = ?1 AND worker_slug = ?2 AND outcome IS NULL
             ORDER BY rowid DESC LIMIT 1";
const ROUTING_OUTCOME_FOR_WORKER_CLAIM_SQL: &str = "UPDATE routing_history
                 SET outcome = ?1, outcome_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = (
                     SELECT id FROM routing_history
                     WHERE task_id = ?2 AND worker_slug = ?3 AND outcome IS NULL
                     ORDER BY rowid DESC LIMIT 1
                 )";
const ROUTING_OUTCOME_FOR_TASK_SQL: &str = "UPDATE routing_history
             SET outcome = ?1, outcome_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 duration_ms = CASE WHEN ?2 > 0 THEN ?2 ELSE duration_ms END,
                 tokens_used = CASE WHEN ?3 > 0 THEN ?3 ELSE tokens_used END
             WHERE id = (
                 SELECT id FROM routing_history
                 WHERE task_id = ?4 AND outcome IS NULL
                 ORDER BY rowid DESC LIMIT 1
             )";
const TASK_BY_ID_SQL: &str = "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE id = ?1";
const TASKS_BY_STATUS_SQL: &str = "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at,
                         claimed_by_pid, claim_host, diff_analysis,
                         budget_max_tokens, budget_max_usd, budget_max_turns,
                         COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                         COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                         budget_exceeded_reason,
                         phase, phase_order, milestone, epic_id, blocked_by,
                         under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE status = ?1 ORDER BY julianday(created_at) ASC, id ASC";
const TASKS_ALL_BY_CREATED_SQL: &str = "SELECT id, title, description, priority, task_type, status,
                         acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                         created_by, created_at, updated_at, completed_at,
                         claimed_by_pid, claim_host, diff_analysis,
                         budget_max_tokens, budget_max_usd, budget_max_turns,
                         COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                         COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                         budget_exceeded_reason,
                         phase, phase_order, milestone, epic_id, blocked_by,
                         under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks ORDER BY julianday(created_at) ASC, id ASC";
const TASKS_BY_STATUS_AND_WORKER_SQL: &str =
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
             FROM tasks WHERE status = ?1 AND assigned_worker = ?2 ORDER BY id";
const EPIC_TASKS_SQL: &str = "SELECT id, title, description, priority, task_type, status,
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
             ORDER BY COALESCE(phase_order, 999), id";
const READY_TASKS_IN_PHASE_SQL: &str = "SELECT id, title, description, priority, task_type, status,
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
             ORDER BY COALESCE(phase_order, 999), id";
const WORKER_REQUESTS_BY_STATUS_SQL: &str =
    "SELECT id, worker_slug, task_id, request_type, payload, status,
                    resolution_note, created_at, resolved_at
             FROM worker_requests WHERE status = ?1 ORDER BY julianday(created_at) DESC, id DESC";
const WORKER_REQUESTS_RECENT_SQL: &str =
    "SELECT id, worker_slug, task_id, request_type, payload, status,
                    resolution_note, created_at, resolved_at
             FROM worker_requests ORDER BY julianday(created_at) DESC, id DESC";
const CLAIM_NEXT_READY_SQL: &str = "UPDATE tasks
                    SET status = 'in_progress', assigned_worker = ?1,
                        claimed_by_pid = ?2, claim_host = ?3,
                        updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                  WHERE id = (SELECT id FROM tasks
                               WHERE status = 'ready' AND task_type != 'epic'
                               ORDER BY julianday(created_at) ASC, id ASC
                               LIMIT 1)
                    AND (SELECT COUNT(*) FROM tasks
                          WHERE assigned_worker = ?1
                            AND status = 'in_progress') = 0
                RETURNING id";
const CLAIM_TASK_WITH_PID_SQL: &str = "UPDATE tasks
             SET status = 'in_progress', assigned_worker = ?1,
                 claimed_by_pid = ?2, claim_host = ?3,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?4 AND status = 'assigned'
               AND assigned_worker = ?1 AND task_type != 'epic'
               AND NOT EXISTS (
                   SELECT 1 FROM tasks active
                   WHERE active.assigned_worker = ?1
                     AND active.status = 'in_progress'
                     AND active.id != ?4
               )";
const ASSIGN_WITH_ROUTING_DECISION_SQL: &str = "UPDATE tasks SET status = 'assigned', assigned_worker = ?2,
                              claimed_by_pid = NULL, claim_host = NULL,
                              rejection_reason = NULL, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                              completed_at = NULL
             WHERE id = ?1 AND task_type != 'epic'
               AND status IN ('ready', 'assigned', 'failed', 'rejected')
               AND NOT EXISTS (
                   SELECT 1 FROM tasks
                    WHERE assigned_worker = ?2 AND status = 'in_progress' AND id != ?1
               )";
const HOLD_INTERRUPTED_CLAIMS_SELECT_SQL: &str = "SELECT id FROM tasks
             WHERE status = 'in_progress' AND assigned_worker = ?1
               AND claimed_by_pid = ?2 AND claim_host = ?3
             ORDER BY id";
const ROUTING_HISTORY_RECENT_SQL: &str =
    "SELECT id, task_id, task_title, task_type, worker_slug, assigned_at,
                    outcome, outcome_at, duration_ms, tokens_used
             FROM routing_history
             WHERE assignment_method = 'automatic'
             ORDER BY julianday(assigned_at) DESC, rowid DESC LIMIT ?1";
const ROUTING_AFFINITY_STATS_SQL: &str = "SELECT worker_slug, task_type,
                    COUNT(*) AS total,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) AS successes,
                    SUM(CASE WHEN outcome = 'failure' THEN 1 ELSE 0 END) AS failures,
                    CAST(AVG(CASE WHEN duration_ms > 0 THEN duration_ms END) AS INTEGER) AS avg_dur
             FROM routing_history
             WHERE assignment_method = 'automatic'
             GROUP BY worker_slug, task_type
             ORDER BY worker_slug, task_type";
const WORKER_MESSAGES_FOR_RECIPIENT_SQL: &str =
    "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages WHERE to_slug = ?1
             ORDER BY julianday(created_at) ASC, id ASC";
const WORKER_MESSAGES_UNACKED_FOR_RECIPIENT_SQL: &str =
    "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE to_slug = ?1 AND status IN ('pending', 'read')
             ORDER BY julianday(created_at) ASC, id ASC";
const WORKER_MESSAGES_FOR_TASK_SQL: &str =
    "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages WHERE task_id = ?1
             ORDER BY julianday(created_at) ASC, id ASC LIMIT ?2";
const WORKER_MESSAGES_RECENT_SQL: &str =
    "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             ORDER BY julianday(created_at) DESC, id DESC LIMIT ?1";
const WORKER_MESSAGES_STALE_SQL: &str =
    "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages
             WHERE status IN ('pending','read')
               AND julianday(created_at) <= julianday('now', ?1)
             ORDER BY julianday(created_at) ASC, id ASC";
const UNREAD_MESSAGE_COUNT_SQL: &str = "SELECT COUNT(*) FROM worker_messages
             WHERE to_slug = ?1 AND status = 'pending'";
const UNACKNOWLEDGED_MESSAGE_COUNT_SQL: &str = "SELECT COUNT(*) FROM worker_messages
             WHERE to_slug = ?1 AND status IN ('pending', 'read')";
const ORPHAN_CANDIDATES_SQL: &str = "SELECT id, title, description, priority, task_type, status,
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
             ORDER BY julianday(updated_at) ASC, id ASC";
const LATEST_BUILD_BATCH_FOR_QC_SQL: &str = "SELECT outcome, expected_count, observed_count,
                    passing_count, workspace_path, workspace_fingerprint,
                    (SELECT COUNT(*) FROM build_evidence e WHERE e.batch_id = b.id),
                    (SELECT COUNT(*) FROM build_evidence e WHERE e.batch_id = b.id
                       AND (e.exit_code != 0 OR e.exit_code_attributable != 1
                            OR e.workspace_path IS NOT b.workspace_path
                            OR e.workspace_fingerprint IS NOT b.workspace_fingerprint)),
                    arcana_qc_selected_workspace_matches(?1, b.workspace_path)
             FROM project_build_batches b WHERE task_id = ?1 ORDER BY id DESC LIMIT 1";

#[cfg(test)]
pub(crate) fn synthetic_build_evidence_workspace() -> &'static (std::path::PathBuf, String) {
    static WORKSPACE: std::sync::LazyLock<(std::path::PathBuf, String)> =
        std::sync::LazyLock::new(|| {
            let path = std::env::temp_dir().join(format!(
                "arcana-test-build-evidence-workspace-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&path).expect("create test build workspace");
            let path = path.canonicalize().expect("canonical test build workspace");
            let fingerprint = crate::platform::workspace_fingerprint::fingerprint(&path)
                .expect("fingerprint test build workspace");
            (path, fingerprint)
        });
    &WORKSPACE
}
const LATEST_BUILD_EVIDENCE_FOR_TASK_SQL: &str = "SELECT task_id, session_id, command, exit_code,
                    exit_code_attributable, kind, created_at, workspace_path,
                    workspace_fingerprint
             FROM build_evidence WHERE task_id = ?1 AND kind != 'smoke'
             ORDER BY id DESC LIMIT 1";
const TASKS_DAILY_BUDGET_SUMMARY_SQL: &str = "SELECT
               COALESCE(SUM(CASE WHEN budget_max_tokens IS NOT NULL THEN budget_max_tokens ELSE 0 END), 0),
               COALESCE(SUM(budget_tokens_used), 0),
               COALESCE(SUM(CASE WHEN budget_max_usd IS NOT NULL THEN budget_max_usd ELSE 0.0 END), 0.0),
               COALESCE(SUM(budget_cost_usd), 0.0),
               COALESCE(SUM(CASE WHEN budget_exceeded = 1 THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN budget_max_tokens IS NOT NULL OR budget_max_usd IS NOT NULL THEN 1 ELSE 0 END), 0)
             FROM tasks
             WHERE date(created_at) = date('now')";
const QC_CLAIMS_SQL: &str =
    "SELECT id, under_review_by FROM tasks WHERE under_review_by IS NOT NULL";
const TASK_BUDGET_STATUS_SQL: &str =
    "SELECT budget_tokens_used, budget_cost_usd, budget_turns_used,
                    budget_exceeded, budget_exceeded_reason
             FROM tasks WHERE id = ?1";
const BLOCKED_TASKS_FOR_UNBLOCK_SQL: &str = "SELECT id, blocked_by FROM tasks
             WHERE status = 'needs_info' AND blocked_by IS NOT NULL AND blocked_by != '[]'
               AND budget_exceeded = 0";
const TASK_STATUS_BY_ID_SQL: &str = "SELECT status FROM tasks WHERE id = ?1";
const TASK_EPIC_ID_BY_ID_SQL: &str = "SELECT epic_id FROM tasks WHERE id = ?1";
const EPIC_COMPLETION_COUNTS_SQL: &str =
    "SELECT COUNT(*), COALESCE(SUM(CASE WHEN status != 'done' THEN 1 ELSE 0 END), 0)
                 FROM tasks WHERE epic_id = ?1";
const GET_BLOCKED_TASKS_SQL: &str = "SELECT id, title, description, priority, task_type, status,
                    acceptance_criteria, assigned_worker, rejection_reason, retry_count,
                    created_by, created_at, updated_at, completed_at,
                    claimed_by_pid, claim_host, diff_analysis,
                    budget_max_tokens, budget_max_usd, budget_max_turns,
                    COALESCE(budget_tokens_used,0), COALESCE(budget_cost_usd,0.0),
                    COALESCE(budget_turns_used,0), COALESCE(budget_exceeded,0),
                    budget_exceeded_reason,
                    phase, phase_order, milestone, epic_id, blocked_by,
                    under_review_by, COALESCE(task_qc_cost_usd, 0.0)
             FROM tasks WHERE blocked_by IS NOT NULL AND blocked_by != '[]'";

pub struct TasksDb {
    /// Crate-visible so sibling modules can build realistic fixtures against a
    /// temporary database — `orphan`'s TTL tests need isolated rows with both
    /// canonical and legacy timestamp formats.
    pub(crate) conn: Connection,
    qc_workspace_context: std::sync::Arc<std::sync::Mutex<Option<(i64, String)>>>,
    supervisor_approval_context: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

struct SupervisorApprovalContextGuard {
    context: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl SupervisorApprovalContextGuard {
    fn enable(context: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        context.store(true, std::sync::atomic::Ordering::SeqCst);
        Self { context }
    }
}

impl Drop for SupervisorApprovalContextGuard {
    fn drop(&mut self) {
        self.context
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

struct QcWorkspaceContextGuard {
    context: std::sync::Arc<std::sync::Mutex<Option<(i64, String)>>>,
}

impl Drop for QcWorkspaceContextGuard {
    fn drop(&mut self) {
        self.context
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
    }
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
        Self::open_at(&arcana_dir()?.join("tasks.db"))
    }

    /// As `open`, against an explicit path, so tests never write to the user's
    /// real `~/.arcana/tasks.db`.
    pub fn open_at(path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(path)?;
        let db = Self::from_connection(conn)?;
        db.migrate()?;
        Ok(db)
    }

    fn from_connection(conn: Connection) -> Result<Self> {
        let qc_workspace_context = std::sync::Arc::new(std::sync::Mutex::new(None));
        let supervisor_approval_context =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let selected_workspace = std::sync::Arc::clone(&qc_workspace_context);
        let approval_context = std::sync::Arc::clone(&supervisor_approval_context);
        conn.create_scalar_function(
            "arcana_workspace_fingerprint_matches",
            2,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            |context| {
                let path = context.get::<String>(0)?;
                let expected = context.get::<String>(1)?;
                let matches = crate::platform::workspace_fingerprint::fingerprint(
                    std::path::Path::new(&path),
                )
                .is_ok_and(|actual| actual == expected);
                Ok(i64::from(matches))
            },
        )?;
        conn.create_scalar_function(
            "arcana_qc_selected_workspace_matches",
            2,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |context| {
                let task_id = context.get::<i64>(0)?;
                let batch_path = context.get::<String>(1)?;
                let selected = selected_workspace
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                Ok(i64::from(selected.as_ref().is_some_and(
                    |(selected_task, path)| {
                        *selected_task == task_id
                            && crate::platform::workspace::workspace_identity(std::path::Path::new(
                                &batch_path,
                            ))
                            .is_ok_and(|batch_identity| path == &batch_identity)
                    },
                )))
            },
        )?;
        conn.create_scalar_function(
            "arcana_supervisor_approval_authorized",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                Ok(i64::from(
                    approval_context.load(std::sync::atomic::Ordering::SeqCst),
                ))
            },
        )?;
        Ok(Self {
            conn,
            qc_workspace_context,
            supervisor_approval_context,
        })
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
    const LADDER: &[Migration] = &[
        Migration {
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
                created_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                updated_at          TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                completed_at        TEXT,
                -- Columns added after the original CREATE TABLE. They belong
                -- here, not only in ADDED_COLUMNS: `ensure_columns` runs before
                -- the ladder and returns early when the table does not exist
                -- yet, so a FRESH database is created entirely by this rung.
                -- Omitting them left new installs without `claimed_by_pid` and
                -- seventeen others, while every existing database looked fine.
                claimed_by_pid         INTEGER,
                claim_host             TEXT,
                diff_analysis          TEXT,
                budget_max_tokens      INTEGER,
                budget_max_usd         REAL,
                budget_max_turns       INTEGER DEFAULT 20,
                budget_tokens_used     INTEGER DEFAULT 0,
                budget_cost_usd        REAL    DEFAULT 0.0,
                budget_turns_used      INTEGER DEFAULT 0,
                budget_exceeded        INTEGER DEFAULT 0,
                budget_exceeded_reason TEXT,
                phase                  TEXT,
                phase_order            INTEGER,
                milestone              TEXT,
                epic_id                TEXT,
                blocked_by             TEXT,
                under_review_by        TEXT,
                task_qc_cost_usd       REAL    DEFAULT 0.0
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status     ON tasks(status);
            CREATE INDEX IF NOT EXISTS idx_tasks_assigned   ON tasks(assigned_worker);
            CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);

            CREATE TRIGGER IF NOT EXISTS tasks_updated_at
            AFTER UPDATE ON tasks
            BEGIN
                UPDATE tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
            END;

            CREATE TABLE IF NOT EXISTS worker_requests (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                worker_slug     TEXT    NOT NULL,
                task_id         INTEGER,
                request_type    TEXT    NOT NULL,
                payload         TEXT    NOT NULL DEFAULT '{}',
                status          TEXT    NOT NULL DEFAULT 'pending',
                resolution_note TEXT,
                created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                resolved_at     TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id)
            );
            CREATE INDEX IF NOT EXISTS idx_worker_requests_status     ON worker_requests(status);
            CREATE INDEX IF NOT EXISTS idx_worker_requests_worker    ON worker_requests(worker_slug);
            CREATE INDEX IF NOT EXISTS idx_worker_requests_task      ON worker_requests(task_id);

            -- Arcana Communication Protocol — structured worker↔worker inbox.
            -- One row per (sender, target) pair; a broadcast inserts N rows.
            CREATE TABLE IF NOT EXISTS worker_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                from_slug   TEXT    NOT NULL,
                to_slug     TEXT    NOT NULL,
                body        TEXT    NOT NULL,
                kind        TEXT    NOT NULL DEFAULT 'nudge',
                task_id     INTEGER,
                status      TEXT    NOT NULL DEFAULT 'pending',
                created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
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
        },
        Migration {
            version: 2,
            name: "task-status-lifecycle",
            sql: r#"
            -- All writers, including direct claim/recovery/budget/epic SQL,
            -- pass through these guards. Existing rows remain readable; an
            -- unknown legacy value is rejected by row_to_task and on update.
            CREATE TRIGGER tasks_status_insert_guard
            BEFORE INSERT ON tasks
            WHEN NEW.status NOT IN ('ready','assigned','in_progress','qc_review',
                                    'done','build_failed','failed','cancelled',
                                    'rejected','needs_info')
              OR (NEW.status = 'ready' AND NEW.assigned_worker IS NOT NULL)
              OR (NEW.status IN ('assigned','in_progress') AND NEW.assigned_worker IS NULL)
            BEGIN
                SELECT RAISE(ABORT, 'unknown task status');
            END;

            CREATE TRIGGER tasks_status_update_guard
            BEFORE UPDATE ON tasks
            WHEN OLD.status NOT IN ('ready','assigned','in_progress','qc_review',
                                    'done','build_failed','failed','cancelled',
                                    'rejected','needs_info')
              OR NEW.status NOT IN ('ready','assigned','in_progress','qc_review',
                                    'done','build_failed','failed','cancelled',
                                    'rejected','needs_info')
              OR (NEW.status = 'ready' AND NEW.assigned_worker IS NOT NULL)
              OR (NEW.status IN ('assigned','in_progress') AND NEW.assigned_worker IS NULL)
              OR (OLD.task_type = 'epic' AND NEW.status = 'done'
                  AND OLD.status != 'done' AND (
                      NOT EXISTS (SELECT 1 FROM tasks child
                                  WHERE child.epic_id = CAST(OLD.id AS TEXT))
                      OR EXISTS (SELECT 1 FROM tasks child
                                 WHERE child.epic_id = CAST(OLD.id AS TEXT)
                                   AND child.status != 'done')))
              OR (NEW.status != OLD.status AND NOT (
                  (OLD.status = 'ready' AND NEW.status IN
                      ('assigned','in_progress','needs_info','cancelled'))
                  OR (OLD.status = 'assigned' AND NEW.status IN
                      ('ready','in_progress','failed','needs_info','cancelled'))
                  OR (OLD.status = 'in_progress' AND NEW.status IN
                      ('ready','qc_review','done','build_failed','failed',
                       'rejected','needs_info','cancelled'))
                  OR (OLD.status = 'qc_review' AND NEW.status IN
                      ('done','rejected','build_failed','failed','cancelled'))
                  OR (OLD.status = 'build_failed' AND NEW.status IN
                      ('ready','failed','cancelled'))
                  OR (OLD.status = 'failed' AND NEW.status IN
                      ('ready','assigned','cancelled'))
                  OR (OLD.status = 'rejected' AND NEW.status IN
                      ('ready','assigned','cancelled'))
                  OR (OLD.status = 'needs_info' AND NEW.status IN
                      ('ready','failed','cancelled'))
                  OR (OLD.task_type = 'epic' AND OLD.status = 'ready'
                      AND NEW.status IN ('done','rejected')
                      AND EXISTS (SELECT 1 FROM tasks child
                                  WHERE child.epic_id = CAST(OLD.id AS TEXT))
                      AND NOT EXISTS (SELECT 1 FROM tasks child
                                      WHERE child.epic_id = CAST(OLD.id AS TEXT)
                                        AND child.status != 'done'))
              ))
            BEGIN
                SELECT RAISE(ABORT, 'illegal task status transition');
            END;
        "#,
        },
        Migration {
            version: 3,
            name: "epics-are-not-assignable",
            sql: r#"
            CREATE TRIGGER tasks_epic_assignment_insert_guard
            BEFORE INSERT ON tasks
            WHEN NEW.task_type = 'epic'
                 AND NEW.status IN ('assigned', 'in_progress')
            BEGIN
                SELECT RAISE(ABORT, 'epic tasks cannot be assigned or claimed');
            END;

            CREATE TRIGGER tasks_epic_assignment_update_guard
            BEFORE UPDATE ON tasks
            WHEN NEW.task_type = 'epic'
                 AND NEW.status IN ('assigned', 'in_progress')
            BEGIN
                SELECT RAISE(ABORT, 'epic tasks cannot be assigned or claimed');
            END;
        "#,
        },
        Migration {
            version: 4,
            name: "routing-history",
            sql: r#"
            CREATE TABLE IF NOT EXISTS routing_history (
                id          TEXT PRIMARY KEY,
                task_id     TEXT NOT NULL,
                task_title  TEXT NOT NULL,
                task_type   TEXT NOT NULL,
                worker_slug TEXT NOT NULL,
                assigned_at TEXT NOT NULL,
                outcome     TEXT,
                outcome_at  TEXT,
                duration_ms INTEGER,
                tokens_used INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_routing_history_task
                ON routing_history(task_id);
            CREATE INDEX IF NOT EXISTS idx_routing_history_worker_type
                ON routing_history(worker_slug, task_type);
            CREATE INDEX IF NOT EXISTS idx_routing_history_assigned_at
                ON routing_history(assigned_at);
        "#,
        },
        Migration {
            version: 5,
            name: "task-build-evidence",
            sql: r#"
            CREATE TABLE IF NOT EXISTS build_evidence (
                id                      INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id                 INTEGER NOT NULL,
                session_id              TEXT    NOT NULL,
                command                 TEXT    NOT NULL,
                exit_code               INTEGER NOT NULL,
                exit_code_attributable  INTEGER NOT NULL CHECK (exit_code_attributable IN (0,1)),
                kind                    TEXT    NOT NULL CHECK (kind IN ('node','cargo','python','go','make','smoke')),
                created_at              TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_build_evidence_task_id
                ON build_evidence(task_id, id DESC);
        "#,
        },
        Migration {
            version: 6,
            name: "task-claim-metadata-invariants",
            sql: r#"
            CREATE TRIGGER tasks_claim_insert_guard
            BEFORE INSERT ON tasks
            WHEN NEW.status = 'in_progress'
              OR NEW.claimed_by_pid IS NOT NULL
              OR NEW.claim_host IS NOT NULL
            BEGIN
                SELECT RAISE(ABORT, 'tasks must be atomically claimed from assigned');
            END;

            CREATE TRIGGER tasks_claim_update_guard
            BEFORE UPDATE ON tasks
            WHEN (NEW.status = 'in_progress' AND (
                    NEW.assigned_worker IS NULL
                    OR NEW.claimed_by_pid IS NULL
                    OR NEW.claim_host IS NULL
                    OR (OLD.status != 'in_progress' AND (
                        OLD.status NOT IN ('assigned', 'ready')
                        OR (OLD.status = 'assigned'
                            AND OLD.assigned_worker IS NOT NEW.assigned_worker)
                        OR (OLD.status = 'ready' AND OLD.assigned_worker IS NOT NULL)
                        OR OLD.claimed_by_pid IS NOT NULL
                        OR OLD.claim_host IS NOT NULL))
                    OR (OLD.status = 'in_progress' AND (
                        OLD.assigned_worker IS NOT NEW.assigned_worker
                        OR OLD.claimed_by_pid IS NOT NEW.claimed_by_pid
                        OR OLD.claim_host IS NOT NEW.claim_host))))
              OR (NEW.status != 'in_progress'
                  AND (NEW.claimed_by_pid IS NOT NULL OR NEW.claim_host IS NOT NULL))
            BEGIN
                SELECT RAISE(ABORT, 'task claim metadata must match its active owner');
            END;
        "#,
        },
        Migration {
            version: 7,
            name: "qc-review-requires-build-evidence",
            sql: r#"
            CREATE TRIGGER tasks_qc_review_evidence_guard
            BEFORE UPDATE OF status ON tasks
            WHEN NEW.status = 'qc_review' AND OLD.status != 'qc_review'
              AND COALESCE((
                  SELECT exit_code = 0 AND exit_code_attributable = 1
                  FROM build_evidence
                  WHERE task_id = NEW.id AND kind != 'smoke'
                  ORDER BY id DESC LIMIT 1
              ), 0) != 1
            BEGIN
                SELECT RAISE(ABORT, 'qc_review requires latest attributable passing build evidence');
            END;
        "#,
        },
        Migration {
            version: 8,
            name: "routing-assignment-provenance",
            sql: r#"
            ALTER TABLE routing_history
                ADD COLUMN assignment_method TEXT NOT NULL DEFAULT 'legacy'
                CHECK (assignment_method IN ('automatic', 'override', 'legacy'));
            ALTER TABLE routing_history ADD COLUMN override_reason TEXT;
            CREATE INDEX IF NOT EXISTS idx_routing_history_learning
                ON routing_history(assignment_method, task_type, outcome);
        "#,
        },
        Migration {
            version: 9,
            name: "canonical-utc-timestamps",
            sql: r#"
            DROP TRIGGER IF EXISTS tasks_updated_at;
            CREATE TRIGGER tasks_updated_at
            AFTER UPDATE ON tasks
            BEGIN
                UPDATE tasks SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS tasks_timestamps_canonical_insert
            AFTER INSERT ON tasks
            BEGIN
                UPDATE tasks SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                    updated_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.updated_at), NEW.updated_at),
                    completed_at = CASE WHEN NEW.completed_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.completed_at), NEW.completed_at) END
                 WHERE id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS worker_requests_timestamps_canonical_insert
            AFTER INSERT ON worker_requests
            BEGIN
                UPDATE worker_requests SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                    resolved_at = CASE WHEN NEW.resolved_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.resolved_at), NEW.resolved_at) END
                 WHERE id = NEW.id;
            END;
            CREATE TRIGGER IF NOT EXISTS worker_requests_timestamps_canonical_update
            AFTER UPDATE OF created_at, resolved_at ON worker_requests
            BEGIN
                UPDATE worker_requests SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                    resolved_at = CASE WHEN NEW.resolved_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.resolved_at), NEW.resolved_at) END
                 WHERE id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS worker_messages_timestamps_canonical_insert
            AFTER INSERT ON worker_messages
            BEGIN
                UPDATE worker_messages SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                    read_at = CASE WHEN NEW.read_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.read_at), NEW.read_at) END
                 WHERE id = NEW.id;
            END;
            CREATE TRIGGER IF NOT EXISTS worker_messages_timestamps_canonical_update
            AFTER UPDATE OF created_at, read_at ON worker_messages
            BEGIN
                UPDATE worker_messages SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                    read_at = CASE WHEN NEW.read_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.read_at), NEW.read_at) END
                 WHERE id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS routing_history_timestamps_canonical_insert
            AFTER INSERT ON routing_history
            BEGIN
                UPDATE routing_history SET
                    assigned_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.assigned_at), NEW.assigned_at),
                    outcome_at = CASE WHEN NEW.outcome_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.outcome_at), NEW.outcome_at) END
                 WHERE id = NEW.id;
            END;
            CREATE TRIGGER IF NOT EXISTS routing_history_timestamps_canonical_update
            AFTER UPDATE OF assigned_at, outcome_at ON routing_history
            BEGIN
                UPDATE routing_history SET
                    assigned_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.assigned_at), NEW.assigned_at),
                    outcome_at = CASE WHEN NEW.outcome_at IS NULL THEN NULL
                        ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.outcome_at), NEW.outcome_at) END
                 WHERE id = NEW.id;
            END;

            CREATE TRIGGER IF NOT EXISTS build_evidence_timestamps_canonical_insert
            AFTER INSERT ON build_evidence
            BEGIN
                UPDATE build_evidence SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at)
                 WHERE id = NEW.id;
            END;
            CREATE TRIGGER IF NOT EXISTS build_evidence_timestamps_canonical_update
            AFTER UPDATE OF created_at ON build_evidence
            BEGIN
                UPDATE build_evidence SET
                    created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at)
                 WHERE id = NEW.id;
            END;
        "#,
        },
        Migration {
            version: 10,
            name: "worker-request-status-time-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_worker_requests_status_created_at
                ON worker_requests(status, julianday(created_at) DESC, id DESC);
        "#,
        },
        Migration {
            version: 11,
            name: "ready-task-dispatch-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_ready_dispatch
                ON tasks(julianday(created_at), id)
                WHERE status = 'ready' AND task_type != 'epic';
        "#,
        },
        Migration {
            version: 12,
            name: "routing-history-read-indexes",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_routing_history_recent_auto
                ON routing_history(assignment_method, julianday(assigned_at) DESC);
        "#,
        },
        Migration {
            version: 13,
            name: "worker-message-recent-and-stale-indexes",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_worker_messages_recent
                ON worker_messages(julianday(created_at) DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_worker_messages_stale_time
                ON worker_messages(julianday(created_at), id)
                WHERE status IN ('pending', 'read');
        "#,
        },
        Migration {
            version: 14,
            name: "epic-task-progress-indexes",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_epic_phase_order
                ON tasks(epic_id, COALESCE(phase_order, 999), id)
                WHERE epic_id IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_tasks_ready_phase_order
                ON tasks(epic_id, phase, COALESCE(phase_order, 999), id)
                WHERE epic_id IS NOT NULL AND phase IS NOT NULL;
        "#,
        },
        Migration {
            version: 15,
            name: "task-daily-budget-summary-date-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_created_date
                ON tasks(date(created_at));
        "#,
        },
        Migration {
            version: 16,
            name: "task-qc-claim-recovery-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_qc_claims
                ON tasks(under_review_by, id)
                WHERE under_review_by IS NOT NULL;
        "#,
        },
        Migration {
            version: 17,
            name: "blocked-task-reader-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_tasks_blocked_tasks_plan ON tasks(id)
                WHERE blocked_by IS NOT NULL AND blocked_by != '[]';
        "#,
        },
        Migration {
            version: 18,
            name: "workspace-fingerprinted-build-evidence",
            sql: r#"
            ALTER TABLE build_evidence ADD COLUMN workspace_path TEXT;
            ALTER TABLE build_evidence ADD COLUMN workspace_fingerprint TEXT;

            DROP TRIGGER IF EXISTS tasks_qc_review_evidence_guard;
            CREATE TRIGGER tasks_qc_review_evidence_guard
            BEFORE UPDATE OF status ON tasks
            WHEN NEW.status = 'qc_review' AND OLD.status != 'qc_review'
              AND COALESCE((
                  SELECT exit_code = 0
                     AND exit_code_attributable = 1
                     AND workspace_path IS NOT NULL
                     AND workspace_fingerprint IS NOT NULL
                     AND arcana_workspace_fingerprint_matches(
                         workspace_path, workspace_fingerprint
                     ) = 1
                  FROM build_evidence
                  WHERE task_id = NEW.id AND kind != 'smoke'
                  ORDER BY id DESC LIMIT 1
              ), 0) != 1
            BEGIN
                SELECT RAISE(ABORT, 'qc_review requires latest passing evidence for the current workspace fingerprint');
            END;
        "#,
        },
        Migration {
            version: 19,
            name: "complete-project-build-batches",
            sql: r#"
            CREATE TABLE project_build_batches (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                worker_slug TEXT NOT NULL,
                expected_count INTEGER NOT NULL CHECK (expected_count > 0),
                observed_count INTEGER NOT NULL DEFAULT 0 CHECK (observed_count >= 0),
                passing_count INTEGER NOT NULL DEFAULT 0 CHECK (passing_count >= 0),
                outcome TEXT NOT NULL CHECK (outcome IN ('running','passed','failed','incomplete')),
                workspace_path TEXT NOT NULL,
                workspace_fingerprint TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                completed_at TEXT,
                FOREIGN KEY(task_id) REFERENCES tasks(id) ON DELETE CASCADE,
                CHECK (observed_count <= expected_count),
                CHECK (passing_count <= observed_count),
                CHECK ((outcome = 'running' AND completed_at IS NULL)
                    OR (outcome != 'running' AND completed_at IS NOT NULL))
            );
            CREATE INDEX idx_project_build_batches_task_id
                ON project_build_batches(task_id, id DESC);
            ALTER TABLE build_evidence ADD COLUMN batch_id INTEGER
                REFERENCES project_build_batches(id) ON DELETE CASCADE;
            CREATE INDEX idx_build_evidence_batch_id
                ON build_evidence(batch_id, id DESC);

            DROP TRIGGER IF EXISTS tasks_qc_review_evidence_guard;
            CREATE TRIGGER tasks_qc_review_evidence_guard
            BEFORE UPDATE OF status ON tasks
            WHEN NEW.status = 'qc_review' AND OLD.status != 'qc_review'
              AND COALESCE((
                SELECT outcome = 'passed'
                   AND expected_count > 0
                   AND observed_count = expected_count
                   AND passing_count = expected_count
                   AND workspace_fingerprint IS NOT NULL
                   AND arcana_workspace_fingerprint_matches(
                       workspace_path, workspace_fingerprint
                   ) = 1
                   AND arcana_qc_selected_workspace_matches(NEW.id, workspace_path) = 1
                   AND (SELECT COUNT(*) FROM build_evidence e WHERE e.batch_id = b.id)
                       = expected_count
                   AND (SELECT COUNT(*) FROM build_evidence e WHERE e.batch_id = b.id
                       AND (e.exit_code != 0 OR e.exit_code_attributable != 1
                            OR e.workspace_path IS NOT b.workspace_path
                            OR e.workspace_fingerprint IS NOT b.workspace_fingerprint)) = 0
                FROM project_build_batches b WHERE b.task_id = NEW.id
                ORDER BY b.id DESC LIMIT 1
              ), 0) != 1
            BEGIN
                SELECT RAISE(ABORT, 'qc_review requires a complete passing project build batch for the current workspace');
            END;

            CREATE TRIGGER tasks_qc_review_insert_guard
            BEFORE INSERT ON tasks
            WHEN NEW.status = 'qc_review'
            BEGIN
                SELECT RAISE(ABORT, 'tasks cannot be inserted directly into qc_review');
            END;
        "#,
        },
        Migration {
            version: 20,
            name: "single-use-supervisor-build-retry-approvals",
            sql: r#"
            ALTER TABLE worker_requests ADD COLUMN diagnostic_batch_id INTEGER
                REFERENCES project_build_batches(id);
            CREATE TABLE build_retry_approvals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id INTEGER NOT NULL UNIQUE REFERENCES worker_requests(id) ON DELETE CASCADE,
                task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                worker_slug TEXT NOT NULL,
                diagnostic_batch_id INTEGER NOT NULL REFERENCES project_build_batches(id),
                approved_by TEXT NOT NULL,
                reason TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('available','consumed','revoked')),
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                consumed_at TEXT
            );
            CREATE UNIQUE INDEX idx_build_retry_approval_one_available
                ON build_retry_approvals(task_id) WHERE status = 'available';
            CREATE INDEX idx_build_retry_approval_task_history
                ON build_retry_approvals(task_id, id DESC);
            CREATE TRIGGER build_retry_approval_revoke_on_task_change
            AFTER UPDATE OF assigned_worker, status ON tasks
            WHEN NEW.assigned_worker IS NOT OLD.assigned_worker
              OR NEW.status IS NOT OLD.status
            BEGIN
                UPDATE build_retry_approvals SET status = 'revoked'
                WHERE task_id = NEW.id AND status = 'available';
            END;
            CREATE TRIGGER build_retry_approval_insert_guard
            BEFORE INSERT ON build_retry_approvals
            WHEN arcana_supervisor_approval_authorized() != 1
              OR NEW.approved_by != 'supervisor' OR length(trim(NEW.reason)) = 0
              OR NOT EXISTS (
                SELECT 1 FROM worker_requests r JOIN tasks t ON t.id = r.task_id
                WHERE r.id = NEW.request_id AND r.task_id = NEW.task_id
                  AND r.worker_slug = NEW.worker_slug
                  AND r.diagnostic_batch_id = NEW.diagnostic_batch_id
                  AND r.request_type = 'build_retry_approval' AND r.status = 'pending'
                  AND t.status = 'in_progress' AND t.assigned_worker = NEW.worker_slug
                  AND t.retry_count >= 3
                  AND (SELECT id FROM project_build_batches
                       WHERE task_id = NEW.task_id ORDER BY id DESC LIMIT 1)
                      = NEW.diagnostic_batch_id
                  AND EXISTS (SELECT 1 FROM project_build_batches b
                      WHERE b.id = NEW.diagnostic_batch_id
                        AND b.outcome IN ('failed','incomplete')
                        AND b.completed_at IS NOT NULL)
              )
            BEGIN
                SELECT RAISE(ABORT, 'build retry approval must match an eligible pending diagnosis and Supervisor');
            END;
            CREATE TRIGGER build_retry_approval_update_guard
            BEFORE UPDATE ON build_retry_approvals
            WHEN NEW.request_id != OLD.request_id OR NEW.task_id != OLD.task_id
              OR NEW.worker_slug != OLD.worker_slug
              OR NEW.diagnostic_batch_id != OLD.diagnostic_batch_id
              OR NEW.approved_by != OLD.approved_by
              OR NEW.reason != OLD.reason OR OLD.status != 'available'
              OR NEW.status NOT IN ('consumed','revoked')
              OR (NEW.status = 'consumed' AND NEW.consumed_at IS NULL)
            BEGIN
                SELECT RAISE(ABORT, 'build retry approvals are immutable and single-use');
            END;
            ALTER TABLE project_build_batches ADD COLUMN retry_approval_id INTEGER
                REFERENCES build_retry_approvals(id);
            CREATE UNIQUE INDEX idx_project_build_batches_retry_approval
                ON project_build_batches(retry_approval_id)
                WHERE retry_approval_id IS NOT NULL;
            CREATE TRIGGER project_build_batch_retry_approval_guard
            BEFORE INSERT ON project_build_batches
            WHEN ((SELECT retry_count FROM tasks WHERE id = NEW.task_id) >= 3
                  AND NOT EXISTS (
                    SELECT 1 FROM build_retry_approvals a
                    WHERE a.id = NEW.retry_approval_id AND a.task_id = NEW.task_id
                      AND a.worker_slug = NEW.worker_slug AND a.status = 'consumed'
                      AND a.diagnostic_batch_id = (SELECT id FROM project_build_batches
                          WHERE task_id = NEW.task_id ORDER BY id DESC LIMIT 1)
                  ))
              OR (NEW.retry_approval_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM build_retry_approvals a
                    WHERE a.id = NEW.retry_approval_id AND a.task_id = NEW.task_id
                      AND a.worker_slug = NEW.worker_slug AND a.status = 'consumed'
                  ))
            BEGIN
                SELECT RAISE(ABORT, 'build batch beyond the diagnosis threshold requires one consumed Supervisor approval');
            END;
        "#,
        },
    ];

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
        ("phase", "ALTER TABLE tasks ADD COLUMN phase TEXT;"),
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
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_create")
                .with_database("tasks");
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
        let task = self
            .get_by_id(id)?
            .ok_or_else(|| ArcanaError::Agent("Task not found after insert".into()))?;
        operation = operation.with_task_id(Some(id)).with_result_count(1);
        operation.succeed();
        Ok(task)
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Task>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_read")
                .with_database("tasks")
                .with_task_id(Some(id));
        let mut stmt = self.conn.prepare(TASK_BY_ID_SQL)?;
        let task = stmt.query_row(params![id], row_to_task).optional()?;
        operation = operation.with_result_count(usize::from(task.is_some()));
        if task.is_some() {
            operation.succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(task)
    }

    /// Hold an SQLite write reservation while a worker performs one final
    /// host-owned mutation. This closes the validation-to-side-effect race:
    /// every competing task/status writer waits until the guarded operation
    /// returns and the connection is dropped. The caller must keep this
    /// transaction short and must not await while it is held.
    pub(crate) fn lock_current_worker_claim(&self, id: i64, worker: &str) -> Result<bool> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let validation = (|| {
            let Some(task) = self.get_by_id(id)? else {
                return Ok(false);
            };
            if task.status.as_str() != "in_progress"
                || task.assigned_worker.as_deref() != Some(worker)
            {
                return Ok(false);
            }
            Ok(true)
        })();
        if !validation.as_ref().is_ok_and(|valid| *valid) {
            let _ = self.conn.execute_batch("ROLLBACK");
        }
        validation
    }

    /// Resolve the single task currently claimed by a worker. More than one
    /// row is a broken dispatch invariant; do not attach build proof
    /// ambiguously.
    pub fn in_progress_task_for_worker(&self, worker_slug: &str) -> Result<Option<i64>> {
        let operation = crate::observability::timing::OperationTimer::start(
            "database_worker_claimed_task_read",
        )
        .with_database("tasks")
        .with_worker(worker_slug);
        let mut stmt = self.conn.prepare(CLAIMED_TASK_IDS_FOR_WORKER_SQL)?;
        let ids = stmt
            .query_map(params![worker_slug], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [] => {
                operation.with_result_count(0).set_outcome("empty");
                Ok(None)
            }
            [id] => {
                operation.with_task_id(Some(*id)).with_result_count(1).succeed();
                Ok(Some(*id))
            }
            _ => Err(ArcanaError::Agent(format!(
                "worker '{worker_slug}' has multiple in-progress tasks; build evidence cannot be attributed"
            ))),
        }
    }

    #[cfg(test)]
    pub fn insert_build_evidence(
        &self,
        task_id: i64,
        session_id: &str,
        command: &str,
        exit_code: i32,
        exit_code_attributable: bool,
        kind: &str,
    ) -> Result<()> {
        let workspace = synthetic_build_evidence_workspace();
        let workspace_path = workspace.0.to_string_lossy().into_owned();
        let tx = self.conn.unchecked_transaction()?;
        let passed = exit_code == 0 && exit_code_attributable;
        tx.execute(
            "INSERT INTO project_build_batches
             (task_id, session_id, worker_slug, expected_count, observed_count,
              passing_count, outcome, workspace_path, workspace_fingerprint, completed_at)
             VALUES (?1, ?2, 'test-fixture', 1, 1, ?3, ?4, ?5, ?6,
                     strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                task_id,
                session_id,
                i64::from(passed),
                if passed { "passed" } else { "failed" },
                workspace_path,
                workspace.1
            ],
        )?;
        let batch_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO build_evidence
             (task_id, session_id, command, exit_code, exit_code_attributable, kind,
              workspace_path, workspace_fingerprint, batch_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                task_id,
                session_id,
                command,
                exit_code,
                exit_code_attributable,
                kind,
                workspace_path,
                workspace.1,
                batch_id
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Persist an observed Rust-spawned build result and update the task's
    /// consecutive nonzero-exit streak under the same owner/claim transaction.
    /// Spawn failures and timeouts never call this method, so they cannot
    /// fabricate an exit result or advance the streak.
    pub fn record_build_attempt_for_worker(
        &self,
        worker_slug: &str,
        session_id: &str,
        command: &str,
        exit_code: i32,
        kind: &str,
    ) -> Result<Option<(i64, i64)>> {
        self.record_build_attempts_for_worker(
            worker_slug,
            session_id,
            &[(command.to_owned(), exit_code, kind.to_owned())],
            None,
        )
        .map(|result| result.map(|(task_id, streak, _)| (task_id, streak)))
    }

    /// Record every observed build-process exit for one `project_command`
    /// invocation and update the consecutive nonzero-exit streak atomically.
    /// `aggregate_failure` identifies a real failed child whose evidence must
    /// remain latest when a later child passed but the workspace invocation as
    /// a whole still failed; that row is repeated as aggregate evidence without
    /// counting a second observed process failure.
    pub fn record_build_attempts_for_worker(
        &self,
        worker_slug: &str,
        session_id: &str,
        attempts: &[(String, i32, String)],
        aggregate_failure: Option<usize>,
    ) -> Result<Option<(i64, i64, bool)>> {
        let attempts = attempts
            .iter()
            .map(|(command, exit_code, kind)| {
                (command.clone(), *exit_code, kind.clone(), None, None)
            })
            .collect::<Vec<_>>();
        self.record_build_attempts_with_workspace_for_worker(
            worker_slug,
            session_id,
            &attempts,
            aggregate_failure,
        )
    }

    /// Persist observed build exits with the canonical workspace snapshot they
    /// were executed against. The evidence row remains tied to a real exit;
    /// workspace fingerprinting only adds freshness provenance.
    pub fn record_build_attempts_with_workspace_for_worker(
        &self,
        worker_slug: &str,
        session_id: &str,
        attempts: &[(String, i32, String, Option<String>, Option<String>)],
        aggregate_failure: Option<usize>,
    ) -> Result<Option<(i64, i64, bool)>> {
        let mut operation = OperationTimer::start("database_build_attempt_batch_record")
            .with_database("tasks")
            .with_session_id(session_id)
            .with_worker(worker_slug);
        if attempts.is_empty() {
            operation.with_result_count(0).set_outcome("empty");
            return Ok(None);
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut claim_query = tx.prepare(CLAIMED_TASK_IDS_FOR_WORKER_SQL)?;
        let task_ids = claim_query
            .query_map(params![worker_slug], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(claim_query);
        let task_id = match task_ids.as_slice() {
            [] => {
                tx.commit()?;
                operation.with_result_count(0).set_outcome("not_found");
                return Ok(None);
            }
            [task_id] => *task_id,
            _ => {
                return Err(ArcanaError::Agent(format!(
                    "worker '{worker_slug}' has multiple in-progress tasks; build result cannot be attributed"
                )))
            }
        };

        let mut streak: i64 =
            tx.query_row(TASK_RETRY_COUNT_BY_ID_SQL, params![task_id], |row| {
                row.get(0)
            })?;
        let mut diagnostic_reached = false;
        for (command, exit_code, kind, workspace_path, workspace_fingerprint) in attempts {
            if *exit_code == 0 {
                streak = 0;
            } else {
                streak += 1;
                diagnostic_reached |= streak >= 3;
            }
            tx.execute(
                "INSERT INTO build_evidence
                 (task_id, session_id, command, exit_code, exit_code_attributable, kind,
                  workspace_path, workspace_fingerprint)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
                params![
                    task_id,
                    session_id,
                    command,
                    exit_code,
                    kind,
                    workspace_path,
                    workspace_fingerprint
                ],
            )?;
        }
        if let Some(index) = aggregate_failure {
            let (command, exit_code, kind, workspace_path, workspace_fingerprint) =
                attempts.get(index).ok_or_else(|| {
                    ArcanaError::Agent("aggregate build failure index is out of range".into())
                })?;
            if *exit_code == 0 {
                return Err(ArcanaError::Agent(
                    "aggregate build failure must refer to a nonzero exit".into(),
                ));
            }
            tx.execute(
                "INSERT INTO build_evidence
                 (task_id, session_id, command, exit_code, exit_code_attributable, kind,
                  workspace_path, workspace_fingerprint)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7)",
                params![
                    task_id,
                    session_id,
                    command,
                    exit_code,
                    kind,
                    workspace_path,
                    workspace_fingerprint
                ],
            )?;
        }
        tx.execute(
            "UPDATE tasks
             SET retry_count = ?2
             WHERE id = ?1 AND assigned_worker = ?3 AND status = 'in_progress'
               AND claimed_by_pid IS NOT NULL",
            params![task_id, streak, worker_slug],
        )?;
        tx.commit()?;
        operation = operation
            .with_task_id(Some(task_id))
            .with_result_count(attempts.len() + usize::from(aggregate_failure.is_some()));
        operation.succeed();
        Ok(Some((task_id, streak, diagnostic_reached)))
    }

    /// Persist an in-flight project build batch before any selected child is
    /// spawned. If the host exits or the future is cancelled, this durable
    /// `running` row supersedes every older passing batch for QC admission.
    pub fn begin_project_build_batch_for_worker(
        &self,
        worker_slug: &str,
        session_id: &str,
        expected_count: usize,
        workspace_path: &str,
        workspace_fingerprint: Option<&str>,
    ) -> Result<Option<ProjectBuildBatch>> {
        let operation = OperationTimer::start("database_project_build_batch_begin")
            .with_database("tasks")
            .with_session_id(session_id)
            .with_worker(worker_slug);
        if expected_count == 0 {
            operation.with_result_count(0).set_outcome("empty");
            return Ok(None);
        }
        let tx = self.conn.unchecked_transaction()?;
        let mut claim_query = tx.prepare(CLAIMED_TASK_IDS_FOR_WORKER_SQL)?;
        let task_ids = claim_query
            .query_map(params![worker_slug], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(claim_query);
        let task_id = match task_ids.as_slice() {
            [] => {
                tx.commit()?;
                operation.with_result_count(0).set_outcome("not_found");
                return Ok(None);
            }
            [task_id] => *task_id,
            _ => {
                return Err(ArcanaError::Agent(format!(
                    "worker '{worker_slug}' has multiple in-progress tasks; build batch cannot be attributed"
                )))
            }
        };
        let retry_count: i64 =
            tx.query_row(TASK_RETRY_COUNT_BY_ID_SQL, params![task_id], |row| {
                row.get(0)
            })?;
        let retry_approval_id = if retry_count >= 3 {
            let latest_batch_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM project_build_batches WHERE task_id = ?1
                     ORDER BY id DESC LIMIT 1",
                    params![task_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(latest_batch_id) = latest_batch_id else {
                return Err(ArcanaError::Agent(
                    "build_diagnosis_required: no failed build batch is available for Supervisor review".into(),
                ));
            };
            let approval_id: Option<i64> = tx
                .query_row(
                    "SELECT id FROM build_retry_approvals
                     WHERE task_id = ?1 AND worker_slug = ?2 AND status = 'available'
                       AND diagnostic_batch_id = ?3
                     ORDER BY id DESC LIMIT 1",
                    params![task_id, worker_slug, latest_batch_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(approval_id) = approval_id else {
                return Err(ArcanaError::Agent(
                    "build_diagnosis_required: a Supervisor must approve exactly one additional build batch before another build can run".into(),
                ));
            };
            let consumed = tx.execute(
                "UPDATE build_retry_approvals
                 SET status = 'consumed', consumed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?1 AND task_id = ?2 AND worker_slug = ?3 AND status = 'available'",
                params![approval_id, task_id, worker_slug],
            )?;
            if consumed != 1 {
                return Err(ArcanaError::Agent(
                    "build_diagnosis_required: the Supervisor retry approval was no longer available".into(),
                ));
            }
            Some(approval_id)
        } else {
            None
        };
        tx.execute(
            "INSERT INTO project_build_batches
             (task_id, session_id, worker_slug, expected_count, outcome,
              workspace_path, workspace_fingerprint, retry_approval_id)
             VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)",
            params![
                task_id,
                session_id,
                worker_slug,
                expected_count as i64,
                workspace_path,
                workspace_fingerprint,
                retry_approval_id
            ],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        if retry_approval_id.is_some() {
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "build_retry_approval_consumed",
                task_id,
                worker = %worker_slug,
                approval_id = retry_approval_id.unwrap_or_default(),
                batch_id = id,
                outcome = "consumed",
                "Supervisor retry approval consumed by one project build batch"
            );
        }
        operation
            .with_task_id(Some(task_id))
            .with_result_count(1)
            .succeed();
        Ok(Some(ProjectBuildBatch {
            id,
            task_id,
            expected_count,
            supervisor_retry_approved: retry_approval_id.is_some(),
        }))
    }

    /// Atomically attach all observed exits and finalize one running batch.
    /// Missing, timed-out, cancelled, or unspawned children remain missing;
    /// no synthetic exit code is written. Only observed nonzero exits advance
    /// the task's repeated-build-failure streak.
    pub fn finish_project_build_batch_for_worker(
        &self,
        batch_id: i64,
        worker_slug: &str,
        session_id: &str,
        attempts: &[(String, i32, String, Option<String>, Option<String>)],
        aggregate_failure: Option<usize>,
        all_children_completed: bool,
    ) -> Result<Option<(i64, i64, bool)>> {
        let operation = OperationTimer::start("database_project_build_batch_finish")
            .with_database("tasks")
            .with_session_id(session_id)
            .with_worker(worker_slug);
        let tx = self.conn.unchecked_transaction()?;
        let batch: Option<(i64, i64, String, Option<String>)> = tx
            .query_row(
                "SELECT task_id, expected_count, workspace_path, workspace_fingerprint
                 FROM project_build_batches
                 WHERE id = ?1 AND session_id = ?2 AND worker_slug = ?3 AND outcome = 'running'",
                params![batch_id, session_id, worker_slug],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((task_id, expected_count, batch_workspace, batch_fingerprint)) = batch else {
            tx.commit()?;
            operation.with_result_count(0).set_outcome("not_found");
            return Ok(None);
        };
        if attempts.len() > expected_count as usize {
            return Err(ArcanaError::Agent(
                "observed build results exceed the selected batch size".into(),
            ));
        }
        let mut streak: i64 =
            tx.query_row(TASK_RETRY_COUNT_BY_ID_SQL, params![task_id], |row| {
                row.get(0)
            })?;
        let mut diagnostic_reached = false;
        let mut passing_count = 0usize;
        for (command, exit_code, kind, workspace_path, workspace_fingerprint) in attempts {
            if *exit_code == 0 {
                streak = 0;
                passing_count += 1;
            } else {
                streak += 1;
                diagnostic_reached |= streak >= 3;
            }
            tx.execute(
                "INSERT INTO build_evidence
                 (task_id, session_id, command, exit_code, exit_code_attributable, kind,
                  workspace_path, workspace_fingerprint, batch_id)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, ?8)",
                params![
                    task_id,
                    session_id,
                    command,
                    exit_code,
                    kind,
                    workspace_path,
                    workspace_fingerprint,
                    batch_id
                ],
            )?;
        }
        if let Some(index) = aggregate_failure {
            let (_, exit_code, _, _, _) = attempts.get(index).ok_or_else(|| {
                ArcanaError::Agent("aggregate build failure index is out of range".into())
            })?;
            if *exit_code == 0 {
                return Err(ArcanaError::Agent(
                    "aggregate build failure must refer to a nonzero exit".into(),
                ));
            }
        }
        let fingerprints_match = batch_fingerprint.is_some()
            && attempts.iter().all(|(_, _, _, path, fingerprint)| {
                path.as_deref() == Some(batch_workspace.as_str())
                    && fingerprint.as_deref() == batch_fingerprint.as_deref()
            });
        let outcome = if attempts
            .iter()
            .any(|(_, exit_code, _, _, _)| *exit_code != 0)
        {
            "failed"
        } else if all_children_completed
            && attempts.len() == expected_count as usize
            && passing_count == expected_count as usize
            && fingerprints_match
        {
            "passed"
        } else {
            "incomplete"
        };
        tx.execute(
            "UPDATE project_build_batches
             SET observed_count = ?2, passing_count = ?3, outcome = ?4,
                 completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?1 AND outcome = 'running'",
            params![
                batch_id,
                attempts.len() as i64,
                passing_count as i64,
                outcome
            ],
        )?;
        tx.execute(
            "UPDATE tasks SET retry_count = ?2
             WHERE id = ?1 AND assigned_worker = ?3 AND status = 'in_progress'
               AND claimed_by_pid IS NOT NULL",
            params![task_id, streak, worker_slug],
        )?;
        tx.commit()?;
        operation
            .with_task_id(Some(task_id))
            .with_result_count(attempts.len())
            .succeed();
        Ok(Some((task_id, streak, diagnostic_reached)))
    }

    pub fn latest_build_evidence(&self, task_id: i64) -> Result<Option<BuildEvidenceRow>> {
        let operation = OperationTimer::start("database_build_evidence_read")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        let evidence = self
            .conn
            .query_row(
                LATEST_BUILD_EVIDENCE_FOR_TASK_SQL,
                params![task_id],
                |row| {
                    Ok(BuildEvidenceRow {
                        task_id: row.get(0)?,
                        session_id: row.get(1)?,
                        command: row.get(2)?,
                        exit_code: row.get(3)?,
                        exit_code_attributable: row.get::<_, i64>(4)? != 0,
                        kind: row.get(5)?,
                        created_at: row.get(6)?,
                        workspace_path: row.get(7)?,
                        workspace_fingerprint: row.get(8)?,
                    })
                },
            )
            .optional()?;
        if evidence.is_some() {
            operation.with_result_count(1).succeed();
        } else {
            operation.with_result_count(0).set_outcome("not_found");
        }
        Ok(evidence)
    }

    pub fn latest_project_build_batch_status(
        &self,
        task_id: i64,
    ) -> Result<Option<(String, i64, i64, i64, String, Option<String>, i64, i64, i64)>> {
        self.conn
            .query_row(LATEST_BUILD_BATCH_FOR_QC_SQL, params![task_id], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })
            .optional()
            .map_err(Into::into)
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<Task>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_list")
                .with_database("tasks");
        let (query, values): (&str, Vec<String>) = if let Some(status) = status_filter {
            (TASKS_BY_STATUS_SQL, vec![status.to_string()])
        } else {
            (TASKS_ALL_BY_CREATED_SQL, vec![])
        };

        let mut stmt = self.conn.prepare(query)?;
        let tasks = if values.is_empty() {
            stmt.query_map([], row_to_task)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_task)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        operation = operation.with_result_count(tasks.len());
        operation.succeed();
        Ok(tasks)
    }

    pub fn list_tasks_by_status_and_worker(&self, status: &str, worker: &str) -> Result<Vec<Task>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_task_list")
                .with_database("tasks")
                .with_worker(worker);
        let mut stmt = self.conn.prepare(TASKS_BY_STATUS_AND_WORKER_SQL)?;
        let tasks = stmt
            .query_map(params![status, worker], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation = operation.with_result_count(tasks.len());
        operation.succeed();
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
        self.update_status_with_reason_and_run_id(
            id,
            status,
            assigned_worker,
            rejection_reason,
            None,
        )
    }

    pub fn update_status_with_reason_and_run_id(
        &self,
        id: i64,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
        run_id: Option<&str>,
    ) -> Result<()> {
        let timing = OperationTimer::start("database_task_status_transition")
            .with_database("tasks")
            .with_task_id(Some(id));
        let mut timing = timing;
        if let Some(run_id) = run_id {
            timing = timing.with_run_id(run_id);
        }
        let tx = self.conn.unchecked_transaction()?;
        Self::update_status_with_reason_on_connection(
            &tx,
            id,
            status,
            assigned_worker,
            rejection_reason,
        )?;
        tx.commit()?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    /// Transition task status with the workspace selected by the current
    /// worker/UI operation. QC admission binds evidence to this exact
    /// canonical directory for both the shared API and SQLite trigger.
    pub fn update_status_with_workspace_and_run_id(
        &self,
        id: i64,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
        run_id: Option<&str>,
        workspace_path: &std::path::Path,
    ) -> Result<()> {
        let _context = self.bind_qc_workspace_context(id, workspace_path)?;
        self.update_status_with_reason_and_run_id(
            id,
            status,
            assigned_worker,
            rejection_reason,
            run_id,
        )
    }

    fn bind_qc_workspace_context(
        &self,
        task_id: i64,
        workspace_path: &std::path::Path,
    ) -> Result<QcWorkspaceContextGuard> {
        let canonical = crate::platform::workspace::workspace_identity(workspace_path)?;
        *self
            .qc_workspace_context
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((task_id, canonical));
        Ok(QcWorkspaceContextGuard {
            context: std::sync::Arc::clone(&self.qc_workspace_context),
        })
    }

    /// Atomically update task status and close the exact pending routing
    /// assignment that produced the outcome. The worker slug is the owner
    /// observed before this status transition, never the incoming new owner.
    pub fn update_status_with_routing_outcome(
        &self,
        id: i64,
        expected_status: &str,
        expected_worker: &str,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
        outcome: &str,
    ) -> Result<()> {
        self.update_status_with_routing_outcome_and_run_id(
            id,
            expected_status,
            expected_worker,
            status,
            assigned_worker,
            rejection_reason,
            outcome,
            None,
        )
    }

    pub fn update_status_with_routing_outcome_and_run_id(
        &self,
        id: i64,
        expected_status: &str,
        expected_worker: &str,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
        outcome: &str,
        run_id: Option<&str>,
    ) -> Result<()> {
        let timing = OperationTimer::start("database_task_status_transition")
            .with_database("tasks")
            .with_task_id(Some(id))
            .with_worker(expected_worker);
        let mut timing = timing;
        if let Some(run_id) = run_id {
            timing = timing.with_run_id(run_id);
        }
        let tx = self.conn.unchecked_transaction()?;
        let current: Option<(String, Option<String>)> = tx
            .query_row(TASK_STATUS_AND_OWNER_BY_ID_SQL, params![id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        let Some((current_status, current_worker)) = current else {
            return Err(ArcanaError::Agent(format!("Task #{id} not found")));
        };
        if current_status != expected_status || current_worker.as_deref() != Some(expected_worker) {
            return Err(ArcanaError::Agent(format!(
                "Task #{id} changed from {expected_status} owned by {expected_worker} before the status update; reload it and retry"
            )));
        }
        Self::update_status_with_reason_on_connection(
            &tx,
            id,
            status,
            assigned_worker,
            rejection_reason,
        )?;

        let pending: Option<(String, String)> = tx
            .query_row(
                ROUTING_LATEST_PENDING_FOR_WORKER_SQL,
                params![id.to_string(), expected_worker],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((history_id, assigned_at)) = pending else {
            // Manually assigned work has no affinity row. The task status is
            // still valid; there is simply no routing observation to update.
            tx.commit()?;
            timing.with_result_count(1).succeed();
            return Ok(());
        };
        let duration_ms = crate::db::timestamps::parse_db_timestamp(&assigned_at)
            .ok()
            .map(|assigned| {
                chrono::Utc::now()
                    .signed_duration_since(assigned)
                    .num_milliseconds()
            })
            .filter(|ms| *ms >= 0)
            .unwrap_or(0);
        let updated = tx.execute(
            "UPDATE routing_history
             SET outcome = ?1, outcome_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 duration_ms = CASE WHEN ?2 > 0 THEN ?2 ELSE duration_ms END
             WHERE id = ?3 AND outcome IS NULL",
            params![outcome, duration_ms, history_id],
        )?;
        if updated != 1 {
            return Err(ArcanaError::Agent(format!(
                "Routing assignment {history_id} for task #{id} changed before its outcome was recorded"
            )));
        }
        tx.commit()?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    /// Transition a task on behalf of its currently claimed worker. The
    /// ownership check and update share one SQLite transaction so a stale
    /// worker cannot finish work after reassignment.
    pub fn update_status_for_worker_claim(
        &self,
        id: i64,
        expected_worker: &str,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
    ) -> Result<()> {
        self.update_status_for_worker_claim_and_run_id(
            id,
            expected_worker,
            status,
            assigned_worker,
            rejection_reason,
            None,
            None,
        )
    }

    pub fn update_status_for_worker_claim_and_run_id(
        &self,
        id: i64,
        expected_worker: &str,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
        run_id: Option<&str>,
        workspace_path: Option<&std::path::Path>,
    ) -> Result<()> {
        let _context = if status == "qc_review" {
            Some(self.bind_qc_workspace_context(
                id,
                workspace_path.ok_or_else(|| {
                    ArcanaError::Agent(
                        "QC transition requires the currently selected workspace".into(),
                    )
                })?,
            )?)
        } else {
            None
        };
        let timing = OperationTimer::start("database_task_status_transition")
            .with_database("tasks")
            .with_task_id(Some(id))
            .with_worker(expected_worker);
        let mut timing = timing;
        if let Some(run_id) = run_id {
            timing = timing.with_run_id(run_id);
        }
        let tx = self.conn.unchecked_transaction()?;
        let current: Option<(String, Option<String>)> = tx
            .query_row(TASK_STATUS_AND_OWNER_BY_ID_SQL, params![id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        let Some((current_status, current_worker)) = current else {
            return Err(ArcanaError::Agent(format!("Task #{id} not found")));
        };
        if current_status != "in_progress" || current_worker.as_deref() != Some(expected_worker) {
            return Err(ArcanaError::Agent(format!(
                "Task #{id} is no longer claimed in progress by {expected_worker}; reload the task before retrying"
            )));
        }

        Self::update_status_with_reason_on_connection(
            &tx,
            id,
            status,
            assigned_worker,
            rejection_reason,
        )?;

        let outcome = match status {
            "done" => Some("success"),
            "failed" => Some("failure"),
            _ => None,
        };
        if let Some(outcome) = outcome {
            tx.execute(
                ROUTING_OUTCOME_FOR_WORKER_CLAIM_SQL,
                params![outcome, id.to_string(), expected_worker],
            )?;
        }
        tx.commit()?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    fn update_status_with_reason_on_connection(
        conn: &rusqlite::Connection,
        id: i64,
        status: &str,
        assigned_worker: Option<&str>,
        rejection_reason: Option<&str>,
    ) -> Result<()> {
        if status == "qc_review" {
            let batch: Option<(String, i64, i64, i64, String, Option<String>, i64, i64, i64)> =
                conn.query_row(LATEST_BUILD_BATCH_FOR_QC_SQL, params![id], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                    ))
                })
                .optional()?;
            match batch {
                None => {
                    return Err(ArcanaError::Agent(format!(
                        "Cannot move task #{id} to qc_review without a persisted project build batch"
                    )))
                }
                Some((outcome, expected, observed, passing, path, fingerprint, evidence_rows, bad_rows, selected_workspace_matches))
                    if outcome != "passed"
                        || expected <= 0
                        || observed != expected
                        || passing != expected
                        || evidence_rows != expected
                        || bad_rows != 0
                        || selected_workspace_matches != 1
                        || !fingerprint.as_ref().is_some_and(|expected_fingerprint| {
                            crate::platform::workspace_fingerprint::fingerprint(
                                std::path::Path::new(&path),
                            )
                            .is_ok_and(|actual| actual == *expected_fingerprint)
                        }) =>
                {
                    return Err(ArcanaError::Agent(format!(
                        "Cannot move task #{id} to qc_review because its latest project build batch is {outcome} or lacks complete passing evidence for the current workspace"
                    )))
                }
                Some(_) => {}
            }
        }
        // completed_at is set atomically: strftime('%Y-%m-%dT%H:%M:%fZ','now') on done, NULL on any other transition.
        // This ensures the "Done Today" filter stays accurate across status cycles.
        // updated_at is always refreshed so stall detection (supervisor) stays accurate.
        let completed_at_sql = if status == "done" {
            "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
        } else {
            "NULL"
        };
        let rows = if let Some(worker) = assigned_worker {
            let sql = format!(
                "UPDATE tasks SET status = ?1, assigned_worker = ?2, rejection_reason = ?3, \
                 claimed_by_pid = CASE WHEN ?1 = 'in_progress' AND status = 'in_progress' \
                                       AND assigned_worker = ?2 THEN claimed_by_pid ELSE NULL END, \
                 claim_host = CASE WHEN ?1 = 'in_progress' AND status = 'in_progress' \
                                   AND assigned_worker = ?2 THEN claim_host ELSE NULL END, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), completed_at = {completed_at_sql} WHERE id = ?4"
            );
            conn.execute(&sql, params![status, worker, rejection_reason, id])?
        } else {
            // P8-29 — `None` UNASSIGNS. It used to omit the column entirely.
            //
            // 🔴 The tool's own description tells the model *"Pass null to
            // unassign"* (`update_task_status_tool.rs:48`), and a comment
            // twenty lines below it said *"null or absent → leave
            // unassigned"* — two different meanings for one argument, and the
            // SQL implemented the second. So `update_status(id, "ready", None)`
            // moved the task to `ready` while it stayed tagged with its
            // worker: half-assigned, claimable by nobody, and invisible to any
            // query that reads `assigned_worker` to decide who owns it.
            //
            // ⚠ The claim fields go with it. `release_orphaned_task` (below)
            // already clears `assigned_worker`, `claimed_by_pid` and
            // `claim_host` as one unit, because a task with no worker cannot
            // have a live claim — leaving a PID behind makes it an orphan
            // candidate on the next sweep. This makes THIS path internally
            // consistent; the broader "clear them on every non-claim write" is
            // `P4-40`'s job and is not done here.
            let sql = format!(
                "UPDATE tasks SET status = ?1, assigned_worker = NULL, \
                 claimed_by_pid = NULL, claim_host = NULL, rejection_reason = ?2, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), completed_at = {completed_at_sql} WHERE id = ?3"
            );
            conn.execute(&sql, params![status, rejection_reason, id])?
        };
        if rows == 0 {
            return Err(ArcanaError::Agent(format!("Task #{id} not found")));
        }
        Ok(())
    }

    /// Assign a task and record the matching affinity-history row in one
    /// commit. Batch callers may invoke this once per entry to preserve their
    /// existing partial-success contract without leaving half an assignment.
    pub fn assign_with_routing_history(
        &self,
        id: i64,
        worker_slug: &str,
        entry: &RoutingHistoryEntry,
    ) -> Result<()> {
        self.assign_with_routing_decision(id, worker_slug, entry, "automatic", None)
    }

    /// Persist explicit worker choices for audit without letting them train
    /// automatic routing. A non-empty reason is required by construction.
    pub fn assign_with_routing_override(
        &self,
        id: i64,
        worker_slug: &str,
        entry: &RoutingHistoryEntry,
        reason: &str,
    ) -> Result<()> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(ArcanaError::Agent(
                "an explicit assignment override requires a reason".into(),
            ));
        }
        self.assign_with_routing_decision(id, worker_slug, entry, "override", Some(reason))
    }

    fn assign_with_routing_decision(
        &self,
        id: i64,
        worker_slug: &str,
        entry: &RoutingHistoryEntry,
        assignment_method: &str,
        override_reason: Option<&str>,
    ) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_task_assignment",
            event_type = "trace_span",
            operation = "database_task_assign_with_routing",
            database = "tasks",
            task_id = id,
            worker = %worker_slug,
        )
        .entered();
        let timing = OperationTimer::start("database_task_assign_with_routing")
            .with_database("tasks")
            .with_task_id(Some(id))
            .with_worker(worker_slug);
        if entry.task_id != id.to_string() || entry.worker_slug != worker_slug {
            return Err(ArcanaError::Agent(
                "routing-history task and worker must match the assignment".into(),
            ));
        }

        let tx = self.conn.unchecked_transaction()?;
        let changed = tx.execute(ASSIGN_WITH_ROUTING_DECISION_SQL, params![id, worker_slug])?;
        if changed != 1 {
            return Err(ArcanaError::Agent(format!(
                "Task #{id} is not assignable to '{worker_slug}': it may not be dispatchable, or the worker already has an in-progress task"
            )));
        }

        tx.execute(
            "INSERT INTO routing_history
             (id, task_id, task_title, task_type, worker_slug, assigned_at,
              outcome, outcome_at, duration_ms, tokens_used, assignment_method,
              override_reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                assignment_method,
                override_reason,
            ],
        )?;
        tx.commit()?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    /// Atomically increment retry_count for a task. Called by the Supervisor
    /// whenever it resets a build_failed task back to ready for another attempt.
    pub fn increment_retry(&self, id: i64) -> Result<()> {
        let mut operation = OperationTimer::start("database_task_retry_increment")
            .with_database("tasks")
            .with_task_id(Some(id));
        let rows = self.conn.execute(
            "UPDATE tasks SET retry_count = retry_count + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?1",
            params![id],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    pub fn get_retry_count(&self, id: i64) -> Result<i64> {
        let operation = OperationTimer::start("database_task_retry_count_read")
            .with_database("tasks")
            .with_task_id(Some(id));
        let count = self
            .conn
            .query_row(TASK_RETRY_COUNT_BY_ID_SQL, params![id], |row| row.get(0))?;
        operation.with_result_count(1).succeed();
        Ok(count)
    }

    pub fn delete(&self, id: i64) -> Result<()> {
        let mut operation = OperationTimer::start("database_task_delete")
            .with_database("tasks")
            .with_task_id(Some(id));
        let deleted = self
            .conn
            .execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        operation = operation.with_result_count(deleted);
        if deleted == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
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
    /// GAP-11 / §0.8: this is the one wholesale delete in the task store — every
    /// task and every worker request, with the id sequences reset — so it takes
    /// a backup first. A failure to back up **stops the clear**: emptying the
    /// board on the belief that a snapshot exists is precisely the outcome this
    /// guards against.
    ///
    /// Row-level deletes deliberately do not do this. Copying the database
    /// before deleting one task would make the board unusable, and a single row
    /// is not the loss this rule is about.
    pub fn clear_all(&self) -> Result<()> {
        let operation = OperationTimer::start("database_tasks_clear_all").with_database("tasks");
        if let Some(path) = self
            .conn
            .path()
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from)
        {
            let backup = crate::db::backup::backup_before(&path, "clear-board").map_err(|e| {
                ArcanaError::Agent(format!(
                    "refusing to clear the task board because no backup could be taken: {e}"
                ))
            })?;
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "task_board_backup_created",
                operation = "clear_board_backup",
                bytes = backup.bytes,
                reason = "clear-board",
                "Task board backup created before clear"
            );
        }

        self.conn.execute_batch(
            "BEGIN;
             DELETE FROM worker_requests;
             DELETE FROM tasks;
             DELETE FROM sqlite_sequence WHERE name IN ('tasks', 'worker_requests');
             COMMIT;",
        )?;
        operation.with_result_count(1).succeed();
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Worker Requests — worker→supervisor communication queue
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn create_worker_request(&self, input: &CreateWorkerRequestInput) -> Result<WorkerRequest> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_request_create")
                .with_database("tasks")
                .with_task_id(input.task_id)
                .with_worker(&input.worker_slug);
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
        let request = self
            .get_worker_request_by_id(id)?
            .ok_or_else(|| ArcanaError::Agent("Worker request not found after insert".into()))?;
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(request)
    }

    /// Create the typed escalation used when a worker has reached the
    /// repeated-build-failure threshold. The task claim, owner and threshold
    /// are checked in the same transaction as the request insert.
    pub fn create_build_retry_request_for_worker(
        &self,
        worker_slug: &str,
        task_id: i64,
        payload: &str,
    ) -> Result<WorkerRequest> {
        let mut operation = OperationTimer::start("database_build_retry_request_create")
            .with_database("tasks")
            .with_task_id(Some(task_id))
            .with_worker(worker_slug);
        let tx = self.conn.unchecked_transaction()?;
        let diagnostic_batch_id: Option<i64> = tx
            .query_row(
                "SELECT b.id FROM project_build_batches b JOIN tasks t ON t.id = b.task_id
                 WHERE t.id = ?1 AND t.assigned_worker = ?2 AND t.status = 'in_progress'
                   AND t.claimed_by_pid IS NOT NULL AND t.retry_count >= 3
                   AND b.outcome IN ('failed','incomplete') AND b.completed_at IS NOT NULL
                   AND b.id = (SELECT MAX(latest.id) FROM project_build_batches latest
                               WHERE latest.task_id = t.id)",
                params![task_id, worker_slug],
                |row| row.get(0),
            )
            .optional()?;
        let Some(diagnostic_batch_id) = diagnostic_batch_id else {
            return Err(ArcanaError::Agent(
                "a build retry approval request requires the current claimed task, three observed failures, and a latest failed or incomplete build batch".into(),
            ));
        };
        let pending: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM worker_requests
             WHERE task_id = ?1 AND request_type = 'build_retry_approval' AND status = 'pending')",
            params![task_id],
            |row| row.get(0),
        )?;
        if pending {
            return Err(ArcanaError::Agent(
                "a build retry approval request is already waiting for the Supervisor".into(),
            ));
        }
        tx.execute(
            "INSERT INTO worker_requests
             (worker_slug, task_id, request_type, payload, status, diagnostic_batch_id)
             VALUES (?1, ?2, 'build_retry_approval', ?3, 'pending', ?4)",
            params![worker_slug, task_id, payload, diagnostic_batch_id],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        operation = operation.with_result_count(1);
        operation.succeed();
        self.get_worker_request_by_id(id)?
            .ok_or_else(|| ArcanaError::Agent("build retry request missing after insert".into()))
    }

    /// Supervisor-only, audited approval of one subsequent project build
    /// batch. The grant remains unavailable until the worker's batch-start
    /// transaction consumes it.
    pub fn approve_build_retry_request(
        &self,
        request_id: i64,
        supervisor_slug: &str,
        reason: &str,
    ) -> Result<()> {
        let mut operation = OperationTimer::start("database_build_retry_approval_create")
            .with_database("tasks")
            .with_worker(supervisor_slug);
        if supervisor_slug.trim() != "supervisor" {
            return Err(ArcanaError::Agent(
                "only the Supervisor can approve a build retry".into(),
            ));
        }
        let reason = reason.trim();
        if reason.is_empty() || reason.len() > 500 {
            return Err(ArcanaError::Agent(
                "approval requires a reason between 1 and 500 bytes".into(),
            ));
        }
        let tx = self.conn.unchecked_transaction()?;
        let request: Option<(i64, String, String, i64)> = tx
            .query_row(
                "SELECT r.task_id, r.worker_slug, t.assigned_worker, r.diagnostic_batch_id
                 FROM worker_requests r JOIN tasks t ON t.id = r.task_id
                 WHERE r.id = ?1 AND r.request_type = 'build_retry_approval'
                   AND r.status = 'pending' AND t.status = 'in_progress'
                   AND t.retry_count >= 3
                   AND r.diagnostic_batch_id = (SELECT MAX(b.id) FROM project_build_batches b
                                                WHERE b.task_id = t.id
                                                  AND b.outcome IN ('failed','incomplete')
                                                  AND b.completed_at IS NOT NULL)",
                params![request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        let Some((task_id, worker_slug, assigned_worker, diagnostic_batch_id)) = request else {
            return Err(ArcanaError::Agent(
                "build retry request is missing, stale, or no longer eligible".into(),
            ));
        };
        if assigned_worker != worker_slug {
            return Err(ArcanaError::Agent(
                "build retry request worker no longer owns the task".into(),
            ));
        }
        let available: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM build_retry_approvals
             WHERE task_id = ?1 AND status = 'available')",
            params![task_id],
            |row| row.get(0),
        )?;
        if available {
            return Err(ArcanaError::Agent(
                "this task already has an unused Supervisor build retry approval".into(),
            ));
        }
        let _approval_context = SupervisorApprovalContextGuard::enable(std::sync::Arc::clone(
            &self.supervisor_approval_context,
        ));
        tx.execute(
            "INSERT INTO build_retry_approvals
             (request_id, task_id, worker_slug, diagnostic_batch_id, approved_by, reason, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'available')",
            params![
                request_id,
                task_id,
                worker_slug,
                diagnostic_batch_id,
                supervisor_slug,
                reason
            ],
        )?;
        drop(_approval_context);
        let changed = tx.execute(
            "UPDATE worker_requests SET status = 'completed', resolution_note = ?1,
             resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?2 AND request_type = 'build_retry_approval' AND status = 'pending'",
            params![format!("Approved one build batch: {reason}"), request_id],
        )?;
        if changed != 1 {
            return Err(ArcanaError::Agent(
                "build retry request changed while approval was being recorded".into(),
            ));
        }
        tx.commit()?;
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(())
    }

    pub fn get_worker_request_by_id(&self, id: i64) -> Result<Option<WorkerRequest>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_request_read")
                .with_database("tasks");
        let mut stmt = self.conn.prepare(
            "SELECT id, worker_slug, task_id, request_type, payload, status,
                    resolution_note, created_at, resolved_at
             FROM worker_requests WHERE id = ?1",
        )?;
        let req = stmt
            .query_row(params![id], row_to_worker_request)
            .optional()?;
        operation = operation.with_result_count(usize::from(req.is_some()));
        if req.is_some() {
            operation.succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(req)
    }

    pub fn list_worker_requests(&self, status_filter: Option<&str>) -> Result<Vec<WorkerRequest>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_requests_list")
                .with_database("tasks");
        let (query, values): (&str, Vec<String>) = if let Some(status) = status_filter {
            (WORKER_REQUESTS_BY_STATUS_SQL, vec![status.to_string()])
        } else {
            (WORKER_REQUESTS_RECENT_SQL, vec![])
        };
        let mut stmt = self.conn.prepare(query)?;
        let reqs = if values.is_empty() {
            stmt.query_map([], row_to_worker_request)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![values[0]], row_to_worker_request)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        operation = operation.with_result_count(reqs.len());
        operation.succeed();
        Ok(reqs)
    }

    pub fn update_worker_request_status(
        &self,
        id: i64,
        status: &str,
        resolution_note: Option<&str>,
    ) -> Result<()> {
        let operation =
            crate::observability::timing::OperationTimer::start("database_worker_request_update")
                .with_database("tasks");
        let resolved_at = if status == "completed" || status == "rejected" {
            "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
        } else {
            "NULL"
        };
        let sql = format!(
            "UPDATE worker_requests SET status = ?1, resolution_note = ?2, resolved_at = {}
             WHERE id = ?3",
            resolved_at
        );
        let rows = self
            .conn
            .execute(&sql, params![status, resolution_note, id])?;
        operation.with_result_count(rows).set_outcome(if rows == 0 {
            "not_found"
        } else {
            "success"
        });
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Arcana Communication Protocol (ACP) — worker↔worker mediated inbox
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
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_message_insert")
                .with_database("tasks")
                .with_task_id(task_id)
                .with_worker(to_slug);
        self.conn.execute(
            "INSERT INTO worker_messages (from_slug, to_slug, body, kind, task_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![from_slug, to_slug, body, kind, task_id],
        )?;
        let id = self.conn.last_insert_rowid();
        let message = self.get_worker_message(id)?.ok_or_else(|| {
            ArcanaError::Agent("Worker message inserted but vanished on read-back".into())
        })?;
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(message)
    }

    pub fn get_worker_message(&self, id: i64) -> Result<Option<WorkerMessage>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_message_read")
                .with_database("tasks");
        let mut stmt = self.conn.prepare(
            "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
             FROM worker_messages WHERE id = ?1",
        )?;
        let msg = stmt
            .query_row(params![id], row_to_worker_message)
            .optional()?;
        operation = operation.with_result_count(usize::from(msg.is_some()));
        if msg.is_some() {
            operation.succeed();
        } else {
            operation.set_outcome("not_found");
        }
        Ok(msg)
    }

    /// List messages addressed to `to_slug`. When `unread_only` is true,
    /// returns every unacknowledged message (`pending` or `read`). Listing a
    /// pending message transitions it to `read`, but it remains deliverable
    /// with the same ID until the recipient acknowledges it.
    pub fn list_worker_messages_for(
        &self,
        to_slug: &str,
        unread_only: bool,
    ) -> Result<Vec<WorkerMessage>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_inbox_read")
                .with_database("tasks")
                .with_worker(to_slug);
        // Step 1: snapshot the rows we're about to return so the caller sees
        // the row's `status='pending'` state as it was when they listed
        // (rather than racing the UPDATE below). The mark-read transition is
        // applied immediately afterwards under the same connection so any
        // follow-up read sees `read`.
        let sql = if unread_only {
            WORKER_MESSAGES_UNACKED_FOR_RECIPIENT_SQL
        } else {
            WORKER_MESSAGES_FOR_RECIPIENT_SQL
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
                let tx = self.conn.unchecked_transaction()?;
                {
                    let mut update = tx.prepare_cached(
                        "UPDATE worker_messages SET status='read', read_at=strftime('%Y-%m-%dT%H:%M:%fZ','now')
                         WHERE id = ?1 AND status='pending'",
                    )?;
                    for id in pending_ids {
                        update.execute(params![id])?;
                    }
                }
                tx.commit()?;
            }
        }
        operation = operation.with_result_count(messages.len());
        operation.succeed();
        Ok(messages)
    }

    /// Mark a message as acknowledged. Caller must verify ownership
    /// (`to_slug` matches the calling agent) at the tool layer.
    pub fn acknowledge_worker_message(&self, id: i64) -> Result<()> {
        let operation = crate::observability::timing::OperationTimer::start(
            "database_worker_message_acknowledge",
        )
        .with_database("tasks");
        let rows = self.conn.execute(
            "UPDATE worker_messages
             SET status = 'acknowledged'
             WHERE id = ?1 AND status IN ('pending','read')",
            params![id],
        )?;
        if rows == 0 {
            operation.with_result_count(0).set_outcome("not_found");
            return Err(ArcanaError::Agent(format!(
                "Worker message #{id} not found or already acknowledged."
            )));
        }
        operation.with_result_count(rows).succeed();
        Ok(())
    }

    /// Supervisor drain: messages still pending OR read (un-acked) for more
    /// than `older_than_minutes`. Used by the Supervisor recipe to surface
    /// stuck cross-worker conversations as escalations.
    pub fn list_stale_worker_messages(
        &self,
        older_than_minutes: i64,
    ) -> Result<Vec<WorkerMessage>> {
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_worker_stale_messages_read",
        )
        .with_database("tasks");
        let mut stmt = self.conn.prepare(WORKER_MESSAGES_STALE_SQL)?;
        let offset = format!("-{older_than_minutes} minutes");
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![offset], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation = operation.with_result_count(messages.len());
        operation.succeed();
        Ok(messages)
    }

    /// Fleet Chat panel — full message list for one task, in chronological
    /// order. Returns at most `limit` rows.
    pub fn list_worker_messages_for_task(
        &self,
        task_id: i64,
        limit: i64,
    ) -> Result<Vec<WorkerMessage>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_messages_read")
                .with_database("tasks")
                .with_task_id(Some(task_id));
        let mut stmt = self.conn.prepare(WORKER_MESSAGES_FOR_TASK_SQL)?;
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![task_id, limit], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation = operation.with_result_count(messages.len());
        operation.succeed();
        Ok(messages)
    }

    /// Fleet Chat panel — all messages across all tasks, newest first.
    /// Bounded by `limit` so the UI never pulls an unbounded backlog.
    pub fn list_recent_worker_messages(&self, limit: i64) -> Result<Vec<WorkerMessage>> {
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_worker_messages_recent_read",
        )
        .with_database("tasks");
        let mut stmt = self.conn.prepare(WORKER_MESSAGES_RECENT_SQL)?;
        let messages: Vec<WorkerMessage> = stmt
            .query_map(params![limit], row_to_worker_message)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation = operation.with_result_count(messages.len());
        operation.succeed();
        Ok(messages)
    }

    /// Per-target unread count, used by the AgentLiveCard badge.
    pub fn count_unread_for(&self, to_slug: &str) -> Result<i64> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_unread_count")
                .with_database("tasks")
                .with_worker(to_slug);
        let count: i64 =
            self.conn
                .query_row(UNREAD_MESSAGE_COUNT_SQL, params![to_slug], |row| row.get(0))?;
        operation = operation.with_result_count(count.max(0) as usize);
        operation.succeed();
        Ok(count)
    }

    /// Count messages that still need durable acknowledgement. `read` remains
    /// deliverable because a consumer may fail after reading but before ack.
    pub fn count_unacknowledged_for(&self, to_slug: &str) -> Result<i64> {
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_worker_unacknowledged_count",
        )
        .with_database("tasks")
        .with_worker(to_slug);
        let count: i64 =
            self.conn
                .query_row(UNACKNOWLEDGED_MESSAGE_COUNT_SQL, params![to_slug], |row| {
                    row.get(0)
                })?;
        operation = operation.with_result_count(count.max(0) as usize);
        operation.succeed();
        Ok(count)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // PID-Based Orphan Detection (Ruflo Feature 2)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Claim a task and record the worker's PID for orphan detection.
    /// Transitions status from `assigned` → `in_progress`.
    ///
    /// Enforce owner, task kind, and one-active-task constraints in the same
    /// write that changes status. A preflight read is not safe here: concurrent
    /// callers could both observe an idle worker and then claim different rows.
    pub fn claim_task_with_pid(&self, id: i64, worker_slug: &str, pid: u32) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_task_assigned_claim",
            event_type = "trace_span",
            operation = "database_task_claim_assigned",
            database = "tasks",
            task_id = id,
            worker = %worker_slug,
        )
        .entered();
        let timing = OperationTimer::start("database_task_claim_assigned")
            .with_database("tasks")
            .with_task_id(Some(id))
            .with_worker(worker_slug);
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        let rows = self.conn.execute(
            CLAIM_TASK_WITH_PID_SQL,
            params![worker_slug, pid as i64, host, id],
        )?;
        if rows == 0 {
            // The write predicate also rejects a mismatched owner, an epic,
            // and a worker with another active task. Do not perform a second
            // read here; the state could change again before that read.
            return Err(ArcanaError::Agent(format!(
                "Task #{id} could not be claimed by '{worker_slug}'. It must be \
                 assigned to that worker, be a non-epic task, and the worker \
                 must have no other in-progress task."
            )));
        }
        timing.with_result_count(rows).succeed();
        Ok(())
    }

    /// Claim the oldest `ready` task in ONE atomic statement.
    ///
    /// **Why this exists (A-22).** The two-step handoff — `update_status` to
    /// `assigned`, then `claim_task_with_pid` — models the supervisor's
    /// workflow, where a human or the orchestrator decides *who* gets a task
    /// before that worker picks it up. A self-serve worker pulling from the
    /// queue has no such decider, and running the two steps back to back leaves
    /// a window: a second worker can claim the task in between. The loser's
    /// claim then fails with the task left `assigned` and no PID, a state the
    /// orphan detector only releases after its 4-hour TTL. The post-phase audit
    /// found `arcanactl task next --claim` doing exactly that.
    ///
    /// A single `UPDATE` is atomic in SQLite: the write lock is held for the
    /// whole statement, so a concurrent claimer either waits and then re-runs
    /// the subselect — seeing the row is no longer `ready` and taking the next
    /// one — or finds the queue empty. There is no window to lose.
    ///
    /// `claim_task_with_pid` is untouched. This is an additional entry point
    /// for the self-serve case, not a change to the assigned-handoff contract.
    ///
    /// Returns `Ok(None)` when no task was ready. The one-in-progress-per-worker
    /// guard is enforced *inside* the statement, so a busy worker claims
    /// nothing even under a race; `Err` then carries which of the two it was.
    pub fn claim_next_ready(&self, worker_slug: &str, pid: u32) -> Result<Option<Task>> {
        let mut timing = OperationTimer::start("database_task_claim_next_ready")
            .with_database("tasks")
            .with_worker(worker_slug);
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        // The guard is a subquery rather than a prior SELECT for the same
        // reason the claim is one statement: a check that happens before the
        // write can be true when it is read and false when it is used.
        let claimed: Option<i64> = self
            .conn
            .query_row(
                CLAIM_NEXT_READY_SQL,
                params![worker_slug, pid as i64, host],
                |row| row.get(0),
            )
            .optional()?;

        match claimed {
            Some(id) => match self.get_by_id(id) {
                Ok(Some(task)) => {
                    timing
                        .with_task_id(Some(task.id))
                        .with_result_count(1)
                        .succeed();
                    Ok(Some(task))
                }
                Ok(None) => {
                    timing.set_outcome("empty");
                    Ok(None)
                }
                Err(error) => Err(error),
            },
            None => {
                // Nothing changed. Two causes, and the caller needs to tell
                // them apart: an empty queue is normal, a busy worker is a
                // rule violation the worker recipe warns about.
                let active: i64 = self.conn.query_row(
                    "SELECT COUNT(*) FROM tasks
                          WHERE assigned_worker = ?1 AND status = 'in_progress'",
                    params![worker_slug],
                    |row| row.get(0),
                )?;
                if active > 0 {
                    return Err(ArcanaError::Agent(format!(
                        "Worker '{worker_slug}' already owns {active} in_progress \
                         task(s); finish or release them before claiming another. \
                         This guard backs the worker recipe's 'NEVER claim a \
                         second task while one is in_progress' rule."
                    )));
                }
                timing.set_outcome("empty");
                Ok(None)
            }
        }
    }

    /// Returns all tasks in `assigned` or `in_progress` state — candidates for
    /// orphan checking. Both PID-tracked and non-PID (TTL fallback) tasks included.
    pub fn get_orphan_candidates(&self) -> Result<Vec<Task>> {
        let operation =
            OperationTimer::start("database_task_orphan_candidates_read").with_database("tasks");
        let mut stmt = self.conn.prepare(ORPHAN_CANDIDATES_SQL)?;
        let tasks = stmt
            .query_map([], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if tasks.is_empty() {
            operation.with_result_count(0).set_outcome("empty");
        } else {
            operation.with_result_count(tasks.len()).succeed();
        }
        Ok(tasks)
    }

    /// Release an automatically detected orphan only if the row still matches
    /// the snapshot whose process/TTL was checked. This compare-and-set keeps
    /// a delayed reconciler from clearing a newer assignment or claim.
    pub fn release_orphaned_task_if_unchanged(
        &self,
        observed: &Task,
        reason: &str,
    ) -> Result<bool> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_task_orphan_release",
            event_type = "trace_span",
            operation = "database_task_orphan_release",
            database = "tasks",
            task_id = observed.id,
        )
        .entered();
        let timing = OperationTimer::start("database_task_orphan_release")
            .with_database("tasks")
            .with_task_id(Some(observed.id));
        let changed = self.conn.execute(
            "UPDATE tasks SET status = 'ready', assigned_worker = NULL,
                             claimed_by_pid = NULL, claim_host = NULL,
                             rejection_reason = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?2 AND status = ?3 AND assigned_worker IS ?4
               AND claimed_by_pid IS ?5 AND claim_host IS ?6
               AND updated_at = ?7",
            params![
                format!("[orphan] {reason}"),
                observed.id,
                observed.status.as_str(),
                observed.assigned_worker,
                observed.claimed_by_pid,
                observed.claim_host,
                observed.updated_at,
            ],
        )?;
        timing
            .with_result_count(changed)
            .set_outcome(if changed == 1 { "success" } else { "stale" });
        Ok(changed == 1)
    }

    /// Test fixture for legacy worker-wide recovery semantics. Production
    /// recovery must use the run-owner-scoped method below.
    #[cfg(test)]
    pub fn release_in_progress_claims_for_worker(
        &self,
        worker_slug: &str,
        reason: &str,
    ) -> Result<Vec<i64>> {
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare(
            "SELECT id FROM tasks
             WHERE status = 'in_progress' AND assigned_worker = ?1
             ORDER BY id",
        )?;
        let ids = stmt
            .query_map([worker_slug], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        for id in &ids {
            tx.execute(
                "UPDATE tasks
                 SET status = 'ready', assigned_worker = NULL,
                     claimed_by_pid = NULL, claim_host = NULL,
                     rejection_reason = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?2 AND status = 'in_progress'
                   AND assigned_worker = ?3",
                params![format!("[worker recovery] {reason}"), id, worker_slug],
            )?;
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Quarantine only claims belonging to the exact interrupted worker
    /// process. A replacement must reconcile uncertain side effects before
    /// returning these tasks to the automatic `ready` queue.
    pub fn hold_interrupted_claims_for_worker_run(
        &self,
        worker_slug: &str,
        claim_pid: i64,
        claim_host: &str,
        reason: &str,
    ) -> Result<Vec<i64>> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_task_recovery_quarantine",
            event_type = "trace_span",
            operation = "database_task_recovery_quarantine",
            database = "tasks",
            worker = %worker_slug,
        )
        .entered();
        let timing = OperationTimer::start("database_task_recovery_quarantine")
            .with_database("tasks")
            .with_worker(worker_slug);
        let tx = self.conn.unchecked_transaction()?;
        let mut stmt = tx.prepare(HOLD_INTERRUPTED_CLAIMS_SELECT_SQL)?;
        let ids = stmt
            .query_map(params![worker_slug, claim_pid, claim_host], |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        for id in &ids {
            tx.execute(
                "UPDATE tasks
                 SET status = 'needs_info',
                     claimed_by_pid = NULL, claim_host = NULL,
                     rejection_reason = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                 WHERE id = ?2 AND status = 'in_progress'
                   AND assigned_worker = ?3 AND claimed_by_pid = ?4 AND claim_host = ?5",
                params![
                    format!("[worker recovery] {reason}"),
                    id,
                    worker_slug,
                    claim_pid,
                    claim_host
                ],
            )?;
        }
        tx.commit()?;
        timing.with_result_count(ids.len()).succeed();
        Ok(ids)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Routing History — Supervisor assignment records for affinity learning
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_routing_history(&self, entry: &RoutingHistoryEntry) -> Result<()> {
        let operation = OperationTimer::start("database_routing_history_insert")
            .with_database("tasks")
            .with_task_id(entry.task_id.parse::<i64>().ok())
            .with_worker(&entry.worker_slug);
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
        operation.with_result_count(1).succeed();
        Ok(())
    }

    pub fn update_routing_outcome(
        &self,
        task_id: &str,
        outcome: &str,
        duration_ms: i64,
        tokens_used: i64,
    ) -> Result<()> {
        let operation = OperationTimer::start("database_routing_history_outcome_update")
            .with_database("tasks")
            .with_task_id(task_id.parse::<i64>().ok());
        let changed = self.conn.execute(
            ROUTING_OUTCOME_FOR_TASK_SQL,
            params![outcome, duration_ms, tokens_used, task_id],
        )?;
        if changed == 0 {
            operation
                .with_result_count(changed)
                .set_outcome("not_found");
        } else {
            operation.with_result_count(changed).succeed();
        }
        Ok(())
    }

    /// Update only the newest pending history row for this task and its
    /// previously assigned worker. Returns `false` when there is no matching
    /// assignment, which is expected for tasks assigned manually outside the
    /// routing path.
    pub fn update_latest_routing_outcome_for_worker(
        &self,
        task_id: &str,
        worker_slug: &str,
        outcome: &str,
    ) -> Result<bool> {
        let operation = OperationTimer::start("database_routing_history_claim_outcome_update")
            .with_database("tasks")
            .with_task_id(task_id.parse::<i64>().ok())
            .with_worker(worker_slug);
        let pending: Option<(String, String)> = self
            .conn
            .query_row(
                ROUTING_LATEST_PENDING_FOR_WORKER_SQL,
                params![task_id, worker_slug],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((history_id, assigned_at)) = pending else {
            operation.with_result_count(0).set_outcome("not_found");
            return Ok(false);
        };
        let duration_ms = crate::db::timestamps::parse_db_timestamp(&assigned_at)
            .ok()
            .map(|assigned| {
                chrono::Utc::now()
                    .signed_duration_since(assigned)
                    .num_milliseconds()
            })
            .filter(|ms| *ms >= 0)
            .unwrap_or(0);
        let updated = self.conn.execute(
            "UPDATE routing_history
             SET outcome = ?1, outcome_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                 duration_ms = CASE WHEN ?2 > 0 THEN ?2 ELSE duration_ms END
             WHERE id = ?3 AND outcome IS NULL",
            params![outcome, duration_ms, history_id],
        )?;
        if updated == 1 {
            operation.with_result_count(updated).succeed();
        } else {
            operation
                .with_result_count(updated)
                .set_outcome("not_found");
        }
        Ok(updated == 1)
    }

    pub fn get_routing_history(&self, limit: i64) -> Result<Vec<RoutingHistoryEntry>> {
        let operation =
            OperationTimer::start("database_routing_history_read").with_database("tasks");
        let mut stmt = self.conn.prepare(ROUTING_HISTORY_RECENT_SQL)?;
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
        operation.with_result_count(entries.len()).succeed();
        Ok(entries)
    }

    /// Returns per-worker success rate keyed as "{worker_slug}:{task_type}" → rate (0.0–1.0).
    /// Only workers with at least 1 completed (success or failure) entry are included.
    pub fn get_worker_success_rates(&self) -> Result<HashMap<String, f64>> {
        let operation =
            OperationTimer::start("database_routing_success_rates_read").with_database("tasks");
        let mut stmt = self.conn.prepare(
            "SELECT worker_slug, task_type,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) AS successes,
                    SUM(CASE WHEN outcome IN ('success','failure') THEN 1 ELSE 0 END) AS total
             FROM routing_history
             WHERE assignment_method = 'automatic' AND outcome IS NOT NULL
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
        operation.with_result_count(map.len()).succeed();
        Ok(map)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Diff Risk Analysis — Ruflo Feature 3
    // ═══════════════════════════════════════════════════════════════════════════

    /// Store a serialized DiffAnalysisResult JSON on a task so the QC review
    /// outcome is available later for audit and so the Arcana panel can render
    /// the risk card without re-running the analysis on every refresh.
    pub fn set_task_diff_analysis(&self, task_id: i64, analysis_json: &str) -> Result<()> {
        let mut operation = OperationTimer::start("database_task_diff_analysis_write")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET diff_analysis = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![analysis_json, task_id],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
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
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_budget_set")
                .with_database("tasks")
                .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET budget_max_tokens = ?1, budget_max_usd = ?2,
             budget_max_turns = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?4",
            rusqlite::params![
                max_tokens.map(|t| t as i64),
                max_usd,
                max_turns.map(|t| t as i64),
                task_id,
            ],
        )?;
        operation = operation.with_result_count(rows);
        operation.set_outcome(if rows == 0 { "not_found" } else { "success" });
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
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_task_budget_usage_update",
        )
        .with_database("tasks")
        .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET budget_tokens_used = ?1, budget_cost_usd = ?2,
             budget_turns_used = ?3, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?4",
            rusqlite::params![tokens_used as i64, cost_usd, turns_used as i64, task_id],
        )?;
        operation = operation.with_result_count(rows);
        operation.set_outcome(if rows == 0 { "not_found" } else { "success" });
        Ok(())
    }

    /// Mark a task as budget-exceeded and transition it to needs_info.
    pub fn mark_task_budget_exceeded(&self, task_id: i64, reason: &str) -> Result<()> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_budget_exceeded")
                .with_database("tasks")
                .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET budget_exceeded = 1, budget_exceeded_reason = ?1,
             status = 'needs_info', claimed_by_pid = NULL, claim_host = NULL,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            rusqlite::params![reason, task_id],
        )?;
        operation = operation.with_result_count(rows);
        operation.set_outcome(if rows == 0 { "not_found" } else { "success" });
        Ok(())
    }

    /// Read back the current budget usage for a task.
    pub fn get_task_budget_status(&self, task_id: i64) -> Result<crate::budget::BudgetStatus> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_task_budget_read")
                .with_database("tasks")
                .with_task_id(Some(task_id));
        let row =
            self.conn
                .query_row(TASK_BUDGET_STATUS_SQL, rusqlite::params![task_id], |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?.unwrap_or(0) as u64,
                        row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                        row.get::<_, Option<i64>>(2)?.unwrap_or(0) as u32,
                        row.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                        row.get::<_, Option<String>>(4)?,
                    ))
                })?;
        let exceeded_reason = row.4.as_deref().map(|r| match r {
            "BUDGET_EXCEEDED_TOKENS" => crate::budget::BudgetExceededReason::TokenLimitReached,
            "BUDGET_EXCEEDED_USD" => crate::budget::BudgetExceededReason::CostLimitReached,
            _ => crate::budget::BudgetExceededReason::TurnLimitReached,
        });
        let status = crate::budget::BudgetStatus {
            tokens_used: row.0,
            cost_usd: row.1,
            turns_used: row.2,
            exceeded: row.3,
            exceeded_reason,
        };
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(status)
    }

    /// Aggregate today's budget usage across all tasks for the daily summary.
    pub fn get_daily_budget_summary(&self) -> Result<crate::budget::DailyBudgetSummary> {
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_daily_budget_summary_read",
        )
        .with_database("tasks");
        let row = self
            .conn
            .query_row(TASKS_DAILY_BUDGET_SUMMARY_SQL, [], |row| {
                Ok(crate::budget::DailyBudgetSummary {
                    total_allocated_tokens: row.get::<_, i64>(0)? as u64,
                    total_spent_tokens: row.get::<_, i64>(1)? as u64,
                    total_allocated_usd: row.get(2)?,
                    total_spent_usd: row.get(3)?,
                    tasks_exceeded: row.get::<_, i64>(4)? as u32,
                    tasks_with_budget: row.get::<_, i64>(5)? as u32,
                })
            })?;
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(row)
    }

    /// Aggregate routing_history into per-worker, per-task-type affinity stats.
    /// Returns rows ordered by worker_slug then task_type.
    pub fn get_affinity_stats(&self) -> Result<Vec<(String, String, i64, i64, i64, Option<i64>)>> {
        let operation =
            OperationTimer::start("database_routing_affinity_stats_read").with_database("tasks");
        let mut stmt = self.conn.prepare(ROUTING_AFFINITY_STATS_SQL)?;
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
        operation.with_result_count(rows.len()).succeed();
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
        let mut operation = OperationTimer::start("database_epic_create").with_database("tasks");
        self.conn.execute(
            "INSERT INTO tasks (title, description, priority, task_type, status, acceptance_criteria, created_by)
             VALUES (?1, ?2, 'p1', 'epic', 'ready', 'All subtasks complete', ?3)",
            params![title, description, created_by],
        )?;
        let id = self.conn.last_insert_rowid();
        let task = self
            .get_by_id(id)?
            .ok_or_else(|| ArcanaError::Agent("Epic not found after insert".into()))?;
        operation = operation.with_task_id(Some(id));
        operation.with_result_count(1).succeed();
        Ok(task)
    }

    /// Create a task with full phase metadata and optional blocker list.
    pub fn create_phased_task(&self, input: CreatePhasedTaskInput) -> Result<Task> {
        let mut operation =
            OperationTimer::start("database_phased_task_create").with_database("tasks");
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
        let task = self
            .get_by_id(id)?
            .ok_or_else(|| ArcanaError::Agent("Phased task not found after insert".into()))?;
        operation = operation.with_task_id(Some(id));
        operation.with_result_count(1).succeed();
        Ok(task)
    }

    /// All tasks belonging to an epic, ordered by phase_order then id.
    pub fn get_epic_tasks(&self, epic_id: &str) -> Result<Vec<Task>> {
        let operation = OperationTimer::start("database_epic_tasks_read").with_database("tasks");
        let mut stmt = self.conn.prepare(EPIC_TASKS_SQL)?;
        let tasks = stmt
            .query_map(params![epic_id], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation.with_result_count(tasks.len()).succeed();
        Ok(tasks)
    }

    /// Per-phase progress summary for an epic.
    pub fn get_phase_summary(&self, epic_id: &str) -> Result<Vec<PhaseSummary>> {
        let operation = OperationTimer::start("database_phase_summary_read").with_database("tasks");
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

        operation.with_result_count(summaries.len()).succeed();
        Ok(summaries)
    }

    /// Tasks with blocked_by entries where at least one blocker is not yet done.
    pub fn get_blocked_tasks(&self) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(GET_BLOCKED_TASKS_SQL)?;
        let candidates = stmt
            .query_map([], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut blocked = Vec::new();
        for task in candidates {
            let mut has_incomplete_blocker = false;
            for blocker in task.blocked_by.as_deref().unwrap_or_default() {
                let blocker_id = blocker.parse::<i64>().map_err(|error| {
                    ArcanaError::Agent(format!(
                        "Invalid blocker id '{blocker}' for task #{}: {error}",
                        task.id
                    ))
                })?;
                let status = self
                    .conn
                    .query_row(TASK_STATUS_BY_ID_SQL, params![blocker_id], |row| {
                        row.get::<_, String>(0)
                    })
                    .optional()?;
                if status.as_deref() != Some("done") {
                    has_incomplete_blocker = true;
                    break;
                }
            }
            if has_incomplete_blocker {
                blocked.push(task);
            }
        }
        Ok(blocked)
    }

    /// Called after a task is marked done. Checks all tasks blocked by `completed_task_id`
    /// — if ALL their blockers are now done, transitions them from needs_info → ready.
    /// Returns the IDs of tasks that were unblocked.
    /// Also auto-completes the parent epic if all its subtasks are done. Both
    /// operations commit together so a failed epic update cannot leave a
    /// partially advanced dependency graph.
    pub fn unblock_ready_tasks(&self, completed_task_id: i64) -> Result<Vec<i64>> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_task_dependency_reconcile",
            event_type = "trace_span",
            operation = "database_task_dependency_reconcile",
            database = "tasks",
            task_id = completed_task_id,
        )
        .entered();
        let timing = OperationTimer::start("database_task_dependency_reconcile")
            .with_database("tasks")
            .with_task_id(Some(completed_task_id));
        let tx = self.conn.unchecked_transaction()?;
        let completed_str = completed_task_id.to_string();
        let mut blocked_stmt = tx.prepare(BLOCKED_TASKS_FOR_UNBLOCK_SQL)?;
        let blocked_tasks = blocked_stmt
            .query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(blocked_stmt);
        let mut unblocked_ids: Vec<i64> = Vec::new();

        for (task_id, blocker_json) in &blocked_tasks {
            let blockers = serde_json::from_str::<Vec<String>>(blocker_json).map_err(|error| {
                ArcanaError::Agent(format!("Invalid blocker list for task #{task_id}: {error}"))
            })?;
            if !blockers.contains(&completed_str) {
                continue;
            }
            // Check if ALL blockers are now done
            let mut all_done = true;
            for blocker_id in &blockers {
                let blocker_id = blocker_id.parse::<i64>().map_err(|error| {
                    ArcanaError::Agent(format!(
                        "Invalid blocker id '{blocker_id}' for task #{task_id}: {error}"
                    ))
                })?;
                let status = tx
                    .query_row(TASK_STATUS_BY_ID_SQL, params![blocker_id], |row| {
                        row.get::<_, String>(0)
                    })
                    .optional()?;
                if status.as_deref() != Some("done") {
                    all_done = false;
                    break;
                }
            }
            if all_done {
                // Transition needs_info → ready
                let changed = tx.execute(
                    "UPDATE tasks SET status = 'ready', assigned_worker = NULL,
                                      claimed_by_pid = NULL, claim_host = NULL,
                                      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?1 AND status = 'needs_info'",
                    params![task_id],
                )?;
                if changed == 0 {
                    continue;
                }
                unblocked_ids.push(*task_id);
            }
        }

        // Epic auto-completion: if the completed task has an epic_id,
        // check if all subtasks of that epic are now done.
        let epic_id: Option<String> = tx
            .query_row(TASK_EPIC_ID_BY_ID_SQL, params![completed_task_id], |row| {
                row.get(0)
            })
            .optional()?
            .flatten();
        let mut completed_epic = None;
        if let Some(epic_id) = epic_id {
            let epic_id_int = epic_id.parse::<i64>().map_err(|error| {
                ArcanaError::Agent(format!(
                    "Invalid epic id '{epic_id}' on task #{completed_task_id}: {error}"
                ))
            })?;
            let (total, unfinished): (i64, i64) =
                tx.query_row(EPIC_COMPLETION_COUNTS_SQL, params![epic_id], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })?;
            if total > 0 && unfinished == 0 {
                let changed = tx.execute(
                    "UPDATE tasks SET status = 'done', completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                                      updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                     WHERE id = ?1 AND task_type = 'epic' AND status != 'done'",
                    params![epic_id_int],
                )?;
                if changed > 0 {
                    completed_epic = Some(epic_id_int);
                }
            }
        }

        tx.commit()?;

        for task_id in &unblocked_ids {
            crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                task_id: Some(*task_id),
                kind: "unblocked".into(),
                status: Some("ready".into()),
                assigned_worker: None,
            });
        }
        if let Some(epic_id) = completed_epic {
            crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                task_id: Some(epic_id),
                kind: "epic_complete".into(),
                status: Some("done".into()),
                assigned_worker: None,
            });
        }

        timing.with_result_count(unblocked_ids.len()).succeed();
        Ok(unblocked_ids)
    }

    // ─── Doc 04 Phase 2/4 — QC reviewer claim + cost rollup ──────────────────

    /// Atomically claim a task for QC review by setting `under_review_by` to
    /// the given subagent slug **only if** the slot is currently NULL. Returns
    /// `true` if the claim succeeded, `false` if another reviewer already
    /// owns the task. This is the single anti-double-spawn primitive — every
    /// `spawn_qc_review` call must check the return value before proceeding.
    pub fn try_claim_for_qc_review(&self, task_id: i64, subagent_slug: &str) -> Result<bool> {
        let mut operation = OperationTimer::start("database_qc_claim_try")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET under_review_by = ?2
             WHERE id = ?1 AND under_review_by IS NULL",
            params![task_id, subagent_slug],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 1 {
            operation.succeed();
        } else {
            operation.set_outcome("not_claimed");
        }
        Ok(rows == 1)
    }

    /// Clear the QC claim regardless of current value. Called when a
    /// reviewer reaches a terminal state (Completed / Failed / Terminated)
    /// or when the stranded-reviewer reconciler sweeps a dead subagent.
    pub fn release_qc_claim(&self, task_id: i64) -> Result<()> {
        let mut operation = OperationTimer::start("database_qc_claim_release")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET under_review_by = NULL WHERE id = ?1",
            params![task_id],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    /// Update the QC claim from the synthetic claim_token to the real
    /// subagent_id once the spawn has returned a handle. Lets the
    /// stranded-reviewer reconciler match `under_review_by` directly against
    /// `SubagentManager::list`'s live ids.
    pub fn upgrade_qc_claim(&self, task_id: i64, real_subagent_id: &str) -> Result<()> {
        let mut operation = OperationTimer::start("database_qc_claim_upgrade")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        let rows = self.conn.execute(
            "UPDATE tasks SET under_review_by = ?2 WHERE id = ?1",
            params![task_id, real_subagent_id],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    /// List task IDs currently claimed for QC review along with their slug.
    /// Used by the stranded-reviewer reconciler to detect rows whose
    /// reviewer subagent is no longer alive in the SubagentManager registry.
    pub fn list_qc_claims(&self) -> Result<Vec<(i64, String)>> {
        let operation = OperationTimer::start("database_qc_claims_list").with_database("tasks");
        let mut stmt = self.conn.prepare(QC_CLAIMS_SQL)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation.with_result_count(rows.len()).succeed();
        Ok(rows)
    }

    /// Add a USD increment to the running `task_qc_cost_usd` for a task. Called
    /// from the spawn_qc_review completion hook with the reviewer's
    /// `SubagentResult.cost_usd` so the QC ROI panel can show qc_cost /
    /// worker_cost ratios per task.
    pub fn add_qc_cost(&self, task_id: i64, cost_usd: f64) -> Result<()> {
        let mut operation = OperationTimer::start("database_qc_cost_add")
            .with_database("tasks")
            .with_task_id(Some(task_id));
        if cost_usd <= 0.0 {
            operation.set_outcome("skipped");
            operation.with_result_count(0);
            return Ok(());
        }
        let rows = self.conn.execute(
            "UPDATE tasks
             SET task_qc_cost_usd = COALESCE(task_qc_cost_usd, 0.0) + ?2
             WHERE id = ?1",
            params![task_id, cost_usd],
        )?;
        operation = operation.with_result_count(rows);
        if rows == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    /// Returns tasks in a specific phase that are ready to assign (status = ready | assigned).
    pub fn get_ready_tasks_in_phase(&self, epic_id: &str, phase: &str) -> Result<Vec<Task>> {
        let operation =
            OperationTimer::start("database_phase_ready_tasks_read").with_database("tasks");
        let mut stmt = self.conn.prepare(READY_TASKS_IN_PHASE_SQL)?;
        let tasks = stmt
            .query_map(params![epic_id, phase], row_to_task)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        operation.with_result_count(tasks.len()).succeed();
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
        status: {
            let raw: String = row.get(5)?;
            TaskStatus::from_str(&raw).map_err(|message| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        message,
                    )),
                )
            })?
        },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p4_23_message_replays_with_the_same_id_after_read_until_acknowledged() {
        let path = sandbox("p4-23-message-replay");
        let db = TasksDb::open_at(&path).expect("open isolated task database");
        let sent = db
            .insert_worker_message(
                "worker-backend",
                "worker-frontend",
                "Please wire the endpoint.",
                "request",
                None,
            )
            .expect("persist message");
        let second = db
            .insert_worker_message(
                "worker-qc",
                "worker-frontend",
                "The first route needs a regression check.",
                "note",
                None,
            )
            .expect("persist second message");
        for timestamp in [&sent.created_at, &second.created_at] {
            assert_eq!(
                timestamp.len(),
                24,
                "new message timestamp is canonical: {timestamp}"
            );
            assert!(timestamp.ends_with('Z'));
            crate::db::timestamps::parse_db_timestamp(timestamp).expect("message timestamp parses");
        }

        let first_delivery = db
            .list_worker_messages_for("worker-frontend", true)
            .expect("first inbox read");
        assert_eq!(first_delivery.len(), 2);
        assert_eq!(first_delivery[0].id, sent.id);
        assert_eq!(first_delivery[0].status, "pending");
        assert_eq!(first_delivery[1].id, second.id);
        assert_eq!(first_delivery[1].status, "pending");
        assert_eq!(db.count_unacknowledged_for("worker-frontend").unwrap(), 2);
        drop(db);

        let reopened = TasksDb::open_at(&path).expect("reopen isolated task database");
        let replay = reopened
            .list_worker_messages_for("worker-frontend", true)
            .expect("deliver unacknowledged message again");
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].id, sent.id, "replay uses the stable message ID");
        assert_eq!(replay[0].status, "read");
        let read_at = replay[0]
            .read_at
            .as_deref()
            .expect("production inbox reader returns persisted read_at");
        assert_eq!(read_at.len(), 24, "read timestamp is canonical: {read_at}");
        assert!(read_at.ends_with('Z'));
        crate::db::timestamps::parse_db_timestamp(read_at).expect("read timestamp parses");
        assert_eq!(replay[1].id, second.id);
        assert_eq!(replay[1].status, "read");

        let task = ready_task(&reopened, "timestamped worker request");
        let request = reopened
            .create_worker_request(&CreateWorkerRequestInput {
                worker_slug: "worker-frontend".into(),
                task_id: Some(task.id),
                request_type: "needs_info".into(),
                payload: "waiting on endpoint shape".into(),
            })
            .expect("create request through production API");
        assert_eq!(request.created_at.len(), 24);
        assert!(request.created_at.ends_with('Z'));
        crate::db::timestamps::parse_db_timestamp(&request.created_at)
            .expect("request creation timestamp parses");
        reopened
            .update_worker_request_status(request.id, "completed", Some("resolved"))
            .expect("resolve through production API");
        let completed = reopened
            .get_worker_request_by_id(request.id)
            .expect("read resolved request")
            .expect("request remains persisted");
        let resolved_at = completed
            .resolved_at
            .as_deref()
            .expect("production reader returns persisted resolution time");
        assert_eq!(resolved_at.len(), 24);
        assert!(resolved_at.ends_with('Z'));
        crate::db::timestamps::parse_db_timestamp(resolved_at)
            .expect("request resolution timestamp parses");

        reopened
            .acknowledge_worker_message(sent.id)
            .expect("acknowledge delivered message");
        let remaining = reopened
            .list_worker_messages_for("worker-frontend", true)
            .expect("read after ack");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, second.id);
        assert_eq!(
            reopened
                .count_unacknowledged_for("worker-frontend")
                .unwrap(),
            1
        );
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    fn ready_task(db: &TasksDb, title: &str) -> Task {
        db.create(CreateTaskInput {
            title: title.to_string(),
            description: None,
            priority: "p2".to_string(),
            task_type: "chore".to_string(),
            acceptance_criteria: "n/a".to_string(),
            created_by: Some("test".to_string()),
        })
        .expect("create")
    }

    fn move_to_qc_with_synthetic_workspace(db: &TasksDb, task_id: i64, worker: &str) -> Result<()> {
        db.update_status_with_workspace_and_run_id(
            task_id,
            "qc_review",
            Some(worker),
            None,
            None,
            &synthetic_build_evidence_workspace().0,
        )
    }

    #[test]
    fn worker_message_operations_emit_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let task = ready_task(&db, "message test task");
        let message = db
            .insert_worker_message(
                "worker-sender",
                "worker-recipient",
                "private-message-body-marker",
                "note",
                Some(task.id),
            )
            .expect("insert worker message");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let inbox = db
                .list_worker_messages_for("worker-recipient", true)
                .expect("deliver pending inbox message");
            let unread = db.count_unread_for("worker-recipient").unwrap();
            let unacknowledged = db.count_unacknowledged_for("worker-recipient").unwrap();
            let task_messages = db
                .list_worker_messages_for_task(task.id, 20)
                .expect("read task conversation");
            let stale = db.list_stale_worker_messages(0).expect("read stale inbox");
            db.acknowledge_worker_message(message.id)
                .expect("acknowledge delivered message");
            let after_ack = db.count_unacknowledged_for("worker-recipient").unwrap();
            let duplicate_ack = db.acknowledge_worker_message(message.id).is_err();
            (
                inbox,
                unread,
                unacknowledged,
                task_messages,
                stale,
                after_ack,
                duplicate_ack,
            )
        });

        assert_eq!(observed.0.len(), 1);
        assert_eq!(observed.0[0].id, message.id);
        assert_eq!(observed.0[0].status, "pending");
        assert_eq!(observed.1, 0);
        assert_eq!(observed.2, 1);
        assert_eq!(observed.3.len(), 1);
        assert_eq!(observed.4.len(), 1);
        assert_eq!(observed.5, 0);
        assert!(observed.6);

        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_worker_inbox_read",
            "database_worker_unread_count",
            "database_worker_unacknowledged_count",
            "database_task_messages_read",
            "database_worker_stale_messages_read",
            "database_worker_message_acknowledge",
        ] {
            assert!(
                operation_events
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing operation span {operation}"
            );
        }
        let missing_ack = operation_events
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_worker_message_acknowledge"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("duplicate acknowledgement is distinguishable");
        assert_eq!(missing_ack["fields"]["result_count"], 0);
        assert_eq!(missing_ack["fields"]["result_count_observed"], true);
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("private-message-body-marker"));
    }

    #[test]
    fn supervisor_worker_request_operations_emit_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let task = ready_task(&db, "request test task");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let request = db
                .create_worker_request(&CreateWorkerRequestInput {
                    worker_slug: "worker-request-test".into(),
                    task_id: Some(task.id),
                    request_type: "needs_info".into(),
                    payload: "private-request-payload-marker".into(),
                })
                .expect("create worker request");
            let fetched = db
                .get_worker_request_by_id(request.id)
                .expect("read worker request");
            let listed = db
                .list_worker_requests(Some("pending"))
                .expect("list pending worker requests");
            db.update_worker_request_status(
                request.id,
                "completed",
                Some("private-resolution-marker"),
            )
            .expect("resolve worker request");
            let completed = db
                .list_worker_requests(Some("completed"))
                .expect("list completed requests");
            (fetched, listed, completed)
        });

        assert_eq!(
            observed.0.as_ref().map(|request| request.task_id),
            Some(Some(task.id))
        );
        assert_eq!(observed.1.len(), 1);
        assert_eq!(observed.2.len(), 1);
        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_worker_request_create",
            "database_worker_request_read",
            "database_worker_requests_list",
            "database_worker_request_update",
        ] {
            assert!(
                operation_events
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing operation span {operation}"
            );
        }
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("private-request-payload-marker"));
        assert!(!serialized.contains("private-resolution-marker"));
    }

    #[test]
    fn task_budget_operations_emit_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let task = ready_task(&db, "budget telemetry task");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            db.set_task_budget(task.id, Some(100), Some(1.25), Some(4))
                .expect("set task budget");
            db.update_task_budget_usage(task.id, 25, 0.31, 2)
                .expect("update task usage");
            let status = db
                .get_task_budget_status(task.id)
                .expect("read task budget");
            let summary = db
                .get_daily_budget_summary()
                .expect("read daily task budget summary");
            db.mark_task_budget_exceeded(task.id, "private-budget-reason-marker")
                .expect("mark exceeded budget");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_budget BEFORE UPDATE OF budget_cost_usd ON tasks
                     BEGIN SELECT RAISE(ABORT, 'private-budget-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed = db.update_task_budget_usage(task.id, 26, 0.32, 3).is_err();
            (status, summary, failed)
        });

        assert_eq!(observed.0.tokens_used, 25);
        assert!((observed.0.cost_usd - 0.31).abs() < f64::EPSILON);
        assert_eq!(observed.0.turns_used, 2);
        assert_eq!(observed.1.tasks_with_budget, 1);
        assert_eq!(observed.1.total_allocated_tokens, 100);
        assert!(observed.2);
        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_task_budget_set",
            "database_task_budget_usage_update",
            "database_task_budget_read",
            "database_daily_budget_summary_read",
            "database_task_budget_exceeded",
        ] {
            assert!(
                operation_events
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing operation span {operation}"
            );
        }
        let failure = operation_events
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_task_budget_usage_update"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("budget persistence error event");
        assert_eq!(failure["fields"]["result_count_observed"], false);
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("private-budget-reason-marker"));
        assert!(!serialized.contains("private-budget-trigger-marker"));
    }

    #[test]
    fn task_board_readers_and_creator_emit_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let task = ready_task(&db, "task-private-title-marker");
            let present = db.get_by_id(task.id).expect("read created task");
            let missing = db.get_by_id(i64::MAX).expect("read absent task");
            let all = db.list(None).expect("read board list");
            let ready = db.list(Some("ready")).expect("read filtered board list");
            let empty_worker = db
                .in_progress_task_for_worker("worker-board-test")
                .expect("read unclaimed worker task");
            db.claim_next_ready("worker-board-test", 913)
                .expect("claim board task");
            let claimed = db
                .in_progress_task_for_worker("worker-board-test")
                .expect("read claimed worker task");
            let worker_rows = db
                .list_tasks_by_status_and_worker("in_progress", "worker-board-test")
                .expect("read worker task list");
            (
                task.id,
                present,
                missing,
                all,
                ready,
                empty_worker,
                claimed,
                worker_rows,
            )
        });

        assert_eq!(observed.1.as_ref().map(|task| task.id), Some(observed.0));
        assert!(observed.2.is_none());
        assert_eq!(observed.3.len(), 1);
        assert_eq!(observed.4.len(), 1);
        assert!(observed.5.is_none());
        assert_eq!(observed.6, Some(observed.0));
        assert_eq!(observed.7.len(), 1);

        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_task_create",
            "database_task_read",
            "database_task_list",
            "database_worker_claimed_task_read",
            "database_worker_task_list",
        ] {
            assert!(
                operation_events
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing operation span {operation}"
            );
        }
        let not_found = operation_events
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_task_read"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("missing task is distinct from an empty board");
        assert_eq!(not_found["fields"]["result_count"], 0);
        assert_eq!(not_found["fields"]["result_count_observed"], true);
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("task-private-title-marker"));
    }

    #[test]
    fn build_evidence_operations_emit_correlated_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let task = ready_task(&db, "build evidence telemetry task");
        db.update_status(task.id, "assigned", Some("worker-build-telemetry"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-build-telemetry", 914)
            .expect("claim task");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let recorded = db
                .record_build_attempts_with_workspace_for_worker(
                    "worker-build-telemetry",
                    "session-build-telemetry",
                    &[(
                        "private-build-command-marker".into(),
                        0,
                        "cargo".into(),
                        Some("private-workspace-path-marker".into()),
                        Some("private-fingerprint-marker".into()),
                    )],
                    None,
                )
                .expect("persist build attempt");
            let evidence = db
                .latest_build_evidence(task.id)
                .expect("read latest build evidence");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_build_evidence BEFORE INSERT ON build_evidence
                     BEGIN SELECT RAISE(ABORT, 'private-build-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed = db
                .record_build_attempts_with_workspace_for_worker(
                    "worker-build-telemetry",
                    "session-build-telemetry",
                    &[(
                        "private-failed-command-marker".into(),
                        1,
                        "cargo".into(),
                        Some("private-failed-path-marker".into()),
                        Some("private-failed-fingerprint-marker".into()),
                    )],
                    None,
                )
                .is_err();
            (recorded, evidence, failed)
        });

        assert_eq!(observed.0.map(|(id, _, _)| id), Some(task.id));
        assert_eq!(observed.1.as_ref().map(|row| row.exit_code), Some(0));
        assert!(observed.2);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        let write = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_build_attempt_batch_record"
                    && event["fields"]["outcome"] == "success"
            })
            .expect("successful build batch span");
        assert_eq!(write["fields"]["task_id"], task.id.to_string());
        assert_eq!(write["fields"]["result_count"], 1);
        let read = operations
            .iter()
            .find(|event| event["fields"]["operation"] == "database_build_evidence_read")
            .expect("build evidence reader span");
        assert_eq!(read["fields"]["result_count"], 1);
        let failure = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_build_attempt_batch_record"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("failed build evidence write span");
        assert_eq!(failure["fields"]["result_count_observed"], false);
        let serialized = serde_json::to_string(&events).expect("serialize build telemetry");
        for private_value in [
            "private-build-command-marker",
            "private-workspace-path-marker",
            "private-fingerprint-marker",
            "private-failed-command-marker",
            "private-failed-path-marker",
            "private-failed-fingerprint-marker",
            "private-build-trigger-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }

    #[test]
    fn routing_history_operations_emit_private_result_spans() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let task = ready_task(&db, "private-routing-title-marker");
        let entry = crate::routing::TaskRouter::assignment_entry(&task, "worker-routing-test");
        db.assign_with_routing_history(task.id, "worker-routing-test", &entry)
            .expect("persist automatic routing assignment");
        let legacy_task = ready_task(&db, "legacy routing telemetry task");
        let legacy_entry =
            crate::routing::TaskRouter::assignment_entry(&legacy_task, "worker-routing-legacy");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            db.insert_routing_history(&legacy_entry)
                .expect("insert legacy routing row");
            let updated = db
                .update_latest_routing_outcome_for_worker(
                    &task.id.to_string(),
                    "worker-routing-test",
                    "success",
                )
                .expect("update latest routing outcome");
            db.update_routing_outcome(
                &i64::MAX.to_string(),
                "private-missing-outcome-marker",
                5,
                8,
            )
            .expect("update absent task outcome");
            let history = db.get_routing_history(10).expect("read history");
            let success_rates = db.get_worker_success_rates().expect("read success rates");
            let affinity = db.get_affinity_stats().expect("read affinity stats");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_routing BEFORE INSERT ON routing_history
                     BEGIN SELECT RAISE(ABORT, 'private-routing-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_insert = db.insert_routing_history(&legacy_entry).is_err();
            (updated, history, success_rates, affinity, failed_insert)
        });

        assert!(observed.0);
        assert_eq!(observed.1.len(), 1);
        assert_eq!(observed.2.len(), 1);
        assert_eq!(observed.3.len(), 1);
        assert!(observed.4);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_routing_history_insert",
            "database_routing_history_claim_outcome_update",
            "database_routing_history_outcome_update",
            "database_routing_history_read",
            "database_routing_success_rates_read",
            "database_routing_affinity_stats_read",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing routing span {operation}"
            );
        }
        let missing_update = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_routing_history_outcome_update"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("missing task outcome update span");
        assert_eq!(missing_update["fields"]["result_count"], 0);
        assert_eq!(missing_update["fields"]["outcome"], "not_found");
        let serialized = serde_json::to_string(&events).expect("serialize routing telemetry");
        for private_value in [
            "private-routing-title-marker",
            "private-missing-outcome-marker",
            "private-routing-trigger-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }

    #[test]
    fn task_claim_and_status_emit_safe_result_events() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");
        let task = ready_task(&db, "observable task claim");

        let (claimed, events) = crate::observability::timing::capture_json_events(|| {
            db.claim_next_ready("worker-observability", 911)
        });
        assert_eq!(
            claimed.expect("claim task").expect("claimed task").id,
            task.id
        );
        let claim = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_claim_next_ready")
            .expect("claim event");
        assert_eq!(claim["fields"]["database"], "tasks");
        assert_eq!(claim["fields"]["worker"], "worker-observability");
        assert_eq!(claim["fields"]["task_id"], task.id.to_string());
        assert_eq!(claim["fields"]["outcome"], "success");
        assert_eq!(claim["fields"]["result_count"], 1);

        let (empty, events) = crate::observability::timing::capture_json_events(|| {
            db.claim_next_ready("worker-empty", 912)
        });
        assert!(empty.expect("empty queue is a normal result").is_none());
        let empty_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_claim_next_ready")
            .expect("empty-claim event");
        assert_eq!(empty_event["fields"]["outcome"], "empty");
        assert_eq!(empty_event["fields"]["result_count"], 0);

        let (updated, events) = crate::observability::timing::capture_json_events(|| {
            db.update_status_with_reason(task.id, "needs_info", Some("worker-observability"), None)
        });
        updated.expect("valid task status transition");
        let status = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("status event");
        assert_eq!(status["fields"]["database"], "tasks");
        assert_eq!(status["fields"]["task_id"], task.id.to_string());
        assert_eq!(status["fields"]["outcome"], "success");
        assert_eq!(status["fields"]["result_count"], 1);

        let (missing, events) = crate::observability::timing::capture_json_events(|| {
            db.update_status_with_reason(i64::MAX, "needs_info", None, None)
        });
        assert!(
            missing.is_err(),
            "missing task updates must not report success"
        );
        let failed = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("failed status event");
        assert_eq!(failed["fields"]["task_id"], i64::MAX.to_string());
        assert_eq!(failed["fields"]["outcome"], "error");

        let (stale, events) = crate::observability::timing::capture_json_events(|| {
            db.update_status_for_worker_claim(
                task.id,
                "worker-stale-owner",
                "failed",
                Some("worker-stale-owner"),
                None,
            )
        });
        assert!(stale.is_err(), "stale worker ownership must be rejected");
        let stale_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("stale-owner failure event");
        assert_eq!(stale_event["fields"]["worker"], "worker-stale-owner");
        assert_eq!(stale_event["fields"]["outcome"], "error");

        let (stale_route, events) = crate::observability::timing::capture_json_events(|| {
            db.update_status_with_routing_outcome(
                task.id,
                "in_progress",
                "worker-stale-route",
                "failed",
                Some("worker-stale-route"),
                None,
                "failure",
            )
        });
        assert!(
            stale_route.is_err(),
            "stale routing snapshot must be rejected"
        );
        let stale_route_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("stale-route failure event");
        assert_eq!(stale_route_event["fields"]["worker"], "worker-stale-route");
        assert_eq!(stale_route_event["fields"]["outcome"], "error");

        let worker_task = ready_task(&db, "worker-attributed status transition");
        db.update_status(worker_task.id, "assigned", Some("worker-owned"))
            .expect("assign worker-owned task");
        db.claim_task_with_pid(worker_task.id, "worker-owned", 913)
            .expect("claim worker-owned task");
        let (transition, events) = crate::observability::timing::capture_json_events(|| {
            db.update_status_for_worker_claim_and_run_id(
                worker_task.id,
                "worker-owned",
                "needs_info",
                Some("worker-owned"),
                None,
                Some("run-task-status"),
                None,
            )
        });
        transition.expect("worker-owned status transition");
        let worker_transition = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("worker transition event");
        assert_eq!(worker_transition["fields"]["worker"], "worker-owned");
        assert_eq!(worker_transition["fields"]["run_id"], "run-task-status");
        assert_eq!(
            worker_transition["fields"]["task_id"],
            worker_task.id.to_string()
        );
        assert_eq!(worker_transition["fields"]["outcome"], "success");
    }

    #[test]
    fn assignment_claim_and_recovery_emit_committed_result_events() {
        let mut connection = Connection::open_in_memory().expect("open isolated task database");
        crate::observability::sqlite::install_profile_for(&mut connection, "tasks.db");
        let db = TasksDb::from_connection(connection).expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");
        let task = ready_task(&db, "telemetry lifecycle task");
        let entry = crate::routing::TaskRouter::assignment_entry(&task, "worker-telemetry");

        let (assigned, events) = crate::observability::timing::capture_json_events(|| {
            db.assign_with_routing_history(task.id, "worker-telemetry", &entry)
        });
        assigned.expect("assign and persist routing history");
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_assign_with_routing")
            .expect("assignment result event");
        assert_eq!(event["fields"]["database"], "tasks");
        assert_eq!(event["fields"]["task_id"], task.id.to_string());
        assert_eq!(event["fields"]["worker"], "worker-telemetry");
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 1);
        let statement = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "database_statement")
            .expect("profiled production task SQL event");
        assert!(
            statement["spans"].as_array().is_some_and(|spans| {
                spans.iter().any(|span| {
                    span["task_id"] == task.id
                        && span["operation"] == "database_task_assign_with_routing"
                })
            }),
            "statement event did not carry task span: {statement}"
        );
        assert_eq!(statement["fields"]["database"], "tasks");
        assert!(!statement.to_string().contains("telemetry lifecycle task"));
        assert_eq!(
            db.get_by_id(task.id)
                .expect("read committed task")
                .expect("task exists")
                .status,
            TaskStatus::Assigned
        );

        let (claimed, events) = crate::observability::timing::capture_json_events(|| {
            db.claim_task_with_pid(task.id, "worker-telemetry", 991)
        });
        claimed.expect("claim assigned task");
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_claim_assigned")
            .expect("assigned claim event");
        assert_eq!(event["fields"]["task_id"], task.id.to_string());
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 1);
        let claimed_task = db.get_by_id(task.id).expect("read claim").unwrap();
        assert_eq!(claimed_task.status, TaskStatus::InProgress);
        assert_eq!(claimed_task.claimed_by_pid, Some(991));

        let (refused, events) = crate::observability::timing::capture_json_events(|| {
            db.claim_task_with_pid(task.id, "worker-wrong", 992)
        });
        assert!(refused.is_err());
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_claim_assigned")
            .expect("rejected claim event");
        assert_eq!(event["fields"]["outcome"], "error");
        assert_eq!(event["fields"]["worker"], "worker-wrong");

        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        let (held, events) = crate::observability::timing::capture_json_events(|| {
            db.hold_interrupted_claims_for_worker_run(
                "worker-telemetry",
                991,
                &host,
                "captured process interruption",
            )
        });
        assert_eq!(held.expect("quarantine interrupted run"), vec![task.id]);
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_recovery_quarantine")
            .expect("recovery result event");
        assert_eq!(event["fields"]["worker"], "worker-telemetry");
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 1);
        assert_eq!(
            db.get_by_id(task.id)
                .expect("read committed quarantine")
                .unwrap()
                .status,
            TaskStatus::NeedsInfo
        );

        let (unblocked, events) =
            crate::observability::timing::capture_json_events(|| db.unblock_ready_tasks(i64::MAX));
        assert!(unblocked
            .expect("no dependent tasks is a normal outcome")
            .is_empty());
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_dependency_reconcile")
            .expect("empty dependency reconciliation event");
        assert_eq!(event["fields"]["task_id"], i64::MAX.to_string());
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 0);
    }

    #[test]
    fn shared_task_status_api_rejects_qc_review_without_latest_attributable_passing_build() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");

        assert!(
            db.conn
                .execute(
                    "INSERT INTO tasks (title, status) VALUES ('direct-qc-insert', 'qc_review')",
                    [],
                )
                .is_err(),
            "no writer may create a task directly in qc_review"
        );

        let task = ready_task(&db, "task API requires build evidence");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 810).unwrap();
        let transition = || db.update_status(task.id, "qc_review", Some("worker-a"));
        assert!(
            transition().is_err(),
            "missing evidence must be rejected by the shared status API"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "direct SQL must be rejected by the SQLite evidence trigger"
        );

        db.insert_build_evidence(task.id, "session-1", "cargo test", 0, false, "cargo")
            .unwrap();
        assert!(
            transition().is_err(),
            "unattributable evidence must be rejected"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "direct SQL must reject unattributable evidence too"
        );

        db.insert_build_evidence(task.id, "session-2", "cargo test", 0, true, "cargo")
            .unwrap();
        move_to_qc_with_synthetic_workspace(&db, task.id, "worker-a")
            .expect("latest attributable passing evidence authorizes QC in selected workspace");

        let direct = ready_task(&db, "direct SQL still needs passing evidence");
        db.update_status(direct.id, "assigned", Some("worker-c"))
            .unwrap();
        db.claim_task_with_pid(direct.id, "worker-c", 812).unwrap();
        db.insert_build_evidence(direct.id, "session-5", "cargo test", 0, true, "cargo")
            .unwrap();
        assert!(db.conn
            .execute(
                "UPDATE tasks SET status='qc_review', claimed_by_pid=NULL, claim_host=NULL WHERE id=?1",
                params![direct.id],
            )
            .is_err(), "direct SQL without a selected-workspace context fails closed");

        let retry = ready_task(&db, "latest result controls QC admission");
        db.update_status(retry.id, "assigned", Some("worker-b"))
            .unwrap();
        db.claim_task_with_pid(retry.id, "worker-b", 811).unwrap();
        db.insert_build_evidence(retry.id, "session-3", "cargo test", 0, true, "cargo")
            .unwrap();
        db.insert_build_evidence(retry.id, "session-4", "cargo test", 1, true, "cargo")
            .unwrap();
        assert!(
            move_to_qc_with_synthetic_workspace(&db, retry.id, "worker-b").is_err(),
            "an older pass cannot mask a newer failure"
        );
    }

    #[test]
    fn project_build_batches_block_partial_and_interrupted_qc_until_complete_pass() {
        let path = sandbox("project-build-batch-completeness");
        let workspace = path.parent().unwrap().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create selected workspace");
        let workspace = workspace
            .canonicalize()
            .expect("canonical selected workspace");
        std::fs::write(workspace.join("Cargo.toml"), "[workspace]\nmembers = []\n")
            .expect("seed workspace fingerprint");
        let workspace_fingerprint = crate::platform::workspace_fingerprint::fingerprint(&workspace)
            .expect("fingerprint selected workspace");
        let workspace_path = workspace.to_string_lossy().into_owned();
        let db = TasksDb::open_at(&path).expect("open migrated task DB");
        let task = ready_task(&db, "batch completeness task");
        db.update_status(task.id, "assigned", Some("worker-batch"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-batch", 910)
            .expect("claim task");

        let partial = db
            .begin_project_build_batch_for_worker(
                "worker-batch",
                "session-partial",
                2,
                &workspace_path,
                Some(&workspace_fingerprint),
            )
            .expect("persist partial batch intent")
            .expect("claimed task resolves");
        let passing_sibling = vec![(
            "cargo build".into(),
            0,
            "cargo".into(),
            Some(workspace_path.clone()),
            Some(workspace_fingerprint.clone()),
        )];
        db.finish_project_build_batch_for_worker(
            partial.id,
            "worker-batch",
            "session-partial",
            &passing_sibling,
            None,
            false,
        )
        .expect("finalize batch with a missing child")
        .expect("batch remains attributable");
        assert!(
            move_to_qc_with_synthetic_workspace(&db, task.id, "worker-batch").is_err(),
            "a passing sibling cannot hide a missing or timed-out selected child"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "SQLite must reject direct SQL QC over an incomplete batch"
        );

        let interrupted = db
            .begin_project_build_batch_for_worker(
                "worker-batch",
                "session-interrupted",
                1,
                &workspace_path,
                Some(&workspace_fingerprint),
            )
            .expect("persist running batch before child start")
            .expect("claimed task resolves");
        drop(db);

        let reopened = TasksDb::open_at(&path).expect("reopen task database after interruption");
        let latest = reopened
            .latest_project_build_batch_status(task.id)
            .expect("read latest durable batch")
            .expect("running batch survives reopen");
        assert_eq!(latest.0, "running");
        assert_eq!(latest.1, interrupted.expected_count as i64);
        assert!(
            reopened
                .update_status_with_workspace_and_run_id(
                    task.id,
                    "qc_review",
                    Some("worker-batch"),
                    None,
                    None,
                    std::path::Path::new(&workspace_path),
                )
                .is_err(),
            "a process interruption must leave an ineligible running batch"
        );

        let complete = reopened
            .begin_project_build_batch_for_worker(
                "worker-batch",
                "session-complete",
                2,
                &workspace_path,
                Some(&workspace_fingerprint),
            )
            .expect("persist complete batch intent")
            .expect("claimed task resolves");
        let two_passes = vec![
            (
                "cargo build".into(),
                0,
                "cargo".into(),
                Some(workspace_path.clone()),
                Some(workspace_fingerprint.clone()),
            ),
            (
                "npm run build".into(),
                0,
                "node".into(),
                Some(workspace_path.clone()),
                Some(workspace_fingerprint),
            ),
        ];
        reopened
            .finish_project_build_batch_for_worker(
                complete.id,
                "worker-batch",
                "session-complete",
                &two_passes,
                None,
                true,
            )
            .expect("finalize complete passing batch")
            .expect("batch remains attributable");
        let other_workspace = path.parent().unwrap().join("other-workspace");
        std::fs::create_dir_all(&other_workspace).expect("create alternate selected workspace");
        assert!(
            reopened
                .update_status_with_workspace_and_run_id(
                    task.id,
                    "qc_review",
                    Some("worker-batch"),
                    None,
                    None,
                    &other_workspace,
                )
                .is_err(),
            "a passing batch for another directory cannot authorize QC"
        );
        reopened
            .update_status_with_workspace_and_run_id(
                task.id,
                "qc_review",
                Some("worker-batch"),
                None,
                None,
                std::path::Path::new(&workspace_path),
            )
            .expect("complete passing batch authorizes QC");
        drop(reopened);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn migration_18_preserves_legacy_build_rows_but_does_not_trust_unfingerprinted_passes() {
        let path = sandbox("qc-workspace-fingerprint-upgrade");
        let original_home = path.parent().unwrap().join("original-home");
        let _original_home = crate::db::test_support::with_home(&original_home);
        let db = TasksDb::open_at(&path).expect("open task database");
        let task = ready_task(&db, "legacy build row requires fresh evidence");
        db.update_status(task.id, "assigned", Some("worker-legacy"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-legacy", 813)
            .unwrap();
        db.insert_build_evidence(task.id, "legacy-session", "cargo test", 0, true, "cargo")
            .unwrap();
        db.conn
            .execute_batch(
                "DROP TRIGGER tasks_qc_review_evidence_guard;
                 DROP TRIGGER tasks_qc_review_insert_guard;
                 DROP TRIGGER build_retry_approval_revoke_on_task_change;
                 DROP TRIGGER build_retry_approval_insert_guard;
                 DROP TRIGGER build_retry_approval_update_guard;
                 DROP TRIGGER project_build_batch_retry_approval_guard;
                 DROP INDEX idx_build_retry_approval_one_available;
                 DROP INDEX idx_build_retry_approval_task_history;
                 DROP INDEX idx_project_build_batches_retry_approval;
                 DROP TABLE build_retry_approvals;
                 DROP INDEX idx_build_evidence_batch_id;
                 ALTER TABLE build_evidence DROP COLUMN batch_id;
                 DROP INDEX idx_project_build_batches_task_id;
                 ALTER TABLE project_build_batches DROP COLUMN retry_approval_id;
                 DROP TABLE project_build_batches;
                 ALTER TABLE worker_requests DROP COLUMN diagnostic_batch_id;
                 ALTER TABLE build_evidence DROP COLUMN workspace_fingerprint;
                 ALTER TABLE build_evidence DROP COLUMN workspace_path;
                 PRAGMA user_version = 17;",
            )
            .expect("prepare a schema-v17 database with an existing pass");
        drop(db);
        drop(_original_home);

        let upgrade_home = path.parent().unwrap().join("upgrade-home");
        std::fs::create_dir_all(&upgrade_home).expect("create isolated upgrade home");
        let upgrade_path = upgrade_home.join("tasks.db");
        std::fs::copy(&path, &upgrade_path).expect("copy schema-v17 fixture");
        let _upgrade_home = crate::db::test_support::with_home(&upgrade_home);

        let migrated = TasksDb::open_at(&upgrade_path).expect("apply additive migration 18");
        let preserved = migrated
            .latest_build_evidence(task.id)
            .unwrap()
            .expect("legacy evidence row survives");
        assert_eq!(preserved.exit_code, 0);
        assert!(preserved.workspace_path.is_none());
        assert!(preserved.workspace_fingerprint.is_none());
        assert!(
            migrated
                .conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "legacy passing evidence without workspace provenance must fail closed"
        );
        drop(migrated);
        drop(_upgrade_home);
        let _ = std::fs::remove_dir_all(path.parent().expect("fixture directory"));
    }

    #[test]
    fn observed_build_exits_advance_reset_and_expose_the_diagnostic_threshold() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");
        let task = ready_task(&db, "build failure streak");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 820).unwrap();

        for expected in 1..=3 {
            let recorded = db
                .record_build_attempts_for_worker(
                    "worker-a",
                    "session",
                    &[("cargo build".into(), 1, "cargo".into())],
                    None,
                )
                .unwrap()
                .unwrap();
            assert_eq!(recorded.0, task.id);
            assert_eq!(recorded.1, expected);
            assert_eq!(recorded.2, expected == 3);
        }

        let success = db
            .record_build_attempts_for_worker(
                "worker-a",
                "session",
                &[("cargo build".into(), 0, "cargo".into())],
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(success.1, 0, "an observed passing build resets the streak");
        assert!(!success.2);
        assert_eq!(db.get_retry_count(task.id).unwrap(), 0);

        let workspace = "C:/fixture/workspace";
        let fingerprint = "fixture-fingerprint";
        let concurrent_batch = db
            .begin_project_build_batch_for_worker(
                "worker-a",
                "session-concurrent",
                2,
                workspace,
                Some(fingerprint),
            )
            .unwrap()
            .expect("persist concurrent build batch before observed exits");
        let concurrent_failures = db
            .finish_project_build_batch_for_worker(
                concurrent_batch.id,
                "worker-a",
                "session-concurrent",
                &[
                    (
                        "cargo build -p frontend".into(),
                        1,
                        "cargo".into(),
                        Some(workspace.into()),
                        Some(fingerprint.into()),
                    ),
                    (
                        "cargo build -p backend".into(),
                        101,
                        "cargo".into(),
                        Some(workspace.into()),
                        Some(fingerprint.into()),
                    ),
                ],
                None,
                true,
            )
            .unwrap()
            .expect("persist both concurrent command exits");
        assert_eq!(concurrent_failures.1, 2);
        assert!(
            !concurrent_failures.2,
            "two failed exits in one concurrent batch are counted, but do not reach three"
        );

        let third_batch = db
            .begin_project_build_batch_for_worker(
                "worker-a",
                "session-third-failure",
                1,
                workspace,
                Some(fingerprint),
            )
            .unwrap()
            .expect("persist third failure batch");
        let third_failure = db
            .finish_project_build_batch_for_worker(
                third_batch.id,
                "worker-a",
                "session-third-failure",
                &[(
                    "cargo test".into(),
                    1,
                    "cargo".into(),
                    Some(workspace.into()),
                    Some(fingerprint.into()),
                )],
                None,
                true,
            )
            .unwrap()
            .expect("persist third observed exit");
        assert_eq!(third_failure.1, 3);
        assert!(third_failure.2, "the third nonzero exit requests diagnosis");

        let request = db
            .create_build_retry_request_for_worker(
                "worker-a",
                task.id,
                "{\"diagnosis\":\"fixed issue\"}",
            )
            .expect("worker submits diagnosis for Supervisor decision");
        db.approve_build_retry_request(
            request.id,
            "supervisor",
            "one focused verification batch is justified",
        )
        .expect("Supervisor grants one retry batch");
        let reset_batch = db
            .begin_project_build_batch_for_worker(
                "worker-a",
                "session-streak-reset",
                1,
                workspace,
                Some(fingerprint),
            )
            .unwrap()
            .expect("persist reset batch");
        let reset = db
            .finish_project_build_batch_for_worker(
                reset_batch.id,
                "worker-a",
                "session-streak-reset",
                &[(
                    "cargo test".into(),
                    0,
                    "cargo".into(),
                    Some(workspace.into()),
                    Some(fingerprint.into()),
                )],
                None,
                true,
            )
            .unwrap()
            .expect("persist passing exit that resets streak");
        assert_eq!(reset.1, 0);
        assert!(!reset.2);
        assert_eq!(db.get_retry_count(task.id).unwrap(), 0);

        let workspace_result = db
            .record_build_attempts_for_worker(
                "worker-a",
                "session",
                &[
                    ("cargo build -p frontend".into(), 1, "cargo".into()),
                    ("cargo build -p backend".into(), 0, "cargo".into()),
                ],
                Some(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(workspace_result.1, 0);
        assert_eq!(
            db.latest_build_evidence(task.id)
                .unwrap()
                .unwrap()
                .exit_code,
            1,
            "an aggregate failure remains the QC-visible latest result"
        );
    }

    #[test]
    fn supervisor_build_retry_grant_is_task_bound_audited_and_consumed_once() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");
        let task = ready_task(&db, "one supervised build retry");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 821).unwrap();
        for attempt in 0..3 {
            let session_id = format!("diagnostic-failure-{attempt}");
            let batch = db
                .begin_project_build_batch_for_worker(
                    "worker-a",
                    &session_id,
                    1,
                    "C:/fixture/workspace",
                    Some("fingerprint"),
                )
                .unwrap()
                .expect("persist diagnostic build attempt");
            let result = db
                .finish_project_build_batch_for_worker(
                    batch.id,
                    "worker-a",
                    &session_id,
                    &[(
                        "cargo test".into(),
                        1,
                        "cargo".into(),
                        Some("C:/fixture/workspace".into()),
                        Some("fingerprint".into()),
                    )],
                    None,
                    true,
                )
                .unwrap()
                .expect("record observed failed command");
            assert_eq!(result.1, attempt + 1);
        }

        assert!(
            db.begin_project_build_batch_for_worker(
                "worker-a",
                "session-denied",
                1,
                "C:/fixture/workspace",
                Some("fingerprint"),
            )
            .is_err(),
            "threshold build is denied without Supervisor approval"
        );
        assert!(
            db.conn
                .execute(
                    "INSERT INTO project_build_batches
                     (task_id, session_id, worker_slug, expected_count, outcome, workspace_path)
                     VALUES (?1, 'direct-sql', 'worker-a', 1, 'running', 'C:/fixture/workspace')",
                    params![task.id],
                )
                .is_err(),
            "SQLite blocks direct unapproved batch creation at the threshold"
        );

        let request = db
            .create_build_retry_request_for_worker(
                "worker-a",
                task.id,
                "{\"diagnosis\":\"bounded fix\"}",
            )
            .expect("claimed worker can submit a typed diagnosis request");
        let diagnostic_batch_id: i64 = db
            .conn
            .query_row(
                "SELECT diagnostic_batch_id FROM worker_requests WHERE id = ?1",
                params![request.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            db.conn
                .execute(
                    "INSERT INTO build_retry_approvals
                     (request_id, task_id, worker_slug, diagnostic_batch_id, approved_by, reason, status)
                     VALUES (?1, ?2, 'worker-a', ?3, 'supervisor', 'forged SQL approval', 'available')",
                    params![request.id, task.id, diagnostic_batch_id],
                )
                .is_err(),
            "SQLite rejects a forged Supervisor identity without the host authorization context"
        );
        assert!(
            db.approve_build_retry_request(request.id, "worker-a", "self approval")
                .is_err(),
            "workers cannot grant their own retry permission"
        );
        db.approve_build_retry_request(
            request.id,
            "supervisor",
            "the diagnosis supports one focused build batch",
        )
        .expect("Supervisor grants exactly one batch");

        let batch = db
            .begin_project_build_batch_for_worker(
                "worker-a",
                "session-approved",
                1,
                "C:/fixture/workspace",
                Some("fingerprint"),
            )
            .unwrap()
            .expect("approved task remains attributable");
        assert!(batch.supervisor_retry_approved);
        let linked_approval: i64 = db
            .conn
            .query_row(
                "SELECT retry_approval_id FROM project_build_batches WHERE id = ?1",
                params![batch.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            linked_approval > 0,
            "batch durably links its consumed grant"
        );
        assert!(
            db.begin_project_build_batch_for_worker(
                "worker-a",
                "session-reuse-denied",
                1,
                "C:/fixture/workspace",
                Some("fingerprint"),
            )
            .is_err(),
            "one grant cannot authorize a second batch"
        );

        let audit: (String, String, String, String) = db
            .conn
            .query_row(
                "SELECT worker_slug, approved_by, reason, status
                 FROM build_retry_approvals WHERE request_id = ?1",
                params![request.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            audit,
            (
                "worker-a".into(),
                "supervisor".into(),
                "the diagnosis supports one focused build batch".into(),
                "consumed".into(),
            )
        );
    }

    #[test]
    fn p4_task_health_and_routing_outcome_readers_use_populated_plans_and_results() {
        use rusqlite::types::Value;

        fn explain(db: &TasksDb, sql: &str, bindings: &[Value]) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare production query plan");
            statement
                .query_map(rusqlite::params_from_iter(bindings.iter()), |row| {
                    row.get::<_, String>(3)
                })
                .expect("explain production query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect production query plan")
        }

        fn assert_task_primary_key_plan(details: &[String]) {
            assert!(
                details.iter().any(|detail| {
                    detail.contains("INTEGER PRIMARY KEY") || detail.contains("PRIMARY KEY")
                }),
                "task point read should use the existing primary key; plan: {details:?}"
            );
        }

        fn assert_routing_task_and_rowid_plan(details: &[String]) {
            assert!(
                details
                    .iter()
                    .any(|detail| detail.contains("idx_routing_history_task")),
                "routing lookup should use the existing task_id index; plan: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
                "routing lookup should use the index's rowid order for newest-first selection; plan: {details:?}"
            );
        }

        fn assert_embedded_worker_routing_plan(details: &[String]) {
            assert!(
                details.iter().any(|detail| {
                    detail.contains("idx_routing_history_task")
                        || detail.contains("idx_routing_history_worker_type")
                }),
                "embedded worker routing lookup should use an existing routing index; plan: {details:?}"
            );
            if details
                .iter()
                .any(|detail| detail.contains("idx_routing_history_worker_type"))
            {
                assert!(
                    details
                        .iter()
                        .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
                    "worker_slug index plan should document its rowid sort; plan: {details:?}"
                );
            } else {
                assert!(
                    !details
                        .iter()
                        .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
                    "task_id index should provide rowid order; plan: {details:?}"
                );
            }
        }

        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");

        // Populate both indexed tables enough that the planner has real
        // alternatives. Routing reads keep the shipped task_id index and use
        // its implicit rowid ordering; worker_slug/outcome remain residual
        // filters because the current schema has no narrower index.
        for index in 0..128 {
            let task = ready_task(&db, &format!("reader plan fixture {index}"));
            let entry = crate::routing::TaskRouter::assignment_entry(
                &task,
                if index % 8 == 0 {
                    "worker-plan-target"
                } else {
                    "worker-plan-other"
                },
            );
            db.insert_routing_history(&entry)
                .expect("populate routing history fixture");
        }

        let build_task = ready_task(&db, "retry count reader result");
        db.update_status(build_task.id, "assigned", Some("worker-build"))
            .unwrap();
        db.claim_task_with_pid(build_task.id, "worker-build", 9101)
            .unwrap();
        db.conn
            .execute(
                "UPDATE tasks SET retry_count = 2 WHERE id = ?1",
                params![build_task.id],
            )
            .expect("seed retry count");

        let routed_done = ready_task(&db, "routing outcome status transition");
        let done_entry = crate::routing::TaskRouter::assignment_entry(&routed_done, "worker-done");
        db.assign_with_routing_history(routed_done.id, "worker-done", &done_entry)
            .unwrap();
        db.claim_task_with_pid(routed_done.id, "worker-done", 9102)
            .unwrap();

        let worker_claim = ready_task(&db, "worker claim routing subquery");
        let older_claim_entry =
            crate::routing::TaskRouter::assignment_entry(&worker_claim, "worker-claim");
        db.insert_routing_history(&older_claim_entry)
            .expect("seed an older pending worker claim outcome");
        let claim_entry =
            crate::routing::TaskRouter::assignment_entry(&worker_claim, "worker-claim");
        db.assign_with_routing_history(worker_claim.id, "worker-claim", &claim_entry)
            .unwrap();
        db.claim_task_with_pid(worker_claim.id, "worker-claim", 9103)
            .unwrap();

        let worker_reader = ready_task(&db, "latest worker routing reader");
        let worker_entry =
            crate::routing::TaskRouter::assignment_entry(&worker_reader, "worker-latest");
        db.insert_routing_history(&worker_entry).unwrap();

        let task_reader = ready_task(&db, "latest task routing reader");
        let older_entry = crate::routing::TaskRouter::assignment_entry(&task_reader, "worker-old");
        let latest_entry = crate::routing::TaskRouter::assignment_entry(&task_reader, "worker-new");
        db.insert_routing_history(&older_entry).unwrap();
        db.insert_routing_history(&latest_entry).unwrap();
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze populated fixture");

        let retry_plan = explain(
            &db,
            TASK_RETRY_COUNT_BY_ID_SQL,
            &[Value::from(build_task.id)],
        );
        assert_task_primary_key_plan(&retry_plan);
        let owner_plan = explain(
            &db,
            TASK_STATUS_AND_OWNER_BY_ID_SQL,
            &[Value::from(routed_done.id)],
        );
        assert_task_primary_key_plan(&owner_plan);

        let routed_done_id = routed_done.id.to_string();
        let worker_routing_plan = explain(
            &db,
            ROUTING_LATEST_PENDING_FOR_WORKER_SQL,
            &[
                Value::from(routed_done_id.clone()),
                Value::from(String::from("worker-done")),
            ],
        );
        assert_routing_task_and_rowid_plan(&worker_routing_plan);

        let embedded_worker_plan = explain(
            &db,
            ROUTING_OUTCOME_FOR_WORKER_CLAIM_SQL,
            &[
                Value::from(String::from("failure")),
                Value::from(worker_claim.id.to_string()),
                Value::from(String::from("worker-claim")),
            ],
        );
        // SQLite currently prefers the existing worker_slug index for this
        // embedded shape, then sorts for newest rowid; retain that measured
        // behavior as a documented baseline rather than adding an index.
        assert_embedded_worker_routing_plan(&embedded_worker_plan);

        let embedded_task_plan = explain(
            &db,
            ROUTING_OUTCOME_FOR_TASK_SQL,
            &[
                Value::from(String::from("success")),
                Value::from(15_i64),
                Value::from(20_i64),
                Value::from(task_reader.id.to_string()),
            ],
        );
        assert_routing_task_and_rowid_plan(&embedded_task_plan);

        let build_result = db
            .record_build_attempts_for_worker(
                "worker-build",
                "reader-plan-session",
                &[("cargo check".into(), 1, "cargo".into())],
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(build_result, (build_task.id, 3, true));
        assert_eq!(db.get_retry_count(build_task.id).unwrap(), 3);

        db.update_status_with_routing_outcome(
            routed_done.id,
            "in_progress",
            "worker-done",
            "done",
            Some("worker-done"),
            None,
            "success",
        )
        .expect("production status and routing reader transition");
        assert_eq!(
            db.get_by_id(routed_done.id).unwrap().unwrap().status,
            TaskStatus::Done
        );
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT outcome FROM routing_history WHERE id = ?1",
                    params![done_entry.id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap()
                .as_deref(),
            Some("success")
        );

        db.update_status_for_worker_claim(
            worker_claim.id,
            "worker-claim",
            "failed",
            Some("worker-claim"),
            None,
        )
        .expect("production worker claim reader transition");
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT outcome FROM routing_history WHERE id = ?1",
                    params![claim_entry.id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap()
                .as_deref(),
            Some("failure")
        );
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT outcome FROM routing_history WHERE id = ?1",
                    params![older_claim_entry.id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap(),
            None,
            "worker completion must leave the older pending outcome untouched"
        );

        assert!(db
            .update_latest_routing_outcome_for_worker(
                &worker_reader.id.to_string(),
                "worker-latest",
                "success",
            )
            .unwrap());
        assert_eq!(
            db.conn
                .query_row(
                    "SELECT outcome FROM routing_history WHERE id = ?1",
                    params![worker_entry.id],
                    |row| row.get::<_, Option<String>>(0),
                )
                .unwrap()
                .as_deref(),
            Some("success")
        );

        db.update_routing_outcome(&task_reader.id.to_string(), "success", 15, 20)
            .expect("production latest task routing reader");
        let outcomes: Vec<(String, Option<String>)> = db
            .conn
            .prepare("SELECT id, outcome FROM routing_history WHERE task_id = ?1 ORDER BY rowid")
            .unwrap()
            .query_map(params![task_reader.id.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(
            outcomes,
            vec![
                (older_entry.id, None),
                (latest_entry.id, Some("success".into()))
            ]
        );
    }

    fn sandbox(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("arcana-claim-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated test directory");
        dir.join("tasks.db")
    }

    #[test]
    fn worker_claim_lookup_uses_an_index_with_populated_tasks() {
        let path = sandbox("query-plan-worker-claim");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        for index in 0..128 {
            let task = ready_task(&db, &format!("query plan task {index}"));
            let worker = if index % 8 == 0 {
                "worker-target"
            } else {
                "worker-other"
            };
            db.update_status(task.id, "assigned", Some(worker))
                .expect("assign task");
            if index == 0 {
                db.claim_task_with_pid(task.id, worker, 900 + index as u32)
                    .expect("claim assigned task");
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let details = {
            let explain_sql = format!("EXPLAIN QUERY PLAN {CLAIMED_TASK_IDS_FOR_WORKER_SQL}");
            let mut statement = db.conn.prepare(&explain_sql).expect("prepare query plan");
            statement
                .query_map(["worker-target"], |row| row.get::<_, String>(3))
                .expect("explain query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        };

        assert!(
            details.iter().any(|detail| {
                detail.contains("idx_tasks_assigned") || detail.contains("idx_tasks_status")
            }),
            "worker claim lookup should use an applicable worker/status index; plan: {details:?}"
        );
        assert!(
            !details.iter().any(|detail| detail.contains("SCAN tasks")),
            "worker claim lookup scanned the task table; plan: {details:?}"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn daily_budget_summary_uses_measured_date_index_and_preserves_utc_day_totals() {
        fn explain(db: &TasksDb) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!(
                    "EXPLAIN QUERY PLAN {TASKS_DAILY_BUDGET_SUMMARY_SQL}"
                ))
                .expect("prepare daily budget plan");
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("explain daily budget query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect daily budget plan")
        }

        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let today_utc: String = db
            .conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |row| {
                row.get(0)
            })
            .expect("read fixture UTC timestamp");
        for index in 0..2_000 {
            let task = ready_task(&db, &format!("daily budget plan task {index}"));
            let is_today = index % 40 == 0;
            db.conn
                .execute(
                    "UPDATE tasks SET created_at = ?1,
                        budget_max_tokens = ?2, budget_tokens_used = ?3,
                        budget_max_usd = ?4, budget_cost_usd = ?5,
                        budget_exceeded = ?6
                     WHERE id = ?7",
                    params![
                        if is_today {
                            &today_utc
                        } else {
                            "2020-01-01T00:00:00.000Z"
                        },
                        if is_today { Some(100_i64) } else { None },
                        if is_today { 10_i64 } else { 0_i64 },
                        if is_today { Some(1.25_f64) } else { None },
                        if is_today { 0.25_f64 } else { 0.0_f64 },
                        if is_today && index % 80 == 0 {
                            1_i64
                        } else {
                            0_i64
                        },
                        task.id
                    ],
                )
                .expect("set representative task date and budget");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");
        db.conn
            .execute_batch("DROP INDEX idx_tasks_created_date; ANALYZE")
            .expect("remove candidate for baseline plan");
        let baseline = explain(&db);
        assert!(
            baseline.iter().any(|detail| detail.contains("SCAN tasks")),
            "daily budget baseline should scan the populated task table: {baseline:?}"
        );

        db.conn
            .execute_batch(
                "CREATE INDEX idx_tasks_created_date ON tasks(date(created_at)); ANALYZE",
            )
            .expect("create measured date index");
        let indexed = explain(&db);
        assert!(
            indexed
                .iter()
                .any(|detail| detail.contains("idx_tasks_created_date")),
            "daily budget query should seek the measured date index: {indexed:?}"
        );
        assert!(
            !indexed.iter().any(|detail| detail.contains("SCAN tasks")),
            "daily budget query still scans task rows: {indexed:?}"
        );

        let summary = db
            .get_daily_budget_summary()
            .expect("read production daily budget summary");
        assert_eq!(summary.total_allocated_tokens, 5_000);
        assert_eq!(summary.total_spent_tokens, 500);
        assert_eq!(summary.total_allocated_usd, 62.5);
        assert_eq!(summary.total_spent_usd, 12.5);
        assert_eq!(summary.tasks_exceeded, 25);
        assert_eq!(summary.tasks_with_budget, 50);
    }

    #[test]
    fn qc_claim_reconciliation_uses_partial_index_with_populated_tasks() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let mut expected = Vec::new();
        for index in 0..2_000 {
            let task = ready_task(&db, &format!("QC claim plan task {index}"));
            if index % 100 == 0 {
                let reviewer = format!("reviewer-{index}");
                assert!(db.try_claim_for_qc_review(task.id, &reviewer).unwrap());
                expected.push((task.id, reviewer));
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");
        db.conn
            .execute_batch("DROP INDEX idx_tasks_qc_claims; ANALYZE")
            .expect("remove candidate for baseline plan");
        let explain = |db: &TasksDb| {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {QC_CLAIMS_SQL}"))
                .expect("prepare QC claim plan");
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("explain QC claim query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect QC claim plan")
        };
        let baseline = explain(&db);
        assert!(
            baseline.iter().any(|detail| detail.contains("SCAN tasks")),
            "QC claim baseline should scan populated task rows: {baseline:?}"
        );

        db.conn
            .execute_batch(
                "CREATE INDEX idx_tasks_qc_claims ON tasks(under_review_by, id) WHERE under_review_by IS NOT NULL; ANALYZE",
            )
            .expect("create measured partial QC claim index");
        let indexed = explain(&db);
        assert!(
            indexed
                .iter()
                .any(|detail| detail.contains("idx_tasks_qc_claims")),
            "QC claim reader should use its partial index: {indexed:?}"
        );
        assert!(
            !indexed.iter().any(|detail| detail.contains("SCAN tasks")),
            "QC claim reader still scans tasks: {indexed:?}"
        );
        let mut actual = db.list_qc_claims().unwrap();
        actual.sort_unstable_by_key(|(task_id, _)| *task_id);
        expected.sort_unstable_by_key(|(task_id, _)| *task_id);
        assert_eq!(actual, expected);
    }

    #[test]
    fn task_budget_and_unblock_readers_use_existing_indexes_with_populated_tasks() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply task migrations");
        let migrated_blocked_index: bool = db
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_tasks_blocked_tasks_plan')",
                [],
                |row| row.get(0),
            )
            .expect("read migrated blocker index");
        assert!(
            migrated_blocked_index,
            "blocked-task index migration must run"
        );
        // Drop the migrated index only inside this isolated test so the same
        // populated fixture can compare baseline and candidate plans.
        db.conn
            .execute_batch("DROP INDEX idx_tasks_blocked_tasks_plan")
            .expect("remove blocker index for baseline measurement");
        let mut target_id = None;
        let mut child_id = None;
        let mut done_blocker_id = None;
        let mut dependency_fixture_ids = Vec::new();
        let mut sparse_blocked_ids = Vec::new();
        for index in 0..5_000 {
            let task = ready_task(&db, &format!("unblock plan task {index}"));
            if index == 0 {
                target_id = Some(task.id);
            }
            if index == 1 {
                child_id = Some(task.id);
            }
            if index == 20 {
                done_blocker_id = Some(task.id);
            }
            if (2..=7).contains(&index) {
                dependency_fixture_ids.push(task.id);
            }
            if index > 1 && index % 40 == 0 {
                db.conn
                    .execute(
                        "UPDATE tasks SET status = 'needs_info', blocked_by = ?1 WHERE id = ?2",
                        params![format!("[\"{}\"]", 1_999_999), task.id],
                    )
                    .expect("set sparse blocked task");
                sparse_blocked_ids.push(task.id);
            }
        }
        let target_id = target_id.expect("target task");
        let child_id = child_id.expect("child task");
        let done_blocker_id = done_blocker_id.expect("done blocker outside epic fixture");
        let [_unused_dependency_fixture, incomplete_blocker_id, done_only_child_id, incomplete_child_id, mixed_child_id, empty_blockers_id]: [i64; 6] = dependency_fixture_ids
            .try_into()
            .expect("six representative dependency tasks");
        db.update_status(done_blocker_id, "assigned", Some("dependency-fixture"))
            .unwrap();
        db.claim_task_with_pid(done_blocker_id, "dependency-fixture", 9002)
            .unwrap();
        db.insert_build_evidence(
            done_blocker_id,
            "dependency-fixture",
            "cargo test",
            0,
            true,
            "cargo",
        )
        .unwrap();
        move_to_qc_with_synthetic_workspace(&db, done_blocker_id, "dependency-fixture").unwrap();
        db.update_status(done_blocker_id, "done", Some("dependency-fixture"))
            .unwrap();
        for (task_id, blockers) in [
            (done_only_child_id, vec![done_blocker_id.to_string()]),
            (incomplete_child_id, vec![incomplete_blocker_id.to_string()]),
            (
                mixed_child_id,
                vec![done_blocker_id.to_string(), "1999999".to_string()],
            ),
            (empty_blockers_id, Vec::new()),
        ] {
            db.conn
                .execute(
                    "UPDATE tasks SET status = 'needs_info', blocked_by = ?1 WHERE id = ?2",
                    params![serde_json::to_string(&blockers).unwrap(), task_id],
                )
                .expect("set representative dependency state");
        }
        db.conn
            .execute(
                "UPDATE tasks SET budget_tokens_used = 321, budget_cost_usd = 1.25,
                    budget_turns_used = 4 WHERE id = ?1",
                params![target_id],
            )
            .expect("set target budget");
        db.conn
            .execute(
                "UPDATE tasks SET epic_id = '700' WHERE id BETWEEN ?1 AND ?2",
                params![target_id, target_id + 19],
            )
            .expect("set representative epic members");
        db.conn
            .execute(
                "UPDATE tasks SET status = 'needs_info', blocked_by = ?1 WHERE id = ?2",
                params![format!("[\"{}\"]", target_id), child_id],
            )
            .expect("set target-dependent task");
        db.conn.execute_batch("ANALYZE").unwrap();

        let explain = |sql: &str, with_parameter: bool| {
            let statement = format!("EXPLAIN QUERY PLAN {sql}");
            let mut stmt = db.conn.prepare(&statement).unwrap();
            if with_parameter {
                stmt.query_map([target_id], |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .unwrap()
            } else {
                stmt.query_map([], |row| row.get::<_, String>(3))
                    .unwrap()
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .unwrap()
            }
        };
        let blocked_baseline_plan = explain(GET_BLOCKED_TASKS_SQL, false);
        // The candidate SQL returns complete rows and blocker membership lives in JSON text.
        assert!(
            blocked_baseline_plan
                .iter()
                .any(|detail| detail.contains("SCAN tasks")),
            "blocked-task baseline should scan the populated task table: {blocked_baseline_plan:?}"
        );
        let total_task_count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        let eligible_blocked_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE blocked_by IS NOT NULL AND blocked_by != '[]'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(total_task_count >= 5_000);
        let benchmark_median_ns = || {
            for _ in 0..5 {
                std::hint::black_box(db.get_blocked_tasks().unwrap());
            }
            let mut samples = Vec::with_capacity(31);
            for _ in 0..31 {
                let started = std::time::Instant::now();
                std::hint::black_box(db.get_blocked_tasks().unwrap());
                samples.push(started.elapsed().as_nanos());
            }
            samples.sort_unstable();
            samples[samples.len() / 2]
        };
        let mut blocked_baseline = db
            .get_blocked_tasks()
            .unwrap()
            .into_iter()
            .map(|task| (task.id, task.status, task.blocked_by))
            .collect::<Vec<_>>();
        blocked_baseline.sort_by_key(|(task_id, _, _)| *task_id);
        let baseline_median_ns = benchmark_median_ns();
        db.conn
            .execute_batch(
                "CREATE INDEX idx_tasks_blocked_tasks_plan ON tasks(id)
                 WHERE blocked_by IS NOT NULL AND blocked_by != '[]'; ANALYZE",
            )
            .expect("create measured partial blocked-task index");
        let blocked_candidate_plan = explain(GET_BLOCKED_TASKS_SQL, false);
        assert!(
            blocked_candidate_plan
                .iter()
                .any(|detail| { detail.contains("idx_tasks_blocked_tasks_plan") }),
            "blocked-task query should use the measured partial index: {blocked_candidate_plan:?}"
        );
        assert!(
            !blocked_candidate_plan
                .iter()
                .any(|detail| detail == "SCAN tasks"),
            "blocked-task query should avoid scanning the task table: {blocked_candidate_plan:?}"
        );
        let candidate_median_ns = benchmark_median_ns();
        eprintln!(
            "get_blocked_tasks measurement: total_tasks={total_task_count}, eligible_blocked_rows={eligible_blocked_count}, baseline_median_ns={baseline_median_ns}, candidate_median_ns={candidate_median_ns}, baseline_plan={blocked_baseline_plan:?}, candidate_plan={blocked_candidate_plan:?}"
        );
        let budget_plan = explain(TASK_BUDGET_STATUS_SQL, true);
        let status_plan = explain(TASK_STATUS_BY_ID_SQL, true);
        let epic_id_plan = explain(TASK_EPIC_ID_BY_ID_SQL, true);
        for (name, plan) in [
            ("task budget", budget_plan),
            ("blocker status", status_plan),
            ("completed-task epic", epic_id_plan),
        ] {
            assert!(
                plan.iter()
                    .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
                "{name} point reader must use the primary key: {plan:?}"
            );
        }
        let epic_counts_plan = explain(EPIC_COMPLETION_COUNTS_SQL, true);
        assert!(
            epic_counts_plan
                .iter()
                .any(|detail| detail.contains("idx_tasks_epic_phase_order")),
            "epic completion aggregate should use the measured epic index: {epic_counts_plan:?}"
        );
        let epic_counts: (i64, i64) = db
            .conn
            .query_row(EPIC_COMPLETION_COUNTS_SQL, ["700"], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!(epic_counts, (20, 20));

        let mut blocked_after_index = db
            .get_blocked_tasks()
            .unwrap()
            .into_iter()
            .map(|task| (task.id, task.status, task.blocked_by))
            .collect::<Vec<_>>();
        blocked_after_index.sort_by_key(|(task_id, _, _)| *task_id);
        assert_eq!(blocked_after_index, blocked_baseline);
        let mut blocked_ids = blocked_after_index
            .iter()
            .map(|(task_id, _, _)| *task_id)
            .collect::<Vec<_>>();
        let mut expected_blocked_ids = sparse_blocked_ids;
        expected_blocked_ids.extend([child_id, incomplete_child_id, mixed_child_id]);
        blocked_ids.sort_unstable();
        expected_blocked_ids.sort_unstable();
        assert_eq!(blocked_ids, expected_blocked_ids);

        let baseline = explain(BLOCKED_TASKS_FOR_UNBLOCK_SQL, false);
        assert!(
            baseline
                .iter()
                .any(|detail| detail.contains("idx_tasks_status")),
            "unblock baseline should use the existing status index: {baseline:?}"
        );

        let budget = db.get_task_budget_status(target_id).unwrap();
        assert_eq!((budget.tokens_used, budget.turns_used), (321, 4));
        assert_eq!(budget.cost_usd, 1.25);
        db.update_status(target_id, "assigned", Some("unblock-fixture"))
            .unwrap();
        db.claim_task_with_pid(target_id, "unblock-fixture", 9001)
            .unwrap();
        db.insert_build_evidence(target_id, "unblock-fixture", "cargo test", 0, true, "cargo")
            .unwrap();
        move_to_qc_with_synthetic_workspace(&db, target_id, "unblock-fixture").unwrap();
        db.update_status(target_id, "done", Some("unblock-fixture"))
            .unwrap();
        let unblocked = db.unblock_ready_tasks(target_id).unwrap();
        assert_eq!(unblocked, vec![child_id]);
        assert_eq!(
            db.get_by_id(child_id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
    }

    #[test]
    fn orphan_candidate_scan_uses_status_index_with_populated_tasks() {
        let path = sandbox("query-plan-orphan-candidates");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        for index in 0..128 {
            let task = ready_task(&db, &format!("orphan candidate {index}"));
            if index % 4 == 0 {
                db.update_status(task.id, "assigned", Some(&format!("worker-{index}")))
                    .expect("assign task");
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let explain_sql = format!("EXPLAIN QUERY PLAN {ORPHAN_CANDIDATES_SQL}");
        let details = {
            let mut statement = db
                .conn
                .prepare(&explain_sql)
                .expect("prepare orphan candidate query plan");
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("explain production orphan candidate query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        };

        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_tasks_status")),
            "orphan candidate lookup should use the status index; plan: {details:?}"
        );
        assert!(
            !details.iter().any(|detail| detail.contains("SCAN tasks")),
            "orphan candidate lookup scanned the task table; plan: {details:?}"
        );
        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn latest_build_evidence_and_qc_guard_use_task_index_with_populated_rows() {
        let path = sandbox("query-plan-build-evidence");
        let db = TasksDb::open_at(&path).expect("open isolated task database");
        let task = ready_task(&db, "build evidence query-plan task");
        for index in 0..512 {
            db.insert_build_evidence(
                task.id,
                "session-build-plan",
                &format!("cargo test run {index}"),
                0,
                true,
                "cargo",
            )
            .expect("record build evidence");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        for (sql, expected_index) in [(
            LATEST_BUILD_EVIDENCE_FOR_TASK_SQL,
            "idx_build_evidence_task_id",
        )] {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare build evidence plan");
            let details = statement
                .query_map([task.id], |row| row.get::<_, String>(3))
                .expect("explain production build evidence query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan");
            assert!(
                details
                    .iter()
                    .any(|detail| detail.contains(expected_index)),
                "latest task build evidence should use its task/id index ({expected_index}); plan: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.contains("SCAN build_evidence")),
                "latest task build evidence scanned the table; plan: {details:?}"
            );
        }

        let latest = db
            .latest_build_evidence(task.id)
            .expect("read latest evidence")
            .expect("latest evidence exists");
        assert_eq!(latest.command, "cargo test run 511");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .expect("assign task before QC");
        db.claim_task_with_pid(task.id, "worker-a", std::process::id() as u32)
            .expect("claim task before QC");
        move_to_qc_with_synthetic_workspace(&db, task.id, "worker-a")
            .expect("QC guard reads the passing latest evidence");
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            TaskStatus::QcReview
        );

        // Populate real batch rows after the QC transition so the latest-batch
        // plan is measured against the production table and its task/id index.
        for index in 0..128 {
            db.conn
                .execute(
                    "INSERT INTO project_build_batches
                     (task_id, session_id, worker_slug, expected_count, observed_count,
                      passing_count, outcome, workspace_path, completed_at)
                     VALUES (?1, ?2, 'worker-plan', 1, 1, 0, 'failed', '/fixture', 'done')",
                    params![task.id, format!("historical-batch-{index}")],
                )
                .expect("populate historical build batch rows");
        }
        db.conn
            .execute(
                "INSERT INTO project_build_batches
                 (task_id, session_id, worker_slug, expected_count, observed_count,
                  passing_count, outcome, workspace_path, completed_at)
                 VALUES (?1, 'latest-batch', 'worker-plan', 512, 512, 512,
                         'passed', '/fixture', 'done')",
                params![task.id],
            )
            .expect("populate latest passing batch row");
        let latest_batch_id: i64 = db
            .conn
            .query_row("SELECT MAX(id) FROM project_build_batches", [], |row| {
                row.get(0)
            })
            .expect("read latest fixture batch id");
        db.conn
            .execute(
                "UPDATE build_evidence SET batch_id = ?1 WHERE task_id = ?2",
                params![latest_batch_id, task.id],
            )
            .expect("link populated evidence to latest batch");
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze batch fixture");

        let batch_plan = {
            let mut statement = db
                .conn
                .prepare(&format!(
                    "EXPLAIN QUERY PLAN {LATEST_BUILD_BATCH_FOR_QC_SQL}"
                ))
                .expect("prepare populated latest-batch plan");
            statement
                .query_map([task.id], |row| row.get::<_, String>(3))
                .expect("explain populated latest-batch query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect latest-batch plan")
        };
        assert!(
            batch_plan
                .iter()
                .any(|detail| detail.contains("idx_project_build_batches_task_id")),
            "latest project batch should use its task/id index; plan: {batch_plan:?}"
        );
        assert!(
            !batch_plan
                .iter()
                .any(|detail| detail.contains("SCAN project_build_batches")),
            "latest project batch scanned the batch table; plan: {batch_plan:?}"
        );
        let latest_batch = db
            .latest_project_build_batch_status(task.id)
            .expect("read latest project build batch")
            .expect("latest batch exists");
        assert_eq!((latest_batch.0.as_str(), latest_batch.1), ("passed", 512));
        let latest_batch_evidence_count: i64 = db
            .conn
            .query_row(LATEST_BUILD_BATCH_FOR_QC_SQL, params![task.id], |row| {
                row.get(6)
            })
            .expect("read evidence count for latest populated batch");
        assert_eq!(latest_batch_evidence_count, 512);
        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn build_batch_query_plans_cover_populated_retry_and_trigger_reads() {
        fn explain<P: rusqlite::Params>(db: &TasksDb, sql: &str, params: P) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare production-shaped query plan");
            statement
                .query_map(params, |row| row.get::<_, String>(3))
                .expect("explain populated production query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect production query plan")
        }

        fn assert_index(plan: &[String], index: &str, query: &str) {
            assert!(
                plan.iter().any(|detail| detail.contains(index)),
                "{query} should use {index}; plan: {plan:?}"
            );
        }

        fn assert_no_table_scan(plan: &[String], table: &str, query: &str) {
            assert!(
                !plan.iter().any(|detail| {
                    detail.contains(&format!("SCAN {table}")) && !detail.contains("USING INDEX")
                }),
                "{query} should not scan {table}; plan: {plan:?}"
            );
        }

        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register SQLite functions");
        db.migrate().expect("apply task migrations");

        // Keep many eligible tasks, failed diagnoses, pending requests, and
        // available grants in the fixture so each exact production predicate
        // is planned against populated indexed tables.
        let mut fixtures = Vec::new();
        for index in 0..48 {
            let task = ready_task(&db, &format!("build retry plan fixture {index}"));
            let worker = format!("worker-plan-{index}");
            db.update_status(task.id, "assigned", Some(&worker))
                .expect("assign retry fixture task");
            db.claim_task_with_pid(task.id, &worker, 30_000 + index)
                .expect("claim retry fixture task");
            db.conn
                .execute(
                    "INSERT INTO project_build_batches
                     (task_id, session_id, worker_slug, expected_count, observed_count,
                      passing_count, outcome, workspace_path, completed_at)
                     VALUES (?1, ?2, ?3, 1, 1, 0, 'failed', '/fixture', 'done')",
                    params![task.id, format!("diagnostic-{index}"), worker],
                )
                .expect("seed failed diagnostic batch");
            db.conn
                .execute(
                    "UPDATE tasks SET retry_count = 3 WHERE id = ?1",
                    params![task.id],
                )
                .expect("seed diagnostic threshold after initial batch");
            let request = db
                .create_build_retry_request_for_worker(&worker, task.id, "focused retry")
                .expect("create retry request for fixture");
            if index == 0 {
                fixtures.push((task.id, worker, request.id));
            } else {
                db.approve_build_retry_request(request.id, "supervisor", "fixture approval")
                    .expect("approve populated retry fixture");
            }
        }
        let (task_id, worker, request_id) = fixtures.pop().expect("target retry fixture");
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze populated fixture");

        // Migration 19's outer QC lookup and evidence-count predicates are
        // explained through their exact production SELECT, since trigger
        // subprogram plans are omitted from EXPLAIN on the outer UPDATE.
        let qc_plan = explain(&db, LATEST_BUILD_BATCH_FOR_QC_SQL, params![task_id]);
        assert_index(
            &qc_plan,
            "idx_project_build_batches_task_id",
            "migration 19 latest-batch QC predicate",
        );
        assert_no_table_scan(
            &qc_plan,
            "project_build_batches",
            "migration 19 QC predicate",
        );
        assert_index(
            &qc_plan,
            "idx_build_evidence_batch_id",
            "migration 19 QC evidence-count predicates",
        );

        let begin_latest = explain(
            &db,
            "SELECT id FROM project_build_batches WHERE task_id = ?1
             ORDER BY id DESC LIMIT 1",
            params![task_id],
        );
        assert_index(
            &begin_latest,
            "idx_project_build_batches_task_id",
            "begin batch latest-diagnosis lookup",
        );
        let available_approval = explain(
            &db,
            "SELECT id FROM build_retry_approvals
             WHERE task_id = ?1 AND worker_slug = ?2 AND status = 'available'
               AND diagnostic_batch_id = ?3 ORDER BY id DESC LIMIT 1",
            params![task_id, worker.as_str(), 1_i64],
        );
        assert_index(
            &available_approval,
            "idx_build_retry_approval_task_history",
            "begin batch available-approval lookup",
        );
        assert_no_table_scan(
            &available_approval,
            "build_retry_approvals",
            "begin batch available-approval lookup",
        );

        let create_diagnostic = explain(
            &db,
            "SELECT b.id FROM project_build_batches b JOIN tasks t ON t.id = b.task_id
             WHERE t.id = ?1 AND t.assigned_worker = ?2 AND t.status = 'in_progress'
               AND t.claimed_by_pid IS NOT NULL AND t.retry_count >= 3
               AND b.outcome IN ('failed','incomplete') AND b.completed_at IS NOT NULL
               AND b.id = (SELECT MAX(latest.id) FROM project_build_batches latest
                           WHERE latest.task_id = t.id)",
            params![task_id, worker.as_str()],
        );
        assert_index(
            &create_diagnostic,
            "idx_project_build_batches_task_id",
            "create retry request latest-diagnosis lookup",
        );
        assert_no_table_scan(
            &create_diagnostic,
            "tasks",
            "create retry request predicate",
        );
        assert_no_table_scan(
            &create_diagnostic,
            "project_build_batches",
            "create retry request predicate",
        );
        let pending_request = explain(
            &db,
            "SELECT EXISTS(SELECT 1 FROM worker_requests
             WHERE task_id = ?1 AND request_type = 'build_retry_approval' AND status = 'pending')",
            params![task_id],
        );
        assert_index(
            &pending_request,
            "idx_worker_requests_task",
            "create retry request pending-request check",
        );
        assert_no_table_scan(
            &pending_request,
            "worker_requests",
            "create retry request pending-request check",
        );

        let approve_request = explain(
            &db,
            "SELECT r.task_id, r.worker_slug, t.assigned_worker, r.diagnostic_batch_id
             FROM worker_requests r JOIN tasks t ON t.id = r.task_id
             WHERE r.id = ?1 AND r.request_type = 'build_retry_approval'
               AND r.status = 'pending' AND t.status = 'in_progress'
               AND t.retry_count >= 3
               AND r.diagnostic_batch_id = (SELECT MAX(b.id) FROM project_build_batches b
                    WHERE b.task_id = t.id AND b.outcome IN ('failed','incomplete')
                      AND b.completed_at IS NOT NULL)",
            params![request_id],
        );
        assert_index(
            &approve_request,
            "idx_project_build_batches_task_id",
            "approve retry latest-diagnosis lookup",
        );
        assert_no_table_scan(&approve_request, "tasks", "approve retry request predicate");
        assert_no_table_scan(
            &approve_request,
            "worker_requests",
            "approve retry request predicate",
        );
        let existing_available = explain(
            &db,
            "SELECT EXISTS(SELECT 1 FROM build_retry_approvals
             WHERE task_id = ?1 AND status = 'available')",
            params![task_id],
        );
        assert_index(
            &existing_available,
            "idx_build_retry_approval_one_available",
            "approve retry existing-grant check",
        );

        // Migration 20's trigger reads cannot be planned through INSERT. Plan
        // each exact inner predicate independently against this same fixture.
        let approval_guard = explain(
            &db,
            "SELECT 1 FROM worker_requests r JOIN tasks t ON t.id = r.task_id
             WHERE r.id = ?1 AND r.task_id = ?2 AND r.worker_slug = ?3
               AND r.diagnostic_batch_id = ?4
               AND r.request_type = 'build_retry_approval' AND r.status = 'pending'
               AND t.status = 'in_progress' AND t.assigned_worker = ?3
               AND t.retry_count >= 3
               AND (SELECT id FROM project_build_batches WHERE task_id = ?2
                    ORDER BY id DESC LIMIT 1) = ?4
               AND EXISTS (SELECT 1 FROM project_build_batches b WHERE b.id = ?4
                    AND b.outcome IN ('failed','incomplete') AND b.completed_at IS NOT NULL)",
            params![request_id, task_id, worker.as_str(), 1_i64],
        );
        assert_index(
            &approval_guard,
            "idx_project_build_batches_task_id",
            "migration 20 approval-insert latest-diagnosis predicate",
        );
        assert_no_table_scan(
            &approval_guard,
            "worker_requests",
            "migration 20 approval-insert predicate",
        );
        let latest_batch_guard = explain(
            &db,
            "SELECT id FROM project_build_batches WHERE task_id = ?1
             ORDER BY id DESC LIMIT 1",
            params![task_id],
        );
        assert_index(
            &latest_batch_guard,
            "idx_project_build_batches_task_id",
            "migration 20 batch-insert latest-diagnosis predicate",
        );
        let retry_threshold_guard = explain(
            &db,
            "SELECT retry_count FROM tasks WHERE id = ?1",
            params![task_id],
        );
        assert!(
            retry_threshold_guard
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "migration 20 batch-insert retry threshold should use task primary key; plan: {retry_threshold_guard:?}"
        );
        let approval_by_id = explain(
            &db,
            "SELECT 1 FROM build_retry_approvals a
             WHERE a.id = ?1 AND a.task_id = ?2 AND a.worker_slug = ?3
               AND a.status = 'consumed' AND a.diagnostic_batch_id = ?4",
            params![1_i64, task_id, worker.as_str(), 1_i64],
        );
        assert!(
            approval_by_id
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "migration 20 batch-insert grant predicate should use approval primary key; plan: {approval_by_id:?}"
        );
        let approval_by_id_without_diagnosis = explain(
            &db,
            "SELECT 1 FROM build_retry_approvals a
             WHERE a.id = ?1 AND a.task_id = ?2 AND a.worker_slug = ?3
               AND a.status = 'consumed'",
            params![1_i64, task_id, worker.as_str()],
        );
        assert!(
            approval_by_id_without_diagnosis
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "migration 20 batch-insert grant-existence predicate should use approval primary key; plan: {approval_by_id_without_diagnosis:?}"
        );

        // Run the actual consumer path to produce a running batch, then plan
        // its exact owner/session-qualified identity read before finalizing it.
        db.approve_build_retry_request(request_id, "supervisor", "plan fixture grant")
            .expect("approve target retry request");
        let granted = db
            .begin_project_build_batch_for_worker(
                &worker,
                "plan-fixture-session",
                1,
                "/fixture",
                Some("fingerprint"),
            )
            .expect("begin approved retry batch")
            .expect("claimed task resolves");
        let finish_batch = explain(
            &db,
            "SELECT task_id, expected_count, workspace_path, workspace_fingerprint
             FROM project_build_batches WHERE id = ?1 AND session_id = ?2
               AND worker_slug = ?3 AND outcome = 'running'",
            params![granted.id, "plan-fixture-session", worker.as_str()],
        );
        assert!(
            finish_batch
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "finish batch identity read should use the batch primary key; plan: {finish_batch:?}"
        );
        assert_no_table_scan(
            &finish_batch,
            "project_build_batches",
            "finish batch identity read",
        );
        assert_eq!(
            db.finish_project_build_batch_for_worker(
                granted.id,
                &worker,
                "plan-fixture-session",
                &[],
                None,
                false,
            )
            .expect("finish plan fixture batch")
            .expect("running batch remains attributable")
            .0,
            task_id
        );
    }

    #[test]
    fn p4_open_task_readers_use_populated_plans_and_production_results() {
        use rusqlite::types::Value;

        fn explain(db: &TasksDb, sql: &str, bindings: &[Value]) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare production-shaped query plan");
            statement
                .query_map(rusqlite::params_from_iter(bindings.iter()), |row| {
                    row.get::<_, String>(3)
                })
                .expect("explain populated query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        }

        fn assert_primary_key_lookup(details: &[String], table: &str) {
            assert!(
                details.iter().any(|detail| {
                    detail.contains(&format!("SEARCH {table}"))
                        && (detail.contains("INTEGER PRIMARY KEY")
                            || detail.contains("PRIMARY KEY"))
                }),
                "{table} point read should use its primary key; plan: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.contains(&format!("SCAN {table}"))),
                "{table} point read scanned the populated table; plan: {details:?}"
            );
        }

        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("apply all task migrations");

        let mut target_message = None;
        let mut target_request = None;
        for index in 0..240 {
            let message = db
                .insert_worker_message(
                    &format!("sender-{index}"),
                    if index % 2 == 0 {
                        "worker-recipient"
                    } else {
                        "worker-other"
                    },
                    &format!("populated message {index}"),
                    "question",
                    None,
                )
                .expect("populate ACP messages");
            let request = db
                .create_worker_request(&CreateWorkerRequestInput {
                    worker_slug: format!("worker-{index}"),
                    task_id: None,
                    request_type: "report_issue".into(),
                    payload: format!("{{\"sequence\":{index}}}"),
                })
                .expect("populate supervisor requests");
            if index == 173 {
                target_message = Some(message);
                target_request = Some(request);
            }
        }
        let target_message = target_message.expect("selected message fixture");
        let target_request = target_request.expect("selected request fixture");

        for index in 0..240 {
            let (worker_slug, task_type, assignment_method, outcome) = match index {
                0 | 1 => ("worker-backend", "chore", "automatic", Some("success")),
                2 => ("worker-backend", "chore", "automatic", Some("failure")),
                3 => ("worker-frontend", "bug", "automatic", Some("success")),
                4 | 5 => ("worker-failing", "chore", "automatic", Some("failure")),
                6 => ("worker-pending", "chore", "automatic", None),
                _ => ("worker-manual", "chore", "override", Some("success")),
            };
            db.conn
                .execute(
                    "INSERT INTO routing_history (
                         id, task_id, task_title, task_type, worker_slug, assigned_at,
                         outcome, outcome_at, assignment_method
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        format!("p4-reader-{index}"),
                        format!("task-{index}"),
                        format!("routing fixture {index}"),
                        task_type,
                        worker_slug,
                        "2026-10-02T00:00:00Z",
                        outcome,
                        outcome.map(|_| "2026-10-02T00:01:00Z"),
                        assignment_method,
                    ],
                )
                .expect("populate routing history");
        }
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze populated reader fixtures");

        for (table, sql, id) in [
            (
                "worker_messages",
                "SELECT id, from_slug, to_slug, body, kind, task_id, status, created_at, read_at
                 FROM worker_messages WHERE id = ?1",
                target_message.id,
            ),
            (
                "worker_requests",
                "SELECT id, worker_slug, task_id, request_type, payload, status,
                        resolution_note, created_at, resolved_at
                 FROM worker_requests WHERE id = ?1",
                target_request.id,
            ),
        ] {
            let plan = explain(&db, sql, &[Value::from(id)]);
            assert_primary_key_lookup(&plan, table);
        }

        let message = db
            .get_worker_message(target_message.id)
            .expect("read ACP message by id")
            .expect("selected ACP message exists");
        assert_eq!(message.id, target_message.id);
        assert_eq!(message.from_slug, "sender-173");
        assert_eq!(message.to_slug, "worker-other");
        assert_eq!(message.body, "populated message 173");
        assert_eq!(message.kind, "question");
        assert_eq!(message.task_id, None);
        assert_eq!(message.status, "pending");
        assert!(db
            .get_worker_message(-1)
            .expect("missing message lookup")
            .is_none());

        let request = db
            .get_worker_request_by_id(target_request.id)
            .expect("read supervisor request by id")
            .expect("selected supervisor request exists");
        assert_eq!(request.id, target_request.id);
        assert_eq!(request.worker_slug, "worker-173");
        assert_eq!(request.task_id, None);
        assert_eq!(request.request_type, "report_issue");
        assert_eq!(request.payload, "{\"sequence\":173}");
        assert_eq!(request.status, WorkerRequestStatus::Pending);
        assert!(db
            .get_worker_request_by_id(-1)
            .expect("missing request lookup")
            .is_none());

        let routing_plan = explain(
            &db,
            "SELECT worker_slug, task_type,
                    SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) AS successes,
                    SUM(CASE WHEN outcome IN ('success','failure') THEN 1 ELSE 0 END) AS total
             FROM routing_history
             WHERE assignment_method = 'automatic' AND outcome IS NOT NULL
             GROUP BY worker_slug, task_type
             HAVING total > 0",
            &[],
        );
        assert!(
            routing_plan
                .iter()
                .any(|detail| {
                    detail.contains("idx_routing_history_learning")
                        || detail.contains("idx_routing_history_recent_auto")
                }),
            "success-rate aggregate should use an existing assignment-method index; plan: {routing_plan:?}"
        );
        assert!(
            !routing_plan
                .iter()
                .any(|detail| detail.contains("SCAN routing_history")),
            "success-rate aggregate scanned populated routing history; plan: {routing_plan:?}"
        );

        let rates = db
            .get_worker_success_rates()
            .expect("read production routing success-rate aggregate");
        assert_eq!(
            rates.len(),
            3,
            "only completed automatic groups are included"
        );
        assert_eq!(rates.get("worker-backend:chore"), Some(&(2.0 / 3.0)));
        assert_eq!(rates.get("worker-frontend:bug"), Some(&1.0));
        assert_eq!(rates.get("worker-failing:chore"), Some(&0.0));
        assert!(!rates.contains_key("worker-pending:chore"));
        assert!(!rates.contains_key("worker-manual:chore"));
    }

    #[test]
    fn worker_request_status_filter_uses_an_index_with_populated_rows() {
        let path = sandbox("query-plan-worker-requests");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        for index in 0..160 {
            db.create_worker_request(&CreateWorkerRequestInput {
                worker_slug: format!("worker-{index}"),
                task_id: None,
                request_type: "report_issue".into(),
                payload: "{}".into(),
            })
            .expect("create worker request");
            if index % 4 == 0 {
                db.update_worker_request_status(index as i64 + 1, "acknowledged", None)
                    .expect("acknowledge worker request");
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let details = {
            let explain_sql = format!("EXPLAIN QUERY PLAN {WORKER_REQUESTS_BY_STATUS_SQL}");
            let mut statement = db
                .conn
                .prepare(&explain_sql)
                .expect("prepare worker request plan");
            statement
                .query_map(["pending"], |row| row.get::<_, String>(3))
                .expect("explain production worker request query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        };

        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_worker_requests_status_created_at")),
            "worker request status filter should use the measured status/time index; plan: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN worker_requests")),
            "worker request status filter scanned the request table; plan: {details:?}"
        );
        let recent_requests = {
            let explain_sql = format!("EXPLAIN QUERY PLAN {WORKER_REQUESTS_RECENT_SQL}");
            let mut statement = db
                .conn
                .prepare(&explain_sql)
                .expect("prepare unfiltered worker request plan");
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .expect("explain full worker request query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect full worker request plan")
        };
        assert!(!recent_requests.is_empty());
        assert_eq!(
            db.list_worker_requests(None)
                .expect("read all worker requests")
                .len(),
            160,
            "the unfiltered request list intentionally returns the full result"
        );
        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn task_board_reads_use_primary_and_filter_indexes_with_populated_rows() {
        fn explain<P: rusqlite::Params>(db: &TasksDb, sql: &str, params: P) -> Vec<String> {
            let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
            let mut statement = db.conn.prepare(&explain_sql).expect("prepare query plan");
            statement
                .query_map(params, |row| row.get::<_, String>(3))
                .expect("explain production task-board query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        }

        let path = sandbox("query-plan-task-board-reads");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        let mut target_task_id = None;
        for index in 0..200 {
            let task = ready_task(&db, &format!("task board query {index}"));
            if index % 10 == 0 {
                let worker = if index == 0 {
                    "worker-target"
                } else {
                    "worker-other"
                };
                db.update_status(task.id, "assigned", Some(worker))
                    .expect("assign task");
                if index == 0 {
                    target_task_id = Some(task.id);
                }
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let point_read = explain(&db, TASK_BY_ID_SQL, [1_i64]);
        assert!(
            point_read
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "task point-read should use the rowid primary key; plan: {point_read:?}"
        );
        assert!(
            !point_read
                .iter()
                .any(|detail| detail.contains("SCAN tasks")),
            "task point-read scanned the board; plan: {point_read:?}"
        );

        let filtered_board = explain(&db, TASKS_BY_STATUS_SQL, ["assigned"]);
        assert!(
            filtered_board
                .iter()
                .any(|detail| detail.contains("idx_tasks_status")),
            "status-filtered board should use idx_tasks_status; plan: {filtered_board:?}"
        );
        assert!(
            !filtered_board
                .iter()
                .any(|detail| detail.contains("SCAN tasks")),
            "status-filtered board scanned the task table; plan: {filtered_board:?}"
        );

        let full_board = explain(&db, TASKS_ALL_BY_CREATED_SQL, []);
        assert!(
            !full_board.is_empty(),
            "the registered full-result board query should have a plan"
        );
        assert_eq!(
            db.list(None).expect("read complete task board").len(),
            200,
            "the unfiltered board intentionally returns the full task result"
        );

        let worker_tasks = explain(
            &db,
            TASKS_BY_STATUS_AND_WORKER_SQL,
            ["assigned", "worker-target"],
        );
        assert!(
            worker_tasks.iter().any(|detail| {
                detail.contains("idx_tasks_assigned") || detail.contains("idx_tasks_status")
            }),
            "worker task filter should use an applicable status/worker index; plan: {worker_tasks:?}"
        );
        assert!(
            !worker_tasks
                .iter()
                .any(|detail| detail.contains("SCAN tasks")),
            "worker task filter scanned the task table; plan: {worker_tasks:?}"
        );

        let result = db
            .list_tasks_by_status_and_worker("assigned", "worker-target")
            .expect("read target worker tasks");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, target_task_id.expect("target task id"));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn epic_progress_readers_use_measured_indexes_with_populated_tasks() {
        fn explain<P: rusqlite::Params>(db: &TasksDb, sql: &str, params: P) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare epic progress query plan");
            statement
                .query_map(params, |row| row.get::<_, String>(3))
                .expect("explain epic progress query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect epic progress query plan")
        }

        let path = sandbox("query-plan-epic-progress");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        {
            let tx = db
                .conn
                .unchecked_transaction()
                .expect("start fixture insert");
            for index in 0..5_000 {
                let epic_id = (1_000 + index % 100).to_string();
                let phase = format!("phase-{}", index % 4);
                tx.execute(
                    "INSERT INTO tasks (title, task_type, status, created_by, epic_id, phase, phase_order)
                     VALUES (?1, 'feature', 'ready', 'query-plan-test', ?2, ?3, ?4)",
                    params![format!("fixture-{index}"), epic_id, phase, index % 4],
                )
                .expect("insert legal ready task fixture");
            }
            tx.commit().expect("commit populated task fixture");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        db.conn
            .execute_batch(
                "DROP INDEX idx_tasks_epic_phase_order;
                 DROP INDEX idx_tasks_ready_phase_order;
                 ANALYZE",
            )
            .expect("remove candidate indexes for baseline plan");
        let epic_baseline = explain(&db, EPIC_TASKS_SQL, ["1001"]);
        assert!(
            epic_baseline.iter().any(|detail| detail == "SCAN tasks"),
            "unindexed epic progress reader should expose its full task scan; plan: {epic_baseline:?}"
        );
        assert!(
            epic_baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "unindexed epic progress reader should expose its sort; plan: {epic_baseline:?}"
        );
        let phase_baseline = explain(&db, READY_TASKS_IN_PHASE_SQL, ["1001", "phase-1"]);
        assert!(
            phase_baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "unindexed ready-phase reader should expose its sort; plan: {phase_baseline:?}"
        );

        db.conn
            .execute_batch(
                "CREATE INDEX idx_tasks_epic_phase_order
                    ON tasks(epic_id, COALESCE(phase_order, 999), id)
                    WHERE epic_id IS NOT NULL;
                 CREATE INDEX idx_tasks_ready_phase_order
                    ON tasks(epic_id, phase, COALESCE(phase_order, 999), id)
                    WHERE epic_id IS NOT NULL AND phase IS NOT NULL;
                 ANALYZE",
            )
            .expect("create measured epic progress indexes");
        let epic_plan = explain(&db, EPIC_TASKS_SQL, ["1001"]);
        assert!(
            epic_plan
                .iter()
                .any(|detail| detail.contains("idx_tasks_epic_phase_order")),
            "epic progress reader should use the measured epic/order index; plan: {epic_plan:?}"
        );
        assert!(
            !epic_plan.iter().any(|detail| detail.contains("SCAN tasks"))
                && !epic_plan
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "epic progress reader should avoid a task scan and temp sort; plan: {epic_plan:?}"
        );

        // Migration 2's epic-completion lifecycle trigger runs these same two
        // child predicates before allowing an epic to become done. EXPLAIN the
        // predicates independently because SQLite does not include trigger
        // subprogram plans in EXPLAIN QUERY PLAN for an UPDATE statement.
        let epic_completion_guard = "SELECT NOT EXISTS (
                SELECT 1 FROM tasks child WHERE child.epic_id = ?1
            ) OR EXISTS (
                SELECT 1 FROM tasks child
                WHERE child.epic_id = ?1 AND child.status != 'done'
            )";
        let lifecycle_plan = explain(&db, epic_completion_guard, ["1001"]);
        let child_lookups = lifecycle_plan
            .iter()
            .filter(|detail| detail.contains("SEARCH child"))
            .collect::<Vec<_>>();
        assert_eq!(
            child_lookups.len(),
            2,
            "both trigger subqueries must use selective child lookups: {lifecycle_plan:?}"
        );
        assert!(
            child_lookups
                .iter()
                .all(|detail| detail.contains("idx_tasks_epic_phase_order")),
            "epic lifecycle trigger predicates should use the measured epic index: {lifecycle_plan:?}"
        );
        let incomplete_epic: bool = db
            .conn
            .query_row(epic_completion_guard, ["1001"], |row| row.get(0))
            .expect("read populated epic lifecycle predicate");
        assert!(incomplete_epic, "fixture epic has unfinished child tasks");

        let phase_plan = explain(&db, READY_TASKS_IN_PHASE_SQL, ["1001", "phase-1"]);
        assert!(
            phase_plan
                .iter()
                .any(|detail| detail.contains("idx_tasks_ready_phase_order")),
            "ready-phase reader should use the measured epic/phase/order index; plan: {phase_plan:?}"
        );
        assert!(
            !phase_plan
                .iter()
                .any(|detail| detail.contains("SCAN tasks"))
                && !phase_plan
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "ready-phase reader should avoid a task scan and temp sort; plan: {phase_plan:?}"
        );

        let epic_tasks = db.get_epic_tasks("1001").expect("read epic tasks");
        assert_eq!(epic_tasks.len(), 50);
        assert!(epic_tasks.windows(2).all(|pair| {
            (pair[0].phase_order, pair[0].id) <= (pair[1].phase_order, pair[1].id)
        }));
        let phase_tasks = db
            .get_ready_tasks_in_phase("1001", "phase-1")
            .expect("read ready tasks in phase");
        assert_eq!(phase_tasks.len(), 50);
        assert!(phase_tasks
            .iter()
            .all(|task| task.epic_id.as_deref() == Some("1001")
                && task.phase.as_deref() == Some("phase-1")
                && matches!(task.status, TaskStatus::Ready | TaskStatus::Assigned)));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn ready_task_claim_uses_dispatch_and_active_claim_indexes() {
        fn explain(db: &TasksDb, sql: &str) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare production claim plan");
            statement
                .query_map(params!["worker-target", 4242_i64, "test-host"], |row| {
                    row.get::<_, String>(3)
                })
                .expect("explain atomic claim")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect claim plan")
        }

        let path = sandbox("query-plan-ready-task-claim");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        let first = ready_task(&db, "first ready dispatch candidate");
        for index in 0..300 {
            let task = ready_task(&db, &format!("dispatch candidate {index}"));
            if index % 4 == 0 {
                db.update_status(task.id, "needs_info", None)
                    .expect("remove task from ready dispatch set");
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        db.conn
            .execute_batch("DROP INDEX idx_tasks_ready_dispatch; ANALYZE")
            .expect("measure pre-index claim plan");
        let baseline_plan = explain(&db, CLAIM_NEXT_READY_SQL);
        assert!(
            baseline_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "baseline dispatch should expose its created_at ordering sort; plan: {baseline_plan:?}"
        );
        db.conn
            .execute_batch(
                "CREATE INDEX idx_tasks_ready_dispatch
                    ON tasks(julianday(created_at), id)
                    WHERE status = 'ready' AND task_type != 'epic';
                 ANALYZE",
            )
            .expect("measure candidate dispatch index");

        let assigned = ready_task(&db, "assigned task claim candidate");
        db.update_status(assigned.id, "assigned", Some("claim-worker"))
            .expect("assign pid claim fixture");
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze claim fixture");
        let mut claim_statement = db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {CLAIM_TASK_WITH_PID_SQL}"))
            .expect("prepare assigned-task claim plan");
        let claim_plan = claim_statement
            .query_map(
                params!["claim-worker", 9911_i64, "test-host", assigned.id],
                |row| row.get::<_, String>(3),
            )
            .expect("explain assigned-task claim")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect assigned-task claim plan");
        drop(claim_statement);
        assert!(
            claim_plan
                .iter()
                .any(|detail| detail.contains("INTEGER PRIMARY KEY")),
            "assigned claim should point-read its task by primary key; plan: {claim_plan:?}"
        );
        assert!(
            claim_plan.iter().any(|detail| {
                detail.contains("SEARCH active USING INDEX idx_tasks_status")
                    || detail.contains("SEARCH active USING INDEX idx_tasks_assigned")
            }),
            "embedded one-active-task guard should use the existing status/worker index; plan: {claim_plan:?}"
        );
        db.claim_task_with_pid(assigned.id, "claim-worker", 9911)
            .expect("claim assigned task with pid");
        assert_eq!(
            db.get_by_id(assigned.id).unwrap().unwrap().status.as_str(),
            "in_progress"
        );

        let plan = explain(&db, CLAIM_NEXT_READY_SQL);
        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_tasks_ready_dispatch")),
            "ready selection should use its partial chronological index; plan: {plan:?}"
        );
        assert!(
            plan.iter().any(|detail| {
                detail.contains("idx_tasks_assigned") || detail.contains("idx_tasks_status")
            }),
            "one-active-claim guard should use a status/worker index; plan: {plan:?}"
        );
        assert!(
            !plan.iter().any(|detail| {
                detail.contains("SCAN tasks") && !detail.contains("idx_tasks_ready_dispatch")
            }),
            "atomic dispatch performed a base-table scan; plan: {plan:?}"
        );

        let claimed = db
            .claim_next_ready("worker-target", 4242)
            .expect("claim ready task")
            .expect("ready task exists");
        assert_eq!(claimed.id, first.id, "oldest ready task is selected");
        assert!(
            db.claim_next_ready("worker-target", 4242).is_err(),
            "a worker already holding an in-progress task cannot claim another"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn routing_history_and_affinity_reads_use_measured_indexes() {
        fn explain<P: rusqlite::Params>(db: &TasksDb, sql: &str, params: P) -> Vec<String> {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare production routing query plan");
            statement
                .query_map(params, |row| row.get::<_, String>(3))
                .expect("explain production routing query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect routing query plan")
        }

        let path = sandbox("query-plan-routing-history");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        for index in 0..720 {
            let assignment_method = if index % 5 == 0 {
                "override"
            } else {
                "automatic"
            };
            let outcome = match index % 3 {
                0 => "success",
                1 => "failure",
                _ => "pending",
            };
            db.conn
                .execute(
                    "INSERT INTO routing_history
                     (id, task_id, task_title, task_type, worker_slug, assigned_at, outcome,
                      duration_ms, tokens_used, assignment_method, override_reason)
                     VALUES (?1, ?2, 'routing fixture', ?3, ?4, ?5, ?6, ?7, 20, ?8, ?9)",
                    params![
                        format!("route-{index:04}"),
                        format!("task-{index}"),
                        format!("type-{}", index % 6),
                        format!("worker-{}", (index / 6) % 18),
                        format!("2026-09-{:02}T12:00:00Z", index % 28 + 1),
                        if index % 3 == 2 {
                            None::<String>
                        } else {
                            Some(outcome.to_string())
                        },
                        if index % 4 == 0 { Some(450_i64) } else { None },
                        assignment_method,
                        (assignment_method == "override").then_some("user choice"),
                    ],
                )
                .expect("populate routing history");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        db.conn
            .execute_batch("DROP INDEX idx_routing_history_recent_auto; ANALYZE")
            .expect("measure baseline routing plans");
        let recent_baseline = explain(&db, ROUTING_HISTORY_RECENT_SQL, [30_i64]);
        let affinity_baseline = explain(&db, ROUTING_AFFINITY_STATS_SQL, []);
        assert!(
            recent_baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "baseline routing history needs a timestamp sort; plan: {recent_baseline:?}"
        );
        assert!(
            affinity_baseline
                .iter()
                .any(|detail| detail.contains("idx_routing_history_worker_type")),
            "affinity aggregation should retain its existing worker/type index; plan: {affinity_baseline:?}"
        );
        assert!(
            !affinity_baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE")),
            "affinity grouping already follows worker/type index order; plan: {affinity_baseline:?}"
        );

        db.conn
            .execute_batch(
                "CREATE INDEX idx_routing_history_recent_auto
                    ON routing_history(assignment_method, julianday(assigned_at) DESC);
                 ANALYZE",
            )
            .expect("measure candidate routing indexes");
        let recent_plan = explain(&db, ROUTING_HISTORY_RECENT_SQL, [30_i64]);
        assert!(
            recent_plan
                .iter()
                .any(|detail| detail.contains("idx_routing_history_recent_auto")),
            "routing history should use its automatic/date index; plan: {recent_plan:?}"
        );
        assert!(
            !recent_plan
                .iter()
                .any(|detail| detail.contains("SCAN routing_history")),
            "routing history performed an unindexed scan; plan: {recent_plan:?}"
        );

        let affinity_plan = explain(&db, ROUTING_AFFINITY_STATS_SQL, []);
        assert!(
            affinity_plan
                .iter()
                .any(|detail| detail.contains("idx_routing_history_worker_type")),
            "affinity aggregation should use its worker/type index; plan: {affinity_plan:?}"
        );
        assert!(
            !affinity_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE")),
            "affinity grouping should follow worker/type index order; plan: {affinity_plan:?}"
        );

        let recent = db
            .get_routing_history(30)
            .expect("read recent routing history");
        assert_eq!(recent.len(), 30);
        for row in &recent {
            let method: String = db
                .conn
                .query_row(
                    "SELECT assignment_method FROM routing_history WHERE id = ?1",
                    [&row.id],
                    |record| record.get(0),
                )
                .expect("read assignment provenance");
            assert_eq!(method, "automatic");
        }
        let affinity = db.get_affinity_stats().expect("read routing affinity");
        assert_eq!(affinity.len(), 18 * 6);

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn assignment_guard_uses_worker_index_and_preserves_one_active_task_limit() {
        let path = sandbox("query-plan-assignment-guard");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        for index in 0..700 {
            let task = ready_task(&db, &format!("assignment candidate {index}"));
            db.update_status(task.id, "assigned", Some(&format!("worker-{index}")))
                .expect("seed assigned task for guard plan");
        }
        let candidate = ready_task(&db, "candidate for automatic assignment");
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let mut statement = db
            .conn
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {ASSIGN_WITH_ROUTING_DECISION_SQL}"
            ))
            .expect("prepare production assignment plan");
        let plan = statement
            .query_map(params![candidate.id, "worker-target"], |row| {
                row.get::<_, String>(3)
            })
            .expect("explain production assignment");
        let plan = plan
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect assignment plan");
        drop(statement);
        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_tasks_assigned")),
            "the embedded one-active-task guard must seek by assigned worker: {plan:?}"
        );
        assert!(
            !plan.iter().any(|detail| {
                detail.contains("SCAN tasks") && !detail.contains("idx_tasks_status")
            }),
            "assignment should not scan the tasks base table: {plan:?}"
        );

        let entry = crate::routing::TaskRouter::assignment_entry(&candidate, "worker-target");
        db.assign_with_routing_history(candidate.id, "worker-target", &entry)
            .expect("assign task while worker has no active claim");
        assert_eq!(
            db.get_by_id(candidate.id)
                .unwrap()
                .unwrap()
                .assigned_worker
                .as_deref(),
            Some("worker-target")
        );

        let active = ready_task(&db, "active task prevents another assignment");
        db.update_status(active.id, "assigned", Some("worker-target"))
            .expect("assign active-claim fixture");
        db.claim_task_with_pid(active.id, "worker-target", 873)
            .expect("claim active-claim fixture");
        let blocked = ready_task(&db, "refused while target worker is busy");
        let entry = crate::routing::TaskRouter::assignment_entry(&blocked, "worker-target");
        assert!(
            db.assign_with_routing_history(blocked.id, "worker-target", &entry)
                .is_err(),
            "production assignment must refuse a second active task"
        );
        assert_eq!(
            db.get_by_id(blocked.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn interrupted_claim_reconciliation_uses_owner_filter_index() {
        let path = sandbox("query-plan-interrupted-claims");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        let mut expected_ids = Vec::new();
        for index in 0..280 {
            let task = ready_task(&db, &format!("recovery candidate {index}"));
            let (worker, pid, host) = match index % 4 {
                0 => ("worker-target", 4242_i64, "test-host"),
                1 => ("worker-target", 4243_i64, "test-host"),
                2 => ("worker-target", 4242_i64, "other-host"),
                _ => ("worker-other", 4242_i64, "test-host"),
            };
            db.conn
                .execute(
                    "UPDATE tasks SET status='in_progress', assigned_worker=?1,
                        claimed_by_pid=?2, claim_host=?3 WHERE id=?4",
                    params![worker, pid, host, task.id],
                )
                .expect("seed interrupted claim");
            if index % 4 == 0 {
                expected_ids.push(task.id);
            }
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let mut statement = db
            .conn
            .prepare(&format!(
                "EXPLAIN QUERY PLAN {HOLD_INTERRUPTED_CLAIMS_SELECT_SQL}"
            ))
            .expect("prepare interrupted-claim plan");
        let plan = statement
            .query_map(params!["worker-target", 4242_i64, "test-host"], |row| {
                row.get::<_, String>(3)
            })
            .expect("explain interrupted-claim selection")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect interrupted-claim plan");
        drop(statement);
        assert!(
            plan.iter().any(|detail| {
                detail.contains("idx_tasks_assigned") || detail.contains("idx_tasks_status")
            }),
            "interrupted claim lookup should use its owner/status index; plan: {plan:?}"
        );
        assert!(
            !plan.iter().any(|detail| detail.contains("SCAN tasks")),
            "interrupted claim lookup scanned the task table; plan: {plan:?}"
        );

        let held = db
            .hold_interrupted_claims_for_worker_run(
                "worker-target",
                4242,
                "test-host",
                "controlled fixture interruption",
            )
            .expect("hold exact-owner claims");
        assert_eq!(held, expected_ids);
        assert!(held.iter().all(|id| db
            .get_by_id(*id)
            .unwrap()
            .is_some_and(|task| task.status.as_str() == "needs_info")));
        let mismatched: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE status='in_progress' AND assigned_worker='worker-target'",
                [],
                |row| row.get(0),
            )
            .expect("count other-owner claims");
        assert_eq!(mismatched, 140);

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn worker_message_inbox_and_counts_use_recipient_or_task_indexes() {
        fn explain<P: rusqlite::Params>(db: &TasksDb, sql: &str, params: P) -> Vec<String> {
            let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
            let mut statement = db.conn.prepare(&explain_sql).expect("prepare query plan");
            statement
                .query_map(params, |row| row.get::<_, String>(3))
                .expect("explain production ACP query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect query plan")
        }

        let path = sandbox("query-plan-worker-messages");
        let db = TasksDb::open_at(&path).expect("open isolated tasks database");
        let task = ready_task(&db, "worker message query task");
        for index in 0..360 {
            let recipient = if index % 12 == 0 {
                "worker-target"
            } else {
                "worker-other"
            };
            let task_id = if index % 6 == 0 { Some(task.id) } else { None };
            let message = db
                .insert_worker_message(
                    "worker-sender",
                    recipient,
                    "query-plan fixture",
                    "fyi",
                    task_id,
                )
                .expect("insert worker message");
            if index % 4 == 0 {
                db.conn
                    .execute(
                        "UPDATE worker_messages SET status='read' WHERE id=?1",
                        params![message.id],
                    )
                    .expect("mark fixture read");
            } else if index % 5 == 0 {
                db.acknowledge_worker_message(message.id)
                    .expect("acknowledge fixture message");
            }
            let created_at = crate::db::timestamps::format_utc_timestamp(
                chrono::Utc::now() - chrono::Duration::minutes(index as i64),
            );
            db.conn
                .execute(
                    "UPDATE worker_messages SET created_at=?1 WHERE id=?2",
                    params![created_at, message.id],
                )
                .expect("age ACP plan fixture");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let recipient_plan = explain(&db, WORKER_MESSAGES_FOR_RECIPIENT_SQL, ["worker-target"]);
        assert!(
            recipient_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_messages_to_status")),
            "recipient inbox should use the recipient/status index; plan: {recipient_plan:?}"
        );
        assert!(
            !recipient_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_messages")),
            "recipient inbox scanned the ACP table; plan: {recipient_plan:?}"
        );

        let unacked_plan = explain(
            &db,
            WORKER_MESSAGES_UNACKED_FOR_RECIPIENT_SQL,
            ["worker-target"],
        );
        assert!(
            unacked_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_messages_to_status")),
            "unacknowledged inbox should use the recipient/status index; plan: {unacked_plan:?}"
        );
        assert!(
            !unacked_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_messages")),
            "unacknowledged inbox scanned the ACP table; plan: {unacked_plan:?}"
        );

        for sql in [UNREAD_MESSAGE_COUNT_SQL, UNACKNOWLEDGED_MESSAGE_COUNT_SQL] {
            let count_plan = explain(&db, sql, ["worker-target"]);
            assert!(
                count_plan
                    .iter()
                    .any(|detail| detail.contains("idx_worker_messages_to_status")),
                "ACP count should use the recipient/status index; plan: {count_plan:?}"
            );
            assert!(
                !count_plan
                    .iter()
                    .any(|detail| detail.contains("SCAN worker_messages")),
                "ACP count scanned the message table; plan: {count_plan:?}"
            );
        }

        let task_plan = explain(&db, WORKER_MESSAGES_FOR_TASK_SQL, [task.id, 50_i64]);
        assert!(
            task_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_messages_task")),
            "task chat should use the task index; plan: {task_plan:?}"
        );
        assert!(
            !task_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_messages")),
            "task chat scanned the ACP table; plan: {task_plan:?}"
        );

        db.conn
            .execute_batch(
                "DROP INDEX idx_worker_messages_recent;
                 DROP INDEX idx_worker_messages_stale_time;
                 ANALYZE",
            )
            .expect("measure baseline recent/stale ACP plans");
        let recent_baseline = explain(&db, WORKER_MESSAGES_RECENT_SQL, [50_i64]);
        let stale_baseline = explain(&db, WORKER_MESSAGES_STALE_SQL, ["-60 minutes"]);
        assert!(
            recent_baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "baseline recent inbox should require an order sort; plan: {recent_baseline:?}"
        );
        assert!(
            stale_baseline
                .iter()
                .any(|detail| detail.contains("SCAN worker_messages")),
            "baseline stale inbox should scan without its selective index; plan: {stale_baseline:?}"
        );
        db.conn
            .execute_batch(
                "CREATE INDEX idx_worker_messages_recent
                    ON worker_messages(julianday(created_at) DESC, id DESC);
                 CREATE INDEX idx_worker_messages_stale_time
                    ON worker_messages(julianday(created_at), id)
                    WHERE status IN ('pending', 'read');
                 ANALYZE",
            )
            .expect("measure recent/stale ACP indexes");

        let recent_plan = explain(&db, WORKER_MESSAGES_RECENT_SQL, [50_i64]);
        assert!(
            recent_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_messages_recent")),
            "bounded recent inbox should use its ordering index; plan: {recent_plan:?}"
        );
        assert!(
            !recent_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "bounded recent inbox should not temp-sort; plan: {recent_plan:?}"
        );

        let stale_plan = explain(&db, WORKER_MESSAGES_STALE_SQL, ["-60 minutes"]);
        assert!(
            stale_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_messages_stale_time")),
            "stale inbox should seek by status and timestamp; plan: {stale_plan:?}"
        );
        assert!(
            !stale_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_messages")),
            "stale inbox scanned the ACP table; plan: {stale_plan:?}"
        );

        assert_eq!(db.list_recent_worker_messages(50).unwrap().len(), 50);
        let stale = db
            .list_stale_worker_messages(60)
            .expect("read stale worker messages");
        assert!(!stale.is_empty());
        assert!(stale
            .iter()
            .all(|message| message.status == "pending" || message.status == "read"));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    /// P8-69: unknown task statuses fail, while every declared state parses.
    #[test]
    fn t36_the_round_trip_accepts_every_real_status_and_rejects_a_typo() {
        for real in [
            "ready",
            "assigned",
            "in_progress",
            "qc_review",
            "done",
            "build_failed",
            "failed",
            "cancelled",
            "rejected",
            "needs_info",
        ] {
            assert_eq!(
                TaskStatus::from_str(real).expect("known status").as_str(),
                real,
                "{real} is a real status and the round trip lost it"
            );
        }
        for typo in ["in-progress", "Done", "qc review", "", "teleport"] {
            assert!(TaskStatus::from_str(typo).is_err(), "{typo:?} was accepted");
        }
    }

    #[test]
    fn t36_an_unknown_status_does_not_become_ready() {
        assert!(TaskStatus::from_str("in-progress").is_err());
    }

    #[test]
    fn p8_69_rejects_invalid_manual_and_direct_sql_transitions() {
        let db = TasksDb::open_at(&sandbox("p8-69-invalid")).expect("open");
        let task = ready_task(&db, "guarded");
        assert!(db.update_status(task.id, "done", None).is_err());
        assert!(db.update_status(task.id, "in-progress", None).is_err());
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
        assert!(db
            .conn
            .execute(
                "UPDATE tasks SET status='done' WHERE id=?1",
                params![task.id]
            )
            .is_err());
        assert!(db
            .conn
            .execute(
                "INSERT INTO tasks(title,status) VALUES('bad','in-progress')",
                []
            )
            .is_err());
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='assigned' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "assigned state without an owner must be rejected at the DB boundary"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='in_progress' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "in-progress state without an owner must be rejected at the DB boundary"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET assigned_worker='worker-backend' WHERE id=?1",
                    params![task.id]
                )
                .is_err(),
            "ready tasks cannot carry a stale worker assignment"
        );
        assert!(db
            .conn
            .execute(
                "INSERT INTO tasks(title,status) VALUES('ownerless','assigned')",
                []
            )
            .is_err());

        db.update_status(task.id, "assigned", Some("worker"))
            .expect("assign");
        db.claim_task_with_pid(task.id, "worker", 123)
            .expect("claim");
        db.insert_build_evidence(task.id, "session", "cargo test", 0, true, "cargo")
            .expect("record passing build evidence");
        move_to_qc_with_synthetic_workspace(&db, task.id, "worker").expect("review");
        db.update_status(task.id, "done", Some("worker"))
            .expect("approve");
        assert!(db
            .update_status(task.id, "in_progress", Some("worker"))
            .is_err());
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            TaskStatus::Done
        );
    }

    #[test]
    fn p4_claim_is_owner_checked_atomic_and_epics_cannot_be_assigned() {
        let db = TasksDb::open_at(&sandbox("p4-claim-owner-epic")).expect("open");
        let first = ready_task(&db, "first");
        let second = ready_task(&db, "second");
        db.update_status(first.id, "assigned", Some("worker-a"))
            .expect("assign first");
        db.update_status(second.id, "assigned", Some("worker-a"))
            .expect("assign second");

        assert!(db.claim_task_with_pid(first.id, "worker-b", 101).is_err());
        db.claim_task_with_pid(first.id, "worker-a", 102)
            .expect("owner claims first");
        assert!(db.claim_task_with_pid(second.id, "worker-a", 103).is_err());
        assert_eq!(
            db.get_by_id(second.id).unwrap().unwrap().status,
            TaskStatus::Assigned
        );

        let epic = db.create_epic("parent", None, "test").expect("epic");
        assert!(db
            .update_status(epic.id, "assigned", Some("worker-a"))
            .is_err());
        assert!(db
            .conn
            .execute(
                "UPDATE tasks SET status='in_progress', assigned_worker='worker-a' WHERE id=?1",
                params![epic.id],
            )
            .is_err());
    }

    #[test]
    fn p4_concurrent_claims_cannot_put_one_worker_on_two_tasks() {
        let path = sandbox("p4-concurrent-claim");
        let setup = TasksDb::open_at(&path).expect("open setup");
        let first = ready_task(&setup, "concurrent first");
        let second = ready_task(&setup, "concurrent second");
        setup
            .update_status(first.id, "assigned", Some("worker"))
            .unwrap();
        setup
            .update_status(second.id, "assigned", Some("worker"))
            .unwrap();
        drop(setup);

        let db_a = TasksDb::open_at(&path).expect("open first connection");
        let db_b = TasksDb::open_at(&path).expect("open second connection");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let barrier_a = barrier.clone();
        let first_thread = std::thread::spawn(move || {
            barrier_a.wait();
            db_a.claim_task_with_pid(first.id, "worker", 601)
        });
        let barrier_b = barrier.clone();
        let second_thread = std::thread::spawn(move || {
            barrier_b.wait();
            db_b.claim_task_with_pid(second.id, "worker", 602)
        });

        let outcomes = [
            first_thread.join().expect("first claimant did not panic"),
            second_thread.join().expect("second claimant did not panic"),
        ];
        assert_eq!(
            outcomes.iter().filter(|result| result.is_ok()).count(),
            1,
            "exactly one concurrent claim should succeed: {outcomes:?}"
        );
        let verify = TasksDb::open_at(&path).expect("reopen to verify");
        let active: i64 = verify
            .conn
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE assigned_worker='worker' AND status='in_progress'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, 1);
    }

    #[test]
    fn p4_competing_claims_on_one_task_have_exactly_one_winner() {
        let path = sandbox("p4-same-task-claim");
        let setup = TasksDb::open_at(&path).expect("open setup");
        let task = ready_task(&setup, "same task claim");
        setup
            .update_status(task.id, "assigned", Some("worker"))
            .unwrap();
        drop(setup);

        let db_a = TasksDb::open_at(&path).expect("open first connection");
        let db_b = TasksDb::open_at(&path).expect("open second connection");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let barrier_a = barrier.clone();
        let task_id = task.id;
        let first = std::thread::spawn(move || {
            barrier_a.wait();
            db_a.claim_task_with_pid(task_id, "worker", 801)
        });
        let barrier_b = barrier.clone();
        let second = std::thread::spawn(move || {
            barrier_b.wait();
            db_b.claim_task_with_pid(task_id, "worker", 802)
        });
        let outcomes = [
            first.join().expect("first claimant did not panic"),
            second.join().expect("second claimant did not panic"),
        ];
        assert_eq!(
            outcomes.iter().filter(|result| result.is_ok()).count(),
            1,
            "exactly one concurrent claim should win: {outcomes:?}"
        );
        let verify = TasksDb::open_at(&path).expect("reopen to verify");
        let claimed = verify.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(claimed.status, TaskStatus::InProgress);
        assert!(matches!(claimed.claimed_by_pid, Some(801 | 802)));
    }

    #[test]
    fn p4_unblock_and_epic_progress_roll_back_together() {
        let db = TasksDb::open_at(&sandbox("p4-unblock-rollback")).expect("open");
        let epic = db.create_epic("parent", None, "test").expect("epic");
        let completed = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "coding".into(),
                phase_order: 1,
                title: "completed blocker".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "done".into(),
                blocked_by: vec![],
                milestone: None,
                created_by: Some("test".into()),
            })
            .expect("completed child");
        let waiting = ready_task(&db, "waiting task outside the epic");
        db.conn
            .execute(
                "UPDATE tasks SET status='needs_info', blocked_by=?1 WHERE id=?2",
                params![
                    serde_json::json!([completed.id.to_string()]).to_string(),
                    waiting.id
                ],
            )
            .expect("set dependency fixture");
        db.update_status(completed.id, "assigned", Some("worker"))
            .unwrap();
        db.claim_task_with_pid(completed.id, "worker", 501).unwrap();
        db.update_status(completed.id, "done", Some("worker"))
            .unwrap();

        db.conn
            .execute_batch(
                "CREATE TRIGGER fail_epic_completion
                 BEFORE UPDATE OF status ON tasks
                 WHEN OLD.task_type = 'epic' AND NEW.status = 'done'
                 BEGIN SELECT RAISE(ABORT, 'injected epic failure'); END;",
            )
            .expect("inject late transaction failure");

        assert!(db.unblock_ready_tasks(completed.id).is_err());
        assert_eq!(
            db.get_by_id(waiting.id).unwrap().unwrap().status,
            TaskStatus::NeedsInfo,
            "dependency update must roll back when the later epic update fails"
        );
        assert_eq!(
            db.get_by_id(epic.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
    }

    #[test]
    fn p4_assignment_and_routing_history_commit_per_entry() {
        let db = TasksDb::open_at(&sandbox("p4-assignment-transaction")).expect("open");
        let valid = ready_task(&db, "valid batch entry");
        let valid_entry = crate::routing::TaskRouter::assignment_entry(&valid, "worker-a");
        assert_eq!(valid_entry.assigned_at.len(), 24);
        assert!(valid_entry.assigned_at.ends_with('Z'));
        db.assign_with_routing_history(valid.id, "worker-a", &valid_entry)
            .expect("assignment and history commit");
        let history = db.get_routing_history(10).expect("read routing history");
        let recorded = history
            .iter()
            .find(|entry| entry.task_id == valid.id.to_string())
            .expect("production routing reader returns assignment");
        assert_eq!(recorded.assigned_at, valid_entry.assigned_at);
        let assigned = db.get_by_id(valid.id).unwrap().unwrap();
        assert_eq!(assigned.status, TaskStatus::Assigned);
        assert_eq!(assigned.assigned_worker.as_deref(), Some("worker-a"));

        let failed = ready_task(&db, "entry with history failure");
        db.conn
            .execute_batch(
                "CREATE TRIGGER fail_worker_b_history
                 BEFORE INSERT ON routing_history WHEN NEW.worker_slug = 'worker-b'
                 BEGIN SELECT RAISE(ABORT, 'injected history failure'); END;",
            )
            .expect("inject history write failure");
        let failed_entry = crate::routing::TaskRouter::assignment_entry(&failed, "worker-b");
        assert!(db
            .assign_with_routing_history(failed.id, "worker-b", &failed_entry)
            .is_err());
        let unchanged = db.get_by_id(failed.id).unwrap().unwrap();
        assert_eq!(unchanged.status, TaskStatus::Ready);
        assert!(unchanged.assigned_worker.is_none());

        let history_for_failed: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM routing_history WHERE task_id = ?1",
                params![failed.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(history_for_failed, 0);

        let history_for_valid: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM routing_history WHERE task_id = ?1",
                params![valid.id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(history_for_valid, 1);
    }

    #[test]
    fn routing_overrides_are_audited_and_never_enter_automatic_learning() {
        let db = TasksDb::open_at(&sandbox("p4-routing-override-audit")).expect("open");
        let automatic = ready_task(&db, "automatic dispatch");
        let auto_entry = crate::routing::TaskRouter::assignment_entry(&automatic, "worker-a");
        db.assign_with_routing_history(automatic.id, "worker-a", &auto_entry)
            .expect("automatic dispatch persists");
        db.update_latest_routing_outcome_for_worker(
            &automatic.id.to_string(),
            "worker-a",
            "success",
        )
        .unwrap();

        let overridden = ready_task(&db, "audited override");
        let override_entry = crate::routing::TaskRouter::assignment_entry(&overridden, "worker-b");
        assert!(db
            .assign_with_routing_override(
                overridden.id,
                "worker-b",
                &override_entry,
                "the task requires the frontend specialist",
            )
            .is_ok());
        db.update_latest_routing_outcome_for_worker(
            &overridden.id.to_string(),
            "worker-b",
            "success",
        )
        .unwrap();

        let audit: (String, String) = db
            .conn
            .query_row(
                "SELECT assignment_method, override_reason FROM routing_history WHERE task_id=?1",
                params![overridden.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(audit.0, "override");
        assert_eq!(audit.1, "the task requires the frontend specialist");
        let learned = db.get_routing_history(20).unwrap();
        assert!(learned
            .iter()
            .any(|entry| entry.task_id == automatic.id.to_string()));
        assert!(!learned
            .iter()
            .any(|entry| entry.task_id == overridden.id.to_string()));
        assert_eq!(db.get_affinity_stats().unwrap().len(), 1);
        assert!(db
            .assign_with_routing_override(overridden.id, "worker-b", &override_entry, "   ",)
            .is_err());
    }

    #[test]
    fn legacy_routing_outcome_updates_only_newest_pending_attempt() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("migrate isolated tasks database");
        let task = ready_task(&db, "legacy routing outcome");
        let older = crate::routing::TaskRouter::assignment_entry(&task, "worker-a");
        let newer = crate::routing::TaskRouter::assignment_entry(&task, "worker-b");
        db.insert_routing_history(&older)
            .expect("insert older attempt");
        db.insert_routing_history(&newer)
            .expect("insert newer attempt");

        db.update_routing_outcome(&task.id.to_string(), "success", 15, 20)
            .expect("record one outcome");

        let attempts: Vec<(String, Option<String>)> = db
            .conn
            .prepare("SELECT id, outcome FROM routing_history WHERE task_id = ?1 ORDER BY rowid")
            .expect("prepare attempt query")
            .query_map(params![task.id.to_string()], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .expect("query attempts")
            .collect::<std::result::Result<_, _>>()
            .expect("collect attempts");
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0], (older.id, None));
        assert_eq!(attempts[1], (newer.id, Some("success".to_string())));
    }

    #[test]
    fn stale_task_owner_cannot_overwrite_reassignment_or_claim_its_outcome() {
        let db = TasksDb::open_at(&sandbox("p4-routing-owner-cas")).expect("open isolated db");
        let task = ready_task(&db, "status update races reassignment");
        let first = crate::routing::TaskRouter::assignment_entry(&task, "worker-a");
        db.assign_with_routing_history(task.id, "worker-a", &first)
            .expect("assign initial owner");
        let stale_snapshot = db.get_by_id(task.id).unwrap().unwrap();

        db.update_status(task.id, "failed", Some("worker-a"))
            .expect("finish old assignment");
        let ready = db.get_by_id(task.id).unwrap().unwrap();
        let second = crate::routing::TaskRouter::assignment_entry(&ready, "worker-b");
        db.assign_with_routing_history(task.id, "worker-b", &second)
            .expect("reassign task");

        let delayed_completion = db.update_status_with_routing_outcome(
            task.id,
            stale_snapshot.status.as_str(),
            "worker-a",
            "done",
            Some("worker-a"),
            None,
            "success",
        );
        assert!(delayed_completion
            .expect_err("stale owner must not complete reassigned task")
            .to_string()
            .contains("changed"));

        let current = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(current.status, TaskStatus::Assigned);
        assert_eq!(current.assigned_worker.as_deref(), Some("worker-b"));
        let attempts = db.get_routing_history(10).unwrap();
        assert!(attempts.iter().all(|attempt| attempt.outcome.is_none()));
    }

    #[test]
    fn p4_38_batch_entry_rejects_worker_with_active_task_without_failing_other_entries() {
        let db = TasksDb::from_connection(
            Connection::open_in_memory().expect("open isolated task database"),
        )
        .expect("register fingerprint function");
        db.migrate().expect("migrate isolated tasks database");

        let active = ready_task(&db, "worker's active task");
        let next = ready_task(&db, "next batch entry");
        let active_entry = crate::routing::TaskRouter::assignment_entry(&active, "worker-a");
        db.assign_with_routing_history(active.id, "worker-a", &active_entry)
            .expect("assign first task");
        db.claim_task_with_pid(active.id, "worker-a", 702)
            .expect("load first task");

        let next_entry = crate::routing::TaskRouter::assignment_entry(&next, "worker-a");
        let refusal = db
            .assign_with_routing_history(next.id, "worker-a", &next_entry)
            .expect_err("a worker already loaded with work cannot receive another entry");
        assert!(refusal.to_string().contains("in-progress task"));
        let untouched = db.get_by_id(next.id).expect("read next entry").unwrap();
        assert_eq!(untouched.status, TaskStatus::Ready);
        assert!(untouched.assigned_worker.is_none());

        // Batch callers continue entry-by-entry: another worker can still
        // receive this same task after the first worker's entry is refused.
        let other_entry = crate::routing::TaskRouter::assignment_entry(&next, "worker-b");
        db.assign_with_routing_history(next.id, "worker-b", &other_entry)
            .expect("another valid entry remains independently successful");
        assert_eq!(
            db.get_by_id(next.id)
                .expect("read successful entry")
                .unwrap()
                .assigned_worker
                .as_deref(),
            Some("worker-b")
        );
    }

    #[test]
    fn p4_claim_metadata_does_not_survive_reassignment_or_claim_exit() {
        let db = TasksDb::open_at(&sandbox("p4-claim-metadata")).expect("open");
        let task = ready_task(&db, "claim metadata lifetime");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 701).unwrap();
        let claimed = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(claimed.claimed_by_pid, Some(701));
        assert!(claimed.claim_host.is_some());

        // Reassignment obeys the same lifecycle as every other writer: return
        // the task to a reassignable state before changing its owner.
        db.update_status(task.id, "failed", Some("worker-a"))
            .unwrap();
        let reassignment = crate::routing::TaskRouter::assignment_entry(&claimed, "worker-b");
        db.assign_with_routing_history(task.id, "worker-b", &reassignment)
            .expect("reassign atomically");
        let reassigned = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(reassigned.status, TaskStatus::Assigned);
        assert_eq!(reassigned.assigned_worker.as_deref(), Some("worker-b"));
        assert_eq!(reassigned.claimed_by_pid, None);
        assert_eq!(reassigned.claim_host, None);

        db.claim_task_with_pid(task.id, "worker-b", 702).unwrap();
        db.insert_build_evidence(task.id, "session", "cargo test", 0, true, "cargo")
            .unwrap();
        move_to_qc_with_synthetic_workspace(&db, task.id, "worker-b").unwrap();
        let in_review = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(in_review.claimed_by_pid, None);
        assert_eq!(in_review.claim_host, None);

        let budgeted = ready_task(&db, "budget releases claim");
        db.update_status(budgeted.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(budgeted.id, "worker-a", 703)
            .unwrap();
        db.mark_task_budget_exceeded(budgeted.id, "test limit")
            .unwrap();
        let held = db.get_by_id(budgeted.id).unwrap().unwrap();
        assert_eq!(held.status, TaskStatus::NeedsInfo);
        assert_eq!(held.claimed_by_pid, None);
        assert_eq!(held.claim_host, None);
    }

    #[test]
    fn stale_orphan_snapshot_cannot_release_a_newer_worker_claim() {
        let db = TasksDb::open_at(&sandbox("p4-stale-orphan-cas")).expect("open");
        let task = ready_task(&db, "claim changes after orphan scan");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 801).unwrap();
        let observed = db.get_orphan_candidates().unwrap().remove(0);

        // Model another recovery path releasing and reassigning the task
        // between orphan detection and the delayed compare-and-set.
        db.release_orphaned_task_if_unchanged(&observed, "prior recovery")
            .unwrap();
        let ready = db.get_by_id(task.id).unwrap().unwrap();
        let reassignment = crate::routing::TaskRouter::assignment_entry(&ready, "worker-b");
        db.assign_with_routing_history(task.id, "worker-b", &reassignment)
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-b", 802).unwrap();

        assert!(!db
            .release_orphaned_task_if_unchanged(&observed, "stale PID result")
            .unwrap());
        let current = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(current.status, TaskStatus::InProgress);
        assert_eq!(current.assigned_worker.as_deref(), Some("worker-b"));
        assert_eq!(current.claimed_by_pid, Some(802));
        assert_eq!(
            current.claim_host.as_deref(),
            observed.claim_host.as_deref()
        );
    }

    #[test]
    fn p4_interrupted_worker_claim_release_is_atomic_and_owner_checked() {
        let db = TasksDb::open_at(&sandbox("p4-interrupted-claim-release")).expect("open");
        let task = ready_task(&db, "interrupted claim");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 811).unwrap();

        assert!(db
            .release_in_progress_claims_for_worker("worker-b", "stale cleanup")
            .unwrap()
            .is_empty());
        let released = db
            .release_in_progress_claims_for_worker("worker-a", "timeout")
            .unwrap();
        assert_eq!(released, vec![task.id]);

        let recovered = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(recovered.status, TaskStatus::Ready);
        assert!(recovered.assigned_worker.is_none());
        assert!(recovered.claimed_by_pid.is_none());
        assert!(recovered.claim_host.is_none());
        assert!(recovered
            .rejection_reason
            .as_deref()
            .unwrap_or_default()
            .contains("timeout"));
    }

    #[test]
    fn p4_interrupted_run_quarantines_exact_claim_for_reconciliation() {
        let db = TasksDb::open_at(&sandbox("p4-interrupted-run-hold")).expect("open");
        let task = ready_task(&db, "reconcile interrupted work before retry");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 812).unwrap();
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        assert!(db
            .hold_interrupted_claims_for_worker_run("worker-b", 812, &host, "test")
            .unwrap()
            .is_empty());
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            TaskStatus::InProgress,
            "another worker must not reconcile this process claim"
        );

        assert_eq!(
            db.hold_interrupted_claims_for_worker_run("worker-a", 812, &host, "test")
                .unwrap(),
            vec![task.id]
        );
        let held = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(held.status, TaskStatus::NeedsInfo);
        assert_eq!(held.assigned_worker.as_deref(), Some("worker-a"));
        assert_eq!(held.claimed_by_pid, None);
        assert_eq!(held.claim_host, None);
        assert!(held
            .rejection_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("[worker recovery]")));
    }

    #[test]
    fn dependency_completion_does_not_resume_budget_exceeded_task() {
        let db = TasksDb::open_at(&sandbox("p4-budget-blocker-priority")).expect("open");
        let blocker = ready_task(&db, "completed dependency");
        let held = ready_task(&db, "budget-held dependent");
        db.update_status(held.id, "needs_info", None)
            .expect("block dependent");
        db.conn
            .execute(
                "UPDATE tasks SET blocked_by = ?1 WHERE id = ?2",
                params![
                    serde_json::json!([blocker.id.to_string()]).to_string(),
                    held.id
                ],
            )
            .expect("persist dependency");
        db.mark_task_budget_exceeded(held.id, "saved budget reached")
            .expect("hold task for explicit budget resolution");
        db.update_status(blocker.id, "assigned", Some("worker-blocker"))
            .expect("assign prerequisite");
        db.claim_task_with_pid(blocker.id, "worker-blocker", 334)
            .expect("claim prerequisite");
        db.update_status(blocker.id, "done", Some("worker-blocker"))
            .expect("complete prerequisite");

        assert!(db.unblock_ready_tasks(blocker.id).unwrap().is_empty());
        let still_held = db.get_by_id(held.id).unwrap().unwrap();
        assert_eq!(still_held.status, TaskStatus::NeedsInfo);
        assert!(still_held.budget_exceeded);
    }

    #[test]
    fn p8_69_direct_claim_release_budget_and_unblock_follow_lifecycle() {
        let db = TasksDb::open_at(&sandbox("p8-69-writers")).expect("open");
        let claimed = ready_task(&db, "self serve");
        let taken = db
            .claim_next_ready("worker", 321)
            .expect("claim")
            .expect("row");
        assert_eq!(taken.id, claimed.id);
        assert_eq!(taken.status, TaskStatus::InProgress);
        let observed = db.get_by_id(taken.id).unwrap().unwrap();
        assert!(db
            .release_orphaned_task_if_unchanged(&observed, "dead process")
            .expect("release"));
        let released = db.get_by_id(taken.id).unwrap().unwrap();
        assert_eq!(released.status, TaskStatus::Ready);
        assert!(released.assigned_worker.is_none());

        let budgeted = ready_task(&db, "budgeted");
        db.mark_task_budget_exceeded(budgeted.id, "limit")
            .expect("budget");
        assert_eq!(
            db.get_by_id(budgeted.id).unwrap().unwrap().status,
            TaskStatus::NeedsInfo
        );
        db.update_status(budgeted.id, "ready", None)
            .expect("resume");

        let epic = db.create_epic("parent", None, "test").expect("epic");
        let child = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "coding".into(),
                phase_order: 1,
                title: "child".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "done".into(),
                blocked_by: vec![],
                milestone: None,
                created_by: Some("test".into()),
            })
            .expect("child");
        let blocked = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "coding".into(),
                phase_order: 1,
                title: "blocked".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "done".into(),
                blocked_by: vec![child.id.to_string()],
                milestone: None,
                created_by: Some("test".into()),
            })
            .expect("blocked child");
        assert_eq!(blocked.status, TaskStatus::NeedsInfo);
        // A needs_info row can retain ownership after budget/recovery paths or
        // an upgrade from the older unconstrained database. Unblocking must
        // clear that stale owner atomically with the return to ready.
        db.conn
            .execute(
                "UPDATE tasks SET assigned_worker = 'worker-stale' WHERE id = ?1",
                params![blocked.id],
            )
            .expect("fixture an owned needs_info task");
        assert!(db.update_status(epic.id, "done", None).is_err());
        db.update_status(child.id, "assigned", Some("worker"))
            .unwrap();
        db.claim_task_with_pid(child.id, "worker", 335).unwrap();
        db.update_status(child.id, "done", Some("worker")).unwrap();
        assert_eq!(db.unblock_ready_tasks(child.id).unwrap(), vec![blocked.id]);
        assert_eq!(
            db.get_by_id(blocked.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
        assert_eq!(
            db.get_by_id(blocked.id).unwrap().unwrap().assigned_worker,
            None,
            "unblocking must clear the owner before the row becomes ready"
        );
        assert_eq!(
            db.get_by_id(epic.id).unwrap().unwrap().status,
            TaskStatus::Ready
        );
        db.update_status(blocked.id, "assigned", Some("worker"))
            .unwrap();
        db.claim_task_with_pid(blocked.id, "worker", 336).unwrap();
        db.update_status(blocked.id, "done", Some("worker"))
            .unwrap();
        assert!(db.unblock_ready_tasks(blocked.id).unwrap().is_empty());
        assert_eq!(
            db.get_by_id(epic.id).unwrap().unwrap().status,
            TaskStatus::Done
        );
    }

    /// `P8-29` — passing `None` must actually unassign.
    ///
    /// 🔴 The tool description promises *"Pass null to unassign"*
    /// (`update_task_status_tool.rs:48`) and the SQL omitted the column
    /// entirely, so the task moved to `ready` while still tagged with its
    /// worker: half-assigned, owned by nobody, and wrong to every query that
    /// reads `assigned_worker` to decide who holds it.
    ///
    /// ⚠ Asserts the FIELD IS CLEARED rather than "the update succeeded" —
    /// the old code returned `Ok(())` while leaving the stale value, so a
    /// test on the return value would have passed over the defect (`L-C14`'s
    /// shape: check that the thing is gone, not that something happened).
    #[test]
    fn p8_29_passing_none_unassigns_rather_than_leaving_the_worker_tag() {
        let path = sandbox("p8-29-unassign");
        let db = TasksDb::open_at(&path).expect("open");
        let task = ready_task(&db, "a task that will be handed back");

        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign");
        let assigned = db.get_by_id(task.id).expect("fetch").expect("row");
        assert_eq!(
            assigned.assigned_worker.as_deref(),
            Some("worker-backend"),
            "the fixture must actually assign, or the unassign below proves nothing"
        );

        db.update_status(task.id, "ready", None).expect("unassign");
        let after = db.get_by_id(task.id).expect("fetch").expect("row");

        assert_eq!(after.status, TaskStatus::Ready);
        assert_eq!(
            after.assigned_worker, None,
            "the task is `ready` but still tagged with {:?} — half-assigned",
            after.assigned_worker
        );

        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// The falsification's other half: an explicit worker must still be set.
    ///
    /// Without this, "always write NULL" would satisfy the test above while
    /// destroying assignment entirely.
    #[test]
    fn p8_29_passing_a_worker_still_assigns() {
        let path = sandbox("p8-29-assign");
        let db = TasksDb::open_at(&path).expect("open");
        let task = ready_task(&db, "a task that will be assigned");

        db.update_status(task.id, "assigned", Some("worker-frontend"))
            .expect("assign");
        let after = db.get_by_id(task.id).expect("fetch").expect("row");
        assert_eq!(after.assigned_worker.as_deref(), Some("worker-frontend"));

        let _ = std::fs::remove_dir_all(path.parent().expect("parent"));
    }

    /// A-22 · eight workers race for twelve tasks and no task is claimed twice.
    ///
    /// This is the test the two-step path could not pass. `arcanactl task next
    /// --claim` used to read the queue, write `assigned`, then claim — and a
    /// worker that lost the race between those writes left the row `assigned`
    /// with no PID, invisible to everything until the orphan detector's 4-hour
    /// TTL. Both failure modes are asserted: no duplicate claim, and no row
    /// stranded in `assigned`.
    ///
    /// Real threads on one real file, each with its own connection — the race
    /// is between SQLite writers, so an in-process mock would prove nothing.
    #[test]
    fn a22_concurrent_claims_never_double_claim_or_strand_a_task() {
        const WORKERS: usize = 8;
        const TASKS: usize = 12;

        let path = sandbox("race");
        {
            let db = TasksDb::open_at(&path).expect("open");
            for i in 0..TASKS {
                ready_task(&db, &format!("task {i}"));
            }
        }

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WORKERS));
        let mut handles = Vec::new();
        for w in 0..WORKERS {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                let db = TasksDb::open_at(&path).expect("open");
                // Release every thread at the same instant; staggered starts
                // would serialise themselves and the test would pass without
                // ever exercising contention.
                barrier.wait();
                db.claim_next_ready(&format!("worker-{w}"), 1000 + w as u32)
                    .expect("claim must not error: the queue is deep enough")
                    .map(|t| t.id)
            }));
        }

        let claimed: Vec<i64> = handles
            .into_iter()
            .map(|h| h.join().expect("thread panicked"))
            .map(|id| id.expect("every worker had a task available"))
            .collect();

        let distinct: std::collections::HashSet<i64> = claimed.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            WORKERS,
            "a task was claimed by more than one worker: {claimed:?}"
        );

        let db = TasksDb::open_at(&path).expect("reopen");
        let stranded = db.list(Some("assigned")).expect("list assigned");
        assert!(
            stranded.is_empty(),
            "{} task(s) left in 'assigned' with no owner — the exact state the two-step claim stranded rows in: {:?}",
            stranded.len(),
            stranded.iter().map(|t| t.id).collect::<Vec<_>>()
        );

        assert_eq!(db.list(Some("in_progress")).expect("list").len(), WORKERS);
        assert_eq!(
            db.list(Some("ready")).expect("list").len(),
            TASKS - WORKERS,
            "the untaken tasks must still be claimable"
        );

        // Every claim recorded its owner: a claim without a PID is precisely
        // what makes orphan detection blind.
        for t in db.list(Some("in_progress")).expect("list") {
            assert!(t.claimed_by_pid.is_some(), "task #{} has no PID", t.id);
            assert!(t.assigned_worker.is_some(), "task #{} has no worker", t.id);
        }
    }

    /// The claim takes only `ready` rows — a task mid-handoff to someone else
    /// is not up for grabs.
    #[test]
    fn a22_claim_ignores_tasks_that_are_not_ready() {
        let path = sandbox("notready");
        let db = TasksDb::open_at(&path).expect("open");
        let t = ready_task(&db, "spoken for");
        db.update_status(t.id, "assigned", Some("someone-else"))
            .expect("assign");

        assert!(
            db.claim_next_ready("latecomer", 4242)
                .expect("claim")
                .is_none(),
            "an assigned task was claimed out from under its worker"
        );
    }

    /// The one-task-per-worker guard survives the move into the statement.
    #[test]
    fn a22_a_worker_holding_a_task_claims_nothing_further() {
        let path = sandbox("guard");
        let db = TasksDb::open_at(&path).expect("open");
        ready_task(&db, "first");
        ready_task(&db, "second");

        let first = db.claim_next_ready("solo", 7).expect("first claim");
        assert!(first.is_some(), "the first claim must succeed");

        let err = db
            .claim_next_ready("solo", 7)
            .expect_err("a worker already holding a task must be refused");
        assert!(
            err.to_string().contains("already owns"),
            "the refusal must say why, got: {err}"
        );

        // And the refusal must not have consumed the second task.
        assert_eq!(
            db.list(Some("ready")).expect("list").len(),
            1,
            "a refused claim still took a task off the queue"
        );
    }

    /// Every column the live `~/.arcana/tasks.db` carries, in its own order.
    ///
    /// A fresh database must match it. The two are built by different paths —
    /// a new install runs only the ladder's baseline rung, while an existing
    /// one was built by the original `CREATE TABLE` plus twenty `ALTER`s — and
    /// nothing else compares them.
    const EXPECTED_COLUMNS: &[&str] = &[
        "id",
        "title",
        "description",
        "priority",
        "task_type",
        "status",
        "acceptance_criteria",
        "assigned_worker",
        "rejection_reason",
        "retry_count",
        "created_by",
        "created_at",
        "updated_at",
        "completed_at",
        "claimed_by_pid",
        "claim_host",
        "diff_analysis",
        "budget_max_tokens",
        "budget_max_usd",
        "budget_max_turns",
        "budget_tokens_used",
        "budget_cost_usd",
        "budget_turns_used",
        "budget_exceeded",
        "budget_exceeded_reason",
        "phase",
        "phase_order",
        "milestone",
        "epic_id",
        "blocked_by",
        "under_review_by",
        "task_qc_cost_usd",
    ];

    /// The regression this guards against was introduced by P1-10 and caught by
    /// an unrelated test.
    ///
    /// `ensure_columns` runs *before* the ladder and returns early when the
    /// table does not exist yet, so a **fresh** database is created entirely by
    /// the baseline rung. The baseline was the original `CREATE TABLE`, which
    /// lacks the twenty columns added later — so new installs came up without
    /// `claimed_by_pid` and seventeen others, while every existing database
    /// looked perfectly fine. No live-data check could have found it.
    /// The wholesale delete must leave a recoverable copy behind. Without this,
    /// "Clear Board" is an unrecoverable action reachable from a single click.
    #[test]
    fn clearing_the_board_backs_up_first_and_the_backup_holds_the_tasks() {
        let dir =
            std::env::temp_dir().join(format!("arcana-clearboard-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
        let path = dir.join("tasks.db");

        let db = TasksDb::open_at(&path).expect("open");
        db.create(CreateTaskInput {
            title: "about to be cleared".into(),
            description: None,
            priority: "p2".into(),
            task_type: "chore".into(),
            acceptance_criteria: String::new(),
            created_by: Some("test".into()),
        })
        .expect("create");

        let (cleared, events) = crate::observability::timing::capture_json_events(|| {
            let _home = crate::db::test_support::with_home(&dir);
            db.clear_all()
        });
        cleared.expect("clear_all");

        assert!(
            db.list(None).expect("list").is_empty(),
            "the board is cleared"
        );

        let backups = dir.join(".arcana").join("backups");
        let found: Vec<_> = std::fs::read_dir(&backups)
            .expect("a backup directory must exist")
            .flatten()
            .map(|e| e.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains("clear-board"))
            })
            .collect();
        assert_eq!(found.len(), 1, "expected one backup, got {found:?}");
        assert!(events.iter().any(|event| {
            event["fields"]["event_type"] == "operation_timing"
                && event["fields"]["operation"] == "database_tasks_clear_all"
                && event["fields"]["outcome"] == "success"
        }));
        assert!(events.iter().any(|event| {
            event["fields"]["event_type"] == "task_board_backup_created"
                && event["fields"]["operation"] == "clear_board_backup"
                && event["fields"]["bytes"].as_u64().unwrap_or_default() > 0
        }));
        let serialized = serde_json::to_string(&events).expect("serialize clear telemetry");
        assert!(!serialized.contains(path.to_string_lossy().as_ref()));
        assert!(!serialized.contains("private"));

        let restored = Connection::open(&found[0]).expect("open backup");
        let title: String = restored
            .query_row("SELECT title FROM tasks", [], |r| r.get(0))
            .expect("the backup must still contain the cleared task");
        assert_eq!(title, "about to be cleared");

        drop(restored);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_fresh_database_has_every_column_an_existing_one_has() {
        let dir =
            std::env::temp_dir().join(format!("arcana-tasks-schema-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
        let path = dir.join("tasks.db");

        let db = TasksDb::open_at(&path).expect("open");
        let actual: Vec<String> = {
            let mut st = db.conn.prepare("PRAGMA table_info(tasks)").unwrap();
            let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            r.map(|c| c.unwrap()).collect()
        };

        let missing: Vec<&str> = EXPECTED_COLUMNS
            .iter()
            .filter(|c| !actual.iter().any(|a| a == *c))
            .copied()
            .collect();
        assert!(
            missing.is_empty(),
            "a fresh tasks.db is missing {missing:?}; the baseline rung and the live schema have diverged"
        );

        let unexpected: Vec<&String> = actual
            .iter()
            .filter(|a| !EXPECTED_COLUMNS.contains(&a.as_str()))
            .collect();
        assert!(
            unexpected.is_empty(),
            "a fresh tasks.db has columns the live schema does not: {unexpected:?}. If they are intended, add them to EXPECTED_COLUMNS."
        );

        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every column named in `ADDED_COLUMNS` must also be in the baseline, or a
    /// fresh install differs from a migrated one.
    #[test]
    fn every_repaired_column_is_also_in_the_baseline() {
        for (name, _) in TasksDb::ADDED_COLUMNS {
            assert!(
                EXPECTED_COLUMNS.contains(name),
                "{name} is repaired by ensure_columns but is not in the expected schema"
            );
        }
    }

    #[test]
    fn production_task_reader_returns_canonical_timestamps_and_legacy_rows_survive_upgrade() {
        let path = sandbox("p4-57-timestamps");
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
        let db = TasksDb::open_at(&path).expect("open isolated production database");
        let fresh = db
            .create(CreateTaskInput {
                title: "canonical timestamp task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "chore".into(),
                acceptance_criteria: String::new(),
                created_by: Some("test".into()),
            })
            .expect("create through production writer");
        for value in [&fresh.created_at, &fresh.updated_at] {
            assert_eq!(value.len(), 24, "expected fixed millisecond UTC: {value}");
            assert!(value.ends_with('Z'), "expected UTC Z suffix: {value}");
            assert!(chrono::DateTime::parse_from_rfc3339(value).is_ok());
        }

        // Model a row written by the pre-P4-57 schema, then reopen through the
        // production migration path. Existing rows are retained byte-for-byte
        // and remain parseable by the shared legacy-compatible reader.
        db.conn
            .execute_batch(
                "DROP TRIGGER tasks_timestamps_canonical_insert;
                 DROP TRIGGER tasks_qc_review_evidence_guard;
                 DROP TRIGGER tasks_qc_review_insert_guard;
                 DROP TRIGGER build_retry_approval_revoke_on_task_change;
                 DROP TRIGGER build_retry_approval_insert_guard;
                 DROP TRIGGER build_retry_approval_update_guard;
                 DROP TRIGGER project_build_batch_retry_approval_guard;
                 DROP INDEX idx_build_retry_approval_one_available;
                 DROP INDEX idx_build_retry_approval_task_history;
                 DROP INDEX idx_project_build_batches_retry_approval;
                 DROP TABLE build_retry_approvals;
                 DROP INDEX idx_build_evidence_batch_id;
                 ALTER TABLE build_evidence DROP COLUMN batch_id;
                 DROP INDEX idx_project_build_batches_task_id;
                 ALTER TABLE project_build_batches DROP COLUMN retry_approval_id;
                 DROP TABLE project_build_batches;
                 ALTER TABLE worker_requests DROP COLUMN diagnostic_batch_id;
                 ALTER TABLE build_evidence DROP COLUMN workspace_fingerprint;
                 ALTER TABLE build_evidence DROP COLUMN workspace_path;
                 PRAGMA user_version = 8;",
            )
            .expect("prepare legacy-format row");
        db.conn
            .execute(
                "INSERT INTO tasks (title, priority, task_type, status, acceptance_criteria,
                                    created_by, created_at, updated_at)
                 VALUES ('legacy timestamp task', 'p2', 'chore', 'ready', '', 'test',
                         '2026-01-02 03:04:05', '2026-01-02 03:04:05')",
                [],
            )
            .expect("insert legacy row");
        db.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint legacy fixture before copying");
        drop(db);

        let upgrade_dir = path.parent().unwrap().join("upgrade-home");
        std::fs::create_dir_all(&upgrade_dir).expect("create isolated upgrade fixture");
        let upgrade_path = upgrade_dir.join("tasks.db");
        std::fs::copy(&path, &upgrade_path).expect("copy the pre-upgrade database fixture");
        drop(_home);
        let _upgrade_home = crate::db::test_support::with_home(&upgrade_dir);
        let upgraded =
            TasksDb::open_at(&upgrade_path).expect("run additive migrations through v20");
        assert_eq!(upgraded.schema_version().unwrap(), 20);
        let blocked_index_exists: bool = upgraded
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = 'idx_tasks_blocked_tasks_plan')",
                [],
                |row| row.get(0),
            )
            .expect("read migrated blocked-task index");
        assert!(
            blocked_index_exists,
            "v17 migration installs blocked-task index"
        );
        let legacy = upgraded
            .list(None)
            .expect("read through production task reader")
            .into_iter()
            .find(|task| task.title == "legacy timestamp task")
            .expect("legacy task preserved");
        assert_eq!(legacy.created_at, "2026-01-02 03:04:05");
        assert_eq!(legacy.updated_at, "2026-01-02 03:04:05");
        assert_eq!(
            crate::db::timestamps::parse_db_timestamp(&legacy.updated_at)
                .expect("legacy timestamp remains readable")
                .to_rfc3339(),
            "2026-01-02T03:04:05+00:00"
        );
        drop(upgraded);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn qc_claim_and_cost_operations_emit_private_result_spans() {
        let path = sandbox("p4-qc-claim-observability");
        let db = TasksDb::open_at(&path).expect("open isolated task DB");
        let task = ready_task(&db, "private-qc-task-title-marker");
        let missing_task_id = task.id + 10_000;

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let claimed = db
                .try_claim_for_qc_review(task.id, "private-qc-subagent-marker")
                .expect("first QC claim");
            let duplicate = db
                .try_claim_for_qc_review(task.id, "private-qc-duplicate-marker")
                .expect("conflicting QC claim");
            db.upgrade_qc_claim(task.id, "private-qc-real-id-marker")
                .expect("upgrade QC claim");
            let claims = db.list_qc_claims().expect("read QC claims");
            db.add_qc_cost(task.id, 123.456789).expect("add QC cost");
            db.add_qc_cost(task.id, 0.0).expect("skip zero QC cost");
            db.release_qc_claim(task.id).expect("release QC claim");
            db.release_qc_claim(missing_task_id)
                .expect("release missing task claim");
            db.upgrade_qc_claim(missing_task_id, "private-qc-missing-id-marker")
                .expect("upgrade missing task claim");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_qc_cost BEFORE UPDATE OF task_qc_cost_usd ON tasks
                     BEGIN SELECT RAISE(ABORT, 'private-qc-cost-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_cost = db.add_qc_cost(task.id, 456.789123).is_err();
            (claimed, duplicate, claims, failed_cost)
        });

        assert!(observed.0);
        assert!(!observed.1);
        assert_eq!(observed.2.len(), 1);
        assert!(observed.3);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_qc_claim_try",
            "database_qc_claim_release",
            "database_qc_claim_upgrade",
            "database_qc_claims_list",
            "database_qc_cost_add",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing QC span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_qc_claim_try"
                && event["fields"]["outcome"] == "not_claimed"
                && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_qc_claim_release"
                && event["fields"]["outcome"] == "not_found"
                && event["fields"]["task_id"] == missing_task_id.to_string()
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_qc_cost_add"
                && event["fields"]["outcome"] == "skipped"
                && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_qc_cost_add"
                && event["fields"]["outcome"] == "error"
        }));
        let serialized = serde_json::to_string(&events).expect("serialize QC telemetry");
        for private_value in [
            "private-qc-task-title-marker",
            "private-qc-subagent-marker",
            "private-qc-duplicate-marker",
            "private-qc-real-id-marker",
            "private-qc-missing-id-marker",
            "private-qc-cost-trigger-marker",
            path.to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "telemetry leaked {private_value}"
            );
        }
        assert!(serialized.contains(&task.id.to_string()));
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn orphan_candidate_scan_distinguishes_empty_rows_and_database_failure() {
        let path = sandbox("p4-orphan-scan-observability");
        let db = TasksDb::open_at(&path).expect("open isolated task DB");
        let task = ready_task(&db, "private-orphan-scan-title-marker");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let empty = db
                .get_orphan_candidates()
                .expect("ready task is not an orphan candidate");
            db.update_status(task.id, "assigned", Some("private-worker-marker"))
                .expect("assign candidate task");
            let assigned = db.get_orphan_candidates().expect("read assigned candidate");
            db.claim_task_with_pid(task.id, "private-worker-marker", 321)
                .expect("claim candidate into progress");
            let in_progress = db.get_orphan_candidates().expect("read orphan candidate");
            db.conn
                .execute_batch("CREATE TEMP TABLE tasks (id INTEGER);")
                .expect("install isolated malformed shadow schema");
            let failed = db.get_orphan_candidates().is_err();
            (empty, assigned, in_progress, failed)
        });

        assert!(observed.0.is_empty());
        assert_eq!(observed.1.len(), 1);
        assert_eq!(observed.2.len(), 1);
        assert!(observed.3);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .filter(|event| event["fields"]["operation"] == "database_task_orphan_candidates_read")
            .collect();
        assert_eq!(operations.len(), 4);
        for outcome in ["empty", "success", "error"] {
            assert!(
                operations.iter().any(|event| {
                    event["fields"]["outcome"] == outcome
                        && event["fields"]["database"] == "tasks"
                        && event["fields"]["duration_ns"].as_u64().is_some()
                }),
                "missing orphan-candidate outcome {outcome}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["outcome"] == "empty" && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["outcome"] == "success" && event["fields"]["result_count"] == 1
        }));
        let serialized = serde_json::to_string(&operations).expect("serialize orphan spans");
        for private_value in [
            "private-orphan-scan-title-marker",
            "private-worker-marker",
            path.to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "orphan telemetry leaked {private_value}"
            );
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn epic_phase_operations_emit_private_result_spans() {
        let path = sandbox("p4-epic-phase-observability");
        let db = TasksDb::open_at(&path).expect("open isolated task DB");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let epic = db
                .create_epic(
                    "private-epic-title-marker",
                    Some("private-epic-description-marker"),
                    "private-epic-creator-marker",
                )
                .expect("create epic");
            let phase_task = db
                .create_phased_task(CreatePhasedTaskInput {
                    epic_id: epic.id.to_string(),
                    phase: "private-phase-name-marker".into(),
                    phase_order: 1,
                    title: "private-phase-task-title-marker".into(),
                    description: Some("private-phase-task-description-marker".into()),
                    priority: "p1".into(),
                    task_type: "feature".into(),
                    acceptance_criteria: "private-phase-criteria-marker".into(),
                    blocked_by: vec![],
                    milestone: Some("private-milestone-marker".into()),
                    created_by: Some("private-phase-creator-marker".into()),
                })
                .expect("create phased task");
            let epic_tasks = db
                .get_epic_tasks(&epic.id.to_string())
                .expect("read epic tasks");
            let summary = db
                .get_phase_summary(&epic.id.to_string())
                .expect("read phase summary");
            let ready = db
                .get_ready_tasks_in_phase(&epic.id.to_string(), "private-phase-name-marker")
                .expect("read ready phase tasks");
            let empty = db
                .get_epic_tasks("private-missing-epic-id-marker")
                .expect("read missing epic tasks");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_epic BEFORE INSERT ON tasks
                     WHEN NEW.title = 'private-failed-epic-title-marker'
                     BEGIN SELECT RAISE(ABORT, 'private-epic-trigger-error-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_create = db
                .create_epic("private-failed-epic-title-marker", None, "test")
                .is_err();
            (
                epic_tasks.len(),
                summary.len(),
                ready.len(),
                empty.len(),
                failed_create,
                phase_task.id,
            )
        });

        assert_eq!(
            (observed.0, observed.1, observed.2, observed.3),
            (1, 1, 1, 0)
        );
        assert!(observed.4);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_epic_create",
            "database_phased_task_create",
            "database_epic_tasks_read",
            "database_phase_summary_read",
            "database_phase_ready_tasks_read",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing epic/phase span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_epic_create"
                && event["fields"]["outcome"] == "error"
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_phase_ready_tasks_read"
                && event["fields"]["result_count"] == 1
                && event["fields"]["task_id"] == ""
        }));
        let serialized = serde_json::to_string(&events).expect("serialize epic telemetry");
        for private_value in [
            "private-epic-title-marker",
            "private-epic-description-marker",
            "private-epic-creator-marker",
            "private-phase-name-marker",
            "private-phase-task-title-marker",
            "private-phase-task-description-marker",
            "private-phase-criteria-marker",
            "private-milestone-marker",
            "private-phase-creator-marker",
            "private-missing-epic-id-marker",
            "private-failed-epic-title-marker",
            "private-epic-trigger-error-marker",
            path.to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "telemetry leaked {private_value}"
            );
        }
        assert!(serialized.contains(&observed.5.to_string()));
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }

    #[test]
    fn task_retry_diff_and_delete_operations_emit_private_result_spans() {
        let path = sandbox("p4-task-retry-diff-observability");
        let db = TasksDb::open_at(&path).expect("open isolated task DB");
        let task = ready_task(&db, "private-retry-task-title-marker");
        let missing_id = task.id + 10_000;
        let analysis = r#"{"risk":"private-diff-analysis-marker","path":"C:/private/path-marker"}"#;

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            db.increment_retry(task.id).expect("increment retry");
            let retries = db.get_retry_count(task.id).expect("read retry count");
            db.set_task_diff_analysis(task.id, analysis)
                .expect("store diff analysis");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_diff BEFORE UPDATE OF diff_analysis ON tasks
                     BEGIN SELECT RAISE(ABORT, 'private-diff-trigger-error-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_diff = db
                .set_task_diff_analysis(task.id, "private-failed-diff-marker")
                .is_err();
            db.delete(task.id).expect("delete task");
            db.delete(task.id).expect("delete missing task");
            db.increment_retry(missing_id)
                .expect("increment missing task retry");
            (
                retries,
                failed_diff,
                db.get_by_id(task.id).unwrap().is_none(),
            )
        });

        assert_eq!(observed.0, 1);
        assert!(observed.1);
        assert!(observed.2);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_task_retry_increment",
            "database_task_retry_count_read",
            "database_task_diff_analysis_write",
            "database_task_delete",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing task span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_task_diff_analysis_write"
                && event["fields"]["outcome"] == "error"
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_task_delete"
                && event["fields"]["outcome"] == "not_found"
        }));
        let serialized = serde_json::to_string(&events).expect("serialize task telemetry");
        for private_value in [
            "private-retry-task-title-marker",
            "private-diff-analysis-marker",
            "C:/private/path-marker",
            "private-failed-diff-marker",
            "private-diff-trigger-error-marker",
            path.to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "telemetry leaked {private_value}"
            );
        }
        let _ = std::fs::remove_dir_all(path.parent().expect("test directory"));
    }
}
