// Agent commands — send messages, cancel, respond to permission dialogs.

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};
use tracing::info;

use crate::errors::{AutoDevError, Result};
use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::traits::Provider;
use crate::session::{manager::SessionManager, types::Message};
use crate::state::AppState;
use crate::vault::keychain::{VaultManager, provider_key};

/// Send a user message to the agent.
/// Fires up the PA-mode agentic loop in a background tokio task.
/// Streams events: agent:token, agent:tool-start, agent:tool-end, agent:done, agent:error
#[tauri::command]
pub async fn send_message(
    session_id:   String,
    user_message: String,
    pa_mode:      Option<String>,  // "fast_development" | "review" | "delegate"
    app:          AppHandle,
    state:        State<'_, Arc<Mutex<AppState>>>,
    session_mgr:  State<'_, Arc<SessionManager>>,
    pending_perms: State<'_, PendingPermissions>,
) -> Result<String> {
    // Resolve active provider + model from app state
    let (provider_id, model) = {
        let s = state.lock().unwrap();
        let p = s.active_provider.clone()
            .ok_or_else(|| AutoDevError::Agent("No active provider set".into()))?;
        let m = s.active_model.clone()
            .ok_or_else(|| AutoDevError::Agent("No active model set".into()))?;
        (p, m)
    };

    // Load API key from vault (not needed for local Ollama provider)
    let api_key = if provider_id != "ollama" {
        VaultManager::get(&provider_key(&provider_id))?
            .ok_or_else(|| AutoDevError::Agent(format!("No API key for provider '{provider_id}'")))?
    } else {
        String::new()
    };

    // Load session history
    let session = session_mgr.get_session(&session_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Session '{session_id}' not found")))?;

    let working_dir = session.working_dir.clone();

    // Register agent as running in app state
    {
        let mut s = state.lock().unwrap();
        s.agents.insert(session_id.clone(), crate::state::app_state::ActiveAgent {
            session_id:  session_id.clone(),
            is_running:  true,
            cancel_flag: false,
        });
    }

    // Save user message immediately
    let user_msg = Message::new_user(&session_id, &user_message);
    session_mgr.append_message(&user_msg)?;

    // Build conversation history from stored messages (Anthropic format)
    let history = build_history(&session.messages);

    // Spawn agent in background; events streamed to frontend
    let app_clone    = app.clone();
    let state_clone  = state.inner().clone();
    let session_mgr_clone = session_mgr.inner().clone();
    let session_id_c = session_id.clone();
    let model_c      = model.clone();
    let working_dir_c = working_dir.clone();
    let user_msg_c   = user_message.clone();

    let pending_clone = pending_perms.inner().clone();

    // Capture system prompt override before spawning
    let system_prompt_override = {
        let s = state.lock().unwrap();
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
        "review"   => crate::agent::PAMode::Review,
        "delegate" => crate::agent::PAMode::Delegate,
        _          => crate::agent::PAMode::FastDevelopment,
    };

    tokio::spawn(async move {
        // Create provider based on provider_id
        let provider: Arc<dyn Provider> = match provider_id_c.as_str() {
            "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
            "openai"    => Arc::new(OpenAIProvider::new(api_key)),
            "google"    => Arc::new(GoogleProvider::new(api_key)),
            "ollama"    => Arc::new(OllamaProvider::default_from_env()),
            _ => {
                tracing::error!("Unsupported provider: {}", provider_id_c);
                return;
            }
        };

        let engine   = AgentEngine::new(
            provider,
            PermissionMode::ManualApproval,
            resolved_pa_mode,
            app_clone,
            state_clone.clone(),
            pending_clone,
        ).with_session_manager(session_mgr_clone.clone());

        let result = engine.run(
            &session_id_c,
            &working_dir_c,
            &model_c,
            history,
            &user_msg_c,
            system_prompt_override.as_deref(),
        ).await;

        // Clear running flag
        let mut s = state_clone.lock().unwrap();
        if let Some(agent) = s.agents.get_mut(&session_id_c) {
            agent.is_running  = false;
            agent.cancel_flag = false;
        }

        match result {
            Ok(updated_history) => {
                // Extract assistant's last message from updated history
                // (Skip first message which is the user message we already saved)
                if let Some(last_msg) = updated_history.last() {
                    if last_msg.role == "assistant" {
                        let mut assistant_msg = Message::new_assistant(&session_id_c);
                        // Extract text content from content blocks
                        for block in &last_msg.content {
                            if let crate::provider::types::MessageContent::Text { text } = block {
                                assistant_msg.content = text.clone();
                            }
                        }
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
pub fn cancel_agent(
    session_id: String,
    state:      State<'_, Arc<Mutex<AppState>>>,
) -> Result<()> {
    let mut s = state.lock().unwrap();
    s.cancel_agent(&session_id);
    Ok(())
}

/// Respond to a permission dialog from the frontend.
#[tauri::command]
pub fn respond_to_permission(
    call_id:       String,
    response:      String,
    pending_perms: State<'_, PendingPermissions>,
) -> Result<()> {
    let perm_response = match response.as_str() {
        "allow"        => PermissionResponse::Allow,
        "allow_always" => PermissionResponse::AllowAlways,
        _              => PermissionResponse::Deny,
    };
    let mut pending = pending_perms.lock().unwrap();
    if let Some(tx) = pending.remove(&call_id) {
        info!("Permission granted call_id={call_id} response={response}");
        let _ = tx.send(perm_response);
    } else {
        info!("Permission response for unknown call_id={call_id} (already resolved?)");
    }
    Ok(())
}

use crate::provider::types::{MessageContent, RequestMessage};

fn build_history(messages: &[Message]) -> Vec<RequestMessage> {
    use crate::session::types::MessageRole;
    messages.iter().filter_map(|msg| {
        let role = match msg.role {
            MessageRole::User      => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool      => return None, // Tool results folded into user messages
        };
        Some(RequestMessage {
            role: role.to_string(),
            content: vec![MessageContent::Text { text: msg.content.clone() }],
        })
    }).collect()
}
