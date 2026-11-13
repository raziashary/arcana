// Routing commands — Tauri commands for the Semantic Task Routing feature.
// Exposes suggest_worker_for_task and get_worker_affinity_stats to the frontend.

use crate::errors::Result;
use crate::routing::{TaskRouter, WorkerAffinityStats};

/// Return per-worker, per-task-type affinity stats for the WorkerAffinityPanel.
#[tauri::command]
pub fn get_worker_affinity_stats() -> Result<Vec<WorkerAffinityStats>> {
    TaskRouter::get_affinity_stats()
}
