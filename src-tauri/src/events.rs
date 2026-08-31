// Tauri event emission helpers.
// All agent → frontend communication uses named Tauri events.
// The frontend subscribes to these via @tauri-apps/api listen().

use serde::Serialize;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

// Global AppHandle installed from main.rs after setup. Lets non-Tauri
// subsystems (task-board tools, recipes, hooks) emit fleet events without
// having to thread `AppHandle` through every call site. Fleet tools run in
// background tokio tasks with no Tauri `State<>` access, so this is the
// minimal indirection required for Phase 3.0 R1/R2.
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Install the global AppHandle exactly once. Called from main.rs `.setup()`.
pub fn install_app_handle(app: AppHandle) {
    let _ = APP_HANDLE.set(app);
}

/// Access the global AppHandle. Returns None if emission is attempted before
/// setup completes (tests, early boot). Callers must no-op on None.
pub fn app_handle() -> Option<&'static AppHandle> {
    APP_HANDLE.get()
}

// ── Event name constants ──────────────────────────────────────────────────────

pub const EVT_AGENT_TOKEN: &str = "agent:token";
pub const EVT_AGENT_THINKING: &str = "agent:thinking";
pub const EVT_AGENT_TOOL_START: &str = "agent:tool-start";
pub const EVT_AGENT_TOOL_END: &str = "agent:tool-end";
pub const EVT_AGENT_PERMISSION: &str = "agent:permission-request";
pub const EVT_AGENT_DONE: &str = "agent:done";
pub const EVT_AGENT_ERROR: &str = "agent:error";
pub const EVT_COST_UPDATE: &str = "cost:update";

// Phase 3.0 R2 — fleet status push channel. Replaces polling-based derivation
// of worker status in the Agent Roster. Emitted from RecipeRunner on every
// transition (working / idle / error) and at tick boundaries.
pub const EVT_WORKER_STATUS: &str = "worker:status";
pub const EVT_WORKER_TICK_START: &str = "worker:tick-start";
pub const EVT_WORKER_TICK_END: &str = "worker:tick-end";
pub const EVT_WORKER_REQUEST: &str = "worker:request";

// Doc 06 Phase 3 — free-form one-line breadcrumb pushed into a worker's
// Live Monitor card. Used by the qc-agent skip path to leave a visible
// "idle — no qc_review tasks" trail instead of an empty card body.
pub const EVT_WORKER_LOG_LINE: &str = "worker:log-line";

// Phase 3.0 R1 — task board changes push channel. Emitted from every task DB
// write path so the AutoDev Panel refreshes without polling.
pub const EVT_TASK_CHANGED: &str = "task:changed";

// Phase 3.5 — plan changes push channel. Emitted from every plan DB write.
pub const EVT_PLAN_CHANGED: &str = "plan:changed";

// Ruflo Feature 2 — PID orphan detection. Fired when the Supervisor poll cycle
// releases one or more stale tasks back to the ready queue.
pub const EVT_TASK_ORPHANS_RELEASED: &str = "task:orphans-released";

// Ruflo Feature 4 — Budget Circuit Breaker. Fired when an agent loop halts
// because a per-task token, cost, or turn budget has been exhausted.
pub const EVT_AGENT_BUDGET_EXCEEDED: &str = "agent:budget-exceeded";

// Ruflo Feature 5 — Phase-Based Task Decomposition. Fired when a blocked task
// is unblocked (all its prerequisites are done) or when an entire phase of an
// epic is complete (all subtasks in that phase are done).
pub const EVT_TASK_UNBLOCKED: &str = "task:unblocked";
pub const EVT_TASK_PHASE_COMPLETE: &str = "task:phase-complete";

// Ruflo Feature 6 — MCP Connection Health Monitor. Emitted by McpHealthMonitor
// on each ping cycle and on connection state transitions.
pub const EVT_MCP_HEALTH_UPDATE: &str = "mcp:health-update";
pub const EVT_MCP_CONNECTION_LOST: &str = "mcp:connection-lost";
pub const EVT_MCP_CONNECTION_DEGRADED: &str = "mcp:connection-degraded";
pub const EVT_MCP_RECONNECTED: &str = "mcp:reconnected";
pub const EVT_MCP_RECONNECT_FAILED: &str = "mcp:reconnect-failed";
pub const EVT_MCP_REQUIRES_RESTART: &str = "mcp:requires-restart";

// In-process MCP server port conflict. Emitted ONCE at startup when the
// preferred port (8767) is held by another process and we had to fall
// back to 8768/8769/8770. Payload is `McpPortStatus`.
pub const EVT_MCP_PORT_CONFLICT: &str = "mcp:port-conflict";

