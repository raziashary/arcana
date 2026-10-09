// Tool executor — handles permission checks + concurrent/serial execution.
// Read-only tools run concurrently (tokio tasks, max 10).
// Write tools run serially with user confirmation.
// Pattern from Claude Code: read-only concurrent, writes serial.

use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};

use super::execution_registry::{ActiveTool, ExecutionGuard};
use super::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use super::tool_trait::{AgentIdentity, Tool, ToolContext, ToolResult};
use crate::errors::Result;
use crate::events::{
    emit_agent_stage_change, emit_agent_tool_call_result, emit_agent_tool_call_start,
    emit_permission_request, emit_tool_end, emit_tool_start, stage_for_tool,
    AgentStageChangePayload, AgentToolCallResultPayload, AgentToolCallStartPayload,
    PermissionPayload, ToolEndPayload, ToolStartPayload,
};
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};
use crate::platform::process_sandbox;
use crate::session::manager::SessionManager;
use crate::session::types::Message;
use crate::state::lock::LockTolerant;

// Max concurrent read-only tools (Claude Code uses 10)
/// How long an unanswered permission prompt waits before it is refused.
///
/// The wait used to be unbounded, and the listener dying — a tab switch does it
/// — blocked the engine permanently. Five minutes is long enough that a user who
/// stepped away still gets to answer, and short enough that an abandoned session
/// frees itself.
pub(crate) const PERMISSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

const MAX_CONCURRENT_READ_TOOLS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermissionDecision {
    Allow,
    AllowAlways,
    Deny,
    Cancelled,
    TimedOut,
    ChannelClosed,
}

struct JournalIntent {
    run_id: String,
    cursor: i64,
    target_paths: Vec<String>,
    operation: String,
}

