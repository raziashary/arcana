// Session commands — list, get, delete sessions.

use crate::errors::ArcanaError;
use crate::errors::Result;
use crate::hooks::{HookContext, HookRegistry, HookType};
use crate::session::{
    manager::SessionManager,
    types::{Message, Session, PA_MODES},
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn list_sessions(session_mgr: State<'_, Arc<SessionManager>>) -> Result<Vec<Session>> {
    session_mgr.list_sessions()
}

#[tauri::command]
pub fn get_session(
    session_id: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Option<Session>> {
    session_mgr.get_session(&session_id)
}

#[tauri::command]
pub fn get_session_messages(
    session_id: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Vec<Message>> {
    session_mgr.get_session_messages(&session_id)
}

#[tauri::command]
pub fn delete_session(
    session_id: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<()> {
    // Fire SessionEnd hook before deleting
    let ctx = HookContext {
        hook_type: HookType::SessionEnd.as_str().to_string(),
        timestamp: chrono::Utc::now(),
        session_id: Some(session_id.clone()),
        agent_mode: None,
        tool_call: None,
        file_path: None,
        task_id: None,
        worker_id: None,
        extra: serde_json::json!({}),
    };

    // Use tauri async runtime to fire hook
    tauri::async_runtime::spawn(async move {
        HookRegistry::fire(&HookType::SessionEnd, &ctx).await;
    });

    session_mgr.delete_session(&session_id)
}

#[tauri::command]
pub fn create_session(
    working_dir: String,
    provider: String,
    model: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Session> {
    let session = session_mgr.create_session(&working_dir, &provider, &model)?;

    // Fire SessionStart hook after creation
    let ctx = HookContext {
        hook_type: HookType::SessionStart.as_str().to_string(),
        timestamp: chrono::Utc::now(),
        session_id: Some(session.id.clone()),
        agent_mode: None,
        tool_call: None,
        file_path: Some(working_dir),
        task_id: None,
        worker_id: None,
        extra: serde_json::json!({
            "provider": provider,
            "model": model,
        }),
    };

    // Use tauri async runtime to fire hook
    tauri::async_runtime::spawn(async move {
        HookRegistry::fire(&HookType::SessionStart, &ctx).await;
    });

    Ok(session)
}

#[tauri::command]
pub fn update_session_title(
    session_id: String,
    title: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<()> {
    let session = session_mgr.get_session(&session_id)?.ok_or_else(|| {
        crate::errors::ArcanaError::Session(format!("Session {} not found", session_id))
    })?;

    let mut updated_session = session;
    updated_session.title = title;
    session_mgr.update_session_title(&updated_session)
}

/// Phase 3.0 R3 — persist the PA mode for a session so it survives reloads
/// and engine rebuilds. The frontend `settingsStore.setPAMode` action calls
/// this whenever the active session is known; `send_message` reads the
/// stored value as the source of truth.
#[tauri::command]
pub fn set_session_pa_mode(
    session_id: String,
    pa_mode: String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<()> {
    if !PA_MODES.contains(&pa_mode.as_str()) {
        return Err(ArcanaError::Session(format!(
            "Invalid pa_mode '{pa_mode}'. Expected one of: {}",
            PA_MODES.join(", ")
        )));
    }
    session_mgr.set_pa_mode(&session_id, &pa_mode)
}
