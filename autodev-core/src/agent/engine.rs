// AgentEngine — the PA-mode agentic loop.
// State machine: Idle → Thinking → Streaming → ExecutingTool → Appending → Complete
// Pattern adopted from Goose's agent.rs reply_internal() and
// Claude Code's query.ts runQuery().
// See DEVELOPMENT_PLAN.md Section 9.4 for full state machine diagram.

use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::builtin_tools;
use super::compaction::CompactionEngine;
use super::context::ContextBuilder;
use super::permission::{PendingPermissions, PermissionMode};
use super::tool_executor::ToolExecutor;
use super::tool_registry::ToolRegistry;
use super::tool_trait::Tool;
use crate::cost::tracker::CostTracker;
use crate::errors::{AutoDevError, Result};
use crate::events::{
    emit_agent_done, emit_agent_error, emit_thinking, emit_token, AgentDonePayload,
    AgentErrorPayload, ThinkingPayload, TokenPayload,
};
use crate::hooks::{HookContext, HookRegistry, HookType};
use crate::provider::traits::Provider;
use crate::provider::types::{CompletionRequest, MessageContent, RequestMessage, StreamChunk};
use crate::session::manager::SessionManager;
use crate::state::AppState;
use crate::provider::usage::Usage;

const MAX_TOOL_TURNS: usize = 20;

pub struct AgentEngine {
    provider: Arc<dyn Provider>,
    registry: ToolRegistry,
    mode: PermissionMode,
    pa_mode: super::PAMode,
    pending: PendingPermissions,
    state: Arc<Mutex<AppState>>,
    /// Who this engine runs as, and its nesting depth. Threaded into every
    /// ToolContext, so tool-side identity and the recursion guard depend on the
    /// spawn tree rather than on anything the model says about itself.
    identity: super::tool_trait::AgentIdentity,
    session_mgr: Option<Arc<SessionManager>>,
    // Ruflo Feature 4 — Budget Circuit Breaker: set via with_task_budget()
    task_id: Option<i64>,
    task_budget: Option<crate::budget::TaskBudget>,
}

impl AgentEngine {
    pub fn new(
        provider: Arc<dyn Provider>,
        mode: PermissionMode,
        pa_mode: super::PAMode,
        state: Arc<Mutex<AppState>>,
        pending: PendingPermissions,
        identity: super::tool_trait::AgentIdentity,
    ) -> Self {
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, pa_mode);

