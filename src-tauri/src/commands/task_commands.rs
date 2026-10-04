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
    update_task_status_with_kick(id, status, assigned_worker, crate::recipes::scheduler::kick)
}

fn update_task_status_with_kick(
    id: i64,
    status: String,
    assigned_worker: Option<String>,
    mut kick_worker: impl FnMut(&str),
) -> Result<()> {
    let db = TasksDb::open()?;
    let previous_task = db.get_by_id(id)?;

    // Record only the prior owner's newest pending routing row. A new
    // `assigned_worker` is not the worker whose preceding run produced this
    // completion/reassignment outcome.
    let routing_outcome = match (status.as_str(), previous_task.as_ref()) {
        ("done", Some(task)) => task
            .assigned_worker
            .as_deref()
            .map(|worker| (worker, "success")),
        ("failed", Some(task)) => task
            .assigned_worker
            .as_deref()
            .map(|worker| (worker, "failure")),
        ("assigned", Some(task)) => task
            .assigned_worker
            .as_deref()
            .filter(|previous| Some(*previous) != assigned_worker.as_deref())
            .map(|worker| (worker, "reassigned")),
        _ => None,
    };
    if status == "qc_review" {
        let workspace = arcana_core::platform::workspace::resolve_workspace_dir(None)?;
        db.update_status_with_workspace_and_run_id(
            id,
            &status,
            assigned_worker.as_deref(),
            None,
            None,
            &workspace,
        )?;
    } else if let (Some(task), Some((worker, outcome))) = (previous_task.as_ref(), routing_outcome)
    {
        db.update_status_with_routing_outcome(
            id,
            task.status.as_str(),
            worker,
            &status,
            assigned_worker.as_deref(),
            None,
            outcome,
        )?;
    } else {
        db.update_status(id, &status, assigned_worker.as_deref())?;
    }

    // Persist the transition before waking a worker. The scheduler checks for
    // recoverable work synchronously; kicking first could let it observe the
    // old status and leave this assignment waiting for periodic reconciliation.
    match status.as_str() {
        "qc_review" => kick_worker("qc-agent"),
        "assigned" => {
            if let Some(ref worker) = assigned_worker {
                kick_worker(worker);
            }
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

#[cfg(test)]
mod qc_evidence_tests {
    use super::*;

    #[test]
    fn tauri_status_command_cannot_promote_without_build_evidence() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated task workspace");
        arcana_core::db::app_settings::set(
            arcana_core::db::app_settings::WORKSPACE_KEY,
            workspace.path().to_string_lossy().as_ref(),
        )
        .expect("configure task workspace");
        let db = TasksDb::open().expect("open isolated task DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "Tauri QC evidence gate".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "the Tauri status route uses the SQLite evidence guard".into(),
                created_by: Some("tg4-07-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", 992)
            .expect("claim task");

        let error = update_task_status(task.id, "qc_review".into(), Some("worker-backend".into()))
            .expect_err("Tauri must not bypass the database QC evidence guard");
        assert!(
            error
                .to_string()
                .contains("without a persisted project build batch"),
            "Tauri should surface the actionable shared TasksDb refusal: {error}"
        );
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            arcana_core::db::tasks::TaskStatus::InProgress
        );
    }

    #[tokio::test]
    async fn ui_assignment_persists_before_recipe_runner_dispatch() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let db = TasksDb::open().expect("open isolated task DB");
        let slug = "worker-ui-dispatch-order-test";
        let workers =
            arcana_core::db::workers::WorkersDb::open().expect("open isolated worker roster");
        let worker_id = workers
            .create(
                "UI dispatch test worker",
                slug,
                "backend",
                "gemini-3.8-flash",
                &[],
                120,
                None,
            )
            .expect("create worker");
        let recipe_id = "ui-dispatch-order-test";
        workers
            .bind_recipe(worker_id, recipe_id)
            .expect("bind test recipe");
        let recipes_dir = arcana_core::recipes::recipes_dir().expect("resolve isolated recipes");
        std::fs::create_dir_all(&recipes_dir).expect("create recipes directory");
        arcana_core::recipes::parser::save_recipe(
            &recipes_dir,
            &arcana_core::recipes::types::Recipe {
                version: "1.0.0".into(),
                id: recipe_id.into(),
                title: "UI dispatch ordering test".into(),
                description: None,
                schedule: Some("*/5 * * * *".into()),
                permission_mode: arcana_core::recipes::types::RecipePermissionMode::Autonomous,
                instructions: "Inspect the task board.".into(),
                agent_role: None,
                model: Some("gemini-3.8-flash".into()),
                is_system: false,
            },
        )
        .expect("save isolated recipe");
        let runner = arcana_core::recipes::runner::RecipeRunner::new_with_workspace_override(
            std::sync::Arc::new(std::sync::Mutex::new(
                arcana_core::state::AppState::default(),
            )),
            std::sync::Arc::new(
                arcana_core::session::manager::SessionManager::new()
                    .expect("create isolated session manager"),
            ),
            arcana_core::agent::permission::new_pending_permissions(),
            Some(home.path().to_string_lossy().into_owned()),
        );
        let scheduler =
            arcana_core::recipes::scheduler::RecipeScheduler::new(std::sync::Arc::new(runner));
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "Assignment dispatch ordering".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "the recipe runner sees persisted assignment at kick time"
                    .into(),
                created_by: Some("p4-04-test".into()),
            })
            .expect("create task");

        let mut kicked = false;
        update_task_status_with_kick(task.id, "assigned".into(), Some(slug.into()), |worker| {
            assert_eq!(worker, slug);
            let persisted = TasksDb::open()
                .expect("reopen task DB at kick time")
                .get_by_id(task.id)
                .expect("read task at kick time")
                .expect("assigned task exists");
            assert_eq!(
                persisted.status,
                arcana_core::db::tasks::TaskStatus::Assigned
            );
            assert_eq!(persisted.assigned_worker.as_deref(), Some(worker));
            kicked = true;
            scheduler.kick_worker(worker);
        })
        .expect("update status");

        assert!(kicked, "assignment should kick its worker after commit");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let worker = workers
                    .get_by_slug(slug)
                    .expect("read worker after dispatch")
                    .expect("test worker remains registered");
                if worker.status == arcana_core::db::workers::WorkerStatus::IdleNoProvider {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("real RecipeRunner should reach the no-provider boundary");
    }
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
