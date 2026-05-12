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
        if !pid_checker::is_pid_alive(pid as u32) {
            return Some(format!("host process PID {} exited", pid));
        }
        return None; // PID alive — not orphaned
    }

    // TTL fallback for tasks without a recorded PID (pre-PID tasks or
    // tasks assigned via direct DB edits / UI drag-drop)
    let updated = chrono::DateTime::parse_from_rfc3339(&task.updated_at).ok()?;
    let age_hours = chrono::Utc::now()
        .signed_duration_since(updated.with_timezone(&chrono::Utc))
        .num_hours();
    if age_hours > STALE_TTL_HOURS {
        return Some(format!(
            "no PID recorded and task is {}h stale (TTL fallback)",
            age_hours
        ));
    }

    None
}
