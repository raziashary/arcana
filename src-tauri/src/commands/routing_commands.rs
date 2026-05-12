// Routing commands — Tauri commands for the Semantic Task Routing feature.
// Exposes suggest_worker_for_task and get_worker_affinity_stats to the frontend.

use crate::db::tasks::TasksDb;
use crate::errors::Result;
use crate::routing::{AffinityScore, TaskRouter, WorkerAffinityStats};

/// Get the recommended worker for a task given its ID.
/// Used by the UI "Suggest Worker" button and by the Supervisor (via MCP tool).
#[tauri::command]
pub fn suggest_worker_for_task(task_id: i64) -> Result<AffinityScore> {
    let db = TasksDb::open()?;
    let task = db
        .get_by_id(task_id)?
        .ok_or_else(|| crate::errors::AutoDevError::Agent(
            format!("Task #{task_id} not found"),
        ))?;
    TaskRouter::suggest_worker(&task)
}

/// Return per-worker, per-task-type affinity stats for the WorkerAffinityPanel.
#[tauri::command]
pub fn get_worker_affinity_stats() -> Result<Vec<WorkerAffinityStats>> {
    TaskRouter::get_affinity_stats()
}