        Self {
            provider,
            registry,
            mode,
            pa_mode,
            pending,
            state,
            identity,
            session_mgr: None,
            task_id: None,
            task_budget: None,
        }
    }

    /// Set the session manager for persisting messages
    pub fn with_session_manager(mut self, session_mgr: Arc<SessionManager>) -> Self {
        self.session_mgr = Some(session_mgr);
        self
    }

    /// Attach a task ID and spending budget so the engine enforces per-task token/cost/turn limits.
    /// Call this from the recipe runner before run() when executing a worker task.
    /// Attach a budget with no task id.
    ///
    /// ⚠ **This exists because `with_task_budget` had zero callers for the
    /// whole life of the codebase, and its signature is why.** It demands a
    /// `task_id`, and neither autonomous path has one at construction: the
    /// recipe runner builds the engine before the agent claims a task, and a
    /// subagent has no task at all. So ~100 lines of enforcement — the guard,
    /// `escalate_budget_exceeded`, `TaskBudget::default_worker`,
    /// `BudgetExceededReason` — were unreachable, and **the app had no enforced
    /// spend limit at any scope**.
    ///
    /// A budget is not a property of having a task. It is a property of running
    /// autonomously.
    pub fn with_budget(mut self, budget: crate::budget::TaskBudget) -> Self {
        self.task_budget = Some(budget);
        self
    }

    /// Attach a budget **and** the task it belongs to, so an overrun can be
    /// escalated against that task.
    pub fn with_task_budget(mut self, task_id: i64, budget: crate::budget::TaskBudget) -> Self {
        self.task_id = Some(task_id);
        self.task_budget = Some(budget);
        self
    }

    /// Run the full agentic loop for a single user message.
    /// Streams tokens and tool events to the frontend via Tauri events.
    /// Returns after the LLM stops tool use (stop_reason = "end_turn").
    pub async fn run(
        &self,
        session_id: &str,
        working_dir: &str,
        model: &str,
        mut history: Vec<RequestMessage>,
        user_message: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<Vec<RequestMessage>> {
        // Check for cancellation before starting
        if self.state.lock().unwrap().is_cancelled(session_id) {
            return Err(AutoDevError::Agent(
                "Agent was cancelled before start".into(),
            ));
        }

        let context_builder = ContextBuilder::new(working_dir).with_session(session_id);
        let system_prompt = if let Some(override_str) = system_prompt_override {
            override_str.to_string()
        } else {
            context_builder.build_system_prompt_with_mode(self.pa_mode, working_dir)
        };
        let tool_defs = self.registry.to_definitions();

        // Add user message to history
        history.push(RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: user_message.to_string(),
            }],
        });

        let tool_executor = ToolExecutor::new(
            self.mode.clone(),
            self.pending.clone(),
            session_id.to_string(),
            working_dir.to_string(),
            self.identity.clone(),
        );
        let mut tool_executor = match &self.session_mgr {
            Some(sm) => tool_executor.with_session_manager(sm.clone()),
            None => tool_executor,
        };

        let mut turn = 0;
        let mut total_usage = Usage::default();
        // Ruflo Feature 4 — cumulative budget usage tracked across all turns
        let mut budget_status = crate::budget::BudgetStatus::default();

        loop {
            // Fire AgentLoopStart hook at beginning of each turn
            let loop_ctx = HookContext {
                hook_type: HookType::AgentLoopStart.as_str().to_string(),
                timestamp: chrono::Utc::now(),
                session_id: Some(session_id.to_string()),
                agent_mode: Some(self.pa_mode.as_str().to_string()),
                tool_call: None,
                file_path: Some(working_dir.to_string()),
                task_id: None,
                worker_id: None,
                extra: serde_json::json!({
                    "turn": turn,
                    "total_tokens_in": total_usage.total_input(),
                    "total_tokens_out": total_usage.output,
                }),
            };
            HookRegistry::fire(&HookType::AgentLoopStart, &loop_ctx).await;

            // Ruflo Feature 4 — Budget Circuit Breaker: pre-emptive halt before spending tokens
            budget_status.turns_used += 1;
            if let Some(ref budget) = self.task_budget {
                if let Some(reason) = budget.check(&budget_status) {
                    let reason_str = reason.as_event_str().to_string();
                    crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                        session_id: session_id.to_string(),
                        task_id: self.task_id,
                        reason: reason_str.clone(),
                        tokens_used: budget_status.tokens_used,
                        cost_usd: budget_status.cost_usd,
                        turns_used: budget_status.turns_used,
                    });
                    if let Some(tid) = self.task_id {
                        self.escalate_budget_exceeded(tid, &budget_status, &reason_str);
                    }
                    return Err(AutoDevError::BudgetExceeded(reason_str));
                }
            }

            if turn >= MAX_TOOL_TURNS {
                error!("Max tool turns ({MAX_TOOL_TURNS}) reached for session {session_id}");
                break;
            }

            // Compact if context approaching limit — uses LLM summary when possible.
            //
            // P2-57. This was a hardcoded `200_000`, ignoring
            // `ModelInfo::context_window` which every adapter already populates.
            // Gemini's window is 1M, so a Gemini conversation was compacted at
            // **20% of its real capacity** — throwing away context the model
            // could still hold, and paying for a summarisation call to do it.
            let model_context = self
                .provider
                .models()
                .iter()
                .find(|m| m.id == model)
                .map(|m| m.context_window)
                // A model the adapter does not list falls back to the smallest
                // window we ship rather than the largest: compacting early is
                // wasteful, compacting late overflows the request and fails the
                // turn outright.
                .unwrap_or(200_000);
            if CompactionEngine::needs_compaction(
                CompactionEngine::estimate_tokens(&history),
                model_context,
            ) {
                info!("Compacting conversation for session {session_id}");
                history =
                    CompactionEngine::compact_with_summary(history, self.provider.as_ref(), model)
                        .await;
            }

            // Cancel check
            if self.state.lock().unwrap().is_cancelled(session_id) {
                info!("Agent cancelled during turn {turn}");
                break;
            }

            let request = CompletionRequest {
                model: model.to_string(),
                messages: history.clone(),
                system: Some(system_prompt.clone()),
                tools: tool_defs.clone(),
                max_tokens: 8192,
                temperature: None,
                top_p: None,
                reasoning_effort: None,
                thinking_budget_tokens: None,
            };

            debug!("Starting LLM stream turn={turn} session={session_id}");

            // Stream response from provider
            let mut turn_stop_reason: Option<String> = None;
            let stream_result = self
                .stream_response(
                    session_id,
                    model,
                    request,
                    &mut total_usage,
                    &mut turn_stop_reason,
                )
                .await;

            // P2-46. A turn cut short by the token ceiling is not a finished
            // turn, and continuing the loop over a truncated assistant message
            // silently corrupts the conversation.
            if turn_stop_reason.as_deref() == Some("max_tokens") {
                warn!(
                    "Turn truncated by max_tokens (model={model}); the response is incomplete"
                );
            }

            // Ruflo Feature 4 — accumulate token usage into budget_status after each LLM response
            if let Ok(ref _r) = stream_result {
                budget_status.tokens_used = total_usage.total() as u64;
                // An unpriced model is a configuration error, not a $0 turn.
                // `TG-P2-02` gates that every model the UI offers is priced, so
                // reaching this branch means the model came from somewhere the
                // picker does not control. Logged at error level and NOT folded
                // into the running cost: adding 0.0 would under-report spend
                // and, once the cap lands, let an unpriced model spend past it
                // silently.
                match CostTracker::calculate(model, &total_usage) {
                    Ok(c) => budget_status.cost_usd = c.cost_usd,
                    Err(e) => error!("Cost unavailable for this turn: {e}"),
                }
                if let Some(tid) = self.task_id {
                    if let Ok(db) = crate::db::tasks::TasksDb::open() {
                        // P2-47. This was `let _ =`, in a function that logs
                        // loudly for every other DB failure. It is the ONLY
                        // persistence of running budget consumption, so a
                        // silent failure means a task's spend is invisible to
                        // the next process — and the per-task ceiling then
                        // enforces against a figure that stopped updating.
                        if let Err(e) = db.update_task_budget_usage(
                            tid,
                            budget_status.tokens_used,
                            budget_status.cost_usd,
                            budget_status.turns_used,
                        ) {
                            error!(
                                "Failed to persist budget usage for task #{tid}: {e}. \
                                 The ceiling for this task is now enforcing against a \
                                 stale figure."
                            );
                        }
                    }
                }
            }

            let response_content = match stream_result {
                Ok(content) => content,
                Err(e) => {
                    // Fire OnError hook
                    let error_ctx = HookContext {
                        hook_type: HookType::OnError.as_str().to_string(),
                        timestamp: chrono::Utc::now(),
                        session_id: Some(session_id.to_string()),
                        agent_mode: Some(self.pa_mode.as_str().to_string()),
                        tool_call: None,
                        file_path: Some(working_dir.to_string()),
                        task_id: None,
                        worker_id: None,
                        extra: serde_json::json!({
                            "error": e.to_string(),
                            "turn": turn,
                            "phase": "stream_response",
                        }),
                    };
                    HookRegistry::fire(&HookType::OnError, &error_ctx).await;

                    emit_agent_error(AgentErrorPayload {
                        session_id: session_id.to_string(),
                        message: e.to_string(),
                    });
                    return Err(e);
                }
            };

            // Collect tool calls from the response
            let tool_calls: Vec<&MessageContent> = response_content
                .iter()
                .filter(|c| matches!(c, MessageContent::ToolUse { .. }))
                .collect();

            // Add assistant message to history
            history.push(RequestMessage {
                role: "assistant".to_string(),
                content: response_content.clone(),
            });

            if tool_calls.is_empty() {
                // No tool calls — agent is done
                debug!("Agent done (no tool calls) at turn={turn}");
                break;
            }

            // Execute all tool calls with partitioned concurrency
            // Read-only tools run concurrently (max 10), write tools run serially
            let mut tool_batch: Vec<(Arc<dyn Tool>, serde_json::Value, String)> = Vec::new();
            let mut unknown_tools: Vec<(String, String)> = Vec::new(); // (tool_use_id, name)

            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, input } = tc {
                    match self.registry.get(name) {
                        Some(tool) => {
                            tool_batch.push((tool, input.clone(), id.clone()));
                        }
                        None => {
                            unknown_tools.push((id.clone(), name.clone()));
                        }
                    };
                }
            }

            // Execute batch with concurrency control
            let batch_results = if !tool_batch.is_empty() {
                tool_executor.execute_batch(tool_batch).await?
            } else {
                Vec::new()
            };

            // ── P2-61 · results follow the order of the CALLS ───────────
            //
            // This used to emit every unknown-tool error first and then the
            // batch results, so the result sequence did not parallel the
            // assistant message's tool_use sequence. Anthropic tolerates it;
            // OpenAI emits `tool` messages in exactly this vec order, so a
            // mismatch there pairs results with the wrong calls. It is also a
            // third source of message-level churn once caching is on.
            //
            // Collected into a map and then walked in `tool_calls` order, which
            // is the order the model emitted and (since P2-59) the order the
            // assistant message carries.
            let mut by_id: std::collections::HashMap<String, (String, bool)> =
                std::collections::HashMap::new();
            for (tool_use_id, name) in unknown_tools {
                by_id.insert(tool_use_id, (format!("Unknown tool: {name}"), true));
            }
            for (tool_use_id, result) in batch_results {
                // Hint appended inline so the model reads it as part of the result.
                let content = match result.hint {
                    Some(hint) => format!("{}\n\n[HINT] {}", result.output, hint),
                    None => result.output,
                };
                by_id.insert(tool_use_id, (content, result.is_error));
            }

            let mut tool_results: Vec<MessageContent> = Vec::new();
            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, .. } = tc {
                    if let Some((content, is_error)) = by_id.remove(id) {
                        tool_results.push(MessageContent::ToolResult {
                            tool_use_id: id.clone(),
                            name: name.clone(),
                            content,
                            is_error,
                        });
                    }
                }
            }
            // Anything left produced a result for a call the model did not make.
            // It cannot be placed in order, and dropping it silently would hide
            // a real executor bug — so it goes last, loudly.
            for (tool_use_id, (content, is_error)) in by_id {
                warn!("Tool result for an unrequested call id={tool_use_id}");
                tool_results.push(MessageContent::ToolResult {
                    tool_use_id,
                    name: String::new(),
                    content,
                    is_error,
                });
            }

            // Add tool results as user message (Anthropic convention)
            history.push(RequestMessage {
                role: "user".to_string(),
                content: tool_results,
            });

            turn += 1;
        }

        // Fire AgentLoopEnd hook when loop completes
        let loop_end_ctx = HookContext {
            hook_type: HookType::AgentLoopEnd.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some(session_id.to_string()),
            agent_mode: Some(self.pa_mode.as_str().to_string()),
            tool_call: None,
            file_path: Some(working_dir.to_string()),
            task_id: None,
            worker_id: None,
            extra: serde_json::json!({
                "final_turn": turn,
                "total_tokens_in": total_usage.total_input(),
                "total_tokens_out": total_usage.output,
            }),
        };
        HookRegistry::fire(&HookType::AgentLoopEnd, &loop_end_ctx).await;

        // ── P2-52 · a turn that did work must not report zero usage ──────
        //
        // Eight separate `.unwrap_or(0)` guards cover the usage fields across
        // three adapters, so a provider renaming a field degrades silently to
        // "this call was free" — no error, no log line, and with the spend cap
        // now live, a zero-cost turn passes every ceiling.
        //
        // Checked here as the INVARIANT rather than at each of the eight sites:
        // what matters is not which field was missing, it is that a turn which
        // produced output reported nothing. That is one condition, and it stays
        // true if a ninth extraction site is added.
        if total_usage.total() == 0 && turn > 0 {
            error!(
                "Provider '{}' returned content but reported ZERO tokens for model \
                 '{model}'. Cost and every spend ceiling are blind to this turn. \
                 The usage field names have most likely changed.",
                self.provider.id()
            );
        }

        // Record cost in app state. Tokens are recorded either way — they are
        // provider-reported fact; only the dollar figure depends on a table we
        // maintain, and a missing price must not erase the token record too.
        let cost_usd = match CostTracker::calculate(model, &total_usage) {
            Ok(c) => c.cost_usd,
            Err(e) => {
                error!("Cost unavailable for model '{model}': {e}");
                0.0
            }
        };
        {
            let mut state = self.state.lock().unwrap();
            state.add_cost(cost_usd, total_usage.total_input(), total_usage.output);
        }

        // P2-49. `emit_cost_update` had exactly ONE caller — the Mode 2 worker
        // tick — so the entire Mode 1 agent path was invisible to every cost
        // surface that listens for it, including the only budget warning the UI
        // had. Mode 1 emitted `agent_done` instead, which carries no cost.
        {
            let s = self.state.lock().unwrap();
            crate::events::emit_cost_update(crate::events::CostUpdatePayload {
                session_id: session_id.to_string(),
                tokens_in: total_usage.total_input(),
                tokens_out: total_usage.output,
                cost_usd,
                total_today: s.cost_today_usd,
            });
        }

        // Phase 7.5 — Record token usage in analytics DB.
        // Infer provider from model name (engine only knows the model string).
        {
            // P2-53. This substring ladder was written out here AND in the other
                            // runtime path, and they did not agree: `o4-mini`
                            // matched neither and was filed as "unknown",
                            // attributing real spend to a vendor that does not
                            // exist. One shared function now answers.
                            let analytics_provider =
                                crate::provider::routing::provider_for_analytics(model);
            // Phase 1 of docs/endtoendrun/14may2026/02 — fail loudly so a
            // future analytics regression shows up in the log instead of
            // silently zeroing the Dashboard.
            if let Err(e) = crate::analytics::global().collector().record_token_usage(
                session_id.to_string(),
                "mode1_agent".to_string(),
                analytics_provider.to_string(),
                model.to_string(),
                total_usage.total_input() as i32,
                total_usage.output as i32,
                None,
                cost_usd,
                "agent_session".to_string(),
            ) {
                error!("[Mode1] record_token_usage failed for session {session_id}: {e}");
            }
        }

        emit_agent_done(AgentDonePayload {
            session_id: session_id.to_string(),
            message_id: Uuid::new_v4().to_string(),
            tokens_in: total_usage.total_input(),
            tokens_out: total_usage.output,
            cost_usd: cost_usd,
        });

        Ok(history)
    }

    /// Stream a single LLM response, emitting token events to the frontend.
    /// Returns the fully assembled response content blocks.
    /// `stop_reason` is an out-parameter because the caller must be able to
    /// tell a `max_tokens` truncation from a clean finish (P2-46). It used to be
    /// destructured away with `..`, so the two were literally the same code
    /// path and silent truncation was invisible.
    async fn stream_response(
        &self,
        session_id: &str,
        model: &str,
        request: CompletionRequest,
        total_usage: &mut Usage,
        stop_reason: &mut Option<String>,
    ) -> Result<Vec<MessageContent>> {
        // P2-58: `model` was `_model` — a parameter passed live and never read,
        // sitting exactly where per-model streaming behaviour would go.
        debug!("stream_response: model={model}");
        use futures::StreamExt;

        let mut stream = self.provider.stream(request).await?;

        // ── Ordering (P2-59, P2-60) ──────────────────────────────────────
        //
        // Both maps are keyed by the wire's own **content block index** and are
        // `BTreeMap`s, so assembly below walks them in the order the model
        // actually emitted.
        //
        // This was a `HashMap` keyed by tool-use UUID. `std::collections::HashMap`
        // iterates in an order derived from a per-process `RandomState` seed, so
        // when a model emitted two or more parallel tool calls the block order
        // written into `history` was **arbitrary and differed between runs of the
        // same binary** — and that reordered assistant message is then replayed
        // on every subsequent turn. It corrupts the transcript relative to the
        // wire, and once caching is on it is a messages-level invalidator that
        // looks exactly like "caching doesn't work" while the prefix is perfectly
        // stable: the hardest possible bug to attribute.
        //
        // Text was separately buffered into one string and pushed **first**,
        // inverting any commentary the model emitted after a tool call.
        let mut text_by_block: std::collections::BTreeMap<u64, String> =
            std::collections::BTreeMap::new();
        // block index → (tool_use_id, tool_name, partial_input_json)
        let mut tool_use_map: std::collections::BTreeMap<u64, (String, String, String)> =
            std::collections::BTreeMap::new();
        let mut content_blocks: Vec<MessageContent> = Vec::new();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::TextDelta { text, block_index } => {
                    emit_token(TokenPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                    text_by_block.entry(block_index).or_default().push_str(&text);
                }
                StreamChunk::ThinkingDelta { text } => {
                    emit_thinking(ThinkingPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                }
                StreamChunk::ToolUseStart {
                    id,
                    name,
                    block_index,
                } => {
                    debug!("Tool use start: name={name} id={id} block_index={block_index}");
                    tool_use_map.insert(block_index, (id, name, String::new()));
                }
                StreamChunk::ToolInputDelta {
                    block_index,
                    partial_json,
                } => {
                    // Keyed directly by block index now, so the id->index
                    // correlation map is gone: it existed only because the
                    // accumulator was keyed by id, which is what made ordering
                    // depend on hash iteration in the first place.
                    if let Some(entry) = tool_use_map.get_mut(&block_index) {
                        entry.2.push_str(&partial_json);
                    } else {
                        debug!("ToolInputDelta: no tool started at block_index={block_index}");
                    }
                }
                StreamChunk::MessageDelta {
                    stop_reason: reason,
                    usage,
                } => {
                    total_usage.add(&usage);
                    // `message_start` opens the message and carries no stop
                    // reason; only a non-empty value is a real one.
                    if !reason.is_empty() {
                        *stop_reason = Some(reason);
                    }
                }
                StreamChunk::Done => break,
            }
        }

        content_blocks.extend(assemble_in_wire_order(text_by_block, tool_use_map));

        Ok(content_blocks)
    }

    // ── Ruflo Feature 4 — Budget escalation helper ────────────────────────────
    // Creates a WorkerRequest of type report_issue and transitions the task to
    // needs_info so the Supervisor can decide to increase budget, split the task,
    // or reassign. Infallible — logs on DB failure so the budget halt still works.
    fn escalate_budget_exceeded(
        &self,
        task_id: i64,
        status: &crate::budget::BudgetStatus,
        reason_str: &str,
    ) {
        let budget_limit_str = self
            .task_budget
            .as_ref()
            .map(|b| {
                format!(
                    "{} tokens / ${:.2} USD / {} turns",
                    b.max_tokens
                        .map_or("unlimited".to_string(), |t| t.to_string()),
                    b.max_usd.map_or(0.0, |u| u),
                    b.max_turns.map_or(0, |t| t),
                )
            })
            .unwrap_or_else(|| "unknown".to_string());

        let issue_description = format!(
            "Budget exceeded on task #{task_id}: halted at {} tokens / ${:.4} USD after {} turns. \
             Budget limit: {budget_limit_str}. Task needs budget increase or scope reduction.",
            status.tokens_used, status.cost_usd, status.turns_used,
        );

        let payload = serde_json::json!({
            "issue_description": issue_description,
            "severity": "high",
            "budget_reason": reason_str,
            "tokens_used": status.tokens_used,
            "cost_usd": status.cost_usd,
            "turns_used": status.turns_used,
        });

        let req_input = crate::db::tasks::CreateWorkerRequestInput {
            worker_slug: "engine".to_string(),
            task_id: Some(task_id),
            request_type: "report_issue".to_string(),
            payload: payload.to_string(),
        };

        match crate::db::tasks::TasksDb::open() {
            Ok(db) => {
                if let Err(e) = db.create_worker_request(&req_input) {
                    error!("[budget] failed to create worker request for task {task_id}: {e}");
                }
                if let Err(e) = db.mark_task_budget_exceeded(task_id, reason_str) {
                    error!("[budget] failed to mark task {task_id} budget-exceeded: {e}");
                }
                crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                    task_id: Some(task_id),
                    kind: "budget_exceeded".to_string(),
                    status: Some("needs_info".to_string()),
                    assigned_worker: None,
                });
            }
            Err(e) => {
                error!("[budget] failed to open tasks DB for escalation on task {task_id}: {e}");
            }
        }
    }
}

