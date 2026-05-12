// Affinity scorer — combines historical success rates + task duration into a
// single score per worker. No vector embeddings; pure arithmetic over routing_history rows.

use serde::{Deserialize, Serialize};
use crate::db::{tasks::RoutingHistoryEntry, workers::Worker};
use crate::db::tasks::Task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityScore {
    pub worker_slug: String,
    pub score:       f32,
    pub confidence:  f32,
    pub method:      String,  // "static" | "historical" | "default"
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

    // Gather per-worker stats for the matching task_type
    struct WorkerStats {
        successes:    u32,
        failures:     u32,
        total_dur_ms: i64,
        dur_samples:  u32,
    }
    let mut stats: std::collections::HashMap<String, WorkerStats> =
        std::collections::HashMap::new();

    for entry in history {
        if entry.task_type != task_type_str { continue; }
        let s = stats.entry(entry.worker_slug.clone()).or_insert(WorkerStats {
            successes: 0, failures: 0, total_dur_ms: 0, dur_samples: 0,
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

    let min_dur = avg_durations.iter().map(|(_, d)| *d).fold(f64::INFINITY, f64::min);
    let max_dur = avg_durations.iter().map(|(_, d)| *d).fold(0f64, f64::max);
    let dur_range = (max_dur - min_dur).max(1.0);

    let mut scores: Vec<AffinityScore> = workers
        .iter()
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
                        score:       score.clamp(0.0, 1.0),
                        confidence,
                        method:      "historical".into(),
                    }
                }
                _ => AffinityScore {
                    worker_slug: slug.clone(),
                    score:       0.5,
                    confidence:  0.0,
                    method:      "default".into(),
                },
            }
        })
        .collect();

    scores.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scores
}

/// Select the final worker recommendation, blending static pattern confidence
/// with historical scores.
pub fn select_worker(
    mut scores: Vec<AffinityScore>,
    static_match: Option<(&'static str, f32)>,
) -> AffinityScore {
    if scores.is_empty() {
        return AffinityScore {
            worker_slug: "worker-backend".into(),
            score:       0.5,
            confidence:  0.0,
            method:      "default".into(),
        };
    }

    if let Some((static_slug, static_conf)) = static_match {
        if static_conf > 0.85 {
            let hist_score = scores
                .iter()
                .find(|s| s.worker_slug == static_slug)
                .map(|s| s.score)
                .unwrap_or(0.5);

            if hist_score >= 0.3 {
                return AffinityScore {
                    worker_slug: static_slug.into(),
                    score:       ((static_conf + hist_score) / 2.0).clamp(0.0, 1.0),
                    confidence:  static_conf,
                    method:      "static".into(),
                };
            }
            // Historical score < 0.3: static match is bad performer; fall through to historical
        }
    }

    scores.remove(0)
}
