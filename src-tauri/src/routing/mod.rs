// Semantic Task Routing — stateless router that combines static keyword patterns
// with historical worker affinity to suggest the best worker for a task.
//
// Ruflo reference: hooks_route concept from v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts
// No submodule changes — all data is in tasks.db (routing_history table).

pub mod patterns;
pub mod scorer;

use serde::{Deserialize, Serialize};
use crate::db::tasks::{TasksDb, Task, RoutingHistoryEntry};
use crate::db::workers::WorkersDb;
use crate::errors::Result;
pub use scorer::AffinityScore;

// ─── Worker Affinity Stats (for UI + Tauri command) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAffinityStats {
    pub worker_slug:     String,
    pub task_type:       String,
    pub total_tasks:     i64,
    pub success_count:   i64,
    pub failure_count:   i64,
    pub success_rate:    f64,
    pub avg_duration_ms: Option<i64>,
}

// ─── TaskRouter — stateless; opens DB per call to match project conventions ──

pub struct TaskRouter;

impl TaskRouter {
    /// Suggest the best worker for a task based on static patterns + history.
    pub fn suggest_worker(task: &Task) -> Result<AffinityScore> {
        let static_match = patterns::match_static(
            &task.title,
            task.description.as_deref().unwrap_or(""),
        );

        let task_db = TasksDb::open()?;
        let history = task_db.get_routing_history(200)?;

        let workers_db = WorkersDb::open()?;
        let workers = workers_db.list()?;

        let scores = scorer::score_workers(task, &history, &workers);
        Ok(scorer::select_worker(scores, static_match))
    }

    /// Record a new assignment in routing_history (outcome = NULL / pending).
    pub fn record_assignment(task: &Task, worker_slug: &str) -> Result<()> {
        let entry = RoutingHistoryEntry {
            id:          uuid::Uuid::new_v4().to_string(),
            task_id:     task.id.to_string(),
            task_title:  task.title.clone(),
            task_type:   task.task_type.as_str().to_string(),
            worker_slug: worker_slug.to_string(),
            assigned_at: chrono::Utc::now().to_rfc3339(),
            outcome:     None,
            outcome_at:  None,
            duration_ms: None,
            tokens_used: None,
        };
        let db = TasksDb::open()?;
        db.insert_routing_history(&entry)
    }

    /// Update the pending routing_history row for a task with its final outcome.
    pub fn record_outcome(
        task_id: &str,
        outcome: &str,
        duration_ms: i64,
        tokens: i64,
    ) -> Result<()> {
        let db = TasksDb::open()?;
        db.update_routing_outcome(task_id, outcome, duration_ms, tokens)
    }

    /// Return per-worker, per-task-type affinity stats for the dashboard panel.
    pub fn get_affinity_stats() -> Result<Vec<WorkerAffinityStats>> {
        let db = TasksDb::open()?;
        let raw = db.get_affinity_stats()?;
        let stats = raw
            .into_iter()
            .map(|(worker_slug, task_type, total, successes, failures, avg_dur)| {
                let rate = if successes + failures > 0 {
                    successes as f64 / (successes + failures) as f64
                } else {
                    0.0
                };
                WorkerAffinityStats {
                    worker_slug,
                    task_type,
                    total_tasks:     total,
                    success_count:   successes,
                    failure_count:   failures,
                    success_rate:    rate,
                    avg_duration_ms: avg_dur,
                }
            })
            .collect();
        Ok(stats)
    }
}
