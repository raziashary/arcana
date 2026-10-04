// Workers database — SQLite CRUD for the Worker registry.
// Database path: ~/.arcana/workers.db
// Seeds pre-defined workers + system agents on first open.
// Used by: Worker Management UI, Agent Roster, Supervisor recipe (list_workers__arcana)
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

use super::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};
use rusqlite::{params, Connection, OptionalExtension};

/// Dashboard worker summary reads. Keep the production readers shared with the
/// populated-fixture query-plan regression below.
pub(crate) const DASHBOARD_WORKER_TOTAL_SQL: &str =
    "SELECT COUNT(*) FROM workers WHERE status != 'deleted'";
pub(crate) const DASHBOARD_WORKER_ACTIVE_SQL: &str =
    "SELECT COUNT(*) FROM workers WHERE status NOT IN ('paused', 'deleted')";
pub(crate) const DASHBOARD_WORKER_ADDED_TODAY_SQL: &str =
    "SELECT COUNT(*) FROM workers WHERE date(created_at) = ?1";
const WORKER_ROSTER_SQL: &str = "SELECT id, name, slug, worker_type, status, model, primary_repos,
                    polling_interval, recipe_id, is_system_defined, description,
                    created_at, updated_at, last_seen
             FROM workers WHERE status != 'deleted' ORDER BY is_system_defined DESC, id ASC";
const WORKER_BY_SLUG_SQL: &str = "SELECT id, name, slug, worker_type, status, model, primary_repos,
                    polling_interval, recipe_id, is_system_defined, description,
                    created_at, updated_at, last_seen
             FROM workers WHERE slug = ?1 AND status != 'deleted'";
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkerType {
    Backend,
    Frontend,
    Platform,
    Mcp,
    Cleanup,
    Supervisor,
    Qc,
    Custom,
}

impl WorkerType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Backend => "backend",
            Self::Frontend => "frontend",
            Self::Platform => "platform",
            Self::Mcp => "mcp",
            Self::Cleanup => "cleanup",
            Self::Supervisor => "supervisor",
            Self::Qc => "qc",
            Self::Custom => "custom",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "backend" => Self::Backend,
            "frontend" => Self::Frontend,
            "platform" => Self::Platform,
            "mcp" => Self::Mcp,
            "cleanup" => Self::Cleanup,
            "supervisor" => Self::Supervisor,
            "qc" => Self::Qc,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Created,        // Seeded but not yet started
    Idle,           // Started, polling, no current task
    IdleNoProvider, // Started without a configured provider; retried by scheduler
    Active,         // Running — recipe cron is active
    Working,        // Executing a task
    Paused,         // User paused
    Error,          // Recipe crashed / last run failed
    Deleted,        // Soft-deleted
}

impl WorkerStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Created => "created",
            Self::Idle => "idle",
            Self::IdleNoProvider => "idle_no_provider",
            Self::Active => "active",
            Self::Working => "working",
            Self::Paused => "paused",
            Self::Error => "error",
            Self::Deleted => "deleted",
        }
    }
    pub fn try_from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "created" => Ok(Self::Created),
            "idle" => Ok(Self::Idle),
            "idle_no_provider" => Ok(Self::IdleNoProvider),
            "active" => Ok(Self::Active),
            "working" => Ok(Self::Working),
            "paused" => Ok(Self::Paused),
            "error" => Ok(Self::Error),
            "deleted" => Ok(Self::Deleted),
            _ => Err(format!("unknown worker status '{s}'")),
        }
    }
}

impl FromSql for WorkerStatus {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        Self::try_from_str(text).map_err(|error| {
            FromSqlError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error,
            )))
        })
    }
}

/// The model a worker gets when a caller does not choose one.
///
/// Must equal the `model` column's DEFAULT in `migrate()`. A caller that
/// hardcodes its own string instead — as `arcanactl worker register` would have
/// had to — drifts from the schema the moment either changes, and the drift is
/// invisible because both produce a valid-looking worker.
/// `the_default_model_constant_matches_the_schema_default` pins them together.
// Arcana's owner-selected PA and fleet default; keep the Rust constant, schema
// default, frontend picker, and migration aligned.
pub const DEFAULT_MODEL: &str = "gemini-3.8-flash";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: i64,
    pub name: String,
    pub slug: String, // kebab-case unique identifier
    pub worker_type: String,
    pub status: WorkerStatus,
    pub model: String,
    pub primary_repos: Vec<String>, // JSON array stored as TEXT
    pub polling_interval: i64,      // seconds (default 120)
    pub recipe_id: Option<String>,  // bound recipe YAML id
    pub is_system_defined: bool,    // true = cannot delete
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Last heartbeat, written by `heartbeat()`. `None` for a worker that has
    /// never reported in.
    ///
    /// Added in P1-09. `arcanactl` had been reading and writing this column since
    /// it was written, against a table that never had it — see the migration
    /// note on `add_missing_columns`.
    pub last_seen: Option<String>,
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct WorkersDb {
    conn: Connection,
}

impl WorkersDb {
    /// Build the real seeded roster in memory for cross-module unit tests.
    ///
    /// File-backed `open_at` runs the migration ladder and deliberately writes
    /// a restorable backup first. Tests that only need the seed rows should not
    /// create backup artifacts under the user's Arcana data directory.
    #[cfg(test)]
    pub(crate) fn open_seeded_in_memory_for_test() -> Result<Self> {
        let db = Self {
            conn: Connection::open_in_memory()?,
        };
        db.add_missing_columns()?;
        migrate::run_ladder(&db.conn, "workers.db", Self::LADDER)?;
        db.migrate_model_names()?;
        db.seed_defaults()?;
        Ok(db)
    }

    /// Open (or create) ~/.arcana/workers.db and run migrations + seed.
    /// This database's schema version, as recorded by the migration ladder.
    ///
    /// Exposed so a migration can be *verified* rather than assumed — see
    /// `examples/migrate_check.rs`. Reading it through the same handle the
    /// application uses is the point: a version read from a fresh connection
    /// would prove the file's state, not that the shipped `open()` path put it
    /// there.
    pub fn schema_version(&self) -> Result<i32> {
        migrate::user_version(&self.conn)
    }

    pub fn open() -> Result<Self> {
        Self::open_at(&arcana_dir()?.join("workers.db"))
    }

