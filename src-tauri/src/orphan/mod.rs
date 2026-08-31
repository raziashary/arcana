// PID-based orphan detection (Ruflo Feature 2).
//
// When a worker process dies unexpectedly, its claimed tasks are detected as
// orphaned on the next Supervisor poll cycle and released back to `ready` —
// instead of waiting for the 4-hour TTL. For tasks without a recorded PID,
// the 4-hour TTL fallback still applies.

pub mod pid_checker;

use crate::db::tasks::{Task, TasksDb};
use tracing::warn;

const STALE_TTL_HOURS: i64 = 4;

/// Run orphan reconciliation synchronously. Returns a list of released task IDs.
/// Safe to call from async context — does not block the executor (no async I/O).
/// Errors are logged and swallowed so the caller's control flow is never interrupted.
pub fn reconcile() -> Vec<String> {
    match reconcile_inner() {
        Ok(released) => released,
        Err(e) => {
            warn!("[OrphanDetector] reconcile failed: {e}");
            vec![]
        }
    }
}

fn reconcile_inner() -> crate::errors::Result<Vec<String>> {
    let db = TasksDb::open()?;
    let candidates = db.get_orphan_candidates()?;
    let mut released = Vec::new();

    for task in &candidates {
        if let Some(reason) = check_task(task) {
            let id_str = task.id.to_string();
            if let Err(e) = db.release_orphaned_task(task.id, &reason) {
                warn!("[OrphanDetector] Failed to release task #{}: {e}", task.id);
                continue;
            }
            warn!(
                "[OrphanDetector] Released orphaned task #{} '{}': {}",
                task.id, task.title, reason
            );
            released.push(id_str);
        }
    }

    // Doc 04 Phase 2.4 — stranded QC reviewer sweep. Piggy-backs on the
    // existing reconciler cadence so we don't need a second timer. Released
    // claims aren't reported in the returned Vec (those are task-status
    // releases, not subagent-claim releases — the dispatcher recipe picks
    // them up automatically on its next 30s tick).
    reconcile_stranded_qc_claims(&db);

    Ok(released)
}

/// Doc 04 Phase 2.4 — sweep stranded QC reviewer claims.
///
/// A task is "stranded" when its `under_review_by` field references a
/// subagent that no longer exists in the SubagentManager registry. This
/// happens when:
///   1. The Rust process restarted between claim and reviewer completion.
///   2. The background completion handler tokio task was dropped (very rare).
///   3. The reviewer exceeded its TERMINAL_RETENTION window (60 s) and was
///      pruned from the registry before the next reconciler tick.
///
/// On startup `SubagentManager::list` returns an empty registry, so every
/// claim from the previous process gets cleared on the first reconciler tick.
/// During steady-state the function clears at most a handful of entries per
/// run. Safe to call frequently.
fn reconcile_stranded_qc_claims(db: &TasksDb) {
    let claims = match db.list_qc_claims() {
        Ok(c) => c,
        Err(e) => {
            warn!("[OrphanDetector] list_qc_claims failed: {e}");
            return;
        }
    };
    if claims.is_empty() {
        return;
    }

    // Snapshot live subagent slugs via the global manager. If the manager
    // hasn't been installed yet (very early boot), we treat every claim as
    // stranded — which is correct: nothing from a previous lifecycle can
    // possibly still be running.
    let live_ids: std::collections::HashSet<String> = match crate::subagent::manager::global() {
        Some(mgr) => {
            let summaries = futures::executor::block_on(mgr.list(None));
            summaries.into_iter().map(|s| s.id).collect()
        }
        None => std::collections::HashSet::new(),
    };

    for (task_id, slug) in claims {
        // After spawn_qc_review's `upgrade_qc_claim`, the `slug` value is
        // the real subagent_id ("subagent-qc-dispatcher-..."). For those we
        // do an exact match against the live registry. If the claim still
        // carries the synthetic `qc-rev-task<id>-<ts>` token (because the
        // spawn happened but upgrade_qc_claim raced with this reconciler),
        // we fall back to the timestamp heuristic so a freshly-spawned
        // reviewer isn't accidentally swept.
        let still_claimed = if slug.starts_with("qc-rev-task") {
            is_claim_recent_and_live(&slug, &live_ids)
        } else {
            live_ids.contains(&slug)
        };
        if !still_claimed {
            if let Err(e) = db.release_qc_claim(task_id) {
                warn!("[OrphanDetector] release_qc_claim task #{task_id} ({slug}) failed: {e}");
                continue;
            }
            warn!(
                "[OrphanDetector] Released stranded QC claim task #{task_id} \
                 (claim_token='{slug}') — reviewer no longer alive"
            );
        }
    }
}

/// Best-effort liveness check. The under_review_by token is a synthetic
/// `qc-rev-task<id>-<ts>` value (NOT the actual subagent id) so we can't
/// compare directly. Heuristic: a claim placed in the last 90 seconds is
/// considered "possibly live"; older claims with no matching subagent are
/// stranded.
///
/// This is intentionally conservative — false positives leave a claim
/// hanging for one extra reconciler tick (~60 s), which the dispatcher
/// will then catch on its next pass.
fn is_claim_recent_and_live(
    claim_token: &str,
    _live_ids: &std::collections::HashSet<String>,
) -> bool {
    // Parse the ts suffix off the claim_token.
    let parts: Vec<&str> = claim_token.rsplitn(2, '-').collect();
    let ts_ms: i64 = match parts.first().and_then(|s| s.parse().ok()) {
        Some(n) => n,
        None => return false, // malformed → treat as stranded
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    let age_ms = now_ms.saturating_sub(ts_ms);
    // Anything younger than 90 s might still be mid-flight.
    age_ms < 90_000
}

fn check_task(task: &Task) -> Option<String> {
    if let Some(pid) = task.claimed_by_pid {
        let pid = pid as u32;

        // Multi-machine safety: if the claim was made on a different host,
        // this orphan detector can't reason about the PID — the same PID on
        // a different machine is a totally unrelated process. Skip the
        // liveness check and fall through to the TTL fallback below so we
        // don't incorrectly release another machine's in-flight work.
        let current_host = current_hostname();
        let same_host = task
            .claim_host
            .as_deref()
            .map(|h| h.eq_ignore_ascii_case(&current_host))
            .unwrap_or(true); // missing host metadata → treat as local (back-compat)

        if !same_host {
            // Different machine claimed it; rely on TTL only.
        } else if !pid_checker::is_pid_alive(pid) {
            return Some(format!("host process PID {} exited", pid));
        } else if !pid_checker::is_autodev_process(pid) {
            // PID alive but isn't an AutoDev process — the OS recycled the
            // PID to something unrelated after our worker died. Treat as
            // orphan so the task isn't stranded behind a ghost claim.
            return Some(format!(
                "PID {} is alive but is not an AutoDev process (PID reuse suspected)",
                pid
            ));
        } else {
            return None; // PID alive, name matches → still our worker
        }
    }

    // TTL fallback for tasks without a recorded PID (pre-PID tasks or
    // tasks assigned via direct DB edits / UI drag-drop) AND for tasks
    // claimed on a different host where we can't validate the PID locally.
    let updated = chrono::DateTime::parse_from_rfc3339(&task.updated_at).ok()?;
    let age_hours = chrono::Utc::now()
        .signed_duration_since(updated.with_timezone(&chrono::Utc))
        .num_hours();
    if age_hours > STALE_TTL_HOURS {
        return Some(format!(
            "no local PID validation possible and task is {}h stale (TTL fallback)",
            age_hours
        ));
    }

    None
}

fn current_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}
