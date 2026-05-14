// Workers database — SQLite CRUD for the Worker registry.
// Database path: ~/.autodev/workers.db
// Seeds pre-defined workers + system agents on first open.
// Used by: Worker Management UI, Agent Roster, Supervisor recipe (list_workers__autodev)
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use super::autodev_dir;

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
            Self::Backend    => "backend",
            Self::Frontend   => "frontend",
            Self::Platform   => "platform",
            Self::Mcp        => "mcp",
            Self::Cleanup    => "cleanup",
            Self::Supervisor => "supervisor",
            Self::Qc         => "qc",
            Self::Custom     => "custom",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "backend"    => Self::Backend,
            "frontend"   => Self::Frontend,
            "platform"   => Self::Platform,
            "mcp"        => Self::Mcp,
            "cleanup"    => Self::Cleanup,
            "supervisor" => Self::Supervisor,
            "qc"         => Self::Qc,
            _            => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    Created,  // Seeded but not yet started
    Idle,     // Started, polling, no current task
    Active,   // Running — recipe cron is active
    Working,  // Executing a task
    Paused,   // User paused
    Error,    // Recipe crashed / last run failed
    Deleted,  // Soft-deleted
}

impl WorkerStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Created => "created", Self::Idle    => "idle",
            Self::Active  => "active",  Self::Working => "working",
            Self::Paused  => "paused",  Self::Error   => "error",
            Self::Deleted => "deleted",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "idle"    => Self::Idle,    "active"  => Self::Active,
            "working" => Self::Working, "paused"  => Self::Paused,
            "error"   => Self::Error,   "deleted" => Self::Deleted,
            _         => Self::Created,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worker {
    pub id:               i64,
    pub name:             String,
    pub slug:             String,            // kebab-case unique identifier
    pub worker_type:      String,
    pub status:           String,
    pub model:            String,
    pub primary_repos:    Vec<String>,       // JSON array stored as TEXT
    pub polling_interval: i64,              // seconds (default 120)
    pub recipe_id:        Option<String>,   // bound recipe YAML id
    pub is_system_defined: bool,            // true = cannot delete
    pub description:      Option<String>,
    pub created_at:       String,
    pub updated_at:       String,
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct WorkersDb {
    conn: Connection,
}

impl WorkersDb {
    /// Open (or create) ~/.autodev/workers.db and run migrations + seed.
    pub fn open() -> Result<Self> {
        let db_path = autodev_dir()?.join("workers.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| AutoDevError::Agent(format!("workers.db open failed: {e}")))?;
        let db = Self { conn };
        db.migrate()?;
        db.migrate_model_names()?;  // Phase 7.1 — fix any stale old-format model slugs
        db.seed_defaults()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(r#"
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
                updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_workers_slug   ON workers(slug);
            CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
            CREATE TRIGGER IF NOT EXISTS update_workers_timestamp
                AFTER UPDATE ON workers
                BEGIN
                    UPDATE workers SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
                END;
        "#).map_err(|e| AutoDevError::Agent(format!("workers.db migrate: {e}")))
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
        self.conn.execute_batch(r#"
            UPDATE workers SET model = 'claude-haiku-4-5'  WHERE model = 'claude-4-5-haiku';
            UPDATE workers SET model = 'claude-sonnet-4-5' WHERE model = 'claude-4-5-sonnet';
            UPDATE workers SET model = 'claude-opus-4-5'   WHERE model = 'claude-4-5-opus';
        "#).map_err(|e| AutoDevError::Agent(format!("migrate_model_names: {e}")))?;
        Ok(())
    }

    /// Seed the 5 pre-defined workers + 2 system agents if the table is empty.
    /// Each worker is bound to its corresponding recipe on creation.
    fn seed_defaults(&self) -> Result<()> {
        let count: i64 = self.conn
            .query_row("SELECT COUNT(*) FROM workers", [], |r| r.get(0))
            .unwrap_or(0);
        if count > 0 { return Ok(()); }

        let seeds: &[(&str, &str, &str, &str, i64, bool, &str, Option<&str>)] = &[
            // (name, slug, type, model, interval, is_system, description, recipe_id)
            ("Supervisor",        "supervisor",       "supervisor", "claude-opus-4-5",    120, true,  "Polls the task board, assigns tasks to workers.", Some("supervisor-poll")),
            // Doc 04 Phase 3 — QC worker now runs the qc-dispatcher recipe.
            // The slug stays `qc-agent` for backward compat (Live Monitor
            // card, scheduler hardcodes, hooks); the dispatcher recipe runs
            // on haiku and fans out per-task sonnet reviewer subagents via
            // `spawn_qc_review`. Old `qc-agent-review` recipe still exists in
            // the registry for users who manually rebind workers.
            ("QC Agent",          "qc-agent",         "qc",         "claude-haiku-4-5",   60,  true,  "Dispatches per-task QC reviewers (Doc 04 Phase 3).", Some("qc-dispatcher")),
            ("Worker Backend",    "worker-backend",   "backend",    "claude-sonnet-4-5",  120, false, "Implements backend tasks (Python / Rust).", Some("worker-backend-poll")),
            ("Worker Frontend",   "worker-frontend",  "frontend",   "claude-sonnet-4-5",  120, false, "Implements frontend tasks (React / TypeScript).", Some("worker-frontend-poll")),
            ("Worker Platform",   "worker-platform",  "platform",   "claude-sonnet-4-5",  120, false, "Infrastructure, CI/CD, and DevOps tasks.", Some("worker-platform-poll")),
            ("Worker MCP",        "worker-mcp",       "mcp",        "claude-opus-4-5",    120, false, "Builds and maintains MCP server extensions.", Some("worker-mcp-poll")),
            ("Worker Cleanup",    "worker-cleanup",   "cleanup",    "claude-haiku-4-5",   120, false, "Lint, dead code removal, and housekeeping.", Some("worker-cleanup-poll")),
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
        let mut stmt = self.conn.prepare(
            "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at \
             FROM workers WHERE status != 'deleted' ORDER BY is_system_defined DESC, id ASC"
        ).map_err(|e| AutoDevError::Agent(format!("workers list prepare: {e}")))?;

        let rows = stmt.query_map([], |row| {
            let repos_json: String = row.get(6)?;
            let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
            Ok(Worker {
                id:               row.get(0)?,
                name:             row.get(1)?,
                slug:             row.get(2)?,
                worker_type:      row.get(3)?,
                status:           row.get(4)?,
                model:            row.get(5)?,
                primary_repos:    repos,
                polling_interval: row.get(7)?,
                recipe_id:        row.get(8)?,
                is_system_defined: row.get::<_, i64>(9)? != 0,
                description:      row.get(10)?,
                created_at:       row.get(11)?,
                updated_at:       row.get(12)?,
            })
        }).map_err(|e| AutoDevError::Agent(format!("workers list query: {e}")))?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Agent(format!("workers list collect: {e}")))
    }

    pub fn get_by_id(&self, id: i64) -> Result<Option<Worker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at \
             FROM workers WHERE id = ?1 AND status != 'deleted'"
        ).map_err(|e| AutoDevError::Agent(format!("get_by_id prepare: {e}")))?;

        stmt.query_row(params![id], |row| {
            let repos_json: String = row.get(6)?;
            let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
            Ok(Worker {
                id:               row.get(0)?,
                name:             row.get(1)?,
                slug:             row.get(2)?,
                worker_type:      row.get(3)?,
                status:           row.get(4)?,
                model:            row.get(5)?,
                primary_repos:    repos,
                polling_interval: row.get(7)?,
                recipe_id:        row.get(8)?,
                is_system_defined: row.get::<_, i64>(9)? != 0,
                description:      row.get(10)?,
                created_at:       row.get(11)?,
                updated_at:       row.get(12)?,
            })
        }).optional()
         .map_err(|e| AutoDevError::Agent(format!("get_by_id: {e}")))
    }

    pub fn get_by_slug(&self, slug: &str) -> Result<Option<Worker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, slug, worker_type, status, model, primary_repos, \
                    polling_interval, recipe_id, is_system_defined, description, \
                    created_at, updated_at \
             FROM workers WHERE slug = ?1 AND status != 'deleted'"
        ).map_err(|e| AutoDevError::Agent(format!("get_by_slug prepare: {e}")))?;

        stmt.query_row(params![slug], |row| {
            let repos_json: String = row.get(6)?;
            let repos: Vec<String> = serde_json::from_str(&repos_json).unwrap_or_default();
            Ok(Worker {
                id:               row.get(0)?,
                name:             row.get(1)?,
                slug:             row.get(2)?,
                worker_type:      row.get(3)?,
                status:           row.get(4)?,
                model:            row.get(5)?,
                primary_repos:    repos,
                polling_interval: row.get(7)?,
                recipe_id:        row.get(8)?,
                is_system_defined: row.get::<_, i64>(9)? != 0,
                description:      row.get(10)?,
                created_at:       row.get(11)?,
                updated_at:       row.get(12)?,
            })
        }).optional()
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
        self.conn.execute(
            "UPDATE workers SET status = ?1 WHERE id = ?2",
            params![status, id],
        ).map_err(|e| AutoDevError::Agent(format!("update_status: {e}")))?;
        Ok(())
    }

    pub fn update_model(&self, id: i64, model: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET model = ?1 WHERE id = ?2",
            params![model, id],
        ).map_err(|e| AutoDevError::Agent(format!("update_model: {e}")))?;
        Ok(())
    }

    pub fn update_repos(&self, id: i64, primary_repos: &[String]) -> Result<()> {
        let repos_json = serde_json::to_string(primary_repos)
            .map_err(|e| AutoDevError::Agent(format!("repos json: {e}")))?;
        self.conn.execute(
            "UPDATE workers SET primary_repos = ?1 WHERE id = ?2",
            params![repos_json, id],
        ).map_err(|e| AutoDevError::Agent(format!("update_repos: {e}")))?;
        Ok(())
    }

    pub fn bind_recipe(&self, id: i64, recipe_id: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE workers SET recipe_id = ?1 WHERE id = ?2",
            params![recipe_id, id],
        ).map_err(|e| AutoDevError::Agent(format!("bind_recipe: {e}")))?;
        Ok(())
    }

    /// Soft-delete — system agents cannot be deleted.
    pub fn delete(&self, id: i64) -> Result<()> {
        let is_system: i64 = self.conn
            .query_row("SELECT is_system_defined FROM workers WHERE id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(|e| AutoDevError::Agent(format!("delete check: {e}")))?
            .unwrap_or(0);
        if is_system != 0 {
            return Err(AutoDevError::Agent("Cannot delete system-defined agents".into()));
        }
        self.conn.execute(
            "UPDATE workers SET status = 'deleted' WHERE id = ?1",
            params![id],
        ).map_err(|e| AutoDevError::Agent(format!("delete worker: {e}")))?;
        Ok(())
    }

    pub fn slug_exists(&self, slug: &str) -> Result<bool> {
        let count: i64 = self.conn
            .query_row("SELECT COUNT(*) FROM workers WHERE slug = ?1 AND status != 'deleted'", params![slug], |r| r.get(0))
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