// Phase 4.9 — subagent lifecycle events. One event per SSE frame received
// from core-orchestrator's `/api/agents/{id}/invoke/stream`, plus internal
// status transitions (Spawning / Idle / Running / Completed / Failed /
// Terminated). Fan-out is handled by SubagentManager::pump_events.
pub const EVT_SUBAGENT_EVENT: &str = "subagent:event";

// Phase 4.9b — QC approve/reject decisions surfaced as UI toasts.
pub const EVT_QC_DECISION: &str = "qc:decision";

// Doc 07 Phase 3/4 — workspace-related startup signals.
//   • workspace:strays-detected — list of stray worker-generated files
//     found in src-tauri/ or src/ at startup. Frontend banner renders
//     them with "delete" / "move to workspace" affordances.
//   • workspace:mismatch — current_dir() differs from the configured
//     workspace. Surfaces as a warning banner with a "set workspace" CTA.
pub const EVT_WORKSPACE_STRAYS: &str = "workspace:strays-detected";
pub const EVT_WORKSPACE_MISMATCH: &str = "workspace:mismatch";

// Doc 05 Phase 1 — sidecar circuit-breaker open. The watchdog gives up on
// restart attempts after N crashes in a sliding window; this event
// surfaces the captured stderr tail to the frontend so the user can copy
// a triage report without opening the log file.
pub const EVT_SIDECAR_CRASH: &str = "sidecar:crash";

// AutoDev Communication Protocol (ACP) — structured worker↔worker message.
// Fires once per insert into `worker_messages`. Powers the Fleet Chat panel
// and the per-worker "N unread" badge on AgentLiveCard.
pub const EVT_WORKER_MESSAGE: &str = "worker:message";

