// Semantic Task Routing — stateless router that combines static keyword patterns
// with historical worker affinity to suggest the best worker for a task.
//
// Ruflo reference: hooks_route concept from v3/@claude-flow/cli/src/mcp-tools/hooks-tools.ts
// No submodule changes — all data is in tasks.db (routing_history table).

pub mod patterns;
pub mod scorer;

use crate::db::tasks::{RoutingHistoryEntry, Task, TasksDb};
use crate::db::workers::{Worker, WorkerStatus, WorkersDb};
use crate::errors::Result;
pub use scorer::AffinityScore;
use serde::{Deserialize, Serialize};

// ─── Worker Affinity Stats (for UI + Tauri command) ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAffinityStats {
    pub worker_slug: String,
    pub task_type: String,
    pub total_tasks: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub success_rate: f64,
    pub avg_duration_ms: Option<i64>,
}

// ─── TaskRouter — stateless; opens DB per call to match project conventions ──

pub struct TaskRouter;

impl TaskRouter {
    /// Suggest the best worker for a task based on static patterns + history.
    pub fn suggest_worker(task: &Task) -> Result<AffinityScore> {
        let task_db = TasksDb::open()?;
        let history = task_db.get_routing_history(200)?;
        let workers_db = WorkersDb::open()?;
        let workers = workers_db.list()?;
        Self::select_runnable_worker(task, &history, &workers).ok_or_else(|| {
            anyhow::anyhow!("No non-system implementation workers are available for routing").into()
        })
    }

    pub fn select_worker_for_dispatch(task: &Task) -> Result<AffinityScore> {
        let task_db = TasksDb::open()?;
        let history = task_db.get_routing_history(200)?;
        let workers = WorkersDb::open()?.list()?;
        Self::select_runnable_worker(task, &history, &workers).ok_or_else(|| {
            anyhow::anyhow!("No started, runnable implementation workers are available").into()
        })
    }

    /// Choose among started, runnable implementation workers using only
    /// deterministic task signals and automatic assignment history.
    pub fn select_runnable_worker(
        task: &Task,
        history: &[RoutingHistoryEntry],
        workers: &[Worker],
    ) -> Option<AffinityScore> {
        let runnable = workers
            .iter()
            .filter(|worker| Self::is_runnable_implementation_worker(worker))
            .cloned()
            .collect::<Vec<_>>();
        let static_match =
            patterns::match_static(&task.title, task.description.as_deref().unwrap_or(""));
        let scores = scorer::score_workers(task, history, &runnable);
        scorer::select_worker(scores, static_match)
    }

    pub fn is_runnable_implementation_worker(worker: &Worker) -> bool {
        !worker.is_system_defined
            && matches!(
                worker.status,
                WorkerStatus::Idle
                    | WorkerStatus::Active
                    | WorkerStatus::IdleNoProvider
                    | WorkerStatus::Error
            )
    }

    /// Record a new assignment in routing_history (outcome = NULL / pending).
    pub fn record_assignment(task: &Task, worker_slug: &str) -> Result<()> {
        let entry = Self::assignment_entry(task, worker_slug);
        let db = TasksDb::open()?;
        db.insert_routing_history(&entry)
    }

    /// Build the history row used by the atomic task assignment transaction.
    pub fn assignment_entry(task: &Task, worker_slug: &str) -> RoutingHistoryEntry {
        RoutingHistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            task_id: task.id.to_string(),
            task_title: task.title.clone(),
            task_type: task.task_type.as_str().to_string(),
            worker_slug: worker_slug.to_string(),
            assigned_at: crate::db::timestamps::now_rfc3339(),
            outcome: None,
            outcome_at: None,
            duration_ms: None,
            tokens_used: None,
        }
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

    /// Record an outcome against the newest pending assignment for the worker
    /// who owned the task before its status changed. Manual assignments with
    /// no routing-history row remain valid and simply have nothing to learn.
    pub fn record_outcome_auto_for_worker(
        task_id: &str,
        worker_slug: &str,
        outcome: &str,
    ) -> Result<bool> {
        let db = TasksDb::open()?;
        db.update_latest_routing_outcome_for_worker(task_id, worker_slug, outcome)
    }

    /// Return per-worker, per-task-type affinity stats for the dashboard panel.
    pub fn get_affinity_stats() -> Result<Vec<WorkerAffinityStats>> {
        let db = TasksDb::open()?;
        let raw = db.get_affinity_stats()?;
        let stats = raw
            .into_iter()
            .map(
                |(worker_slug, task_type, total, successes, failures, avg_dur)| {
                    let rate = if successes + failures > 0 {
                        successes as f64 / (successes + failures) as f64
                    } else {
                        0.0
                    };
                    WorkerAffinityStats {
                        worker_slug,
                        task_type,
                        total_tasks: total,
                        success_count: successes,
                        failure_count: failures,
                        success_rate: rate,
                        avg_duration_ms: avg_dur,
                    }
                },
            )
            .collect();
        Ok(stats)
    }
}
