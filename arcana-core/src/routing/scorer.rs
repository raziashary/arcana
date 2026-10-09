// Affinity scorer — combines historical success rates + task duration into a
// single score per worker. No vector embeddings; pure arithmetic over routing_history rows.

use crate::db::tasks::Task;
use crate::db::{tasks::RoutingHistoryEntry, workers::Worker};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityScore {
    pub worker_slug: String,
    pub score: f32,
    pub confidence: f32,
    pub method: String, // "static" | "historical" | "default"
}

/// Score every active worker for the given task using routing history.
/// Workers with ≥3 relevant historical entries get a computed score;
/// others receive a neutral 0.5 ("not enough data").
pub fn score_workers(
    task: &Task,
    history: &[RoutingHistoryEntry],
    workers: &[Worker],
) -> Vec<AffinityScore> {
    let task_type_str = task.task_type.as_str();
    let eligible_worker_slugs: std::collections::HashSet<&str> = workers
        .iter()
        .filter(|worker| !worker.is_system_defined)
        .map(|worker| worker.slug.as_str())
        .collect();

    // Gather per-worker stats for the matching task_type
    struct WorkerStats {
        successes: u32,
        failures: u32,
        total_dur_ms: i64,
        dur_samples: u32,
    }
    let mut stats: std::collections::HashMap<String, WorkerStats> =
        std::collections::HashMap::new();

    for entry in history {
        if entry.task_type != task_type_str
            || !eligible_worker_slugs.contains(entry.worker_slug.as_str())
        {
            continue;
        }
        let s = stats
            .entry(entry.worker_slug.clone())
            .or_insert(WorkerStats {
                successes: 0,
                failures: 0,
                total_dur_ms: 0,
                dur_samples: 0,
            });
        match entry.outcome.as_deref() {
            Some("success") => s.successes += 1,
            Some("failure") => s.failures += 1,
            _ => {}
        }
        if let Some(dur) = entry.duration_ms {
            if dur > 0 {
                s.total_dur_ms += dur;
                s.dur_samples += 1;
            }
        }
    }

    // Compute avg durations for normalization
    let avg_durations: Vec<(String, f64)> = stats
        .iter()
        .filter(|(_, s)| s.successes + s.failures >= 3 && s.dur_samples > 0)
        .map(|(slug, s)| (slug.clone(), s.total_dur_ms as f64 / s.dur_samples as f64))
        .collect();

    let min_dur = avg_durations
        .iter()
        .map(|(_, d)| *d)
        .fold(f64::INFINITY, f64::min);
    let max_dur = avg_durations.iter().map(|(_, d)| *d).fold(0f64, f64::max);
    let dur_range = (max_dur - min_dur).max(1.0);

    let mut scores: Vec<AffinityScore> = workers
        .iter()
        // Orchestration and QC agents are not implementation candidates,
        // regardless of whether their routing history is strong.
        .filter(|worker| !worker.is_system_defined)
        .map(|w| {
            let slug = &w.slug;
            match stats.get(slug) {
                Some(s) if s.successes + s.failures >= 3 => {
                    let total = (s.successes + s.failures) as f32;
                    let success_rate = s.successes as f32 / total;

                    let duration_score = if s.dur_samples > 0 {
                        let avg = s.total_dur_ms as f64 / s.dur_samples as f64;
                        // Fastest worker → 1.0, slowest → 0.0
                        1.0 - ((avg - min_dur) / dur_range) as f32
                    } else {
                        0.5
                    };

                    let score = success_rate * 0.6 + duration_score * 0.4;
                    let confidence = (total / (total + 5.0)).min(1.0);
                    AffinityScore {
                        worker_slug: slug.clone(),
                        score: score.clamp(0.0, 1.0),
                        confidence,
                        method: "historical".into(),
                    }
                }
                _ => AffinityScore {
                    worker_slug: slug.clone(),
                    score: 0.5,
                    confidence: 0.0,
                    method: "default".into(),
                },
            }
        })
        .collect();

    scores.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.worker_slug.cmp(&b.worker_slug))
    });
    scores
}

