// AgentEngine — the PA-mode agentic loop.
// State machine: Idle → Thinking → Streaming → ExecutingTool → Appending → Complete
// Pattern adopted from Goose's agent.rs reply_internal() and
// Claude Code's query.ts runQuery().
// See DEVELOPMENT_PLAN.md Section 9.4 for full state machine diagram.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::errors::{AutoDevError, Result};
use crate::events::{
    emit_agent_done, emit_agent_error, emit_thinking, emit_token,
    AgentDonePayload, AgentErrorPayload, ThinkingPayload, TokenPayload,
};
use crate::cost::tracker::CostTracker;
use crate::hooks::{HookRegistry, HookType, HookContext};
use crate::provider::types::{
    CompletionRequest, MessageContent, RequestMessage, StreamChunk,
};
use crate::session::manager::SessionManager;
use crate::state::AppState;
use super::builtin_tools;
use super::compaction::CompactionEngine;
use super::context::ContextBuilder;
use super::permission::{PendingPermissions, PermissionMode};
use super::tool_executor::ToolExecutor;
use super::tool_registry::ToolRegistry;
use super::tool_trait::Tool;
use crate::provider::traits::Provider;

const MAX_TOOL_TURNS: usize = 20;

pub struct AgentEngine {
    provider:     Arc<dyn Provider>,
    registry:     ToolRegistry,
    mode:         PermissionMode,
    pa_mode:      super::PAMode,
    pending:      PendingPermissions,
    app:          AppHandle,
    state:        Arc<Mutex<AppState>>,
    session_mgr:  Option<Arc<SessionManager>>,
    // Ruflo Feature 4 — Budget Circuit Breaker: set via with_task_budget()
    task_id:      Option<i64>,
    task_budget:  Option<crate::budget::TaskBudget>,
}

impl AgentEngine {
    pub fn new(
        provider: Arc<dyn Provider>,
        mode:     PermissionMode,
        pa_mode:  super::PAMode,
        app:      AppHandle,
        state:    Arc<Mutex<AppState>>,
        pending:  PendingPermissions,
    ) -> Self {
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, pa_mode);

