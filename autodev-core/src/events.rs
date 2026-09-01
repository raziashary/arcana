// The event boundary between AutoDev's core and whatever is hosting it.
//
// Core emits events. It must not know that a Tauri window exists.
//
// Before P1 the only way to emit was `crate::events::emit_*(&AppHandle, …)`,
// backed by a process-wide `OnceLock<AppHandle>`. That single type is what made
// `agent/`, `recipes/` and `subagent/` unmovable: 52 call sites across those
// three directories each needed a `tauri::AppHandle`, so extracting them into a
// UI-free crate was impossible while the signature demanded one.
//
// `EventSink` is that boundary. Core depends on the trait; the host injects an
// implementation. The desktop app installs one that forwards to Tauri; the CLI
// installs one that does nothing, or logs. Neither is core's concern.
//
// ⚠ Honest limitation, stated rather than glossed: the sink is installed once
// into a process-global, mirroring the architecture it replaces. That removes
// the *Tauri type* from core — which is what unblocks the extraction — but it
// does not yet make the sink per-instance injectable, which would mean
// threading it through every constructor in the agent runtime. That further
// step is real work and is not pretended to be done here.
//
// See ADR-LOG-RESTRUCT-0011.

use serde::Serialize;
use std::sync::{Arc, OnceLock};

/// A host that can deliver events to a user interface.
///
/// Implementors must be cheap to call and must never panic: core calls this
/// from inside agent turns and tool execution, where a panic would take down
/// work that has nothing to do with the UI.
pub trait EventSink: Send + Sync {
    /// Deliver one event. `name` is the event's stable identifier
    /// (e.g. `"tool:start"`); `payload` is its already-serialised body.
    fn emit(&self, name: &str, payload: serde_json::Value);
}

/// A sink that discards everything.
///
/// The default before a host installs one, and the right choice for `airsctl`
/// and for tests. Emitting into the void is correct behaviour for a headless
/// process — it is not an error, and it must not be logged as one on every
/// event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullEventSink;

impl EventSink for NullEventSink {
    fn emit(&self, _name: &str, _payload: serde_json::Value) {}
}

static SINK: OnceLock<Arc<dyn EventSink>> = OnceLock::new();

/// Install the host's sink. The first call wins; later calls are ignored.
///
/// Returns whether this call installed it, so a host can tell the difference
/// between "I configured the process" and "something already had". Silently
/// returning either way would hide a double-initialisation bug.
pub fn install_sink(sink: Arc<dyn EventSink>) -> bool {
    SINK.set(sink).is_ok()
}

/// The installed sink, or a null sink when the host installed none.
///
/// Never `None`: a core call site must not have to decide what to do when
/// there is no UI. That decision belongs to the host, and its default is
/// "discard".
pub fn sink() -> Arc<dyn EventSink> {
    SINK.get()
        .cloned()
        .unwrap_or_else(|| Arc::new(NullEventSink) as Arc<dyn EventSink>)
}

/// Emit an event with an already-built payload.
pub fn emit(name: &str, payload: serde_json::Value) {
    sink().emit(name, payload);
}