/// Select the final worker recommendation, blending static pattern confidence
/// with historical scores.
pub fn select_worker(
    mut scores: Vec<AffinityScore>,
    static_match: Option<(&'static str, f32)>,
) -> Option<AffinityScore> {
    if scores.is_empty() {
        return None;
    }

    if let Some((static_slug, static_conf)) = static_match {
        if static_conf >= 0.85 {
            let eligible_score = scores
                .iter()
                .find(|s| s.worker_slug == static_slug)
                .map(|s| s.score);

            if let Some(hist_score) = eligible_score.filter(|score| *score >= 0.3) {
                return Some(AffinityScore {
                    worker_slug: static_slug.into(),
                    score: ((static_conf + hist_score) / 2.0).clamp(0.0, 1.0),
                    confidence: static_conf,
                    method: "static".into(),
                });
            }
            // Historical score < 0.3: static match is bad performer; fall through to historical
        }
    }

    Some(scores.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tasks::{TaskPriority, TaskStatus, TaskType};
    use crate::db::workers::{WorkerStatus, WorkersDb};

    fn task() -> Task {
        Task {
            id: 1,
            task_identity: uuid::Uuid::new_v4().to_string(),
            title: "Implement API endpoint".into(),
            description: None,
            priority: TaskPriority::P2,
            task_type: TaskType::Feature,
            status: TaskStatus::Ready,
            acceptance_criteria: String::new(),
            assigned_worker: None,
            rejection_reason: None,
            retry_count: 0,
            created_by: "test".into(),
            created_at: String::new(),
            updated_at: String::new(),
            completed_at: None,
            claimed_by_pid: None,
            claim_host: None,
            diff_analysis: None,
            budget_max_tokens: None,
            budget_max_usd: None,
            budget_max_turns: None,
            budget_tokens_used: 0,
            budget_cost_usd: 0.0,
            budget_turns_used: 0,
            budget_exceeded: false,
            budget_exceeded_reason: None,
            phase: None,
            phase_order: None,
            milestone: None,
            epic_id: None,
            blocked_by: None,
            under_review_by: None,
            task_qc_cost_usd: 0.0,
        }
    }

    fn worker(slug: &str, is_system_defined: bool) -> Worker {
        Worker {
            id: 1,
            name: slug.into(),
            slug: slug.into(),
            worker_type: "custom".into(),
            status: WorkerStatus::Idle,
            model: "test-model".into(),
            primary_repos: Vec::new(),
            polling_interval: 60,
            recipe_id: None,
            is_system_defined,
            description: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_seen: None,
        }
    }

    #[test]
    fn automatic_dispatch_uses_outcomes_and_only_runnable_workers() {
        let task = Task {
            title: "Implement an unfamiliar feature".into(),
            ..task()
        };
        let workers = vec![
            worker("worker-backend", false),
            worker("worker-frontend", false),
            worker("worker-paused", false),
            worker("worker-working", false),
            worker("supervisor", true),
        ];
        let mut workers = workers;
        workers[2].status = WorkerStatus::Paused;
        workers[3].status = WorkerStatus::Working;
        let mut history = Vec::new();
        for (slug, outcome) in [
            ("worker-backend", "failure"),
            ("worker-backend", "failure"),
            ("worker-backend", "failure"),
            ("worker-frontend", "success"),
            ("worker-frontend", "success"),
            ("worker-frontend", "success"),
            ("worker-paused", "success"),
            ("worker-working", "success"),
            ("supervisor", "success"),
        ] {
            history.push(RoutingHistoryEntry {
                id: uuid::Uuid::new_v4().to_string(),
                task_id: "prior".into(),
                task_title: task.title.clone(),
                task_type: task.task_type.as_str().into(),
                worker_slug: slug.into(),
                assigned_at: String::new(),
                outcome: Some(outcome.into()),
                outcome_at: None,
                duration_ms: Some(100),
                tokens_used: None,
            });
        }

        let selected =
            crate::routing::TaskRouter::select_runnable_worker(&task, &history, &workers)
                .expect("two runnable implementation workers are present");
        assert_eq!(selected.worker_slug, "worker-frontend");
        assert_eq!(selected.method, "historical");
        // `TaskRouter` filters the roster before calling the low-level scorer;
        // mirror that production boundary when asserting candidate scores.
        let runnable = workers
            .iter()
            .filter(|worker| crate::routing::TaskRouter::is_runnable_implementation_worker(worker))
            .cloned()
            .collect::<Vec<_>>();
        let available = score_workers(&task, &history, &runnable);
        assert_eq!(
            available
                .iter()
                .map(|score| score.worker_slug.as_str())
                .collect::<std::collections::HashSet<_>>(),
            ["worker-backend", "worker-frontend"].into_iter().collect()
        );
    }

    #[test]
    fn seeded_system_agents_are_not_routing_candidates_even_with_strong_history() {
        let task = task();
        let workers_db = WorkersDb::open_seeded_in_memory_for_test()
            .expect("seed default worker roster in memory");
        let workers = workers_db.list().expect("read seeded worker roster");
        assert!(workers.iter().any(|worker| worker.slug == "supervisor"));
        assert!(workers.iter().any(|worker| worker.slug == "qc-agent"));
        let mut history = Vec::new();
        for worker in &workers {
            for _ in 0..3 {
                history.push(RoutingHistoryEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: "1".into(),
                    task_title: task.title.clone(),
                    task_type: task.task_type.as_str().into(),
                    worker_slug: worker.slug.clone(),
                    assigned_at: String::new(),
                    outcome: Some("success".into()),
                    outcome_at: None,
                    duration_ms: Some(1),
                    tokens_used: None,
                });
            }
        }

        let scores = score_workers(&task, &history, &workers);
        assert!(!scores.is_empty());
        assert!(scores.iter().all(|score| !workers
            .iter()
            .any(|worker| worker.slug == score.worker_slug && worker.is_system_defined)));
        let selected = select_worker(scores, Some(("supervisor", 1.0)))
            .expect("seeded default team has implementation workers");
        assert!(workers
            .iter()
            .any(|worker| { worker.slug == selected.worker_slug && !worker.is_system_defined }));
    }

    #[test]
    fn no_implementation_workers_produces_no_suggestion() {
        let scores = score_workers(
            &task(),
            &[],
            &[worker("supervisor", true), worker("qc-agent", true)],
        );

        assert!(select_worker(scores, None).is_none());
    }

    #[test]
    fn system_agent_history_does_not_change_implementation_ranking() {
        let task = task();
        let workers = [
            worker("supervisor", true),
            worker("qc-agent", true),
            worker("worker-backend", false),
            worker("worker-frontend", false),
        ];
        let history_for = |slug: &str, outcomes: &[&str], duration_ms| {
            outcomes
                .iter()
                .map(|outcome| RoutingHistoryEntry {
                    id: uuid::Uuid::new_v4().to_string(),
                    task_id: "1".into(),
                    task_title: task.title.clone(),
                    task_type: task.task_type.as_str().into(),
                    worker_slug: slug.into(),
                    assigned_at: String::new(),
                    outcome: Some((*outcome).into()),
                    outcome_at: None,
                    duration_ms: Some(duration_ms),
                    tokens_used: None,
                })
                .collect::<Vec<_>>()
        };
        let mut implementation_history =
            history_for("worker-backend", &["success", "success", "success"], 100);
        implementation_history.extend(history_for(
            "worker-frontend",
            &["success", "success", "failure"],
            10,
        ));

        let implementation_only = select_worker(
            score_workers(&task, &implementation_history, &workers),
            None,
        )
        .unwrap();
        let mut all_history = implementation_history.clone();
        all_history.extend(history_for(
            "supervisor",
            &["success", "success", "success"],
            10_000,
        ));
        all_history.extend(history_for(
            "qc-agent",
            &["failure", "failure", "failure"],
            1,
        ));

        let with_system_history =
            select_worker(score_workers(&task, &all_history, &workers), None).unwrap();
        assert_eq!(implementation_only.worker_slug, "worker-frontend");
        assert_eq!(
            with_system_history.worker_slug,
            implementation_only.worker_slug
        );
    }

    #[test]
    fn shipped_static_pattern_is_consumed_at_its_configured_threshold() {
        let task = task();
        let static_match = crate::routing::patterns::match_static(&task.title, "");
        assert_eq!(static_match, Some(("worker-backend", 0.85)));

        let workers = WorkersDb::open_seeded_in_memory_for_test()
            .expect("seed default worker roster in memory")
            .list()
            .expect("read seeded worker roster");
        let selected = select_worker(score_workers(&task, &[], &workers), static_match)
            .expect("seeded roster has eligible implementation workers");

        assert_eq!(selected.worker_slug, "worker-backend");
        assert_eq!(selected.method, "static");
    }
}