// ── Payload types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TokenPayload {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThinkingPayload {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolStartPayload {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolEndPayload {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct PermissionPayload {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentDonePayload {
    pub session_id: String,
    pub message_id: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentErrorPayload {
    pub session_id: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CostUpdatePayload {
    pub session_id: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
    pub total_today: f64,
}

// ── Emit helpers ──────────────────────────────────────────────────────────────

pub fn emit_token(app: &AppHandle, payload: TokenPayload) {
    let _ = app.emit(EVT_AGENT_TOKEN, payload);
}

pub fn emit_thinking(app: &AppHandle, payload: ThinkingPayload) {
    let _ = app.emit(EVT_AGENT_THINKING, payload);
}

pub fn emit_tool_start(app: &AppHandle, payload: ToolStartPayload) {
    let _ = app.emit(EVT_AGENT_TOOL_START, payload);
}

pub fn emit_tool_end(app: &AppHandle, payload: ToolEndPayload) {
    let _ = app.emit(EVT_AGENT_TOOL_END, payload);
}

pub fn emit_permission_request(app: &AppHandle, payload: PermissionPayload) {
    let _ = app.emit(EVT_AGENT_PERMISSION, payload);
}

pub fn emit_agent_done(app: &AppHandle, payload: AgentDonePayload) {
    let _ = app.emit(EVT_AGENT_DONE, payload);
}

pub fn emit_agent_error(app: &AppHandle, payload: AgentErrorPayload) {
    let _ = app.emit(EVT_AGENT_ERROR, payload);
}

pub fn emit_cost_update(app: &AppHandle, payload: CostUpdatePayload) {
    let _ = app.emit(EVT_COST_UPDATE, payload);
}

// ── Phase 3.0 R2 payloads + emit helpers ─────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct WorkerStatusPayload {
    pub slug: String,
    pub status: String,
    pub previous_status: Option<String>,
    pub reason: Option<String>,
    pub cooldown_seconds: Option<u32>,
    pub timestamp: String,
    pub tick_duration_ms: Option<u64>, // wall-clock ms of the last agent tick
    pub last_outcome: Option<String>,  // "success" | "fallback" | "error"
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerTickStartPayload {
    pub slug: String,
    pub mode: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerTickEndPayload {
    pub slug: String,
    pub outcome: String,
    pub timestamp: String,
}

pub fn emit_worker_status(app: &AppHandle, payload: WorkerStatusPayload) {
    let _ = app.emit(EVT_WORKER_STATUS, payload);
}

pub fn emit_worker_tick_start(app: &AppHandle, payload: WorkerTickStartPayload) {
    let _ = app.emit(EVT_WORKER_TICK_START, payload);
}

pub fn emit_worker_tick_end(app: &AppHandle, payload: WorkerTickEndPayload) {
    let _ = app.emit(EVT_WORKER_TICK_END, payload);
}

// ── Doc 06 Phase 3 — worker log-line breadcrumb ───────────────────────────────
#[derive(Debug, Serialize, Clone)]
pub struct WorkerLogLinePayload {
    pub slug: String,
    pub text: String,
    pub timestamp: String,
}

pub fn emit_worker_log_line(app: &AppHandle, payload: WorkerLogLinePayload) {
    let _ = app.emit(EVT_WORKER_LOG_LINE, payload);
}

pub fn emit_worker_log_line_global(payload: WorkerLogLinePayload) {
    if let Some(app) = app_handle() {
        emit_worker_log_line(app, payload);
    }
}

// ── Doc 07 Phase 3/4 — workspace startup events ──────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct WorkspaceStraysPayload {
    pub strays: Vec<crate::observability::stray_files::StrayFile>,
}

pub fn emit_workspace_strays_global(payload: WorkspaceStraysPayload) {
    if let Some(app) = app_handle() {
        let _ = app.emit(EVT_WORKSPACE_STRAYS, payload);
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkspaceMismatchPayload {
    pub current_dir: String,
    pub configured_workspace: String,
    pub timestamp: String,
}

pub fn emit_workspace_mismatch_global(payload: WorkspaceMismatchPayload) {
    if let Some(app) = app_handle() {
        let _ = app.emit(EVT_WORKSPACE_MISMATCH, payload);
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SidecarCrashPayload {
    pub sidecar_id: String, // "core-orchestrator" / "memory-mcp-server"
    pub restart_count: u32,
    pub window_secs: u64,
    pub stderr_tail: Vec<String>,
    pub timestamp: String,
}

pub fn emit_sidecar_crash_global(payload: SidecarCrashPayload) {
    if let Some(app) = app_handle() {
        let _ = app.emit(EVT_SIDECAR_CRASH, payload);
    }
}

// ── Worker Request payloads + emit helpers ───────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct WorkerRequestPayload {
    pub id: i64,
    pub worker_slug: String,
    pub task_id: Option<i64>,
    pub request_type: String,
    pub status: String,
    pub payload: String,
}

pub fn emit_worker_request(app: &AppHandle, payload: WorkerRequestPayload) {
    let _ = app.emit(EVT_WORKER_REQUEST, payload);
}

pub fn emit_worker_request_global(payload: WorkerRequestPayload) {
    if let Some(app) = app_handle() {
        emit_worker_request(app, payload);
    }
}

// ── Phase 3.0 R1 payloads + emit helpers ─────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TaskChangedPayload {
    pub task_id: Option<i64>,
    pub kind: String,
    pub status: Option<String>,
    pub assigned_worker: Option<String>,
}

pub fn emit_task_changed(app: &AppHandle, payload: TaskChangedPayload) {
    let _ = app.emit(EVT_TASK_CHANGED, payload);
}

// ── Global emit helpers (use installed AppHandle) ────────────────────────────
// These exist so background subsystems (task DB tools, hooks, schedulers)
// can fire events without being plumbed an AppHandle. They silently no-op
// when `install_app_handle` hasn't run yet — expected during boot and tests.

pub fn emit_task_changed_global(payload: TaskChangedPayload) {
    if let Some(app) = app_handle() {
        emit_task_changed(app, payload);
    } else {
        // BUG-005 — previously silent; now logged so we can detect race conditions
        // where a task DB write happens before the frontend listener mounts.
        tracing::warn!(
            "emit_task_changed_global skipped: AppHandle not installed yet. \
             Task change may not appear in UI until next poll."
        );
    }
}

pub fn emit_worker_status_global(payload: WorkerStatusPayload) {
    if let Some(app) = app_handle() {
        emit_worker_status(app, payload);
    }
}

// ── AutoDev Communication Protocol payload + emit helpers ────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct WorkerMessagePayload {
    pub message_id: i64,
    pub from: String,
    pub to: String,
    pub body: String,
    pub kind: String,
    pub task_id: Option<i64>,
    pub timestamp: String,
}

pub fn emit_worker_message(app: &AppHandle, payload: WorkerMessagePayload) {
    let _ = app.emit(EVT_WORKER_MESSAGE, payload);
}

pub fn emit_worker_message_global(payload: WorkerMessagePayload) {
    if let Some(app) = app_handle() {
        emit_worker_message(app, payload);
    } else {
        tracing::warn!(
            "emit_worker_message_global skipped: AppHandle not installed. \
             Worker message #{} from '{}' to '{}' will not appear in Fleet Chat \
             until next reload.",
            payload.message_id,
            payload.from,
            payload.to
        );
    }
}

// ── Phase 3.5 payloads + emit helpers ──────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PlanChangedPayload {
    pub plan_id: Option<String>,
    pub kind: String,
    pub status: Option<String>,
}

pub fn emit_plan_changed(app: &AppHandle, payload: PlanChangedPayload) {
    let _ = app.emit(EVT_PLAN_CHANGED, payload);
}

pub fn emit_plan_changed_global(payload: PlanChangedPayload) {
    if let Some(app) = app_handle() {
        emit_plan_changed(app, payload);
    }
}

// ── Phase 4.9b QC decision payloads + emit helpers ───────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct QcDecisionPayload {
    pub task_id: i64,
    pub task_title: String,
    pub decision: String, // "approved" | "rejected"
    pub rejection_reason: Option<String>,
    pub worker_slug: String,
}

