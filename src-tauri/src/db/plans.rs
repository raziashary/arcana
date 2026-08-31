// Plan Board database — SQLite CRUD for the AutoDev Plan Management System.
// Database path: ~/.autodev/plans.db
// Used by: PA (create plans in any mode), Supervisor (breakdown epics), Workers (claim items)
// See: DEVELOPMENT_PLAN.md Phase 3.5 — Plan Management System

use super::autodev_dir;
use crate::errors::{AutoDevError, Result};
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
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("plans.db");
        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS plans (
                id              TEXT PRIMARY KEY,
                session_id      TEXT,
                title           TEXT    NOT NULL,
                description     TEXT,
                status          TEXT    NOT NULL DEFAULT 'draft',
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT    NOT NULL DEFAULT (datetime('now')),
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
                created_at      TEXT    NOT NULL DEFAULT (datetime('now')),
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
            ",
        )?;
        Ok(())
    }

    // ─── Plans CRUD ───────────────────────────────────────────────────────────

    pub fn create_plan(&self, input: CreatePlanInput) -> Result<Plan> {
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
        self.get_plan(&id)?
            .ok_or_else(|| AutoDevError::Agent("Plan not found after insert".into()))
    }

    pub fn get_plan(&self, id: &str) -> Result<Option<Plan>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, title, description, status, created_at, updated_at,
                    assigned_agent, created_by, agent_scope, parent_plan_id, task_id
             FROM plans WHERE id = ?1",
        )?;
        let plan = stmt.query_row(params![id], row_to_plan).optional()?;
        Ok(plan)
    }

    pub fn list_plans(
        &self,
        status_filter: Option<&str>,
        scope_filter: Option<&str>,
        session_id: Option<&str>,
    ) -> Result<Vec<Plan>> {
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
        sql.push_str(" ORDER BY created_at ASC");

        let mut stmt = self.conn.prepare(&sql)?;
        let plans = stmt
            .query_map(rusqlite::params_from_iter(params_vec.iter()), row_to_plan)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(plans)
    }

    pub fn update_plan_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE plans SET status = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![status, id],
        )?;
        Ok(())
    }

    pub fn delete_plan(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM plans WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ─── Plan Items CRUD ─────────────────────────────────────────────────────

    pub fn add_plan_item(&self, input: CreatePlanItemInput) -> Result<PlanItem> {
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
        self.get_plan_item(&id)?
            .ok_or_else(|| AutoDevError::Agent("Plan item not found after insert".into()))
    }

    pub fn get_plan_item(&self, id: &str) -> Result<Option<PlanItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items WHERE id = ?1"
        )?;
        let item = stmt.query_row(params![id], row_to_plan_item).optional()?;
        Ok(item)
    }

    pub fn list_plan_items(&self, plan_id: &str) -> Result<Vec<PlanItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items WHERE plan_id = ?1 ORDER BY order_index ASC, created_at ASC"
        )?;
        let items = stmt
            .query_map(params![plan_id], row_to_plan_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn update_plan_item_status(
        &self,
        id: &str,
        status: &str,
        notes: Option<&str>,
    ) -> Result<()> {
        let completed_at = if status == "done" {
            "datetime('now')"
        } else {
            "NULL"
        };
        if let Some(n) = notes {
            let sql = format!(
                "UPDATE plan_items SET status = ?1, result_summary = ?2, completed_at = {completed_at} WHERE id = ?3"
            );
            self.conn.execute(&sql, params![status, n, id])?;
        } else {
            let sql = format!(
                "UPDATE plan_items SET status = ?1, completed_at = {completed_at} WHERE id = ?2"
            );
            self.conn.execute(&sql, params![status, id])?;
        }
        Ok(())
    }

    pub fn claim_next_item(&self, plan_id: &str, worker_id: &str) -> Result<Option<PlanItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, plan_id, parent_id, title, description, status, order_index, agent_type, estimated_tools, created_at, completed_at, assigned_worker, worktree_branch, pr_url, result_summary
             FROM plan_items
             WHERE plan_id = ?1 AND status = 'pending' AND (assigned_worker IS NULL OR assigned_worker = '')
             ORDER BY order_index ASC, created_at ASC
             LIMIT 1"
        )?;
        let item = stmt
            .query_row(params![plan_id], row_to_plan_item)
            .optional()?;
        if let Some(ref it) = item {
            self.conn.execute(
                "UPDATE plan_items SET status = 'in_progress', assigned_worker = ?1 WHERE id = ?2",
                params![worker_id, &it.id],
            )?;
        }
        Ok(item)
    }

    // ─── Dependencies ────────────────────────────────────────────────────────

    pub fn add_dependency(&self, item_id: &str, depends_on: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO plan_item_dependencies (item_id, depends_on_item_id) VALUES (?1, ?2)",
            params![item_id, depends_on],
        )?;
        Ok(())
    }

    pub fn check_dependencies(&self, item_id: &str) -> Result<Vec<bool>> {
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
        Ok(statuses)
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    pub fn plan_exists(&self, id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM plans WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )?;
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