/// Merge buffered text and tool calls back into the order the model emitted.
///
/// Extracted from `stream_response` so the ordering can be tested without a
/// live provider — the defect it fixes (P2-59) was **non-deterministic across
/// processes**, which is exactly the kind that survives manual checking.
///
/// Both inputs are keyed by the wire's content-block index and are `BTreeMap`s,
/// so merging them is simply walking the union in key order.
fn assemble_in_wire_order(
    text_by_block: std::collections::BTreeMap<u64, String>,
    tool_use_map: std::collections::BTreeMap<u64, (String, String, String)>,
) -> Vec<MessageContent> {
    let mut ordered: std::collections::BTreeMap<u64, MessageContent> =
        std::collections::BTreeMap::new();

    for (idx, text) in text_by_block {
        // An empty text block is dropped rather than emitted: an empty content
        // block poisons every later send in the session (P3-07's defect).
        if !text.is_empty() {
            ordered.insert(idx, MessageContent::Text { text });
        }
    }
    for (idx, (id, name, json_str)) in tool_use_map {
        // A tool call whose arguments never arrived becomes `{}` rather than
        // failing the turn — the provider said the model wanted this tool, and
        // dropping the call entirely would leave the model waiting for a result
        // it never gets.
        let input = serde_json::from_str(&json_str)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
        ordered.insert(idx, MessageContent::ToolUse { id, name, input });
    }

    ordered.into_values().collect()
}

