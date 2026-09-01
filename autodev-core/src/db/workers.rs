// Workers database — SQLite CRUD for the Worker registry.
// Database path: ~/.autodev/workers.db
// Seeds pre-defined workers + system agents on first open.
// Used by: Worker Management UI, Agent Roster, Supervisor recipe (list_workers__autodev)
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

use super::autodev_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Created, // Seeded but not yet started
    Idle,    // Started, polling, no current task
    Active,  // Running — recipe cron is active
    Working, // Executing a task
    Paused,  // User paused
    Error,   // Recipe crashed / last run failed
    Deleted, // Soft-deleted
}

impl WorkerStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Created => "created",
            Self::Idle => "idle",
            Self::Active => "active",
            Self::Working => "working",
            Self::Paused => "paused",
            Self::Error => "error",
            Self::Deleted => "deleted",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "idle" => Self::Idle,
            "active" => Self::Active,
            "working" => Self::Working,
            "paused" => Self::Paused,
            "error" => Self::Error,
            "deleted" => Self::Deleted,
            _ => Self::Created,
        }
    }
}

/// The model a worker gets when a caller does not choose one.
///
/// Must equal the `model` column's DEFAULT in `migrate()`. A caller that
/// hardcodes its own string instead — as `airsctl worker register` would have
/// had to — drifts from the schema the moment either changes, and the drift is
/// invisible because both produce a valid-looking worker.
/// `the_default_model_constant_matches_the_schema_default` pins them together.
pub const DEFAULT_MODEL: &str = "claude-sonnet-4-5";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id: i64,
    pub name: String,
    pub slug: String, // kebab-case unique identifier
    pub worker_type: String,
    pub status: String,
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
    /// Added in P1-09. `airsctl` had been reading and writing this column since
    /// it was written, against a table that never had it — see the migration
    /// note on `add_missing_columns`.
    pub last_seen: Option<String>,
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct WorkersDb {
    conn: Connection,
}

