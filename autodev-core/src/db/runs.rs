// `~/.autodev/runs.db` — the run/audit ledger.
//
// One row per run: an application launch, a CLI invocation, a fleet tick, an
// agent turn. Its purpose is to give every later act something durable to
// point at — "this task was claimed, this file was written, this cost was
// incurred, *during run X*" — so that a question asked afterwards has an
// answer that survives the process.
//
// It is built in P1 rather than when the first consumer needs it, for the
// reason the plan gives: a ledger added later cannot describe anything that
// happened before it existed, and retrofitting a `run_id` column onto tables
// that already hold history means those rows are permanently unattributable.
//
// ⚠ **Honest scope.** What is proven here is that runs are recorded and can be
// read back. Later acts do **not** yet carry a `run_id` — no other table has
// that column. Stamping them is the work of the phases that own those tables,
// and this module existing is not evidence that anything is attributed. That
// distinction is exactly the kind this programme exists to keep visible.
//
// See MDP `§5` (sequencing rationale) and `§7 P1-18`.

use rusqlite::{params, OptionalExtension};

use crate::db::migrate::{self, Migration};
use crate::db::timestamps::now_rfc3339;
use crate::errors::{AutoDevError, Result};

/// What kind of run a row describes.
///
/// A plain string in the database rather than a closed enum: a later phase adds
/// kinds without a migration, and an unknown kind read back is data, not a
/// parse failure.
pub mod kind {
    /// The desktop application, from launch to shutdown.
    pub const APP: &str = "app";
    /// One `airsctl` invocation.
    pub const CLI: &str = "cli";
    /// One agent turn.
    pub const AGENT_TURN: &str = "agent-turn";
    /// One fleet scheduler tick.
    pub const FLEET_TICK: &str = "fleet-tick";
}

/// How a run ended.
pub mod outcome {
    pub const OK: &str = "ok";
    pub const FAILED: &str = "failed";
    /// Recorded when a run is found still open at the next start — the process
    /// died without closing it.
    pub const ABANDONED: &str = "abandoned";
}

/// One recorded run.
#[derive(Debug, Clone)]
pub struct Run {
    pub id: String,
    pub kind: String,
    pub actor: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    /// `None` while the run is still open.
    pub outcome: Option<String>,
    pub host: String,
    pub pid: i64,
    pub parent_run_id: Option<String>,
    /// Free-form JSON, for whatever the caller needs to record.
    pub detail: Option<String>,
}

impl Run {
    /// Whether this run is still open.
    pub fn is_open(&self) -> bool {
        self.outcome.is_none()
    }
}

pub struct RunsDb {
    pub(crate) conn: rusqlite::Connection,
}