        Self { provider, registry, mode, pa_mode, pending, app, state, session_mgr: None, task_id: None, task_budget: None }
    }

    /// Set the session manager for persisting messages
    pub fn with_session_manager(mut self, session_mgr: Arc<SessionManager>) -> Self {
        self.session_mgr = Some(session_mgr);
        self
    }

    /// Attach a task ID and spending budget so the engine enforces per-task token/cost/turn limits.
    /// Call this from the recipe runner before run() when executing a worker task.
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
        session_id:  &str,
        working_dir: &str,
        model:       &str,
        mut history: Vec<RequestMessage>,
        user_message: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<Vec<RequestMessage>> {
        // Check for cancellation before starting
        if self.state.lock().unwrap().is_cancelled(session_id) {
            return Err(AutoDevError::Agent("Agent was cancelled before start".into()));
        }

        let context_builder = ContextBuilder::new(working_dir)
            .with_session(session_id);
        let system_prompt   = if let Some(override_str) = system_prompt_override {
            override_str.to_string()
        } else {
            context_builder.build_system_prompt_with_mode(self.pa_mode, working_dir)
        };
        let tool_defs       = self.registry.to_definitions();

        // Add user message to history
        history.push(RequestMessage {
            role:    "user".to_string(),
            content: vec![MessageContent::Text { text: user_message.to_string() }],
        });

        let tool_executor = ToolExecutor::new(
            self.mode.clone(),
            self.pending.clone(),
            self.app.clone(),
            session_id.to_string(),
            working_dir.to_string(),
        );
        let mut tool_executor = match &self.session_mgr {
            Some(sm) => tool_executor.with_session_manager(sm.clone()),
            None => tool_executor,
        };

        let mut turn = 0;
        let mut total_tokens_in:  u32 = 0;
        let mut total_tokens_out: u32 = 0;
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
                    "total_tokens_in": total_tokens_in,
                    "total_tokens_out": total_tokens_out,
                }),
            };
            HookRegistry::fire(&HookType::AgentLoopStart, &loop_ctx).await;

            // Ruflo Feature 4 — Budget Circuit Breaker: pre-emptive halt before spending tokens
            budget_status.turns_used += 1;
            if let Some(ref budget) = self.task_budget {
                if let Some(reason) = budget.check(&budget_status) {
                    let reason_str = reason.as_event_str().to_string();
                    crate::events::emit_budget_exceeded(&self.app, crate::events::BudgetExceededEvent {
                        session_id:  session_id.to_string(),
                        task_id:     self.task_id,
                        reason:      reason_str.clone(),
                        tokens_used: budget_status.tokens_used,
                        cost_usd:    budget_status.cost_usd,
                        turns_used:  budget_status.turns_used,
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

            // Compact if context approaching limit — uses LLM summary when possible
            let model_context = 200_000u32;
            if CompactionEngine::needs_compaction(
                CompactionEngine::estimate_tokens(&history),
                model_context,
            ) {
                info!("Compacting conversation for session {session_id}");
                history = CompactionEngine::compact_with_summary(
                    history,
                    self.provider.as_ref(),
                    model,
                ).await;
            }

            // Cancel check
            if self.state.lock().unwrap().is_cancelled(session_id) {
                info!("Agent cancelled during turn {turn}");
                break;
            }

            let request = CompletionRequest {
                model:                  model.to_string(),
                messages:               history.clone(),
                system:                 Some(system_prompt.clone()),
                tools:                  tool_defs.clone(),
                max_tokens:             8192,
                temperature:            None,
                top_p:                  None,
                reasoning_effort:       None,
                thinking_budget_tokens: None,
            };

            debug!("Starting LLM stream turn={turn} session={session_id}");

            // Stream response from provider
            let stream_result = self.stream_response(
                session_id,
                model,
                request,
                &mut total_tokens_in,
                &mut total_tokens_out,
            ).await;

            // Ruflo Feature 4 — accumulate token usage into budget_status after each LLM response
            if let Ok(ref _r) = stream_result {
                budget_status.tokens_used = (total_tokens_in + total_tokens_out) as u64;
                budget_status.cost_usd    = CostTracker::calculate(model, total_tokens_in, total_tokens_out).cost_usd;
                if let Some(tid) = self.task_id {
                    if let Ok(db) = crate::db::tasks::TasksDb::open() {
                        let _ = db.update_task_budget_usage(
                            tid,
                            budget_status.tokens_used,
                            budget_status.cost_usd,
                            budget_status.turns_used,
                        );
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

                    emit_agent_error(&self.app, AgentErrorPayload {
                        session_id: session_id.to_string(),
                        message:    e.to_string(),
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
                role:    "assistant".to_string(),
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

            // Assemble results in order (unknown tools first, then batch results)
            let mut tool_results: Vec<MessageContent> = unknown_tools
                .into_iter()
                .map(|(tool_use_id, name)| MessageContent::ToolResult {
                    tool_use_id,
                    content: format!("Unknown tool: {name}"),
                    is_error: true,
                })
                .collect();

            tool_results.extend(batch_results.into_iter().map(|(tool_use_id, result)| {
                // Append hint inline so the LLM reads it as part of the result content.
                let content = match result.hint {
                    Some(hint) => format!("{}\n\n[HINT] {}", result.output, hint),
                    None       => result.output,
                };
                MessageContent::ToolResult {
                    tool_use_id,
                    content,
                    is_error: result.is_error,
                }
            }));

            // Add tool results as user message (Anthropic convention)
            history.push(RequestMessage {
                role:    "user".to_string(),
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
                "total_tokens_in": total_tokens_in,
                "total_tokens_out": total_tokens_out,
            }),
        };
        HookRegistry::fire(&HookType::AgentLoopEnd, &loop_end_ctx).await;

        // Record cost in app state
        let cost = CostTracker::calculate(model, total_tokens_in, total_tokens_out);
        {
            let mut state = self.state.lock().unwrap();
            state.add_cost(cost.cost_usd, total_tokens_in, total_tokens_out);
        }

        // Phase 7.5 — Record token usage in analytics DB.
        // Infer provider from model name (engine only knows the model string).
        {
            let analytics_provider = if model.contains("claude") {
                "anthropic"
            } else if model.contains("gpt") || model.starts_with("o1") || model.starts_with("o3") {
                "openai"
            } else if model.contains("gemini") {
                "google"
            } else {
                "unknown"
            };
            let _ = crate::analytics::global()
                .collector()
                .record_token_usage(
                    session_id.to_string(),
                    "mode1_agent".to_string(),
                    analytics_provider.to_string(),
                    model.to_string(),
                    total_tokens_in as i32,
                    total_tokens_out as i32,
                    None,
                    cost.cost_usd,
                    "agent_session".to_string(),
                );
        }

        emit_agent_done(&self.app, AgentDonePayload {
            session_id:  session_id.to_string(),
            message_id:  Uuid::new_v4().to_string(),
            tokens_in:   total_tokens_in,
            tokens_out:  total_tokens_out,
            cost_usd:    cost.cost_usd,
        });

        Ok(history)
    }

    /// Stream a single LLM response, emitting token events to the frontend.
    /// Returns the fully assembled response content blocks.
    async fn stream_response(
        &self,
        session_id:       &str,
        _model:           &str,
        request:          CompletionRequest,
        total_tokens_in:  &mut u32,
        total_tokens_out: &mut u32,
    ) -> Result<Vec<MessageContent>> {
        use futures::StreamExt;

        let mut stream = self.provider.stream(request).await?;

        let mut text_buffer = String::new();
        // Keyed by tool use UUID — accumulates (tool_name, partial_input_json)
        let mut tool_use_map: HashMap<String, (String, String)> = HashMap::new();
        // Maps content block index → tool use UUID for ToolInputDelta correlation
        let mut index_to_id: HashMap<u64, String> = HashMap::new();
        let mut content_blocks: Vec<MessageContent> = Vec::new();

        while let Some(chunk) = stream.next().await {
            match chunk? {
                StreamChunk::TextDelta { text } => {
                    emit_token(&self.app, TokenPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                    text_buffer.push_str(&text);
                }
                StreamChunk::ThinkingDelta { text } => {
                    emit_thinking(&self.app, ThinkingPayload {
                        session_id: session_id.to_string(),
                        text: text.clone(),
                    });
                }
                StreamChunk::ToolUseStart { id, name, block_index } => {
                    debug!("Tool use start: name={name} id={id} block_index={block_index}");
                    index_to_id.insert(block_index, id.clone());
                    tool_use_map.insert(id, (name, String::new()));
                }
                StreamChunk::ToolInputDelta { block_index, partial_json } => {
                    if let Some(tool_id) = index_to_id.get(&block_index) {
                        if let Some(entry) = tool_use_map.get_mut(tool_id) {
                            entry.1.push_str(&partial_json);
                        }
                    } else {
                        debug!("ToolInputDelta: no tool_id for block_index={block_index}");
                    }
                }
                StreamChunk::MessageDelta { tokens_in, tokens_out, .. } => {
                    *total_tokens_in  += tokens_in;
                    *total_tokens_out += tokens_out;
                }
                StreamChunk::Done => break,
            }
        }

        // Assemble content blocks from buffered data
        if !text_buffer.is_empty() {
            content_blocks.push(MessageContent::Text { text: text_buffer });
        }
        for (id, (name, json_str)) in tool_use_map {
            let input = serde_json::from_str(&json_str)
                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
            content_blocks.push(MessageContent::ToolUse { id, name, input });
        }

        Ok(content_blocks)
    }

    // ── Ruflo Feature 4 — Budget escalation helper ────────────────────────────
    // Creates a WorkerRequest of type report_issue and transitions the task to
    // needs_info so the Supervisor can decide to increase budget, split the task,
    // or reassign. Infallible — logs on DB failure so the budget halt still works.
    fn escalate_budget_exceeded(
        &self,
        task_id:    i64,
        status:     &crate::budget::BudgetStatus,
        reason_str: &str,
    ) {
        let budget_limit_str = self.task_budget.as_ref().map(|b| {
            format!(
                "{} tokens / ${:.2} USD / {} turns",
                b.max_tokens.map_or("unlimited".to_string(), |t| t.to_string()),
                b.max_usd.map_or(0.0, |u| u),
                b.max_turns.map_or(0, |t| t),
            )
        }).unwrap_or_else(|| "unknown".to_string());

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
            worker_slug:  "engine".to_string(),
            task_id:      Some(task_id),
            request_type: "report_issue".to_string(),
            payload:      payload.to_string(),
        };

        match crate::db::tasks::TasksDb::open() {
            Ok(db) => {
                if let Err(e) = db.create_worker_request(&req_input) {
                    error!("[budget] failed to create worker request for task {task_id}: {e}");
                }
                if let Err(e) = db.mark_task_budget_exceeded(task_id, reason_str) {
                    error!("[budget] failed to mark task {task_id} budget-exceeded: {e}");
                }
                crate::events::emit_task_changed_global(crate::events::TaskChangedPayload {
                    task_id:         Some(task_id),
                    kind:            "budget_exceeded".to_string(),
                    status:          Some("needs_info".to_string()),
                    assigned_worker: None,
                });
            }
            Err(e) => {
                error!("[budget] failed to open tasks DB for escalation on task {task_id}: {e}");
            }
        }
    }
}