impl WorkersDb {
    /// Open (or create) ~/.autodev/workers.db and run migrations + seed.
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
        Self::open_at(&autodev_dir()?.join("workers.db"))
    }

    /// As `open`, against an explicit path.
    ///
    /// Exists so tests never touch the user's real `~/.autodev/workers.db`.
    /// An earlier helper in this tree did exactly that, and one failed test
    /// left a file behind that made every later run fail — an order-dependent
    /// suite that also wrote to live user data.
    pub fn open_at(db_path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(db_path)
            .map_err(|e| AutoDevError::Agent(format!("workers.db open failed: {e}")))?;
        let db = Self { conn };
        // Order matters: repair a pre-ladder database's columns first, so the
        // baseline rung is a genuine no-op against it, then stamp the version.
        db.add_missing_columns()?; // P1-09 — columns added to an already-created table
        migrate::run_ladder(&db.conn, "workers.db", Self::LADDER)?;
        db.migrate_model_names()?; // Phase 7.1 — fix any stale old-format model slugs
        db.seed_defaults()?;
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
    const LADDER: &[Migration] = &[Migration {
        version: 1,
        name: "workers-baseline",
        sql: r#"
            CREATE TABLE IF NOT EXISTS workers (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                name              TEXT NOT NULL,
                slug              TEXT NOT NULL UNIQUE,
                worker_type       TEXT NOT NULL DEFAULT 'custom',
                status            TEXT NOT NULL DEFAULT 'created',
                model             TEXT NOT NULL DEFAULT 'claude-sonnet-4-5',
                primary_repos     TEXT NOT NULL DEFAULT '[]',
                polling_interval  INTEGER NOT NULL DEFAULT 120,
                recipe_id         TEXT,
                is_system_defined INTEGER NOT NULL DEFAULT 0,
                description       TEXT,
                created_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_seen         TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_workers_slug   ON workers(slug);
            CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
            CREATE TRIGGER IF NOT EXISTS update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                END;
        "#,
    }];

    /// Columns added to `workers` after its original `CREATE TABLE`.
    ///
    /// `src-shared` declared `last_seen` in its own
    /// `CREATE TABLE IF NOT EXISTS workers`, but the table had already been
    /// created without it, so the statement was a silent no-op and every
    /// `airsctl worker` call failed with *no such column*
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
        let rows = self
            .conn
            .execute(
                "UPDATE workers SET last_seen = datetime('now'), status = 'active'                  WHERE slug = ?1 AND status != 'deleted'",
                params![slug],
            )
            .map_err(|e| AutoDevError::Agent(format!("heartbeat: {e}")))?;
        if rows == 0 {
            return Err(AutoDevError::Agent(format!(
                "No active worker '{slug}' to record a heartbeat for"
            )));
        }
        Ok(())
    }

    /// Soft-delete a worker: mark it `deleted` so `list` stops returning it.
    ///
    /// Deliberately **not** routed to `delete`, which removes the row outright
    /// and refuses system-defined workers. `airsctl worker unregister` has
    /// always been reversible, and mapping it onto a hard delete during this
    /// consolidation would have turned an undo-able command into permanent
    /// loss of live rows without anyone asking for that.
    pub fn deactivate(&self, slug: &str) -> Result<()> {
        let rows = self
            .conn
            .execute(
                "UPDATE workers SET status = 'deleted' WHERE slug = ?1 AND status != 'deleted'",
                params![slug],
            )
            .map_err(|e| AutoDevError::Agent(format!("deactivate: {e}")))?;
        if rows == 0 {
            return Err(AutoDevError::Agent(format!(
                "No active worker '{slug}' to unregister"
            )));
        }
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
        "#,
            )
            .map_err(|e| AutoDevError::Agent(format!("migrate_model_names: {e}")))?;
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
                "claude-opus-4-5",
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
                "claude-haiku-4-5",
                60,
                true,
                "Dispatches per-task QC reviewers (Doc 04 Phase 3).",
                Some("qc-dispatcher"),
            ),
            (
                "Worker Backend",
                "worker-backend",
                "backend",
                "claude-sonnet-4-5",
                120,
                false,
                "Implements backend tasks (Python / Rust).",
                Some("worker-backend-poll"),
            ),
            (
                "Worker Frontend",
                "worker-frontend",
                "frontend",
                "claude-sonnet-4-5",
                120,
                false,
                "Implements frontend tasks (React / TypeScript).",
                Some("worker-frontend-poll"),
            ),
            (
                "Worker Platform",
                "worker-platform",
                "platform",
                "claude-sonnet-4-5",
                120,
                false,
                "Infrastructure, CI/CD, and DevOps tasks.",
                Some("worker-platform-poll"),
            ),
            (
                "Worker MCP",
                "worker-mcp",
                "mcp",
                "claude-opus-4-5",
                120,
                false,
                "Builds and maintains MCP server extensions.",
                Some("worker-mcp-poll"),
            ),
            (
                "Worker Cleanup",
                "worker-cleanup",
                "cleanup",
                "claude-haiku-4-5",
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
            ).map_err(|e| AutoDevError::Agent(format!("seed workers: {e}")))?;
        }
        Ok(())
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn list(&self) -> Result<Vec<Worker>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at, last_seen \
             FROM workers WHERE status != 'deleted' ORDER BY is_system_defined DESC, id ASC",
            )
            .map_err(|e| AutoDevError::Agent(format!("workers list prepare: {e}")))?;

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
            .map_err(|e| AutoDevError::Agent(format!("workers list query: {e}")))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("workers list collect: {e}")))
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Worker>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at, last_seen \
             FROM workers WHERE id = ?1 AND status != 'deleted'",
            )
            .map_err(|e| AutoDevError::Agent(format!("get_by_id prepare: {e}")))?;

        stmt.query_row(params![id], |row| {
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
        .map_err(|e| AutoDevError::Agent(format!("get_by_id: {e}")))
    }

    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Worker>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at, last_seen \
             FROM workers WHERE slug = ?1 AND status != 'deleted'",
            )
            .map_err(|e| AutoDevError::Agent(format!("get_by_slug prepare: {e}")))?;

        stmt.query_row(params![slug], |row| {
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
        .map_err(|e| AutoDevError::Agent(format!("get_by_slug: {e}")))
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
        let repos_json = serde_json::to_string(primary_repos)
            .map_err(|e| AutoDevError::Agent(format!("repos json: {e}")))?;
        self.conn.execute(
            "INSERT INTO workers (name, slug, worker_type, model, primary_repos, polling_interval, description) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![name, slug, worker_type, model, repos_json, polling_interval, description],
        ).map_err(|e| AutoDevError::Agent(format!("create worker: {e}")))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE workers SET status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_status: {e}")))?;
        Ok(())
    }

    pub fn update_model(&self, id: i64, model: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE workers SET model = ?1 WHERE id = ?2",
                params![model, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_model: {e}")))?;
        Ok(())
    }

    pub fn update_repos(&self, id: i64, primary_repos: &[String]) -> Result<()> {
        let repos_json = serde_json::to_string(primary_repos)
            .map_err(|e| AutoDevError::Agent(format!("repos json: {e}")))?;
        self.conn
            .execute(
                "UPDATE workers SET primary_repos = ?1 WHERE id = ?2",
                params![repos_json, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("update_repos: {e}")))?;
        Ok(())
    }

    pub fn bind_recipe(&self, id: i64, recipe_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE workers SET recipe_id = ?1 WHERE id = ?2",
                params![recipe_id, id],
            )
            .map_err(|e| AutoDevError::Agent(format!("bind_recipe: {e}")))?;
        Ok(())
    }

    /// Soft-delete — system agents cannot be deleted.
    pub fn delete(&self, id: i64) -> Result<()> {
        let is_system: i64 = self
            .conn
            .query_row(
                "SELECT is_system_defined FROM workers WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| AutoDevError::Agent(format!("delete check: {e}")))?
            .unwrap_or(0);
        if is_system != 0 {
            return Err(AutoDevError::Agent(
                "Cannot delete system-defined agents".into(),
            ));
        }
        self.conn
            .execute(
                "UPDATE workers SET status = 'deleted' WHERE id = ?1",
                params![id],
            )
            .map_err(|e| AutoDevError::Agent(format!("delete worker: {e}")))?;
        Ok(())
    }

    pub fn slug_exists(&self, slug: &str) -> Result<bool> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM workers WHERE slug = ?1 AND status != 'deleted'",
                params![slug],
                |r| r.get(0),
            )
            .map_err(|e| AutoDevError::Agent(format!("slug_exists: {e}")))?;
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

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("autodev-workers-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir.join("workers.db")
    }

    /// The regression this migration exists for.
    ///
    /// Reproduces the live `~/.autodev/workers.db` exactly: a `workers` table
    /// created without `last_seen`. `CREATE TABLE IF NOT EXISTS` cannot repair
    /// that — which is why `airsctl worker list/status/heartbeat` failed with
    /// *no such column: last_seen* from the day they were written.
    #[test]
    fn a_pre_existing_table_without_last_seen_gains_the_column() {
        let path = temp_db("legacy");

        // The shipped schema as it exists on disk today, column-for-column.
        {
            let conn = Connection::open(&path).expect("open");
            conn.execute_batch(
                "CREATE TABLE workers (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE,
                     worker_type TEXT NOT NULL DEFAULT 'custom',
                     status TEXT NOT NULL DEFAULT 'created',
                     model TEXT NOT NULL DEFAULT 'claude-sonnet-4-5',
                     primary_repos TEXT NOT NULL DEFAULT '[]',
                     polling_interval INTEGER NOT NULL DEFAULT 120,
                     recipe_id TEXT, is_system_defined INTEGER NOT NULL DEFAULT 0,
                     description TEXT,
                     created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                     updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
                 );
                 INSERT INTO workers (name, slug) VALUES ('Backend', 'worker-backend');",
            )
            .expect("seed legacy schema");

            // Prove the fixture really is missing the column, or the test proves nothing.
            let cols: Vec<String> = {
                let mut st = conn.prepare("PRAGMA table_info(workers)").unwrap();
                let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
                r.map(|c| c.unwrap()).collect()
            };
            assert!(!cols.iter().any(|c| c == "last_seen"), "fixture is wrong");
        }

        let db = WorkersDb::open_at(&path).expect("open_at");

        // The row survived, and the column now exists and is readable.
        let w = db
            .get_by_slug("worker-backend")
            .expect("get_by_slug")
            .expect("the pre-existing row must survive the migration");
        assert_eq!(w.slug, "worker-backend");
        assert_eq!(w.last_seen, None, "a worker that never reported in");

        // The legacy database is also brought onto the ladder. This is the
        // shape every live database is in today: schema present, version 0.
        assert_eq!(
            migrate::user_version(&db.conn).unwrap(),
            1,
            "a pre-ladder database must be stamped, not left at 0"
        );

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_fresh_database_is_created_at_the_ladder_head() {
        let path = temp_db("fresh-version");
        let db = WorkersDb::open_at(&path).expect("open");
        assert_eq!(migrate::user_version(&db.conn).unwrap(), 1);
        // The baseline rung must create the column, not rely on the repair
        // path — otherwise a fresh install differs from a migrated one.
        let cols: Vec<String> = {
            let mut st = db.conn.prepare("PRAGMA table_info(workers)").unwrap();
            let r = st.query_map([], |r| r.get::<_, String>(1)).unwrap();
            r.map(|c| c.unwrap()).collect()
        };
        assert!(cols.iter().any(|c| c == "last_seen"), "got {cols:?}");
        drop(db);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_column_migration_is_idempotent() {
        let path = temp_db("idempotent");
        drop(WorkersDb::open_at(&path).expect("first open"));
        // Opening again must not fail with "duplicate column name".
        let db = WorkersDb::open_at(&path).expect("second open must succeed");
        assert!(!db.list().expect("list").is_empty(), "seeds should exist");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_heartbeat_is_recorded_and_readable() {
        let path = temp_db("heartbeat");
        let db = WorkersDb::open_at(&path).expect("open");
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
        assert_eq!(after.status, "active");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_heartbeat_for_an_unknown_worker_is_an_error_not_a_silent_no_op() {
        let path = temp_db("heartbeat-missing");
        let db = WorkersDb::open_at(&path).expect("open");
        let err = db.heartbeat("does-not-exist").expect_err("must fail");
        assert!(err.to_string().contains("does-not-exist"), "got: {err}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// `unregister` has always been reversible. This asserts the row is still
    /// there after it, so a future refactor cannot quietly route it to
    /// `delete` and turn it into permanent loss.
    #[test]
    fn deactivate_hides_a_worker_without_deleting_the_row() {
        let path = temp_db("deactivate");
        let db = WorkersDb::open_at(&path).expect("open");
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

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A constant that merely *claims* to match the schema is worth nothing.
    /// This inserts a row without naming a model and reads back what SQLite
    /// chose, so the two cannot drift apart silently.
    #[test]
    fn the_default_model_constant_matches_the_schema_default() {
        let path = temp_db("default-model");
        let db = WorkersDb::open_at(&path).expect("open");
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
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