async fn wait_for_session_cancel(
    state: Option<Arc<std::sync::Mutex<crate::state::AppState>>>,
    session_id: String,
) {
    let Some(state) = state else {
        std::future::pending::<()>().await;
        return;
    };
    loop {
        if state.lock_tolerant().is_cancelled(&session_id) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn wait_for_permission_response(
    rx: oneshot::Receiver<PermissionResponse>,
    state: Option<Arc<std::sync::Mutex<crate::state::AppState>>>,
    session_id: String,
    timeout: std::time::Duration,
) -> PermissionDecision {
    tokio::select! {
        answer = rx => match answer {
            Ok(PermissionResponse::Allow) => PermissionDecision::Allow,
            Ok(PermissionResponse::AllowAlways) => PermissionDecision::AllowAlways,
            Ok(PermissionResponse::Deny) => PermissionDecision::Deny,
            Err(_) => PermissionDecision::ChannelClosed,
        },
        _ = async {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                if state
                    .as_ref()
                    .map(|state| state.lock_tolerant().is_cancelled(&session_id))
                    .unwrap_or(false)
                {
                    break;
                }
            }
        } => PermissionDecision::Cancelled,
        _ = tokio::time::sleep(timeout) => PermissionDecision::TimedOut,
    }
}

/// Persist one intent before a run-scoped side effect. Journal labels and paths
/// are deliberately derived from fixed tool metadata; arguments, source,
/// command text, and returned content never enter runs.db.
fn begin_operation_journal(
    tool_name: &str,
    effect: &crate::agent::tool_effect::ToolEffect,
    input: &serde_json::Value,
    ctx: &ToolContext,
) -> Result<Option<JournalIntent>> {
    let Some(run_id) = ctx.run_id.as_deref() else {
        return Ok(None);
    };
    if *effect == crate::agent::tool_effect::ToolEffect::Read {
        return Ok(None);
    }

    let operation = match tool_name {
        "write_file"
        | "edit_file"
        | "append_file"
        | "annotate_file"
        | "bash"
        | "project_command"
        | "create_task"
        | "assign_task"
        | "assign_tasks_batch"
        | "claim_task"
        | "update_task_status"
        | "send_worker_message"
        | "propose_memory"
        | "git_commit"
        | "write_interface_spec"
        | "mcp_list_tools"
        | "mcp_call_tool" => tool_name.to_owned(),
        _ => "external_tool".to_owned(),
    };
    let target_paths = journal_target_paths(tool_name, input, ctx);
    let task_id = journal_task_id(tool_name, input, ctx);
    let operation_id = uuid::Uuid::new_v4().to_string();
    let runs = crate::db::runs::RunsDb::open()?;
    let cursor = runs.record_operation_intent(
        run_id,
        task_id,
        &operation_id,
        &operation,
        &format!("{effect:?}"),
        &target_paths,
    )?;
    Ok(Some(JournalIntent {
        run_id: run_id.to_owned(),
        cursor,
        target_paths,
        operation,
    }))
}

fn journal_task_id(tool_name: &str, input: &serde_json::Value, ctx: &ToolContext) -> Option<i64> {
    if tool_name == "claim_task" {
        input
            .get("task_id")
            .and_then(serde_json::Value::as_i64)
            .filter(|task_id| *task_id > 0)
            .or(ctx.task_id)
    } else {
        ctx.task_id
    }
}

fn journal_target_paths(
    tool_name: &str,
    input: &serde_json::Value,
    ctx: &ToolContext,
) -> Vec<String> {
    let key = match tool_name {
        "write_file" | "edit_file" | "append_file" => "path",
        "annotate_file" => "file_path",
        "project_command" => "project_path",
        _ => return Vec::new(),
    };
    let Some(raw_path) = input.get(key).and_then(serde_json::Value::as_str) else {
        return Vec::new();
    };
    let root = match crate::platform::workspace::resolve_workspace_dir(Some(&ctx.working_dir)) {
        Ok(root) => root,
        Err(_) => return Vec::new(),
    };
    let target = Path::new(raw_path);
    let target = if target.is_absolute() {
        target.to_path_buf()
    } else {
        root.join(target)
    };
    let Ok(target) =
        super::builtin_tools::path_jail::validate_workspace_path(&target, &ctx.working_dir)
    else {
        return Vec::new();
    };
    let Ok(relative) = target.strip_prefix(&root) else {
        return Vec::new();
    };
    let parts = relative
        .components()
        .map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let Some(parts) = parts else {
        return Vec::new();
    };
    if parts.is_empty() {
        return Vec::new();
    }
    vec![parts.join("/")]
}

fn finish_operation_journal(intent: JournalIntent, result: ToolResult) -> ToolResult {
    let changed_paths = if !result.is_error
        && matches!(
            intent.operation.as_str(),
            "write_file" | "edit_file" | "append_file"
        ) {
        intent.target_paths.clone()
    } else {
        Vec::new()
    };
    let outcome_unknown = serde_json::from_str::<serde_json::Value>(&result.output)
        .ok()
        .map(|value| {
            value
                .get("error")
                .and_then(|error| error.get("outcome_unknown"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .unwrap_or(false);
    let outcome = if outcome_unknown {
        crate::db::runs::OperationOutcome::Unknown
    } else if result.is_error {
        crate::db::runs::OperationOutcome::Error
    } else {
        crate::db::runs::OperationOutcome::Ok
    };
    let recorded = crate::db::runs::RunsDb::open().and_then(|runs| {
        runs.record_operation_outcome(&intent.run_id, intent.cursor, outcome, &changed_paths)
            .map(|_| ())
    });
    if recorded.is_ok() {
        return result;
    }

    tracing::error!(
        target: "arcana::telemetry",
        event_type = "operation_journal_outcome_failed",
        operation = %intent.operation,
        outcome = "unknown",
        error_class = "runs_db",
        "A side effect returned but its durable outcome could not be recorded"
    );
    let mut unknown = ToolResult::categorized_error(
        "side_effect_outcome_unknown",
        "reconciliation_required",
        "The operation may have completed, but Arcana could not persist its observed outcome. Do not repeat it yet.",
        "Inspect the operation journal, task status, workspace and Git state, then reconcile before retrying or reassigning.",
    );
    if let Ok(mut envelope) = serde_json::from_str::<serde_json::Value>(&unknown.output) {
        envelope["error"]["retry_safe"] = serde_json::Value::Bool(false);
        unknown.output = envelope.to_string();
    }
    unknown
}

pub struct ToolExecutor {
    mode: PermissionMode,
    pending: PendingPermissions,
    session_id: String,
    working_dir: String,
    /// Keeps the workspace's OS process sandbox alive for this agent run.
    /// Sequential Bash and project-command launches then reuse the same
    /// AppContainer grants; dropping the executor releases them after child
    /// jobs finish.
    _workspace_sandbox_lease: process_sandbox::WorkspaceLease,
    /// Whether a live host can answer per-call approval requests.
    host_approval_available: bool,
    /// Who this executor runs as, and how deeply nested. Threaded into every
    /// ToolContext it builds so identity is never absent and depth is never
    /// self-reported by the model.
    identity: AgentIdentity,
    task_id: Option<i64>,
    run_id: Option<String>,
    run_token_budget: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
    // Tools auto-approved for this session (AllowAlways responses)
    always_allow: std::collections::HashSet<String>,
    // Session manager for persisting tool messages
    session_mgr: Option<Arc<SessionManager>>,
    /// Shared application state, used **only** to observe cancellation
    /// (`P3-08`). The permission wait must notice the user cancelling while a
    /// prompt is open, and this is the same signal every other cancellation
    /// path reads — a second mechanism here would be a second thing to forget.
    ///
    /// `Option` because `arcanactl` and the tests build an executor with no
    /// application state; there the timeout is the only bound.
    state: Option<Arc<std::sync::Mutex<crate::state::AppState>>>,

    // ── The procedural-memory seam (`P8-40`) ─────────────────────────────────
    //
    // `P4-44` enables this only for in-process fleet workers. Interactive
    // agents and subagents keep the default-off behavior.
    //
    // ⚠ **Off by default, and that is the point.** A recall that fires for
    // every agent would widen the prompt for agents that gain nothing from it,
    // and `TG3-08` bounds input growth.
    /// Whether to consult procedural memory for this turn.
    procedural_recall: bool,
    /// The block retrieved for the current turn, or `None`.
    ///
    /// ⚠ **Once per turn, never per call.** `engine.rs` calls `execute_batch`
    /// exactly once per turn with the whole batch, so retrieving at the top of
    /// that function is once-per-turn *by construction* rather than by a
    /// caller remembering to. The fleet makes many calls per tick; a per-call
    /// recall would multiply the query count by the batch size and grow the
    /// prompt with it.
    procedural_block: Option<String>,
    /// How many times the seam has actually queried, for the whole life of this
    /// executor. Exists so the once-per-turn property is **observable** rather
    /// than asserted in a comment.
    procedural_queries: u32,
}

impl ToolExecutor {
    pub fn new(
        mode: PermissionMode,
        pending: PendingPermissions,
        session_id: String,
        working_dir: String,
        identity: AgentIdentity,
    ) -> Self {
        let workspace_sandbox_lease = process_sandbox::retain_workspace(Path::new(&working_dir));
        Self {
            mode,
            pending,
            session_id,
            working_dir,
            _workspace_sandbox_lease: workspace_sandbox_lease,
            host_approval_available: true,
            identity,
            task_id: None,
            run_id: None,
            run_token_budget: None,
            always_allow: std::collections::HashSet::new(),
            session_mgr: None,
            state: None,
            procedural_recall: false,
            procedural_block: None,
            procedural_queries: 0,
        }
    }

    /// Turn the procedural-memory seam on for this executor (`P8-40`).
    ///
    /// Off unless a caller asks. `P4-44` is the task that asks.
    pub fn with_procedural_recall(mut self, on: bool) -> Self {
        self.procedural_recall = on;
        self
    }

    pub fn set_task_id(&mut self, task_id: Option<i64>) {
        self.task_id = task_id;
    }

    pub fn with_run_id(mut self, run_id: Option<String>) -> Self {
        self.run_id = run_id;
        self
    }

    pub fn with_run_token_budget(
        mut self,
        ledger: Arc<crate::budget::run_tokens::RunTokenLedger>,
    ) -> Self {
        self.run_token_budget = Some(ledger);
        self
    }

    /// The procedural block retrieved for the current turn, if any.
    pub fn procedural_block(&self) -> Option<&str> {
        self.procedural_block.as_deref()
    }

    /// Take the rendered block for one provider request so it cannot be
    /// repeated on later turns without a new retrieval.
    pub fn take_procedural_block(&mut self) -> Option<String> {
        self.procedural_block.take()
    }

    /// How many times this executor has queried procedural memory.
    ///
    /// ⚠ Public so a test can assert **once per turn** by counting, rather
    /// than by reading the code and believing it.
    pub fn procedural_queries(&self) -> u32 {
        self.procedural_queries
    }

    /// Consult procedural memory for this turn. One query, whatever the batch.
    ///
    /// The query is built from the tool names the model actually asked for,
    /// which is what `P4-44` means by "scoped to the tools the model actually
    /// invoked" — a recall keyed on the whole conversation would return the
    /// same rows every turn.
    fn retrieve_procedural(&mut self, tool_names: &[String]) {
        if !self.procedural_recall {
            return;
        }
        let query = tool_names.join(" ");
        if query.trim().is_empty() {
            return;
        }
        self.procedural_queries += 1;
        // Routed through `recall.rs` so the text is flattened and framed the
        // same way every other recalled memory is (`P8-41`).
        self.procedural_block = crate::memory::recall::procedural_context(&query, 3);
    }

    /// Set the session manager for persisting tool messages
    /// Observe cancellation through the shared application state (`P3-08`).
    pub fn with_state(mut self, state: Arc<std::sync::Mutex<crate::state::AppState>>) -> Self {
        self.state = Some(state);
        self
    }

    /// Mark whether permission requests can reach an interactive host.
    pub fn with_host_approval_available(mut self, available: bool) -> Self {
        self.host_approval_available = available;
        self
    }

    pub fn with_session_manager(mut self, session_mgr: Arc<SessionManager>) -> Self {
        self.session_mgr = Some(session_mgr);
        self
    }

    /// Persist a tool message to the database
    fn save_tool_message(
        &self,
        call_id: &str,
        tool_name: &str,
        input: serde_json::Value,
        output: &str,
        is_error: bool,
    ) {
        if let Some(ref session_mgr) = self.session_mgr {
            let tool_msg = Message::new_tool(
                &self.session_id,
                call_id,
                tool_name,
                input,
                output,
                is_error,
            );
            if let Err(e) = session_mgr.append_message(&tool_msg) {
                error!("Failed to save tool message: {}", e);
            } else {
                debug!("Saved tool message: call_id={} tool={}", call_id, tool_name);
            }
        }
    }

    /// Execute a tool call with permission check.
    /// Returns the tool result payload.
    /// Redact a tool result on its way out of the executor.
    ///
    /// Applied at **every** `tool.execute` site rather than at one downstream
    /// point, because there is no single downstream point: `execute` and both
    /// branches of `execute_batch` each have their own path to the
    /// conversation, the `messages` table and the UI event.
    ///
    /// P1 wired only `execute` — which has **no production caller**. The agent
    /// loop calls `execute_batch` (`agent/engine.rs`, in `AgentEngine::run`), so redaction was
    /// live in dead code while every real tool call carried raw output to the
    /// model, the database and the renderer. The phase's own smoke test drove
    /// the unused method and passed, and the gate was a substring check over
    /// the file, so both proofs were green. Found by three independent
    /// post-phase auditors.
    fn redact_result(mut result: ToolResult) -> ToolResult {
        result.output = crate::security::redaction::redact_tool_output(&result.output);
        result
    }

    /// Convert a handler panic into the same bounded tool-result contract as
    /// ordinary handler failures. Keeping the panic inside the per-call path
    /// lets the executor emit/persist the result and continue later calls.
    async fn execute_handler(
        tool: &dyn Tool,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> ToolResult {
        match AssertUnwindSafe(tool.execute(input, ctx)).catch_unwind().await {
            Ok(result) => Self::redact_result(result),
            Err(_) => ToolResult::error(
                "Tool handler panicked before returning a result. Inspect the tool inputs and current task state, then retry only after checking whether the operation had side effects.",
            ),
        }
    }

    /// The form of a tool's arguments that is safe to report.
    ///
    /// The tool itself is given the raw input — it needs the real value to do
    /// its work. This is the copy that goes to the UI event, the hook context
    /// and the `messages` table. `G4:486` recorded unredacted tool arguments
    /// reaching procedural memory verbatim, and memory is permanent.
    fn reported_input(input: &serde_json::Value) -> serde_json::Value {
        crate::security::redaction::redact_args(input)
    }

    // ⚠ The former single-call `ToolExecutor::execute()` path was deleted in `P3-10`.
    //
    // It had **zero callers**. The agent loop calls `execute_batch`, which is
    // the only live path — and that divergence was not academic: for a whole
    // phase the redaction boundary was wired to `execute()`, so every real tool
    // call carried raw output to the model, the database and the UI while the
    // smoke test drove the dead method and the gate matched a substring inside
    // it (`L-C12`). The same shape recurred here: `execute()` consulted
    // `is_read_only()` as its auto-approval key while `execute_batch`'s read
    // partition consulted nothing at all.
    //
    // Deleted rather than left unreferenced, per `TG3-07`: *an unreferenced
    // second path is a defect waiting for a caller.* Two implementations of one
    // security decision drift, and the drift is invisible until a gate that
    // exists on one path is missing from the other.

    /// Execute a batch of tools with partitioned concurrency.
    /// The **one** permission decision in the codebase (`P3-11`, `P3-64`).
    ///
    /// ── What this replaced ───────────────────────────────────────────────────
    ///
    /// There were two divergent gates and one missing one:
    ///
    ///   * `execute()` matched on the mode and consulted `is_read_only()` — a
    ///     **concurrency hint** — as the auto-approval key;
    ///   * the write partition of `execute_batch()` matched on the mode too, but
    ///     differently, without the `is_read_only` clause;
    ///   * and the **read partition had no permission code at all**. It ran
    ///     under the comment `// Execute (read-only always approved)`, having
    ///     cloned `self.mode` and `self.always_allow` into the task and bound
    ///     them as `_mode` / `_always_allow` — underscore-prefixed to silence
    ///     the unused warning.
    ///
    /// `execute_batch` is the only path the agent loop calls. So on the live
    /// path, `is_read_only() -> true` was an **unconditional bypass of every
    /// permission mode** — and `check_file` declared it while executing
    /// binaries resolved from inside the workspace under inspection.
    ///
    /// Keyed on `ToolEffect` now, in one place, so a reviewer can read the
    /// whole policy without reconciling three copies.
    async fn check_permission(
        &mut self,
        call_id: &str,
        tool: &Arc<dyn Tool>,
        input: &serde_json::Value,
    ) -> Result<PermissionDecision> {
        let effect = tool.effect_for_input(input);
        // A standing per-tool allowance the user granted earlier in the session.
        // Free-form shell and host-controlled installation commands are
        // intentionally per-call consent: the former is unconstrained model
        // text, and the latter can run untrusted package setup with host egress.
        if effect.supports_standing_allow() && self.always_allow.contains(tool.name()) {
            return Ok(PermissionDecision::Allow);
        }

        if !effect.requires_prompt(&self.mode) {
            return Ok(PermissionDecision::Allow);
        }

        self.request_permission(call_id, tool, input).await
    }

    /// Read-only tools run concurrently (max 10), write tools run serially.
    /// Returns results in the same order as input.
    /// Pattern from Claude Code's partitionToolCalls() + runTools().
    #[tracing::instrument(
        target = "arcana::telemetry",
        skip_all,
        fields(event_type = "trace_span", operation = "agent_tool_batch", session_id = %self.session_id)
    )]
    pub async fn execute_batch(
        &mut self,
        tools: Vec<(Arc<dyn Tool>, serde_json::Value, String)>, // (tool, input, tool_use_id)
    ) -> Result<Vec<(String, ToolResult)>> {
        // (tool_use_id, result)
        if tools.is_empty() {
            return Ok(Vec::new());
        }

        // ── P8-40 · the procedural seam, once for the whole batch ────────────
        //
        // Here, and not inside the per-call loop below. `engine.rs` calls this
        // function once per turn with every call the model asked for, so one
        // retrieval here is one retrieval per turn no matter how wide the
        // batch. No-op unless `with_procedural_recall(true)`.
        let tool_names: Vec<String> = tools.iter().map(|(t, _, _)| t.name().to_string()).collect();
        let execution_call_ids = tools
            .iter()
            .map(|(_, _, tool_use_id)| format!("{}-{tool_use_id}", self.session_id))
            .collect::<Vec<_>>();
        self.retrieve_procedural(&tool_names);

        // ── P3-64 · permission is decided BEFORE the partition ───────────────
        //
        // The read partition previously ran with no permission check of any
        // kind, and it is the partition `is_read_only()` selects — so declaring
        // read-only was an unconditional bypass.
        //
        // Deciding here, sequentially, before anything is spawned, gives three
        // things the old shape could not: every tool passes exactly one gate;
        // concurrent tools cannot race to open competing approval dialogs; and
        // the order the user is asked in matches the order the model asked.
        let total_calls = tools.len();
        let mut approved_tools = Vec::with_capacity(total_calls);
        let mut refused: Vec<(usize, String, ToolResult)> = Vec::new();

        for (idx, (tool, input, tool_use_id)) in tools.into_iter().enumerate() {
            let call_id = execution_call_ids[idx].clone();
            if tool.effect_for_input(&input) == crate::agent::tool_effect::ToolEffect::HostApproval
                && !self.host_approval_available
            {
                tracing::warn!(
                    target: "arcana::telemetry",
                    event_type = "permission_unavailable",
                    tool = tool.name(),
                    session_id = %self.session_id,
                    call_id = %call_id,
                    outcome = "denied",
                    "Host approval requested without an interactive approval channel"
                );
                refused.push((
                    idx,
                    tool_use_id,
                    ToolResult::categorized_error(
                        "permission_unavailable",
                        "permission",
                        format!(
                            "Host approval is unavailable in this headless process, so `{}` was not run.",
                            tool.name()
                        ),
                        "Use an interactive desktop host with an approval channel.",
                    ),
                ));
                continue;
            }
            match self.check_permission(&call_id, &tool, &input).await? {
                PermissionDecision::Allow | PermissionDecision::AllowAlways => {
                    approved_tools.push((tool, input, tool_use_id, call_id, idx));
                }
                decision => {
                    // A refusal is a RESULT, not a dropped call. Dropping it
                    // would leave the assistant message carrying a `tool_use`
                    // with no matching `tool_result`. Keep distinct stable
                    // outcomes so the model and run telemetry can distinguish
                    // a user decision from timeout, cancellation, or a dead
                    // approval listener. All remain fail-closed.
                    let result = match decision {
                        PermissionDecision::Deny => ToolResult::categorized_error(
                            "permission_denied",
                            "permission",
                            format!(
                                "The user declined permission for `{}` to {}.",
                                tool.name(),
                                tool.effect_for_input(&input).describe()
                            ),
                            "Continue only with an operation covered by the granted permission.",
                        ),
                        PermissionDecision::Cancelled => ToolResult::categorized_error(
                            "operation_cancelled",
                            "cancellation",
                            "The session was cancelled while waiting for tool approval; the tool was not run.",
                            "Wait for the user to resume the session before requesting another operation.",
                        ),
                        PermissionDecision::TimedOut => ToolResult::categorized_error(
                            "permission_timed_out",
                            "timeout",
                            "The tool approval request expired without a response; the tool was not run.",
                            "Request approval again only after confirming the user is available.",
                        ),
                        PermissionDecision::ChannelClosed => ToolResult::categorized_error(
                            "permission_channel_closed",
                            "infrastructure",
                            "The approval listener closed before responding; the tool was not run.",
                            "Restore the host approval listener before retrying this operation.",
                        ),
                        PermissionDecision::Allow | PermissionDecision::AllowAlways => {
                            unreachable!("allow is handled above")
                        }
                    };
                    refused.push((idx, tool_use_id, result));
                }
            }
        }

        // Partition the approved calls: concurrency-safe ones run together.
        let mut read_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, String, usize)> =
            Vec::new();
        let mut write_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, String, usize)> =
            Vec::new();

        for (tool, input, tool_use_id, call_id, idx) in approved_tools {
            if tool.effect_for_input(&input).is_concurrency_safe() {
                read_tools.push((tool, input, tool_use_id, call_id, idx));
            } else {
                write_tools.push((tool, input, tool_use_id, call_id, idx));
            }
        }

        // ── P3-57 · every slot is pre-keyed with its REAL tool_use_id ────────
        //
        // This was `resize(n, (String::new(), error("placeholder")))` — every
        // slot pre-filled with an **empty** id. When a tool panicked, the panic
        // handler logged and left the placeholder in place (its own comment
        // said *"Placeholder will be replaced; we need to find which one
        // failed"*), so the slot kept `tool_use_id = ""`. The engine then
        // inserted it under key `""`, `by_id.remove(real_id)` missed, and the
        // assistant message went out carrying a `tool_use` with **no matching
        // `tool_result`** — a hard 400 from the provider on the next request.
        // With two panicking tools they collided on `""` in the map and all but
        // one were silently dropped. Concurrency is 10.
        //
        // Sized to the ORIGINAL call count, not the approved count, because
        // refused calls are placed at their original index — and pre-keyed, so
        // a panic or a refusal leaves a correctly-addressed error rather than
        // an orphan.
        let mut results: Vec<(String, ToolResult)> = Vec::with_capacity(total_calls);
        results.resize(
            total_calls,
            (
                String::new(),
                ToolResult::error("the tool did not run and reported no reason"),
            ),
        );
        for (tool, _input, tool_use_id, _, idx) in read_tools.iter().chain(write_tools.iter()) {
            let _ = tool;
            results[*idx] = (
                tool_use_id.clone(),
                ToolResult::error("the tool did not complete"),
            );
        }
        for (idx, tool_use_id, result) in refused {
            results[idx] = (tool_use_id, result);
        }

        // Execute read-only tools concurrently with semaphore
        if !read_tools.is_empty() {
            debug!(
                "Executing {} read-only tools concurrently (max {})",
                read_tools.len(),
                MAX_CONCURRENT_READ_TOOLS
            );

            let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_READ_TOOLS));
            let mut read_handles: Vec<
                JoinHandle<(usize, String, ToolResult, String, String, serde_json::Value)>,
            > = Vec::new();

            for (tool, input, tool_use_id, call_id, original_idx) in read_tools {
                let sem_clone = Arc::clone(&semaphore);
                let session_id = self.session_id.clone();
                let working_dir = self.working_dir.clone();
                // (`_mode` / `_always_allow` were cloned here and never used —
                // underscore-prefixed to silence the warning. The permission
                // decision now happens before the partition, so there is
                // nothing for this task to consult. P3-64.)
                let _pending = self.pending.clone();
                let identity = self.identity.clone();
                let task_id = self.task_id;
                let run_id = self.run_id.clone();
                let run_token_budget = self.run_token_budget.clone();
                let state = self.state.clone();
                // G-5. The permission DECISION happens before this partition
                // (P3-64), but the child's ToolContext still has to carry the
                // mode: `spawn_subagent` reads it to stop a subagent
                // outranking its parent.
                let mode = self.mode.clone();

                let handle = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();

                    let ctx = ToolContext {
                        session_id: session_id.clone(),
                        working_dir: working_dir.clone(),
                        call_id: call_id.clone(),
                        run_id: run_id.clone(),
                        run_token_budget: run_token_budget.clone(),
                        depth: identity.depth,
                        agent_role: identity.role.clone(),
                        permission_mode: mode.clone(),
                        task_id,
                    };

                    // Redaction boundary — see `redact_result`. The tool is
                    // given the raw input; everything reported is redacted.
                    let reported = Self::reported_input(&input);
                    let execution = ExecutionGuard::start(ActiveTool {
                        session_id: session_id.clone(),
                        agent_id: identity.role.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        input: reported.clone(),
                    });

                    // Emit start event
                    emit_tool_start(ToolStartPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        input: reported.clone(),
                    });

                    // The Live Monitor's own channel: agent-keyed, not
                    // session-keyed (`P8-66`).
                    emit_agent_tool_call_start(AgentToolCallStartPayload {
                        agent_id: identity.role.clone(),
                        tool_call_id: call_id.clone(),
                        name: tool.name().to_string(),
                        args: reported.to_string(),
                    });
                    if let Some(stage) = stage_for_tool(tool.name()) {
                        emit_agent_stage_change(AgentStageChangePayload {
                            agent_id: identity.role.clone(),
                            stage: stage.to_string(),
                            status: "current".to_string(),
                            elapsed_ms: None,
                            reason: None,
                        });
                    }
                    let started = std::time::Instant::now();
                    let mut telemetry =
                        crate::observability::timing::OperationTimer::start("agent_tool_call")
                            .with_session_id(&session_id)
                            .with_task_id(task_id)
                            .with_call_context(&call_id, &identity.role);

                    // Execute. Permission was decided before the partition
                    // (P3-64) — this comment used to read "read-only always
                    // approved", which described the bypass rather than a
                    // policy.
                    let cancellation_session = session_id.clone();
                    let raw_result = tokio::select! {
                        biased;
                        _ = wait_for_session_cancel(state, cancellation_session) => {
                            ToolResult::categorized_error(
                                "operation_cancelled",
                                "cancellation",
                                "Arcana cancelled this read-only operation before it completed.",
                                "Wait for the session to resume before requesting another operation.",
                            )
                        }
                        result = Self::execute_handler(tool.as_ref(), input.clone(), &ctx) => result,
                    };
                    let result = raw_result.with_execution_context_and_input(
                        tool.name(),
                        &session_id,
                        &identity.role,
                        task_id,
                        &call_id,
                        run_id.as_deref(),
                        Some(&input),
                    );
                    if !result.is_error {
                        telemetry.succeed();
                    }
                    tracing::info!(
                        target: "arcana::telemetry",
                        event_type = "tool_call_result",
                        agent_role = %identity.role,
                        tool_name = tool.name(),
                        call_id = %call_id,
                        task_id = task_id,
                        is_error = result.is_error,
                        output_bytes = result.output.len(),
                        "Agent tool call returned"
                    );

                    // Get tool name for persistence
                    let tool_name = tool.name().to_string();

                    // Emit end event
                    emit_tool_end(ToolEndPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool_name.clone(),
                        output: result.output.clone(),
                        is_error: result.is_error,
                    });
                    execution.finish(result.is_error);

                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    emit_agent_tool_call_result(AgentToolCallResultPayload {
                        agent_id: identity.role.clone(),
                        tool_call_id: call_id.clone(),
                        result: result.output.clone(),
                        is_error: result.is_error,
                        elapsed_ms,
                    });
                    if let Some(stage) = stage_for_tool(&tool_name) {
                        emit_agent_stage_change(AgentStageChangePayload {
                            agent_id: identity.role.clone(),
                            stage: stage.to_string(),
                            status: if result.is_error { "error" } else { "done" }.to_string(),
                            elapsed_ms: Some(elapsed_ms),
                            reason: None,
                        });
                    }

                    (
                        original_idx,
                        tool_use_id,
                        result,
                        call_id,
                        tool_name,
                        reported,
                    )
                });

                read_handles.push(handle);
            }

            // Collect concurrent results
            for handle in read_handles {
                match handle.await {
                    // Bound as `reported`, not `input`: the task returns the
                    // REDACTED copy. It was named `input` here, which reads
                    // like a leak and invites someone to "restore" the raw
                    // value into the messages table.
                    Ok((idx, tool_use_id, result, call_id, tool_name, reported)) => {
                        // Persist tool message
                        if let Some(ref session_mgr) = self.session_mgr {
                            let tool_msg = Message::new_tool(
                                &self.session_id,
                                &call_id,
                                &tool_name,
                                reported,
                                &result.output,
                                result.is_error,
                            );
                            if let Err(e) = session_mgr.append_message(&tool_msg) {
                                error!("Failed to save tool message (concurrent): {}", e);
                            }
                        }
                        results[idx] = (tool_use_id, result);
                    }
                    Err(e) => {
                        error!("Read-only tool task panicked: {}", e);
                        // Placeholder will be replaced; we need to find which one failed
                    }
                }
            }
        }

        // Execute write tools serially (preserve order, respect permissions)
        if !write_tools.is_empty() {
            debug!("Executing {} write tools serially", write_tools.len());

            for (tool, input, tool_use_id, call_id, original_idx) in write_tools {
                // Redaction boundary. The tool is given the raw input; the UI
                // event, the hook context and the `messages` row all get this.
                //
                // ⚠ The first A-01 fix redacted this branch's *result* and left
                // its *input* raw, so the write path still carried credentials
                // to the renderer and the database. Found while wiring A-08.
                let reported = Self::reported_input(&input);
                let ctx = ToolContext {
                    session_id: self.session_id.clone(),
                    working_dir: self.working_dir.clone(),
                    // G-5: the child must not be able to outrank its parent.
                    // G-5: the child must not be able to outrank its parent.
                    permission_mode: self.mode.clone(),
                    call_id: call_id.clone(),
                    run_id: self.run_id.clone(),
                    run_token_budget: self.run_token_budget.clone(),
                    depth: self.identity.depth,
                    agent_role: self.identity.role.clone(),
                    task_id: self.task_id,
                };
                let execution = ExecutionGuard::start(ActiveTool {
                    session_id: self.session_id.clone(),
                    agent_id: self.identity.role.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    input: reported.clone(),
                });

                // Emit start event
                emit_tool_start(ToolStartPayload {
                    session_id: self.session_id.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    input: reported.clone(),
                });

                emit_agent_tool_call_start(AgentToolCallStartPayload {
                    agent_id: self.identity.role.clone(),
                    tool_call_id: call_id.clone(),
                    name: tool.name().to_string(),
                    args: reported.to_string(),
                });
                if let Some(stage) = stage_for_tool(tool.name()) {
                    emit_agent_stage_change(AgentStageChangePayload {
                        agent_id: self.identity.role.clone(),
                        stage: stage.to_string(),
                        status: "current".to_string(),
                        elapsed_ms: None,
                        reason: None,
                    });
                }
                let started = std::time::Instant::now();
                let mut telemetry =
                    crate::observability::timing::OperationTimer::start("agent_tool_call")
                        .with_session_id(&self.session_id)
                        .with_task_id(self.task_id)
                        .with_call_context(&call_id, &self.identity.role);

                // 🔴 G-1 (P3 post-phase gap hunt). A **second** permission
                // check stood here, and it directly contradicted this
                // function's own docstring — "every tool passes exactly one
                // gate". Worse than a duplicate prompt: the pre-partition gate
                // keys on `{session_id}-{tool_use_id}` while this one minted a
                // fresh `Uuid::new_v4()`, so the user saw an unrelated second
                // request for a call they had just approved. Only
                // `always_allow` hid it; "allow once" asked twice.
                //
                // Every tool in `write_tools` came from `approved_tools`, so it
                // has already passed the one gate. Nothing to re-decide.
                //
                // ⚠ `TG-P3-11` did not catch this because it asserted the
                // ABSENCE of `is_read_only` — the shape of the P3-64 fix —
                // rather than the invariant the fix existed to establish. That
                // is `§0.16 L-C6` again, in the gate I wrote an hour after
                // recording L-C6's seventh instance. The gate now counts gates.

                // Fire PreToolUse hook
                let pre_ctx = HookContext {
                    hook_type: HookType::PreToolUse.as_str().to_string(),
                    timestamp: chrono::Utc::now(),
                    session_id: Some(self.session_id.clone()),
                    agent_mode: None,
                    tool_call: Some(ToolCallContext {
                        id: call_id.clone(),
                        name: tool.name().to_string(),
                        input: reported.clone(), // A-08
                        result: None,
                        is_error: false,
                    }),
                    file_path: None,
                    task_id: None,
                    worker_id: None,
                    extra: serde_json::json!({
                        "working_dir": self.working_dir,
                        // The EFFECT, not the concurrency flag. A hook shown
                        // `is_read_only: true` for a tool being prompted as
                        // `Execute` is being told the opposite of what matters.
                        "effect": format!("{:?}", tool.effect_for_input(&input)),
                        "concurrency_safe": tool.effect_for_input(&input).is_concurrency_safe(),
                        "batch": true,
                    }),
                };
                let should_continue = HookRegistry::fire(&HookType::PreToolUse, &pre_ctx).await;

                let result = if !should_continue {
                    warn!("Tool '{}' blocked by PreToolUse hook", tool.name());
                    ToolResult::categorized_error(
                        "permission_denied",
                        "permission",
                        format!("Tool '{}' was blocked by a hook", tool.name()),
                        "Review the hook policy or choose a permitted operation.",
                    )
                } else {
                    match begin_operation_journal(
                        tool.name(),
                        &tool.effect_for_input(&input),
                        &input,
                        &ctx,
                    ) {
                        Err(crate::errors::ArcanaError::OperationReconciliationRequired {
                            operation,
                            intent_cursor,
                        }) => {
                            tracing::warn!(
                                target: "arcana::telemetry",
                                event_type = "operation_reconciliation_required",
                                operation = %operation,
                                intent_cursor,
                                error_class = "recovery_barrier",
                                "Refusing a side effect while a prior task operation is unresolved"
                            );
                            ToolResult::categorized_error(
                                "operation_reconciliation_required",
                                "reconciliation_required",
                                format!(
                                    "A previous `{operation}` operation has an unknown outcome (journal entry {intent_cursor}); this operation was not run."
                                ),
                                "Inspect the operation journal and current task/workspace/remote state, then record one explicit reconciliation decision before retrying.",
                            )
                        }
                        Err(_) => {
                            tracing::error!(
                                target: "arcana::telemetry",
                                event_type = "operation_journal_intent_failed",
                                operation = "agent_tool_call",
                                error_class = "runs_db",
                                "Refusing a run-scoped side effect because its intent could not be persisted"
                            );
                            ToolResult::categorized_error(
                                "operation_intent_unavailable",
                                "durability",
                                "Arcana could not persist this operation's recovery intent, so it was not run.",
                                "Check the local run database and retry only after it is writable.",
                            )
                        }
                        Ok(intent) => {
                            debug!("Executing write tool: {}", tool.name());
                            let state = self.state.clone();
                            let session_id = self.session_id.clone();
                            let mut interrupted = false;
                            let result = tokio::select! {
                                biased;
                                _ = wait_for_session_cancel(state, session_id) => {
                                    interrupted = true;
                                    ToolResult::categorized_error(
                                        "operation_cancelled_outcome_unknown",
                                        "reconciliation_required",
                                        "Arcana cancelled this operation while it was running; its external outcome may be unknown.",
                                        "Inspect the operation journal and current task/workspace/remote state before retrying.",
                                    )
                                }
                                result = Self::execute_handler(tool.as_ref(), input.clone(), &ctx) => result,
                            };
                            match intent {
                                Some(_intent) if interrupted => result,
                                Some(intent) => finish_operation_journal(intent, result),
                                None => result,
                            }
                        }
                    }
                }
                .with_execution_context_and_input(
                    tool.name(),
                    &self.session_id,
                    &self.identity.role,
                    self.task_id,
                    &call_id,
                    self.run_id.as_deref(),
                    Some(&input),
                );
                if !result.is_error {
                    telemetry.succeed();
                }
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "tool_call_result",
                    agent_role = %self.identity.role,
                    tool_name = tool.name(),
                    call_id = %call_id,
                    task_id = self.task_id,
                    is_error = result.is_error,
                    output_bytes = result.output.len(),
                    "Agent tool call returned"
                );

                // Fire PostToolUse or OnToolError hook
                let post_hook_type = if result.is_error {
                    HookType::OnToolError
                } else {
                    HookType::PostToolUse
                };
                let post_ctx = HookContext {
                    hook_type: post_hook_type.as_str().to_string(),
                    timestamp: chrono::Utc::now(),
                    session_id: Some(self.session_id.clone()),
                    agent_mode: None,
                    tool_call: Some(ToolCallContext {
                        id: call_id.clone(),
                        name: tool.name().to_string(),
                        input: reported.clone(), // A-08
                        result: Some(serde_json::json!({"output": &result.output})),
                        is_error: result.is_error,
                    }),
                    file_path: None,
                    task_id: None,
                    worker_id: None,
                    extra: serde_json::json!({
                        "output": &result.output,
                        "is_error": result.is_error,
                        "working_dir": self.working_dir,
                        "batch": true,
                    }),
                };
                HookRegistry::fire(&post_hook_type, &post_ctx).await;

                // Persist tool message
                self.save_tool_message(
                    &call_id,
                    tool.name(),
                    reported,
                    &result.output,
                    result.is_error,
                );

                // Emit end event
                emit_tool_end(ToolEndPayload {
                    session_id: self.session_id.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    output: result.output.clone(),
                    is_error: result.is_error,
                });
                execution.finish(result.is_error);

                let elapsed_ms = started.elapsed().as_millis() as u64;
                emit_agent_tool_call_result(AgentToolCallResultPayload {
                    agent_id: self.identity.role.clone(),
                    tool_call_id: call_id.clone(),
                    result: result.output.clone(),
                    is_error: result.is_error,
                    elapsed_ms,
                });
                if let Some(stage) = stage_for_tool(tool.name()) {
                    emit_agent_stage_change(AgentStageChangePayload {
                        agent_id: self.identity.role.clone(),
                        stage: stage.to_string(),
                        status: if result.is_error { "error" } else { "done" }.to_string(),
                        elapsed_ms: Some(elapsed_ms),
                        reason: None,
                    });
                }

                results[original_idx] = (tool_use_id, result);
            }
        }

        for (index, (_, result)) in results.iter_mut().enumerate() {
            if let Some(tool_name) = tool_names.get(index) {
                *result = std::mem::replace(result, ToolResult::success(""))
                    .with_execution_context(
                        tool_name,
                        &self.session_id,
                        &self.identity.role,
                        self.task_id,
                        &execution_call_ids[index],
                        self.run_id.as_deref(),
                    );
            }
        }
        Ok(results)
    }

    async fn request_permission(
        &mut self,
        call_id: &str,
        tool: &Arc<dyn Tool>,
        input: &serde_json::Value,
    ) -> Result<PermissionDecision> {
        let (tx, rx) = oneshot::channel::<PermissionResponse>();
        {
            let mut pending = self.pending.lock_tolerant();
            pending.insert(call_id.to_string(), tx);
        }

        // ⚠ Deliberately NOT redacted, and the only such site left.
        //
        // This payload is the permission prompt: the user is being asked to
        // approve *this exact call*. Redacting it would mean approving a
        // command they cannot see — `curl -H "Authorization: [REDACTED]"` tells
        // them nothing about where their credential is about to be sent. The
        // recipient is the user, on their own machine, looking at their own
        // secret; the prompt is a *control* over exfiltration, so blinding it
        // makes the product less safe, not more.
        //
        // It is also not durable: nothing persists this payload. Every path
        // that outlives the dialog — the model's context, the `messages` row,
        // the hook payload, the log — is redacted above.
        emit_permission_request(PermissionPayload {
            session_id: self.session_id.clone(),
            call_id: call_id.to_string(),
            tool_name: tool.name().to_string(),
            input: input.clone(),
            description: format!(
                "{} — {}",
                tool.effect_for_input(input).describe(),
                tool.description()
            ),
            allow_always: tool.effect_for_input(input).supports_standing_allow(),
        });

        // ── P3-08 · the wait is bounded, and cancellable ─────────────────────
        //
        // This was a bare `rx.await`. If the listener died — and **a tab switch
        // is enough** — the engine blocked here forever, holding whatever locks
        // the turn had taken, with no timeout and no way for the user to
        // recover short of restarting.
        //
        // Three outcomes are raced:
        //
        //   * the user answers;
        //   * the user cancels the session, polled through the existing
        //     `is_cancelled` flag the UI already sets — no new dependency, and
        //     it is the same signal every other cancellation path reads;
        //   * nothing happens for `PERMISSION_TIMEOUT`, and an unanswered
        //     prompt is treated as a refusal.
        //
        // ⚠ **Timeout denies rather than allows.** A prompt nobody answered is
        // not consent, and this is the branch reached when the listener is
        // gone — precisely when there is no one to have consented.
        let outcome = wait_for_permission_response(
            rx,
            self.state.clone(),
            self.session_id.clone(),
            PERMISSION_TIMEOUT,
        )
        .await;

        match outcome {
            PermissionDecision::Cancelled => {
                debug!(
                    "permission request for '{}' abandoned: the session was cancelled",
                    tool.name()
                );
            }
            PermissionDecision::TimedOut => {
                warn!(
                    "permission request for '{}' timed out after {}s with no answer. \
                     Treating it as a refusal — an unanswered prompt is not consent, and \
                     this is the branch a dead listener reaches.",
                    tool.name(),
                    PERMISSION_TIMEOUT.as_secs()
                );
            }
            _ => {}
        }

        // ⚠ The pending entry is removed on **every** outcome, including
        // timeout and cancellation. It was only ever removed by the responder,
        // so an abandoned prompt leaked its slot — `TG3-03` requires no
        // undrained `PendingPermissions` entry.
        {
            let mut pending = self.pending.lock_tolerant();
            pending.remove(call_id);
        }

        match outcome {
            PermissionDecision::AllowAlways => {
                // Free-form shell and host-controlled installs are excluded
                // from standing allowances above and here. Keep consent bound
                // to this exact call.
                if tool.effect_for_input(input).supports_standing_allow() {
                    self.always_allow.insert(tool.name().to_string());
                    debug!(
                        "Tool '{}' added to always-allow for session {}",
                        tool.name(),
                        self.session_id
                    );
                }
                Ok(PermissionDecision::Allow)
            }
            PermissionDecision::Allow => Ok(PermissionDecision::Allow),
            other => Ok(other),
        }
    }
}

