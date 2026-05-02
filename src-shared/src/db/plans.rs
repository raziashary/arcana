// Plans database — mirrors the schema in src-tauri/src/db/plans.rs.

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use crate::errors::{Result, SharedError};
use super::autodev_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub id:          i64,
    pub title:       String,
    pub description: Option<String>,
    pub status:      String,
    pub scope:       String,
    pub created_by:  String,
    pub created_at:  String,
    pub updated_at:  String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub id:          i64,
    pub plan_id:     i64,
    pub title:       String,
    pub description: Option<String>,
    pub status:      String,
    pub order_index: i32,
    pub created_at:  String,
    pub updated_at:  String,
}

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
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS plans (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 title       TEXT    NOT NULL,
                 description TEXT,
                 status      TEXT    NOT NULL DEFAULT 'draft',
                 scope       TEXT    NOT NULL DEFAULT 'personal',
                 created_by  TEXT    NOT NULL DEFAULT 'airsctl',
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
             );
             CREATE TABLE IF NOT EXISTS plan_steps (
                 id          INTEGER PRIMARY KEY AUTOINCREMENT,
                 plan_id     INTEGER NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
                 title       TEXT    NOT NULL,
                 description TEXT,
                 status      TEXT    NOT NULL DEFAULT 'pending',
                 order_index INTEGER NOT NULL DEFAULT 0,
                 created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
                 updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
             );
             CREATE INDEX IF NOT EXISTS idx_plans_status       ON plans(status);
             CREATE INDEX IF NOT EXISTS idx_plan_steps_plan_id ON plan_steps(plan_id);"
        )?;
        Ok(())
    }

    pub fn create(&self, title: &str, description: Option<&str>, scope: Option<&str>, created_by: &str) -> Result<Plan> {
        self.conn.execute(
            "INSERT INTO plans (title, description, scope, created_by) VALUES (?1, ?2, ?3, ?4)",
            params![title, description, scope.unwrap_or("personal"), created_by],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get(id)?.ok_or_else(|| SharedError::Other("Plan not found after insert".into()))
    }

    pub fn get(&self, id: i64) -> Result<Option<Plan>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,title,description,status,scope,created_by,created_at,updated_at FROM plans WHERE id=?1"
        )?;
        Ok(stmt.query_row(params![id], row_to_plan).optional()?)
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<Plan>> {
        let mut stmt = match status_filter {
            Some(_) => self.conn.prepare(
                "SELECT id,title,description,status,scope,created_by,created_at,updated_at FROM plans WHERE status=?1 ORDER BY created_at DESC"
            )?,
            None => self.conn.prepare(
                "SELECT id,title,description,status,scope,created_by,created_at,updated_at FROM plans ORDER BY created_at DESC"
            )?,
        };
        let plans = match status_filter {
            Some(s) => stmt.query_map(params![s], row_to_plan)?.collect::<std::result::Result<Vec<_>, _>>()?,
            None    => stmt.query_map([], row_to_plan)?.collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(plans)
    }

    pub fn activate(&self, id: i64) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE plans SET status='active', updated_at=datetime('now') WHERE id=?1 AND status='draft'",
            params![id],
        )?;
        if rows == 0 {
            return Err(SharedError::Other(format!("Plan {id} is not in 'draft' status or does not exist")));
        }
        Ok(())
    }

    pub fn add_step(&self, plan_id: i64, title: &str, description: Option<&str>) -> Result<PlanStep> {
        // Get next order index
        let max_order: i32 = self.conn
            .query_row(
                "SELECT COALESCE(MAX(order_index), -1) FROM plan_steps WHERE plan_id=?1",
                params![plan_id],
                |r| r.get(0),
            )
            .unwrap_or(-1);

        self.conn.execute(
            "INSERT INTO plan_steps (plan_id, title, description, order_index) VALUES (?1, ?2, ?3, ?4)",
            params![plan_id, title, description, max_order + 1],
        )?;
        let id = self.conn.last_insert_rowid();
        self.get_step(id)?.ok_or_else(|| SharedError::Other("Step not found after insert".into()))
    }

    pub fn get_step(&self, id: i64) -> Result<Option<PlanStep>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,plan_id,title,description,status,order_index,created_at,updated_at FROM plan_steps WHERE id=?1"
        )?;
        Ok(stmt.query_row(params![id], row_to_step).optional()?)
    }

    pub fn list_steps(&self, plan_id: i64) -> Result<Vec<PlanStep>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,plan_id,title,description,status,order_index,created_at,updated_at FROM plan_steps WHERE plan_id=?1 ORDER BY order_index ASC"
        )?;
        let steps = stmt.query_map(params![plan_id], row_to_step)?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(steps)
    }

    pub fn update_step_status(&self, step_id: i64, status: &str) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE plan_steps SET status=?1, updated_at=datetime('now') WHERE id=?2",
            params![status, step_id],
        )?;
        if rows == 0 {
            return Err(SharedError::Other(format!("Step {step_id} not found")));
        }
        Ok(())
    }
}

fn row_to_plan(row: &rusqlite::Row<'_>) -> rusqlite::Result<Plan> {
    Ok(Plan {
        id:          row.get(0)?,
        title:       row.get(1)?,
        description: row.get(2)?,
        status:      row.get(3)?,
        scope:       row.get(4)?,
        created_by:  row.get(5)?,
        created_at:  row.get(6)?,
        updated_at:  row.get(7)?,
    })
}

fn row_to_step(row: &rusqlite::Row<'_>) -> rusqlite::Result<PlanStep> {
    Ok(PlanStep {
        id:          row.get(0)?,
        plan_id:     row.get(1)?,
        title:       row.get(2)?,
        description: row.get(3)?,
        status:      row.get(4)?,
        order_index: row.get(5)?,
        created_at:  row.get(6)?,
        updated_at:  row.get(7)?,
    })
}
