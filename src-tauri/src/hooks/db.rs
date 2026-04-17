// HooksDb — SQLite persistence for hook configurations and execution logs.
// Stored in ~/.autodev/config.db (hooks + hook_logs tables).
// Phase 3.1

use rusqlite::{Connection, params};
use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};
use super::types::{
    AddHookInput, HookAction, HookConditions, HookConfig, HookExecutionLog, HookOutcome, HookType,
};

pub struct HooksDb {
    conn: Connection,
}

impl HooksDb {
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("config.db");
        let conn = Connection::open(&path)
            .map_err(|e| AutoDevError::Agent(format!("HooksDb open: {e}")))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AutoDevError::Agent(format!("HooksDb pragma: {e}")))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS hooks (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                hook_type   TEXT NOT NULL,
                enabled     INTEGER NOT NULL DEFAULT 1,
                priority    INTEGER NOT NULL DEFAULT 100,
                conditions  TEXT NOT NULL DEFAULT '{}',
                action      TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS hook_logs (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                hook_id     TEXT NOT NULL,
                hook_name   TEXT NOT NULL,
                hook_type   TEXT NOT NULL,
                outcome     TEXT NOT NULL,
                output      TEXT,
                error       TEXT,
                executed_at TEXT NOT NULL DEFAULT (datetime('now')),
                duration_ms INTEGER NOT NULL DEFAULT 0
            );
        "#).map_err(|e| AutoDevError::Agent(format!("HooksDb migrate: {e}")))?;
        Ok(())
    }

    pub fn list_all(&self) -> Result<Vec<HookConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks ORDER BY priority ASC"
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb list_all: {e}")))?;
        let rows = stmt.query_map([], row_to_hook)
            .map_err(|e| AutoDevError::Agent(format!("HooksDb query: {e}")))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }

    pub fn list_by_type(&self, hook_type: &HookType) -> Result<Vec<HookConfig>> {
        let type_str = hook_type.as_str();
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks WHERE hook_type = ?1 ORDER BY priority ASC"
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb list_by_type: {e}")))?;
        let rows = stmt.query_map(params![type_str], row_to_hook)
            .map_err(|e| AutoDevError::Agent(format!("HooksDb query: {e}")))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }

    pub fn get(&self, id: &str) -> Result<Option<HookConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks WHERE id = ?1"
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb get: {e}")))?;
        let mut rows = stmt.query_map(params![id], row_to_hook)
            .map_err(|e| AutoDevError::Agent(format!("HooksDb query: {e}")))?;
        Ok(rows.next().and_then(|r| r.ok()))
    }

    pub fn insert(&self, input: &AddHookInput) -> Result<HookConfig> {
        let id         = uuid::Uuid::new_v4().to_string();
        let enabled    = input.enabled.unwrap_or(true) as i64;
        let priority   = input.priority.unwrap_or(100);
        let conditions = serde_json::to_string(
            &input.conditions.clone().unwrap_or_default()
        ).unwrap_or_else(|_| "{}".into());
        let action     = serde_json::to_string(&input.action)
            .map_err(|e| AutoDevError::Agent(format!("HooksDb serialize action: {e}")))?;
        let type_str   = input.hook_type.as_str();

        self.conn.execute(
            "INSERT INTO hooks (id, name, hook_type, enabled, priority, conditions, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, input.name, type_str, enabled, priority, conditions, action],
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb insert: {e}")))?;

        self.get(&id)?.ok_or_else(|| AutoDevError::Agent("HooksDb: insert+get failed".into()))
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE hooks SET enabled = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![enabled as i64, id],
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb set_enabled: {e}")))?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM hooks WHERE id = ?1", params![id])
            .map_err(|e| AutoDevError::Agent(format!("HooksDb delete: {e}")))?;
        Ok(())
    }

    pub fn insert_log(&self, log: &HookExecutionLog) -> Result<()> {
        let outcome = format!("{:?}", log.outcome).to_lowercase();
        self.conn.execute(
            "INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, output, error, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                log.hook_id, log.hook_name, log.hook_type,
                outcome, log.output, log.error, log.duration_ms as i64,
            ],
        ).map_err(|e| AutoDevError::Agent(format!("HooksDb insert_log: {e}")))?;
        Ok(())
    }

    pub fn get_logs(&self, hook_id: Option<&str>, limit: usize) -> Result<Vec<HookExecutionLog>> {
        let lim = limit as i64;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<HookExecutionLog> {
            let outcome_str: String = row.get(4)?;
            let outcome = match outcome_str.as_str() {
                "executed" => HookOutcome::Executed,
                "blocked"  => HookOutcome::Blocked,
                "skipped"  => HookOutcome::Skipped,
                _          => HookOutcome::Failed,
            };
            Ok(HookExecutionLog {
                id:          row.get(0)?,
                hook_id:     row.get(1)?,
                hook_name:   row.get(2)?,
                hook_type:   row.get(3)?,
                outcome,
                output:      row.get(5)?,
                error:       row.get(6)?,
                executed_at: row.get(7)?,
                duration_ms: row.get::<_, i64>(8)? as u64,
            })
        };

        if let Some(id) = hook_id {
            let mut stmt = self.conn.prepare(
                "SELECT id, hook_id, hook_name, hook_type, outcome, output, error, executed_at, duration_ms
                 FROM hook_logs WHERE hook_id = ?1 ORDER BY id DESC LIMIT ?2"
            ).map_err(|e| AutoDevError::Agent(format!("HooksDb get_logs prepare: {e}")))?;
            let rows = stmt.query_map(params![id, lim], map_row)
                .map_err(|e| AutoDevError::Agent(format!("HooksDb get_logs query: {e}")))?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, hook_id, hook_name, hook_type, outcome, output, error, executed_at, duration_ms
                 FROM hook_logs ORDER BY id DESC LIMIT ?1"
            ).map_err(|e| AutoDevError::Agent(format!("HooksDb get_logs prepare: {e}")))?;
            let rows = stmt.query_map(params![lim], map_row)
                .map_err(|e| AutoDevError::Agent(format!("HooksDb get_logs query: {e}")))?;
            rows.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
        }
    }
}

