// Tool executor — handles permission checks + concurrent/serial execution.
// Read-only tools run concurrently (tokio tasks, max 10).
// Write tools run serially with user confirmation.
// Pattern from Claude Code: read-only concurrent, writes serial.

use std::sync::Arc;
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};
use uuid::Uuid;

use super::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use super::tool_trait::{AgentIdentity, Tool, ToolContext, ToolResult};
use crate::errors::Result;
use crate::events::{
    emit_permission_request, emit_tool_end, emit_tool_start, PermissionPayload, ToolEndPayload,
    ToolStartPayload,
};
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};
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

pub struct ToolExecutor {
    mode: PermissionMode,
    pending: PendingPermissions,
    session_id: String,
    working_dir: String,
    /// Who this executor runs as, and how deeply nested. Threaded into every
    /// ToolContext it builds so identity is never absent and depth is never
    /// self-reported by the model.
    identity: AgentIdentity,
    // Tools auto-approved for this session (AllowAlways responses)
    always_allow: std::collections::HashSet<String>,
    // Session manager for persisting tool messages
    session_mgr: Option<Arc<SessionManager>>,
    /// Shared application state, used **only** to observe cancellation
    /// (`P3-08`). The permission wait must notice the user cancelling while a
    /// prompt is open, and this is the same signal every other cancellation
    /// path reads — a second mechanism here would be a second thing to forget.
    ///
    /// `Option` because `airsctl` and the tests build an executor with no
    /// application state; there the timeout is the only bound.
    state: Option<Arc<std::sync::Mutex<crate::state::AppState>>>,
}

impl ToolExecutor {
    pub fn new(
        mode: PermissionMode,
        pending: PendingPermissions,
        session_id: String,
        working_dir: String,
        identity: AgentIdentity,
    ) -> Self {
        Self {
            mode,
            pending,
            session_id,
            working_dir,
            identity,
            always_allow: std::collections::HashSet::new(),
            session_mgr: None,
            state: None,
        }
    }

