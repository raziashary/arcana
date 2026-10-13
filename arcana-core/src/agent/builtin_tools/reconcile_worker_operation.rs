//! Supervisor-only decision for an uncertain task-attributed side effect.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::runs::{OperationReconciliation, RunsDb};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ReconcileWorkerOperationTool;

#[async_trait]
impl Tool for ReconcileWorkerOperationTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        // This appends to the recovery journal itself. Journaling this
        // reconciliation would recursively create another uncertain effect.
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "reconcile_worker_operation"
    }

    fn description(&self) -> &str {
        "Supervisor-only: after inspecting the exact task, workspace/checkpoint and Git state, persist one decision for an unknown worker side effect. Use reconciled when the effect is verified complete, and do_not_retry when uncertainty remains unsafe. Arcana verifies sealed host snapshots and local file state before granting completion. Retry authorization remains held until operation-bound execution admission can revalidate it; a Supervisor assertion alone cannot permit replay. Legacy/unprovable effects remain held."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "run_id": {"type": "string", "minLength": 1, "maxLength": 128},
                "intent_cursor": {"type": "integer", "minimum": 1},
                "decision": {
                    "type": "string",
                    "enum": ["reconciled", "retry_safe", "do_not_retry"]
                },
                "reason_code": {
                    "type": "string",
                    "enum": [
                        "effect_verified_complete",
                        "idempotent_retry_verified",
                        "unsafe_to_repeat"
                    ]
                }
            },
            "required": ["run_id", "intent_cursor", "decision", "reason_code"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        if ctx.agent_role.trim() != "supervisor" {
            return ToolResult::categorized_error(
                "supervisor_required",
                "authorization",
                "Only the Supervisor can reconcile an uncertain worker operation.",
                "Ask the Supervisor to inspect the task and its workspace/Git state.",
            );
        }
        let run_id = match input["run_id"].as_str() {
            Some(value) if !value.trim().is_empty() => value,
            _ => return ToolResult::error("'run_id' is required"),
        };
        let intent_cursor = match input["intent_cursor"].as_i64() {
            Some(value) if value > 0 => value,
            _ => return ToolResult::error("'intent_cursor' must be a positive integer"),
        };
        let decision = match input["decision"].as_str() {
            Some("reconciled") => OperationReconciliation::Reconciled,
            Some("retry_safe") => OperationReconciliation::RetrySafe,
            Some("do_not_retry") => OperationReconciliation::DoNotRetry,
            _ => {
                return ToolResult::error(
                    "'decision' must be reconciled, retry_safe, or do_not_retry",
                )
            }
        };
        let reason_code = match input["reason_code"].as_str() {
            Some(value) => value,
            None => return ToolResult::error("'reason_code' is required"),
        };
        let workspace =
            match crate::platform::workspace::resolve_workspace_dir(Some(&ctx.working_dir)) {
                Ok(workspace) => workspace,
                Err(_) => {
                    return ToolResult::categorized_error(
                        "recovery_workspace_unavailable",
                        "recovery_conflict",
                        "The selected workspace cannot be verified.",
                        "Select the original workspace and leave the task held.",
                    )
                }
            };
        let lease = if decision != OperationReconciliation::DoNotRetry {
            match crate::platform::workspace_write::WorkspaceWriteLease::acquire(&workspace).await {
                Ok(lease) => Some(lease),
                Err(_) => {
                    return ToolResult::categorized_error(
                        "recovery_workspace_busy",
                        "recovery_conflict",
                        "The host could not acquire the original workspace write lease.",
                        "Wait for managed writers to finish and leave the task held.",
                    )
                }
            }
        } else {
            None
        };
        let db =
            match RunsDb::open() {
                Ok(db) => db,
                Err(_) => return ToolResult::categorized_error(
                    "operation_reconciliation_unavailable",
                    "durability",
                    "Arcana could not open the worker recovery journal.",
                    "Do not retry or reassign this task until the recovery journal is available.",
                ),
            };
        match db.reconcile_operation_as_supervisor(
            &ctx.agent_role,
            run_id,
            intent_cursor,
            decision,
            reason_code,
            lease.as_ref().map(|lease| (workspace.as_path(), lease)),
        ) {
            Ok(decision_cursor) => ToolResult::success(
                json!({
                    "run_id": run_id,
                    "intent_cursor": intent_cursor,
                    "decision_cursor": decision_cursor,
                    "decision": decision.as_str(),
                    "reason_code": reason_code,
                    "retry_permitted": decision == OperationReconciliation::RetrySafe,
                    "operation_complete": decision == OperationReconciliation::Reconciled
                })
                .to_string(),
            ),
            Err(_) => ToolResult::categorized_error(
                "operation_reconciliation_rejected",
                "recovery_conflict",
                "The requested operation decision was stale, already resolved, or did not match the journal entry.",
                "Re-read the Supervisor health journal. Leave the task held if any side effect remains uncertain.",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(role: &str) -> ToolContext {
        ToolContext {
            session_id: "reconcile-test".into(),
            working_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            call_id: "reconcile-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: role.into(),
            run_token_budget: None,
        }
    }

    #[tokio::test]
    async fn workers_cannot_reconcile_an_unknown_side_effect() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = RunsDb::open().expect("open run journal");
        let run_id = runs.begin("worker_tick", Some("worker-backend")).unwrap();
        let cursor = runs
            .record_operation_intent(&run_id, Some(77), "op-1", "write_file", "FileSystem", &[])
            .expect("record operation");
        let result = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": run_id,
                    "intent_cursor": cursor,
                    "decision": "retry_safe",
                    "reason_code": "idempotent_retry_verified"
                }),
                &context("worker-backend"),
            )
            .await;
        assert!(result.is_error);
        assert!(result.output.contains("supervisor_required"));
        assert_eq!(
            runs.unresolved_task_side_effect(77).unwrap().unwrap().2,
            cursor,
            "denial must leave the operation quarantined"
        );

        let denied = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": run_id,
                    "intent_cursor": cursor,
                    "decision": "retry_safe",
                    "reason_code": "idempotent_retry_verified"
                }),
                &context("supervisor"),
            )
            .await;
        assert!(denied.is_error, "legacy intents cannot authorize replay");
        assert!(runs.unresolved_task_side_effect(77).unwrap().is_some());
        let page = runs.operation_journal_page(&run_id, 0, 10).unwrap();
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::Unresolved
        );
        assert_eq!(page.entries[0].reconciliation_actor.as_deref(), None);
        assert_eq!(page.entries[0].reconciliation_reason.as_deref(), None);
    }

    #[tokio::test]
    async fn forged_sql_reconciliation_is_rejected_and_do_not_retry_stays_held() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let runs = RunsDb::open().expect("open run journal");
        let run_id = runs.begin("worker_tick", Some("worker-backend")).unwrap();
        let cursor = runs
            .record_operation_intent(&run_id, Some(78), "op-2", "append", "FileSystem", &[])
            .expect("record operation");
        runs.conn
            .execute_batch(
                "CREATE TRIGGER force_reconciliation_insert_failure
                 BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type = 'reconciliation'
                 BEGIN SELECT RAISE(ABORT, 'injected reconciliation failure'); END;",
            )
            .expect("inject database failure");
        let failed = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": run_id,
                    "intent_cursor": cursor,
                    "decision": "do_not_retry",
                    "reason_code": "unsafe_to_repeat"
                }),
                &context("supervisor"),
            )
            .await;
        assert!(failed.is_error, "injected write failure must be surfaced");
        runs.conn
            .execute_batch("DROP TRIGGER force_reconciliation_insert_failure;")
            .expect("remove injected trigger");
        let forged = runs.conn.execute(
            "INSERT INTO operation_journal
             (run_id, task_id, record_type, parent_id, reconciliation_state,
              reconciliation_actor, reconciliation_reason, created_at)
             VALUES (?1, ?2, 'reconciliation', ?3, 'retry_safe', 'supervisor',
                     'idempotent_retry_verified', '2026-10-03T00:00:00Z')",
            rusqlite::params![run_id, 78, cursor],
        );
        assert!(
            forged.is_err(),
            "raw SQL must not forge Supervisor authority"
        );
        let result = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": run_id,
                    "intent_cursor": cursor,
                    "decision": "do_not_retry",
                    "reason_code": "unsafe_to_repeat"
                }),
                &context("supervisor"),
            )
            .await;
        assert!(!result.is_error, "Supervisor may persist the hold decision");
        assert_eq!(
            runs.unresolved_task_side_effect(78).unwrap().unwrap().2,
            cursor,
            "do_not_retry must keep the task quarantined"
        );
    }

    #[tokio::test]
    async fn host_verifies_registered_write_after_lost_acknowledgement() {
        use crate::agent::tool_trait::AgentIdentity;
        use crate::agent::{
            builtin_tools, permission, tool_executor::ToolExecutor, tool_registry::ToolRegistry,
        };
        use crate::db::tasks::{CreateTaskInput, TasksDb};
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(workspace.path()).unwrap();
        let original_head = repo
            .find_reference("HEAD")
            .unwrap()
            .symbolic_target()
            .unwrap()
            .to_owned();
        let tasks = TasksDb::open().unwrap();
        let task = tasks
            .create(CreateTaskInput {
                title: "snapshot recovery".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "recover without replay".into(),
                created_by: None,
            })
            .unwrap();
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .unwrap();
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .unwrap();
        let runs = RunsDb::open().unwrap();
        let run = runs
            .begin_worker_tick("worker-backend", "snapshot-session")
            .unwrap();
        let mut registry = ToolRegistry::new();
        builtin_tools::register_all(&mut registry);
        let mut executor = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "snapshot-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run.clone()));
        executor.set_task_id(Some(task.id));
        let acknowledged = executor
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                json!({"path":"acknowledged.txt", "content":"acknowledged fixture"}),
                "acknowledged-write".into(),
            )])
            .await
            .unwrap();
        assert!(
            !acknowledged[0].1.is_error,
            "normal mutation must persist its outcome: {}",
            acknowledged[0].1.output
        );
        let acknowledged_page = RunsDb::open()
            .unwrap()
            .operation_journal_page(&run, 0, 10)
            .unwrap();
        assert!(acknowledged_page.entries[0].before_snapshot.is_some());
        assert!(acknowledged_page.entries[0].after_snapshot.is_some());
        assert_eq!(
            acknowledged_page.entries[0].changed_paths,
            vec!["acknowledged.txt".to_owned()]
        );
        runs.conn.execute_batch("CREATE TRIGGER lose_write_ack BEFORE INSERT ON operation_journal WHEN NEW.record_type = 'outcome' BEGIN SELECT RAISE(ABORT, 'lost ack'); END;").unwrap();
        let content = "private recovery content canary";
        let result = executor
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                json!({"path":"target/generated.txt", "content":content}),
                "write-recovery".into(),
            )])
            .await
            .unwrap();
        assert!(result[0].1.is_error, "lost acknowledgement must surface");
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("target/generated.txt")).unwrap(),
            content
        );
        let page = RunsDb::open()
            .unwrap()
            .operation_journal_page(&run, 0, 10)
            .unwrap();
        let entry = &page.entries[1];
        assert!(entry.before_snapshot.is_some());
        assert!(entry.after_snapshot.is_none());
        let sealed = entry.before_snapshot.as_ref().unwrap().encode().unwrap();
        assert!(!sealed.contains(content));
        assert!(!sealed.contains(&workspace.path().to_string_lossy().to_string()));
        let mut malformed: Value = serde_json::from_str(&sealed).unwrap();
        malformed["version"] = json!(99);
        assert!(
            crate::agent::recovery_snapshot::RecoverySnapshot::decode(&malformed.to_string())
                .is_err()
        );
        malformed["version"] = json!(1);
        malformed["unknown_field"] = json!(true);
        assert!(
            crate::agent::recovery_snapshot::RecoverySnapshot::decode(&malformed.to_string())
                .is_err()
        );
        assert!(
            crate::agent::recovery_snapshot::RecoverySnapshot::decode(&" ".repeat(32769)).is_err()
        );
        let cursor = entry.cursor;
        let mut supervisor = context("supervisor");
        supervisor.working_dir = workspace.path().to_string_lossy().into_owned();
        let request = json!({"run_id":run,"intent_cursor":cursor,"decision":"reconciled","reason_code":"effect_verified_complete"});
        assert!(
            ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor)
                .await
                .is_error,
            "live run remains held"
        );
        tasks
            .update_status_for_worker_claim(
                task.id,
                "worker-backend",
                "needs_info",
                Some("worker-backend"),
                Some("interrupted"),
            )
            .unwrap();
        runs.finish_worker_tick(&run, "interrupted", "interrupted", None)
            .unwrap();
        supervisor.working_dir = other.path().to_string_lossy().into_owned();
        assert!(
            ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor)
                .await
                .is_error,
            "another workspace cannot resolve the effect"
        );
        supervisor.working_dir = workspace.path().to_string_lossy().into_owned();
        std::fs::write(
            workspace.path().join("target/generated.txt"),
            "intervening host change",
        )
        .unwrap();
        let retry = json!({"run_id":run,"intent_cursor":cursor,"decision":"retry_safe","reason_code":"idempotent_retry_verified"});
        assert!(
            ReconcileWorkerOperationTool
                .execute(retry, &supervisor)
                .await
                .is_error,
            "generated target changes must block retry"
        );
        assert!(
            ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor)
                .await
                .is_error
        );
        assert!(RunsDb::open()
            .unwrap()
            .unresolved_task_side_effect(task.id)
            .unwrap()
            .is_some());
        std::fs::write(workspace.path().join("target/generated.txt"), content).unwrap();
        repo.set_head("refs/heads/intervening").unwrap();
        assert!(
            ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor)
                .await
                .is_error,
            "Git branch changes block reconciliation"
        );
        repo.set_head(&original_head).unwrap();
        let config = crate::db::config_db::open().unwrap();
        config.execute("INSERT INTO hooks (id,name,hook_type,enabled,priority,conditions,action) VALUES ('malformed-recovery-hook','guard','PreToolUse',1,'bad','{}','{}')", []).unwrap();
        assert!(
            ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor)
                .await
                .is_error,
            "malformed enabled hooks cannot be treated as absent"
        );
        config
            .execute("DELETE FROM hooks WHERE id='malformed-recovery-hook'", [])
            .unwrap();
        let accepted = ReconcileWorkerOperationTool
            .execute(request.clone(), &supervisor)
            .await;
        assert!(
            !accepted.is_error,
            "verified actual file should reconcile: {}",
            accepted.output
        );
        let response: Value = serde_json::from_str(&accepted.output).unwrap();
        assert_eq!(response["operation_complete"], true);
        assert_eq!(response["retry_permitted"], false);
        assert!(
            ReconcileWorkerOperationTool
                .execute(request, &supervisor)
                .await
                .is_error,
            "one durable decision only"
        );
        let reopened = RunsDb::open().unwrap();
        assert!(reopened
            .unresolved_task_side_effect(task.id)
            .unwrap()
            .is_none());
        let resolved = reopened.operation_journal_page(&run, 0, 10).unwrap();
        assert_eq!(
            resolved.entries[1].reconciliation,
            OperationReconciliation::Reconciled
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("target/generated.txt")).unwrap(),
            content
        );

        // A grant does not execute anything. The replacement executor must
        // revalidate unchanged state and consume the exact grant under its lease.
        let retry_run = runs
            .begin_worker_tick("worker-backend", "snapshot-retry-session")
            .unwrap();
        let expected =
            crate::agent::recovery_snapshot::RecoverySnapshot::file_expectation("new content");
        let snapshot = crate::agent::recovery_snapshot::RecoverySnapshot::capture(
            task.id,
            "snapshot-retry-session",
            workspace.path(),
            Some(("target/retry.txt", &expected, true)),
        )
        .unwrap();
        let retry_cursor = runs
            .record_operation_intent_with_snapshot(
                &retry_run,
                Some(task.id),
                "retry-write",
                "write_file",
                "Write",
                &["target/retry.txt".into()],
                Some(&snapshot),
                None,
            )
            .unwrap();
        runs.finish_worker_tick(&retry_run, "interrupted", "interrupted", None)
            .unwrap();
        assert!(
            runs.reconcile_operation_as_supervisor(
                "supervisor",
                &retry_run,
                i64::MIN,
                OperationReconciliation::RetrySafe,
                "idempotent_retry_verified",
                None
            )
            .is_err(),
            "invalid cursor must fail without arithmetic overflow"
        );
        let retry = json!({"run_id":retry_run,"intent_cursor":retry_cursor,"decision":"retry_safe","reason_code":"idempotent_retry_verified"});
        let accepted = ReconcileWorkerOperationTool
            .execute(retry, &supervisor)
            .await;
        assert!(
            !accepted.is_error,
            "unchanged state permits one explicit host-verified retry: {}",
            accepted.output
        );
        assert!(runs.unresolved_task_side_effect(task.id).unwrap().is_some());
        assert!(
            !workspace.path().join("target/retry.txt").exists(),
            "authorization itself must not replay the write"
        );
        let mut supervisor_executor = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "supervisor-retry-health".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        let health = supervisor_executor
            .execute_batch(vec![(
                registry.get("list_worker_health").unwrap(),
                json!({}),
                "pending-grant-health".into(),
            )])
            .await
            .unwrap();
        assert!(!health[0].1.is_error);
        let health: Value = serde_json::from_str(&health[0].1.output).unwrap();
        let worker = health["workers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .unwrap();
        let pending = &worker["run"]["operation_journal"]["entries"][0];
        assert_eq!(pending["cursor"], retry_cursor);
        assert_eq!(pending["retry_worker"], "worker-backend");
        assert_eq!(pending["recovery_required"], true);
        assert!(pending["retry_consumed_by_cursor"].is_null());
        assert!(runs
            .unresolved_task_side_effect_for_claim(task.id, "worker-frontend")
            .unwrap()
            .is_some());
        assert!(runs
            .unresolved_task_side_effect_for_claim(task.id, "worker-backend")
            .unwrap()
            .is_none());
        tasks.update_status(task.id, "ready", None).unwrap();
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .unwrap();
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .unwrap();
        let completion_error = tasks
            .conn
            .execute("UPDATE tasks SET status = 'done' WHERE id = ?1", [task.id])
            .unwrap_err();
        assert!(completion_error
            .to_string()
            .contains("resolved operation outcomes"));
        let replacement_run = runs
            .begin_worker_tick("worker-backend", "replacement-retry-session")
            .unwrap();
        let make_executor = || {
            let mut executor = ToolExecutor::new(
                permission::PermissionMode::Autonomous,
                permission::new_pending_permissions(),
                "replacement-retry-session".into(),
                workspace.path().to_string_lossy().into_owned(),
                AgentIdentity::worker("worker-backend"),
            )
            .with_run_id(Some(replacement_run.clone()));
            executor.set_task_id(Some(task.id));
            executor
        };
        let input = json!({"path":"target/retry.txt", "content":"new content", "overwrite":true, "recovery_intent_cursor":retry_cursor});
        use rusqlite::types::Value as SqlValue;
        for (update, invalid, restored) in [
            (
                "UPDATE runs SET kind = ?1 WHERE id = ?2",
                SqlValue::Text("cli".into()),
                SqlValue::Text(crate::db::runs::kind::FLEET_TICK.into()),
            ),
            (
                "UPDATE runs SET host = ?1 WHERE id = ?2",
                SqlValue::Text("foreign-host".into()),
                SqlValue::Text(crate::db::runs::hostname()),
            ),
            (
                "UPDATE runs SET pid = ?1 WHERE id = ?2",
                SqlValue::Integer(i64::from(std::process::id()) + 1),
                SqlValue::Integer(i64::from(std::process::id())),
            ),
        ] {
            runs.conn
                .execute(update, rusqlite::params![invalid, replacement_run])
                .unwrap();
            let denied = make_executor()
                .execute_batch(vec![(
                    registry.get("write_file").unwrap(),
                    input.clone(),
                    "foreign-run-metadata".into(),
                )])
                .await
                .unwrap();
            assert!(
                denied[0].1.is_error,
                "only a current-process worker tick can consume a grant"
            );
            assert!(!workspace.path().join("target/retry.txt").exists());
            runs.conn
                .execute(update, rusqlite::params![restored, replacement_run])
                .unwrap();
        }
        let mut wrong_worker = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "foreign-retry-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-frontend"),
        )
        .with_run_id(Some(replacement_run.clone()));
        wrong_worker.set_task_id(Some(task.id));
        let foreign = wrong_worker
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                input.clone(),
                "foreign-retry".into(),
            )])
            .await
            .unwrap();
        assert!(
            foreign[0].1.is_error,
            "a foreign executor cannot borrow the bound run"
        );
        for (field, value) in [
            ("content", json!("different content")),
            ("overwrite", json!(false)),
            ("path", json!("target/another.txt")),
        ] {
            let mut changed = input.clone();
            changed[field] = value;
            let result = make_executor()
                .execute_batch(vec![(
                    registry.get("write_file").unwrap(),
                    changed,
                    format!("changed-{field}"),
                )])
                .await
                .unwrap();
            assert!(result[0].1.is_error, "changed retry intent must not run");
            assert!(!workspace.path().join("target/retry.txt").exists());
        }
        std::fs::write(workspace.path().join("target/retry.txt"), "external edit").unwrap();
        let stale = make_executor()
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                input.clone(),
                "stale-retry".into(),
            )])
            .await
            .unwrap();
        assert!(stale[0].1.is_error);
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("target/retry.txt")).unwrap(),
            "external edit"
        );
        std::fs::remove_file(workspace.path().join("target/retry.txt")).unwrap();
        runs.conn
            .execute_batch("DROP TRIGGER lose_write_ack")
            .unwrap();
        let mut first_executor = make_executor();
        let mut second_executor = make_executor();
        let first = first_executor.execute_batch(vec![(
            registry.get("write_file").unwrap(),
            input.clone(),
            "admitted-retry-a".into(),
        )]);
        let second = second_executor.execute_batch(vec![(
            registry.get("write_file").unwrap(),
            input.clone(),
            "admitted-retry-b".into(),
        )]);
        let (first, second) = tokio::join!(first, second);
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(
            first[0].1.is_error, second[0].1.is_error,
            "exactly one racing retry consumes the grant"
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("target/retry.txt")).unwrap(),
            "new content"
        );
        let reopened = RunsDb::open().unwrap();
        let replacement_page = reopened
            .operation_journal_page(&replacement_run, 0, 10)
            .unwrap();
        assert_eq!(
            replacement_page.entries.len(),
            1,
            "denied retries append no mutation intent"
        );
        assert_eq!(
            replacement_page.entries[0].outcome,
            crate::db::runs::OperationOutcome::Ok
        );
        let original_page = reopened.operation_journal_page(&retry_run, 0, 10).unwrap();
        assert_eq!(
            original_page.entries[0].retry_worker.as_deref(),
            Some("worker-backend")
        );
        assert_eq!(
            original_page.entries[0].retry_consumed_by_cursor,
            Some(replacement_page.entries[0].cursor)
        );
        assert!(replacement_page.entries[0].retry_grant_cursor.is_some());
        let health = supervisor_executor
            .execute_batch(vec![(
                registry.get("list_worker_health").unwrap(),
                json!({}),
                "consumed-grant-health".into(),
            )])
            .await
            .unwrap();
        assert!(!health[0].1.is_error);
        let health: Value = serde_json::from_str(&health[0].1.output).unwrap();
        let worker = health["workers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .unwrap();
        let consumed = &worker["run"]["operation_journal"]["entries"][0];
        assert_eq!(consumed["cursor"], replacement_page.entries[0].cursor);
        assert_eq!(
            consumed["retry_grant_cursor"],
            replacement_page.entries[0].retry_grant_cursor.unwrap()
        );
        assert_eq!(consumed["recovery_required"], false);
        assert!(RunsDb::task_recovery_allows_completion(
            &crate::db::arcana_dir().unwrap().join("runs.db"),
            task.id
        )
        .unwrap());
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("acknowledged.txt")).unwrap(),
            "acknowledged fixture"
        );
        assert!(reopened
            .unresolved_task_side_effect(task.id)
            .unwrap()
            .is_none());
        let duplicate = make_executor()
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                input,
                "duplicate-retry".into(),
            )])
            .await
            .unwrap();
        assert!(
            duplicate[0].1.is_error,
            "reopened consumed grant cannot authorize a second execution"
        );
        assert_eq!(
            reopened
                .operation_journal_page(&replacement_run, 0, 10)
                .unwrap()
                .entries
                .len(),
            1
        );
        // Consuming the first grant is not blanket permission to complete:
        // another lost acknowledgement in the replacement run creates a new hold.
        runs.conn.execute_batch("CREATE TRIGGER lose_retry_ack BEFORE INSERT ON operation_journal WHEN NEW.record_type = 'outcome' BEGIN SELECT RAISE(ABORT, 'lost retry ack'); END;").unwrap();
        let unknown = make_executor()
            .execute_batch(vec![(
                registry.get("write_file").unwrap(),
                json!({"path":"post-retry.txt","content":"uncertain replacement fixture"}),
                "replacement-lost-ack".into(),
            )])
            .await
            .unwrap();
        assert!(unknown[0].1.is_error);
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("post-retry.txt")).unwrap(),
            "uncertain replacement fixture"
        );
        let replacement_unknown = reopened
            .operation_journal_page(&replacement_run, 0, 10)
            .unwrap();
        assert_eq!(replacement_unknown.entries.len(), 2);
        assert_eq!(
            replacement_unknown.entries[1].outcome,
            crate::db::runs::OperationOutcome::Unknown
        );
        assert!(!RunsDb::task_recovery_allows_completion(
            &crate::db::arcana_dir().unwrap().join("runs.db"),
            task.id
        )
        .unwrap());
        let error = tasks
            .conn
            .execute("UPDATE tasks SET status = 'done' WHERE id = ?1", [task.id])
            .unwrap_err();
        assert!(error.to_string().contains("resolved operation outcomes"));
        tasks
            .update_status(task.id, "needs_info", Some("worker-backend"))
            .unwrap();
        runs.finish_worker_tick(&replacement_run, "interrupted", "interrupted", None)
            .unwrap();
    }
}
