// Task board commands — Tauri commands for the Internal AutoDev Task Board.
// Frontend reads tasks via these commands to populate the AutoDev Panel kanban.
// PA's create_task tool writes directly via TasksDb; these expose read + status updates.

use crate::db::tasks::{Task, WorkerRequest, TasksDb};
use crate::errors::Result;
use crate::events::{emit_task_changed_global, TaskChangedPayload};

/// List all tasks, optionally filtered by status.
/// Status values: "ready" | "assigned" | "in_progress" | "qc_review" | "done" | "failed" | "cancelled"
#[tauri::command]
pub fn list_tasks(status: Option<String>) -> Result<Vec<Task>> {
    let db = TasksDb::open()?;
    db.list(status.as_deref())
}

/// Get a single task by id.
#[tauri::command]
pub fn get_task(id: i64) -> Result<Option<Task>> {
    let db = TasksDb::open()?;
    db.get_by_id(id)
}

/// Update a task's status and optionally its assigned worker.
/// Used by Supervisor (assigns), Workers (in_progress → done), and QC (qc_review → done/failed).
#[tauri::command]
pub fn update_task_status(id: i64, status: String, assigned_worker: Option<String>) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_status(id, &status, assigned_worker.as_deref())?;

    // Phase 3.0 R1 — push-based dispatch for UI-driven status edits (drag a
    // task back to Ready, reassign via kanban, etc.). Same semantics as the
    // agent tool path so the fleet behaves uniformly regardless of who moved
    // the task.
    match status.as_str() {
        "qc_review" => crate::recipes::scheduler::kick("qc-agent"),
        "assigned"  => {
            if let Some(ref w) = assigned_worker {
                crate::recipes::scheduler::kick(w);
            }
        }
        _ => {}
    }

    // Ruflo Feature 1 — record routing outcome when a task reaches a terminal state.
    // duration_ms and tokens are 0 here (no timing data available at command level);
    // the DB UPDATE only touches rows with outcome IS NULL so it's safe to call always.
    let task_id_str = id.to_string();
    match status.as_str() {
        "done" => {
            let _ = crate::routing::TaskRouter::record_outcome(&task_id_str, "success", 0, 0);
        }
        "failed" => {
            let _ = crate::routing::TaskRouter::record_outcome(&task_id_str, "failure", 0, 0);
        }
        "assigned" => {
            // Reassignment — mark any pending routing row as reassigned
            let _ = crate::routing::TaskRouter::record_outcome(&task_id_str, "reassigned", 0, 0);
        }
        _ => {}
    }

    emit_task_changed_global(TaskChangedPayload {
        task_id: Some(id),
        kind:    "status".into(),
        status:  Some(status),
        assigned_worker,
    });
    Ok(())
}

/// Delete a task permanently. Intended for cleanup only — prefer status updates.
#[tauri::command]
pub fn delete_task(id: i64) -> Result<()> {
    let db = TasksDb::open()?;
    db.delete(id)?;
    emit_task_changed_global(TaskChangedPayload {
        task_id: Some(id),
        kind:    "deleted".into(),
        status:  None,
        assigned_worker: None,
    });
    Ok(())
}

/// Return qc_review tasks with how long they've been waiting (seconds).
/// Used by the UI to surface QC backlog age and alert on stalled reviews.
#[tauri::command]
pub fn get_qc_queue() -> Result<Vec<(Task, i64)>> {
    let db = TasksDb::open()?;
    let tasks = db.list(Some("qc_review"))?;
    let now = chrono::Utc::now().timestamp();
    let queue: Vec<(Task, i64)> = tasks
        .into_iter()
        .map(|t| {
            let age = t
                .updated_at
                .parse::<chrono::DateTime<chrono::Utc>>()
                .map(|dt| now - dt.timestamp())
                .unwrap_or(0);
            (t, age)
        })
        .collect();
    Ok(queue)
}

/// Delete ALL tasks and reset the ID sequence.
/// Used by the "Clear Board" button to start fresh tracking.
#[tauri::command]
pub fn clear_all_tasks() -> Result<()> {
    let db = TasksDb::open()?;
    db.clear_all()?;
    emit_task_changed_global(TaskChangedPayload {
        task_id: None,
        kind:    "cleared".into(),
        status:  None,
        assigned_worker: None,
    });
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// Worker Requests — Fleet→Supervisor communication queue (UI read + resolve)
// ════════════════════════════════════════════════════════════════════════════

/// List worker requests, optionally filtered by status (pending | acknowledged | completed | rejected).
#[tauri::command]
pub fn list_worker_requests(status: Option<String>) -> Result<Vec<WorkerRequest>> {
    let db = TasksDb::open()?;
    db.list_worker_requests(status.as_deref())
}

/// Manually resolve a worker request from the UI (human override).
#[tauri::command]
pub fn resolve_worker_request(
    id: i64,
    action: String,
    note: Option<String>,
) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_worker_request_status(id, &action, note.as_deref())
}
