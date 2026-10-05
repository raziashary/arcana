// `~/.arcana/runs.db` — the run/audit ledger.
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
// ⚠ **Honest scope.** Worker ticks, worker operation history, and selected task
// status transitions now carry a `run_id` and have production readers. That is
// partial attribution, not complete action provenance: provider cost records,
// every file mutation, and all other persisted effects do not yet have a
// proven run-linked readback. This module existing is not evidence that every
// action is attributed. Keep the owning phase's consumer proof visible.
//
// See MDP `§5` (sequencing rationale) and `§7 P1-18`.

use rusqlite::{params, OptionalExtension};

use crate::db::migrate::{self, Migration};
use crate::db::timestamps::{now_rfc3339, parse_db_timestamp};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;

/// What kind of run a row describes.
///
/// A plain string in the database rather than a closed enum: a later phase adds
/// kinds without a migration, and an unknown kind read back is data, not a
/// parse failure.
pub mod kind {
    /// The desktop application, from launch to shutdown.
    pub const APP: &str = "app";
    /// One `arcanactl` invocation.
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
    pub const CANCELLED: &str = "cancelled";
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

/// Latest durable health/progress details for one fleet tick.
#[derive(Debug, Clone)]
pub struct WorkerRunState {
    pub run_id: String,
    pub worker_slug: String,
    pub session_id: String,
    pub phase: String,
    pub task_id: Option<i64>,
    pub heartbeat_at: String,
    pub last_progress_at: Option<String>,
    pub last_progress: Option<String>,
    pub failure_code: Option<String>,
    pub outcome: Option<String>,
    pub started_at: String,
}

#[derive(Debug, Clone)]
pub struct WorkerRunOwner {
    pub run_id: String,
    pub host: String,
    pub pid: i64,
    pub phase: String,
    pub outcome: Option<String>,
}

/// A side effect currently executing inside an open fleet worker run.
#[derive(Debug, Clone)]
pub struct ActiveWorkerOperation {
    pub operation: String,
    pub started_at: String,
    pub elapsed_ms: u64,
}

/// Observed result of one journaled operation. An intent without a following
/// outcome is surfaced as `Unknown` by the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationOutcome {
    Ok,
    Error,
    Unknown,
}

impl OperationOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Unknown => "unknown",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "ok" => Self::Ok,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }
}

/// Durable reconciliation decision attached to an operation intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationReconciliation {
    Unresolved,
    Reconciled,
    RetrySafe,
    DoNotRetry,
}

impl OperationReconciliation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
            Self::Reconciled => "reconciled",
            Self::RetrySafe => "retry_safe",
            Self::DoNotRetry => "do_not_retry",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "reconciled" => Self::Reconciled,
            "retry_safe" => Self::RetrySafe,
            "do_not_retry" => Self::DoNotRetry,
            _ => Self::Unresolved,
        }
    }
}

/// One intent and its latest append-only outcome/reconciliation observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationJournalEntry {
    /// Monotonic SQLite row cursor of the intent record.
    pub cursor: i64,
    pub run_id: String,
    pub task_id: Option<i64>,
    /// Caller-provided stable id for deduplicating intent creation.
    pub operation_id: String,
    /// Safe operation label; never tool arguments or provider content.
    pub operation: String,
    /// Bounded effect category such as `write_file` or `delete_file`.
    pub effect: String,
    /// Relative workspace paths expected to be affected by the operation.
    pub target_paths: Vec<String>,
    pub intent_at: String,
    /// `Unknown` when no outcome record exists yet.
    pub outcome: OperationOutcome,
    /// Relative workspace paths only.
    pub changed_paths: Vec<String>,
    pub outcome_at: Option<String>,
    pub reconciliation: OperationReconciliation,
    /// Actor and bounded reason code recorded by the single reconciliation decision.
    pub reconciliation_actor: Option<String>,
    pub reconciliation_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationJournalPage {
    pub entries: Vec<OperationJournalEntry>,
    /// Cursor after the final returned intent (or the supplied cursor if empty).
    pub next_cursor: i64,
    pub has_more: bool,
}

impl Run {
    /// Whether this run is still open.
    pub fn is_open(&self) -> bool {
        self.outcome.is_none()
    }
}

const LATEST_WORKER_TICK_BY_SLUG_SQL: &str =
    "SELECT s.run_id, s.worker_slug, s.session_id, s.phase, s.task_id,
                        s.heartbeat_at, s.last_progress_at, s.last_progress,
                        s.failure_code, r.outcome, r.started_at
                 FROM worker_run_state s
                 JOIN runs r ON r.id = s.run_id
                 WHERE s.worker_slug = ?1
                 ORDER BY julianday(r.started_at) DESC, r.id DESC LIMIT 1";

const LATEST_WORKER_RUN_OWNER_SQL: &str = "SELECT s.run_id, r.host, r.pid, s.phase, r.outcome
     FROM worker_run_state s JOIN runs r ON r.id = s.run_id
     WHERE s.worker_slug = ?1
     ORDER BY julianday(r.started_at) DESC, r.id DESC LIMIT 1";

const LATEST_WORKER_TICKS_SQL: &str =
    "SELECT s.run_id, s.worker_slug, s.session_id, s.phase, s.task_id,
            s.heartbeat_at, s.last_progress_at, s.last_progress,
            s.failure_code, r.outcome, r.started_at
     FROM worker_run_state s
     JOIN runs r ON r.id = s.run_id
     WHERE s.run_id = (
         SELECT s2.run_id FROM worker_run_state s2
         JOIN runs r2 ON r2.id = s2.run_id
         WHERE s2.worker_slug = s.worker_slug
         ORDER BY julianday(r2.started_at) DESC, r2.id DESC LIMIT 1
     )
     ORDER BY julianday(r.started_at) DESC, r.id DESC LIMIT ?1";

const RECENT_WORKER_PROGRESS_SQL: &str =
    "SELECT event_at, operation, outcome FROM worker_run_events
     WHERE run_id = ?1 ORDER BY id DESC LIMIT ?2";
const PRUNE_WORKER_PROGRESS_SQL: &str =
    "DELETE FROM worker_run_events WHERE run_id = ?1 AND id NOT IN (
                 SELECT id FROM worker_run_events WHERE run_id = ?1 ORDER BY id DESC LIMIT 500
             )";
const OPERATION_JOURNAL_PAGE_SQL: &str = r#"
    SELECT i.id, i.run_id, i.task_id, i.operation_id, i.operation, i.effect,
           i.target_paths, i.created_at,
           COALESCE((SELECT o.outcome FROM operation_journal o
                     WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                     ORDER BY o.id DESC LIMIT 1), 'unknown'),
           COALESCE((SELECT o.changed_paths FROM operation_journal o
                     WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                     ORDER BY o.id DESC LIMIT 1), '[]'),
           (SELECT o.created_at FROM operation_journal o
            WHERE o.parent_id = i.id AND o.record_type = 'outcome'
            ORDER BY o.id DESC LIMIT 1),
           COALESCE((SELECT r.reconciliation_state FROM operation_journal r
                     WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
                     ORDER BY r.id DESC LIMIT 1), 'unresolved'),
           (SELECT r.reconciliation_actor FROM operation_journal r
            WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
            ORDER BY r.id DESC LIMIT 1),
           (SELECT r.reconciliation_reason FROM operation_journal r
            WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
            ORDER BY r.id DESC LIMIT 1)
    FROM operation_journal i
    WHERE i.run_id = ?1 AND i.record_type = 'intent' AND i.id > ?2
    ORDER BY i.id ASC LIMIT ?3
"#;
const ACTIVE_WORKER_OPERATION_SQL: &str = r#"
    SELECT i.operation, i.created_at
    FROM operation_journal i
    JOIN runs r ON r.id = i.run_id
    JOIN worker_run_state s ON s.run_id = i.run_id
    WHERE i.run_id = ?1
      AND i.record_type = 'intent'
      AND r.outcome IS NULL
      AND s.phase = 'running'
      AND NOT EXISTS (
          SELECT 1 FROM operation_journal o
          WHERE o.parent_id = i.id AND o.record_type = 'outcome'
      )
      AND NOT EXISTS (
          SELECT 1 FROM operation_journal x
          WHERE x.parent_id = i.id AND x.record_type = 'reconciliation'
      )
    ORDER BY i.id DESC
    LIMIT 1
"#;

const OPEN_RUNS_SQL: &str = "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid,
            parent_run_id, detail
     FROM runs INDEXED BY idx_runs_open_instant WHERE outcome IS NULL
     ORDER BY julianday(started_at) ASC, id ASC";

const INTERRUPT_WORKER_TICKS_SQL: &str = "SELECT s.run_id FROM worker_run_state s
                     JOIN runs r ON r.id = s.run_id
                     WHERE s.worker_slug = ?1 AND r.outcome IS NULL
                       AND r.host = ?2 AND r.pid = ?3
                       AND s.phase IN ('starting', 'retrying', 'running')";

