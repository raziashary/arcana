// Plan Board database — SQLite CRUD for the Arcana Plan Management System.
// Database path: ~/.arcana/plans.db
// Used by: PA (create plans in any mode), Supervisor (breakdown epics), Workers (claim items)
// See: DEVELOPMENT_PLAN.md Phase 3.5 — Plan Management System

use super::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Active,
    Completed,
    Abandoned,
}

impl PlanStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Abandoned => "abandoned",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "draft" => Self::Draft,
            "active" => Self::Active,
            "completed" => Self::Completed,
            "abandoned" => Self::Abandoned,
            _ => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanScope {
    Personal,
    Fleet,
    Epic,
}

impl PlanScope {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Personal => "personal",
            Self::Fleet => "fleet",
            Self::Epic => "epic",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "personal" => Self::Personal,
            "fleet" => Self::Fleet,
            "epic" => Self::Epic,
            _ => Self::Personal,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PlanItemStatus {
    Pending,
    InProgress,
    Done,
    Blocked,
}

impl PlanItemStatus {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "pending" => Self::Pending,
            "in_progress" => Self::InProgress,
            "done" => Self::Done,
            "blocked" => Self::Blocked,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Pa,
    Supervisor,
    Worker,
    Qc,
}

impl AgentType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Pa => "PA",
            Self::Supervisor => "Supervisor",
            Self::Worker => "Worker",
            Self::Qc => "QC",
        }
    }
    pub fn from_str(s: &str) -> Self {
        match s {
            "PA" => Self::Pa,
            "Supervisor" => Self::Supervisor,
            "Worker" => Self::Worker,
            "QC" => Self::Qc,
            _ => Self::Worker,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub session_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: PlanStatus,
    pub created_at: String,
    pub updated_at: String,
    pub assigned_agent: Option<String>,
    pub created_by: Option<String>,
    pub agent_scope: PlanScope,
    pub parent_plan_id: Option<String>,
    pub task_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub id: String,
    pub plan_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: PlanItemStatus,
    pub order_index: i64,
    pub agent_type: Option<AgentType>,
    pub estimated_tools: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub assigned_worker: Option<String>,
    pub worktree_branch: Option<String>,
    pub pr_url: Option<String>,
    pub result_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExecution {
    pub id: String,
    pub plan_id: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub total_items: i64,
    pub completed_items: i64,
    pub error_log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlanInput {
    pub title: String,
    pub description: Option<String>,
    pub session_id: Option<String>,
    pub assigned_agent: Option<String>,
    pub created_by: Option<String>,
    pub agent_scope: PlanScope,
    pub parent_plan_id: Option<String>,
    pub task_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePlanItemInput {
    pub plan_id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub order_index: i64,
    pub agent_type: Option<AgentType>,
    pub estimated_tools: Option<Vec<String>>,
    pub assigned_worker: Option<String>,
    pub worktree_branch: Option<String>,
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct PlansDb {
    conn: Connection,
}

impl PlansDb {
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
        let path = arcana_dir()?.join("plans.db");
        Self::open_at(&path)
    }

    /// As `open`, against an explicit path so tests do not write to the user's
    /// `~/.arcana/plans.db`.
    pub fn open_at(path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// `plans.db`'s ladder. `v1` is the schema as it stands, created idempotently,
    /// so an existing database is stamped without being altered.
    const LADDER: &[Migration] = &[
        Migration {
            version: 1,
            name: "plans-baseline",
            sql: r#"

            CREATE TABLE IF NOT EXISTS plans (
                id              TEXT PRIMARY KEY,
                session_id      TEXT,
                title           TEXT    NOT NULL,
                description     TEXT,
                status          TEXT    NOT NULL DEFAULT 'draft',
                created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                updated_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                assigned_agent  TEXT,
                created_by      TEXT,
                agent_scope     TEXT    NOT NULL DEFAULT 'personal',
                parent_plan_id  TEXT,
                task_id         INTEGER,
                FOREIGN KEY (parent_plan_id) REFERENCES plans(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_plans_status      ON plans(status);
            CREATE INDEX IF NOT EXISTS idx_plans_scope       ON plans(agent_scope);
            CREATE INDEX IF NOT EXISTS idx_plans_session     ON plans(session_id);
            CREATE INDEX IF NOT EXISTS idx_plans_parent      ON plans(parent_plan_id);
            CREATE INDEX IF NOT EXISTS idx_plans_task        ON plans(task_id);

            CREATE TABLE IF NOT EXISTS plan_items (
                id              TEXT PRIMARY KEY,
                plan_id         TEXT    NOT NULL,
                parent_id       TEXT,
                title           TEXT    NOT NULL,
                description     TEXT,
                status          TEXT    NOT NULL DEFAULT 'pending',
                order_index     INTEGER NOT NULL DEFAULT 0,
                agent_type      TEXT,
                estimated_tools TEXT,
                created_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                completed_at    TEXT,
                assigned_worker TEXT,
                worktree_branch TEXT,
                pr_url          TEXT,
                result_summary  TEXT,
                FOREIGN KEY (plan_id) REFERENCES plans(id) ON DELETE CASCADE,
                FOREIGN KEY (parent_id) REFERENCES plan_items(id)
            );

            CREATE INDEX IF NOT EXISTS idx_items_plan        ON plan_items(plan_id);
            CREATE INDEX IF NOT EXISTS idx_items_status      ON plan_items(status);
            CREATE INDEX IF NOT EXISTS idx_items_worker      ON plan_items(assigned_worker);

            CREATE TABLE IF NOT EXISTS plan_item_dependencies (
                item_id             TEXT NOT NULL,
                depends_on_item_id  TEXT NOT NULL,
                FOREIGN KEY (item_id) REFERENCES plan_items(id) ON DELETE CASCADE,
                FOREIGN KEY (depends_on_item_id) REFERENCES plan_items(id) ON DELETE CASCADE,
                PRIMARY KEY (item_id, depends_on_item_id)
            );

            CREATE TABLE IF NOT EXISTS plan_executions (
                id              TEXT PRIMARY KEY,
                plan_id         TEXT    NOT NULL,
                started_at      TEXT,
                completed_at    TEXT,
                total_items     INTEGER DEFAULT 0,
                completed_items INTEGER DEFAULT 0,
                error_log       TEXT,
                FOREIGN KEY (plan_id) REFERENCES plans(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_exec_plan         ON plan_executions(plan_id);
        "#,
        },
        Migration {
            version: 2,
            name: "canonical-utc-timestamps",
            sql: r#"
        DROP TRIGGER IF EXISTS plans_updated_at;
        CREATE TRIGGER plans_updated_at AFTER UPDATE ON plans
        BEGIN
            UPDATE plans SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plans_timestamps_canonical_insert
        AFTER INSERT ON plans
        BEGIN
            UPDATE plans SET
                created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                updated_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.updated_at), NEW.updated_at)
             WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plans_timestamps_canonical_update
        AFTER UPDATE OF created_at ON plans
        BEGIN
            UPDATE plans SET created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at)
             WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plan_items_timestamps_canonical_insert
        AFTER INSERT ON plan_items
        BEGIN
            UPDATE plan_items SET
                created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                completed_at = CASE WHEN NEW.completed_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.completed_at), NEW.completed_at) END
             WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plan_items_timestamps_canonical_update
        AFTER UPDATE OF created_at, completed_at ON plan_items
        BEGIN
            UPDATE plan_items SET
                created_at = COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.created_at), NEW.created_at),
                completed_at = CASE WHEN NEW.completed_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.completed_at), NEW.completed_at) END
             WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plan_executions_timestamps_canonical_insert
        AFTER INSERT ON plan_executions
        BEGIN
            UPDATE plan_executions SET
                started_at = CASE WHEN NEW.started_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.started_at), NEW.started_at) END,
                completed_at = CASE WHEN NEW.completed_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.completed_at), NEW.completed_at) END
             WHERE id = NEW.id;
        END;
        CREATE TRIGGER IF NOT EXISTS plan_executions_timestamps_canonical_update
        AFTER UPDATE OF started_at, completed_at ON plan_executions
        BEGIN
            UPDATE plan_executions SET
                started_at = CASE WHEN NEW.started_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.started_at), NEW.started_at) END,
                completed_at = CASE WHEN NEW.completed_at IS NULL THEN NULL
                    ELSE COALESCE(strftime('%Y-%m-%dT%H:%M:%fZ', NEW.completed_at), NEW.completed_at) END
             WHERE id = NEW.id;
        END;
    "#,
        },
    ];

    fn migrate(&self) -> Result<()> {
        // PRAGMAs are per-connection and are not carried by a migration.
        self.conn
            .execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")
            .map_err(|e| ArcanaError::Agent(format!("plans.db pragmas: {e}")))?;
        migrate::run_ladder(&self.conn, "plans.db", Self::LADDER)?;
        Ok(())
    }

    // ─── Plans CRUD ───────────────────────────────────────────────────────────

    pub fn create_plan(&self, input: CreatePlanInput) -> Result<Plan> {
        let timing = OperationTimer::start("database_plan_create").with_database("plans");
        let id = uuid::Uuid::new_v4().to_string();
        let status = PlanStatus::Draft.as_str();
        let scope = input.agent_scope.as_str();
        let created_by = input.created_by.as_deref().unwrap_or("pa");
        self.conn.execute(
            "INSERT INTO plans (id, session_id, title, description, status, assigned_agent, created_by, agent_scope, parent_plan_id, task_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                input.session_id,
                &input.title,
                input.description,
                status,
                input.assigned_agent,
                created_by,
                scope,
                input.parent_plan_id,
                input.task_id,
            ],
        )?;
        let plan = self
            .get_plan(&id)?
            .ok_or_else(|| ArcanaError::Agent("Plan not found after insert".into()))?;
        timing.with_result_count(1).succeed();
        Ok(plan)
    }

    pub fn get_plan(&self, id: &str) -> Result<Option<Plan>> {
        let timing = OperationTimer::start("database_plan_read").with_database("plans");
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, title, description, status, created_at, updated_at,
                    assigned_agent, created_by, agent_scope, parent_plan_id, task_id
             FROM plans WHERE id = ?1",
        )?;
        let plan = stmt.query_row(params![id], row_to_plan).optional()?;
        timing
            .with_result_count(usize::from(plan.is_some()))
            .succeed();
        Ok(plan)
    }

    pub fn list_plans(
        &self,
        status_filter: Option<&str>,
        scope_filter: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<Plan>> {
        let timing = OperationTimer::start("database_plan_list").with_database("plans");
        let mut conditions: Vec<String> = Vec::new();
        let mut params_vec: Vec<String> = Vec::new();

        if let Some(s) = status_filter {
            conditions.push(format!("status = ?{}", conditions.len() + 1));
            params_vec.push(s.to_string());
        }
        if let Some(s) = scope_filter {
            conditions.push(format!("agent_scope = ?{}", conditions.len() + 1));
            params_vec.push(s.to_string());
        }
        if let Some(s) = session_id {
            conditions.push(format!("session_id = ?{}", conditions.len() + 1));
            params_vec.push(s.to_string());
        }

        let mut sql = "SELECT id, session_id, title, description, status, created_at, updated_at, assigned_agent, created_by, agent_scope, parent_plan_id, task_id FROM plans".to_string();
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        sql.push_str(" ORDER BY julianday(created_at) ASC, id ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let plans = stmt
            .query_map(rusqlite::params_from_iter(params_vec.iter()), row_to_plan)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        timing.with_result_count(plans.len()).succeed();
        Ok(plans)
    }

    pub fn update_plan_status(&self, id: &str, status: &str) -> Result<()> {
        let timing = OperationTimer::start("database_plan_status_update").with_database("plans");
        let changed = self.conn.execute(
            "UPDATE plans SET status = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?2",
            params![status, id],
        )?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn delete_plan(&self, id: &str) -> Result<()> {
        let timing = OperationTimer::start("database_plan_delete").with_database("plans");
        let changed = self
            .conn
            .execute("DELETE FROM plans WHERE id = ?1", params![id])?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    // ─── Plan Items CRUD ─────────────────────────────────────────────────────

    pub fn add_plan_item(&self, input: CreatePlanItemInput) -> Result<PlanItem> {
        let timing = OperationTimer::start("database_plan_item_create").with_database("plans");
        let id = uuid::Uuid::new_v4().to_string();
        let estimated_tools = input
            .estimated_tools
            .as_ref()
            .map(|t| serde_json::to_string(t).unwrap_or_default());
        let agent_type = input.agent_type.as_ref().map(|a| a.as_str().to_string());
        self.conn.execute(
            "INSERT INTO plan_items (id, plan_id, parent_id, title, description, order_index, agent_type, estimated_tools, assigned_worker, worktree_branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                &input.plan_id,
                input.parent_id,
                &input.title,
                input.description,
                input.order_index,
                agent_type,
                estimated_tools,
                input.assigned_worker,
                input.worktree_branch,
            ],
        )?;
        let item = self
            .get_plan_item(&id)?
            .ok_or_else(|| ArcanaError::Agent("Plan item not found after insert".into()))?;
        timing.with_result_count(1).succeed();
        Ok(item)
    }

    pub fn get_plan_item(&self, id: &str) -> Result<Option<PlanItem>> {
        let timing = OperationTimer::start("database_plan_item_read").with_database("plans");
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items WHERE id = ?1"
        )?;
        let item = stmt.query_row(params![id], row_to_plan_item).optional()?;
        timing
            .with_result_count(usize::from(item.is_some()))
            .succeed();
        Ok(item)
    }

    pub fn list_plan_items(&self, plan_id: &str) -> Result<Vec<PlanItem>> {
        let timing = OperationTimer::start("database_plan_item_list").with_database("plans");
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items WHERE plan_id = ?1 ORDER BY order_index ASC, julianday(created_at) ASC, id ASC"
        )?;
        let items = stmt
            .query_map(params![plan_id], row_to_plan_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        timing.with_result_count(items.len()).succeed();
        Ok(items)
    }

    pub fn update_plan_item_status(
        &self,
        id: &str,
        status: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let timing =
            OperationTimer::start("database_plan_item_status_update").with_database("plans");
        let completed_at = if status == "done" {
            "strftime('%Y-%m-%dT%H:%M:%fZ','now')"
        } else {
            "NULL"
        };
        if let Some(n) = notes {
            let sql = format!(
                "UPDATE plan_items SET status = ?1, result_summary = ?2, completed_at = {completed_at} WHERE id = ?3"
            );
            let changed = self.conn.execute(&sql, params![status, n, id])?;
            timing.with_result_count(changed).succeed();
        } else {
            let sql = format!(
                "UPDATE plan_items SET status = ?1, completed_at = {completed_at} WHERE id = ?2"
            );
            let changed = self.conn.execute(&sql, params![status, id])?;
            timing.with_result_count(changed).succeed();
        }
        Ok(())
    }

    pub fn claim_next_item(&self, plan_id: &str, worker_id: &str) -> Result<Option<PlanItem>> {
        let timing = OperationTimer::start("database_plan_item_claim").with_database("plans");
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items
             WHERE plan_id = ?1 AND status = 'pending' AND (assigned_worker IS NULL OR assigned_worker = '')
             ORDER BY order_index ASC, julianday(created_at) ASC, id ASC
             LIMIT 1"
        )?;
        let item = stmt
            .query_row(params![plan_id], row_to_plan_item)
            .optional()?;
        if let Some(ref it) = item {
            let changed = self.conn.execute(
                "UPDATE plan_items SET status = 'in_progress', assigned_worker = ?1 WHERE id = ?2",
                params![worker_id, &it.id],
            )?;
            timing.with_result_count(changed).succeed();
        } else {
            timing.with_result_count(0).succeed();
        }
        Ok(item)
    }

    // ─── Dependencies ────────────────────────────────────────────────────────

    pub fn add_dependency(&self, item_id: &str, depends_on: &str) -> Result<()> {
        let timing = OperationTimer::start("database_plan_dependency_add").with_database("plans");
        let changed = self.conn.execute(
            "INSERT INTO plan_item_dependencies (item_id, depends_on_item_id) VALUES (?1, ?2)",
            params![item_id, depends_on],
        )?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn check_dependencies(&self, item_id: &str) -> Result<Vec<bool>> {
        let timing = OperationTimer::start("database_plan_dependency_check").with_database("plans");
        let mut stmt = self.conn.prepare(
            "SELECT i.status
             FROM plan_item_dependencies d
             JOIN plan_items i ON d.depends_on_item_id = i.id
             WHERE d.item_id = ?1",
        )?;
        let statuses: Vec<bool> = stmt
            .query_map(params![item_id], |row| {
                let status: String = row.get(0)?;
                Ok(status == "done")
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        timing.with_result_count(statuses.len()).succeed();
        Ok(statuses)
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    pub fn plan_exists(&self, id: &str) -> Result<bool> {
        let timing = OperationTimer::start("database_plan_exists").with_database("plans");
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM plans WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        timing
            .with_result_count(usize::try_from(count).unwrap_or_default())
            .succeed();
        Ok(count > 0)
    }
}

fn row_to_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plan> {
    Ok(Plan {
        id: row.get(0)?,
        session_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: PlanStatus::from_str(row.get::<_, String>(4)?.as_str()),
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        assigned_agent: row.get(7)?,
        created_by: row.get(8)?,
        agent_scope: PlanScope::from_str(row.get::<_, String>(9)?.as_str()),
        parent_plan_id: row.get(10)?,
        task_id: row.get(11)?,
    })
}

fn row_to_plan_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanItem> {
    Ok(PlanItem {
        id: row.get(0)?,
        plan_id: row.get(1)?,
        parent_id: row.get(2)?,
        title: row.get(3)?,
        description: row.get(4)?,
        status: PlanItemStatus::from_str(row.get::<_, String>(5)?.as_str()),
        order_index: row.get(6)?,
        agent_type: row
            .get::<_, Option<String>>(7)?
            .as_ref()
            .map(|s| AgentType::from_str(s.as_str())),
        estimated_tools: row.get(8)?,
        created_at: row.get(9)?,
        completed_at: row.get(10)?,
        assigned_worker: row.get(11)?,
        worktree_branch: row.get(12)?,
        pr_url: row.get(13)?,
        result_summary: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_database_operations_emit_safe_result_events() {
        let dir = tempfile::tempdir().expect("isolated plan database directory");
        let db = PlansDb::open_at(&dir.path().join("plans.db"))
            .expect("open isolated production database");

        let (plan, events) = crate::observability::timing::capture_json_events(|| {
            db.create_plan(CreatePlanInput {
                title: "private plan title is not telemetry".into(),
                description: Some("private plan description".into()),
                session_id: Some("session-private".into()),
                assigned_agent: None,
                created_by: Some("test".into()),
                agent_scope: PlanScope::Fleet,
                parent_plan_id: None,
                task_id: Some(72),
            })
        });
        plan.expect("create plan through production API");
        let create_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_plan_create")
            .expect("plan creation event");
        assert_eq!(create_event["fields"]["database"], "plans");
        assert_eq!(create_event["fields"]["outcome"], "success");
        assert_eq!(create_event["fields"]["result_count"], 1);
        assert!(create_event["fields"]["duration_ns"].as_u64().is_some());
        let serialized = create_event.to_string();
        assert!(!serialized.contains("private plan title"));
        assert!(!serialized.contains("private plan description"));
        assert!(!serialized.contains("session-private"));

        let (listed, events) = crate::observability::timing::capture_json_events(|| {
            db.list_plans(None, Some("fleet"), None)
        });
        assert_eq!(listed.expect("list plans").len(), 1);
        let list_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_plan_list")
            .expect("plan listing event");
        assert_eq!(list_event["fields"]["outcome"], "success");
        assert_eq!(list_event["fields"]["result_count"], 1);

        let (_, events) =
            crate::observability::timing::capture_json_events(|| db.get_plan("missing-plan"));
        let missing_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_plan_read")
            .expect("missing plan read event");
        assert_eq!(missing_event["fields"]["outcome"], "success");
        assert_eq!(missing_event["fields"]["result_count"], 0);

        let (invalid_dependency, events) =
            crate::observability::timing::capture_json_events(|| {
                db.add_dependency("missing-item", "missing-dependency")
            });
        assert!(invalid_dependency.is_err());
        let failure_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_plan_dependency_add")
            .expect("dependency failure event");
        assert_eq!(failure_event["fields"]["database"], "plans");
        assert_eq!(failure_event["fields"]["outcome"], "error");
    }

    #[test]
    fn production_plan_reader_returns_canonical_timestamps_and_keeps_legacy_rows() {
        let dir = tempfile::tempdir().expect("isolated temp directory");
        let _home = crate::db::test_support::with_home(dir.path());
        let path = dir.path().join("plans.db");
        let db = PlansDb::open_at(&path).expect("open isolated production database");
        let plan = db
            .create_plan(CreatePlanInput {
                title: "timestamp plan".into(),
                description: None,
                session_id: None,
                assigned_agent: None,
                created_by: Some("test".into()),
                agent_scope: PlanScope::Personal,
                parent_plan_id: None,
                task_id: None,
            })
            .expect("create through production writer");
        for value in [&plan.created_at, &plan.updated_at] {
            assert_eq!(value.len(), 24, "expected fixed millisecond UTC: {value}");
            assert!(value.ends_with('Z'), "expected UTC Z suffix: {value}");
            assert!(chrono::DateTime::parse_from_rfc3339(value).is_ok());
        }

        let item = db
            .add_plan_item(CreatePlanItemInput {
                plan_id: plan.id.clone(),
                parent_id: None,
                title: "timestamp item".into(),
                description: None,
                order_index: 0,
                agent_type: None,
                estimated_tools: None,
                assigned_worker: None,
                worktree_branch: None,
            })
            .expect("create plan item through production writer");
        assert_eq!(item.created_at.len(), 24);
        assert!(item.created_at.ends_with('Z'));
        db.update_plan_item_status(&item.id, "done", None)
            .expect("complete plan item");
        let completed = db.get_plan_item(&item.id).unwrap().unwrap();
        let completed_at = completed.completed_at.expect("completion timestamp");
        assert_eq!(completed_at.len(), 24);
        assert!(completed_at.ends_with('Z'));

        db.conn
            .execute_batch(
                "DROP TRIGGER plans_timestamps_canonical_insert;
                 DROP TRIGGER plans_timestamps_canonical_update;
                 PRAGMA user_version = 1;",
            )
            .expect("prepare old-format row");
        db.conn
            .execute(
                "UPDATE plans SET created_at = '2026-01-02 03:04:05' WHERE id = ?1",
                params![plan.id],
            )
            .expect("write legacy timestamp fixture");
        db.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .expect("checkpoint legacy fixture before copying");
        drop(db);

        let upgrade_dir = path.parent().unwrap().join("upgrade-home");
        std::fs::create_dir_all(&upgrade_dir).expect("create isolated upgrade fixture");
        let upgrade_path = upgrade_dir.join("plans.db");
        std::fs::copy(&path, &upgrade_path).expect("copy the pre-upgrade database fixture");
        drop(_home);
        let _upgrade_home = crate::db::test_support::with_home(&upgrade_dir);
        let upgraded = PlansDb::open_at(&upgrade_path).expect("run additive v2 migration");
        let legacy = upgraded.get_plan(&plan.id).unwrap().unwrap();
        assert_eq!(legacy.created_at, "2026-01-02 03:04:05");
        assert!(crate::db::timestamps::parse_db_timestamp(&legacy.created_at).is_ok());
        drop(upgraded);
        let _ = std::fs::remove_dir_all(dir.path());
    }

    #[test]
    fn plan_claim_uses_populated_plan_index_and_preserves_claim_guards() {
        const CLAIM_NEXT_ITEM_SQL: &str = "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items
             WHERE plan_id = ?1 AND status = 'pending' AND (assigned_worker IS NULL OR assigned_worker = '')
             ORDER BY order_index ASC, julianday(created_at) ASC, id ASC
             LIMIT 1";

        let dir = tempfile::tempdir().expect("isolated plan database directory");
        let db = PlansDb::open_at(&dir.path().join("plans.db"))
            .expect("open isolated production database");
        let plan = db
            .create_plan(CreatePlanInput {
                title: "dispatch plan".into(),
                description: None,
                session_id: Some("dispatch-fixture".into()),
                assigned_agent: None,
                created_by: Some("test".into()),
                agent_scope: PlanScope::Fleet,
                parent_plan_id: None,
                task_id: None,
            })
            .expect("create through production writer");
        let unrelated_plan = db
            .create_plan(CreatePlanInput {
                title: "unrelated plan".into(),
                description: None,
                session_id: Some("dispatch-fixture".into()),
                assigned_agent: None,
                created_by: Some("test".into()),
                agent_scope: PlanScope::Fleet,
                parent_plan_id: None,
                task_id: None,
            })
            .expect("create unrelated plan");

        let insert_item = |id: &str,
                           plan_id: &str,
                           status: &str,
                           order_index: i64,
                           assigned_worker: Option<&str>| {
            db.conn
                .execute(
                    "INSERT INTO plan_items (id, plan_id, title, status, order_index, assigned_worker)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![id, plan_id, id, status, order_index, assigned_worker],
                )
                .expect("populate plan item fixture");
        };

        insert_item(
            "already-assigned",
            &plan.id,
            "pending",
            0,
            Some("other-worker"),
        );
        insert_item("already-running", &plan.id, "in_progress", 1, None);
        insert_item("eligible-first", &plan.id, "pending", 2, None);
        insert_item("eligible-second", &plan.id, "pending", 3, Some(""));
        insert_item("other-plan-first", &unrelated_plan.id, "pending", -1, None);
        for index in 0..1_000 {
            let status = if index % 4 == 0 { "pending" } else { "done" };
            let assigned_worker = (status == "pending").then_some("fixture-worker");
            insert_item(
                &format!("fixture-{index:04}"),
                &plan.id,
                status,
                10 + index,
                assigned_worker,
            );
        }
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze populated fixture");

        let mut explain = db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {CLAIM_NEXT_ITEM_SQL}"))
            .expect("prepare production claim query plan");
        let plan_details = explain
            .query_map(params![&plan.id], |row| row.get::<_, String>(3))
            .expect("explain production claim query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect claim query plan");
        assert!(
            plan_details
                .iter()
                .any(|detail| {
                    detail.contains("idx_items_plan") || detail.contains("idx_items_status")
                }),
            "claim selection should use an existing plan or status filter index on populated data; plan: {plan_details:?}"
        );
        assert!(
            !plan_details
                .iter()
                .any(|detail| detail.contains("SCAN plan_items")),
            "claim selection should not scan plan_items; plan: {plan_details:?}"
        );

        let first = db
            .claim_next_item(&plan.id, "worker-target")
            .expect("claim first available item")
            .expect("eligible item exists");
        assert_eq!(first.id, "eligible-first");
        assert_eq!(first.status, PlanItemStatus::Pending);
        let persisted_first = db
            .get_plan_item(&first.id)
            .expect("read claimed item")
            .expect("claimed item persists");
        assert_eq!(persisted_first.status, PlanItemStatus::InProgress);
        assert_eq!(
            persisted_first.assigned_worker.as_deref(),
            Some("worker-target")
        );

        let second = db
            .claim_next_item(&plan.id, "worker-target")
            .expect("claim next available item")
            .expect("second eligible item exists");
        assert_eq!(second.id, "eligible-second");
        assert_eq!(
            db.claim_next_item(&plan.id, "worker-target")
                .expect("no remaining unassigned pending item")
                .is_none(),
            true
        );
        assert_eq!(
            db.get_plan_item("already-assigned")
                .unwrap()
                .unwrap()
                .status,
            PlanItemStatus::Pending,
            "an item assigned to another worker remains unclaimed"
        );
        assert_eq!(
            db.get_plan_item("already-running").unwrap().unwrap().status,
            PlanItemStatus::InProgress,
            "an item already in progress remains ineligible"
        );
        assert_eq!(
            db.get_plan_item("other-plan-first")
                .unwrap()
                .unwrap()
                .status,
            PlanItemStatus::Pending,
            "an earlier item in another plan remains untouched"
        );
    }

    #[test]
    fn plan_dependency_reader_uses_populated_indexes_and_returns_statuses() {
        const CHECK_DEPENDENCIES_SQL: &str = "SELECT i.status
             FROM plan_item_dependencies d
             JOIN plan_items i ON d.depends_on_item_id = i.id
             WHERE d.item_id = ?1";

        let dir = tempfile::tempdir().expect("isolated plan database directory");
        let db = PlansDb::open_at(&dir.path().join("plans.db"))
            .expect("open isolated production database");
        let plan = db
            .create_plan(CreatePlanInput {
                title: "dependency plan".into(),
                description: None,
                session_id: Some("dependency-fixture".into()),
                assigned_agent: None,
                created_by: Some("test".into()),
                agent_scope: PlanScope::Fleet,
                parent_plan_id: None,
                task_id: None,
            })
            .expect("create through production writer");
        let insert_item = |id: &str, status: &str, order_index: i64| {
            db.conn
                .execute(
                    "INSERT INTO plan_items (id, plan_id, title, status, order_index)
                     VALUES (?1, ?2, ?1, ?3, ?4)",
                    params![id, &plan.id, status, order_index],
                )
                .expect("populate dependency item fixture");
        };
        insert_item("dependent", "pending", 0);
        insert_item("dependency-done", "done", 1);
        insert_item("dependency-open", "pending", 2);
        for index in 0..1_000 {
            let id = format!("dependency-fixture-{index:04}");
            insert_item(
                &id,
                if index % 2 == 0 { "done" } else { "pending" },
                3 + index,
            );
            db.conn
                .execute(
                    "INSERT INTO plan_item_dependencies (item_id, depends_on_item_id) VALUES (?1, ?2)",
                    params![id, if index % 2 == 0 { "dependency-done" } else { "dependency-open" }],
                )
                .expect("populate dependency index fixture");
        }
        db.conn
            .execute(
                "INSERT INTO plan_item_dependencies (item_id, depends_on_item_id) VALUES (?1, ?2), (?1, ?3)",
                params!["dependent", "dependency-done", "dependency-open"],
            )
            .expect("populate valid dependencies");
        db.conn
            .execute_batch("ANALYZE")
            .expect("analyze populated fixture");

        let mut explain = db
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {CHECK_DEPENDENCIES_SQL}"))
            .expect("prepare production dependency query plan");
        let plan_details = explain
            .query_map(params!["dependent"], |row| row.get::<_, String>(3))
            .expect("explain production dependency query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect dependency query plan");
        assert!(
            plan_details
                .iter()
                .any(|detail| { detail.contains("sqlite_autoindex_plan_item_dependencies_1") }),
            "dependency lookup should use the item_id primary-key index; plan: {plan_details:?}"
        );
        assert!(
            plan_details
                .iter()
                .any(|detail| detail.contains("sqlite_autoindex_plan_items_1")),
            "dependency status should use the item primary-key index; plan: {plan_details:?}"
        );
        assert!(
            !plan_details.iter().any(|detail| {
                detail.contains("SCAN plan_item_dependencies") || detail.contains("SCAN plan_items")
            }),
            "dependency lookup should not scan either plan table; plan: {plan_details:?}"
        );

        let mut statuses = db
            .check_dependencies("dependent")
            .expect("read dependencies through production method");
        statuses.sort_unstable();
        assert_eq!(statuses, vec![false, true]);
        assert_eq!(
            db.check_dependencies("item-without-dependencies")
                .expect("a missing item has no dependency rows"),
            Vec::<bool>::new()
        );
    }
}
