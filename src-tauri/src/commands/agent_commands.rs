// Agent commands — send messages, cancel, respond to permission dialogs.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tracing::info;

use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::session::{manager::SessionManager, types::Message};
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

/// Send a user message to the agent.
/// Fires up the PA-mode agentic loop in a background tokio task.
/// Streams events: agent:token, agent:tool-start, agent:tool-end, agent:done, agent:error
#[tauri::command]
pub async fn send_message(
    session_id: String,
    user_message: String,
    pa_mode: Option<String>, // "fast_development" | "review" | "delegate"
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    session_mgr: State<'_, Arc<SessionManager>>,
    pending_perms: State<'_, PendingPermissions>,
) -> Result<String> {
    // Resolve active provider + model from app state
    let (provider_id, model) = {
        let s = state.lock_tolerant();
        let p = s
            .active_provider
            .clone()
            .ok_or_else(|| AutoDevError::Agent("No active provider set".into()))?;
        let m = s
            .active_model
            .clone()
            .ok_or_else(|| AutoDevError::Agent("No active model set".into()))?;
        (p, m)
    };

    // Load API key from vault. Every supported provider needs one; the
    // keyless local-provider branch went with Ollama (P2-11).
    // Every supported provider takes a key; the keyless branch went with
    // Ollama (P2-11).
    let api_key = VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
        AutoDevError::Agent(format!("No API key for provider '{provider_id}'"))
    })?;

    // Load session history
    let session = session_mgr
        .get_session(&session_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Session '{session_id}' not found")))?;

    let working_dir = session.working_dir.clone();

    // Register agent as running in app state
    {
        let mut s = state.lock_tolerant();
        s.agents.insert(
            session_id.clone(),
            crate::state::app_state::ActiveAgent {
                session_id: session_id.clone(),
                is_running: true,
                cancel_flag: false,
            },
        );
    }

    // Save user message immediately
    let user_msg = Message::new_user(&session_id, &user_message);
    session_mgr.append_message(&user_msg)?;

    // Phase 4.1 — store to episodic memory (Tier 1).
    // Fire-and-forget: errors are logged but don't block the agent.
    {
        let sid = session_id.clone();
        let msg = user_message.clone();
        let wdir = working_dir.clone();
        std::thread::spawn(move || {
            if let Err(e) =
                crate::memory::EpisodicStore::store_user_message(&sid, &msg, Some(&wdir), None)
            {
                tracing::warn!("[memory] failed to store user message: {e}");
            }
            // Rule-based semantic extraction from user messages.
            crate::memory::SemanticStore::extract_and_store(&msg, &sid, Some(&wdir));
        });
    }

    // Build conversation history from stored messages (Anthropic format)
    let history = build_history(&session.messages);

    // Spawn agent in background; events streamed to frontend
    let app_clone = app.clone();
    let state_clone = state.inner().clone();
    let session_mgr_clone = session_mgr.inner().clone();
    let session_id_c = session_id.clone();
    let model_c = model.clone();
    let working_dir_c = working_dir.clone();
    let user_msg_c = user_message.clone();

    let pending_clone = pending_perms.inner().clone();

    // Capture system prompt override before spawning
    let system_prompt_override = {
        let s = state.lock_tolerant();
        s.system_prompt_override.clone()
    };

    let provider_id_c = provider_id.clone();

    // Phase 3.0 R3 — session row is the source of truth for PA mode. The
    // optional `pa_mode` argument from the frontend is accepted only as a
    // last-resort fallback (e.g. a legacy caller that hasn't been updated)
    // and is overridden whenever the session has a stored mode. This stops
    // the "PA visibly switches back to Mode 1" regression where a reload
    // reset the frontend store before `send_message` fired.
    let session_mode = session.pa_mode.clone();
    let effective_mode_str: &str = if !session_mode.is_empty() {
        session_mode.as_str()
    } else {
        pa_mode.as_deref().unwrap_or("fast_development")
    };
    let resolved_pa_mode = match effective_mode_str {
        "review" => crate::agent::PAMode::Review,
        "delegate" => crate::agent::PAMode::Delegate,
        _ => crate::agent::PAMode::FastDevelopment,
    };

    tokio::spawn(async move {
        // ⚠ P3-05. This `match` was a sixth copy of the provider ladder, and
        // its `_ =>` arm **returned without emitting any terminal event** — so
        // an unsupported provider left the UI spinning forever on a task that
        // was already dead. `TG3-01` names "unsupported provider" as one of the
        // exit paths that must emit exactly one terminal event.
        //
        // Built through the shared factory, which also applies the daily
        // ceiling (P2-21) rather than each caller remembering to wrap.
        let provider = match autodev_core::provider::factory::build(&provider_id_c, api_key) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Unsupported provider {provider_id_c}: {e}");
                autodev_core::events::emit_agent_error(
                    autodev_core::events::AgentErrorPayload {
                        session_id: session_id_c.clone(),
                        message: format!("Unsupported provider: {provider_id_c}"),
                    },
                );
                let mut s = state_clone.lock_tolerant();
                if let Some(agent) = s.agents.get_mut(&session_id_c) {
                    agent.is_running = false;
                    agent.cancel_flag = false;
                }
                return;
            }
        };

        let engine = AgentEngine::new(
            provider,
            PermissionMode::ManualApproval,
            resolved_pa_mode,
            state_clone.clone(),
            pending_clone,
            // The interactive assistant is the supervisor — the same role the
            // shipped recipes give it (recipes/defaults.rs:217).
            crate::agent::tool_trait::AgentIdentity::supervisor(),
        )
        .with_session_manager(session_mgr_clone.clone());

        // ── P3-02/P3-03 · the turn is contained, and the flag always clears ──
        //
        // `is_running` was cleared *after* the await. A panic inside the turn
        // therefore skipped it and left the session permanently marked running:
        // the user could never start another turn in that session, and nothing
        // said why.
        //
        // The guard clears it on **every** unwind path, including a panic,
        // because `Drop` runs during unwinding. It is deliberately a guard
        // rather than a line after `catch_unwind` — the latter is one early
        // `return` away from being skipped again.
        struct RunningGuard {
            state: std::sync::Arc<std::sync::Mutex<autodev_core::state::AppState>>,
            session_id: String,
        }
        impl Drop for RunningGuard {
            fn drop(&mut self) {
                let mut s = self.state.lock_tolerant();
                if let Some(agent) = s.agents.get_mut(&self.session_id) {
                    agent.is_running = false;
                    agent.cancel_flag = false;
                }
            }
        }
        let _running = RunningGuard {
            state: state_clone.clone(),
            session_id: session_id_c.clone(),
        };

        // How many assistant messages existed BEFORE this turn, so only the
        // ones this turn produced are persisted. Counted here because
        // `history` is moved into `run`.
        let prior_assistant_count = history.iter().filter(|m| m.role == "assistant").count();

        // `catch_unwind` contains a panicking tool or provider to THIS turn.
        //
        // ⚠ It is necessary and not sufficient. Unwinding is already on (see
        // `P3-01` — the `panic = "abort"` everyone believed was in force was in
        // a file Cargo discards), so a panic already failed only this task. What
        // it also did was **poison every mutex the panicking thread held**, and
        // this codebase had 98 `.lock().unwrap()` sites: one panic became a
        // cascade of unrelated panics at every later lock. `P3-03`'s
        // `lock_tolerant` is the other half.
        //
        // `AssertUnwindSafe` is required because `&mut` state crosses the
        // boundary. That is sound here precisely because of `lock_tolerant`:
        // the state is readable after a panic by design, and the fields it
        // holds carry no cross-field invariant a half-finished turn could break.
        let result = match futures::FutureExt::catch_unwind(std::panic::AssertUnwindSafe(
            engine.run(
                &session_id_c,
                &working_dir_c,
                &model_c,
                history,
                &user_msg_c,
                system_prompt_override.as_deref(),
            ),
        ))
        .await
        {
            Ok(r) => r,
            Err(panic) => {
                // The panic payload is usually a `&str` or `String`; anything
                // else is reported as such rather than dropped.
                let detail = panic
                    .downcast_ref::<&str>()
                    .map(|s| (*s).to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic payload".to_string());

                tracing::error!("Agent turn panicked and was contained: {detail}");
                Err(autodev_core::errors::AutoDevError::Agent(format!(
                    "The turn failed unexpectedly and was contained: {detail}"
                )))
            }
        };

        match result {
            Ok(updated_history) => {
                // ── P3-06 / P3-07 · persist the whole turn, not `.last()` ─────
                //
                // This read `updated_history.last()` and required it to be an
                // assistant message. Three defects followed:
                //
                //   1. On the `MAX_TOOL_TURNS` and cancel exits the last entry
                //      is a **user** message (tool results are sent as one), so
                //      the `role == "assistant"` test failed and **nothing was
                //      persisted at all** — the entire turn lost.
                //   2. `assistant_msg.content = text` **overwrote** inside the
                //      loop, so a multi-block assistant turn kept only its last
                //      text block and silently dropped the rest.
                //   3. `tool_use` blocks were never persisted, so a reloaded
                //      session could not show what the agent actually did.
                //
                // Every assistant message produced by this turn is persisted,
                // with its text blocks joined rather than overwritten.
                let assistant_messages: Vec<&crate::provider::types::RequestMessage> =
                    updated_history.iter().filter(|m| m.role == "assistant").collect();

                // Only the ones this turn added. `history` was the prior
                // conversation, so anything beyond its length is new.
                let new_assistant: Vec<_> = assistant_messages
                    .into_iter()
                    .skip(prior_assistant_count)
                    .collect();

                for msg in new_assistant {
                    {
                        let mut assistant_msg = Message::new_assistant(&session_id_c);

                        // Joined, not overwritten. A turn that reasons across
                        // several text blocks keeps all of them.
                        let text: String = msg
                            .content
                            .iter()
                            .filter_map(|b| match b {
                                crate::provider::types::MessageContent::Text { text } => {
                                    Some(text.as_str())
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("");

                        // The tool calls this turn made, recorded so a reloaded
                        // session shows the work rather than only the prose.
                        let tool_names: Vec<String> = msg
                            .content
                            .iter()
                            .filter_map(|b| match b {
                                crate::provider::types::MessageContent::ToolUse {
                                    name, ..
                                } => Some(name.clone()),
                                _ => None,
                            })
                            .collect();

                        // ⚠ P3-07. An assistant row with empty content is not
                        // written. `build_history` rebuilds every stored row as
                        // a content block, so an empty one becomes
                        // `{"type":"text","text":""}` on **every later send in
                        // that session, forever** — and providers reject an
                        // empty content block.
                        //
                        // A turn that produced only tool calls is not empty:
                        // its record is the calls, so it is kept with a
                        // synthesised summary rather than dropped.
                        if text.trim().is_empty() && tool_names.is_empty() {
                            tracing::debug!(
                                "skipping an empty assistant message for session \
                                 {session_id_c} — persisting it would poison every later \
                                 send in this session"
                            );
                            continue;
                        }

                        assistant_msg.content = if text.trim().is_empty() {
                            format!("[called {}]", tool_names.join(", "))
                        } else {
                            text
                        };
                        if !tool_names.is_empty() {
                            assistant_msg.tool_name = Some(tool_names.join(","));
                        }
                        // Phase 4.1 — store agent response to episodic memory.
                        let agent_text = assistant_msg.content.clone();
                        let sid2 = session_id_c.clone();
                        let wdir2 = working_dir_c.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = crate::memory::EpisodicStore::store_agent_response(
                                &sid2,
                                &agent_text,
                                None,
                                0.0,
                                0,
                                0,
                                None,
                                None,
                                Some(&wdir2),
                                None,
                            ) {
                                tracing::warn!("[memory] failed to store agent response: {e}");
                            }
                            // Phase 4.5 — trigger auto-summarization every N turns.
                            crate::memory::Summarizer::maybe_summarize(&sid2, Some(&wdir2));
                        });
                        if let Err(e) = session_mgr_clone.append_message(&assistant_msg) {
                            tracing::error!("Failed to save assistant message: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Agent error for session {session_id_c}: {e}");
            }
        }
    });

    info!("Agent started for session {session_id}");
    Ok(session_id)
}

/// Cancel a running agent session.
#[tauri::command]
pub fn cancel_agent(session_id: String, state: State<'_, Arc<Mutex<AppState>>>) -> Result<()> {
    let mut s = state.lock_tolerant();
    s.cancel_agent(&session_id);
    Ok(())
}

/// Respond to a permission dialog from the frontend.
#[tauri::command]
pub fn respond_to_permission(
    call_id: String,
    response: String,
    pending_perms: State<'_, PendingPermissions>,
) -> Result<()> {
    let perm_response = match response.as_str() {
        "allow" => PermissionResponse::Allow,
        "allow_always" => PermissionResponse::AllowAlways,
        _ => PermissionResponse::Deny,
    };
    let mut pending = pending_perms.lock_tolerant();
    if let Some(tx) = pending.remove(&call_id) {
        info!("Permission granted call_id={call_id} response={response}");
        let _ = tx.send(perm_response);
    } else {
        info!("Permission response for unknown call_id={call_id} (already resolved?)");
    }
    Ok(())
}

use crate::provider::types::{MessageContent, RequestMessage};
use autodev_core::state::lock::LockTolerant;

fn build_history(messages: &[Message]) -> Vec<RequestMessage> {
    use crate::session::types::MessageRole;
    messages
        .iter()
        .filter_map(|msg| {
            let role = match msg.role {
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => return None, // Tool results folded into user messages
            };
            Some(RequestMessage {
                role: role.to_string(),
                content: vec![MessageContent::Text {
                    text: msg.content.clone(),
                }],
            })
        })
        .collect()
}
