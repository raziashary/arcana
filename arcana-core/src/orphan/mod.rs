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

/// Run orphan reconciliation synchronously. Returns released task IDs or the
/// observed failure; an unavailable task store must not look like an empty scan.
/// Safe to call from async context — this uses synchronous SQLite I/O.
pub fn reconcile() -> crate::errors::Result<Vec<String>> {
    reconcile_inner()
}

fn reconcile_inner() -> crate::errors::Result<Vec<String>> {
    let db = TasksDb::open()?;
    let candidates = read_orphan_candidates(|| db.get_orphan_candidates())?;
    let mut released = Vec::new();

    for task in &candidates {
        if let Some(reason) = check_task(task) {
            let id_str = task.id.to_string();
            match db.release_orphaned_task_if_unchanged(task, &reason) {
                Ok(true) => {}
                Ok(false) => {
                    warn!(
                        "[OrphanDetector] Skipped stale orphan snapshot for task #{}; ownership or claim changed",
                        task.id
                    );
                    continue;
                }
                Err(e) => {
                    warn!("[OrphanDetector] Failed to release task #{}", task.id);
                    return Err(e);
                }
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
    reconcile_stranded_qc_claims(&db)?;

    Ok(released)
}

fn read_orphan_candidates(
    read: impl FnOnce() -> crate::errors::Result<Vec<Task>>,
) -> crate::errors::Result<Vec<Task>> {
    let mut timing = crate::observability::timing::OperationTimer::start("orphan_candidate_scan")
        .with_database("tasks");
    match read() {
        Ok(candidates) => {
            timing = timing.with_result_count(candidates.len());
            timing.succeed();
            Ok(candidates)
        }
        Err(error) => Err(error),
    }
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
fn reconcile_stranded_qc_claims(db: &TasksDb) -> crate::errors::Result<()> {
    let claims = db.list_qc_claims()?;
    if claims.is_empty() {
        return Ok(());
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
            db.release_qc_claim(task_id)?;
            warn!(
                "[OrphanDetector] Released stranded QC claim task #{task_id} \
                 (claim_token='{slug}') — reviewer no longer alive"
            );
        }
    }
    Ok(())
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
        } else if !pid_checker::is_arcana_process(pid) {
            // PID alive but isn't an Arcana process — the OS recycled the
            // PID to something unrelated after our worker died. Treat as
            // orphan so the task isn't stranded behind a ghost claim.
            return Some(format!(
                "PID {} is alive but is not an Arcana process (PID reuse suspected)",
                pid
            ));
        } else {
            return None; // PID alive, name matches → still our worker
        }
    }

    // TTL fallback for tasks without a recorded PID (pre-PID tasks or tasks
    // assigned via direct DB edits / UI drag-drop) and for tasks claimed on a
    // different host where we cannot validate the PID locally. The timestamp
    // reader accepts both current RFC 3339 writes and legacy offset-less
    // SQLite values. Unreadable data remains visible and conservative: failing
    // to judge is not the same thing as deciding a task is healthy.
    let age_hours = match crate::db::timestamps::age_hours(&task.updated_at) {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(
                "[orphan] task {} has an unreadable updated_at ({:?}): {e}. Cannot apply the TTL fallback to it.",
                task.id,
                task.updated_at
            );
            return None;
        }
    };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tasks::TasksDb;

    #[test]
    fn candidate_scan_distinguishes_empty_success_from_database_failure() {
        let (empty, empty_events) = crate::observability::timing::capture_json_events(|| {
            read_orphan_candidates(|| Ok(Vec::new()))
        });
        assert!(empty.expect("empty scan succeeds").is_empty());
        let empty_event = empty_events
            .iter()
            .find(|event| event["fields"]["operation"] == "orphan_candidate_scan")
            .expect("empty scan emits its operation event");
        assert_eq!(empty_event["fields"]["outcome"], "success");
        assert_eq!(empty_event["fields"]["result_count"], 0);
        assert_eq!(empty_event["fields"]["result_count_observed"], true);

        let (failed, failed_events) = crate::observability::timing::capture_json_events(|| {
            read_orphan_candidates(|| {
                Err(crate::errors::ArcanaError::Database(
                    rusqlite::Error::InvalidQuery,
                ))
            })
        });
        assert!(failed.is_err(), "database failure must propagate to caller");
        let failed_event = failed_events
            .iter()
            .find(|event| event["fields"]["operation"] == "orphan_candidate_scan")
            .expect("failed scan emits its operation event");
        assert_eq!(failed_event["fields"]["outcome"], "error");
        assert_eq!(failed_event["fields"]["result_count_observed"], false);
        assert!(!failed_event.to_string().contains("InvalidQuery"));
    }

    #[test]
    fn task_release_database_failure_is_not_reported_as_reconciliation_success() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        db.conn
            .execute_batch("DROP TRIGGER tasks_updated_at;")
            .expect("allow a deliberately stale isolated task row");
        let task = task_aged(&db, STALE_TTL_HOURS + 1);
        db.conn
            .execute_batch(&format!(
                "CREATE TRIGGER fail_orphan_release BEFORE UPDATE ON tasks
                 WHEN OLD.id = {} BEGIN SELECT RAISE(ABORT, 'injected release failure'); END;",
                task.id
            ))
            .expect("install release failure fixture");
        drop(db);

        let error = reconcile().expect_err("failed release must propagate");
        assert!(matches!(error, crate::errors::ArcanaError::Database(_)));
    }

    #[test]
    fn stranded_qc_release_database_failure_is_not_reported_as_success() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "stranded QC release failure".into(),
                description: None,
                priority: "p2".into(),
                task_type: "chore".into(),
                acceptance_criteria: "database release errors remain visible".into(),
                created_by: Some("orphan-test".into()),
            })
            .expect("create isolated task");
        db.conn
            .execute(
                "UPDATE tasks SET under_review_by = ?1 WHERE id = ?2",
                rusqlite::params![format!("qc-rev-task{}-0", task.id), task.id],
            )
            .expect("create an old synthetic reviewer claim");
        db.conn
            .execute_batch(&format!(
                "CREATE TRIGGER fail_qc_release BEFORE UPDATE ON tasks
                 WHEN OLD.id = {} AND NEW.under_review_by IS NULL
                 BEGIN SELECT RAISE(ABORT, 'injected QC release failure'); END;",
                task.id
            ))
            .expect("install QC release failure fixture");
        drop(db);

        let error = reconcile().expect_err("failed QC release must propagate");
        assert!(matches!(error, crate::errors::ArcanaError::Database(_)));
    }

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("arcana-orphan-test-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated test directory");
        dir.join("tasks.db")
    }

    /// Read a task through the production row path and then backdate its
    /// in-memory timestamp to test the orphan decision.
    ///
    /// Inserted rather than updated: `tasks` carries an
    /// `AFTER UPDATE … SET updated_at = datetime('now')` trigger, so any
    /// `UPDATE` that tries to backdate the column is immediately overwritten by
    /// the trigger. That is correct behaviour — `updated_at` means "last
    /// touched" — but it means a stale row can only be constructed by
    /// inserting one.
    fn task_aged(db: &TasksDb, hours: i64) -> crate::db::tasks::Task {
        db.conn
            .execute(
                "INSERT INTO tasks (title, priority, task_type, status, assigned_worker, acceptance_criteria,                  created_by, created_at, updated_at, claimed_by_pid, claim_host)                  VALUES ('ttl probe', 'p2', 'chore', 'assigned', 'worker-backend', '', 'test',                  datetime('now', ?1), datetime('now', ?1), NULL, NULL)",
                rusqlite::params![format!("-{hours} hours")],
            )
            .expect("insert backdated task");
        let id = db.conn.last_insert_rowid();
        db.get_by_id(id).expect("get").expect("task")
    }

    /// Prove an old RFC 3339 timestamp is measured as stale by the same age
    /// calculation used in the orphan decision.
    #[test]
    fn a_stale_task_with_no_pid_is_now_detected_as_an_orphan() {
        let path = temp_db("stale");
        let db = TasksDb::open_at(&path).expect("open");

        let mut task = task_aged(&db, STALE_TTL_HOURS + 6);
        // The production task trigger refreshes updated_at on insert/update.
        // Backdate the queried value after reading so this test isolates the
        // orphan age decision instead of accidentally testing trigger policy.
        task.updated_at = (chrono::Utc::now() - chrono::Duration::hours(STALE_TTL_HOURS + 6))
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        // The SQLite fixture is legacy-shaped, but the production read path
        // may normalize it. Assert the semantic age rather than a storage
        // spelling that no longer crosses the query boundary unchanged.
        assert!(
            crate::db::timestamps::age_hours(&task.updated_at).unwrap() > STALE_TTL_HOURS,
            "fixture must remain past the TTL after the production parser, got {:?}",
            task.updated_at,
        );

        let verdict = check_task(&task);
        assert!(
            verdict.is_some(),
            "a task {}h past the {STALE_TTL_HOURS}h TTL must be an orphan; got None",
            STALE_TTL_HOURS + 6
        );
        assert!(
            verdict.as_deref().unwrap_or_default().contains("stale"),
            "unexpected reason: {verdict:?}"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The other half: a fresh task must not be released. Without this, a
    /// detector that returned `Some` unconditionally would pass the test above.
    #[test]
    fn a_recent_task_is_left_alone() {
        let path = temp_db("fresh");
        let db = TasksDb::open_at(&path).expect("open");

        let task = task_aged(&db, 1);
        assert!(
            check_task(&task).is_none(),
            "a task 1h old is well inside the {STALE_TTL_HOURS}h TTL"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