pub struct RunsDb {
    pub(crate) conn: rusqlite::Connection,
    supervisor_reconciliation_context: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

struct SupervisorReconciliationGuard {
    context: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for SupervisorReconciliationGuard {
    fn drop(&mut self) {
        self.context
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

const LADDER: &[Migration] = &[
    Migration {
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
    },
    Migration {
        version: 2,
        name: "worker-run-health-and-progress",
        sql: r#"
            CREATE TABLE worker_run_state (
                run_id            TEXT PRIMARY KEY NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                worker_slug       TEXT NOT NULL,
                session_id        TEXT NOT NULL,
                phase             TEXT NOT NULL CHECK (phase IN (
                                      'starting', 'retrying', 'running', 'completed',
                                      'failed', 'interrupted', 'cancelled', 'blocked'
                                  )),
                task_id           INTEGER,
                heartbeat_at      TEXT NOT NULL,
                last_progress_at  TEXT,
                last_progress     TEXT,
                failure_code      TEXT
            );
            CREATE INDEX idx_worker_run_state_worker
                ON worker_run_state(worker_slug, run_id);
            CREATE INDEX idx_worker_run_state_phase
                ON worker_run_state(phase, heartbeat_at);
            CREATE TABLE worker_run_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id      TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
                task_id     INTEGER,
                event_at    TEXT NOT NULL,
                operation   TEXT NOT NULL,
                outcome     TEXT NOT NULL CHECK (outcome IN ('ok', 'error'))
            );
            CREATE INDEX idx_worker_run_events_run_time
                ON worker_run_events(run_id, event_at DESC);
            CREATE TRIGGER worker_run_single_active_insert
            BEFORE INSERT ON worker_run_state
            WHEN NEW.phase IN ('starting', 'retrying', 'running')
                 AND EXISTS (
                     SELECT 1 FROM worker_run_state existing
                     JOIN runs existing_run ON existing_run.id = existing.run_id
                     WHERE existing.worker_slug = NEW.worker_slug
                       AND existing.phase IN ('starting', 'retrying', 'running')
                       AND existing_run.outcome IS NULL
                 )
            BEGIN
                SELECT RAISE(ABORT, 'worker already has an active run');
            END;
            CREATE TRIGGER worker_run_single_active_update
            BEFORE UPDATE OF phase ON worker_run_state
            WHEN NEW.phase IN ('starting', 'retrying', 'running')
                 AND EXISTS (
                     SELECT 1 FROM worker_run_state existing
                     JOIN runs existing_run ON existing_run.id = existing.run_id
                     WHERE existing.worker_slug = NEW.worker_slug
                       AND existing.run_id != NEW.run_id
                       AND existing.phase IN ('starting', 'retrying', 'running')
                       AND existing_run.outcome IS NULL
                 )
            BEGIN
                SELECT RAISE(ABORT, 'worker already has an active run');
            END;
        "#,
    },
    Migration {
        version: 3,
        name: "worker-run-progress-retention-index",
        sql: r#"
            CREATE INDEX idx_worker_run_events_run_id_id
                ON worker_run_events(run_id, id DESC);
        "#,
    },
    Migration {
        version: 4,
        name: "append-only-operation-journal",
        sql: r#"
            CREATE TABLE operation_journal (
                id                    INTEGER PRIMARY KEY AUTOINCREMENT,
                run_id                TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
                task_id               INTEGER CHECK (task_id IS NULL OR task_id > 0),
                record_type           TEXT NOT NULL CHECK (record_type IN ('intent', 'outcome', 'reconciliation')),
                operation_id          TEXT,
                operation             TEXT,
                effect                TEXT,
                target_paths          TEXT,
                parent_id             INTEGER REFERENCES operation_journal(id) ON DELETE RESTRICT,
                outcome               TEXT CHECK (outcome IS NULL OR outcome IN ('ok', 'error', 'unknown')),
                changed_paths         TEXT,
                reconciliation_state TEXT CHECK (reconciliation_state IS NULL OR
                                           reconciliation_state IN ('reconciled', 'retry_safe', 'do_not_retry')),
                created_at            TEXT NOT NULL,
                CHECK (
                    (record_type = 'intent' AND parent_id IS NULL AND operation_id IS NOT NULL
                        AND operation IS NOT NULL AND effect IS NOT NULL AND target_paths IS NOT NULL
                        AND outcome IS NULL AND changed_paths IS NULL AND reconciliation_state IS NULL)
                    OR
                    (record_type = 'outcome' AND parent_id IS NOT NULL AND operation_id IS NULL
                        AND operation IS NULL AND effect IS NULL AND target_paths IS NULL
                        AND outcome IS NOT NULL AND changed_paths IS NOT NULL AND reconciliation_state IS NULL)
                    OR
                    (record_type = 'reconciliation' AND parent_id IS NOT NULL AND operation_id IS NULL
                        AND operation IS NULL AND effect IS NULL AND target_paths IS NULL
                        AND outcome IS NULL AND changed_paths IS NULL AND reconciliation_state IS NOT NULL)
                )
            );
            CREATE UNIQUE INDEX idx_operation_journal_run_operation
                ON operation_journal(run_id, operation_id) WHERE record_type = 'intent';
            CREATE UNIQUE INDEX idx_operation_journal_one_outcome
                ON operation_journal(parent_id) WHERE record_type = 'outcome';
            CREATE INDEX idx_operation_journal_run_cursor
                ON operation_journal(run_id, id) WHERE record_type = 'intent';
            CREATE TRIGGER operation_journal_link_to_intent
            BEFORE INSERT ON operation_journal
            WHEN NEW.record_type IN ('outcome', 'reconciliation')
                 AND NOT EXISTS (
                     SELECT 1 FROM operation_journal intent
                     WHERE intent.id = NEW.parent_id
                       AND intent.record_type = 'intent'
                       AND intent.run_id = NEW.run_id
                       AND intent.task_id IS NEW.task_id
                 )
            BEGIN SELECT RAISE(ABORT, 'operation journal record must link to its intent'); END;
            CREATE TRIGGER operation_journal_no_update
            BEFORE UPDATE ON operation_journal
            BEGIN SELECT RAISE(ABORT, 'operation journal is append-only'); END;
            CREATE TRIGGER operation_journal_no_delete
            BEFORE DELETE ON operation_journal
            BEGIN SELECT RAISE(ABORT, 'operation journal is append-only'); END;
        "#,
    },
    Migration {
        version: 5,
        name: "single-winner-operation-reconciliation",
        sql: r#"
            CREATE UNIQUE INDEX idx_operation_journal_one_reconciliation
                ON operation_journal(parent_id) WHERE record_type = 'reconciliation';
        "#,
    },
    Migration {
        version: 6,
        name: "audited-supervisor-operation-reconciliation",
        sql: r#"
            ALTER TABLE operation_journal ADD COLUMN reconciliation_actor TEXT;
            ALTER TABLE operation_journal ADD COLUMN reconciliation_reason TEXT;
            CREATE TRIGGER operation_journal_reconciliation_guard
            BEFORE INSERT ON operation_journal
            WHEN NEW.record_type = 'reconciliation' AND (
                NEW.reconciliation_actor != 'supervisor'
                OR NOT (
                    (NEW.reconciliation_state = 'reconciled'
                     AND NEW.reconciliation_reason = 'effect_verified_complete')
                    OR (NEW.reconciliation_state = 'retry_safe'
                        AND NEW.reconciliation_reason = 'idempotent_retry_verified')
                    OR (NEW.reconciliation_state = 'do_not_retry'
                        AND NEW.reconciliation_reason = 'unsafe_to_repeat')
                )
                OR arcana_supervisor_reconciliation_authorized() != 1
                OR NOT EXISTS (
                    SELECT 1 FROM operation_journal intent
                    WHERE intent.id = NEW.parent_id
                      AND intent.record_type = 'intent'
                      AND intent.effect != 'Board'
                      AND intent.run_id = NEW.run_id
                      AND intent.task_id IS NEW.task_id
                      AND COALESCE((SELECT outcome FROM operation_journal outcome
                                    WHERE outcome.parent_id = intent.id
                                      AND outcome.record_type = 'outcome'
                                    ORDER BY outcome.id DESC LIMIT 1), 'unknown') = 'unknown'
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'operation reconciliation requires an authorized Supervisor decision on an unknown non-Board effect');
            END;
            CREATE TRIGGER operation_journal_reconciliation_metadata_shape
            BEFORE INSERT ON operation_journal
            WHEN NEW.record_type != 'reconciliation'
                 AND (NEW.reconciliation_actor IS NOT NULL OR NEW.reconciliation_reason IS NOT NULL)
            BEGIN
                SELECT RAISE(ABORT, 'reconciliation metadata is only valid on reconciliation records');
            END;
        "#,
    },
    Migration {
        version: 7,
        name: "task-scoped-operation-journal-index",
        sql: r#"
            CREATE INDEX idx_operation_journal_task_cursor
                ON operation_journal(task_id, id DESC)
                WHERE record_type = 'intent' AND task_id IS NOT NULL;
        "#,
    },
    Migration {
        version: 8,
        name: "run-start-instant-order-indexes",
        sql: r#"
            CREATE INDEX idx_runs_started_instant
                ON runs(julianday(started_at) DESC, id DESC);
            CREATE INDEX idx_runs_open_instant
                ON runs(julianday(started_at), id)
                WHERE outcome IS NULL;
        "#,
    },
];

impl RunsDb {
    pub fn open() -> Result<Self> {
        Self::open_at(&crate::db::arcana_dir()?.join("runs.db"))
    }

    /// As `open`, against an explicit path, so tests never write to the user's
    /// real ledger.
    pub fn open_at(path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(path)?;
        let supervisor_reconciliation_context =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let authorized = std::sync::Arc::clone(&supervisor_reconciliation_context);
        conn.create_scalar_function(
            "arcana_supervisor_reconciliation_authorized",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |_| {
                Ok(i64::from(
                    authorized.load(std::sync::atomic::Ordering::SeqCst),
                ))
            },
        )?;
        let db = Self {
            conn,
            supervisor_reconciliation_context,
        };
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
        let timing = OperationTimer::start("database_run_begin")
            .with_database("runs")
            .with_run_id(&id);
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
            .map_err(|e| ArcanaError::Agent(format!("opening a run: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(id)
    }

    /// Atomically create a fleet run and its initial durable health row.
    pub fn begin_worker_tick(&self, worker_slug: &str, session_id: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let timing = OperationTimer::start("database_worker_tick_begin")
            .with_database("runs")
            .with_worker(worker_slug)
            .with_session_id(session_id)
            .with_run_id(&id);
        let now = now_rfc3339();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO runs (id, kind, actor, started_at, host, pid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &id,
                kind::FLEET_TICK,
                worker_slug,
                &now,
                hostname(),
                std::process::id() as i64
            ],
        )
        .map_err(|e| ArcanaError::Agent(format!("opening worker run: {e}")))?;
        tx.execute(
            "INSERT INTO worker_run_state
                 (run_id, worker_slug, session_id, phase, heartbeat_at)
             VALUES (?1, ?2, ?3, 'starting', ?4)",
            params![&id, worker_slug, session_id, &now],
        )
        .map_err(|e| ArcanaError::Agent(format!("opening worker run state: {e}")))?;
        tx.commit()
            .map_err(|e| ArcanaError::Agent(format!("committing worker run: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(id)
    }

    /// Persist a startup/run phase without exposing prompt, file, or command data.
    pub fn update_worker_tick_phase(
        &self,
        run_id: &str,
        phase: &str,
        task_id: Option<i64>,
        failure_code: Option<&str>,
    ) -> Result<()> {
        let timing = OperationTimer::start("database_worker_tick_phase_update")
            .with_database("runs")
            .with_run_id(run_id)
            .with_task_id(task_id);
        let failure_code = failure_code.map(|code| safe_label(code, 64, "run_failed"));
        let rows = self
            .conn
            .execute(
                "UPDATE worker_run_state
                 SET phase = ?1,
                     task_id = COALESCE(?2, task_id),
                     failure_code = COALESCE(?3, failure_code),
                     heartbeat_at = ?4
                 WHERE run_id = ?5",
                params![phase, task_id, failure_code, now_rfc3339(), run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("updating worker run phase: {e}")))?;
        if rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "worker run state not found for run {run_id}"
            )));
        }
        timing.with_result_count(rows).succeed();
        Ok(())
    }

    /// Refresh liveness without claiming that application work made progress.
    pub fn heartbeat_worker_tick(&self, run_id: &str) -> Result<()> {
        let timing = OperationTimer::start("database_worker_tick_heartbeat")
            .with_database("runs")
            .with_run_id(run_id);
        let rows = self
            .conn
            .execute(
                "UPDATE worker_run_state SET heartbeat_at = ?1 WHERE run_id = ?2",
                params![now_rfc3339(), run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("updating worker heartbeat: {e}")))?;
        if rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "worker run state not found for run {run_id}"
            )));
        }
        timing.with_result_count(rows).succeed();
        Ok(())
    }