    /// Set the session manager for persisting tool messages
    /// Observe cancellation through the shared application state (`P3-08`).
    pub fn with_state(mut self, state: Arc<std::sync::Mutex<crate::state::AppState>>) -> Self {
        self.state = Some(state);
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
    /// loop calls `execute_batch` (`agent/engine.rs:314`), so redaction was
    /// live in dead code while every real tool call carried raw output to the
    /// model, the database and the renderer. The phase's own smoke test drove
    /// the unused method and passed, and the gate was a substring check over
    /// the file, so both proofs were green. Found by three independent
    /// post-phase auditors.
    fn redact_result(mut result: ToolResult) -> ToolResult {
        result.output = crate::security::redaction::redact_text(&result.output);
        result
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

    // ⚠ `ToolExecutor::execute()` was DELETED in `P3-10`.
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
    ) -> Result<bool> {
        // A standing per-tool allowance the user granted earlier in the session.
        if self.always_allow.contains(tool.name()) {
            return Ok(true);
        }

        if !tool.effect().requires_prompt(&self.mode) {
            return Ok(true);
        }

        self.request_permission(call_id, tool, input).await
    }

    /// Read-only tools run concurrently (max 10), write tools run serially.
    /// Returns results in the same order as input.
    /// Pattern from Claude Code's partitionToolCalls() + runTools().
    pub async fn execute_batch(
        &mut self,
        tools: Vec<(Arc<dyn Tool>, serde_json::Value, String)>, // (tool, input, tool_use_id)
    ) -> Result<Vec<(String, ToolResult)>> {
        // (tool_use_id, result)
        if tools.is_empty() {
            return Ok(Vec::new());
        }

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
            let call_id = format!("{}-{}", self.session_id, tool_use_id);
            if self.check_permission(&call_id, &tool, &input).await? {
                approved_tools.push((tool, input, tool_use_id, idx));
            } else {
                // A refusal is a RESULT, not a dropped call. Dropping it would
                // leave the assistant message carrying a `tool_use` with no
                // matching `tool_result` — a hard 400 on the next request, the
                // same defect as `P3-57`.
                refused.push((
                    idx,
                    tool_use_id,
                    ToolResult::error(format!(
                        "The user declined permission for `{}` to {}.",
                        tool.name(),
                        tool.effect().describe()
                    )),
                ));
            }
        }

        // Partition the approved calls: concurrency-safe ones run together.
        let mut read_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, usize)> = Vec::new();
        let mut write_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, usize)> = Vec::new();

        for (tool, input, tool_use_id, idx) in approved_tools {
            if tool.effect().is_concurrency_safe() {
                read_tools.push((tool, input, tool_use_id, idx));
            } else {
                write_tools.push((tool, input, tool_use_id, idx));
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
        for (tool, _input, tool_use_id, idx) in read_tools.iter().chain(write_tools.iter()) {
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

            for (tool, input, tool_use_id, original_idx) in read_tools {
                let sem_clone = Arc::clone(&semaphore);
                let session_id = self.session_id.clone();
                let working_dir = self.working_dir.clone();
                // (`_mode` / `_always_allow` were cloned here and never used —
                // underscore-prefixed to silence the warning. The permission
                // decision now happens before the partition, so there is
                // nothing for this task to consult. P3-64.)
                let _pending = self.pending.clone();
                let identity = self.identity.clone();
                // G-5. The permission DECISION happens before this partition
                // (P3-64), but the child's ToolContext still has to carry the
                // mode: `spawn_subagent` reads it to stop a subagent
                // outranking its parent.
                let mode = self.mode.clone();

                let handle = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();

                    let call_id = Uuid::new_v4().to_string();
                    let ctx = ToolContext {
                        session_id: session_id.clone(),
                        working_dir: working_dir.clone(),
                        call_id: call_id.clone(),
                        depth: identity.depth,
                        agent_role: identity.role.clone(),
                        permission_mode: mode.clone(),
                    };

                    // Redaction boundary — see `redact_result`. The tool is
                    // given the raw input; everything reported is redacted.
                    let reported = Self::reported_input(&input);

                    // Emit start event
                    emit_tool_start(ToolStartPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        input: reported.clone(),
                    });

                    // Execute. Permission was decided before the partition
                    // (P3-64) — this comment used to read "read-only always
                    // approved", which described the bypass rather than a
                    // policy.
                    let result = Self::redact_result(tool.execute(input.clone(), &ctx).await);

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

            for (tool, input, tool_use_id, original_idx) in write_tools {
                // Redaction boundary. The tool is given the raw input; the UI
                // event, the hook context and the `messages` row all get this.
                //
                // ⚠ The first A-01 fix redacted this branch's *result* and left
                // its *input* raw, so the write path still carried credentials
                // to the renderer and the database. Found while wiring A-08.
                let reported = Self::reported_input(&input);
                let call_id = Uuid::new_v4().to_string();
                let ctx = ToolContext {
                    session_id: self.session_id.clone(),
                    working_dir: self.working_dir.clone(),
                    // G-5: the child must not be able to outrank its parent.
                    // G-5: the child must not be able to outrank its parent.
                    permission_mode: self.mode.clone(),
                    call_id: call_id.clone(),
                    depth: self.identity.depth,
                    agent_role: self.identity.role.clone(),
                };

                // Emit start event
                emit_tool_start(ToolStartPayload {
                    session_id: self.session_id.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    input: reported.clone(),
                });

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
                        "effect": format!("{:?}", tool.effect()),
                        "concurrency_safe": tool.effect().is_concurrency_safe(),
                        "batch": true,
                    }),
                };
                let should_continue = HookRegistry::fire(&HookType::PreToolUse, &pre_ctx).await;

                let result = if !should_continue {
                    warn!("Tool '{}' blocked by PreToolUse hook", tool.name());
                    ToolResult::error(format!("Tool '{}' was blocked by a hook", tool.name()))
                } else {
                    debug!("Executing write tool: {}", tool.name());
                    Self::redact_result(tool.execute(input.clone(), &ctx).await)
                };

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

                results[original_idx] = (tool_use_id, result);
            }
        }

        Ok(results)
    }

    async fn request_permission(
        &mut self,
        call_id: &str,
        tool: &Arc<dyn Tool>,
        input: &serde_json::Value,
    ) -> Result<bool> {
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
            description: tool.description().to_string(),
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
        let outcome = tokio::select! {
            answer = rx => answer.ok(),

            _ = async {
                // Polled rather than edge-triggered: the flag is a `bool` in
                // shared state, not a channel, and adding a channel here would
                // mean a second cancellation mechanism beside the one every
                // other path already reads.
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    if self.state
                        .as_ref()
                        .map(|s| s.lock_tolerant().is_cancelled(&self.session_id))
                        .unwrap_or(false)
                    {
                        break;
                    }
                }
            } => {
                debug!(
                    "permission request for '{}' abandoned: the session was cancelled",
                    tool.name()
                );
                None
            }

            _ = tokio::time::sleep(PERMISSION_TIMEOUT) => {
                warn!(
                    "permission request for '{}' timed out after {}s with no answer. \
                     Treating it as a refusal — an unanswered prompt is not consent, and \
                     this is the branch a dead listener reaches.",
                    tool.name(),
                    PERMISSION_TIMEOUT.as_secs()
                );
                None
            }
        };

        // ⚠ The pending entry is removed on **every** outcome, including
        // timeout and cancellation. It was only ever removed by the responder,
        // so an abandoned prompt leaked its slot — `TG3-03` requires no
        // undrained `PendingPermissions` entry.
        {
            let mut pending = self.pending.lock_tolerant();
            pending.remove(call_id);
        }

        match outcome {
            Some(PermissionResponse::Allow) => Ok(true),
            Some(PermissionResponse::AllowAlways) => {
                // Add this tool to the always-allow set for this session
                self.always_allow.insert(tool.name().to_string());
                debug!(
                    "Tool '{}' added to always-allow for session {}",
                    tool.name(),
                    self.session_id
                );
                Ok(true)
            }
            Some(PermissionResponse::Deny) | None => Ok(false),
        }
    }
}
