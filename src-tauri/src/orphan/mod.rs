// PID-based orphan detection (Ruflo Feature 2).
//
// When a worker process dies unexpectedly, its claimed tasks are detected as
// orphaned on the next Supervisor poll cycle and released back to `ready` —
// instead of waiting for the 4-hour TTL. For tasks without a recorded PID,
// the 4-hour TTL fallback still applies.

pub mod pid_checker;

use tracing::warn;
use crate::db::tasks::{Task, TasksDb};

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

    Ok(released)
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
        let same_host = task.claim_host
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