    /// As `open`, against an explicit path.
    ///
    /// Exists so tests never touch the user's real `~/.arcana/workers.db`.
    /// An earlier helper in this tree did exactly that, and one failed test
    /// left a file behind that made every later run fail — an order-dependent
    /// suite that also wrote to live user data.
    pub fn open_at(db_path: &std::path::Path) -> Result<Self> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("workers_database_initialize")
                .with_database("workers");
        let conn = crate::db::open_tuned(db_path)
            .map_err(|e| ArcanaError::Agent(format!("workers.db open failed: {e}")))?;
        let db = Self { conn };
        // Order matters: repair a pre-ladder database's columns first, so the
        // baseline rung is a genuine no-op against it, then stamp the version.
        db.add_missing_columns()?; // P1-09 — columns added to an already-created table
        migrate::run_ladder(&db.conn, "workers.db", Self::LADDER)?;
        db.migrate_model_names()?; // Phase 7.1 — fix any stale old-format model slugs
        db.seed_defaults()?;
        operation.succeed();
        Ok(db)
    }

    /// `workers.db`'s ladder.
    ///
    /// `v1` is the schema as it stands, `last_seen` included, so a fresh
    /// database is created complete. Databases that predate the column are
    /// repaired by `add_missing_columns`, which runs **before** the ladder:
    /// expressing that repair as a `v2` rung would issue
    /// `ALTER TABLE … ADD COLUMN last_seen` against live databases that already
    /// have the column and fail with *duplicate column name*.
    const LADDER: &[Migration] = &[
        Migration {
            version: 1,
            name: "workers-baseline",
            sql: r#"
            CREATE TABLE IF NOT EXISTS workers (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                name              TEXT NOT NULL,
                slug              TEXT NOT NULL UNIQUE,
                worker_type       TEXT NOT NULL DEFAULT 'custom',
                status            TEXT NOT NULL DEFAULT 'created'
                                  CHECK (status IN ('created', 'idle', 'idle_no_provider',
                                                    'active', 'working', 'paused', 'error', 'deleted')),
                model             TEXT NOT NULL DEFAULT 'gemini-3.8-flash',
                primary_repos     TEXT NOT NULL DEFAULT '[]',
                polling_interval  INTEGER NOT NULL DEFAULT 120,
                recipe_id         TEXT,
                is_system_defined INTEGER NOT NULL DEFAULT 0,
                description       TEXT,
                created_at        DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                updated_at        DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                last_seen         TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_workers_slug   ON workers(slug);
            CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
            CREATE TRIGGER IF NOT EXISTS update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
                END;
        "#,
        },
        Migration {
            version: 2,
            name: "worker-status-check",
            sql: r#"
            CREATE TABLE workers_status_migration (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                name              TEXT NOT NULL,
                slug              TEXT NOT NULL UNIQUE,
                worker_type       TEXT NOT NULL DEFAULT 'custom',
                status            TEXT NOT NULL DEFAULT 'created'
                                  CHECK (status IN ('created', 'idle', 'idle_no_provider',
                                                    'active', 'working', 'paused', 'error', 'deleted')),
                model             TEXT NOT NULL DEFAULT 'gemini-3.8-flash',
                primary_repos     TEXT NOT NULL DEFAULT '[]',
                polling_interval  INTEGER NOT NULL DEFAULT 120,
                recipe_id         TEXT,
                is_system_defined INTEGER NOT NULL DEFAULT 0,
                description       TEXT,
                created_at        DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                updated_at        DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                last_seen         TEXT
            );
            INSERT INTO workers_status_migration (
                id, name, slug, worker_type, status, model, primary_repos,
                polling_interval, recipe_id, is_system_defined, description,
                created_at, updated_at, last_seen
            ) SELECT
                id, name, slug, worker_type, status, model, primary_repos,
                polling_interval, recipe_id, is_system_defined, description,
                created_at, updated_at, last_seen
            FROM workers;
            DROP TABLE workers;
            ALTER TABLE workers_status_migration RENAME TO workers;
            CREATE INDEX IF NOT EXISTS idx_workers_slug ON workers(slug);
            CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
            CREATE TRIGGER IF NOT EXISTS update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
                END;
        "#,
        },
        Migration {
            version: 3,
            name: "default-built-in-workers-to-gemini-3.8-flash",
            sql: r#"
                UPDATE workers SET model = 'gemini-3.8-flash'
                 WHERE (slug = 'supervisor' AND model = 'claude-opus-5')
                    OR (slug = 'qc-agent' AND model = 'claude-haiku-4-5')
                    OR (slug IN ('worker-backend', 'worker-frontend', 'worker-platform')
                        AND model = 'claude-sonnet-5')
                    OR (slug = 'worker-mcp' AND model = 'claude-opus-5')
                    OR (slug = 'worker-cleanup' AND model = 'claude-haiku-4-5');
            "#,
        },
        Migration {
            version: 4,
            name: "canonical-utc-timestamps",
            sql: r#"
            DROP TRIGGER IF EXISTS update_workers_timestamp;
            CREATE TRIGGER update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
                END;
            CREATE TRIGGER IF NOT EXISTS workers_timestamps_canonical_insert
                AFTER INSERT ON workers
                BEGIN
                    UPDATE workers SET
                        created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                        updated_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.updated_at), NEW.updated_at),
                        last_seen = CASE WHEN NEW.last_seen IS NULL THEN NULL
                            ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.last_seen), NEW.last_seen) END
                     WHERE id = NEW.id;
                END;
            CREATE TRIGGER IF NOT EXISTS workers_timestamps_canonical_update
                AFTER UPDATE OF created_at, last_seen ON workers
                BEGIN
                    UPDATE workers SET
                        created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                        last_seen = CASE WHEN NEW.last_seen IS NULL THEN NULL
                            ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.last_seen), NEW.last_seen) END
                     WHERE id = NEW.id;
                END;
        "#,
        },
        Migration {
            version: 5,
            name: "index-workers-created-date-for-dashboard",
            sql: r#"
                CREATE INDEX IF NOT EXISTS idx_workers_created_date
                    ON workers(date(created_at));
            "#,
        },
        Migration {
            version: 6,
            name: "index-active-worker-roster-order",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_workers_active_roster_order
                ON workers(is_system_defined DESC, id)
                WHERE status != 'deleted';
        "#,
        },
    ];

    /// Columns added to `workers` after its original `CREATE TABLE`.
    ///
    /// `src-shared` declared `last_seen` in its own
    /// `CREATE TABLE IF NOT EXISTS workers`, but the table had already been
    /// created without it, so the statement was a silent no-op and every
    /// `arcanactl worker` call failed with *no such column*
    /// (`ADR-LOG-RESTRUCT-0016`). `notes` was declared there too and has never
    /// been selected by anything, so it is deliberately not resurrected here.
    const ADDED_COLUMNS: &[(&str, &str)] =
        &[("last_seen", "ALTER TABLE workers ADD COLUMN last_seen TEXT")];

    fn add_missing_columns(&self) -> Result<()> {
        migrate::ensure_columns(&self.conn, "workers", Self::ADDED_COLUMNS)?;
        Ok(())
    }

    /// Record that a worker is alive.
    ///
    /// Returns an error when the slug matches nothing, rather than reporting
    /// success for a worker that does not exist — a heartbeat that silently
    /// does nothing is worse than one that fails, because it is read as
    /// liveness.
    pub fn heartbeat(&self, slug: &str) -> Result<()> {
        let timing = OperationTimer::start("database_worker_heartbeat")
            .with_database("workers")
            .with_worker(slug);
        let rows = self
            .conn
            .execute(
                "UPDATE workers
                 SET last_seen = ?2,
                     status = CASE
                         WHEN status IN ('working', 'paused', 'error') THEN status
                         ELSE 'active'
                     END
                 WHERE slug = ?1 AND status != 'deleted'",
                params![slug, crate::db::timestamps::now_rfc3339()],
            )
            .map_err(|e| ArcanaError::Agent(format!("heartbeat: {e}")))?;
        if rows == 0 {
            return Err(ArcanaError::Agent(format!(
                "No active worker '{slug}' to record a heartbeat for"
            )));
        }
        timing.with_result_count(rows).succeed();
        Ok(())
    }

    /// Soft-delete a worker: mark it `deleted` so `list` stops returning it.
    ///
    /// Deliberately **not** routed to `delete`, which removes the row outright
    /// and refuses system-defined workers. `arcanactl worker unregister` has
    /// always been reversible, and mapping it onto a hard delete during this
    /// consolidation would have turned an undo-able command into permanent
    /// loss of live rows without anyone asking for that.
    pub fn deactivate(&self, slug: &str) -> Result<()> {
        let timing = OperationTimer::start("database_worker_deactivate")
            .with_database("workers")
            .with_worker(slug);
        let rows = self
            .conn
            .execute(
                "UPDATE workers SET status = 'deleted' WHERE slug = ?1 AND status != 'deleted'",
                params![slug],
            )
            .map_err(|e| ArcanaError::Agent(format!("deactivate: {e}")))?;
        if rows == 0 {
            timing.with_result_count(0).set_outcome("not_found");
            return Err(ArcanaError::Agent(format!(
                "No active worker '{slug}' to unregister"
            )));
        }
        timing.with_result_count(rows).succeed();
        Ok(())
    }

    /// Phase 7.1 — one-time data migration: rename any workers.db rows that were
    /// created with old-format model slugs to the canonical direct-API IDs.
    ///
    /// Background: before canonical naming was settled, seeds and UI may have stored
    /// "claude-4-5-haiku" / "claude-4-5-sonnet" / "claude-4-5-opus". The Anthropic
    /// provider now maps these correctly, but updating the stored value is cleaner and
    /// prevents confusion when the model name is displayed in the UI or used in logs.
    ///
    /// Safe to run every startup — UPDATE ... WHERE model = '...' is a no-op when
    /// no matching rows exist.
    fn migrate_model_names(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
            UPDATE workers SET model = 'claude-haiku-4-5'  WHERE model = 'claude-4-5-haiku';
            UPDATE workers SET model = 'claude-sonnet-4-5' WHERE model = 'claude-4-5-sonnet';
            UPDATE workers SET model = 'claude-opus-4-5'   WHERE model = 'claude-4-5-opus';

            -- 2026-09-02 (P2-31). The two above are now themselves RETIRED —
            -- Anthropic withdrew the 4.5 Opus and Sonnet ids, so any worker
            -- still carrying one has a model no provider will answer. This is
            -- the same rename this function already does, one generation on.
            --
            -- Haiku 4.5 is deliberately NOT renamed: it is still current and
            -- still in the catalogue.
            UPDATE workers SET model = 'claude-sonnet-5' WHERE model = 'claude-sonnet-4-5';
            UPDATE workers SET model = 'claude-opus-5'   WHERE model = 'claude-opus-4-5';

            -- Google renamed and superseded on the same day: `gemini-3.1-pro`
            -- never existed as a callable id (Google 404s it; the real one
            -- carries `-preview`), and 3.5 Flash was superseded by 3.7.
            UPDATE workers SET model = 'gemini-3.1-pro-preview' WHERE model = 'gemini-3.1-pro';
            UPDATE workers SET model = 'gemini-3.7-flash'       WHERE model = 'gemini-3.5-flash';

            -- Removed for having no verified price, so no worker may sit on
            -- one: an unpriced model bills $0 and runs invisible to the cap.
            UPDATE workers SET model = 'gpt-5.5' WHERE model = 'gpt-5.5-pro';
            UPDATE workers SET model = 'gpt-5.4' WHERE model = 'gpt-5.3-codex';
        "#,
            )
            .map_err(|e| ArcanaError::Agent(format!("migrate_model_names: {e}")))?;
        Ok(())
    }

    /// Seed the 5 pre-defined workers + 2 system agents if the table is empty.
    /// Each worker is bound to its corresponding recipe on creation.
    fn seed_defaults(&self) -> Result<()> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM workers", [], |r| r.get(0))
            .unwrap_or(0);
        if count > 0 {
            return Ok(());
        }

        let seeds: &[(&str, &str, &str, &str, i64, bool, &str, Option<&str>)] = &[
            // (name, slug, type, model, interval, is_system, description, recipe_id)
            (
                "Supervisor",
                "supervisor",
                "supervisor",
                "gemini-3.8-flash",
                120,
                true,
                "Polls the task board, assigns tasks to workers.",
                Some("supervisor-poll"),
            ),
            // Doc 04 Phase 3 — QC worker now runs the qc-dispatcher recipe.
            // The slug stays `qc-agent` for backward compat (Live Monitor
            // card, scheduler hardcodes, hooks); the dispatcher recipe runs
            // on haiku and fans out per-task sonnet reviewer subagents via
            // `spawn_qc_review`. Old `qc-agent-review` recipe still exists in
            // the registry for users who manually rebind workers.
            (
                "QC Agent",
                "qc-agent",
                "qc",
                "gemini-3.8-flash",
                60,
                true,
                "Dispatches per-task QC reviewers (Doc 04 Phase 3).",
                Some("qc-dispatcher"),
            ),
            (
                "Worker Backend",
                "worker-backend",
                "backend",
                "gemini-3.8-flash",
                120,
                false,
                "Implements backend tasks (Python / Rust).",
                Some("worker-backend-poll"),
            ),
            (
                "Worker Frontend",
                "worker-frontend",
                "frontend",
                "gemini-3.8-flash",
                120,
                false,
                "Implements frontend tasks (React / TypeScript).",
                Some("worker-frontend-poll"),
            ),
            (
                "Worker Platform",
                "worker-platform",
                "platform",
                "gemini-3.8-flash",
                120,
                false,
                "Infrastructure, CI/CD, and DevOps tasks.",
                Some("worker-platform-poll"),
            ),
            (
                "Worker MCP",
                "worker-mcp",
                "mcp",
                "gemini-3.8-flash",
                120,
                false,
                "Builds and maintains MCP server extensions.",
                Some("worker-mcp-poll"),
            ),
            (
                "Worker Cleanup",
                "worker-cleanup",
                "cleanup",
                "gemini-3.8-flash",
                120,
                false,
                "Lint, dead code removal, and housekeeping.",
                Some("worker-cleanup-poll"),
            ),
        ];

        for (name, slug, wtype, model, interval, system, desc, recipe_id) in seeds {
            self.conn.execute(
                "INSERT INTO workers (name, slug, worker_type, model, polling_interval, is_system_defined, description, recipe_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![name, slug, wtype, model, interval, *system as i64, desc, recipe_id],
            ).map_err(|e| ArcanaError::Agent(format!("seed workers: {e}")))?;
        }
        Ok(())
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn list(&self) -> Result<Vec<Worker>> {
        let mut operation =
            OperationTimer::start("database_worker_roster_read").with_database("workers");
        let mut stmt = self
            .conn
            .prepare(WORKER_ROSTER_SQL)
            .map_err(|e| ArcanaError::Agent(format!("workers list prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let repos_json: String = row.get(6)?;
                let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
                Ok(Worker {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    slug: row.get(2)?,
                    worker_type: row.get(3)?,
                    status: row.get(4)?,
                    model: row.get(5)?,
                    primary_repos: repos,
                    polling_interval: row.get(7)?,
                    recipe_id: row.get(8)?,
                    is_system_defined: row.get::<_, i64>(9)? != 0,
                    description: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    last_seen: row.get(13)?,
                })
            })
            .map_err(|e| ArcanaError::Agent(format!("workers list query: {e}")))?;

        let workers = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("workers list collect: {e}")))?;
        operation = operation.with_result_count(workers.len());
        operation.succeed();
        Ok(workers)
    }

    /// Registered worker slugs used by both prompt construction and dispatch
    /// validation. Deleted rows stay in history but are not valid targets.
    pub fn registered_slugs(&self) -> Result<Vec<String>> {
        Ok(self
            .list()?
            .into_iter()
            .filter(|worker| worker.status != WorkerStatus::Deleted)
            .map(|worker| worker.slug)
            .collect())
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Worker>> {
        let timing = OperationTimer::start("database_worker_read_by_id").with_database("workers");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at, last_seen \
             FROM workers WHERE id = ?1 AND status != 'deleted'",
            )
            .map_err(|e| ArcanaError::Agent(format!("get_by_id prepare: {e}")))?;

        let worker = stmt
            .query_row(params![id], |row| {
                let repos_json: String = row.get(6)?;
                let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
                Ok(Worker {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    slug: row.get(2)?,
                    worker_type: row.get(3)?,
                    status: row.get(4)?,
                    model: row.get(5)?,
                    primary_repos: repos,
                    polling_interval: row.get(7)?,
                    recipe_id: row.get(8)?,
                    is_system_defined: row.get::<_, i64>(9)? != 0,
                    description: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    last_seen: row.get(13)?,
                })
            })
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("get_by_id: {e}")))?;
        if let Some(worker) = &worker {
            timing
                .with_worker(&worker.slug)
                .with_result_count(1)
                .succeed();
        } else {
            timing.with_result_count(0).set_outcome("not_found");
        }
        Ok(worker)
    }

    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Worker>> {
        let timing = OperationTimer::start("database_worker_read_by_slug").with_database("workers");
        let mut stmt = self
            .conn
            .prepare(WORKER_BY_SLUG_SQL)
            .map_err(|e| ArcanaError::Agent(format!("get_by_slug prepare: {e}")))?;

        let worker = stmt
            .query_row(params![slug], |row| {
                let repos_json: String = row.get(6)?;
                let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
                Ok(Worker {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    slug: row.get(2)?,
                    worker_type: row.get(3)?,
                    status: row.get(4)?,
                    model: row.get(5)?,
                    primary_repos: repos,
                    polling_interval: row.get(7)?,
                    recipe_id: row.get(8)?,
                    is_system_defined: row.get::<_, i64>(9)? != 0,
                    description: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    last_seen: row.get(13)?,
                })
            })
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("get_by_slug: {e}")))?;
        if let Some(worker) = &worker {
            timing
                .with_worker(&worker.slug)
                .with_result_count(1)
                .succeed();
        } else {
            timing.with_result_count(0).set_outcome("not_found");
        }
        Ok(worker)
    }

    pub fn create(
        &self,
        name: &str,
        slug: &str,
        worker_type: &str,
        model: &str,
        primary_repos: &[String],
        polling_interval: i64,
        description: Option<&str>,
    ) -> Result<i64> {
        let timing = OperationTimer::start("database_worker_create")
            .with_database("workers")
            .with_worker(slug);
        let repos_json = serde_json::to_string(primary_repos)
            .map_err(|e| ArcanaError::Agent(format!("repos json: {e}")))?;
        self.conn.execute(
            "INSERT INTO workers (name, slug, worker_type, model, primary_repos, polling_interval, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, slug, worker_type, model, repos_json, polling_interval, description],
        ).map_err(|e| ArcanaError::Agent(format!("create worker: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_status(&self, id: i64, status: WorkerStatus) -> Result<()> {
        let mut operation =
            OperationTimer::start("database_worker_status_update").with_database("workers");
        let changed = self
            .conn
            .execute(
                "UPDATE workers SET status = ?1 WHERE id = ?2",
                params![status.as_str(), id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_status: {e}")))?;
        operation = operation.with_result_count(changed);
        operation.succeed();
        Ok(())
    }

    pub fn update_model(&self, id: i64, model: &str) -> Result<()> {
        let timing = OperationTimer::start("database_worker_model_update").with_database("workers");
        let changed = self
            .conn
            .execute(
                "UPDATE workers SET model = ?1 WHERE id = ?2",
                params![model, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_model: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn update_repos(&self, id: i64, primary_repos: &[String]) -> Result<()> {
        let timing = OperationTimer::start("database_worker_repos_update").with_database("workers");
        let repos_json = serde_json::to_string(primary_repos)
            .map_err(|e| ArcanaError::Agent(format!("repos json: {e}")))?;
        let changed = self
            .conn
            .execute(
                "UPDATE workers SET primary_repos = ?1 WHERE id = ?2",
                params![repos_json, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_repos: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn bind_recipe(&self, id: i64, recipe_id: &str) -> Result<()> {
        let timing = OperationTimer::start("database_worker_recipe_bind").with_database("workers");
        let changed = self
            .conn
            .execute(
                "UPDATE workers SET recipe_id = ?1 WHERE id = ?2",
                params![recipe_id, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("bind_recipe: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    /// Soft-delete — system agents cannot be deleted.
    pub fn delete(&self, id: i64) -> Result<()> {
        let timing = OperationTimer::start("database_worker_delete").with_database("workers");
        let is_system: i64 = self
            .conn
            .query_row(
                "SELECT is_system_defined FROM workers WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| ArcanaError::Agent(format!("delete check: {e}")))?
            .unwrap_or(0);
        if is_system != 0 {
            return Err(ArcanaError::Agent(
                "Cannot delete system-defined agents".into(),
            ));
        }
        let changed = self
            .conn
            .execute(
                "UPDATE workers SET status = 'deleted' WHERE id = ?1",
                params![id],
            )
            .map_err(|e| ArcanaError::Agent(format!("delete worker: {e}")))?;
        if changed == 0 {
            timing.with_result_count(0).set_outcome("not_found");
        } else {
            timing.with_result_count(changed).succeed();
        }
        Ok(())
    }

    pub fn slug_exists(&self, slug: &str) -> Result<bool> {
        let timing = OperationTimer::start("database_worker_slug_exists").with_database("workers");
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM workers WHERE slug = ?1 AND status != 'deleted'",
                params![slug],
                |r| r.get(0),
            )
            .map_err(|e| ArcanaError::Agent(format!("slug_exists: {e}")))?;
        if count > 0 {
            timing.with_result_count(1).succeed();
        } else {
            timing.with_result_count(0).set_outcome("not_found");
        }
        Ok(count > 0)
    }
}

// ─── Slug helper ──────────────────────────────────────────────────────────────

/// Convert a human name to a kebab-case slug, e.g. "My Worker" → "my-worker".
pub fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialize(conn: Connection) -> Result<WorkersDb> {
        let db = WorkersDb { conn };
        db.add_missing_columns()?;
        migrate::run_ladder(&db.conn, "workers.db", WorkersDb::LADDER)?;
        db.migrate_model_names()?;
        db.seed_defaults()?;
        Ok(db)
    }

    fn memory_db() -> WorkersDb {
        initialize(Connection::open_in_memory().expect("open in-memory workers DB"))
            .expect("initialize workers DB")
    }

    #[test]
    fn worker_registry_crud_operations_emit_private_result_spans() {
        let db = memory_db();
        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let id = db
                .create(
                    "private-worker-name-marker",
                    "worker-telemetry-custom",
                    "developer",
                    "private-worker-model-marker",
                    &["private-worker-repository-marker".into()],
                    30,
                    Some("private-worker-description-marker"),
                )
                .expect("create worker");
            let by_id = db.get_by_id(id).expect("read by id");
            let by_slug = db
                .get_by_slug("worker-telemetry-custom")
                .expect("read by slug");
            let missing = db
                .get_by_slug("private-missing-worker-slug-marker")
                .expect("read missing slug");
            db.update_model(id, "private-updated-model-marker")
                .expect("update worker model");
            db.update_repos(id, &["private-updated-repository-marker".into()])
                .expect("update worker repositories");
            db.bind_recipe(id, "private-recipe-id-marker")
                .expect("bind worker recipe");
            let exists = db
                .slug_exists("worker-telemetry-custom")
                .expect("check existing slug");
            let absent = db
                .slug_exists("private-absent-worker-slug-marker")
                .expect("check absent slug");
            db.deactivate("worker-telemetry-custom")
                .expect("soft-delete worker");
            db.delete(id)
                .expect("repeat soft-delete through delete API");
            let deleted_lookup = db
                .get_by_slug("worker-telemetry-custom")
                .expect("read soft-deleted worker");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_worker_insert BEFORE INSERT ON workers
                     BEGIN SELECT RAISE(ABORT, 'private-worker-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_create = db
                .create(
                    "private-failed-worker-name-marker",
                    "worker-telemetry-failed",
                    "developer",
                    "private-failed-model-marker",
                    &[],
                    30,
                    None,
                )
                .is_err();
            (
                by_id,
                by_slug,
                missing,
                exists,
                absent,
                id,
                deleted_lookup,
                failed_create,
            )
        });

        assert_eq!(
            observed.0.as_ref().map(|worker| worker.slug.as_str()),
            Some("worker-telemetry-custom")
        );
        assert_eq!(
            observed.1.as_ref().map(|worker| worker.id),
            Some(observed.5)
        );
        assert!(observed.2.is_none());
        assert!(observed.3);
        assert!(!observed.4);
        assert!(observed.6.is_none());
        assert!(observed.7);

        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_worker_create",
            "database_worker_read_by_id",
            "database_worker_read_by_slug",
            "database_worker_model_update",
            "database_worker_repos_update",
            "database_worker_recipe_bind",
            "database_worker_slug_exists",
            "database_worker_deactivate",
            "database_worker_delete",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing worker database span {operation}"
            );
        }
        let missing = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_worker_read_by_slug"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("missing worker is distinguishable");
        assert_eq!(missing["fields"]["result_count"], 0);
        let failed = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_worker_create"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("worker creation failure span");
        assert_eq!(failed["fields"]["result_count_observed"], false);
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_worker_create"
                && event["fields"]["worker"] == "worker-telemetry-custom"
        }));

        let serialized = serde_json::to_string(&events).expect("serialize worker telemetry");
        for private_value in [
            "private-worker-name-marker",
            "private-worker-model-marker",
            "private-worker-repository-marker",
            "private-worker-description-marker",
            "private-updated-model-marker",
            "private-updated-repository-marker",
            "private-recipe-id-marker",
            "private-missing-worker-slug-marker",
            "private-absent-worker-slug-marker",
            "private-failed-worker-name-marker",
            "private-failed-model-marker",
            "private-worker-trigger-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }

    #[test]
    fn worker_database_initializer_span_covers_seed_result_without_paths() {
        let dir = tempfile::tempdir().expect("isolated workers database directory");
        let valid_path = dir.path().join("workers.db");
        let invalid_path = dir.path().join("missing").join("workers.db");
        let ((opened, failed), events) = crate::observability::timing::capture_json_events(|| {
            let opened = WorkersDb::open_at(&valid_path).is_ok();
            let failed = WorkersDb::open_at(&invalid_path).is_err();
            (opened, failed)
        });
        assert!(opened && failed);
        let spans = events
            .iter()
            .filter(|event| event["fields"]["operation"] == "workers_database_initialize")
            .collect::<Vec<_>>();
        assert_eq!(spans.len(), 2);
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "success"));
        assert!(spans
            .iter()
            .any(|event| event["fields"]["outcome"] == "error"));
        let serialized = events.iter().map(ToString::to_string).collect::<String>();
        assert!(!serialized.contains(valid_path.to_string_lossy().as_ref()));
        assert!(!serialized.contains(invalid_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn worker_heartbeat_emits_success_and_missing_worker_results() {
        let db = memory_db();
        let (updated, events) =
            crate::observability::timing::capture_json_events(|| db.heartbeat("worker-backend"));
        updated.expect("seeded worker heartbeat");
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_heartbeat")
            .expect("heartbeat event");
        assert_eq!(event["fields"]["database"], "workers");
        assert_eq!(event["fields"]["worker"], "worker-backend");
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 1);

        let (missing, events) = crate::observability::timing::capture_json_events(|| {
            db.heartbeat("worker-does-not-exist")
        });
        assert!(missing.is_err());
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_heartbeat")
            .expect("failed heartbeat event");
        assert_eq!(event["fields"]["worker"], "worker-does-not-exist");
        assert_eq!(event["fields"]["outcome"], "error");
    }

    #[test]
    fn worker_roster_and_status_operations_emit_safe_result_events() {
        let db = memory_db();
        let worker = db
            .get_by_slug("worker-backend")
            .expect("read seeded worker")
            .expect("backend worker is seeded");

        let ((listed, updated), events) = crate::observability::timing::capture_json_events(|| {
            (db.list(), db.update_status(worker.id, WorkerStatus::Paused))
        });
        assert!(listed.expect("list roster").len() >= 1);
        updated.expect("update worker status");

        let roster = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_roster_read")
            .expect("roster result event");
        assert_eq!(roster["fields"]["database"], "workers");
        assert_eq!(roster["fields"]["outcome"], "success");
        assert!(roster["fields"]["result_count"].as_u64().unwrap_or(0) > 0);
        assert!(roster["fields"]["duration_ns"].as_u64().is_some());

        let update = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_worker_status_update")
            .expect("status result event");
        assert_eq!(update["fields"]["database"], "workers");
        assert_eq!(update["fields"]["outcome"], "success");
        assert_eq!(update["fields"]["result_count"], 1);

        let serialized = serde_json::to_string(&events).expect("serialize events");
        assert!(!serialized.contains("worker-backend"));
    }

    /// The regression this migration exists for.
    ///
    /// Reproduces the live `~/.arcana/workers.db` exactly: a `workers` table
    /// created without `last_seen`. `CREATE TABLE IF NOT EXISTS` cannot repair
    /// that — which is why `arcanactl worker list/status/heartbeat` failed with
    /// *no such column: last_seen* from the day they were written.
    #[test]
    fn a_pre_existing_table_without_last_seen_gains_the_column() {
        // The shipped schema as it exists on disk today, column-for-column.
        let conn = Connection::open_in_memory().expect("open isolated legacy fixture");
        conn.execute_batch(
                "CREATE TABLE workers (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE,
                     worker_type TEXT NOT NULL DEFAULT 'custom',
                     status TEXT NOT NULL DEFAULT 'created',
                     model TEXT NOT NULL DEFAULT 'gemini-3.8-flash',
                     primary_repos TEXT NOT NULL DEFAULT '[]',
                     polling_interval INTEGER NOT NULL DEFAULT 120,
                     recipe_id TEXT, is_system_defined INTEGER NOT NULL DEFAULT 0,
                     description TEXT,
                     created_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                     updated_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                 );
                 INSERT INTO workers (name, slug, status) VALUES ('Backend', 'worker-backend', 'active');",
        )
        .expect("seed legacy schema");

        // Prove the fixture really is missing the column, or the test proves nothing.
        let cols: Vec<String> = {
            let mut st = conn.prepare("PRAGMA table_info(workers)").unwrap();
            let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            r.map(|c| c.unwrap()).collect()
        };
        assert!(!cols.iter().any(|c| c == "last_seen"), "fixture is wrong");

        let db = initialize(conn).expect("migrate isolated legacy DB");

        // The row survived, and the column now exists and is readable.
        let w = db
            .get_by_slug("worker-backend")
            .expect("get_by_slug")
            .expect("the pre-existing row must survive the migration");
        assert_eq!(w.slug, "worker-backend");
        assert_eq!(w.status, WorkerStatus::Active);
        assert_eq!(w.last_seen, None, "a worker that never reported in");

        let invalid_write = db.conn.execute(
            "UPDATE workers SET status = 'unknown' WHERE slug = 'worker-backend'",
            [],
        );
        assert!(
            invalid_write.is_err(),
            "the migrated CHECK must reject invalid writes"
        );
        assert_eq!(
            db.get_by_slug("worker-backend").unwrap().unwrap().status,
            WorkerStatus::Active,
            "the rejected direct write must leave the existing status intact"
        );

        // The legacy database is also brought onto the ladder. This is the
        // shape every live database is in today: schema present, version 0.
        assert_eq!(
            migrate::user_version(&db.conn).unwrap(),
            6,
            "a pre-ladder database must be stamped, not left at 0"
        );
    }

    #[test]
    fn a_fresh_database_is_created_at_the_ladder_head() {
        let db = memory_db();
        assert_eq!(migrate::user_version(&db.conn).unwrap(), 6);
        assert!(db
            .list()
            .unwrap()
            .iter()
            .all(|worker| worker.model == DEFAULT_MODEL));
        // The baseline rung must create the column, not rely on the repair
        // path — otherwise a fresh install differs from a migrated one.
        let cols: Vec<String> = {
            let mut st = db.conn.prepare("PRAGMA table_info(workers)").unwrap();
            let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            r.map(|c| c.unwrap()).collect()
        };
        assert!(cols.iter().any(|c| c == "last_seen"), "got {cols:?}");
        drop(db);
    }

    #[test]
    fn dashboard_worker_summary_uses_date_index_and_preserves_counts() {
        let mut db = memory_db();
        db.conn.execute("DELETE FROM workers", []).unwrap();
        let today = chrono::Local::now().naive_local().date().to_string();
        let yesterday =
            (chrono::Local::now().naive_local().date() - chrono::Duration::days(1)).to_string();
        {
            let tx = db.conn.transaction().unwrap();
            for index in 0..1_000 {
                let created_at = if index % 2 == 0 { &today } else { &yesterday };
                let status = match index % 5 {
                    0 => "deleted",
                    1 => "paused",
                    _ => "idle",
                };
                tx.execute(
                    "INSERT INTO workers (name, slug, status, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        format!("Worker {index}"),
                        format!("worker-{index}"),
                        status,
                        created_at
                    ],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        db.conn.execute_batch("ANALYZE").unwrap();
        db.conn
            .execute_batch("DROP INDEX idx_workers_created_date; ANALYZE")
            .unwrap();

        let plan = |conn: &Connection, sql: &str, date: Option<&str>| -> Vec<String> {
            let mut statement = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
            let mut rows = match date {
                Some(date) => statement.query([date]).unwrap(),
                None => statement.query([]).unwrap(),
            };
            let mut plans = Vec::new();
            while let Some(row) = rows.next().unwrap() {
                plans.push(row.get::<_, String>(3).unwrap());
            }
            plans
        };

        let today_baseline = plan(&db.conn, DASHBOARD_WORKER_ADDED_TODAY_SQL, Some(&today));
        assert!(
            today_baseline
                .iter()
                .any(|detail| detail.contains("SCAN workers")),
            "expected a baseline scan before candidate index: {today_baseline:?}"
        );
        db.conn
            .execute_batch(
                "CREATE INDEX idx_workers_created_date ON workers(date(created_at)); ANALYZE",
            )
            .unwrap();
        let today_indexed = plan(&db.conn, DASHBOARD_WORKER_ADDED_TODAY_SQL, Some(&today));
        assert!(
            today_indexed
                .iter()
                .any(|detail| detail.contains("idx_workers_created_date")),
            "expected the dashboard date index: {today_indexed:?}"
        );

        // These two summary queries count the full logical roster. A table
        // scan is intrinsic to an exact count over mutable status predicates.
        let total_plan = plan(&db.conn, DASHBOARD_WORKER_TOTAL_SQL, None);
        let active_plan = plan(&db.conn, DASHBOARD_WORKER_ACTIVE_SQL, None);
        assert!(total_plan
            .iter()
            .any(|detail| detail.contains("SCAN workers")));
        assert!(active_plan
            .iter()
            .any(|detail| detail.contains("SCAN workers")));

        let total: i32 = db
            .conn
            .query_row(DASHBOARD_WORKER_TOTAL_SQL, [], |row| row.get(0))
            .unwrap();
        let active: i32 = db
            .conn
            .query_row(DASHBOARD_WORKER_ACTIVE_SQL, [], |row| row.get(0))
            .unwrap();
        let added: i32 = db
            .conn
            .query_row(DASHBOARD_WORKER_ADDED_TODAY_SQL, [today], |row| row.get(0))
            .unwrap();
        assert_eq!((total, active, added), (800, 600, 500));
    }

    #[test]
    fn worker_roster_uses_measured_order_index_and_returns_full_non_deleted_roster() {
        let mut db = memory_db();
        db.conn.execute("DELETE FROM workers", []).unwrap();
        {
            let tx = db.conn.transaction().unwrap();
            for index in 0..2_000 {
                tx.execute(
                    "INSERT INTO workers (name, slug, status, is_system_defined)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        format!("Worker {index}"),
                        format!("roster-{index}"),
                        if index % 10 == 0 { "deleted" } else { "idle" },
                        (index % 2 == 0) as i64
                    ],
                )
                .unwrap();
            }
            tx.commit().unwrap();
        }
        db.conn.execute_batch("ANALYZE").unwrap();
        db.conn
            .execute_batch("DROP INDEX idx_workers_active_roster_order; ANALYZE")
            .unwrap();

        let explain = |conn: &Connection| {
            let mut statement = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {WORKER_ROSTER_SQL}"))
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap()
        };
        let baseline = explain(&db.conn);
        assert!(
            baseline
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "baseline roster should sort its required full result: {baseline:?}"
        );

        db.conn
            .execute_batch(
                "CREATE INDEX idx_workers_active_roster_order
                    ON workers(is_system_defined DESC, id) WHERE status != 'deleted'; ANALYZE",
            )
            .unwrap();
        let indexed = explain(&db.conn);
        assert!(
            indexed
                .iter()
                .any(|detail| detail.contains("idx_workers_active_roster_order")),
            "worker roster should use the measured order index: {indexed:?}"
        );
        assert!(
            !indexed
                .iter()
                .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
            "worker roster should not sort after candidate index: {indexed:?}"
        );

        let roster = db.list().expect("read production worker roster");
        assert_eq!(roster.len(), 1_800);
        assert!(roster
            .iter()
            .all(|worker| worker.status != WorkerStatus::Deleted));
        assert!(roster
            .windows(2)
            .all(|pair| pair[0].is_system_defined >= pair[1].is_system_defined));
        assert!(roster.windows(2).all(|pair| {
            pair[0].is_system_defined != pair[1].is_system_defined || pair[0].id < pair[1].id
        }));
        let mut slug_plan = db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {WORKER_BY_SLUG_SQL}"))
            .unwrap();
        let slug_plan = slug_plan
            .query_map(["roster-1"], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            slug_plan
                .iter()
                .any(|detail| detail.contains("idx_workers_slug")
                    || detail.contains("sqlite_autoindex_workers_1")),
            "worker slug lookup should use the unique slug index: {slug_plan:?}"
        );
        let by_slug = db
            .get_by_slug("roster-1")
            .expect("production worker slug lookup")
            .expect("existing worker slug");
        assert_eq!(by_slug.slug, "roster-1");
        assert_eq!(by_slug.status, WorkerStatus::Idle);
    }

    #[test]
    fn worker_detail_by_id_uses_primary_key_and_returns_only_non_deleted_worker() {
        let mut db = memory_db();
        db.conn.execute("DELETE FROM workers", []).unwrap();
        {
            let tx = db.conn.transaction().unwrap();
            for id in 1..=2_048_i64 {
                let status = if id == 1_025 {
                    "deleted"
                } else if id % 3 == 0 {
                    "active"
                } else {
                    "idle"
                };
                if id == 1_024 {
                    tx.execute(
                        "INSERT INTO workers
                             (id, name, slug, worker_type, status, model, primary_repos,
                              polling_interval, recipe_id, is_system_defined, description,
                              created_at, updated_at, last_seen)
                         VALUES (?1, 'Detail worker', 'detail-worker', 'custom', 'active',
                                 'test-model', '[\"repo-a\",\"repo-b\"]', 45,
                                 'recipe-detail', 1, 'populated lookup fixture',
                                 '2026-01-02T03:04:05Z', '2026-02-03T04:05:06Z',
                                 '2026-03-04T05:06:07Z')",
                        [id],
                    )
                    .unwrap();
                } else {
                    tx.execute(
                        "INSERT INTO workers (id, name, slug, status)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![id, format!("Worker {id}"), format!("worker-{id}"), status],
                    )
                    .unwrap();
                }
            }
            tx.commit().unwrap();
        }
        db.conn.execute_batch("ANALYZE").unwrap();

        // Keep the plan assertion tied to the SQL used by get_by_id below.
        let mut statement = db
            .conn
            .prepare(
                "EXPLAIN QUERY PLAN SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at, last_seen \
             FROM workers WHERE id = ?1 AND status != 'deleted'",
            )
            .expect("prepare production worker detail query plan");
        let plan = statement
            .query_map([1_024_i64], |row| row.get::<_, String>(3))
            .expect("explain worker detail lookup")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect worker detail plan");
        assert!(
            plan.iter()
                .any(|detail| detail.contains("SEARCH workers USING INTEGER PRIMARY KEY")),
            "worker detail lookup should use the primary key: {plan:?}"
        );

        let worker = db
            .get_by_id(1_024)
            .expect("production worker detail reader")
            .expect("active worker row");
        assert_eq!(worker.id, 1_024);
        assert_eq!(worker.name, "Detail worker");
        assert_eq!(worker.slug, "detail-worker");
        assert_eq!(worker.worker_type, "custom");
        assert_eq!(worker.status, WorkerStatus::Active);
        assert_eq!(worker.model, "test-model");
        assert_eq!(worker.primary_repos, vec!["repo-a", "repo-b"]);
        assert_eq!(worker.polling_interval, 45);
        assert_eq!(worker.recipe_id.as_deref(), Some("recipe-detail"));
        assert!(worker.is_system_defined);
        assert_eq!(
            worker.description.as_deref(),
            Some("populated lookup fixture")
        );
        assert_eq!(worker.created_at, "2026-01-02T03:04:05.000Z");
        assert!(
            !worker.updated_at.is_empty(),
            "the detail row should include its database-maintained update time"
        );
        assert_eq!(
            worker.last_seen.as_deref(),
            Some("2026-03-04T05:06:07.000Z")
        );

        assert!(
            db.get_by_id(9_999)
                .expect("missing worker lookup")
                .is_none(),
            "unknown worker IDs should return no row"
        );
        assert!(
            db.get_by_id(1_025)
                .expect("deleted worker lookup")
                .is_none(),
            "deleted workers should not be returned by the detail reader"
        );
    }

    #[test]
    fn model_default_migration_updates_seeded_workers_but_preserves_custom_workers() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE workers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                slug TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'created',
                model TEXT NOT NULL,
                is_system_defined INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             );
             INSERT INTO workers (slug, model) VALUES
               ('supervisor', 'claude-opus-5'),
               ('qc-agent', 'claude-haiku-4-5'),
               ('worker-backend', 'claude-sonnet-5'),
               ('worker-frontend', 'claude-sonnet-5'),
               ('worker-platform', 'claude-sonnet-5'),
               ('worker-mcp', 'claude-opus-5'),
               ('worker-cleanup', 'claude-haiku-4-5'),
               ('my-custom-worker', 'claude-sonnet-5');
             PRAGMA user_version = 2;",
        )
        .unwrap();

        migrate::run_ladder(&conn, "workers.db", WorkersDb::LADDER).unwrap();
        let seeded: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM workers WHERE slug != 'my-custom-worker' AND model = ?1",
                params![DEFAULT_MODEL],
                |row| row.get(0),
            )
            .unwrap();
        let custom_model: String = conn
            .query_row(
                "SELECT model FROM workers WHERE slug = 'my-custom-worker'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(seeded, 7);
        assert_eq!(custom_model, "claude-sonnet-5");
    }

    #[test]
    fn unknown_statuses_fail_during_legacy_row_hydration() {
        let conn = Connection::open_in_memory().expect("open isolated legacy fixture");
        conn.execute_batch(
            "CREATE TABLE workers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE,
                    worker_type TEXT NOT NULL DEFAULT 'custom',
                    status TEXT NOT NULL DEFAULT 'created',
                    model TEXT NOT NULL DEFAULT 'gemini-3.8-flash',
                    primary_repos TEXT NOT NULL DEFAULT '[]',
                    polling_interval INTEGER NOT NULL DEFAULT 120,
                    recipe_id TEXT, is_system_defined INTEGER NOT NULL DEFAULT 0,
                    description TEXT,
                    created_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    updated_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    last_seen TEXT
                );
                INSERT INTO workers (name, slug, status)
                    VALUES ('Legacy', 'legacy-worker', 'mystery');
                PRAGMA user_version = 2;",
        )
        .expect("seed legacy fixture with invalid status");

        let db = initialize(conn).expect("open fixture without rewriting its v2 marker");
        let error = db
            .get_by_slug("legacy-worker")
            .expect_err("an unknown status must fail at row hydration");
        assert!(
            error
                .to_string()
                .contains("unknown worker status 'mystery'"),
            "unexpected hydration error: {error}"
        );
        drop(db);
    }

    #[test]
    fn migration_refuses_and_preserves_unknown_legacy_statuses() {
        let conn = Connection::open_in_memory().expect("open isolated legacy fixture");
        conn.execute_batch(
            "CREATE TABLE workers (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE,
                    worker_type TEXT NOT NULL DEFAULT 'custom',
                    status TEXT NOT NULL DEFAULT 'created',
                    model TEXT NOT NULL DEFAULT 'gemini-3.8-flash',
                    primary_repos TEXT NOT NULL DEFAULT '[]',
                    polling_interval INTEGER NOT NULL DEFAULT 120,
                    recipe_id TEXT, is_system_defined INTEGER NOT NULL DEFAULT 0,
                    description TEXT,
                    created_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    updated_at DATETIME DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    last_seen TEXT
                );
                INSERT INTO workers (name, slug, status)
                    VALUES ('Legacy', 'legacy-worker', 'mystery');
                PRAGMA user_version = 1;",
        )
        .expect("seed isolated legacy fixture");

        let error = migrate::run_ladder(&conn, "workers.db", WorkersDb::LADDER)
            .err()
            .expect("migration must reject a row that cannot satisfy the new CHECK");
        assert!(error.to_string().contains("worker-status-check"), "{error}");
        let version: i32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("read migration version");
        let status: String = conn
            .query_row(
                "SELECT status FROM workers WHERE slug = 'legacy-worker'",
                [],
                |row| row.get(0),
            )
            .expect("read preserved legacy state");
        assert_eq!(version, 1, "failed migration must not advance its version");
        assert_eq!(
            status, "mystery",
            "failed migration must preserve source data"
        );
    }

    #[test]
    fn status_migration_explains_its_bounded_full_copy_and_preserves_rows() {
        let conn = Connection::open_in_memory().expect("open isolated legacy fixture");
        migrate::run_ladder(&conn, "workers.db", &WorkersDb::LADDER[..1])
            .expect("create legacy v1 schema");
        for id in 0..300 {
            conn.execute(
                "INSERT INTO workers (name, slug, status, model, description)
                 VALUES (?1, ?2, 'idle', ?3, ?4)",
                params![
                    format!("Legacy Worker {id}"),
                    format!("legacy-worker-{id}"),
                    format!("model-{id}"),
                    format!("description-{id}"),
                ],
            )
            .expect("seed legacy worker row");
        }
        conn.execute_batch("ANALYZE")
            .expect("analyze legacy fixture");

        let migration = &WorkersDb::LADDER[1];
        let create_start = migration
            .sql
            .find("CREATE TABLE workers_status_migration")
            .expect("migration creates the staging table");
        let create_end = create_start
            + migration.sql[create_start..]
                .find(");")
                .expect("staging table DDL terminator")
            + 2;
        let create_staging = &migration.sql[create_start..create_end];
        conn.execute_batch(create_staging)
            .expect("create temporary migration staging table");

        let copy_start = migration
            .sql
            .find("INSERT INTO workers_status_migration")
            .expect("migration contains its production copy statement");
        let copy_end = copy_start
            + migration.sql[copy_start..]
                .find(';')
                .expect("copy statement terminator");
        let copy_sql = &migration.sql[copy_start..copy_end];
        let explain_sql = format!("EXPLAIN QUERY PLAN {copy_sql}");
        let plan = conn
            .prepare(&explain_sql)
            .expect("prepare production migration plan")
            .query_map([], |row| row.get::<_, String>(3))
            .expect("explain production migration copy")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect migration plan");
        assert!(
            plan.iter().any(|step| step.contains("SCAN workers")),
            "the one-time table-rebuild migration intentionally copies the complete worker roster; plan: {plan:?}"
        );
        conn.execute_batch("DROP TABLE workers_status_migration")
            .expect("remove temporary planning table");

        migrate::run_ladder(&conn, "workers.db", WorkersDb::LADDER)
            .expect("apply the production status migration");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM workers", [], |row| row.get(0))
            .expect("count migrated rows");
        assert_eq!(count, 300);
        let migrated: (String, String, String, String) = conn
            .query_row(
                "SELECT name, slug, model, description FROM workers WHERE id = 151",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read migrated worker fields");
        assert_eq!(
            migrated,
            (
                "Legacy Worker 150".into(),
                "legacy-worker-150".into(),
                "model-150".into(),
                "description-150".into(),
            )
        );
    }

    #[test]
    fn strict_parser_accepts_each_status_and_rejects_unknown_values() {
        let statuses = [
            ("created", WorkerStatus::Created),
            ("idle", WorkerStatus::Idle),
            ("idle_no_provider", WorkerStatus::IdleNoProvider),
            ("active", WorkerStatus::Active),
            ("working", WorkerStatus::Working),
            ("paused", WorkerStatus::Paused),
            ("error", WorkerStatus::Error),
            ("deleted", WorkerStatus::Deleted),
        ];
        for (value, expected) in statuses {
            assert_eq!(WorkerStatus::try_from_str(value), Ok(expected));
            assert_eq!(
                serde_json::to_string(&expected).unwrap(),
                format!("\"{value}\"")
            );
        }
        assert!(WorkerStatus::try_from_str("running").is_err());
    }

    #[test]
    fn registered_roster_includes_custom_workers_and_excludes_deleted_rows() {
        let db = memory_db();
        let id = db
            .create(
                "Custom",
                "custom-coder",
                "custom",
                "gemini-3.7-flash",
                &[],
                120,
                None,
            )
            .expect("create custom worker");
        let slugs = db.registered_slugs().expect("roster");
        assert!(slugs.iter().any(|slug| slug == "custom-coder"));
        db.update_status(id, WorkerStatus::Deleted)
            .expect("delete worker");
        assert!(!db
            .registered_slugs()
            .unwrap()
            .iter()
            .any(|slug| slug == "custom-coder"));
    }

    #[test]
    fn the_column_migration_is_idempotent() {
        let db = memory_db();
        migrate::run_ladder(&db.conn, "workers.db", WorkersDb::LADDER)
            .expect("running the current ladder again must be a no-op");
        assert!(!db.list().expect("list").is_empty(), "seeds should exist");
    }

    #[test]
    fn a_heartbeat_is_recorded_and_readable() {
        let db = memory_db();
        let slug = db.list().expect("list")[0].slug.clone();

        assert_eq!(db.get_by_slug(&slug).unwrap().unwrap().last_seen, None);
        db.heartbeat(&slug).expect("heartbeat");

        // Prove the write is readable back — a heartbeat nothing can read is
        // not a heartbeat.
        let after = db.get_by_slug(&slug).unwrap().unwrap();
        assert!(
            after.last_seen.is_some(),
            "heartbeat did not persist a last_seen"
        );
        assert_eq!(after.status, WorkerStatus::Active);
    }

    #[test]
    fn a_heartbeat_for_an_unknown_worker_is_an_error_not_a_silent_no_op() {
        let db = memory_db();
        let err = db.heartbeat("does-not-exist").expect_err("must fail");
        assert!(err.to_string().contains("does-not-exist"), "got: {err}");
    }

    #[test]
    fn a_heartbeat_preserves_working_paused_and_error_lifecycle_states() {
        let db = memory_db();
        let worker = db.list().expect("list")[0].clone();

        for status in [
            WorkerStatus::Working,
            WorkerStatus::Paused,
            WorkerStatus::Error,
        ] {
            db.update_status(worker.id, status)
                .expect("set lifecycle status");
            db.heartbeat(&worker.slug).expect("record heartbeat");

            let after = db
                .get_by_slug(&worker.slug)
                .expect("read worker")
                .expect("worker exists");
            assert_eq!(after.status, status, "heartbeat changed {status:?}");
            assert!(after.last_seen.is_some(), "heartbeat was not persisted");
        }
    }

    /// `unregister` has always been reversible. This asserts the row is still
    /// there after it, so a future refactor cannot quietly route it to
    /// `delete` and turn it into permanent loss.
    #[test]
    fn deactivate_hides_a_worker_without_deleting_the_row() {
        let db = memory_db();
        let before = db.list().expect("list").len();
        let slug = db.list().expect("list")[0].slug.clone();

        db.deactivate(&slug).expect("deactivate");
        assert_eq!(db.list().expect("list").len(), before - 1, "still listed");

        // `get_by_slug` filters `status != 'deleted'` by design, so survival is
        // asserted against the table itself. Reading it through an accessor
        // that hides deleted rows could not tell a soft delete from a hard one
        // — which is the entire property under test.
        let (count, status): (i64, String) = db
            .conn
            .query_row(
                "SELECT COUNT(*), MAX(status) FROM workers WHERE slug = ?1",
                params![slug],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("raw query");
        assert_eq!(count, 1, "the row must survive a soft delete");
        assert_eq!(status, "deleted");
        assert!(
            db.get_by_slug(&slug).expect("get_by_slug").is_none(),
            "a deactivated worker must not come back from get_by_slug"
        );

        assert!(
            db.deactivate(&slug).is_err(),
            "deactivating twice must report that there was nothing to do"
        );
    }

    /// A constant that merely *claims* to match the schema is worth nothing.
    /// This inserts a row without naming a model and reads back what SQLite
    /// chose, so the two cannot drift apart silently.
    #[test]
    fn the_default_model_constant_matches_the_schema_default() {
        let db = memory_db();
        db.conn
            .execute(
                "INSERT INTO workers (name, slug) VALUES ('Probe', 'probe-default-model')",
                [],
            )
            .expect("insert");
        let chosen: String = db
            .conn
            .query_row(
                "SELECT model FROM workers WHERE slug = 'probe-default-model'",
                [],
                |r| r.get(0),
            )
            .expect("read back");
        assert_eq!(
            chosen, DEFAULT_MODEL,
            "the schema default and DEFAULT_MODEL have drifted"
        );
    }

    #[test]
    fn production_worker_reader_returns_canonical_timestamps_and_keeps_legacy_rows() {
        let dir = tempfile::tempdir().expect("isolated temp directory");
        let _home = crate::db::test_support::with_home(dir.path());
        let path = dir.path().join("workers.db");
        let db = WorkersDb::open_at(&path).expect("open isolated production database");
        let id = db
            .create(
                "Timestamp Probe",
                "timestamp-probe",
                "custom",
                DEFAULT_MODEL,
                &[],
                120,
                None,
            )
            .expect("create through production writer");
        db.heartbeat("timestamp-probe").expect("heartbeat");
        let worker = db.get_by_slug("timestamp-probe").unwrap().unwrap();
        for value in [
            &worker.created_at,
            &worker.updated_at,
            worker.last_seen.as_ref().unwrap(),
        ] {
            assert_eq!(value.len(), 24, "expected fixed millisecond UTC: {value}");
            assert!(value.ends_with('Z'), "expected UTC Z suffix: {value}");
            assert!(chrono::DateTime::parse_from_rfc3339(value).is_ok());
        }

        db.conn
            .execute_batch(
                "DROP TRIGGER workers_timestamps_canonical_update;
                 PRAGMA user_version = 3;",
            )
            .expect("prepare old-format row");
        db.conn
            .execute(
                "UPDATE workers SET created_at = '2026-01-02 03:04:05',
                                    last_seen = '2026-01-02T03:04:05.123456'
                 WHERE id = ?1",
                params![id],
            )
            .expect("write legacy timestamp fixture");
        db.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint legacy fixture before copying");
        drop(db);

        let upgrade_dir = dir.path().join("upgrade-home");
        std::fs::create_dir_all(&upgrade_dir).expect("create isolated upgrade fixture");
        let upgrade_path = upgrade_dir.join("workers.db");
        std::fs::copy(&path, &upgrade_path).expect("copy the pre-upgrade database fixture");
        drop(_home);
        let _upgrade_home = crate::db::test_support::with_home(&upgrade_dir);
        let upgraded = WorkersDb::open_at(&upgrade_path).expect("run additive v4 migration");
        let legacy = upgraded.get_by_slug("timestamp-probe").unwrap().unwrap();
        assert_eq!(legacy.created_at, "2026-01-02 03:04:05");
        assert_eq!(
            legacy.last_seen.as_deref(),
            Some("2026-01-02T03:04:05.123456")
        );
        assert!(crate::db::timestamps::parse_db_timestamp(&legacy.created_at).is_ok());
        assert!(
            crate::db::timestamps::parse_db_timestamp(legacy.last_seen.as_deref().unwrap()).is_ok()
        );
        drop(upgraded);
    }
}
