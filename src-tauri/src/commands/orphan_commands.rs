// Orphan detection commands — Tauri commands for PID-based orphan detection UI.

use crate::db::tasks::{Task, TasksDb};
use crate::errors::Result;
use crate::events::{emit_task_changed_global, TaskChangedPayload};

/// Return all tasks currently stuck in `assigned` or `in_progress` state,
/// with their PID and age. Used by the UI to surface potentially orphaned tasks.
#[tauri::command]
pub fn get_orphaned_tasks() -> Result<Vec<Task>> {
    let db = TasksDb::open()?;
    db.get_orphan_candidates()
}

/// Manually release a stuck task back to `ready`. Human override from the UI
/// for cases where automatic PID detection doesn't apply (remote worker, etc.).
#[tauri::command]
pub fn force_release_task(task_id: i64, reason: Option<String>) -> Result<()> {
    let db = TasksDb::open()?;
    let note = reason.as_deref().unwrap_or("manual release via UI");
    db.release_orphaned_task(task_id, note)?;
    emit_task_changed_global(TaskChangedPayload {
        task_id:         Some(task_id),
        kind:            "status".into(),
        status:          Some("ready".into()),
        assigned_worker: None,
    });
    Ok(())
}
