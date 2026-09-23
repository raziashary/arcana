// Task board commands — Tauri commands for the Internal Arcana Task Board.
// Frontend reads tasks via these commands to populate the Arcana Panel kanban.
// PA's create_task tool writes directly via TasksDb; these expose read + status updates.

use crate::db::tasks::{CreatePhasedTaskInput, PhaseSummary, Task, TasksDb, WorkerRequest};
use crate::errors::Result;
use crate::events::{
    emit_phase_complete, emit_task_changed, emit_task_unblocked, TaskChangedPayload,
};

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
        "assigned" => {
            if let Some(ref w) = assigned_worker {
                crate::recipes::scheduler::kick(w);
            }
        }
        _ => {}
    }

    // Ruflo Feature 1 — record routing outcome when a task reaches a terminal state.
    // `record_outcome_auto` computes duration_ms as now - assigned_at from the
    // pending routing_history row, so the affinity scorer ranks workers by both
    // success rate AND speed. Token counts remain 0 here (the recipe runner
    // tracks per-tick tokens separately via analytics::record_token_usage).
    let task_id_str = id.to_string();
    match status.as_str() {
        "done" => {
            let _ = crate::routing::TaskRouter::record_outcome_auto(&task_id_str, "success");
        }
        "failed" => {
            let _ = crate::routing::TaskRouter::record_outcome_auto(&task_id_str, "failure");
        }
        "assigned" => {
            // Reassignment — mark any pending routing row as reassigned
            let _ = crate::routing::TaskRouter::record_outcome_auto(&task_id_str, "reassigned");
        }
        _ => {}
    }

    // Ruflo Feature 5 — Blocker resolution hook.
    // When a task completes, unblock any tasks that were waiting on it.
    // Emit task:unblocked for each newly ready task, and task:phase-complete
    // if the completing task's phase is now fully done.
    if status == "done" {
        let db2 = TasksDb::open()?;
        if let Ok(unblocked_ids) = db2.unblock_ready_tasks(id) {
            for tid in &unblocked_ids {
                emit_task_unblocked(*tid);
            }
            // Ruflo Feature 5 — kick the fleet so unblocked tasks dispatch on
            // the next event-loop tick instead of waiting up to a full cron
            // cycle. Skipped when nothing was unblocked.
            if !unblocked_ids.is_empty() {
                crate::recipes::scheduler::kick_all_workers();
            }
        }
        // Check if the completed task's phase is fully done
        if let Ok(Some(completed_task)) = db2.get_by_id(id) {
            if let (Some(ref epic_id), Some(ref phase)) =
                (&completed_task.epic_id, &completed_task.phase)
            {
                if let Ok(summaries) = db2.get_phase_summary(epic_id) {
                    if let Some(summary) = summaries.iter().find(|s| &s.phase == phase) {
                        if summary.is_complete {
                            emit_phase_complete(
                                epic_id.clone(),
                                phase.clone(),
                                summary.milestone.clone(),
                            );
                        }
                    }
                }
            }
        }
    }

    emit_task_changed(TaskChangedPayload {
        task_id: Some(id),
        kind: "status".into(),
        status: Some(status),
        assigned_worker,
    });
    Ok(())
}

/// Delete a task permanently. Intended for cleanup only — prefer status updates.
#[tauri::command]
pub fn delete_task(id: i64) -> Result<()> {
    let db = TasksDb::open()?;
    db.delete(id)?;
    emit_task_changed(TaskChangedPayload {
        task_id: Some(id),
        kind: "deleted".into(),
        status: None,
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
            // P8-31 — parse through the shared helper, not `str::parse`.
            //
            // 🔴 `updated_at` is written by SQLite's `datetime('now')`, which
            // emits `"2026-09-01 06:23:58"`: a SPACE separator and **no
            // offset**. `DateTime<Utc>`'s `FromStr` wants RFC 3339, so it
            // failed on every row and `.unwrap_or(0)` turned the failure into
            // an age of **zero seconds** — so every review looked brand new and
            // the stalled-review alert could never fire. A silent wrong answer,
            // not an error.
            //
            // ⚠ `parse_db_timestamp` was built in `P1-13` for exactly this and
            // this call site was never wired to it (`L-C12`). `P1-13`'s sweep
            // searched for the string `parse_from_rfc3339`, which this line
            // does not contain — which is why the gate for this must be
            // behavioural rather than a grep.
            let age = arcana_core::db::timestamps::parse_db_timestamp(&t.updated_at)
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
    emit_task_changed(TaskChangedPayload {
        task_id: None,
        kind: "cleared".into(),
        status: None,
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
pub fn resolve_worker_request(id: i64, action: String, note: Option<String>) -> Result<()> {
    let db = TasksDb::open()?;
    db.update_worker_request_status(id, &action, note.as_deref())
}

// ════════════════════════════════════════════════════════════════════════════
// Epic & Phase Commands — Ruflo Feature 5 (Phase-Based Task Decomposition)
// ════════════════════════════════════════════════════════════════════════════

/// List all subtasks belonging to an epic, ordered by phase_order then id.
#[tauri::command]
pub fn get_epic_tasks(epic_id: String) -> Result<Vec<Task>> {
    let db = TasksDb::open()?;
    db.get_epic_tasks(&epic_id)
}

/// Return per-phase progress summaries for an epic including completion state
/// and unlock gating so the frontend can render a phased progress view.
#[tauri::command]
pub fn get_epic_phase_summary(epic_id: String) -> Result<Vec<PhaseSummary>> {
    let db = TasksDb::open()?;
    db.get_phase_summary(&epic_id)
}

/// Create a subtask inside an epic with full phase metadata.
/// Called by the Supervisor (or PA in FastDevelopment mode) when decomposing
/// an epic into phased work. The `blocked_by` list contains string task IDs
/// that must reach status=done before this task becomes ready.
#[tauri::command]
pub fn create_phased_task(
    epic_id: String,
    phase: String,
    phase_order: i32,
    title: String,
    description: Option<String>,
    priority: String,
    task_type: String,
    acceptance_criteria: String,
    blocked_by: Vec<String>,
    milestone: Option<String>,
) -> Result<Task> {
    let db = TasksDb::open()?;
    let task = db.create_phased_task(CreatePhasedTaskInput {
        epic_id,
        phase,
        phase_order,
        title,
        description,
        priority,
        task_type,
        acceptance_criteria,
        blocked_by,
        milestone,
        created_by: None,
    })?;
    emit_task_changed(TaskChangedPayload {
        task_id: Some(task.id),
        kind: "created".into(),
        status: Some(if task.epic_id.is_some() {
            "needs_info".into()
        } else {
            "ready".into()
        }),
        assigned_worker: None,
    });
    Ok(task)
}