    /// Record a completed safe operation label and its task attribution.
    pub fn record_worker_progress(
        &self,
        run_id: &str,
        task_id: Option<i64>,
        progress: &str,
    ) -> Result<()> {
        let timing = OperationTimer::start("database_worker_progress_record")
            .with_database("runs")
            .with_run_id(run_id)
            .with_task_id(task_id);
        let progress = safe_label(progress, 80, "operation");
        let (operation, outcome) = progress
            .rsplit_once('_')
            .filter(|(_, outcome)| matches!(*outcome, "ok" | "error"))
            .unwrap_or((progress.as_str(), "ok"));
        let tx = self.conn.unchecked_transaction()?;
        let now = now_rfc3339();
        let rows = tx
            .execute(
                "UPDATE worker_run_state
                 SET phase = 'running',
                     task_id = COALESCE(?1, task_id),
                     heartbeat_at = ?2,
                     last_progress_at = ?2,
                     last_progress = ?3
                 WHERE run_id = ?4",
                params![task_id, &now, progress, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("recording worker progress: {e}")))?;
        if rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "worker run state not found for run {run_id}"
            )));
        }
        tx.execute(
            "INSERT INTO worker_run_events (run_id, task_id, event_at, operation, outcome)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, task_id, &now, operation, outcome],
        )
        .map_err(|e| ArcanaError::Agent(format!("appending worker progress event: {e}")))?;
        tx.execute(PRUNE_WORKER_PROGRESS_SQL, params![run_id])
            .map_err(|e| ArcanaError::Agent(format!("pruning worker progress events: {e}")))?;
        tx.commit()
            .map_err(|e| ArcanaError::Agent(format!("committing worker progress: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    /// Return a bounded sequence of completed safe operation labels.
    pub fn recent_worker_progress(
        &self,
        run_id: &str,
        limit: i64,
    ) -> Result<Vec<(String, String, String)>> {
        let timing = OperationTimer::start("database_worker_progress_recent_read")
            .with_database("runs")
            .with_run_id(run_id);
        let mut stmt = self.conn.prepare(RECENT_WORKER_PROGRESS_SQL)?;
        let rows = stmt.query_map(params![run_id, limit.clamp(1, 50)], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        let progress = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("reading worker progress events: {e}")))?;
        timing.with_result_count(progress.len()).succeed();
        Ok(progress)
    }

    /// Append a bounded operation intent before its caller performs a side effect.
    /// The returned row id is the durable cursor used by outcome and page APIs.
    pub fn record_operation_intent(
        &self,
        run_id: &str,
        task_id: Option<i64>,
        operation_id: &str,
        operation: &str,
        effect: &str,
        target_paths: &[String],
    ) -> Result<i64> {
        validate_journal_token(run_id, "run id", 128)?;
        validate_journal_token(operation_id, "operation id", 128)?;
        validate_journal_token(operation, "operation", 80)?;
        validate_journal_token(effect, "effect", 80)?;
        if task_id.is_some_and(|task_id| task_id <= 0) {
            return Err(ArcanaError::Agent(
                "operation journal task id must be positive".to_string(),
            ));
        }
        let target_paths = validate_journal_paths(target_paths)?;
        let target_paths = serde_json::to_string(&target_paths).map_err(|error| {
            ArcanaError::Agent(format!("encoding operation target paths: {error}"))
        })?;
        let tx = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        )?;
        if let Some(task_id) = task_id.filter(|_| effect != "Board") {
            let unresolved: Option<(i64, String)> = tx
                .query_row(
                    "SELECT i.id, i.operation
                     FROM operation_journal i
                     WHERE i.task_id = ?1
                       AND i.record_type = 'intent'
                       AND i.effect <> 'Board'
                       AND COALESCE((
                           SELECT o.outcome FROM operation_journal o
                           WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                           ORDER BY o.id DESC LIMIT 1
                       ), 'unknown') = 'unknown'
                       AND COALESCE((
                           SELECT r.reconciliation_state FROM operation_journal r
                           WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
                           ORDER BY r.id DESC LIMIT 1
                       ), 'unresolved') IN ('unresolved', 'do_not_retry')
                     ORDER BY i.id DESC LIMIT 1",
                    params![task_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            if let Some((intent_cursor, operation)) = unresolved {
                return Err(ArcanaError::OperationReconciliationRequired {
                    operation,
                    intent_cursor,
                });
            }
        }
        tx.execute(
            "INSERT INTO operation_journal
                 (run_id, task_id, record_type, operation_id, operation, effect,
                  target_paths, created_at)
             VALUES (?1, ?2, 'intent', ?3, ?4, ?5, ?6, ?7)",
            params![
                run_id,
                task_id,
                operation_id,
                operation,
                effect,
                target_paths,
                now_rfc3339()
            ],
        )
        .map_err(|error| ArcanaError::Agent(format!("appending operation intent: {error}")))?;
        let cursor = tx.last_insert_rowid();
        tx.commit()
            .map_err(|error| ArcanaError::Agent(format!("committing operation intent: {error}")))?;
        Ok(cursor)
    }

    /// Append the observed result for an earlier intent. An ambiguous/lost
    /// acknowledgement must be written as `Unknown`; it is never inferred as
    /// a failure or permission to replay.
    pub fn record_operation_outcome(
        &self,
        run_id: &str,
        intent_cursor: i64,
        outcome: OperationOutcome,
        changed_paths: &[String],
    ) -> Result<i64> {
        validate_journal_token(run_id, "run id", 128)?;
        if intent_cursor <= 0 {
            return Err(ArcanaError::Agent(
                "operation journal cursor must be positive".to_string(),
            ));
        }
        let changed_paths = validate_journal_paths(changed_paths)?;
        let changed_paths = serde_json::to_string(&changed_paths).map_err(|error| {
            ArcanaError::Agent(format!("encoding operation changed paths: {error}"))
        })?;
        let tx = self.conn.unchecked_transaction()?;
        let intent_exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM operation_journal
             WHERE id = ?1 AND run_id = ?2 AND record_type = 'intent')",
            params![intent_cursor, run_id],
            |row| row.get(0),
        )?;
        if !intent_exists {
            return Err(ArcanaError::Agent(
                "operation journal intent was not found for this run".to_string(),
            ));
        }
        tx.execute(
            "INSERT INTO operation_journal
                 (run_id, task_id, record_type, parent_id, outcome, changed_paths, created_at)
             SELECT run_id, task_id, 'outcome', id, ?1, ?2, ?3
             FROM operation_journal WHERE id = ?4 AND record_type = 'intent'",
            params![
                outcome.as_str(),
                changed_paths,
                now_rfc3339(),
                intent_cursor
            ],
        )
        .map_err(|error| ArcanaError::Agent(format!("appending operation outcome: {error}")))?;
        let cursor = tx.last_insert_rowid();
        tx.commit().map_err(|error| {
            ArcanaError::Agent(format!("committing operation outcome: {error}"))
        })?;
        Ok(cursor)
    }

    /// Append a reconciliation decision without rewriting the original intent
    /// or observed outcome.
    #[cfg(test)]
    pub(crate) fn record_operation_reconciliation(
        &self,
        run_id: &str,
        intent_cursor: i64,
        state: OperationReconciliation,
    ) -> Result<i64> {
        self.reconcile_operation_as_supervisor(
            "supervisor",
            run_id,
            intent_cursor,
            state,
            match state {
                OperationReconciliation::Reconciled => "effect_verified_complete",
                OperationReconciliation::RetrySafe => "idempotent_retry_verified",
                OperationReconciliation::DoNotRetry => "unsafe_to_repeat",
                OperationReconciliation::Unresolved => "",
            },
        )
    }

    /// Record the one audited Supervisor decision for an unknown task side effect.
    pub fn reconcile_operation_as_supervisor(
        &self,
        supervisor_slug: &str,
        run_id: &str,
        intent_cursor: i64,
        state: OperationReconciliation,
        reason_code: &str,
    ) -> Result<i64> {
        validate_journal_token(run_id, "run id", 128)?;
        if supervisor_slug.trim() != "supervisor" {
            return Err(ArcanaError::Agent(
                "only the Supervisor can reconcile an operation".to_string(),
            ));
        }
        let reason_code = reason_code.trim();
        let reason_matches_state = matches!(
            (state, reason_code),
            (
                OperationReconciliation::Reconciled,
                "effect_verified_complete"
            ) | (
                OperationReconciliation::RetrySafe,
                "idempotent_retry_verified"
            ) | (OperationReconciliation::DoNotRetry, "unsafe_to_repeat")
        );
        if intent_cursor <= 0 || !reason_matches_state {
            return Err(ArcanaError::Agent(
                "operation reconciliation requires a positive cursor, a resolved decision, and its matching reason code".to_string(),
            ));
        }
        let tx = self.conn.unchecked_transaction()?;
        self.supervisor_reconciliation_context
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let authorization = SupervisorReconciliationGuard {
            context: std::sync::Arc::clone(&self.supervisor_reconciliation_context),
        };
        let rows = tx
            .execute(
                "INSERT INTO operation_journal
                 (run_id, task_id, record_type, parent_id, reconciliation_state,
                  reconciliation_actor, reconciliation_reason, created_at)
             SELECT run_id, task_id, 'reconciliation', id, ?1, ?2, ?3, ?4
             FROM operation_journal
             WHERE id = ?5 AND run_id = ?6 AND record_type = 'intent'
               AND effect != 'Board'
               AND COALESCE((SELECT outcome FROM operation_journal outcome
                             WHERE outcome.parent_id = operation_journal.id
                               AND outcome.record_type = 'outcome'
                             ORDER BY outcome.id DESC LIMIT 1), 'unknown') = 'unknown'
               AND NOT EXISTS (SELECT 1 FROM operation_journal prior
                               WHERE prior.parent_id = operation_journal.id
                                 AND prior.record_type = 'reconciliation')",
                params![
                    state.as_str(),
                    supervisor_slug,
                    reason_code,
                    now_rfc3339(),
                    intent_cursor,
                    run_id
                ],
            )
            .map_err(|error| {
                ArcanaError::Agent(format!("appending operation reconciliation: {error}"))
            })?;
        drop(authorization);
        if rows == 0 {
            return Err(ArcanaError::Agent(
                "operation journal intent was not found for this run".to_string(),
            ));
        }
        let cursor = tx.last_insert_rowid();
        tx.commit().map_err(|error| {
            ArcanaError::Agent(format!("committing operation reconciliation: {error}"))
        })?;
        Ok(cursor)
    }

    /// Return an unresolved non-board side effect for a task, if one exists.
    /// Task-board effects are reconciled from the authoritative tasks DB; file,
    /// command, and connector effects must be explicitly resolved before a
    /// worker can claim the task again.
    pub fn unresolved_task_side_effect(
        &self,
        task_id: i64,
    ) -> Result<Option<(String, String, i64)>> {
        if task_id <= 0 {
            return Err(ArcanaError::Agent(
                "task id must be positive when checking operation recovery".to_string(),
            ));
        }
        let timing = OperationTimer::start("database_task_unresolved_side_effect_read")
            .with_database("runs")
            .with_task_id(Some(task_id));
        let unresolved = self
            .conn
            .query_row(
                "SELECT i.run_id, i.operation, i.id
                 FROM operation_journal i
                 WHERE i.task_id = ?1
                   AND i.record_type = 'intent'
                   AND i.effect <> 'Board'
                   AND COALESCE((
                       SELECT o.outcome FROM operation_journal o
                       WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                       ORDER BY o.id DESC LIMIT 1
                   ), 'unknown') = 'unknown'
                   AND COALESCE((
                       SELECT r.reconciliation_state FROM operation_journal r
                       WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
                       ORDER BY r.id DESC LIMIT 1
                   ), 'unresolved') IN ('unresolved', 'do_not_retry')
                 ORDER BY i.id DESC LIMIT 1",
                params![task_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        timing
            .with_result_count(usize::from(unresolved.is_some()))
            .succeed();
        Ok(unresolved)
    }

    /// Read a bounded, ascending page of intent records. An intent lacking an
    /// outcome remains visible as `Unknown`/`Unresolved` after interruption.
    pub fn operation_journal_page(
        &self,
        run_id: &str,
        after_cursor: i64,
        limit: i64,
    ) -> Result<OperationJournalPage> {
        validate_journal_token(run_id, "run id", 128)?;
        if after_cursor < 0 {
            return Err(ArcanaError::Agent(
                "operation journal cursor cannot be negative".to_string(),
            ));
        }
        let bounded_limit = limit.clamp(1, 50);
        let mut statement = self.conn.prepare(OPERATION_JOURNAL_PAGE_SQL)?;
        let rows =
            statement.query_map(params![run_id, after_cursor, bounded_limit + 1], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<String>>(13)?,
                ))
            })?;
        let raw_entries = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = raw_entries.len() > bounded_limit as usize;
        let mut entries = Vec::with_capacity(raw_entries.len().min(bounded_limit as usize));
        for (
            cursor,
            run_id,
            task_id,
            operation_id,
            operation,
            effect,
            target_paths,
            intent_at,
            outcome,
            changed_paths,
            outcome_at,
            reconciliation,
            reconciliation_actor,
            reconciliation_reason,
        ) in raw_entries.into_iter().take(bounded_limit as usize)
        {
            entries.push(OperationJournalEntry {
                cursor,
                run_id,
                task_id,
                operation_id,
                operation,
                effect,
                target_paths: decode_journal_paths(&target_paths, "target")?,
                intent_at,
                outcome: OperationOutcome::parse(&outcome),
                changed_paths: decode_journal_paths(&changed_paths, "changed")?,
                outcome_at,
                reconciliation: OperationReconciliation::parse(&reconciliation),
                reconciliation_actor,
                reconciliation_reason,
            });
        }
        let next_cursor = entries.last().map_or(after_cursor, |entry| entry.cursor);
        Ok(OperationJournalPage {
            entries,
            next_cursor,
            has_more,
        })
    }

    /// Read the latest unresolved operation only while its worker run is live.
    /// An unresolved intent from an interrupted/finished run is recovery work,
    /// not an operation still executing.
    pub fn active_worker_operation(&self, run_id: &str) -> Result<Option<ActiveWorkerOperation>> {
        validate_journal_token(run_id, "run id", 128)?;
        let timing = OperationTimer::start("database_worker_active_operation_read")
            .with_database("runs")
            .with_run_id(run_id);
        let raw = self
            .conn
            .query_row(ACTIVE_WORKER_OPERATION_SQL, params![run_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .optional()?;
        let active = raw
            .map(|(operation, started_at)| -> Result<ActiveWorkerOperation> {
                let started = parse_db_timestamp(&started_at).map_err(|_| {
                    ArcanaError::Agent("active worker operation timestamp is invalid".to_string())
                })?;
                let elapsed_ms = chrono::Utc::now()
                    .signed_duration_since(started)
                    .num_milliseconds()
                    .max(0) as u64;
                Ok(ActiveWorkerOperation {
                    operation: safe_label(&operation, 80, "operation"),
                    started_at,
                    elapsed_ms,
                })
            })
            .transpose()?;
        timing
            .with_result_count(usize::from(active.is_some()))
            .succeed();
        Ok(active)
    }

    /// Finish state and ledger rows atomically so readers never observe a
    /// terminal worker phase paired with an open run (or the reverse).
    pub fn finish_worker_tick(
        &self,
        run_id: &str,
        phase: &str,
        outcome: &str,
        failure_code: Option<&str>,
    ) -> Result<()> {
        let timing = OperationTimer::start("database_worker_tick_finish")
            .with_database("runs")
            .with_run_id(run_id);
        let failure_code = failure_code.map(|code| safe_label(code, 64, "run_failed"));
        let now = now_rfc3339();
        let tx = self.conn.unchecked_transaction()?;
        let state_rows = tx
            .execute(
                "UPDATE worker_run_state
                 SET phase = ?1, failure_code = COALESCE(?2, failure_code), heartbeat_at = ?3
                 WHERE run_id = ?4",
                params![phase, failure_code, &now, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("finishing worker state: {e}")))?;
        if state_rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "worker run state not found for run {run_id}"
            )));
        }
        let run_rows = tx
            .execute(
                "UPDATE runs SET ended_at = ?1, outcome = ?2
                 WHERE id = ?3 AND outcome IS NULL",
                params![&now, outcome, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("finishing worker run: {e}")))?;
        if run_rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "no open worker run {run_id} to finish"
            )));
        }
        tx.commit()
            .map_err(|e| ArcanaError::Agent(format!("committing worker run finish: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(())
    }

    /// Mark still-open worker ticks interrupted after task ownership has been
    /// reconciled by the scheduler.
    pub fn interrupt_worker_ticks(&self, worker_slug: &str, failure_code: &str) -> Result<usize> {
        let timing = OperationTimer::start("database_worker_ticks_interrupt")
            .with_database("runs")
            .with_worker(worker_slug);
        let failure_code = safe_label(failure_code, 64, "interrupted");
        let tx = self.conn.unchecked_transaction()?;
        let run_ids = {
            let mut stmt = tx
                .prepare(INTERRUPT_WORKER_TICKS_SQL)
                .map_err(|e| ArcanaError::Agent(format!("reading interrupted worker runs: {e}")))?;
            let rows = stmt
                .query_map(
                    params![worker_slug, hostname(), std::process::id() as i64],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|e| {
                    ArcanaError::Agent(format!("querying interrupted worker runs: {e}"))
                })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        let now = now_rfc3339();
        for run_id in &run_ids {
            tx.execute(
                "UPDATE worker_run_state SET phase = 'interrupted', failure_code = ?1,
                     heartbeat_at = ?2 WHERE run_id = ?3",
                params![&failure_code, &now, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("marking worker run interrupted: {e}")))?;
            tx.execute(
                "UPDATE runs SET ended_at = ?1, outcome = ?2
                 WHERE id = ?3 AND outcome IS NULL",
                params![&now, outcome::ABANDONED, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("closing interrupted worker run: {e}")))?;
        }
        tx.commit()
            .map_err(|e| ArcanaError::Agent(format!("committing interrupted worker runs: {e}")))?;
        timing.with_result_count(run_ids.len()).succeed();
        Ok(run_ids.len())
    }

    /// Return the newest durable worker tick for a worker slug.
    pub fn latest_worker_tick(&self, worker_slug: &str) -> Result<Option<WorkerRunState>> {
        let mut timing = OperationTimer::start("database_worker_tick_latest_read")
            .with_database("runs")
            .with_worker(worker_slug);
        let state = self
            .conn
            .query_row(
                LATEST_WORKER_TICK_BY_SLUG_SQL,
                params![worker_slug],
                row_to_worker_run_state,
            )
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("reading worker run state: {e}")))?;
        timing = timing.with_result_count(if state.is_some() { 1 } else { 0 });
        timing.succeed();
        Ok(state)
    }

    /// Return the newest run owner for a worker, including ended runs, so a
    /// recovery caller can distinguish a live foreign process from a dead run.
    pub fn latest_worker_run_owner(&self, worker_slug: &str) -> Result<Option<WorkerRunOwner>> {
        let mut timing = OperationTimer::start("database_worker_run_owner_latest_read")
            .with_database("runs")
            .with_worker(worker_slug);
        let owner = self
            .conn
            .query_row(LATEST_WORKER_RUN_OWNER_SQL, params![worker_slug], |row| {
                Ok(WorkerRunOwner {
                    run_id: row.get(0)?,
                    host: row.get(1)?,
                    pid: row.get(2)?,
                    phase: row.get(3)?,
                    outcome: row.get(4)?,
                })
            })
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("reading worker run owner: {e}")))?;
        timing = timing.with_result_count(if owner.is_some() { 1 } else { 0 });
        timing.succeed();
        Ok(owner)
    }

    /// Read the newest durable fleet tick for every worker, newest first.
    pub fn latest_worker_ticks(&self, limit: i64) -> Result<Vec<WorkerRunState>> {
        let mut timing =
            OperationTimer::start("database_worker_ticks_latest_read").with_database("runs");
        let mut stmt = self
            .conn
            .prepare(LATEST_WORKER_TICKS_SQL)
            .map_err(|e| ArcanaError::Agent(format!("preparing worker run states: {e}")))?;
        let rows = stmt
            .query_map(params![limit.clamp(1, 100)], row_to_worker_run_state)
            .map_err(|e| ArcanaError::Agent(format!("querying worker run states: {e}")))?;
        let states = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("collecting worker run states: {e}")))?;
        timing = timing.with_result_count(states.len());
        timing.succeed();
        Ok(states)
    }

    /// Close a run.
    ///
    /// Closing a run that does not exist, or one already closed, is an error
    /// rather than a silent no-op: both mean the caller's idea of the run
    /// differs from the ledger's, and a ledger that quietly accepts that is not
    /// an audit trail.
    pub fn finish(&self, run_id: &str, outcome: &str) -> Result<()> {
        let timing = OperationTimer::start("database_run_finish").with_database("runs");
        let rows = self
            .conn
            .execute(
                "UPDATE runs SET ended_at = ?1, outcome = ?2 \
                 WHERE id = ?3 AND outcome IS NULL",
                params![now_rfc3339(), outcome, run_id],
            )
            .map_err(|e| ArcanaError::Agent(format!("closing run {run_id}: {e}")))?;
        if rows == 0 {
            timing.with_result_count(0).set_outcome("not_found");
            return Err(ArcanaError::Agent(format!(
                "no open run {run_id} to close — it does not exist, or it was already closed"
            )));
        }
        timing.with_run_id(run_id).with_result_count(rows).succeed();
        Ok(())
    }

    pub fn get(&self, run_id: &str) -> Result<Option<Run>> {
        let timing = OperationTimer::start("database_run_read").with_database("runs");
        let run = self
            .conn
            .query_row(
                "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid, \
                        parent_run_id, detail \
                 FROM runs WHERE id = ?1",
                params![run_id],
                row_to_run,
            )
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("reading run {run_id}: {e}")))?;
        if let Some(run) = &run {
            timing.with_run_id(&run.id).with_result_count(1).succeed();
        } else {
            timing.with_result_count(0).set_outcome("not_found");
        }
        Ok(run)
    }

    /// The most recent runs, newest first.
    pub fn recent(&self, limit: i64) -> Result<Vec<Run>> {
        let timing = OperationTimer::start("database_runs_recent_read").with_database("runs");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, actor, started_at, ended_at, outcome, host, pid, \
                        parent_run_id, detail \
                 FROM runs
                 ORDER BY julianday(started_at) DESC, id DESC LIMIT ?1",
            )
            .map_err(|e| ArcanaError::Agent(format!("recent runs prepare: {e}")))?;
        let rows = stmt
            .query_map(params![limit], row_to_run)
            .map_err(|e| ArcanaError::Agent(format!("recent runs query: {e}")))?;
        let runs = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("recent runs collect: {e}")))?;
        timing.with_result_count(runs.len()).succeed();
        Ok(runs)
    }

    /// Runs still open — no `outcome`.
    pub fn open_runs(&self) -> Result<Vec<Run>> {
        let timing = OperationTimer::start("database_runs_open_read").with_database("runs");
        let mut stmt = self
            .conn
            .prepare(OPEN_RUNS_SQL)
            .map_err(|e| ArcanaError::Agent(format!("open runs prepare: {e}")))?;
        let rows = stmt
            .query_map([], row_to_run)
            .map_err(|e| ArcanaError::Agent(format!("open runs query: {e}")))?;
        let runs = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("open runs collect: {e}")))?;
        timing.with_result_count(runs.len()).succeed();
        Ok(runs)
    }

    /// Close runs left open by a process that is no longer alive.
    ///
    /// A crash leaves a run open forever, and an open run reads as "still
    /// running" to anything querying the ledger.
    ///
    /// ⚠ **The first version tested `NOT (pid = ? AND host = ?)`** — "not my
    /// pid" treated as "dead". Running `arcanactl` while the desktop app was open
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
        let timing = OperationTimer::start("database_runs_reap_abandoned").with_database("runs");
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
            let tx = self.conn.unchecked_transaction()?;
            let changed = tx
                .execute(
                    "UPDATE runs SET ended_at = ?1, outcome = ?2 \
                     WHERE id = ?3 AND outcome IS NULL",
                    params![now_rfc3339(), outcome::ABANDONED, &run.id],
                )
                .map_err(|e| ArcanaError::Agent(format!("reaping run {}: {e}", run.id)))?;
            if changed == 0 {
                tx.rollback()?;
                continue;
            }
            tx.execute(
                "UPDATE worker_run_state
                     SET phase = 'interrupted', failure_code = 'process_exited', heartbeat_at = ?1
                     WHERE run_id = ?2 AND phase IN ('starting', 'retrying', 'running')",
                params![now_rfc3339(), &run.id],
            )
            .map_err(|e| ArcanaError::Agent(format!("reaping worker state {}: {e}", run.id)))?;
            tx.commit().map_err(|e| {
                ArcanaError::Agent(format!("committing reaped run {}: {e}", run.id))
            })?;
            reaped += 1;
        }

        if reaped > 0 {
            tracing::info!("[runs] closed {reaped} run(s) left open by a dead process");
        }
        timing.with_result_count(reaped).succeed();
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