fn row_to_hook(row: &rusqlite::Row<'_>) -> rusqlite::Result<HookConfig> {
    let type_str: String = row.get(2)?;
    let hook_type = parse_hook_type(&type_str);
    let conditions: HookConditions = serde_json::from_str(
        &row.get::<_, String>(5)?
    ).unwrap_or_default();
    let action: HookAction = serde_json::from_str(
        &row.get::<_, String>(6)?
    ).unwrap_or(HookAction::Block { message: Some("Corrupt action".into()) });

    Ok(HookConfig {
        id:         row.get(0)?,
        name:       row.get(1)?,
        hook_type,
        enabled:    row.get::<_, i64>(3)? != 0,
        priority:   row.get(4)?,
        conditions,
        action,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn parse_hook_type(s: &str) -> HookType {
    match s {
        "session_start"         => HookType::SessionStart,
        "session_end"           => HookType::SessionEnd,
        "agent_loop_start"      => HookType::AgentLoopStart,
        "agent_loop_end"        => HookType::AgentLoopEnd,
        "on_error"              => HookType::OnError,
        "pre_tool_use"          => HookType::PreToolUse,
        "post_tool_use"         => HookType::PostToolUse,
        "on_tool_error"         => HookType::OnToolError,
        "on_permission_request" => HookType::OnPermissionRequest,
        "pre_file_read"         => HookType::PreFileRead,
        "post_file_write"       => HookType::PostFileWrite,
        "pre_file_delete"       => HookType::PreFileDelete,
        "on_checkpoint_create"  => HookType::OnCheckpointCreate,
        "on_checkpoint_restore" => HookType::OnCheckpointRestore,
        "pre_commit"            => HookType::PreCommit,
        "post_commit"           => HookType::PostCommit,
        "pre_push"              => HookType::PrePush,
        "on_branch_switch"      => HookType::OnBranchSwitch,
        "on_merge_conflict"     => HookType::OnMergeConflict,
        "pre_plan_step"         => HookType::PrePlanStep,
        "post_plan_step"        => HookType::PostPlanStep,
        "on_plan_activate"      => HookType::OnPlanActivate,
        "on_plan_complete"      => HookType::OnPlanComplete,
        "on_plan_abandon"       => HookType::OnPlanAbandon,
        "on_task_claim"         => HookType::OnTaskClaim,
        "on_task_complete"      => HookType::OnTaskComplete,
        "on_task_fail"          => HookType::OnTaskFail,
        "on_qc_review_start"    => HookType::OnQcReviewStart,
        "on_qc_approve"         => HookType::OnQcApprove,
        "on_qc_fail"            => HookType::OnQcFail,
        _                       => HookType::OnError,
    }
}

// Helper trait to allow `.pipe(Ok)` on Vec
trait Pipe: Sized {
    fn pipe<F, U>(self, f: F) -> U where F: FnOnce(Self) -> U { f(self) }
}
impl<T> Pipe for Vec<T> {}
