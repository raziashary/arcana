// Orphan detection commands — Tauri commands for PID-based orphan detection UI.

use crate::db::tasks::{Task, TaskStatus, TasksDb};
use crate::errors::Result;
use crate::events::{emit_task_changed, TaskChangedPayload};

/// Return all tasks currently stuck in `assigned` or `in_progress` state,
/// with their PID and age. Used by the UI to surface potentially orphaned tasks.
#[tauri::command]
pub fn get_orphaned_tasks() -> Result<Vec<Task>> {
    let db = TasksDb::open()?;
    db.get_orphan_candidates()
}

/// Manually release a stuck task back to `ready`. Human override from the UI
/// for cases where automatic PID detection doesn't apply (remote worker, etc.).
#[tauri::command]
pub fn force_release_task(
    task_id: i64,
    expected_status: String,
    expected_worker: Option<String>,
    expected_pid: Option<i64>,
    expected_host: Option<String>,
    expected_updated_at: String,
    reason: Option<String>,
) -> Result<()> {
    let db = TasksDb::open()?;
    let observed = db
        .get_by_id(task_id)?
        .ok_or_else(|| crate::errors::ArcanaError::Agent(format!("Task #{task_id} not found")))?;
    if observed.status
        != TaskStatus::from_str(&expected_status)
            .map_err(|error| crate::errors::ArcanaError::Agent(error.to_string()))?
        || observed.assigned_worker != expected_worker
        || observed.claimed_by_pid != expected_pid
        || observed.claim_host != expected_host
        || observed.updated_at != expected_updated_at
    {
        return Err(crate::errors::ArcanaError::Agent(format!(
            "Task #{task_id} changed since it was displayed; refresh the task list before releasing it"
        )));
    }
    let note = reason.as_deref().unwrap_or("manual release via UI");
    if !db.release_orphaned_task_if_unchanged(&observed, note)? {
        return Err(crate::errors::ArcanaError::Agent(format!(
            "Task #{task_id} changed while it was being released; refresh the task list"
        )));
    }
    emit_task_changed(TaskChangedPayload {
        task_id: Some(task_id),
        kind: "status".into(),
        status: Some("ready".into()),
        assigned_worker: None,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_release_command_clears_claim_and_updates_board_readers() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let db = TasksDb::open().expect("open isolated task DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "normal manual release".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "release clears metadata and the board observes it".into(),
                created_by: Some("test".into()),
            })
            .unwrap();
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 43001).unwrap();
        let observed = get_orphaned_tasks()
            .unwrap()
            .into_iter()
            .find(|row| row.id == task.id)
            .expect("the production orphan reader sees the claim");
        force_release_task(
            task.id,
            observed.status.as_str().to_string(),
            observed.assigned_worker,
            observed.claimed_by_pid,
            observed.claim_host,
            observed.updated_at,
            Some("user confirmed obsolete claim".into()),
        )
        .expect("an unchanged observed claim may be released");
        let released = crate::commands::task_commands::get_task(task.id)
            .unwrap()
            .expect("production board reader returns released task");
        assert_eq!(released.status, TaskStatus::Ready);
        assert_eq!(released.assigned_worker, None);
        assert_eq!(released.claimed_by_pid, None);
        assert_eq!(released.claim_host, None);
        assert_eq!(
            released.rejection_reason.as_deref(),
            Some("[orphan] user confirmed obsolete claim")
        );
        assert!(!get_orphaned_tasks()
            .unwrap()
            .iter()
            .any(|row| row.id == task.id));
    }

    #[test]
    fn manual_release_command_cas_preserves_claim_replaced_after_preflight() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        use tracing_subscriber::{layer::SubscriberExt, Layer};

        struct ReplaceClaimAtRelease {
            path: std::path::PathBuf,
            observed: Task,
            fired: Arc<AtomicBool>,
        }

        impl<S: tracing::Subscriber> Layer<S> for ReplaceClaimAtRelease {
            fn on_new_span(
                &self,
                attributes: &tracing::span::Attributes<'_>,
                _id: &tracing::span::Id,
                _context: tracing_subscriber::layer::Context<'_, S>,
            ) {
                if attributes.metadata().name() != "database_task_orphan_release"
                    || self.fired.swap(true, Ordering::SeqCst)
                {
                    return;
                }
                let path = self.path.clone();
                let observed = self.observed.clone();
                std::thread::spawn(move || {
                    let writer = TasksDb::open_at(&path).expect("open competing connection");
                    assert!(writer
                        .release_orphaned_task_if_unchanged(&observed, "race replacement")
                        .unwrap());
                    writer
                        .update_status(observed.id, "assigned", Some("worker-b"))
                        .unwrap();
                    writer
                        .claim_task_with_pid(observed.id, "worker-b", 42002)
                        .unwrap();
                })
                .join()
                .expect("competing claim commits before the delayed CAS");
            }
        }

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let db = TasksDb::open().expect("open isolated TasksDb");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "late manual release race".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a replacement after preflight survives stale release".into(),
                created_by: Some("test".into()),
            })
            .unwrap();
        db.update_status(task.id, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task.id, "worker-a", 42001).unwrap();
        let observed = db.get_by_id(task.id).unwrap().unwrap();
        let fired = Arc::new(AtomicBool::new(false));
        let subscriber = tracing_subscriber::registry().with(ReplaceClaimAtRelease {
            path: home.path().join(".arcana").join("tasks.db"),
            observed: observed.clone(),
            fired: fired.clone(),
        });
        let error = tracing::subscriber::with_default(subscriber, || {
            force_release_task(
                task.id,
                observed.status.as_str().to_string(),
                observed.assigned_worker,
                observed.claimed_by_pid,
                observed.claim_host.clone(),
                observed.updated_at,
                Some("delayed UI action".into()),
            )
        })
        .expect_err("the post-preflight compare-and-set must lose the race");
        assert!(
            fired.load(Ordering::SeqCst),
            "preflight must reach the real release boundary"
        );
        assert!(error
            .to_string()
            .contains("changed while it was being released"));
        let current = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(current.status, TaskStatus::InProgress);
        assert_eq!(current.assigned_worker.as_deref(), Some("worker-b"));
        assert_eq!(current.claimed_by_pid, Some(42002));
        assert_eq!(current.claim_host, observed.claim_host);
    }

    #[test]
    fn manual_release_command_rejects_a_stale_claim_snapshot() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let db = TasksDb::open().expect("open isolated task DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "manual release CAS".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a stale UI snapshot cannot release a newer claim".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .expect("assign first worker");
        db.claim_task_with_pid(task.id, "worker-a", 41001)
            .expect("first worker claims task");
        let observed = db
            .get_by_id(task.id)
            .unwrap()
            .expect("first claim snapshot");

        assert!(db
            .release_orphaned_task_if_unchanged(&observed, "replace claim")
            .expect("release first claim"));
        db.update_status(task.id, "assigned", Some("worker-b"))
            .expect("assign replacement worker");
        db.claim_task_with_pid(task.id, "worker-b", 41002)
            .expect("replacement worker claims task");

        let error = force_release_task(
            task.id,
            observed.status.as_str().to_string(),
            observed.assigned_worker.clone(),
            observed.claimed_by_pid,
            observed.claim_host.clone(),
            observed.updated_at.clone(),
            Some("stale UI action".into()),
        )
        .expect_err("the stale manual-release command must lose its compare-and-set");
        assert!(error.to_string().contains("changed since it was displayed"));

        let current = db.get_by_id(task.id).unwrap().expect("replacement claim");
        assert_eq!(current.status, TaskStatus::InProgress);
        assert_eq!(current.assigned_worker.as_deref(), Some("worker-b"));
        assert_eq!(current.claimed_by_pid, Some(41002));
    }
}
