// Session commands — list, get, delete sessions.

use std::sync::Arc;
use tauri::State;
use crate::errors::Result;
use crate::session::{manager::SessionManager, types::{Session, Message}};

#[tauri::command]
pub fn list_sessions(
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Vec<Session>> {
    session_mgr.list_sessions()
}

#[tauri::command]
pub fn get_session(
    session_id:  String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Option<Session>> {
    session_mgr.get_session(&session_id)
}

#[tauri::command]
pub fn get_session_messages(
    session_id:  String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Vec<Message>> {
    session_mgr.get_session_messages(&session_id)
}

#[tauri::command]
pub fn delete_session(
    session_id:  String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<()> {
    session_mgr.delete_session(&session_id)
}

#[tauri::command]
pub fn create_session(
    working_dir: String,
    provider:    String,
    model:       String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<Session> {
    session_mgr.create_session(&working_dir, &provider, &model)
}

#[tauri::command]
pub fn update_session_title(
    session_id: String,
    title:      String,
    session_mgr: State<'_, Arc<SessionManager>>,
) -> Result<()> {
    let session = session_mgr.get_session(&session_id)?
        .ok_or_else(|| crate::errors::AutoDevError::Session(format!("Session {} not found", session_id)))?;
    
    let mut updated_session = session;
    updated_session.title = title;
    session_mgr.update_session_title(&updated_session)
}
