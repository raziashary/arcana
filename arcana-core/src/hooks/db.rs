// HooksDb — SQLite persistence for hook configurations and execution logs.
// Stored in ~/.arcana/config.db (hooks + hook_logs tables).
// Phase 3.1

use super::types::{
    AddHookInput, HookAction, HookConditions, HookConfig, HookExecutionLog, HookOutcome, HookType,
};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
use rusqlite::{params, Connection};

pub struct HooksDb {
    conn: Connection,
}

impl HooksDb {
    /// Open `config.db` through its schema owner.
    ///
    /// The `hooks` and `hook_logs` DDL now lives in `db::config_db`'s ladder
    /// (v2). This module used to create them itself, which left three of
    /// `config.db`'s six tables outside the version that claimed to describe
    /// the file.
    pub fn open() -> Result<Self> {
        Ok(Self {
            conn: crate::db::config_db::open()?,
        })
    }

    /// Open the production config schema at an explicit path. Hosts use this
    /// for isolated profiles, and integration tests use it to exercise the
    /// same hook-log lifecycle without touching the user's config database.
    pub fn open_at(db_path: &std::path::Path) -> Result<Self> {
        Ok(Self {
            conn: crate::db::config_db::open_at(db_path)?,
        })
    }

    pub fn list_all(&self) -> Result<Vec<HookConfig>> {
        let operation = OperationTimer::start("database_hooks_list").with_database("config");
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks ORDER BY priority ASC"
        ).map_err(|e| ArcanaError::Agent(format!("HooksDb list_all: {e}")))?;
        let rows = stmt
            .query_map([], row_to_hook)
            .map_err(|e| ArcanaError::Agent(format!("HooksDb query: {e}")))?;
        let hooks = rows.filter_map(|r| r.ok()).collect::<Vec<_>>();
        operation.with_result_count(hooks.len()).succeed();
        Ok(hooks)
    }

    pub fn list_by_type(&self, hook_type: &HookType) -> Result<Vec<HookConfig>> {
        let operation =
            OperationTimer::start("database_hooks_by_type_read").with_database("config");
        let type_str = hook_type.as_str();
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks WHERE hook_type = ?1 ORDER BY priority ASC"
        ).map_err(|e| ArcanaError::Agent(format!("HooksDb list_by_type: {e}")))?;
        let rows = stmt
            .query_map(params![type_str], row_to_hook)
            .map_err(|e| ArcanaError::Agent(format!("HooksDb query: {e}")))?;
        let hooks = rows.filter_map(|r| r.ok()).collect::<Vec<_>>();
        operation.with_result_count(hooks.len()).succeed();
        Ok(hooks)
    }

    pub fn get(&self, id: &str) -> Result<Option<HookConfig>> {
        let operation = OperationTimer::start("database_hook_read").with_database("config");
        let mut stmt = self.conn.prepare(
            "SELECT id, name, hook_type, enabled, priority, conditions, action, created_at, updated_at
             FROM hooks WHERE id = ?1"
        ).map_err(|e| ArcanaError::Agent(format!("HooksDb get: {e}")))?;
        let mut rows = stmt
            .query_map(params![id], row_to_hook)
            .map_err(|e| ArcanaError::Agent(format!("HooksDb query: {e}")))?;
        let hook = rows.next().and_then(|r| r.ok());
        if hook.is_some() {
            operation.with_result_count(1).succeed();
        } else {
            operation.with_result_count(0).set_outcome("not_found");
        }
        Ok(hook)
    }

    pub fn insert(&self, input: &AddHookInput) -> Result<HookConfig> {
        let operation = OperationTimer::start("database_hook_insert").with_database("config");
        if let HookAction::Shell {
            command,
            working_dir,
            ..
        } = &input.action
        {
            crate::platform::shell_portability::validate_shell_command(command)
                .map_err(|e| ArcanaError::Agent(format!("Hook command {e}")))?;
            if let Some(working_dir) = working_dir {
                crate::platform::shell_portability::validate_working_directory(working_dir)
                    .map_err(|e| ArcanaError::Agent(format!("Hook working directory {e}")))?;
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let enabled = input.enabled.unwrap_or(true) as i64;
        let priority = input.priority.unwrap_or(100);
        let conditions = serde_json::to_string(&input.conditions.clone().unwrap_or_default())
            .unwrap_or_else(|_| "{}".into());
        let action = serde_json::to_string(&input.action)
            .map_err(|e| ArcanaError::Agent(format!("HooksDb serialize action: {e}")))?;
        let type_str = input.hook_type.as_str();

        self.conn
            .execute(
                "INSERT INTO hooks (id, name, hook_type, enabled, priority, conditions, action)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![id, input.name, type_str, enabled, priority, conditions, action],
            )
            .map_err(|e| ArcanaError::Agent(format!("HooksDb insert: {e}")))?;

        let hook = self
            .get(&id)?
            .ok_or_else(|| ArcanaError::Agent("HooksDb: insert+get failed".into()))?;
        operation.with_result_count(1).succeed();
        Ok(hook)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        let mut operation =
            OperationTimer::start("database_hook_enable_update").with_database("config");
        let updated_at = crate::db::timestamps::now_rfc3339();
        let changed = self
            .conn
            .execute(
                "UPDATE hooks SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![enabled as i64, updated_at, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("HooksDb set_enabled: {e}")))?;
        operation = operation.with_result_count(changed);
        if changed == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut operation = OperationTimer::start("database_hook_delete").with_database("config");
        let changed = self
            .conn
            .execute("DELETE FROM hooks WHERE id = ?1", params![id])
            .map_err(|e| ArcanaError::Agent(format!("HooksDb delete: {e}")))?;
        operation = operation.with_result_count(changed);
        if changed == 0 {
            operation.set_outcome("not_found");
        } else {
            operation.succeed();
        }
        Ok(())
    }

    pub fn insert_log(&self, log: &HookExecutionLog) -> Result<()> {
        let operation =
            OperationTimer::start("database_hook_execution_log_insert").with_database("config");
        let outcome = format!("{:?}", log.outcome).to_lowercase();
        self.conn.execute(
            "INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, output, error, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                log.hook_id, log.hook_name, log.hook_type,
                outcome, log.output, log.error, log.duration_ms as i64,
            ],
        ).map_err(|e| ArcanaError::Agent(format!("HooksDb insert_log: {e}")))?;
        operation.with_result_count(1).succeed();
        Ok(())
    }

    /// Prune raw execution details strictly older than the retention cutoff.
    /// Invalid timestamps fail closed and are reported to the owning lifecycle.
    pub fn prune_logs_before(&mut self, cutoff: &str) -> Result<usize> {
        let mut operation =
            OperationTimer::start("database_hook_logs_retention_prune").with_database("config");
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| ArcanaError::Agent(format!("Hook log retention transaction: {e}")))?;
        let invalid_rows: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM hook_logs WHERE julianday(executed_at) IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| ArcanaError::Agent(format!("Hook log retention validation: {e}")))?;
        if invalid_rows > 0 {
            return Err(ArcanaError::Agent(
                "Hook log retention refused rows with invalid timestamps".into(),
            ));
        }
        let deleted = tx
            .execute(
                "DELETE FROM hook_logs WHERE julianday(executed_at) < julianday(?1)",
                [cutoff],
            )
            .map_err(|e| ArcanaError::Agent(format!("Hook log retention prune: {e}")))?;
        tx.commit()
            .map_err(|e| ArcanaError::Agent(format!("Hook log retention commit: {e}")))?;
        operation = operation.with_result_count(deleted);
        operation.succeed();
        Ok(deleted)
    }

    pub fn get_logs(&self, hook_id: Option<&str>, limit: usize) -> Result<Vec<HookExecutionLog>> {
        let operation =
            OperationTimer::start("database_hook_execution_logs_read").with_database("config");
        let lim = limit as i64;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<HookExecutionLog> {
            let outcome_str: String = row.get(4)?;
            let outcome = match outcome_str.as_str() {
                "executed" => HookOutcome::Executed,
                "blocked" => HookOutcome::Blocked,
                "skipped" => HookOutcome::Skipped,
                _ => HookOutcome::Failed,
            };
            Ok(HookExecutionLog {
                id: row.get(0)?,
                hook_id: row.get(1)?,
                hook_name: row.get(2)?,
                hook_type: row.get(3)?,
                outcome,
                output: row.get(5)?,
                error: row.get(6)?,
                executed_at: row.get(7)?,
                duration_ms: row.get::<_, i64>(8)? as u64,
            })
        };

        if let Some(id) = hook_id {
            let mut stmt = self.conn.prepare(
                "SELECT id, hook_id, hook_name, hook_type, outcome, output, error, executed_at, duration_ms
                 FROM hook_logs WHERE hook_id = ?1 ORDER BY id DESC LIMIT ?2"
            ).map_err(|e| ArcanaError::Agent(format!("HooksDb get_logs prepare: {e}")))?;
            let rows = stmt
                .query_map(params![id, lim], map_row)
                .map_err(|e| ArcanaError::Agent(format!("HooksDb get_logs query: {e}")))?;
            let logs = rows.filter_map(|r| r.ok()).collect::<Vec<_>>();
            operation.with_result_count(logs.len()).succeed();
            Ok(logs)
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT id, hook_id, hook_name, hook_type, outcome, output, error, executed_at, duration_ms
                 FROM hook_logs ORDER BY id DESC LIMIT ?1"
            ).map_err(|e| ArcanaError::Agent(format!("HooksDb get_logs prepare: {e}")))?;
            let rows = stmt
                .query_map(params![lim], map_row)
                .map_err(|e| ArcanaError::Agent(format!("HooksDb get_logs query: {e}")))?;
            let logs = rows.filter_map(|r| r.ok()).collect::<Vec<_>>();
            operation.with_result_count(logs.len()).succeed();
            Ok(logs)
        }
    }

    /// Read complete retained hook history at or after a UTC timestamp cutoff.
    /// This is used by explicit full analytics export; unlike the UI reader it
    /// has no row limit and propagates corrupt-row errors instead of skipping.
    pub fn get_logs_since(&self, cutoff: &str) -> Result<Vec<HookExecutionLog>> {
        let operation = OperationTimer::start("database_hook_logs_retained_export_read")
            .with_database("config");
        let invalid_timestamps: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM hook_logs WHERE julianday(executed_at) IS NULL",
            [],
            |row| row.get(0),
        )?;
        if invalid_timestamps > 0 {
            return Err(ArcanaError::Agent(
                "hook-log export refused rows with invalid timestamps".into(),
            ));
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, hook_id, hook_name, hook_type, outcome, output, error, executed_at, duration_ms
             FROM hook_logs WHERE julianday(executed_at) >= julianday(?1) ORDER BY id DESC",
        )?;
        let rows = stmt.query_map(params![cutoff], row_to_hook_log)?;
        let logs = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        operation.with_result_count(logs.len()).succeed();
        Ok(logs)
    }
}

