// worker_message_commands — Tauri commands for the Fleet Chat panel.
//
// These are read-only queries against the ACP `worker_messages` table.
// Writes happen exclusively through the agent tools (send_worker_message,
// acknowledge_worker_message) so the budget/orphan/audit guards stay in
// the canonical path.

use crate::db::tasks::{TasksDb, WorkerMessage};
use crate::errors::Result;

/// Full message history for one task, oldest first. Used by Fleet Chat
/// when the user clicks a task to expand its transcript.
#[tauri::command]
pub fn list_worker_messages_for_task(task_id: i64, limit: Option<i64>) -> Result<Vec<WorkerMessage>> {
    let db = TasksDb::open()?;
    db.list_worker_messages_for_task(task_id, limit.unwrap_or(500))
}

/// Recent messages across all tasks, newest first. Used by Fleet Chat on
/// initial mount to seed the transcript before the live `worker:message`
/// stream takes over.
#[tauri::command]
pub fn list_recent_worker_messages(limit: Option<i64>) -> Result<Vec<WorkerMessage>> {
    let db = TasksDb::open()?;
    db.list_recent_worker_messages(limit.unwrap_or(200))
}

/// Per-worker unread count. Used by AgentLiveCard to render the badge.
#[tauri::command]
pub fn count_unread_worker_messages_for(slug: String) -> Result<i64> {
    let db = TasksDb::open()?;
    db.count_unread_for(&slug)
}
