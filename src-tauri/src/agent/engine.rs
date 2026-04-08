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
use crate::provider::types::{
    CompletionRequest, MessageContent, RequestMessage, StreamChunk,
};
use crate::state::AppState;
use super::builtin_tools;
use super::compaction::CompactionEngine;
use super::context::ContextBuilder;
use super::permission::{PendingPermissions, PermissionMode};
use super::tool_executor::ToolExecutor;
use super::tool_registry::ToolRegistry;
use crate::provider::traits::Provider;

const MAX_TOOL_TURNS: usize = 20;

pub struct AgentEngine {
    provider:    Arc<dyn Provider>,
    registry:    ToolRegistry,
    mode:        PermissionMode,
    pending:     PendingPermissions,
    app:         AppHandle,
    state:       Arc<Mutex<AppState>>,
}

impl AgentEngine {
    pub fn new(
        provider: Arc<dyn Provider>,
        mode:     PermissionMode,
        app:      AppHandle,
        state:    Arc<Mutex<AppState>>,
        pending:  PendingPermissions,
    ) -> Self {
        let mut registry = ToolRegistry::new();
        builtin_tools::register_all(&mut registry);

        Self { provider, registry, mode, pending, app, state }
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

        let context_builder = ContextBuilder::new(working_dir);
        let system_prompt   = context_builder.build_system_prompt(system_prompt_override);
        let tool_defs       = self.registry.to_definitions();

        // Add user message to history
        history.push(RequestMessage {
            role:    "user".to_string(),
            content: vec![MessageContent::Text { text: user_message.to_string() }],
        });

        let mut tool_executor = ToolExecutor::new(
            self.mode.clone(),
            self.pending.clone(),
            self.app.clone(),
            session_id.to_string(),
            working_dir.to_string(),
        );

        let mut turn = 0;
        let mut total_tokens_in:  u32 = 0;
        let mut total_tokens_out: u32 = 0;

        loop {
            if turn >= MAX_TOOL_TURNS {
                error!("Max tool turns ({MAX_TOOL_TURNS}) reached for session {session_id}");
                break;
            }

            // Compact if context approaching limit
            let model_context = 200_000u32;
            if CompactionEngine::needs_compaction(
                CompactionEngine::estimate_tokens(&history),
                model_context,
            ) {
                info!("Compacting conversation for session {session_id}");
                history = CompactionEngine::compact(history);
            }

            // Cancel check
            if self.state.lock().unwrap().is_cancelled(session_id) {
                info!("Agent cancelled during turn {turn}");
                break;
            }

            let request = CompletionRequest {
                model:       model.to_string(),
                messages:    history.clone(),
                system:      Some(system_prompt.clone()),
                tools:       tool_defs.clone(),
                max_tokens:  8192,
                temperature: None,
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

            let response_content = match stream_result {
                Ok(content) => content,
                Err(e) => {
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

            // Execute all tool calls; collect results
            let mut tool_results = Vec::new();
            for tc in &tool_calls {
                if let MessageContent::ToolUse { id, name, input } = tc {
                    let tool = match self.registry.get(name) {
                        Some(t) => t,
                        None    => {
                            let result = MessageContent::ToolResult {
                                tool_use_id: id.clone(),
                                content:     format!("Unknown tool: {name}"),
                                is_error:    true,
                            };
                            tool_results.push(result);
                            continue;
                        }
                    };

                    let exec_result = tool_executor.execute(tool, input.clone()).await?;
                    tool_results.push(MessageContent::ToolResult {
                        tool_use_id: id.clone(),
                        content:     exec_result.output,
                        is_error:    exec_result.is_error,
                    });
                }
            }

            // Add tool results as user message (Anthropic convention)
            history.push(RequestMessage {
                role:    "user".to_string(),
                content: tool_results,
            });

            turn += 1;
        }

        // Record cost in app state
        let cost = CostTracker::calculate(model, total_tokens_in, total_tokens_out);
        {
            let mut state = self.state.lock().unwrap();
            state.add_cost(cost.cost_usd, total_tokens_in, total_tokens_out);
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
}