fn row_to_hook_log(row: &rusqlite::Row<'_>) -> rusqlite::Result<HookExecutionLog> {
    let outcome_str: String = row.get(4)?;
    let outcome = match outcome_str.as_str() {
        "executed" => HookOutcome::Executed,
        "blocked" => HookOutcome::Blocked,
        "skipped" => HookOutcome::Skipped,
        _ => HookOutcome::Failed,
    };
    Ok(HookExecutionLog {
        id: row.get(0)?,
        hook_id: row.get(1)?,
        hook_name: row.get(2)?,
        hook_type: row.get(3)?,
        outcome,
        output: row.get(5)?,
        error: row.get(6)?,
        executed_at: row.get(7)?,
        duration_ms: row.get::<_, i64>(8)? as u64,
    })
}

fn row_to_hook(row: &rusqlite::Row<'_>) -> rusqlite::Result<HookConfig> {
    let type_str: String = row.get(2)?;
    let hook_type = parse_hook_type(&type_str);
    let conditions: HookConditions =
        serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default();
    let action: HookAction =
        serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or(HookAction::Block {
            message: Some("Corrupt action".into()),
        });

    Ok(HookConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        hook_type,
        enabled: row.get::<_, i64>(3)? != 0,
        priority: row.get(4)?,
        conditions,
        action,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn parse_hook_type(s: &str) -> HookType {
    match s {
        "session_start" => HookType::SessionStart,
        "session_end" => HookType::SessionEnd,
        "agent_loop_start" => HookType::AgentLoopStart,
        "agent_loop_end" => HookType::AgentLoopEnd,
        "on_error" => HookType::OnError,
        "pre_tool_use" => HookType::PreToolUse,
        "post_tool_use" => HookType::PostToolUse,
        "on_tool_error" => HookType::OnToolError,
        "on_permission_request" => HookType::OnPermissionRequest,
        "pre_file_read" => HookType::PreFileRead,
        "post_file_write" => HookType::PostFileWrite,
        "pre_file_delete" => HookType::PreFileDelete,
        "on_checkpoint_create" => HookType::OnCheckpointCreate,
        "on_checkpoint_restore" => HookType::OnCheckpointRestore,
        "pre_commit" => HookType::PreCommit,
        "post_commit" => HookType::PostCommit,
        "pre_push" => HookType::PrePush,
        "on_branch_switch" => HookType::OnBranchSwitch,
        "on_merge_conflict" => HookType::OnMergeConflict,
        "pre_plan_step" => HookType::PrePlanStep,
        "post_plan_step" => HookType::PostPlanStep,
        "on_plan_activate" => HookType::OnPlanActivate,
        "on_plan_complete" => HookType::OnPlanComplete,
        "on_plan_abandon" => HookType::OnPlanAbandon,
        "on_task_claim" => HookType::OnTaskClaim,
        "on_task_complete" => HookType::OnTaskComplete,
        "on_task_fail" => HookType::OnTaskFail,
        "on_qc_review_start" => HookType::OnQcReviewStart,
        "on_qc_approve" => HookType::OnQcApprove,
        "on_qc_fail" => HookType::OnQcFail,
        _ => HookType::OnError,
    }
}

#[cfg(test)]
mod portability_tests {
    use super::*;

    fn db() -> HooksDb {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE hooks (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, hook_type TEXT NOT NULL,
                enabled INTEGER NOT NULL, priority INTEGER NOT NULL,
                conditions TEXT NOT NULL, action TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )
        .unwrap();
        HooksDb { conn }
    }

    #[test]
    fn file_backed_defaults_and_set_enabled_reader_use_canonical_timestamps() {
        let dir = tempfile::tempdir().expect("isolated config DB directory");
        let conn = crate::db::config_db::open_at(&dir.path().join("config.db"))
            .expect("production config DB opener");
        let db = HooksDb { conn };
        let inserted = db
            .insert(&AddHookInput {
                name: "timestamp hook".into(),
                hook_type: HookType::PreToolUse,
                enabled: Some(true),
                priority: None,
                conditions: None,
                action: HookAction::Block { message: None },
            })
            .expect("production hook writer and reader");
        assert!(inserted.created_at.ends_with('Z'));
        assert_eq!(inserted.created_at.len(), 24);
        assert_eq!(inserted.updated_at, inserted.created_at);

        db.set_enabled(&inserted.id, false).expect("update hook");
        let updated = db.get(&inserted.id).expect("read updated hook").unwrap();
        assert!(updated.updated_at.ends_with('Z'));
        assert_eq!(updated.updated_at.len(), 24);
    }

    #[test]
    fn custom_shell_hook_writer_rejects_nonportable_command_before_persisting() {
        let db = db();
        let input = AddHookInput {
            name: "custom hook".into(),
            hook_type: HookType::PreToolUse,
            enabled: Some(true),
            priority: None,
            conditions: None,
            action: HookAction::Shell {
                command: "npm test && npm lint".into(),
                working_dir: None,
                timeout_secs: None,
            },
        };
        let error = db
            .insert(&input)
            .expect_err("writer must validate saved command");
        assert!(error.to_string().contains("non-portable shell operator"));
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM hooks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn legacy_persisted_shell_hook_is_refused_by_production_reader_and_executor() {
        let directory = tempfile::tempdir().expect("isolated config database directory");
        let db = HooksDb::open_at(&directory.path().join("config.db"))
            .expect("open production config database");
        let action = serde_json::to_string(&HookAction::Shell {
            command: "echo should-not-run && exit 0".into(),
            working_dir: None,
            timeout_secs: Some(2),
        })
        .expect("serialize legacy shell action");

        // Insert directly to represent a pre-validator persisted row. New
        // writes are rejected by HooksDb::insert; this exercises the reader
        // and runtime guard for data already present on disk.
        db.conn
            .execute(
                "INSERT INTO hooks (id, name, hook_type, enabled, priority, conditions, action)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "legacy-portability-hook",
                    "Legacy portability hook",
                    HookType::PreToolUse.as_str(),
                    1,
                    100,
                    "{}",
                    action
                ],
            )
            .expect("seed pre-validator persisted hook");

        let hook = db
            .get("legacy-portability-hook")
            .expect("read legacy hook through production reader")
            .expect("legacy hook exists");
        let context = crate::hooks::types::HookContext {
            hook_type: HookType::PreToolUse.as_str().into(),
            timestamp: chrono::Utc::now(),
            session_id: None,
            agent_mode: None,
            tool_call: None,
            file_path: None,
            task_id: None,
            worker_id: None,
            extra: serde_json::Value::Null,
        };

        let log = crate::hooks::executor::HookExecutor::execute(&hook, &context).await;
        assert_eq!(log.outcome, HookOutcome::Failed);
        assert!(log
            .error
            .as_deref()
            .is_some_and(|error| error.contains("non-portable shell operator")));
        assert!(
            log.output.is_none(),
            "a refused hook must not spawn its shell"
        );
    }
}

#[cfg(test)]
mod observability_tests {
    use super::*;

    #[test]
    fn hook_configuration_and_execution_log_operations_emit_private_result_spans() {
        let directory = tempfile::tempdir().expect("isolated config database directory");
        let db = HooksDb {
            conn: crate::db::config_db::open_at(&directory.path().join("config.db"))
                .expect("open production config database"),
        };
        let input = AddHookInput {
            name: "private-hook-name-marker".into(),
            hook_type: HookType::PreToolUse,
            enabled: Some(true),
            priority: Some(50),
            conditions: None,
            action: HookAction::Block {
                message: Some("private-hook-action-marker".into()),
            },
        };

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let inserted = db.insert(&input).expect("insert hook");
            let all = db.list_all().expect("list hooks");
            let by_type = db
                .list_by_type(&HookType::PreToolUse)
                .expect("list hooks by type");
            let fetched = db.get(&inserted.id).expect("read hook");
            let missing = db
                .get("private-missing-hook-id-marker")
                .expect("missing hook");
            db.set_enabled(&inserted.id, false).expect("disable hook");
            db.set_enabled("private-missing-hook-id-marker", false)
                .expect("disable missing hook");
            let execution = HookExecutionLog {
                id: 0,
                hook_id: inserted.id.clone(),
                hook_name: "private-log-hook-name-marker".into(),
                hook_type: "pre_tool_use".into(),
                outcome: HookOutcome::Failed,
                output: Some("private-hook-output-marker".into()),
                error: Some("private-hook-error-marker".into()),
                executed_at: String::new(),
                duration_ms: 37,
            };
            db.insert_log(&execution).expect("insert execution log");
            let by_hook = db
                .get_logs(Some(&inserted.id), 10)
                .expect("read hook execution logs");
            let recent = db.get_logs(None, 10).expect("read recent execution logs");
            db.conn
                .execute_batch(
                    "CREATE TRIGGER reject_test_hook_log BEFORE INSERT ON hook_logs
                     BEGIN SELECT RAISE(ABORT, 'private-hook-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_log = db.insert_log(&execution).is_err();
            db.delete(&inserted.id).expect("delete hook");
            db.delete(&inserted.id).expect("delete absent hook");
            (all, by_type, fetched, missing, by_hook, recent, failed_log)
        });

        assert_eq!(observed.0.len(), 1);
        assert_eq!(observed.1.len(), 1);
        assert!(observed.2.is_some());
        assert!(observed.3.is_none());
        assert_eq!(observed.4.len(), 1);
        assert_eq!(observed.5.len(), 1);
        assert_eq!(observed.4[0].executed_at.len(), 24);
        assert!(observed.4[0].executed_at.ends_with('Z'));
        crate::db::timestamps::parse_db_timestamp(&observed.4[0].executed_at)
            .expect("production hook-log default is readable by the shared timestamp parser");
        assert!(observed.6);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_hook_insert",
            "database_hooks_list",
            "database_hooks_by_type_read",
            "database_hook_read",
            "database_hook_enable_update",
            "database_hook_execution_log_insert",
            "database_hook_execution_logs_read",
            "database_hook_delete",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing hook database span {operation}"
            );
        }
        let not_found = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_hook_read"
                    && event["fields"]["outcome"] == "not_found"
            })
            .expect("missing hook read is distinguishable");
        assert_eq!(not_found["fields"]["result_count"], 0);
        let failed = operations
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_hook_execution_log_insert"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("hook-log database error span");
        assert_eq!(failed["fields"]["result_count_observed"], false);

        let serialized = serde_json::to_string(&events).expect("serialize hook telemetry");
        for private_value in [
            "private-hook-name-marker",
            "private-hook-action-marker",
            "private-missing-hook-id-marker",
            "private-log-hook-name-marker",
            "private-hook-output-marker",
            "private-hook-error-marker",
            "private-hook-trigger-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }
}