const LADDER: &[Migration] = &[Migration {
    version: 1,
    name: "runs-baseline",
    sql: r#"
        CREATE TABLE IF NOT EXISTS runs (
            id            TEXT PRIMARY KEY,
            kind          TEXT NOT NULL,
            actor         TEXT,
            started_at    TEXT NOT NULL,
            ended_at      TEXT,
            outcome       TEXT,
            host          TEXT NOT NULL,
            pid           INTEGER NOT NULL,
            parent_run_id TEXT REFERENCES runs(id),
            detail        TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_runs_started ON runs(started_at);
        CREATE INDEX IF NOT EXISTS idx_runs_kind    ON runs(kind, started_at);
        CREATE INDEX IF NOT EXISTS idx_runs_open    ON runs(outcome) WHERE outcome IS NULL;
    "#,
}];

impl RunsDb {
    pub fn open() -> Result<Self> {
        Self::open_at(&crate::db::autodev_dir()?.join("runs.db"))
    }

    /// As `open`, against an explicit path, so tests never write to the user's
    /// real ledger.
    pub fn open_at(path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(path)?;
        let db = Self { conn };
        migrate::run_ladder(&db.conn, "runs.db", LADDER)?;
        Ok(db)
    }

    /// This database's schema version.
    pub fn schema_version(&self) -> Result<i32> {
        migrate::user_version(&self.conn)
    }

    /// Open a run and return its id.
    pub fn begin(&self, kind: &str, actor: Option<&str>) -> Result<String> {
        self.begin_with(kind, actor, None, None)
    }

    /// Open a run, optionally nested inside another and carrying detail.
    pub fn begin_with(
        &self,
        kind: &str,
        actor: Option<&str>,
        parent_run_id: Option<&str>,
        detail: Option<&str>,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        self.conn
            .execute(
                "INSERT INTO runs (id, kind, actor, started_at, host, pid, parent_run_id, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &id,
                    kind,
                    actor,
                    now_rfc3339(),
                    hostname(),
                    std::process::id() as i64,
                    parent_run_id,
                    detail
                ],
            )
            .map_err(|e| AutoDevError::Agent(format!("opening a run: {e}")))?;
        Ok(id)
    }

    /// Close a run.
    ///
    /// Closing a run that does not exist, or one already closed, is an error
    /// rather than a silent no-op: both mean the caller's idea of the run
    /// differs from the ledger's, and a ledger that quietly accepts that is not
    /// an audit trail.
    pub fn finish(&self, run_id: &str, outcome: &str) -> Result<()> {
        let rows = self
            .conn
            .execute(
                "UPDATE runs SET ended_at = ?1, outcome = ?2 \
                 WHERE id = ?3 AND outcome IS NULL",
                params![now_rfc3339(), outcome, run_id],
            )
            .map_err(|e| AutoDevError::Agent(format!("closing run {run_id}: {e}")))?;
        if rows == 0 {
            return Err(AutoDevError::Agent(format!(
                "no open run {run_id} to close — it does not exist, or it was already closed"
            )));
        }
        Ok(())
    }

    pub fn get(&self, run_id: &str) -> Result<Option<Run>> {
        self.conn
            .query_row(
                "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid, \
                        parent_run_id, detail \
                 FROM runs WHERE id = ?1",
                params![run_id],
                row_to_run,
            )
            .optional()
            .map_err(|e| AutoDevError::Agent(format!("reading run {run_id}: {e}")))
    }

    /// The most recent runs, newest first.
    pub fn recent(&self, limit: i64) -> Result<Vec<Run>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid, \
                        parent_run_id, detail \
                 FROM runs ORDER BY started_at DESC, rowid DESC LIMIT ?1",
            )
            .map_err(|e| AutoDevError::Agent(format!("recent runs prepare: {e}")))?;
        let rows = stmt
            .query_map(params![limit], row_to_run)
            .map_err(|e| AutoDevError::Agent(format!("recent runs query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("recent runs collect: {e}")))
    }

    /// Runs still open — no `outcome`.
    pub fn open_runs(&self) -> Result<Vec<Run>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid, \
                        parent_run_id, detail \
                 FROM runs WHERE outcome IS NULL ORDER BY started_at ASC",
            )
            .map_err(|e| AutoDevError::Agent(format!("open runs prepare: {e}")))?;
        let rows = stmt
            .query_map([], row_to_run)
            .map_err(|e| AutoDevError::Agent(format!("open runs query: {e}")))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("open runs collect: {e}")))
    }

    /// Close runs left open by a process that is no longer alive.
    ///
    /// A crash leaves a run open forever, and an open run reads as "still
    /// running" to anything querying the ledger.
    ///
    /// ⚠ **The first version tested `NOT (pid = ? AND host = ?)`** — "not my
    /// pid" treated as "dead". Running `airsctl` while the desktop app was open
    /// therefore marked the **live app's run** abandoned, every time. Worse,
    /// `orphan::pid_checker::is_pid_alive` was sitting unused in the same
    /// crate: the instrument was already there. Found by the post-phase audit
    /// (A-09 / W-08).
    ///
    /// Now: a run is reaped only when it is on **this host** and its pid is
    /// **not alive**. Runs from another host are left alone — this process
    /// cannot see that machine's process table, and guessing is what caused the
    /// bug.
    ///
    /// Returns how many were reaped.
    pub fn reap_abandoned(&self) -> Result<usize> {
        let this_host = hostname();
        let mut reaped = 0usize;

        for run in self.open_runs()? {
            if run.host != this_host {
                continue; // another machine's process table is not ours to read
            }
            let pid = match u32::try_from(run.pid) {
                Ok(p) => p,
                Err(_) => continue,
            };
            if crate::orphan::pid_checker::is_pid_alive(pid) {
                continue; // genuinely still running — this includes our own run
            }
            self.conn
                .execute(
                    "UPDATE runs SET ended_at = ?1, outcome = ?2 \
                     WHERE id = ?3 AND outcome IS NULL",
                    params![now_rfc3339(), outcome::ABANDONED, &run.id],
                )
                .map_err(|e| AutoDevError::Agent(format!("reaping run {}: {e}", run.id)))?;
            reaped += 1;
        }

        if reaped > 0 {
            tracing::info!("[runs] closed {reaped} run(s) left open by a dead process");
        }
        Ok(reaped)
    }
}