fn row_to_worker_run_state(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkerRunState> {
    Ok(WorkerRunState {
        run_id: row.get(0)?,
        worker_slug: row.get(1)?,
        session_id: row.get(2)?,
        phase: row.get(3)?,
        task_id: row.get(4)?,
        heartbeat_at: row.get(5)?,
        last_progress_at: row.get(6)?,
        last_progress: row.get(7)?,
        failure_code: row.get(8)?,
        outcome: row.get(9)?,
        started_at: row.get(10)?,
    })
}

fn safe_label(value: &str, max_chars: usize, fallback: &str) -> String {
    let label: String = value
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':' | '.')
        })
        .take(max_chars)
        .collect();
    if label.is_empty() {
        fallback.to_string()
    } else {
        label
    }
}

fn validate_journal_token(value: &str, field: &str, max_bytes: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(ArcanaError::Agent(format!(
            "operation journal {field} must be a safe token of at most {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_journal_paths(paths: &[String]) -> Result<Vec<String>> {
    if paths.len() > 64 {
        return Err(ArcanaError::Agent(
            "operation journal path list exceeds 64 entries".to_string(),
        ));
    }
    for path in paths {
        if path.is_empty()
            || path.len() > 512
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains(':')
            || path.chars().any(char::is_control)
            || path
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(ArcanaError::Agent(
                "operation journal paths must be safe relative workspace references".to_string(),
            ));
        }
    }
    let encoded = serde_json::to_vec(paths)
        .map_err(|error| ArcanaError::Agent(format!("encoding operation paths: {error}")))?;
    if encoded.len() > 8_192 {
        return Err(ArcanaError::Agent(
            "operation journal path data exceeds 8192 bytes".to_string(),
        ));
    }
    Ok(paths.to_vec())
}

fn decode_journal_paths(value: &str, kind: &str) -> Result<Vec<String>> {
    let paths: Vec<String> = serde_json::from_str(value).map_err(|error| {
        ArcanaError::Agent(format!("reading operation journal {kind} paths: {error}"))
    })?;
    validate_journal_paths(&paths)
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

    fn explain_with_text_parameter(
        conn: &rusqlite::Connection,
        sql: &str,
        value: &str,
    ) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = conn.prepare(&explain_sql).expect("prepare query plan");
        statement
            .query_map([value], |row| row.get::<_, String>(3))
            .expect("explain query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    fn explain_with_integer_parameter(
        conn: &rusqlite::Connection,
        sql: &str,
        value: i64,
    ) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = conn.prepare(&explain_sql).expect("prepare query plan");
        statement
            .query_map([value], |row| row.get::<_, String>(3))
            .expect("explain query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    fn explain_with_text_and_integer_parameters(
        conn: &rusqlite::Connection,
        sql: &str,
        text: &str,
        integer: i64,
    ) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = conn.prepare(&explain_sql).expect("prepare query plan");
        statement
            .query_map(params![text, integer], |row| row.get::<_, String>(3))
            .expect("explain query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    fn explain_without_parameters(conn: &rusqlite::Connection, sql: &str) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = conn.prepare(&explain_sql).expect("prepare query plan");
        statement
            .query_map([], |row| row.get::<_, String>(3))
            .expect("explain query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    fn explain_with_values(
        conn: &rusqlite::Connection,
        sql: &str,
        values: &[rusqlite::types::Value],
    ) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = conn.prepare(&explain_sql).expect("prepare query plan");
        statement
            .query_map(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, String>(3)
            })
            .expect("explain query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    struct TestDbPath {
        _directory: tempfile::TempDir,
        path: std::path::PathBuf,
    }

    impl std::ops::Deref for TestDbPath {
        type Target = std::path::Path;

        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl AsRef<std::path::Path> for TestDbPath {
        fn as_ref(&self) -> &std::path::Path {
            &self.path
        }
    }

    fn temp_db(tag: &str) -> TestDbPath {
        let directory = tempfile::Builder::new()
            .prefix(&format!("arcana-runs-test-{tag}-"))
            .tempdir()
            .expect("create unique temporary runs directory");
        let path = directory.path().join("runs.db");
        TestDbPath {
            _directory: directory,
            path,
        }
    }

    #[test]
    fn production_run_chronology_reads_mixed_timestamp_formats_by_instant() {
        let path = temp_db("mixed-chronology");
        let db = RunsDb::open_at(&path).expect("isolated migrated RunsDb");
        let later = db.begin(kind::APP, None).expect("later run writer");
        let earlier = db.begin(kind::CLI, None).expect("earlier run writer");
        for (id, timestamp) in [
            (&later, "2026-10-05 12:00:00"),
            (&earlier, "2026-10-05T09:00:00.000Z"),
        ] {
            db.conn
                .execute(
                    "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                    params![timestamp, id],
                )
                .expect("persist legacy/canonical fixture time");
        }
        let recent = db.recent(2).expect("production recent reader");
        let open = db.open_runs().expect("production open reader");
        assert_eq!(
            (
                recent.iter().map(|run| &run.id).collect::<Vec<_>>(),
                open.iter().map(|run| &run.id).collect::<Vec<_>>()
            ),
            (vec![&later, &earlier], vec![&earlier, &later]),
        );

        db.conn
            .execute(
                "UPDATE runs SET started_at = '2026-10-05T09:00:00.000Z' WHERE id = ?1",
                [&later],
            )
            .expect("persist equal-instant fixture time");
        let tied = db.recent(2).expect("stable tied run ordering");
        assert_eq!(
            tied.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
            vec![later.as_str(), earlier.as_str()]
        );
    }

    #[test]
    fn production_worker_run_readers_select_latest_instant_across_legacy_rows() {
        let path = temp_db("mixed-worker-chronology");
        let db = RunsDb::open_at(&path).expect("isolated migrated RunsDb");
        let later = db
            .begin_worker_tick("worker-backend", "later-session")
            .expect("first worker writer");
        db.finish_worker_tick(&later, "completed", outcome::OK, None)
            .expect("close first run atomically");
        let earlier = db
            .begin_worker_tick("worker-backend", "earlier-session")
            .expect("second worker writer");
        for (id, timestamp) in [
            (&later, "2026-10-05 12:00:00"),
            (&earlier, "2026-10-05T09:00:00.000Z"),
        ] {
            db.conn
                .execute(
                    "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                    params![timestamp, id],
                )
                .expect("persist legacy/canonical worker time");
        }
        let latest = db
            .latest_worker_tick("worker-backend")
            .expect("latest tick reader")
            .expect("tick exists");
        let owner = db
            .latest_worker_run_owner("worker-backend")
            .expect("latest owner reader")
            .expect("owner exists");
        let roster = db.latest_worker_ticks(10).expect("latest roster reader");
        assert_eq!(
            (latest.run_id, owner.run_id, roster[0].run_id.clone()),
            (later.clone(), later.clone(), later)
        );
        db.finish_worker_tick(&earlier, "completed", outcome::OK, None)
            .expect("finish fixture run");
    }

    #[test]
    fn runs_v7_upgrade_preserves_rows_and_indexes_instant_order_readers() {
        let path = temp_db("runs-v8-index-migration");
        let db = RunsDb::open_at(&path).expect("open isolated RunsDb");
        let mut expected = Vec::new();
        for index in 0..128 {
            let id = db
                .begin(kind::APP, Some("migration-preservation"))
                .expect("create pre-upgrade run");
            let timestamp = if index % 2 == 0 {
                format!("2026-10-05 {:02}:00:00", index % 24)
            } else {
                format!("2026-10-05T{:02}:00:00.000Z", index % 24)
            };
            db.conn
                .execute(
                    "UPDATE runs SET started_at = ?1 WHERE id = ?2",
                    params![timestamp, id],
                )
                .expect("persist mixed-format migration fixture");
            expected.push((id, timestamp));
        }
        db.conn
            .execute_batch(
                "DROP INDEX idx_runs_started_instant;
                 DROP INDEX idx_runs_open_instant;
                 PRAGMA user_version = 7;",
            )
            .expect("simulate a populated v7 database");
        drop(db);

        let upgraded = RunsDb::open_at(&path).expect("upgrade populated v7 RunsDb");
        assert_eq!(upgraded.schema_version().expect("schema version"), 8);
        let rows = upgraded
            .conn
            .prepare("SELECT id, started_at FROM runs ORDER BY id")
            .expect("prepare preservation read")
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("read preserved rows")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("collect preserved rows");
        let mut expected_by_id = expected;
        expected_by_id.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(rows, expected_by_id);

        let recent_plan = explain_without_parameters(
            &upgraded.conn,
            "SELECT id FROM runs ORDER BY julianday(started_at) DESC, id DESC LIMIT 10",
        );
        assert!(
            recent_plan
                .iter()
                .any(|step| step.contains("idx_runs_started_instant")),
            "recent query should use its instant-order index: {recent_plan:?}"
        );
        let open_plan = explain_without_parameters(
            &upgraded.conn,
            "SELECT id FROM runs INDEXED BY idx_runs_open_instant WHERE outcome IS NULL
             ORDER BY julianday(started_at) ASC, id ASC",
        );
        assert!(
            open_plan
                .iter()
                .any(|step| step.contains("idx_runs_open_instant")),
            "open-run query should use its partial instant-order index: {open_plan:?}"
        );

        let recent = upgraded.recent(10).expect("production recent reader");
        let open = upgraded.open_runs().expect("production open reader");
        assert_eq!(recent.len(), 10);
        assert_eq!(open.len(), 128);
        assert_eq!(
            recent[0].id,
            expected_by_id
                .iter()
                .filter(|(_, raw)| { raw.ends_with("23:00:00") || raw.ends_with("23:00:00.000Z") })
                .map(|(id, _)| id)
                .max()
                .expect("latest fixture row")
                .clone()
        );
    }

    #[test]
    fn a_run_is_recorded_and_read_back() {
        let path = temp_db("roundtrip");
        let db = RunsDb::open_at(&path).expect("open");
        assert_eq!(db.schema_version().unwrap(), 8);

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
    fn operation_journal_pages_keep_order_and_surface_unknown_outcomes() {
        let path = temp_db("journal-cursor");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let run_id = db.begin(kind::AGENT_TURN, None).expect("begin run");
        let targets = vec!["src/main.rs".to_string()];
        let first = db
            .record_operation_intent(&run_id, Some(23), "op-1", "edit", "write_file", &targets)
            .expect("append first intent");
        let second = db
            .record_operation_intent(&run_id, None, "op-2", "verify", "command", &[])
            .expect("append second intent");

        let page = db
            .operation_journal_page(&run_id, 0, 1)
            .expect("first page");
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].cursor, first);
        assert_eq!(page.entries[0].task_id, Some(23));
        assert_eq!(page.entries[0].effect, "write_file");
        assert_eq!(page.entries[0].target_paths, targets);
        assert_eq!(page.entries[0].outcome, OperationOutcome::Unknown);
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::Unresolved
        );
        assert!(page.has_more);

        db.record_operation_outcome(&run_id, first, OperationOutcome::Unknown, &[])
            .expect("append outcome");
        db.record_operation_reconciliation(&run_id, first, OperationReconciliation::Reconciled)
            .expect("append reconciliation");
        let updated = db
            .operation_journal_page(&run_id, 0, 1)
            .expect("read updated page");
        assert_eq!(updated.entries[0].cursor, first);
        assert_eq!(updated.entries[0].outcome, OperationOutcome::Unknown);
        assert!(updated.entries[0].changed_paths.is_empty());
        assert_eq!(
            updated.entries[0].reconciliation,
            OperationReconciliation::Reconciled
        );

        let next = db
            .operation_journal_page(&run_id, page.next_cursor, 10)
            .expect("read next page");
        assert_eq!(next.entries[0].cursor, second);
        assert_eq!(next.next_cursor, second);
        assert!(!next.has_more);
    }

    #[test]
    fn active_worker_operation_reads_canonical_and_legacy_timestamps() {
        let path = temp_db("active-operation-timestamp");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let run_id = db
            .begin_worker_tick("timestamp-worker", "timestamp-session")
            .expect("begin worker tick");
        db.update_worker_tick_phase(&run_id, "running", Some(1), None)
            .expect("mark worker run active");
        let cursor = db
            .record_operation_intent(
                &run_id,
                Some(1),
                "timestamp-operation",
                "write",
                "write_file",
                &["src/main.rs".to_string()],
            )
            .expect("record active operation");

        let canonical: String = db
            .conn
            .query_row(
                "SELECT created_at FROM operation_journal WHERE id = ?1",
                [cursor],
                |row| row.get(0),
            )
            .expect("read canonical operation timestamp");
        let canonical_read = db
            .active_worker_operation(&run_id)
            .expect("read canonical active operation")
            .expect("operation remains active");
        assert_eq!(canonical_read.started_at, canonical);

        db.record_operation_outcome(&run_id, cursor, OperationOutcome::Ok, &[])
            .expect("finish canonical operation");

        // Legacy runs.db rows may have been written with SQLite's offset-less
        // datetime form. Seed an append-only historical row directly rather
        // than updating the production journal, whose trigger correctly
        // prohibits mutation.
        let legacy = "2026-09-01 06:23:58";
        db.conn
            .execute(
                "INSERT INTO operation_journal
                 (run_id, task_id, record_type, operation_id, operation, effect, target_paths, created_at)
                 VALUES (?1, 1, 'intent', 'legacy-operation', 'legacy-write', 'write_file', '[]', ?2)",
                params![run_id, legacy],
            )
            .expect("seed legacy timestamp");
        let legacy_read = db
            .active_worker_operation(&run_id)
            .expect("read legacy active operation")
            .expect("legacy operation remains active");
        assert_eq!(legacy_read.started_at, legacy);
        assert!(legacy_read.elapsed_ms > 0);
    }

    #[test]
    fn operation_journal_query_plans_use_populated_indexes_and_preserve_guards() {
        const RECORD_INTENT_UNRESOLVED_SQL: &str = "SELECT i.id, i.operation
             FROM operation_journal i
             WHERE i.task_id = ?1
               AND i.record_type = 'intent'
               AND i.effect <> 'Board'
               AND COALESCE((
                   SELECT o.outcome FROM operation_journal o
                   WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                   ORDER BY o.id DESC LIMIT 1
               ), 'unknown') = 'unknown'
               AND COALESCE((
                   SELECT r.reconciliation_state FROM operation_journal r
                   WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
                   ORDER BY r.id DESC LIMIT 1
               ), 'unresolved') IN ('unresolved', 'do_not_retry')
             ORDER BY i.id DESC LIMIT 1";
        const RECORD_OUTCOME_EXISTS_SQL: &str = "SELECT EXISTS(SELECT 1 FROM operation_journal
             WHERE id = ?1 AND run_id = ?2 AND record_type = 'intent')";
        const RECORD_OUTCOME_INSERT_SQL: &str = "INSERT INTO operation_journal
             (run_id, task_id, record_type, parent_id, outcome, changed_paths, created_at)
             SELECT run_id, task_id, 'outcome', id, ?1, ?2, ?3
             FROM operation_journal WHERE id = ?4 AND record_type = 'intent'";
        const RECONCILE_OPERATION_SQL: &str = "INSERT INTO operation_journal
             (run_id, task_id, record_type, parent_id, reconciliation_state,
              reconciliation_actor, reconciliation_reason, created_at)
             SELECT run_id, task_id, 'reconciliation', id, ?1, ?2, ?3, ?4
             FROM operation_journal
             WHERE id = ?5 AND run_id = ?6 AND record_type = 'intent'
               AND effect != 'Board'
               AND COALESCE((SELECT outcome FROM operation_journal outcome
                             WHERE outcome.parent_id = operation_journal.id
                               AND outcome.record_type = 'outcome'
                             ORDER BY outcome.id DESC LIMIT 1), 'unknown') = 'unknown'
               AND NOT EXISTS (SELECT 1 FROM operation_journal prior
                               WHERE prior.parent_id = operation_journal.id
                                 AND prior.record_type = 'reconciliation')";
        const UNRESOLVED_TASK_SIDE_EFFECT_SQL: &str = "SELECT i.run_id, i.operation, i.id
             FROM operation_journal i
             WHERE i.task_id = ?1
               AND i.record_type = 'intent'
               AND i.effect <> 'Board'
               AND COALESCE((
                   SELECT o.outcome FROM operation_journal o
                   WHERE o.parent_id = i.id AND o.record_type = 'outcome'
                   ORDER BY o.id DESC LIMIT 1
               ), 'unknown') = 'unknown'
               AND COALESCE((
                   SELECT r.reconciliation_state FROM operation_journal r
                   WHERE r.parent_id = i.id AND r.record_type = 'reconciliation'
                   ORDER BY r.id DESC LIMIT 1
               ), 'unresolved') IN ('unresolved', 'do_not_retry')
             ORDER BY i.id DESC LIMIT 1";
        const JOURNAL_LINK_GUARD_SQL: &str = "SELECT 1 FROM operation_journal intent
             WHERE intent.id = ?1
               AND intent.record_type = 'intent'
               AND intent.run_id = ?2
               AND intent.task_id IS ?3";
        const SUPERVISOR_RECONCILIATION_GUARD_SQL: &str = "SELECT 1 FROM operation_journal intent
             WHERE intent.id = ?1
               AND intent.record_type = 'intent'
               AND intent.effect != 'Board'
               AND intent.run_id = ?2
               AND intent.task_id IS ?3
               AND COALESCE((SELECT outcome FROM operation_journal outcome
                             WHERE outcome.parent_id = intent.id
                               AND outcome.record_type = 'outcome'
                             ORDER BY outcome.id DESC LIMIT 1), 'unknown') = 'unknown'";

        fn assert_plan_uses(plan: &[String], index_name: &str) {
            assert!(
                plan.iter().any(|step| step.contains(index_name)),
                "expected query plan to use {index_name}; got {plan:#?}"
            );
        }

        fn assert_no_operation_journal_scan(plan: &[String]) {
            assert!(
                !plan.iter().any(|step| {
                    let step = step.to_ascii_lowercase();
                    step.contains("scan operation_journal") || step.contains("scan i ")
                }),
                "unexpected operation_journal scan; got {plan:#?}"
            );
        }

        let path = temp_db("journal-query-plans");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let run_id = db
            .begin_worker_tick("plan-worker", "journal-plan-session")
            .expect("begin worker tick");
        db.update_worker_tick_phase(&run_id, "running", Some(1), None)
            .expect("mark worker run active");

        // Populate the production journal shape so SQLite has meaningful row
        // counts and statistics for its choices. Each historical intent has
        // an outcome; one later task effect remains unknown for recovery.
        for index in 0..180 {
            let task_id = 100 + index;
            let cursor = db
                .record_operation_intent(
                    &run_id,
                    Some(task_id),
                    &format!("plan-operation-{index}"),
                    "edit",
                    "write_file",
                    &[format!("src/file-{index}.rs")],
                )
                .expect("append populated intent");
            db.record_operation_outcome(&run_id, cursor, OperationOutcome::Ok, &[])
                .expect("append populated outcome");
        }
        let unresolved_task_id = 7777;
        let unresolved_cursor = db
            .record_operation_intent(
                &run_id,
                Some(unresolved_task_id),
                "unknown-plan-operation",
                "write_file",
                "Write",
                &["src/uncertain.rs".to_string()],
            )
            .expect("append unresolved task effect");
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let page_plan = explain_with_values(
            &db.conn,
            OPERATION_JOURNAL_PAGE_SQL,
            &[
                rusqlite::types::Value::Text(run_id.clone()),
                rusqlite::types::Value::Integer(0),
                rusqlite::types::Value::Integer(51),
            ],
        );
        assert_plan_uses(&page_plan, "idx_operation_journal_run_cursor");
        assert_plan_uses(&page_plan, "idx_operation_journal_one_outcome");
        assert_plan_uses(&page_plan, "idx_operation_journal_one_reconciliation");
        assert_no_operation_journal_scan(&page_plan);

        let active_plan =
            explain_with_text_parameter(&db.conn, ACTIVE_WORKER_OPERATION_SQL, &run_id);
        assert_plan_uses(&active_plan, "idx_operation_journal_run_cursor");
        assert_plan_uses(&active_plan, "idx_operation_journal_one_outcome");
        assert_plan_uses(&active_plan, "idx_operation_journal_one_reconciliation");
        assert_no_operation_journal_scan(&active_plan);

        let task_value = rusqlite::types::Value::Integer(unresolved_task_id);
        let intent_guard_plan = explain_with_values(
            &db.conn,
            RECORD_INTENT_UNRESOLVED_SQL,
            std::slice::from_ref(&task_value),
        );
        assert_plan_uses(&intent_guard_plan, "idx_operation_journal_task_cursor");
        assert_plan_uses(&intent_guard_plan, "idx_operation_journal_one_outcome");
        assert_plan_uses(
            &intent_guard_plan,
            "idx_operation_journal_one_reconciliation",
        );
        assert_no_operation_journal_scan(&intent_guard_plan);

        let outcome_exists_plan = explain_with_values(
            &db.conn,
            RECORD_OUTCOME_EXISTS_SQL,
            &[
                rusqlite::types::Value::Integer(unresolved_cursor),
                rusqlite::types::Value::Text(run_id.clone()),
            ],
        );
        assert!(
            outcome_exists_plan
                .iter()
                .any(|step| step.contains("INTEGER PRIMARY KEY")),
            "outcome existence guard should use the intent rowid; got {outcome_exists_plan:#?}"
        );
        assert_no_operation_journal_scan(&outcome_exists_plan);

        let outcome_insert_plan = explain_with_values(
            &db.conn,
            RECORD_OUTCOME_INSERT_SQL,
            &[
                rusqlite::types::Value::Text("unknown".to_string()),
                rusqlite::types::Value::Text("[]".to_string()),
                rusqlite::types::Value::Text(now_rfc3339()),
                rusqlite::types::Value::Integer(unresolved_cursor),
            ],
        );
        assert!(
            outcome_insert_plan
                .iter()
                .any(|step| step.contains("INTEGER PRIMARY KEY")),
            "outcome append source should use the intent rowid; got {outcome_insert_plan:#?}"
        );
        // The full INSERT plan can include trigger subprogram work. The exact
        // link predicate is asserted separately below because SQLite's plan
        // output does not attribute those steps to the trigger body.

        let reconcile_values = [
            rusqlite::types::Value::Text("retry_safe".to_string()),
            rusqlite::types::Value::Text("supervisor".to_string()),
            rusqlite::types::Value::Text("idempotent_retry_verified".to_string()),
            rusqlite::types::Value::Text(now_rfc3339()),
            rusqlite::types::Value::Integer(unresolved_cursor),
            rusqlite::types::Value::Text(run_id.clone()),
        ];
        let reconcile_plan =
            explain_with_values(&db.conn, RECONCILE_OPERATION_SQL, &reconcile_values);
        assert!(
            reconcile_plan
                .iter()
                .any(|step| step.contains("INTEGER PRIMARY KEY")),
            "reconciliation candidate should use the intent rowid; got {reconcile_plan:#?}"
        );
        assert_plan_uses(&reconcile_plan, "idx_operation_journal_one_outcome");
        assert_plan_uses(&reconcile_plan, "idx_operation_journal_one_reconciliation");
        // The outer INSERT includes SQLite trigger subprograms in its plan;
        // the migration 6 inner SELECT is checked explicitly below.

        let unresolved_plan = explain_with_values(
            &db.conn,
            UNRESOLVED_TASK_SIDE_EFFECT_SQL,
            std::slice::from_ref(&task_value),
        );
        assert_plan_uses(&unresolved_plan, "idx_operation_journal_task_cursor");
        assert_plan_uses(&unresolved_plan, "idx_operation_journal_one_outcome");
        assert_plan_uses(&unresolved_plan, "idx_operation_journal_one_reconciliation");
        assert_no_operation_journal_scan(&unresolved_plan);

        let link_guard_plan = explain_with_values(
            &db.conn,
            JOURNAL_LINK_GUARD_SQL,
            &[
                rusqlite::types::Value::Integer(unresolved_cursor),
                rusqlite::types::Value::Text(run_id.clone()),
                rusqlite::types::Value::Integer(unresolved_task_id),
            ],
        );
        assert!(
            link_guard_plan
                .iter()
                .any(|step| step.contains("INTEGER PRIMARY KEY")),
            "migration 4 link guard should use the parent intent rowid; got {link_guard_plan:#?}"
        );
        assert_no_operation_journal_scan(&link_guard_plan);

        let supervisor_guard_plan = explain_with_values(
            &db.conn,
            SUPERVISOR_RECONCILIATION_GUARD_SQL,
            &[
                rusqlite::types::Value::Integer(unresolved_cursor),
                rusqlite::types::Value::Text(run_id.clone()),
                rusqlite::types::Value::Integer(unresolved_task_id),
            ],
        );
        assert!(
            supervisor_guard_plan.iter().any(|step| step.contains("INTEGER PRIMARY KEY")),
            "migration 6 reconciliation guard should use the parent intent rowid; got {supervisor_guard_plan:#?}"
        );
        assert_plan_uses(&supervisor_guard_plan, "idx_operation_journal_one_outcome");
        assert_no_operation_journal_scan(&supervisor_guard_plan);

        // SQLite EXPLAIN QUERY PLAN omits trigger subprograms, so the exact
        // inner predicates above are explained directly and trigger behavior
        // is separately exercised through valid and invalid production paths.
        assert!(
            db.conn
                .execute(
                    "INSERT INTO operation_journal
                    (run_id, task_id, record_type, parent_id, outcome, changed_paths, created_at)
                 VALUES (?1, ?2, 'outcome', ?3, 'ok', '[]', ?4)",
                    params![
                        &run_id,
                        unresolved_task_id + 1,
                        unresolved_cursor,
                        now_rfc3339()
                    ],
                )
                .is_err(),
            "migration 4 link guard rejects a mismatched intent task"
        );
        assert!(
            db.conn
                .execute(
                    "INSERT INTO operation_journal
                    (run_id, task_id, record_type, parent_id, outcome, changed_paths, created_at)
                 VALUES (?1, ?2, 'outcome', ?3, 'unknown', '[]', ?4)",
                    params![
                        &run_id,
                        unresolved_task_id,
                        unresolved_cursor,
                        now_rfc3339()
                    ],
                )
                .is_ok(),
            "matching outcome link is valid"
        );
        let outcome_cursor = db
            .record_operation_intent(
                &run_id,
                Some(unresolved_task_id + 2),
                "second-unknown-operation",
                "write_file",
                "Write",
                &[],
            )
            .expect("second independent unresolved intent");
        assert!(
            db.conn
                .execute(
                    "INSERT INTO operation_journal
                    (run_id, task_id, record_type, parent_id, reconciliation_state,
                     reconciliation_actor, reconciliation_reason, created_at)
                 VALUES (?1, ?2, 'reconciliation', ?3, 'retry_safe',
                         'supervisor', 'idempotent_retry_verified', ?4)",
                    params![
                        &run_id,
                        unresolved_task_id + 2,
                        outcome_cursor,
                        now_rfc3339()
                    ],
                )
                .is_err(),
            "migration 6 reconciliation guard rejects an unauthorized direct insert"
        );

        assert_eq!(
            db.active_worker_operation(&run_id)
                .expect("read active worker operation")
                .expect("unresolved operation is active")
                .operation,
            "write_file"
        );
        let unresolved = db
            .unresolved_task_side_effect(unresolved_task_id)
            .expect("read unresolved task effect")
            .expect("unknown effect blocks task retry");
        assert_eq!(
            unresolved,
            (run_id.clone(), "write_file".to_string(), unresolved_cursor)
        );
        assert!(matches!(
            db.record_operation_intent(
                &run_id,
                Some(unresolved_task_id),
                "blocked-retry",
                "write_file",
                "Write",
                &[],
            ),
            Err(ArcanaError::OperationReconciliationRequired { intent_cursor, .. })
                if intent_cursor == unresolved_cursor
        ));
        assert!(db
            .reconcile_operation_as_supervisor(
                "worker-not-supervisor",
                &run_id,
                unresolved_cursor,
                OperationReconciliation::RetrySafe,
                "idempotent_retry_verified",
            )
            .is_err());
        assert!(db
            .reconcile_operation_as_supervisor(
                "supervisor",
                &run_id,
                unresolved_cursor,
                OperationReconciliation::RetrySafe,
                "unsafe_to_repeat",
            )
            .is_err());
        db.reconcile_operation_as_supervisor(
            "supervisor",
            &run_id,
            unresolved_cursor,
            OperationReconciliation::RetrySafe,
            "idempotent_retry_verified",
        )
        .expect("authorized Supervisor reconciliation");
        assert_eq!(
            db.unresolved_task_side_effect(unresolved_task_id)
                .expect("read reconciled task effect"),
            None
        );
        db.record_operation_intent(
            &run_id,
            Some(unresolved_task_id),
            "retry-after-reconciliation",
            "write_file",
            "Write",
            &[],
        )
        .expect("retry-safe reconciliation permits another intent");

        let page = db
            .operation_journal_page(&run_id, unresolved_cursor - 1, 1)
            .expect("read populated journal page");
        let entry = page
            .entries
            .iter()
            .find(|entry| entry.cursor == unresolved_cursor)
            .expect("read back target intent");
        assert_eq!(entry.outcome, OperationOutcome::Unknown);
        assert_eq!(entry.reconciliation, OperationReconciliation::RetrySafe);
        assert_eq!(entry.reconciliation_actor.as_deref(), Some("supervisor"));
        assert_eq!(
            entry.reconciliation_reason.as_deref(),
            Some("idempotent_retry_verified")
        );
    }

    #[test]
    fn operation_journal_v7_upgrade_and_append_only_guards() {
        let path = temp_db("journal-migration");
        {
            let conn = crate::db::open_tuned(&path).expect("create legacy runs database");
            conn.execute_batch(
                "CREATE TABLE runs (
                    id TEXT PRIMARY KEY, kind TEXT NOT NULL, actor TEXT, started_at TEXT NOT NULL,
                    ended_at TEXT, outcome TEXT, host TEXT NOT NULL, pid INTEGER NOT NULL,
                    parent_run_id TEXT REFERENCES runs(id), detail TEXT
                 );
                 CREATE TABLE worker_run_events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    run_id TEXT NOT NULL REFERENCES runs(id), task_id INTEGER,
                    event_at TEXT NOT NULL, operation TEXT NOT NULL, outcome TEXT NOT NULL
                 );
                 PRAGMA user_version = 3;",
            )
            .expect("write v3 schema");
        }
        let db = RunsDb::open_at(&path).expect("upgrade v3 database");
        assert_eq!(db.schema_version().expect("schema version"), 8);
        let run_id = db.begin(kind::AGENT_TURN, None).expect("begin run");
        let cursor = db
            .record_operation_intent(&run_id, None, "op-1", "edit", "write_file", &[])
            .expect("append intent");
        assert!(db
            .conn
            .execute(
                "UPDATE operation_journal SET operation = 'changed' WHERE id = ?1",
                params![cursor],
            )
            .is_err());
        assert!(db
            .conn
            .execute(
                "DELETE FROM operation_journal WHERE id = ?1",
                params![cursor],
            )
            .is_err());
        assert_eq!(
            db.operation_journal_page(&run_id, 0, 10)
                .expect("read persisted intent")
                .entries
                .len(),
            1
        );

        let indexed_task_intent = db
            .record_operation_intent(
                &run_id,
                Some(42),
                "task-index-migration-proof",
                "edit",
                "write_file",
                &[],
            )
            .expect("append task-scoped intent");
        db.conn
            .execute_batch(
                "DROP INDEX idx_operation_journal_task_cursor;
                 PRAGMA user_version = 6;",
            )
            .expect("simulate existing v6 database");
        drop(db);

        let db = RunsDb::open_at(&path).expect("upgrade existing v6 database");
        assert_eq!(db.schema_version().expect("upgraded schema version"), 8);
        let plan = explain_without_parameters(
            &db.conn,
            "SELECT id FROM operation_journal
             WHERE record_type = 'intent' AND task_id = 42
             ORDER BY id DESC LIMIT 1",
        );
        assert!(
            plan.iter()
                .any(|step| step.contains("idx_operation_journal_task_cursor")),
            "upgraded task-intent lookup should use the migration index; plan: {plan:?}"
        );
        assert_eq!(
            db.operation_journal_page(&run_id, indexed_task_intent - 1, 1)
                .expect("read task-scoped intent after upgrade")
                .entries
                .first()
                .expect("task intent remains readable")
                .cursor,
            indexed_task_intent
        );
    }

    #[test]
    fn unresolved_task_effect_blocks_new_intents_until_reconciled() {
        let path = temp_db("journal-task-barrier");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let first_run = db
            .begin(kind::AGENT_TURN, None)
            .expect("begin interrupted run");
        let next_run = db
            .begin(kind::AGENT_TURN, None)
            .expect("begin replacement run");
        let first = db
            .record_operation_intent(
                &first_run,
                Some(23),
                "uncertain-write",
                "write_file",
                "Write",
                &["src/calendar.rs".into()],
            )
            .expect("persist interrupted side-effect intent");

        let blocked = db
            .record_operation_intent(
                &next_run,
                Some(23),
                "replacement-write",
                "write_file",
                "Write",
                &["src/calendar.rs".into()],
            )
            .expect_err("a second run must not begin a side effect for the same unresolved task");
        assert!(matches!(
            blocked,
            ArcanaError::OperationReconciliationRequired {
                operation,
                intent_cursor
            } if operation == "write_file" && intent_cursor == first
        ));

        db.record_operation_reconciliation(&first_run, first, OperationReconciliation::RetrySafe)
            .expect("append explicit retry-safe decision");
        db.record_operation_intent(
            &next_run,
            Some(23),
            "replacement-write-after-review",
            "write_file",
            "Write",
            &["src/calendar.rs".into()],
        )
        .expect("a reconciled retry-safe operation may proceed");
    }

    #[test]
    fn operation_reconciliation_has_one_durable_winner() {
        let path = temp_db("journal-reconciliation-cas");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let run_id = db.begin(kind::AGENT_TURN, None).expect("begin run");
        let intent = db
            .record_operation_intent(
                &run_id,
                Some(23),
                "one-decision",
                "write_file",
                "Write",
                &[],
            )
            .expect("append intent");
        db.record_operation_reconciliation(&run_id, intent, OperationReconciliation::RetrySafe)
            .expect("first reconciliation decision wins");

        assert!(
            db.record_operation_reconciliation(
                &run_id,
                intent,
                OperationReconciliation::DoNotRetry
            )
            .is_err(),
            "a conflicting second decision must not be appended"
        );
        let page = db
            .operation_journal_page(&run_id, 0, 10)
            .expect("read immutable journal");
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::RetrySafe
        );
    }

    #[test]
    fn concurrent_task_intents_allow_at_most_one_unresolved_side_effect() {
        let path = temp_db("journal-task-barrier-race");
        let setup = RunsDb::open_at(&path).expect("open isolated runs database");
        let first_run = setup
            .begin(kind::AGENT_TURN, None)
            .expect("begin first run");
        let second_run = setup
            .begin(kind::AGENT_TURN, None)
            .expect("begin second run");
        drop(setup);

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let first_path = path.path.clone();
        let first_barrier = std::sync::Arc::clone(&barrier);
        let first = std::thread::spawn(move || {
            let db = RunsDb::open_at(&first_path).expect("open first connection");
            first_barrier.wait();
            db.record_operation_intent(
                &first_run,
                Some(42),
                "concurrent-first",
                "write_file",
                "Write",
                &[],
            )
        });
        let second_path = path.path.clone();
        let second_barrier = std::sync::Arc::clone(&barrier);
        let second = std::thread::spawn(move || {
            let db = RunsDb::open_at(&second_path).expect("open second connection");
            second_barrier.wait();
            db.record_operation_intent(
                &second_run,
                Some(42),
                "concurrent-second",
                "write_file",
                "Write",
                &[],
            )
        });
        let results = [
            first.join().expect("first writer finished"),
            second.join().expect("second writer finished"),
        ];
        assert_eq!(
            results.iter().filter(|result| result.is_ok()).count(),
            1,
            "only one concurrent task side effect may acquire an unresolved intent"
        );
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(ArcanaError::OperationReconciliationRequired { .. })
                ))
                .count(),
            1,
            "the loser sees the committed unresolved effect and receives a recovery barrier"
        );
    }

    #[test]
    fn operation_journal_rejects_unsafe_and_unbounded_paths_and_links_runs() {
        let path = temp_db("journal-input-bounds");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let run_id = db.begin(kind::AGENT_TURN, None).expect("begin run");
        assert!(db
            .record_operation_intent(&run_id, None, "../secret", "edit", "write_file", &[])
            .is_err());
        for unsafe_path in ["../secret", "C:/outside", "/outside", "dir\\file", "a//b"] {
            assert!(db
                .record_operation_intent(
                    &run_id,
                    None,
                    "unsafe-path",
                    "edit",
                    "write_file",
                    &[unsafe_path.to_string()],
                )
                .is_err());
        }
        let too_many = vec!["src/file".to_string(); 65];
        assert!(db
            .record_operation_intent(&run_id, None, "too-many", "edit", "write_file", &too_many)
            .is_err());
        let too_long = vec!["a".repeat(513)];
        assert!(db
            .record_operation_intent(&run_id, None, "too-long", "edit", "write_file", &too_long)
            .is_err());
        let too_much_encoded_data = vec!["a".repeat(512); 17];
        assert!(db
            .record_operation_intent(
                &run_id,
                None,
                "too-much-data",
                "edit",
                "write_file",
                &too_much_encoded_data,
            )
            .is_err());
        assert!(db
            .record_operation_intent(&run_id, Some(0), "bad-task", "edit", "write_file", &[])
            .is_err());
        for index in 0..51 {
            db.record_operation_intent(
                &run_id,
                None,
                &format!("page-{index}"),
                "validate",
                "read",
                &[],
            )
            .expect("append bounded page fixture");
        }
        assert!(db.operation_journal_page(&run_id, -1, 1).is_err());
        let capped_page = db
            .operation_journal_page(&run_id, 0, 500)
            .expect("page limit is capped");
        assert_eq!(capped_page.entries.len(), 50);
        assert!(capped_page.has_more);
    }

    #[test]
    fn run_ledger_lifecycle_and_recovery_emit_private_result_spans() {
        let path = temp_db("run-telemetry");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let run_id = db
                .begin_with(
                    "private-run-kind-marker",
                    Some("private-run-actor-marker"),
                    None,
                    Some("private-run-detail-marker"),
                )
                .expect("begin run");
            let fetched = db.get(&run_id).expect("read run");
            let missing = db
                .get("private-missing-run-id-marker")
                .expect("missing run");
            let recent = db.recent(10).expect("read recent runs");
            let open = db.open_runs().expect("read open runs");
            db.finish(&run_id, outcome::OK).expect("finish run");
            let duplicate_finish = db.finish(&run_id, outcome::FAILED).is_err();

            let worker_tick = db
                .begin_worker_tick("worker-run-telemetry", "session-run-telemetry")
                .expect("begin worker tick");
            let interrupted = db
                .interrupt_worker_ticks("worker-run-telemetry", "private-failure-code-marker")
                .expect("interrupt worker tick");

            let abandoned = db
                .begin(kind::CLI, Some("private-run-actor-marker"))
                .expect("begin run to reap");
            db.conn
                .execute(
                    "UPDATE runs SET pid = ?1 WHERE id = ?2",
                    params![u32::MAX as i64 - 1, &abandoned],
                )
                .expect("simulate a dead same-host process");
            let reaped = db.reap_abandoned().expect("reap dead process run");

            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_run_insert BEFORE INSERT ON runs
                     BEGIN SELECT RAISE(ABORT, 'private-run-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_begin = db
                .begin_with(
                    "private-failed-run-kind-marker",
                    None,
                    None,
                    Some("private-failed-run-detail-marker"),
                )
                .is_err();
            (
                run_id,
                fetched,
                missing,
                recent,
                open,
                duplicate_finish,
                worker_tick,
                interrupted,
                reaped,
                failed_begin,
            )
        });

        assert_eq!(
            observed.1.as_ref().map(|run| run.id.as_str()),
            Some(observed.0.as_str())
        );
        assert!(observed.2.is_none());
        assert_eq!(observed.3.len(), 1);
        assert_eq!(observed.4.len(), 1);
        assert!(observed.5);
        assert_eq!(observed.7, 1);
        assert_eq!(observed.8, 1);
        assert!(observed.9);

        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_run_begin",
            "database_run_read",
            "database_runs_recent_read",
            "database_runs_open_read",
            "database_run_finish",
            "database_worker_tick_begin",
            "database_worker_ticks_interrupt",
            "database_runs_reap_abandoned",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing run ledger span {operation}"
            );
        }
        let not_found = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_run_read"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("missing run read is distinguishable");
        assert_eq!(not_found["fields"]["result_count"], 0);
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_worker_tick_begin"
                && event["fields"]["session_id"] == "session-run-telemetry"
        }));
        let failed_begin = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_run_begin"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("run insert failure span");
        assert_eq!(failed_begin["fields"]["result_count_observed"], false);
        let serialized = serde_json::to_string(&events).expect("serialize run telemetry");
        for private_value in [
            "private-run-kind-marker",
            "private-run-actor-marker",
            "private-run-detail-marker",
            "private-missing-run-id-marker",
            "private-failure-code-marker",
            "private-run-trigger-marker",
            "private-failed-run-kind-marker",
            "private-failed-run-detail-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn worker_run_phase_heartbeat_progress_and_terminal_outcome_round_trip() {
        let path = temp_db("worker-roundtrip");
        let db = RunsDb::open_at(&path).expect("open");
        let run_id = db
            .begin_worker_tick("worker-backend", "recipe-worker-backend-session")
            .expect("begin worker tick");

        let starting = db
            .latest_worker_tick("worker-backend")
            .expect("read starting health")
            .expect("worker run exists");
        assert_eq!(starting.phase, "starting");
        assert_eq!(starting.session_id, "recipe-worker-backend-session");
        assert!(starting.last_progress_at.is_none());

        db.update_worker_tick_phase(&run_id, "retrying", None, None)
            .expect("record startup retry state");
        db.heartbeat_worker_tick(&run_id)
            .expect("record heartbeat separately");
        db.record_worker_progress(&run_id, Some(42), "tool_batch_completed")
            .expect("record task-attributed progress");
        let running = db
            .latest_worker_tick("worker-backend")
            .expect("read running health")
            .expect("worker run exists");
        assert_eq!(running.phase, "running");
        assert_eq!(running.task_id, Some(42));
        assert_eq!(
            running.last_progress.as_deref(),
            Some("tool_batch_completed")
        );
        assert!(running.last_progress_at.is_some());
        assert!(running.heartbeat_at >= starting.heartbeat_at);
        let events = db
            .recent_worker_progress(&run_id, 10)
            .expect("read progress history");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "tool_batch_completed");
        assert_eq!(events[0].2, "ok");

        db.finish_worker_tick(&run_id, "completed", outcome::OK, None)
            .expect("finish worker tick");
        let finished = db
            .latest_worker_tick("worker-backend")
            .expect("read completed health")
            .expect("worker run exists");
        assert_eq!(finished.phase, "completed");
        assert_eq!(finished.outcome.as_deref(), Some(outcome::OK));
        assert!(db.get(&run_id).unwrap().unwrap().ended_at.is_some());

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn worker_health_readers_emit_safe_result_spans() {
        let path = temp_db("health-reader-telemetry");
        let db = RunsDb::open_at(&path).expect("open isolated runs DB");
        db.begin_worker_tick("worker-backend", "private-session")
            .expect("begin tick for health readers");

        let ((latest, owner, ticks), events) =
            crate::observability::timing::capture_json_events(|| {
                (
                    db.latest_worker_tick("worker-backend"),
                    db.latest_worker_run_owner("worker-backend"),
                    db.latest_worker_ticks(10),
                )
            });
        assert!(latest.expect("read latest state").is_some());
        assert!(owner.expect("read latest owner").is_some());
        assert_eq!(ticks.expect("read latest ticks").len(), 1);

        let expected = [
            ("database_worker_tick_latest_read", Some("worker-backend")),
            (
                "database_worker_run_owner_latest_read",
                Some("worker-backend"),
            ),
            ("database_worker_ticks_latest_read", None),
        ];
        for (operation, worker) in expected {
            let event = events
                .iter()
                .find(|event| event["fields"]["operation"] == operation)
                .unwrap_or_else(|| panic!("missing operation event {operation}"));
            assert_eq!(event["fields"]["database"], "runs");
            assert_eq!(event["fields"]["outcome"], "success");
            assert!(event["fields"]["duration_ns"].as_u64().is_some());
            if let Some(worker) = worker {
                assert_eq!(event["fields"]["worker"], worker);
                assert_eq!(event["fields"]["result_count"], 1);
            }
        }
        let serialized = serde_json::to_string(&events).expect("serialize events");
        assert!(!serialized.contains("private-session"));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn worker_tick_lifecycle_emits_safe_result_events_with_run_correlation() {
        let path = temp_db("telemetry-lifecycle");
        let db = RunsDb::open_at(&path).expect("open");
        let (run_id, events) = crate::observability::timing::capture_json_events(|| {
            db.begin_worker_tick("worker-backend", "session-telemetry")
                .expect("begin worker tick")
        });
        let begin = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_tick_begin")
            .expect("begin event");
        assert_eq!(begin["fields"]["database"], "runs");
        assert_eq!(begin["fields"]["worker"], "worker-backend");
        assert_eq!(begin["fields"]["session_id"], "session-telemetry");
        assert_eq!(begin["fields"]["run_id"], run_id);
        assert_eq!(begin["fields"]["outcome"], "success");
        assert_eq!(begin["fields"]["result_count"], 1);

        let (phase_updated, events) = crate::observability::timing::capture_json_events(|| {
            db.update_worker_tick_phase(&run_id, "running", Some(42), None)
        });
        phase_updated.expect("update worker tick phase");
        let phase = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_tick_phase_update")
            .expect("phase event");
        assert_eq!(phase["fields"]["run_id"], run_id);
        assert_eq!(phase["fields"]["task_id"], "42");
        assert_eq!(phase["fields"]["outcome"], "success");

        let (progress_recorded, events) = crate::observability::timing::capture_json_events(|| {
            db.record_worker_progress(&run_id, Some(42), "tool_batch_completed")
        });
        progress_recorded.expect("record worker progress");
        let progress = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_progress_record")
            .expect("progress event");
        assert_eq!(progress["fields"]["run_id"], run_id);
        assert_eq!(progress["fields"]["task_id"], "42");
        assert_eq!(progress["fields"]["outcome"], "success");

        let (recent, events) = crate::observability::timing::capture_json_events(|| {
            db.recent_worker_progress(&run_id, 10)
        });
        assert_eq!(recent.expect("read worker progress").len(), 1);
        let read = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_progress_recent_read")
            .expect("progress-read event");
        assert_eq!(read["fields"]["run_id"], run_id);
        assert_eq!(read["fields"]["result_count"], 1);

        let (finished, events) = crate::observability::timing::capture_json_events(|| {
            db.finish_worker_tick(&run_id, "completed", "ok", None)
        });
        finished.expect("finish worker tick");
        let finish = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_tick_finish")
            .expect("finish event");
        assert_eq!(finish["fields"]["database"], "runs");
        assert_eq!(finish["fields"]["run_id"], run_id);
        assert_eq!(finish["fields"]["outcome"], "success");
        assert_eq!(finish["fields"]["result_count"], 1);

        let (_, events) = crate::observability::timing::capture_json_events(|| {
            db.finish_worker_tick("missing-run", "completed", "ok", None)
        });
        let failed = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_tick_finish")
            .expect("failed finish event");
        assert_eq!(failed["fields"]["run_id"], "missing-run");
        assert_eq!(failed["fields"]["outcome"], "error");

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn database_rejects_a_second_active_run_for_the_same_worker() {
        let path = temp_db("worker-single-active");
        let db = RunsDb::open_at(&path).expect("open");
        for index in 0..300 {
            let worker = format!("completed-worker-{index}");
            let run_id = db
                .begin_worker_tick(&worker, "completed-fixture")
                .expect("start populated terminal-run fixture");
            db.finish_worker_tick(&run_id, "completed", outcome::OK, None)
                .expect("finish populated terminal-run fixture");
        }
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze run fixture");
        let active_insert_guard = "SELECT 1 FROM worker_run_state existing
            JOIN runs existing_run ON existing_run.id = existing.run_id
            WHERE existing.worker_slug = ?1
              AND existing.phase IN ('starting', 'retrying', 'running')
              AND existing_run.outcome IS NULL";
        let active_update_guard = "SELECT 1 FROM worker_run_state existing
            JOIN runs existing_run ON existing_run.id = existing.run_id
            WHERE existing.worker_slug = ?1
              AND existing.run_id != ?2
              AND existing.phase IN ('starting', 'retrying', 'running')
              AND existing_run.outcome IS NULL";
        let insert_plan =
            explain_with_text_parameter(&db.conn, active_insert_guard, "worker-target");
        assert!(
            insert_plan.iter().any(|step| {
                step.contains("SEARCH existing")
                    && step.contains("idx_worker_run_state_worker")
            }),
            "the insert trigger's active-run guard should seek by worker on populated history; plan: {insert_plan:?}"
        );
        let mut update_statement = db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {active_update_guard}"))
            .expect("prepare update-trigger plan");
        let update_plan = update_statement
            .query_map(params!["worker-target", "candidate-run"], |row| {
                row.get::<_, String>(3)
            })
            .expect("explain update-trigger guard")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect update-trigger plan");
        drop(update_statement);
        assert!(
            update_plan.iter().any(|step| {
                step.contains("SEARCH existing")
                    && step.contains("idx_worker_run_state_worker")
            }),
            "the update trigger's active-run guard should seek by worker on populated history; plan: {update_plan:?}"
        );

        let completed_candidate = db
            .begin_worker_tick("worker-target", "completed-candidate")
            .expect("create candidate run before active run");
        db.finish_worker_tick(&completed_candidate, "completed", outcome::OK, None)
            .expect("complete candidate run");
        let first = db
            .begin_worker_tick("worker-target", "first-session")
            .expect("first run starts");
        assert!(
            db.begin_worker_tick("worker-target", "competing-session")
                .is_err(),
            "a second process must not admit another active run for the same worker"
        );
        assert!(
            db.update_worker_tick_phase(&completed_candidate, "running", None, None)
                .is_err(),
            "the update trigger must reject reactivating a second run for the same worker"
        );
        let candidate_phase: String = db
            .conn
            .query_row(
                "SELECT phase FROM worker_run_state WHERE run_id = ?1",
                [&completed_candidate],
                |row| row.get(0),
            )
            .expect("read candidate after rejected update");
        assert_eq!(candidate_phase, "completed");
        db.finish_worker_tick(&first, "completed", outcome::OK, None)
            .expect("close first run");
        db.begin_worker_tick("worker-target", "after-completion")
            .expect("worker may start after terminal outcome");

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn interrupted_worker_tick_is_closed_with_a_recovery_reason() {
        let path = temp_db("worker-interrupted");
        let db = RunsDb::open_at(&path).expect("open");
        let run_id = db
            .begin_worker_tick("worker-backend", "recipe-worker-interrupted")
            .expect("begin worker tick");
        db.update_worker_tick_phase(&run_id, "running", Some(17), None)
            .expect("mark running");

        assert_eq!(
            db.interrupt_worker_ticks("worker-backend", "scheduler_cancelled")
                .expect("reconcile interrupted tick"),
            1
        );
        let interrupted = db
            .latest_worker_tick("worker-backend")
            .expect("read interrupted state")
            .expect("worker run exists");
        assert_eq!(interrupted.phase, "interrupted");
        assert_eq!(
            interrupted.failure_code.as_deref(),
            Some("scheduler_cancelled")
        );
        assert_eq!(interrupted.outcome.as_deref(), Some(outcome::ABANDONED));

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn interruption_only_closes_the_current_process_worker_run() {
        let path = temp_db("worker-interrupted-owner");
        let db = RunsDb::open_at(&path).expect("open");
        let run_id = db
            .begin_worker_tick("worker-backend", "other-process-session")
            .expect("begin worker tick");
        db.conn
            .execute(
                "UPDATE runs SET pid = ?1 WHERE id = ?2",
                params![std::process::id() as i64 + 100_000, &run_id],
            )
            .expect("simulate another process owning this run");

        assert_eq!(
            db.interrupt_worker_ticks("worker-backend", "scheduler_cancelled")
                .expect("only current-process runs are recoverable here"),
            0
        );
        let state = db
            .latest_worker_tick("worker-backend")
            .expect("read state")
            .expect("run exists");
        assert_eq!(state.phase, "starting");
        assert!(state.outcome.is_none());

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn startup_reaper_marks_dead_worker_run_interrupted_atomically() {
        let path = temp_db("worker-process-reaped");
        let db = RunsDb::open_at(&path).expect("open");
        let run_id = db
            .begin_worker_tick("worker-backend", "dead-process-session")
            .expect("begin worker tick");
        db.update_worker_tick_phase(&run_id, "running", Some(31), None)
            .expect("mark worker run active");
        db.conn
            .execute(
                "UPDATE runs SET pid = 999999 WHERE id = ?1",
                params![&run_id],
            )
            .expect("simulate process exit");

        assert_eq!(db.reap_abandoned().expect("reap dead process"), 1);
        let state = db
            .latest_worker_tick("worker-backend")
            .expect("read interrupted worker state")
            .expect("worker run exists");
        assert_eq!(state.phase, "interrupted");
        assert_eq!(state.failure_code.as_deref(), Some("process_exited"));
        assert_eq!(state.outcome.as_deref(), Some(outcome::ABANDONED));

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

        let id = db.begin(kind::CLI, Some("arcanactl")).expect("begin");
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
    /// was reaped too, so `arcanactl` marked the running desktop app abandoned.
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
            "a live process on this host was marked abandoned — this is exactly what happened when arcanactl ran while the app was open"
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

    #[test]
    fn latest_worker_tick_lookup_uses_worker_index_with_populated_runs() {
        let path = temp_db("query-plan-latest-worker-tick");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        for index in 0..128 {
            let worker = if index % 8 == 0 {
                "worker-target"
            } else {
                "worker-other"
            };
            let run_id = db
                .begin_worker_tick(worker, &format!("session-{index}"))
                .expect("persist worker tick");
            db.finish_worker_tick(&run_id, "completed", outcome::OK, None)
                .expect("finish prior tick so the worker can be scheduled again");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let details =
            explain_with_text_parameter(&db.conn, LATEST_WORKER_TICK_BY_SLUG_SQL, "worker-target");

        assert!(
            details
                .iter()
                .any(|detail| detail.contains("idx_worker_run_state_worker")),
            "latest worker tick lookup should use its worker index; plan: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN worker_run_state")),
            "latest worker tick lookup scanned worker_run_state; plan: {details:?}"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn interrupted_worker_run_selection_uses_worker_index_and_reads_expected_runs() {
        let path = temp_db("query-plan-interrupted-worker-runs");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        for index in 0..256 {
            let run_id = db
                .begin_worker_tick(&format!("worker-{index}"), &format!("session-{index}"))
                .expect("persist historical worker tick");
            db.finish_worker_tick(&run_id, "completed", outcome::OK, None)
                .expect("finish historical worker tick");
        }
        let target_run = db
            .begin_worker_tick("worker-target", "session-target")
            .expect("persist target open tick");
        let unrelated_run = db
            .begin_worker_tick("worker-unrelated", "session-unrelated")
            .expect("persist unrelated open tick");
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let plan = {
            let mut statement = db
                .conn
                .prepare(&format!("EXPLAIN QUERY PLAN {INTERRUPT_WORKER_TICKS_SQL}"))
                .expect("prepare interrupted-run query plan");
            statement
                .query_map(
                    params!["worker-target", hostname(), std::process::id() as i64],
                    |row| row.get::<_, String>(3),
                )
                .expect("explain interrupted-run selection")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect interrupted-run query plan")
        };
        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_worker_run_state_worker")),
            "interrupted-run selection should use its worker index; plan: {plan:?}"
        );
        assert!(
            !plan.iter().any(|detail| {
                detail.contains("SCAN worker_run_state") || detail.contains("SCAN runs")
            }),
            "interrupted-run selection scanned a base table; plan: {plan:?}"
        );

        assert_eq!(
            db.interrupt_worker_ticks("worker-target", "scheduler_reconciled_interruption")
                .expect("interrupt target worker ticks"),
            1
        );
        let target = db
            .latest_worker_tick("worker-target")
            .expect("read target state")
            .expect("target state exists");
        assert_eq!(target.run_id, target_run);
        assert_eq!(target.phase, "interrupted");
        assert_eq!(
            target.failure_code.as_deref(),
            Some("scheduler_reconciled_interruption")
        );
        assert_eq!(
            db.get(&target_run)
                .expect("read target run")
                .expect("target run exists")
                .outcome
                .as_deref(),
            Some(outcome::ABANDONED)
        );
        let unrelated = db
            .latest_worker_tick("worker-unrelated")
            .expect("read unrelated state")
            .expect("unrelated state exists");
        assert_eq!(unrelated.run_id, unrelated_run);
        assert_eq!(unrelated.phase, "starting");
        assert!(db
            .get(&unrelated_run)
            .expect("read unrelated run")
            .unwrap()
            .is_open());

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn fleet_recovery_and_health_reads_use_expected_indexes_with_populated_runs() {
        let path = temp_db("query-plans-fleet-reads");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");

        let mut target_runs = Vec::new();
        let mut all_runs = Vec::new();
        for worker_index in 0..48 {
            let worker = if worker_index < 8 {
                "worker-target"
            } else {
                "worker-other"
            };
            for tick_index in 0..4 {
                let run_id = db
                    .begin_worker_tick(worker, &format!("session-{worker_index}-{tick_index}"))
                    .expect("persist worker tick");
                all_runs.push(run_id.clone());
                if worker == "worker-target" {
                    target_runs.push(run_id.clone());
                }
                db.finish_worker_tick(&run_id, "completed", outcome::OK, None)
                    .expect("finish worker tick");
            }
        }

        let progress_run = target_runs.last().expect("target run fixture");
        for index in 0..96 {
            db.conn
                .execute(
                    "INSERT INTO worker_run_events (run_id, event_at, operation, outcome)
                     VALUES (?1, ?2, 'tool_batch', 'ok')",
                    params![progress_run, format!("2026-10-02T00:{:02}:00Z", index % 60)],
                )
                .expect("insert progress event fixture");
        }
        for run_id in &all_runs {
            if run_id == progress_run {
                continue;
            }
            for index in 0..24 {
                db.conn
                    .execute(
                        "INSERT INTO worker_run_events (run_id, event_at, operation, outcome)
                         VALUES (?1, ?2, 'tool_batch', 'ok')",
                        params![run_id, format!("2026-10-01T00:{:02}:00Z", index % 60)],
                    )
                    .expect("insert unrelated progress event fixture");
            }
        }

        let open_run = db.begin(kind::APP, Some("open-fixture")).expect("open run");
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        let owner_plan =
            explain_with_text_parameter(&db.conn, LATEST_WORKER_RUN_OWNER_SQL, "worker-target");
        assert!(
            owner_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_run_state_worker")),
            "latest owner should use the worker-slug index; plan: {owner_plan:?}"
        );
        assert!(
            !owner_plan.iter().any(|detail| {
                detail.contains("SCAN worker_run_state") || detail.contains("SCAN runs")
            }),
            "latest owner should not scan either base table; plan: {owner_plan:?}"
        );
        assert!(
            owner_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "ordering joined run timestamps requires a sort; plan: {owner_plan:?}"
        );

        let roster_plan = explain_with_integer_parameter(&db.conn, LATEST_WORKER_TICKS_SQL, 100);
        assert!(
            roster_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_run_state_worker")),
            "latest-per-worker lookup should use the worker-slug index; plan: {roster_plan:?}"
        );
        assert!(
            !roster_plan.iter().any(|detail| {
                detail.contains("SCAN worker_run_state s2")
                    || detail.contains("SCAN runs r2")
                    || detail.contains("SCAN runs r ")
            }),
            "latest-per-worker inner lookup should not scan its joined base tables; plan: {roster_plan:?}"
        );
        assert!(
            roster_plan
                .iter()
                .any(|detail| detail.contains("SCAN r USING INDEX idx_runs_started")),
            "roster construction walks the start-time index to enumerate candidate runs; plan: {roster_plan:?}"
        );
        assert!(
            roster_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "global newest-first roster ordering requires a sort; plan: {roster_plan:?}"
        );

        let progress_plan = explain_with_text_and_integer_parameters(
            &db.conn,
            RECENT_WORKER_PROGRESS_SQL,
            progress_run,
            20,
        );
        assert!(
            progress_plan
                .iter()
                .any(|detail| detail.contains("idx_worker_run_events_run_id_id")),
            "recent progress should seek by run id; plan: {progress_plan:?}"
        );
        assert!(
            !progress_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_run_events")),
            "recent progress should not scan the event table; plan: {progress_plan:?}"
        );
        assert!(
            !progress_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "the run/event index should provide newest-first event ordering; plan: {progress_plan:?}"
        );

        let open_plan = explain_without_parameters(&db.conn, OPEN_RUNS_SQL);
        assert!(
            open_plan
                .iter()
                .any(|detail| detail.contains("idx_runs_open")),
            "open-run recovery should use its partial open-runs index; plan: {open_plan:?}"
        );
        assert!(
            !open_plan.iter().any(|detail| detail.contains("SCAN runs")),
            "open-run recovery should not scan the runs table; plan: {open_plan:?}"
        );
        assert!(
            open_plan
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "the partial outcome index does not order open runs by start time; plan: {open_plan:?}"
        );

        let target_latest_id = target_runs.last().expect("latest target run fixture");
        let unrelated_latest_id = all_runs.last().expect("latest unrelated run fixture");
        let target_run = db
            .get(target_latest_id)
            .expect("read latest target run")
            .expect("latest target run exists");
        let owner = db
            .latest_worker_run_owner("worker-target")
            .expect("read latest target owner")
            .expect("target owner exists");
        assert_eq!(owner.run_id, *target_latest_id);
        assert_eq!(owner.host, target_run.host);
        assert_eq!(owner.pid, target_run.pid);
        assert_eq!(owner.phase, "completed");
        assert_eq!(owner.outcome.as_deref(), Some(outcome::OK));

        let latest_ticks = db
            .latest_worker_ticks(100)
            .expect("read latest fleet ticks");
        assert_eq!(latest_ticks.len(), 2, "one latest tick per worker");
        assert_eq!(
            latest_ticks
                .iter()
                .map(|tick| (tick.worker_slug.as_str(), tick.run_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("worker-other", unrelated_latest_id.as_str()),
                ("worker-target", target_latest_id.as_str()),
            ],
            "fleet ticks should be newest-first, scoped to each worker, with no stale tick leakage"
        );
        assert_eq!(latest_ticks[0].session_id, "session-47-3");
        assert_eq!(latest_ticks[0].phase, "completed");
        assert_eq!(latest_ticks[0].outcome.as_deref(), Some(outcome::OK));
        assert_eq!(latest_ticks[1].session_id, "session-7-3");
        assert_eq!(latest_ticks[1].phase, "completed");
        assert_eq!(latest_ticks[1].outcome.as_deref(), Some(outcome::OK));

        assert!(db.open_runs().unwrap().iter().any(|run| run.id == open_run));
        assert_eq!(
            db.recent_worker_progress(progress_run, 20).unwrap().len(),
            20,
            "the production progress reader must return its bounded rows"
        );

        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn worker_progress_retention_uses_run_index_and_keeps_latest_500() {
        fn explain_worker_progress_prune_plan(
            conn: &rusqlite::Connection,
            run_id: &str,
        ) -> Vec<String> {
            let mut statement = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {PRUNE_WORKER_PROGRESS_SQL}"))
                .expect("prepare retention plan");
            statement
                .query_map([run_id], |row| row.get::<_, String>(3))
                .expect("explain retention query")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("collect retention plan")
        }

        let path = temp_db("query-plan-worker-progress-retention");
        let db = RunsDb::open_at(&path).expect("open isolated runs database");
        let target_run = db
            .begin_worker_tick("worker-target", "progress-retention")
            .expect("begin target run");
        let other_run = db
            .begin_worker_tick("worker-other", "unrelated-progress")
            .expect("begin unrelated run");
        for index in 0..620 {
            db.conn
                .execute(
                    "INSERT INTO worker_run_events (run_id, event_at, operation, outcome)
                     VALUES (?1, ?2, 'operation', 'ok')",
                    params![&target_run, format!("2026-10-02T00:{:02}:00Z", index % 60)],
                )
                .expect("insert target progress event");
        }
        for index in 0..24 {
            db.conn
                .execute(
                    "INSERT INTO worker_run_events (run_id, event_at, operation, outcome)
                     VALUES (?1, ?2, 'other', 'ok')",
                    params![&other_run, format!("2026-10-01T00:{:02}:00Z", index % 60)],
                )
                .expect("insert unrelated progress event");
        }
        db.conn.execute_batch("ANALYZE").expect("analyze fixture");

        db.conn
            .execute_batch("DROP INDEX idx_worker_run_events_run_id_id; ANALYZE")
            .expect("measure baseline retention plan");
        let baseline_plan = explain_worker_progress_prune_plan(&db.conn, &target_run);
        assert!(
            baseline_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_run_events")),
            "baseline progress subquery should expose the global event scan: {baseline_plan:?}"
        );
        db.conn
            .execute_batch(
                "CREATE INDEX idx_worker_run_events_run_id_id
                    ON worker_run_events(run_id, id DESC);
                 ANALYZE",
            )
            .expect("install measured retention index");

        let plan = explain_worker_progress_prune_plan(&db.conn, &target_run);
        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_worker_run_events_run_id_id")),
            "bounded retention should select the run/event index: {plan:?}"
        );
        assert!(
            !plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_run_events")),
            "retention should not scan the global event table: {plan:?}"
        );

        db.record_worker_progress(&target_run, Some(50), "final_progress")
            .expect("append and prune target progress");
        let target_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM worker_run_events WHERE run_id=?1",
                [&target_run],
                |row| row.get(0),
            )
            .expect("count target events");
        let other_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM worker_run_events WHERE run_id=?1",
                [&other_run],
                |row| row.get(0),
            )
            .expect("count unrelated events");
        assert_eq!(target_count, 500, "only the latest 500 rows are retained");
        assert_eq!(other_count, 24, "another run's evidence is untouched");
        assert_eq!(
            db.recent_worker_progress(&target_run, 1).unwrap()[0].1,
            "final_progress"
        );

        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