#[cfg(test)]
mod retention_tests {
    use super::*;

    fn database() -> (tempfile::TempDir, HooksDb) {
        let directory = tempfile::tempdir().expect("isolated config database directory");
        let db = HooksDb {
            conn: crate::db::config_db::open_at(&directory.path().join("config.db"))
                .expect("open production config database"),
        };
        (directory, db)
    }

    fn insert_log(db: &HooksDb, timestamp: &str) {
        db.conn
            .execute(
                "INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, executed_at)
                 VALUES ('hook', 'name', 'pre_tool_use', 'executed', ?1)",
                [timestamp],
            )
            .expect("insert hook log fixture");
    }

    #[test]
    fn hook_log_retention_keeps_the_exact_cutoff_and_newer_rows() {
        let (_directory, mut db) = database();
        let cutoff = "2026-07-06T12:00:00Z";
        insert_log(&db, "2026-07-06T11:59:59Z");
        insert_log(&db, cutoff);
        insert_log(&db, "2026-07-06T12:00:01Z");

        assert_eq!(db.prune_logs_before(cutoff).expect("prune old logs"), 1);
        assert_eq!(db.get_logs(None, 10).expect("read retained logs").len(), 2);
    }

    #[test]
    fn hook_log_export_reads_complete_retained_rows_and_refuses_invalid_dates() {
        let (_directory, db) = database();
        insert_log(&db, "2026-07-06T11:59:59Z");
        insert_log(&db, "2026-07-06T12:00:00Z");
        insert_log(&db, "malformed");

        let error = db
            .get_logs_since("2026-07-06T12:00:00Z")
            .expect_err("invalid timestamps must not disappear from a full export");
        assert!(error.to_string().contains("invalid timestamps"));

        db.conn
            .execute("DELETE FROM hook_logs WHERE executed_at = 'malformed'", [])
            .expect("remove malformed isolated fixture row");
        let rows = db
            .get_logs_since("2026-07-06T12:00:00Z")
            .expect("read complete retained rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].executed_at, "2026-07-06T12:00:00Z");
    }

    #[test]
    fn invalid_hook_log_timestamp_fails_closed_without_pruning_valid_rows() {
        let (_directory, mut db) = database();
        insert_log(&db, "2020-01-01T00:00:00Z");
        insert_log(&db, "malformed");

        let (error, events) = crate::observability::timing::capture_json_events(|| {
            db.prune_logs_before("2026-07-06T12:00:00Z")
                .expect_err("unknown age fails closed")
        });
        assert!(error.to_string().contains("invalid timestamps"));
        assert!(events.iter().any(|event| {
            event["fields"]["operation"] == "database_hook_logs_retention_prune"
                && event["fields"]["outcome"] == "error"
                && event["fields"]["database"] == "config"
        }));
        assert_eq!(
            db.get_logs(None, 10)
                .expect("read rows after refusal")
                .len(),
            2
        );
    }

    #[test]
    fn hook_log_retention_serializes_with_an_active_database_writer() {
        let directory = tempfile::tempdir().expect("isolated config database directory");
        let path = directory.path().join("config.db");
        let db = HooksDb {
            conn: crate::db::config_db::open_at(&path).expect("open production config database"),
        };
        let mut writer = crate::db::open_tuned(&path).expect("open concurrent writer");
        let tx = writer
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("begin active writer transaction");
        tx.execute(
            "INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, executed_at)
             VALUES ('hook', 'name', 'pre_tool_use', 'executed', '2020-01-01T00:00:00Z')",
            [],
        )
        .expect("write expired hook row while holding writer lock");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let pruner = std::thread::spawn(move || {
            let mut db = db;
            started_tx.send(()).expect("signal pruning thread start");
            let deleted = db.prune_logs_before("2026-07-06T12:00:00Z");
            (db, deleted)
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("pruning thread started");
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            !pruner.is_finished(),
            "cleanup waits for the active SQLite writer transaction"
        );
        tx.commit().expect("commit active writer");
        let (db, deleted) = pruner.join().expect("join retention worker");
        assert_eq!(deleted.expect("prune after writer commits"), 1);
        assert!(db
            .get_logs(None, 10)
            .expect("read after concurrent retention")
            .is_empty());
    }
}

#[cfg(test)]
mod analytics_timestamp_pair_tests {
    use super::*;
    use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

    #[test]
    fn analytics_timestamp_pairs_hook_writer_and_readers_cover_current_and_legacy_year_rows() {
        let directory = tempfile::tempdir().expect("isolated config database directory");
        let mut db = HooksDb::open_at(&directory.path().join("config.db"))
            .expect("open production config database");

        let offsets = [
            FixedOffset::west_opt(12 * 60 * 60).unwrap(),
            FixedOffset::east_opt(0).unwrap(),
            FixedOffset::east_opt(14 * 60 * 60).unwrap(),
        ];
        let mut corpus = Vec::with_capacity(1_110);
        let year_start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        for day_index in 0..366 {
            let day = year_start + chrono::Duration::days(day_index);
            for offset in offsets {
                corpus.push(
                    offset
                        .from_local_datetime(&day.and_hms_opt(0, 0, 0).unwrap())
                        .single()
                        .expect("fixed-offset local midnight")
                        .with_timezone(&Utc),
                );
            }
        }
        for boundary in [
            "2024-03-10T01:59:59-05:00",
            "2024-03-10T03:00:00-04:00",
            "2024-11-03T01:59:59-04:00",
            "2024-11-03T01:00:00-05:00",
            "2024-04-07T02:59:59+11:00",
            "2024-04-07T02:00:00+10:00",
            "2024-10-06T01:59:59+10:00",
            "2024-10-06T03:00:00+11:00",
        ] {
            corpus.push(
                chrono::DateTime::parse_from_rfc3339(boundary)
                    .expect("DST edge timestamp")
                    .with_timezone(&Utc),
            );
        }
        assert!(corpus.len() < 1_200, "keep the isolated fixture bounded");

        // The hook-log API does not accept an explicit timestamp; seed the
        // historical representations directly to test its actual read path.
        // The production writer/read pair below exercises its SQLite-now path.
        {
            let transaction = db.conn.transaction().expect("begin isolated corpus write");
            {
                let mut insert = transaction
                    .prepare(
                        "INSERT INTO hook_logs
                         (hook_id, hook_name, hook_type, outcome, executed_at)
                         VALUES (?1, ?2, 'pre_tool_use', 'executed', ?3)",
                    )
                    .expect("prepare historical log fixture writer");
                for (index, instant) in corpus.iter().copied().enumerate() {
                    let timestamp = if index % 2 == 0 {
                        crate::db::timestamps::format_utc_timestamp(instant)
                    } else {
                        instant.format("%Y-%m-%d %H:%M:%S").to_string()
                    };
                    insert
                        .execute(params![
                            format!("timestamp-corpus-{index}"),
                            format!("timestamp-corpus-{index}"),
                            timestamp,
                        ])
                        .expect("seed canonical or legacy retained row");
                }
            }
            transaction
                .commit()
                .expect("commit isolated history corpus");
        }

        let cutoff = "2024-07-01T00:00:00Z";
        let expected_since_cutoff = corpus
            .iter()
            .filter(|instant| **instant >= Utc.with_ymd_and_hms(2024, 7, 1, 0, 0, 0).unwrap())
            .count();
        let retained = db
            .get_logs_since("2023-12-31T00:00:00Z")
            .expect("production retained-export reader");
        assert_eq!(retained.len(), corpus.len());
        let filtered = db
            .get_logs_since(cutoff)
            .expect("production cutoff reader over mixed timestamp formats");
        assert_eq!(filtered.len(), expected_since_cutoff);
        for (index, instant) in corpus.iter().copied().enumerate() {
            let row = retained
                .iter()
                .find(|row| row.hook_id == format!("timestamp-corpus-{index}"))
                .expect("each persisted corpus row is returned");
            assert_eq!(
                crate::db::timestamps::parse_db_timestamp(&row.executed_at)
                    .expect("reader timestamp remains parseable"),
                instant,
                "hook log historical read #{index}"
            );
        }

        let supplied_timestamp = "2001-01-01T00:00:00.000Z";
        let execution = HookExecutionLog {
            id: 0,
            hook_id: "production-writer-hook".into(),
            hook_name: "production writer timestamp".into(),
            hook_type: "pre_tool_use".into(),
            outcome: HookOutcome::Executed,
            output: None,
            error: None,
            executed_at: supplied_timestamp.into(),
            duration_ms: 1,
        };
        db.insert_log(&execution)
            .expect("production execution-log writer");
        let latest = db
            .get_logs(Some("production-writer-hook"), 1)
            .expect("production per-hook reader");
        assert_eq!(latest.len(), 1);
        let persisted: String = db
            .conn
            .query_row(
                "SELECT executed_at FROM hook_logs WHERE id=?1",
                [latest[0].id],
                |row| row.get(0),
            )
            .expect("read actual production writer output");
        assert_eq!(latest[0].executed_at, persisted);
        assert_eq!(persisted.len(), 24);
        assert!(persisted.ends_with('Z'));
        assert_ne!(
            persisted, supplied_timestamp,
            "the current writer uses the schema's SQLite timestamp, not the model field"
        );
        crate::db::timestamps::parse_db_timestamp(&persisted)
            .expect("production SQLite timestamp is canonical and parseable");
        let exported = db
            .get_logs_since("2023-12-31T00:00:00Z")
            .expect("full export includes current production write");
        assert!(exported.iter().any(|row| row.id == latest[0].id));
    }
}