#[cfg(test)]
mod wire_order_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn tool(id: &str, name: &str, json: &str) -> (String, String, String) {
        (id.to_string(), name.to_string(), json.to_string())
    }

    #[test]
    fn blocks_come_back_in_the_order_the_model_emitted() {
        let mut text = BTreeMap::new();
        text.insert(1u64, "thinking about it".to_string());
        let mut tools = BTreeMap::new();
        tools.insert(0u64, tool("a", "read_file", r#"{"p":"a"}"#));
        tools.insert(2u64, tool("b", "grep", r#"{"q":"x"}"#));

        let out = assemble_in_wire_order(text, tools);
        assert_eq!(out.len(), 3);
        // Tool at 0, text at 1, tool at 2 — not "all text, then all tools".
        assert!(matches!(out[0], MessageContent::ToolUse { .. }));
        assert!(matches!(out[1], MessageContent::Text { .. }));
        assert!(matches!(out[2], MessageContent::ToolUse { .. }));
    }

    #[test]
    fn text_after_a_tool_call_stays_after_it() {
        // The old code pushed all buffered text FIRST, unconditionally, so a
        // model that called a tool and then commented had the two swapped —
        // and the swapped message is replayed on every later turn.
        let mut text = BTreeMap::new();
        text.insert(5u64, "and that is why".to_string());
        let mut tools = BTreeMap::new();
        tools.insert(1u64, tool("a", "read_file", "{}"));

        let out = assemble_in_wire_order(text, tools);
        assert!(matches!(out[0], MessageContent::ToolUse { .. }));
        assert!(matches!(out[1], MessageContent::Text { .. }));
    }

    #[test]
    fn parallel_tool_calls_keep_their_relative_order_every_time() {
        // The defect was non-deterministic ACROSS PROCESSES: HashMap iteration
        // is seeded per process, so a single run proved nothing. Ten identical
        // assemblies must agree, and with a BTreeMap they agree by construction
        // rather than by luck.
        let mut expected: Option<Vec<String>> = None;
        for _ in 0..10 {
            let mut tools = BTreeMap::new();
            for i in 0..6u64 {
                tools.insert(i, tool(&format!("id{i}"), &format!("tool{i}"), "{}"));
            }
            let names: Vec<String> = assemble_in_wire_order(BTreeMap::new(), tools)
                .into_iter()
                .map(|b| match b {
                    MessageContent::ToolUse { name, .. } => name,
                    _ => "?".to_string(),
                })
                .collect();
            match &expected {
                None => expected = Some(names),
                Some(e) => assert_eq!(e, &names, "assembly order must be stable"),
            }
        }
        assert_eq!(
            expected.unwrap(),
            vec!["tool0", "tool1", "tool2", "tool3", "tool4", "tool5"]
        );
    }

    #[test]
    fn an_empty_text_block_is_dropped() {
        // An empty content block poisons every later send in the session.
        let mut text = BTreeMap::new();
        text.insert(0u64, String::new());
        assert!(assemble_in_wire_order(text, BTreeMap::new()).is_empty());
    }

    #[test]
    fn a_tool_call_with_no_arguments_becomes_an_empty_object() {
        // Dropping the call would leave the model waiting for a result it never
        // gets; `{}` at least lets the tool refuse cleanly.
        let mut tools = BTreeMap::new();
        tools.insert(0u64, tool("a", "read_file", ""));
        let out = assemble_in_wire_order(BTreeMap::new(), tools);
        match &out[0] {
            MessageContent::ToolUse { input, .. } => {
                assert!(input.is_object() && input.as_object().unwrap().is_empty());
            }
            _ => panic!("expected a tool use"),
        }
    }
}