fn row_to_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<Run> {
    Ok(Run {
        id: row.get(0)?,
        kind: row.get(1)?,
        actor: row.get(2)?,
        started_at: row.get(3)?,
        ended_at: row.get(4)?,
        outcome: row.get(5)?,
        host: row.get(6)?,
        pid: row.get(7)?,
        parent_run_id: row.get(8)?,
        detail: row.get(9)?,
    })
}

/// Best-effort host identity, for telling apart runs from different machines.
///
/// Falls back to `"unknown"` rather than failing: a ledger entry with an
/// unknown host is far better than no ledger entry.
fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("autodev-runs-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir.join("runs.db")
    }

    #[test]
    fn a_run_is_recorded_and_read_back() {
        let path = temp_db("roundtrip");
        let db = RunsDb::open_at(&path).expect("open");
        assert_eq!(db.schema_version().unwrap(), 1);

        let id = db.begin(kind::APP, None).expect("begin");

        // Prove the reader reads it: a ledger nothing can query is not a ledger.
        let run = db.get(&id).expect("get").expect("the run must be there");
        assert_eq!(run.kind, kind::APP);
        assert!(run.is_open(), "a run is open until it is finished");
        assert_eq!(run.pid, std::process::id() as i64);
        assert!(!run.started_at.is_empty());

        db.finish(&id, outcome::OK).expect("finish");
        let closed = db.get(&id).expect("get").expect("still there");
        assert!(!closed.is_open());
        assert_eq!(closed.outcome.as_deref(), Some(outcome::OK));
        assert!(closed.ended_at.is_some());

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn closing_an_unknown_or_already_closed_run_is_an_error() {
        let path = temp_db("badclose");
        let db = RunsDb::open_at(&path).expect("open");

        let err = db
            .finish("no-such-run", outcome::OK)
            .expect_err("must fail");
        assert!(err.to_string().contains("no-such-run"), "got: {err}");

        let id = db.begin(kind::CLI, Some("airsctl")).expect("begin");
        db.finish(&id, outcome::OK).expect("first close");
        assert!(
            db.finish(&id, outcome::FAILED).is_err(),
            "closing twice must be an error — a ledger that accepts it is not an audit trail"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A crash leaves a run open forever, and an open run reads as "still
    /// running". Startup must correct that — but only for processes that are
    /// genuinely gone.
    ///
    /// The first version of `reap_abandoned` tested `NOT (pid = ? AND host = ?)`
    /// and this test passed, because its fixture used a different *host*. That
    /// hid the real defect: any run from another **process on this machine**
    /// was reaped too, so `airsctl` marked the running desktop app abandoned.
    /// The fixture now uses this host with a dead pid, which is the case that
    /// actually matters.
    #[test]
    fn a_run_left_open_by_a_dead_process_is_reaped() {
        let path = temp_db("reap");
        let db = RunsDb::open_at(&path).expect("open");

        // A crashed run: THIS host, a pid that is not alive.
        let dead_pid = u32::MAX - 1;
        assert!(
            !crate::orphan::pid_checker::is_pid_alive(dead_pid),
            "fixture needs a pid that is genuinely not running"
        );
        db.conn
            .execute(
                "INSERT INTO runs (id, kind, started_at, host, pid)                  VALUES ('ghost', 'app', ?1, ?2, ?3)",
                params![now_rfc3339(), hostname(), dead_pid as i64],
            )
            .expect("insert ghost");

        // A run on another machine. We cannot read that process table, so it
        // must be left alone rather than guessed at.
        db.conn
            .execute(
                "INSERT INTO runs (id, kind, started_at, host, pid)                  VALUES ('elsewhere', 'app', ?1, 'some-other-host', 4242)",
                params![now_rfc3339()],
            )
            .expect("insert remote");

        // And one of ours, genuinely open and genuinely alive.
        let mine = db.begin(kind::APP, None).expect("begin");

        assert_eq!(db.open_runs().expect("open").len(), 3);
        assert_eq!(db.reap_abandoned().expect("reap"), 1, "only the dead one");

        assert_eq!(
            db.get("ghost")
                .expect("get")
                .expect("recorded")
                .outcome
                .as_deref(),
            Some(outcome::ABANDONED)
        );
        assert!(
            db.get("elsewhere")
                .expect("get")
                .expect("recorded")
                .is_open(),
            "a run on another host must not be judged from here"
        );
        assert!(
            db.get(&mine).expect("get").expect("there").is_open(),
            "this process's own live run must not be reaped out from under it"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The regression that motivated the fix, as its own case: a second live
    /// process on the same host must not have its run reaped.
    #[test]
    fn a_run_from_another_live_process_on_this_host_is_not_reaped() {
        let path = temp_db("reap-live");
        let db = RunsDb::open_at(&path).expect("open");

        // The current process stands in for "the desktop app, still running"
        // — its pid is alive by definition.
        db.conn
            .execute(
                "INSERT INTO runs (id, kind, started_at, host, pid)                  VALUES ('the-app', 'app', ?1, ?2, ?3)",
                params![now_rfc3339(), hostname(), std::process::id() as i64],
            )
            .expect("insert");

        assert_eq!(
            db.reap_abandoned().expect("reap"),
            0,
            "a live process on this host was marked abandoned — this is exactly what happened when airsctl ran while the app was open"
        );
        assert!(db.get("the-app").expect("get").expect("there").is_open());

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn runs_can_be_nested_so_a_turn_points_at_its_launch() {
        let path = temp_db("nested");
        let db = RunsDb::open_at(&path).expect("open");

        let app = db.begin(kind::APP, None).expect("app run");
        let turn = db
            .begin_with(kind::AGENT_TURN, Some("supervisor"), Some(&app), None)
            .expect("turn run");

        let t = db.get(&turn).expect("get").expect("there");
        assert_eq!(t.parent_run_id.as_deref(), Some(app.as_str()));
        assert_eq!(t.actor.as_deref(), Some("supervisor"));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn recent_returns_newest_first_and_respects_its_limit() {
        let path = temp_db("recent");
        let db = RunsDb::open_at(&path).expect("open");
        let mut ids = Vec::new();
        for _ in 0..5 {
            ids.push(db.begin(kind::CLI, None).expect("begin"));
        }

        let recent = db.recent(3).expect("recent");
        assert_eq!(recent.len(), 3, "limit must be honoured");
        assert_eq!(recent[0].id, *ids.last().unwrap(), "newest first");

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