#[cfg(test)]
mod p8_execution_registry_tests {
    use super::*;
    use crate::agent::execution_registry::active_by_session;
    use crate::agent::tool_effect::ToolEffect;
    use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
    use crate::state::app_state::ActiveAgent;
    use async_trait::async_trait;
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::sync::Notify;
    use uuid::Uuid;

    #[test]
    fn claim_operation_journal_binds_requested_task_before_runtime_context_does() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "claim-journal-session")
            .expect("begin worker run");
        let identity = AgentIdentity::worker("worker-backend");
        let context = ToolContext {
            session_id: "claim-journal-session".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "claim-journal-call".into(),
            run_id: Some(run_id.clone()),
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: identity.depth,
            agent_role: identity.role,

            run_token_budget: None,
        };
        let requested = serde_json::json!({"task_id": 42});
        assert_eq!(
            journal_task_id("claim_task", &requested, &context),
            Some(42)
        );
        assert_eq!(journal_task_id("write_file", &requested, &context), None);
        assert_eq!(
            journal_task_id("claim_task", &serde_json::json!({"task_id": -1}), &context),
            None
        );
        begin_operation_journal("claim_task", &ToolEffect::Board, &requested, &context)
            .expect("persist claim intent before handler");
        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read claim intent");
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(journal.entries[0].task_id, Some(42));
    }

    struct PanickingTool {
        effect: ToolEffect,
    }

    #[async_trait]
    impl Tool for PanickingTool {
        fn name(&self) -> &str {
            "panicking_test_tool"
        }
        fn description(&self) -> &str {
            "Panics to verify executor failure containment."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            self.effect.clone()
        }

        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            panic!("private panic payload must not reach the agent");
        }
    }

    struct ReturningTool {
        effect: ToolEffect,
    }

    #[async_trait]
    impl Tool for ReturningTool {
        fn name(&self) -> &str {
            "returning_test_tool"
        }
        fn description(&self) -> &str {
            "Returns a deterministic success result."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            self.effect.clone()
        }

        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            ToolResult::success("continued")
        }
    }

    struct RunIdCaptureTool {
        effect: ToolEffect,
        captured: Arc<std::sync::Mutex<Vec<Option<String>>>>,
    }

    struct PendingSideEffectTool {
        started: Arc<Notify>,
    }

    struct PendingReadTool {
        started: Arc<Notify>,
        dropped: Arc<AtomicUsize>,
    }

    struct CountDrop(Arc<AtomicUsize>);

    impl Drop for CountDrop {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    struct AmbiguousSideEffectTool {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for AmbiguousSideEffectTool {
        fn name(&self) -> &str {
            "ambiguous_side_effect_test_tool"
        }
        fn description(&self) -> &str {
            "Returns an uncertain side-effect result for journal verification."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            ToolEffect::Network
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            self.calls.fetch_add(1, Ordering::SeqCst);
            ToolResult::categorized_error(
                "operation_timed_out",
                "timeout",
                "A remote operation timed out after dispatch.",
                "Inspect remote state before retrying.",
            )
            .with_unknown_side_effect_outcome()
        }
    }

    #[async_trait]
    impl Tool for PendingSideEffectTool {
        fn name(&self) -> &str {
            "pending_side_effect_test_tool"
        }
        fn description(&self) -> &str {
            "Waits indefinitely so executor cancellation can be observed."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            ToolEffect::Network
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            self.started.notify_one();
            std::future::pending().await
        }
    }

    #[async_trait]
    impl Tool for PendingReadTool {
        fn name(&self) -> &str {
            "pending_read_test_tool"
        }
        fn description(&self) -> &str {
            "Waits indefinitely so executor cancellation can be observed."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            ToolEffect::Read
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            let _drop_guard = CountDrop(Arc::clone(&self.dropped));
            self.started.notify_one();
            std::future::pending().await
        }
    }

    #[async_trait]
    impl Tool for RunIdCaptureTool {
        fn name(&self) -> &str {
            "run_id_capture_test_tool"
        }
        fn description(&self) -> &str {
            "Captures the run context passed through the executor."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            self.effect.clone()
        }
        async fn execute(&self, _input: Value, ctx: &ToolContext) -> ToolResult {
            self.captured
                .lock()
                .expect("captured run IDs")
                .push(ctx.run_id.clone());
            ToolResult::success("captured")
        }
    }

    #[tokio::test]
    async fn worker_run_id_reaches_read_and_write_tool_contexts() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let run_id = runs
            .begin_worker_tick("supervisor", "journal-context-session")
            .expect("begin durable worker run");
        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let read_tool: Arc<dyn Tool> = Arc::new(RunIdCaptureTool {
            effect: ToolEffect::Read,
            captured: Arc::clone(&captured),
        });
        let write_tool: Arc<dyn Tool> = Arc::new(RunIdCaptureTool {
            effect: ToolEffect::Write,
            captured: Arc::clone(&captured),
        });
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            format!("p4-run-context-{}", Uuid::new_v4()),
            ".".into(),
            AgentIdentity::supervisor(),
        )
        .with_run_id(Some(run_id.clone()));

        let results = executor
            .execute_batch(vec![
                (read_tool, serde_json::json!({}), "read-call".into()),
                (write_tool, serde_json::json!({}), "write-call".into()),
            ])
            .await
            .expect("tools should execute");

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, result)| !result.is_error));
        let mut run_ids = captured.lock().expect("captured run IDs").clone();
        run_ids.sort();
        assert_eq!(run_ids, vec![Some(run_id.clone()); 2]);
        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read run-scoped operation journal");
        assert_eq!(
            journal.entries.len(),
            1,
            "reads are not side-effect journaled"
        );
        assert_eq!(journal.entries[0].operation, "external_tool");
        assert_eq!(
            journal.entries[0].outcome,
            crate::db::runs::OperationOutcome::Ok
        );
        assert_eq!(
            journal.entries[0].reconciliation,
            crate::db::runs::OperationReconciliation::Unresolved
        );
    }

    #[tokio::test]
    async fn operation_journal_outcome_failure_surfaces_unknown_without_retrying_the_effect() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "journal-unknown-session")
            .expect("begin durable run");
        runs.conn
            .execute_batch(
                "CREATE TRIGGER reject_operation_outcome
                 BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type = 'outcome'
                 BEGIN SELECT RAISE(ABORT, 'injected outcome-store failure'); END;",
            )
            .expect("inject an outcome acknowledgement failure");

        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let write_tool: Arc<dyn Tool> = Arc::new(RunIdCaptureTool {
            effect: ToolEffect::Write,
            captured: Arc::clone(&captured),
        });
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "journal-unknown-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run_id.clone()));
        let results = executor
            .execute_batch(vec![(
                write_tool,
                serde_json::json!({}),
                "write-call".into(),
            )])
            .await
            .expect("tool batch returns a structured unknown result");

        assert_eq!(captured.lock().unwrap().len(), 1, "effect ran once");
        assert!(results[0].1.is_error);
        let error: serde_json::Value =
            serde_json::from_str(&results[0].1.output).expect("structured unknown result");
        assert_eq!(error["error"]["code"], "side_effect_outcome_unknown");
        assert_eq!(error["error"]["retry_safe"], false);
        assert!(!results[0]
            .1
            .output
            .contains("injected outcome-store failure"));
        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read unresolved journal intent");
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(
            journal.entries[0].outcome,
            crate::db::runs::OperationOutcome::Unknown
        );
        assert_eq!(
            journal.entries[0].reconciliation,
            crate::db::runs::OperationReconciliation::Unresolved
        );
    }

    #[tokio::test]
    async fn ambiguous_side_effect_error_is_journaled_unknown_not_definite_error() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated runs DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "journal-ambiguous-session")
            .expect("begin durable run");
        let calls = Arc::new(AtomicUsize::new(0));
        let tool: Arc<dyn Tool> = Arc::new(AmbiguousSideEffectTool {
            calls: Arc::clone(&calls),
        });
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "journal-ambiguous-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run_id.clone()));
        let result = executor
            .execute_batch(vec![(tool, serde_json::json!({}), "ambiguous-call".into())])
            .await
            .expect("tool executor returns the ambiguous result");

        assert!(result[0].1.is_error);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no automatic replay");
        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read uncertain operation");
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(
            journal.entries[0].outcome,
            crate::db::runs::OperationOutcome::Unknown
        );
        assert_eq!(
            journal.entries[0].reconciliation,
            crate::db::runs::OperationReconciliation::Unresolved
        );
    }

    #[tokio::test]
    async fn operation_journal_intent_failure_blocks_the_mutating_handler() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "journal-intent-session")
            .expect("begin durable run");
        runs.conn
            .execute_batch(
                "CREATE TRIGGER reject_operation_intent
                 BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type = 'intent'
                 BEGIN SELECT RAISE(ABORT, 'injected intent-store failure'); END;",
            )
            .expect("inject an intent-store failure");

        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let write_tool: Arc<dyn Tool> = Arc::new(RunIdCaptureTool {
            effect: ToolEffect::Write,
            captured: Arc::clone(&captured),
        });
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "journal-intent-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run_id.clone()));
        let results = executor
            .execute_batch(vec![(
                write_tool,
                serde_json::json!({}),
                "write-call".into(),
            )])
            .await
            .expect("tool batch returns a structured refusal");

        assert!(captured.lock().unwrap().is_empty(), "effect must not run");
        assert!(results[0].1.is_error);
        let error: serde_json::Value =
            serde_json::from_str(&results[0].1.output).expect("structured intent failure");
        assert_eq!(error["error"]["code"], "operation_intent_unavailable");
        assert_eq!(
            runs.operation_journal_page(&run_id, 0, 10)
                .expect("empty failed intent reader")
                .entries
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn unresolved_prior_task_effect_blocks_executor_handler_until_reconciled() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let interrupted_run = runs
            .begin_worker_tick("worker-backend", "interrupted-effect-session")
            .expect("begin interrupted run");
        let uncertain = runs
            .record_operation_intent(
                &interrupted_run,
                Some(42),
                "interrupted-write",
                "write_file",
                "Write",
                &["src/state.rs".into()],
            )
            .expect("persist prior uncertain effect");
        runs.finish_worker_tick(&interrupted_run, "interrupted", "interrupted", None)
            .expect("close interrupted worker run");
        let replacement_run = runs
            .begin_worker_tick("worker-backend", "replacement-effect-session")
            .expect("begin replacement run");

        let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
        let write_tool: Arc<dyn Tool> = Arc::new(RunIdCaptureTool {
            effect: ToolEffect::Write,
            captured: Arc::clone(&captured),
        });
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "replacement-effect-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(replacement_run.clone()));
        executor.set_task_id(Some(42));

        let blocked = executor
            .execute_batch(vec![(
                Arc::clone(&write_tool),
                serde_json::json!({}),
                "blocked-write".into(),
            )])
            .await
            .expect("blocked operation is returned as a tool result");
        assert!(blocked[0].1.is_error);
        let error: Value = serde_json::from_str(&blocked[0].1.output).expect("error envelope");
        assert_eq!(error["error"]["code"], "operation_reconciliation_required");
        assert_eq!(captured.lock().expect("captured handler calls").len(), 0);
        assert!(!blocked[0].1.output.contains("state.rs"));
        let prior_page = runs
            .operation_journal_page(&interrupted_run, 0, 10)
            .expect("read unresolved prior effect");
        assert_eq!(
            prior_page.entries.len(),
            1,
            "blocked calls append no intent"
        );

        runs.record_operation_reconciliation(
            &interrupted_run,
            uncertain,
            crate::db::runs::OperationReconciliation::RetrySafe,
        )
        .expect("record explicit recovery decision");
        let resumed = executor
            .execute_batch(vec![(
                write_tool,
                serde_json::json!({}),
                "retry-write".into(),
            )])
            .await
            .expect("retry after reconciliation");
        assert!(!resumed[0].1.is_error, "reconciled effect can proceed");
        assert_eq!(captured.lock().expect("captured handler calls").len(), 1);
        let resumed_page = runs
            .operation_journal_page(&replacement_run, 0, 10)
            .expect("read resumed journal");
        assert_eq!(resumed_page.entries.len(), 1);
        assert_eq!(resumed_page.entries[0].task_id, Some(42));
    }

    #[tokio::test]
    async fn cancellation_drops_inflight_run_side_effect_and_leaves_unknown_journal_intent() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = crate::db::runs::RunsDb::open().expect("open isolated run DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "journal-cancel-session")
            .expect("begin durable run");
        let state = Arc::new(std::sync::Mutex::new(crate::state::AppState::new()));
        state.lock().unwrap().agents.insert(
            "journal-cancel-session".into(),
            ActiveAgent {
                session_id: "journal-cancel-session".into(),
                is_running: true,
                cancel_flag: false,
            },
        );
        let started = Arc::new(Notify::new());
        let tool: Arc<dyn Tool> = Arc::new(PendingSideEffectTool {
            started: Arc::clone(&started),
        });
        let executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "journal-cancel-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run_id.clone()))
        .with_state(Arc::clone(&state));
        let task = tokio::spawn(async move {
            let mut executor = executor;
            executor
                .execute_batch(vec![(tool, serde_json::json!({}), "remote-call".into())])
                .await
        });

        tokio::time::timeout(Duration::from_secs(2), started.notified())
            .await
            .expect("side-effect handler started");
        state.lock().unwrap().cancel_agent("journal-cancel-session");
        let results = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("executor observes cancellation")
            .expect("executor task joins")
            .expect("tool batch returns structured cancellation");
        assert!(results[0].1.is_error);
        let error: Value = serde_json::from_str(&results[0].1.output).unwrap();
        assert_eq!(
            error["error"]["code"],
            "operation_cancelled_outcome_unknown"
        );
        assert_eq!(error["error"]["retry_safe"], false);
        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read unresolved operation intent");
        assert_eq!(journal.entries.len(), 1);
        assert_eq!(
            journal.entries[0].outcome,
            crate::db::runs::OperationOutcome::Unknown
        );
        assert_eq!(
            journal.entries[0].reconciliation,
            crate::db::runs::OperationReconciliation::Unresolved
        );
    }

    #[tokio::test]
    async fn cancelling_session_drops_inflight_concurrent_read_and_returns_correlated_failure() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let session_id = format!("read-cancel-{}", Uuid::new_v4());
        let state = Arc::new(std::sync::Mutex::new(crate::state::AppState::default()));
        state.lock_tolerant().agents.insert(
            session_id.clone(),
            ActiveAgent {
                session_id: session_id.clone(),
                is_running: true,
                cancel_flag: false,
            },
        );
        let started = Arc::new(Notify::new());
        let dropped = Arc::new(AtomicUsize::new(0));
        let tool: Arc<dyn Tool> = Arc::new(PendingReadTool {
            started: Arc::clone(&started),
            dropped: Arc::clone(&dropped),
        });
        let executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        )
        .with_state(Arc::clone(&state));
        let task = tokio::spawn(async move {
            let mut executor = executor;
            executor
                .execute_batch(vec![(tool, serde_json::json!({}), "blocked-read".into())])
                .await
        });

        tokio::time::timeout(Duration::from_secs(2), started.notified())
            .await
            .expect("read handler started");
        state.lock_tolerant().cancel_agent(&session_id);
        let results = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("executor returns promptly after cancellation")
            .expect("executor task joins")
            .expect("read batch returns a structured cancellation");

        assert_eq!(results.len(), 1);
        let (tool_use_id, result) = &results[0];
        assert_eq!(tool_use_id, "blocked-read");
        assert!(result.is_error);
        let error: Value = serde_json::from_str(&result.output).expect("structured error");
        assert_eq!(error["error"]["code"], "operation_cancelled");
        assert_eq!(error["error"]["category"], "cancellation");
        assert_eq!(
            error["error"]["context"]["tool_call_id"],
            format!("{session_id}-blocked-read")
        );
        assert_eq!(dropped.load(Ordering::SeqCst), 1, "read future was dropped");
        assert!(
            crate::agent::execution_registry::active_by_session(&session_id).is_empty(),
            "cancelled read removes its active tool registration"
        );
    }

    #[tokio::test]
    async fn handler_panics_become_structured_results_for_read_and_write_partitions() {
        for effect in [ToolEffect::Read, ToolEffect::Write] {
            let panic_tool: Arc<dyn Tool> = Arc::new(PanickingTool {
                effect: effect.clone(),
            });
            let success_tool: Arc<dyn Tool> = Arc::new(ReturningTool { effect });
            let mut executor = ToolExecutor::new(
                PermissionMode::Autonomous,
                crate::agent::permission::new_pending_permissions(),
                format!("p4-tool-panic-{}", Uuid::new_v4()),
                ".".into(),
                AgentIdentity::supervisor(),
            );

            let results = executor
                .execute_batch(vec![
                    (panic_tool, serde_json::json!({}), "panic-call".into()),
                    (success_tool, serde_json::json!({}), "success-call".into()),
                ])
                .await
                .expect("a handler panic must not unwind the batch");

            assert_eq!(results.len(), 2);
            assert!(results[0].1.is_error);
            let output: serde_json::Value =
                serde_json::from_str(&results[0].1.output).expect("stable JSON error");
            assert_eq!(output["schema_version"], 1);
            assert_eq!(output["ok"], false);
            assert_eq!(output["error"]["retry_safe"], false);
            assert!(output["error"]["next_step"].as_str().is_some());
            assert!(!results[0]
                .1
                .output
                .contains("private panic payload must not reach the agent"));
            assert_eq!(results[1].1.output, "continued");
            assert!(!results[1].1.is_error);
        }
    }

    struct BlockedReadTool {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    #[async_trait]
    impl Tool for BlockedReadTool {
        fn name(&self) -> &str {
            "blocked_read"
        }
        fn description(&self) -> &str {
            "A deterministic test tool."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            ToolEffect::Read
        }

        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            self.started.notify_one();
            self.release.notified().await;
            ToolResult::success("released")
        }
    }

    #[tokio::test]
    async fn execute_batch_exposes_running_tool_to_registry_reader_then_cleans_it() {
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let tool: Arc<dyn Tool> = Arc::new(BlockedReadTool {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        });
        let session_id = format!("p8-execution-{}", Uuid::new_v4());
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            session_id.clone(),
            ".".into(),
            AgentIdentity::supervisor(),
        );

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(
                    tool,
                    serde_json::json!({"path":"x"}),
                    "tool-use".into(),
                )])
                .await
        });
        started.notified().await;
        let active = active_by_session(&session_id);
        assert_eq!(
            active.len(),
            1,
            "the production executor must register before dispatch"
        );
        assert_eq!(active[0].tool_name, "blocked_read");
        assert_eq!(active[0].input, serde_json::json!({"path":"x"}));

        release.notify_one();
        let results = execution
            .await
            .expect("executor task")
            .expect("batch result");
        assert_eq!(results[0].1.output, "released");
        assert!(
            active_by_session(&session_id).is_empty(),
            "completed execution must leave the query surface"
        );
    }

    #[tokio::test]
    async fn autonomous_bash_requires_permission_before_shell_execution() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let marker = temp.path().join("approval-marker.txt");
        let session_id = format!("p4-bash-approval-{}", Uuid::new_v4());
        let pending = crate::agent::permission::new_pending_permissions();
        let tool: Arc<dyn Tool> = Arc::new(crate::agent::builtin_tools::bash::BashTool);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(
                    tool,
                    serde_json::json!({"command":"echo p4 > approval-marker.txt"}),
                    "bash-write".into(),
                )])
                .await
        });

        let permission_id = format!("{session_id}-bash-write");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if pending.lock_tolerant().contains_key(&permission_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("autonomous Bash must open a permission request");

        let responder = pending
            .lock_tolerant()
            .remove(&permission_id)
            .expect("pending permission entry");
        responder
            .send(PermissionResponse::Deny)
            .expect("permission waiter is still active");

        let results = execution
            .await
            .expect("executor task")
            .expect("batch result");
        assert!(
            results[0].1.is_error,
            "a denied call must return a tool error"
        );
        assert!(
            results[0].1.output.contains("declined permission"),
            "the model must receive an actionable denial result"
        );
        assert!(
            !marker.exists(),
            "Bash must not spawn the command before approval"
        );
        assert!(
            pending.lock_tolerant().is_empty(),
            "answered permission requests must drain their pending entry"
        );
    }

    #[tokio::test]
    async fn autonomous_project_install_requires_a_per_call_host_approval() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let session_id = format!("p4-project-install-approval-{}", Uuid::new_v4());
        let pending = crate::agent::permission::new_pending_permissions();
        let tool: Arc<dyn Tool> =
            Arc::new(crate::agent::builtin_tools::project_command::ProjectCommandTool);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        // A stale/session-wide allowance for the general project command
        // cannot authorize a network-capable install intent.
        executor.always_allow.insert("project_command".to_string());

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(
                    tool,
                    serde_json::json!({"intent":"install"}),
                    "install-once".into(),
                )])
                .await
        });
        let permission_id = format!("{session_id}-install-once");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if pending.lock_tolerant().contains_key(&permission_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("autonomous install must request host approval");

        pending
            .lock_tolerant()
            .remove(&permission_id)
            .expect("pending install approval")
            .send(PermissionResponse::Deny)
            .expect("permission waiter is still active");
        let results = execution
            .await
            .expect("executor task")
            .expect("batch result");
        assert!(results[0].1.is_error);
        assert!(results[0].1.output.contains("declined permission"));
        assert!(pending.lock_tolerant().is_empty());
    }

    #[tokio::test]
    async fn bash_standing_allowance_does_not_skip_per_call_approval() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let marker = temp.path().join("standing-allow-marker.txt");
        let session_id = format!("p4-bash-standing-allow-{}", Uuid::new_v4());
        let pending = crate::agent::permission::new_pending_permissions();
        let tool: Arc<dyn Tool> = Arc::new(crate::agent::builtin_tools::bash::BashTool);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        // Simulate a stale or previously saved allowance. FreeformShell must
        // still request approval for this specific invocation.
        executor.always_allow.insert("bash".to_string());

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(
                    tool,
                    serde_json::json!({"command":"echo p4 > standing-allow-marker.txt"}),
                    "bash-standing-allow".into(),
                )])
                .await
        });

        let permission_id = format!("{session_id}-bash-standing-allow");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if pending.lock_tolerant().contains_key(&permission_id) {
                    break;
                }
                assert!(
                    !execution.is_finished(),
                    "a stored Bash allowance must not execute the command"
                );
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("standing Bash allowance must still prompt");

        let responder = pending
            .lock_tolerant()
            .remove(&permission_id)
            .expect("pending permission entry");
        responder
            .send(PermissionResponse::Deny)
            .expect("permission waiter is still active");

        let results = execution
            .await
            .expect("executor task")
            .expect("batch result");
        assert!(results[0].1.is_error);
        assert!(!marker.exists(), "denied shell command must not run");
        assert!(pending.lock_tolerant().is_empty());
    }

    #[tokio::test]
    async fn approved_project_install_still_fails_closed_before_child_spawn() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let session_id = format!("p4-project-install-staging-{}", Uuid::new_v4());
        let pending = crate::agent::permission::new_pending_permissions();
        let tool: Arc<dyn Tool> =
            Arc::new(crate::agent::builtin_tools::project_command::ProjectCommandTool);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(
                    tool,
                    serde_json::json!({"intent":"install"}),
                    "install-approved-once".into(),
                )])
                .await
        });
        let permission_id = format!("{session_id}-install-approved-once");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if pending.lock_tolerant().contains_key(&permission_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("approved install must first request one-call host consent");

        pending
            .lock_tolerant()
            .remove(&permission_id)
            .expect("pending install approval")
            .send(PermissionResponse::Allow)
            .expect("permission waiter is still active");
        let results = execution
            .await
            .expect("executor task")
            .expect("batch result");
        let error: Value = serde_json::from_str(&results[0].1.output)
            .expect("staging refusal is a structured error");

        assert!(results[0].1.is_error);
        assert_eq!(error["error"]["kind"], "sandbox_violation");
        assert_eq!(error["error"]["capability"], "network");
        assert_eq!(error["error"]["policy"], "deny");
        assert_eq!(error["error"]["command"], "project_command install");
        assert_eq!(error["error"]["category"], "sandbox");
        assert_eq!(error["error"]["tool"], "project_command");
        assert_eq!(error["error"]["operation"], "tool.project_command");
        assert_eq!(error["error"]["context"]["session_id"], session_id);
        assert_eq!(
            error["error"]["context"]["tool_call_id"],
            format!("{session_id}-install-approved-once")
        );
        assert!(error["error"]["timestamp"].as_str().is_some());
        assert!(pending.lock_tolerant().is_empty());
        assert_eq!(
            std::fs::read_dir(temp.path())
                .expect("workspace remains readable")
                .count(),
            0,
            "host consent alone must not spawn an install or create workspace output"
        );
    }

    struct HostApprovalCounter(std::sync::atomic::AtomicUsize);

    #[async_trait]
    impl Tool for HostApprovalCounter {
        fn name(&self) -> &str {
            "host_approval_counter"
        }
        fn description(&self) -> &str {
            "Counts whether a protected handler was reached."
        }
        fn input_schema(&self) -> Value {
            serde_json::json!({"type":"object"})
        }
        fn effect(&self) -> ToolEffect {
            ToolEffect::HostApproval
        }
        async fn execute(&self, _input: Value, _ctx: &ToolContext) -> ToolResult {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            ToolResult::success("executed")
        }
    }

    #[tokio::test]
    async fn headless_host_approval_fails_immediately_without_running_handler() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let pending = crate::agent::permission::new_pending_permissions();
        let tool = Arc::new(HostApprovalCounter(std::sync::atomic::AtomicUsize::new(0)));
        let tool_dyn: Arc<dyn Tool> = tool.clone();
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            format!("headless-approval-{}", Uuid::new_v4()),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        )
        .with_host_approval_available(false);

        let session_id = executor.session_id.clone();
        let results = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            executor.execute_batch(vec![(tool_dyn, serde_json::json!({}), "install".into())]),
        )
        .await
        .expect("headless denial must not wait for the interactive approval timeout")
        .expect("tool batch should return a denial result");

        assert!(results[0].1.is_error);
        assert!(results[0].1.output.contains("headless process"));
        let error: Value = serde_json::from_str(&results[0].1.output)
            .expect("headless approval refusal is structured");
        assert_eq!(error["error"]["code"], "permission_unavailable");
        assert_eq!(error["error"]["category"], "permission");
        assert_eq!(error["error"]["tool"], "host_approval_counter");
        assert_eq!(error["error"]["context"]["session_id"], session_id);
        assert_eq!(
            error["error"]["context"]["tool_call_id"],
            format!("{session_id}-install")
        );
        assert_eq!(
            tool.0.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "the protected tool handler must not run without a host responder"
        );
        assert!(pending.lock_tolerant().is_empty());
    }

    #[tokio::test]
    async fn approval_cancellation_is_not_reported_as_user_denial() {
        let temp = tempfile::tempdir().expect("temporary workspace");
        let session_id = format!("approval-cancel-{}", Uuid::new_v4());
        let pending = crate::agent::permission::new_pending_permissions();
        let state = Arc::new(std::sync::Mutex::new(crate::state::AppState::default()));
        state.lock_tolerant().agents.insert(
            session_id.clone(),
            ActiveAgent {
                session_id: session_id.clone(),
                is_running: true,
                cancel_flag: false,
            },
        );
        let tool = Arc::new(HostApprovalCounter(std::sync::atomic::AtomicUsize::new(0)));
        let tool_dyn: Arc<dyn Tool> = tool.clone();
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::clone(&pending),
            session_id.clone(),
            temp.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        executor.state = Some(Arc::clone(&state));

        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(tool_dyn, serde_json::json!({}), "install".into())])
                .await
        });
        let permission_id = format!("{session_id}-install");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                if pending.lock_tolerant().contains_key(&permission_id) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("approval request becomes pending");
        state.lock_tolerant().cancel_agent(&session_id);

        let results = execution
            .await
            .expect("executor task")
            .expect("batch returns the cancelled tool result");
        let result = &results[0].1;
        let error: Value = serde_json::from_str(&result.output).expect("structured error");
        assert!(result.is_error);
        assert_eq!(error["error"]["code"], "operation_cancelled");
        assert_eq!(error["error"]["category"], "cancellation");
        assert_eq!(error["error"]["context"]["tool_call_id"], permission_id);
        assert_eq!(tool.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(pending.lock_tolerant().is_empty());
    }

    #[tokio::test]
    async fn approval_timeout_and_closed_listener_have_distinct_decisions() {
        let (_sender, receiver) = oneshot::channel::<PermissionResponse>();
        assert_eq!(
            wait_for_permission_response(
                receiver,
                None,
                "timeout-session".into(),
                std::time::Duration::from_millis(1),
            )
            .await,
            PermissionDecision::TimedOut
        );

        let (sender, receiver) = oneshot::channel::<PermissionResponse>();
        drop(sender);
        assert_eq!(
            wait_for_permission_response(
                receiver,
                None,
                "closed-session".into(),
                std::time::Duration::from_secs(1),
            )
            .await,
            PermissionDecision::ChannelClosed
        );
    }

    async fn run_builtin_matrix_call(
        executor: &mut ToolExecutor,
        registry: &crate::agent::tool_registry::ToolRegistry,
        name: &str,
        input: Value,
        use_id: &str,
    ) -> ToolResult {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("FastDevelopment registry is missing {name}"));
        let results = executor
            .execute_batch(vec![(tool, input, use_id.to_string())])
            .await
            .expect("registered builtin call returns a result");
        assert_eq!(results.len(), 1, "one result for {name}");
        let (returned_use_id, result) = results.into_iter().next().expect("one result");
        assert_eq!(returned_use_id, use_id);
        result
    }

    fn assert_builtin_matrix_failure(
        tool_name: &str,
        use_id: &str,
        result: &ToolResult,
        private_arguments: &[&str],
    ) {
        assert!(result.is_error, "{tool_name} malformed input must fail");
        assert!(result.output.len() <= 8 * 1024, "bounded: {tool_name}");
        let envelope: Value =
            serde_json::from_str(&result.output).expect("normalized JSON failure envelope");
        assert_eq!(envelope["schema_version"], 1, "{tool_name}");
        assert_eq!(envelope["ok"], false, "{tool_name} cannot look successful");
        assert_eq!(envelope["error"]["tool"], tool_name, "{tool_name}");
        assert_eq!(
            envelope["error"]["operation"],
            format!("tool.{tool_name}"),
            "{tool_name}"
        );
        assert_eq!(
            envelope["error"]["context"]["tool_call_id"],
            format!("builtin-matrix-session-{use_id}"),
            "{tool_name} call correlation survives"
        );
        assert_eq!(
            envelope["error"]["context"]["session_id"], "builtin-matrix-session",
            "{tool_name} session correlation survives"
        );
        assert_eq!(
            envelope["error"]["context"]["agent_id"], "supervisor",
            "{tool_name} agent correlation survives"
        );
        assert!(envelope["error"]["retryable"].is_boolean(), "{tool_name}");
        assert!(
            envelope["error"]["retry_after_ms"].is_null()
                || envelope["error"]["retry_after_ms"].is_u64(),
            "{tool_name} retry delay is optional and numeric"
        );
        for field in [
            "error_code",
            "category",
            "message",
            "cause",
            "suggested_action",
        ] {
            assert!(
                envelope["error"][field].as_str().is_some(),
                "{tool_name} has string {field}: {envelope}"
            );
        }
        for field in ["message", "cause", "suggested_action"] {
            assert!(
                envelope["error"][field].as_str().unwrap_or_default().len() <= 512,
                "{tool_name} {field} is bounded"
            );
        }
        assert!(
            envelope["error"]["context"].is_object(),
            "{tool_name} carries structured context"
        );
        for private in private_arguments {
            assert!(
                !result.output.contains(private),
                "{tool_name} leaked private argument {private}"
            );
        }
    }

    #[tokio::test]
    async fn builtin_file_task_epic_budget_matrix_uses_live_fastdevelopment_registry() {
        use crate::agent::{builtin_tools, tool_registry::ToolRegistry, PAMode};
        use crate::db::tasks::{CreateTaskInput, TasksDb};

        let home = tempfile::tempdir().expect("isolated Arcana home and task database");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated builtin workspace");
        std::fs::write(
            workspace.path().join("matrix-proof.txt"),
            "line one\nneedle is present\nline three\n",
        )
        .expect("seed isolated workspace file");

        let tasks = TasksDb::open().expect("open isolated task database");
        let seeded_task = tasks
            .create(CreateTaskInput {
                title: "executor matrix seed task".into(),
                description: Some("private seed description".into()),
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "seed remains isolated".into(),
                created_by: Some("test".into()),
            })
            .expect("seed task in temporary database");

        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
        let live_names = registry
            .all()
            .iter()
            .map(|tool| tool.name().to_string())
            .collect::<std::collections::BTreeSet<_>>();
        let expected_live_names = [
            "read_file",
            "write_file",
            "edit_file",
            "bash",
            "git_commit",
            "project_command",
            "mcp_list_tools",
            "mcp_call_tool",
            "list_directory",
            "grep",
            "glob",
            "check_file",
            "append_file",
            "list_tasks",
            "get_task",
            "update_task_status",
            "assign_task",
            "assign_tasks",
            "create_task",
            "request_supervisor_action",
            "planner",
            "spawn_subagent",
            "spawn_parallel_subagents",
            "spawn_qc_review",
            "write_interface_spec",
            "e2e_probe",
            "search_memory",
            "get_memory",
            "list_memories",
            "propose_memory",
            "suggest_worker",
            "claim_task",
            "analyze_diff",
            "annotate_file",
            "set_task_budget",
            "create_epic",
            "create_phased_task",
            "get_epic_progress",
            "get_ready_tasks_in_phase",
            "list_worker_requests",
            "resolve_worker_request",
            "send_worker_message",
            "list_worker_messages",
            "list_stale_worker_messages",
            "list_worker_health",
            "reconcile_worker_operation",
            "acknowledge_worker_message",
        ]
        .into_iter()
        .map(|name| name.to_string())
        .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            live_names, expected_live_names,
            "review the matrix inventory whenever FastDevelopment registration changes"
        );
        let covered = [
            "read_file",
            "write_file",
            "edit_file",
            "append_file",
            "list_directory",
            "grep",
            "glob",
            "get_task",
            "create_epic",
            "create_phased_task",
            "get_epic_progress",
            "get_ready_tasks_in_phase",
            "set_task_budget",
        ]
        .into_iter()
        .map(|name| name.to_string())
        .collect::<std::collections::BTreeSet<_>>();
        assert!(
            covered.is_subset(&live_names),
            "covered names must be concrete FastDevelopment registrations; missing: {:?}",
            covered.difference(&live_names).collect::<Vec<_>>()
        );
        let uncovered = live_names
            .difference(&covered)
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        assert!(covered.is_disjoint(&uncovered));
        assert_eq!(
            covered
                .union(&uncovered)
                .cloned()
                .collect::<std::collections::BTreeSet<_>>(),
            live_names,
            "live registry coverage partitions every registered builtin"
        );

        let pending = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            pending,
            "builtin-matrix-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );

        let read = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "read_file",
            serde_json::json!({"path":"matrix-proof.txt"}),
            "read-success",
        )
        .await;
        assert!(!read.is_error, "read_file output: {}", read.output);
        assert!(read.output.contains("needle is present"));
        let read_failure_input = serde_json::json!({
            "path": 7,
            "private": "PRIVATE_READ_FILE_ARGUMENT"
        });
        let read_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "read_file",
            read_failure_input,
            "read-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "read_file",
            "read-malformed",
            &read_failure,
            &["PRIVATE_READ_FILE_ARGUMENT"],
        );

        let listed = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "list_directory",
            serde_json::json!({}),
            "directory-success",
        )
        .await;
        assert!(!listed.is_error, "list_directory output: {}", listed.output);
        assert!(listed.output.contains("matrix-proof.txt"));
        let outside_path = workspace
            .path()
            .parent()
            .expect("temporary workspace parent")
            .join("PRIVATE_DIRECTORY_ARGUMENT")
            .to_string_lossy()
            .into_owned();
        let directory_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "list_directory",
            serde_json::json!({"path":outside_path.clone()}),
            "directory-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "list_directory",
            "directory-malformed",
            &directory_failure,
            &[&outside_path],
        );

        let grep = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "grep",
            serde_json::json!({"pattern":"needle", "path":"."}),
            "grep-success",
        )
        .await;
        assert!(!grep.is_error, "grep output: {}", grep.output);
        assert!(grep.output.contains("needle is present"));
        let grep_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "grep",
            serde_json::json!({
                "pattern": 5,
                "path": "PRIVATE_GREP_ARGUMENT",
                "context_lines": "PRIVATE_GREP_CONTEXT_ARGUMENT"
            }),
            "grep-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "grep",
            "grep-malformed",
            &grep_failure,
            &["PRIVATE_GREP_ARGUMENT", "PRIVATE_GREP_CONTEXT_ARGUMENT"],
        );

        let glob = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "glob",
            serde_json::json!({"pattern":"*.txt", "path":"."}),
            "glob-success",
        )
        .await;
        assert!(!glob.is_error, "glob output: {}", glob.output);
        assert!(glob.output.contains("matrix-proof.txt"));
        let glob_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "glob",
            serde_json::json!({
                "pattern": 5,
                "path": "PRIVATE_GLOB_ARGUMENT",
                "max_results": "PRIVATE_GLOB_LIMIT_ARGUMENT"
            }),
            "glob-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "glob",
            "glob-malformed",
            &glob_failure,
            &["PRIVATE_GLOB_ARGUMENT", "PRIVATE_GLOB_LIMIT_ARGUMENT"],
        );

        let mutation_path = workspace.path().join("mutation-proof.txt");
        let write = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "write_file",
            serde_json::json!({"path":"mutation-proof.txt", "content":"before"}),
            "write-success",
        )
        .await;
        assert!(!write.is_error, "write_file output: {}", write.output);
        assert_eq!(
            std::fs::read_to_string(&mutation_path).expect("read back written file"),
            "before"
        );
        let write_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "write_file",
            serde_json::json!({
                "path":"mutation-proof.txt",
                "content": 7,
                "private": "PRIVATE_WRITE_CONTENT_ARGUMENT"
            }),
            "write-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "write_file",
            "write-malformed",
            &write_failure,
            &["PRIVATE_WRITE_CONTENT_ARGUMENT"],
        );
        assert_eq!(
            std::fs::read_to_string(&mutation_path).expect("malformed write leaves file intact"),
            "before"
        );

        let edit = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "edit_file",
            serde_json::json!({
                "path":"mutation-proof.txt",
                "old_string":"before",
                "new_string":"after"
            }),
            "edit-success",
        )
        .await;
        assert!(!edit.is_error, "edit_file output: {}", edit.output);
        assert_eq!(
            std::fs::read_to_string(&mutation_path).expect("read back edited file"),
            "after"
        );
        let edit_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "edit_file",
            serde_json::json!({
                "path":"mutation-proof.txt",
                "old_string":"PRIVATE_EDIT_OLD_STRING_ARGUMENT",
                "new_string":"must-not-be-written"
            }),
            "edit-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "edit_file",
            "edit-malformed",
            &edit_failure,
            &["PRIVATE_EDIT_OLD_STRING_ARGUMENT"],
        );
        assert_eq!(
            std::fs::read_to_string(&mutation_path).expect("failed edit leaves file intact"),
            "after"
        );

        let append = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "append_file",
            serde_json::json!({"path":"mutation-proof.txt", "content":"tail"}),
            "append-success",
        )
        .await;
        assert!(!append.is_error, "append_file output: {}", append.output);
        assert_eq!(
            std::fs::read_to_string(&mutation_path).expect("read back appended file"),
            "after\ntail"
        );
        let append_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "append_file",
            serde_json::json!({
                "path":"PRIVATE_APPEND_PATH_ARGUMENT",
                "content": 9
            }),
            "append-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "append_file",
            "append-malformed",
            &append_failure,
            &["PRIVATE_APPEND_PATH_ARGUMENT"],
        );
        assert_eq!(
            std::fs::read_to_string(&mutation_path)
                .expect("malformed append leaves the target file intact"),
            "after\ntail"
        );

        let task = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_task",
            serde_json::json!({"id": seeded_task.id}),
            "task-success",
        )
        .await;
        assert!(!task.is_error, "get_task output: {}", task.output);
        assert!(task.output.contains("executor matrix seed task"));
        let task_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_task",
            serde_json::json!({"id":"PRIVATE_GET_TASK_ARGUMENT"}),
            "task-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "get_task",
            "task-malformed",
            &task_failure,
            &["PRIVATE_GET_TASK_ARGUMENT"],
        );

        let epic = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "create_epic",
            serde_json::json!({"title":"executor matrix epic"}),
            "epic-create-success",
        )
        .await;
        assert!(!epic.is_error, "create_epic output: {}", epic.output);
        let epic_output: Value = serde_json::from_str(&epic.output).expect("epic success JSON");
        let epic_id = epic_output["epic_id"]
            .as_i64()
            .expect("create_epic returns a database epic id");
        let epic_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "create_epic",
            serde_json::json!({
                "title": "",
                "description": "PRIVATE_CREATE_EPIC_ARGUMENT"
            }),
            "epic-create-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "create_epic",
            "epic-create-malformed",
            &epic_failure,
            &["PRIVATE_CREATE_EPIC_ARGUMENT"],
        );

        let phase = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "create_phased_task",
            serde_json::json!({
                "epic_id": epic_id.to_string(),
                "phase": "verification",
                "phase_order": 1,
                "title": "executor matrix phase task",
                "priority": "p2",
                "task_type": "feature",
                "acceptance_criteria": "phase task is returned by the real phase reader"
            }),
            "phase-create-success",
        )
        .await;
        assert!(
            !phase.is_error,
            "create_phased_task output: {}",
            phase.output
        );
        assert!(phase.output.contains("executor matrix phase task"));
        let phase_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "create_phased_task",
            serde_json::json!({
                "epic_id": epic_id.to_string(),
                "phase": "PRIVATE_PHASE_CREATE_ARGUMENT"
            }),
            "phase-create-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "create_phased_task",
            "phase-create-malformed",
            &phase_failure,
            &["PRIVATE_PHASE_CREATE_ARGUMENT"],
        );

        let progress = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_epic_progress",
            serde_json::json!({"epic_id": epic_id.to_string()}),
            "epic-progress-success",
        )
        .await;
        assert!(
            !progress.is_error,
            "get_epic_progress output: {}",
            progress.output
        );
        assert!(progress.output.contains("verification"));
        let progress_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_epic_progress",
            serde_json::json!({"description":"PRIVATE_EPIC_PROGRESS_ARGUMENT"}),
            "epic-progress-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "get_epic_progress",
            "epic-progress-malformed",
            &progress_failure,
            &["PRIVATE_EPIC_PROGRESS_ARGUMENT"],
        );

        let ready = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_ready_tasks_in_phase",
            serde_json::json!({"epic_id": epic_id.to_string(), "phase":"verification"}),
            "phase-ready-success",
        )
        .await;
        assert!(
            !ready.is_error,
            "get_ready_tasks_in_phase output: {}",
            ready.output
        );
        assert!(ready.output.contains("executor matrix phase task"));
        let ready_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "get_ready_tasks_in_phase",
            serde_json::json!({"phase":"PRIVATE_READY_PHASE_ARGUMENT"}),
            "phase-ready-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "get_ready_tasks_in_phase",
            "phase-ready-malformed",
            &ready_failure,
            &["PRIVATE_READY_PHASE_ARGUMENT"],
        );

        let budget = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "set_task_budget",
            serde_json::json!({
                "task_id": seeded_task.id,
                "max_tokens": 2_000,
                "max_usd": 0.25,
                "max_turns": 3
            }),
            "budget-success",
        )
        .await;
        assert!(
            !budget.is_error,
            "set_task_budget output: {}",
            budget.output
        );
        let saved_budget = tasks
            .get_by_id(seeded_task.id)
            .expect("read actual saved budget")
            .expect("budget task exists");
        assert_eq!(saved_budget.budget_max_tokens, Some(2_000));
        assert_eq!(saved_budget.budget_max_usd, Some(0.25));
        assert_eq!(saved_budget.budget_max_turns, Some(3));
        let budget_failure = run_builtin_matrix_call(
            &mut executor,
            &registry,
            "set_task_budget",
            serde_json::json!({"task_id":"PRIVATE_BUDGET_ARGUMENT"}),
            "budget-malformed",
        )
        .await;
        assert_builtin_matrix_failure(
            "set_task_budget",
            "budget-malformed",
            &budget_failure,
            &["PRIVATE_BUDGET_ARGUMENT"],
        );

        assert!(
            uncovered.contains(&"bash".to_string()),
            "the live registry inventory should retain uncovered entries: {uncovered:?}"
        );
    }
}