pub fn emit_qc_decision(app: &AppHandle, payload: QcDecisionPayload) {
    let _ = app.emit(EVT_QC_DECISION, payload);
}

pub fn emit_qc_decision_global(payload: QcDecisionPayload) {
    if let Some(app) = app_handle() {
        emit_qc_decision(app, payload);
    }
}

// ── Phase 4.9 payloads + emit helpers ─────────────────────────────────────────

/// Wraps a `SubagentEvent` for the global event bus. The event itself
/// carries its own discriminator (kind + subagent_id) inside `event`, so
/// the frontend can route by parent/subagent and render appropriately.
#[derive(Debug, Serialize, Clone)]
pub struct SubagentEventPayload {
    pub subagent_id: String,
    pub event: crate::subagent::SubagentEvent,
}

pub fn emit_subagent_event(app: &AppHandle, payload: SubagentEventPayload) {
    let _ = app.emit(EVT_SUBAGENT_EVENT, payload);
}

pub fn emit_subagent_event_global(payload: SubagentEventPayload) {
    if let Some(app) = app_handle() {
        emit_subagent_event(app, payload);
    }
}

// ── Ruflo Feature 2 — Orphan detection events ─────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct OrphansReleasedPayload {
    pub task_ids: Vec<String>,
    pub count: usize,
}

pub fn emit_orphans_released_global(task_ids: Vec<String>) {
    if let Some(app) = app_handle() {
        let count = task_ids.len();
        let _ = app.emit(
            EVT_TASK_ORPHANS_RELEASED,
            OrphansReleasedPayload { task_ids, count },
        );
    }
}

// ── Ruflo Feature 4 — Budget Circuit Breaker events ───────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct BudgetExceededEvent {
    pub session_id: String,
    pub task_id: Option<i64>,
    pub reason: String, // "BUDGET_EXCEEDED_TOKENS" | "BUDGET_EXCEEDED_USD" | "BUDGET_EXCEEDED_TURNS"
    pub tokens_used: u64,
    pub cost_usd: f64,
    pub turns_used: u32,
}

pub fn emit_budget_exceeded(app: &AppHandle, payload: BudgetExceededEvent) {
    let _ = app.emit(EVT_AGENT_BUDGET_EXCEEDED, payload);
}

pub fn emit_budget_exceeded_global(payload: BudgetExceededEvent) {
    if let Some(app) = app_handle() {
        emit_budget_exceeded(app, payload);
    }
}

// ── Ruflo Feature 5 — Phase-Based Task Decomposition events ───────────────────

/// Emitted for each task that transitions from needs_info → ready when its
/// blockers have all been completed.
#[derive(Debug, Serialize, Clone)]
pub struct TaskUnblockedPayload {
    pub task_id: i64,
}

/// Emitted once when the last task in a phase reaches status=done, signalling
/// that the next phase's tasks may now be assigned.
#[derive(Debug, Serialize, Clone)]
pub struct PhaseCompletePayload {
    pub epic_id: String,
    pub phase: String,
    pub milestone: Option<String>,
}

pub fn emit_task_unblocked_global(task_id: i64) {
    if let Some(app) = app_handle() {
        let _ = app.emit(EVT_TASK_UNBLOCKED, TaskUnblockedPayload { task_id });
    }
}

pub fn emit_phase_complete_global(epic_id: String, phase: String, milestone: Option<String>) {
    if let Some(app) = app_handle() {
        let _ = app.emit(
            EVT_TASK_PHASE_COMPLETE,
            PhaseCompletePayload {
                epic_id,
                phase,
                milestone,
            },
        );
    }
}