/// Emit an event whose payload is a serialisable value.
///
/// Serialisation failure is logged and dropped rather than propagated: an
/// event that cannot be encoded is a bug in the payload type, and it must not
/// abort the agent turn that happened to be reporting progress.
pub fn emit_serialized<T: serde::Serialize>(name: &str, payload: &T) {
    match serde_json::to_value(payload) {
        Ok(value) => sink().emit(name, value),
        Err(e) => tracing::warn!("[events] dropping '{name}': payload did not serialise: {e}"),
    }
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

// Subagent lifecycle events. Status transitions (Spawning / Idle / Running / Completed / Failed /
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

pub fn emit_token(payload: TokenPayload) {
    emit_serialized(EVT_AGENT_TOKEN, &payload);
}

pub fn emit_thinking(payload: ThinkingPayload) {
    emit_serialized(EVT_AGENT_THINKING, &payload);
}

pub fn emit_tool_start(payload: ToolStartPayload) {
    emit_serialized(EVT_AGENT_TOOL_START, &payload);
}

pub fn emit_tool_end(payload: ToolEndPayload) {
    emit_serialized(EVT_AGENT_TOOL_END, &payload);
}

pub fn emit_permission_request(payload: PermissionPayload) {
    emit_serialized(EVT_AGENT_PERMISSION, &payload);
}

pub fn emit_agent_done(payload: AgentDonePayload) {
    emit_serialized(EVT_AGENT_DONE, &payload);
}

pub fn emit_agent_error(payload: AgentErrorPayload) {
    emit_serialized(EVT_AGENT_ERROR, &payload);
}

pub fn emit_cost_update(payload: CostUpdatePayload) {
    emit_serialized(EVT_COST_UPDATE, &payload);
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

pub fn emit_worker_status(payload: WorkerStatusPayload) {
    emit_serialized(EVT_WORKER_STATUS, &payload);
}

pub fn emit_worker_tick_start(payload: WorkerTickStartPayload) {
    emit_serialized(EVT_WORKER_TICK_START, &payload);
}

pub fn emit_worker_tick_end(payload: WorkerTickEndPayload) {
    emit_serialized(EVT_WORKER_TICK_END, &payload);
}

// ── Doc 06 Phase 3 — worker log-line breadcrumb ───────────────────────────────
#[derive(Debug, Serialize, Clone)]
pub struct WorkerLogLinePayload {
    pub slug: String,
    pub text: String,
    pub timestamp: String,
}

pub fn emit_worker_log_line(payload: WorkerLogLinePayload) {
    emit_serialized(EVT_WORKER_LOG_LINE, &payload);
}

// ── Doc 07 Phase 3/4 — workspace startup events ──────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct WorkspaceStraysPayload {
    pub strays: Vec<crate::observability::stray_files::StrayFile>,
}

pub fn emit_workspace_strays(payload: WorkspaceStraysPayload) {
    emit_serialized(EVT_WORKSPACE_STRAYS, &payload);
}

#[derive(Debug, Serialize, Clone)]
pub struct WorkspaceMismatchPayload {
    pub current_dir: String,
    pub configured_workspace: String,
    pub timestamp: String,
}

pub fn emit_workspace_mismatch(payload: WorkspaceMismatchPayload) {
    emit_serialized(EVT_WORKSPACE_MISMATCH, &payload);
}

#[derive(Debug, Serialize, Clone)]
pub struct SidecarCrashPayload {
    pub sidecar_id: String, // "core-orchestrator" / "memory-mcp-server"
    pub restart_count: u32,
    pub window_secs: u64,
    pub stderr_tail: Vec<String>,
    pub timestamp: String,
}

pub fn emit_sidecar_crash(payload: SidecarCrashPayload) {
    emit_serialized(EVT_SIDECAR_CRASH, &payload);
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

pub fn emit_worker_request(payload: WorkerRequestPayload) {
    emit_serialized(EVT_WORKER_REQUEST, &payload);
}

// ── Phase 3.0 R1 payloads + emit helpers ─────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct TaskChangedPayload {
    pub task_id: Option<i64>,
    pub kind: String,
    pub status: Option<String>,
    pub assigned_worker: Option<String>,
}

pub fn emit_task_changed(payload: TaskChangedPayload) {
    emit_serialized(EVT_TASK_CHANGED, &payload);
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

pub fn emit_worker_message(payload: WorkerMessagePayload) {
    emit_serialized(EVT_WORKER_MESSAGE, &payload);
}

// ── Phase 3.5 payloads + emit helpers ──────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct PlanChangedPayload {
    pub plan_id: Option<String>,
    pub kind: String,
    pub status: Option<String>,
}

pub fn emit_plan_changed(payload: PlanChangedPayload) {
    emit_serialized(EVT_PLAN_CHANGED, &payload);
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

pub fn emit_qc_decision(payload: QcDecisionPayload) {
    emit_serialized(EVT_QC_DECISION, &payload);
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

pub fn emit_subagent_event(payload: SubagentEventPayload) {
    emit_serialized(EVT_SUBAGENT_EVENT, &payload);
}

// ── Ruflo Feature 2 — Orphan detection events ─────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct OrphansReleasedPayload {
    pub task_ids: Vec<String>,
    pub count: usize,
}

pub fn emit_orphans_released(task_ids: Vec<String>) {
    let count = task_ids.len();
    emit_serialized(
        EVT_TASK_ORPHANS_RELEASED,
        &OrphansReleasedPayload { task_ids, count },
    );
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

pub fn emit_budget_exceeded(payload: BudgetExceededEvent) {
    emit_serialized(EVT_AGENT_BUDGET_EXCEEDED, &payload);
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

pub fn emit_task_unblocked(task_id: i64) {
    emit_serialized(EVT_TASK_UNBLOCKED, &TaskUnblockedPayload { task_id });
}

pub fn emit_phase_complete(epic_id: String, phase: String, milestone: Option<String>) {
    emit_serialized(
        EVT_TASK_PHASE_COMPLETE,
        &PhaseCompletePayload {
            epic_id,
            phase,
            milestone,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Recorder {
        seen: Mutex<Vec<(String, serde_json::Value)>>,
    }

    impl EventSink for Recorder {
        fn emit(&self, name: &str, payload: serde_json::Value) {
            self.seen.lock().unwrap().push((name.to_string(), payload));
        }
    }

    #[test]
    fn a_null_sink_discards_without_panicking() {
        let s = NullEventSink;
        s.emit("anything", serde_json::json!({"a": 1}));
    }

    #[test]
    fn an_uninstalled_sink_still_returns_something_usable() {
        // The property that matters: a core call site never has to handle
        // "there is no UI". Emitting before any host installs a sink must be a
        // no-op, not a panic and not an error the caller has to thread.
        emit("before-install", serde_json::json!({}));
    }

    #[test]
    fn a_recorder_sees_name_and_payload() {
        let rec = Arc::new(Recorder::default());
        rec.emit("tool:start", serde_json::json!({"tool": "bash"}));

        let seen = rec.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, "tool:start");
        assert_eq!(seen[0].1["tool"], "bash");
    }

    #[test]
    fn a_payload_that_cannot_serialise_is_dropped_not_propagated() {
        // f64::NAN is not representable in JSON. The event is dropped and the
        // caller continues — an agent turn must not fail because a progress
        // event could not be encoded.
        #[derive(serde::Serialize)]
        struct Bad {
            v: f64,
        }
        emit_serialized("bad", &Bad { v: f64::NAN });
    }
}
