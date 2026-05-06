// Tauri event emission helpers.
// All agent → frontend communication uses named Tauri events.
// The frontend subscribes to these via @tauri-apps/api listen().

use std::sync::OnceLock;
use serde::Serialize;
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

pub const EVT_AGENT_TOKEN:       &str = "agent:token";
pub const EVT_AGENT_THINKING:    &str = "agent:thinking";
pub const EVT_AGENT_TOOL_START:  &str = "agent:tool-start";
pub const EVT_AGENT_TOOL_END:    &str = "agent:tool-end";
pub const EVT_AGENT_PERMISSION:  &str = "agent:permission-request";
pub const EVT_AGENT_DONE:        &str = "agent:done";
pub const EVT_AGENT_ERROR:       &str = "agent:error";
pub const EVT_COST_UPDATE:       &str = "cost:update";

// Phase 3.0 R2 — fleet status push channel. Replaces polling-based derivation
// of worker status in the Agent Roster. Emitted from RecipeRunner on every
// transition (working / idle / error) and at tick boundaries.
pub const EVT_WORKER_STATUS:     &str = "worker:status";
pub const EVT_WORKER_TICK_START: &str = "worker:tick-start";
pub const EVT_WORKER_TICK_END:   &str = "worker:tick-end";

// Phase 3.0 R1 — task board changes push channel. Emitted from every task DB
// write path so the AutoDev Panel refreshes without polling.
pub const EVT_TASK_CHANGED:      &str = "task:changed";

// Phase 3.5 — plan changes push channel. Emitted from every plan DB write.
pub const EVT_PLAN_CHANGED:      &str = "plan:changed";

// Phase 4.9 — subagent lifecycle events. One event per SSE frame received
// from core-orchestrator's `/api/agents/{id}/invoke/stream`, plus internal
// status transitions (Spawning / Idle / Running / Completed / Failed /
// Terminated). Fan-out is handled by SubagentManager::pump_events.
pub const EVT_SUBAGENT_EVENT:    &str = "subagent:event";

// ── Payload types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TokenPayload {
    pub session_id: String,
    pub text:       String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThinkingPayload {
    pub session_id: String,
    pub text:       String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolStartPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub input:      serde_json::Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToolEndPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub output:     String,
    pub is_error:   bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct PermissionPayload {
    pub session_id: String,
    pub call_id:    String,
    pub tool_name:  String,
    pub input:      serde_json::Value,
    pub description: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentDonePayload {
    pub session_id:    String,
    pub message_id:    String,
    pub tokens_in:     u32,
    pub tokens_out:    u32,
    pub cost_usd:      f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct AgentErrorPayload {
    pub session_id: String,
    pub message:    String,
}

#[derive(Debug, Serialize, Clone)]
pub struct CostUpdatePayload {
    pub session_id:   String,
    pub tokens_in:    u32,
    pub tokens_out:   u32,
    pub cost_usd:     f64,
    pub total_today:  f64,
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
    pub slug:               String,
    pub status:             String,
    pub previous_status:    Option<String>,
    pub reason:             Option<String>,
    pub cooldown_seconds:   Option<u32>,
    pub timestamp:          String,
    pub tick_duration_ms:   Option<u64>,   // wall-clock ms of the last agent tick
    pub last_outcome:       Option<String>, // "success" | "fallback" | "error"
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerTickStartPayload {
    pub slug:      String,
    pub mode:      String,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkerTickEndPayload {
    pub slug:      String,
    pub outcome:   String,
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

// ── Phase 3.0 R1 payloads + emit helpers ─────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TaskChangedPayload {
    pub task_id: Option<i64>,
    pub kind:    String,
    pub status:  Option<String>,
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
    }
}

pub fn emit_worker_status_global(payload: WorkerStatusPayload) {
    if let Some(app) = app_handle() {
        emit_worker_status(app, payload);
    }
}

// ── Phase 3.5 payloads + emit helpers ──────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PlanChangedPayload {
    pub plan_id: Option<String>,
    pub kind:    String,
    pub status:  Option<String>,
}

pub fn emit_plan_changed(app: &AppHandle, payload: PlanChangedPayload) {
    let _ = app.emit(EVT_PLAN_CHANGED, payload);
}

pub fn emit_plan_changed_global(payload: PlanChangedPayload) {
    if let Some(app) = app_handle() {
        emit_plan_changed(app, payload);
    }
}

// ── Phase 4.9 payloads + emit helpers ─────────────────────────────────────────

/// Wraps a `SubagentEvent` for the global event bus. The event itself
/// carries its own discriminator (kind + subagent_id) inside `event`, so
/// the frontend can route by parent/subagent and render appropriately.
#[derive(Debug, Serialize, Clone)]
pub struct SubagentEventPayload {
    pub subagent_id: String,
    pub event:       crate::subagent::SubagentEvent,
}

pub fn emit_subagent_event(app: &AppHandle, payload: SubagentEventPayload) {
    let _ = app.emit(EVT_SUBAGENT_EVENT, payload);
}

pub fn emit_subagent_event_global(payload: SubagentEventPayload) {
    if let Some(app) = app_handle() {
        emit_subagent_event(app, payload);
    }
}
