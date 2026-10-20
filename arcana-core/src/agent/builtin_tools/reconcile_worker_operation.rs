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
        "Supervisor-only: after inspecting the exact task, workspace/checkpoint and Git state, persist one decision for an unknown worker side effect. Use reconciled when the effect is verified complete, and do_not_retry when uncertainty remains unsafe. Arcana verifies sealed host snapshots and local file state before granting completion. For a verifiable unchanged write, retry_safe creates a task/worker-bound one-use grant. Tell the owning worker to call write_file with the original recovery_intent_cursor; Arcana revalidates and atomically consumes that grant under the write lease. A grant alone never executes or clears uncertainty. Legacy/unprovable effects remain held."
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

    // ── TG4-27 / P4-53 conflict-refusal boundary ────────────────────────────
    // A manual continuation must be refused when the workspace Git state
    // changed between interruption and handoff. `RecoverySnapshot::verify_state`
    // (recovery_snapshot.rs) compares the captured git head / branch digest /
    // index digest (`GitSnapshot`), and the reconcile tool maps every snapshot
    // refusal to the `recovery_conflict` envelope
    // (`operation_reconciliation_rejected` / `recovery_conflict`) at
    // reconcile_worker_operation.rs::execute.
    struct ConflictFixture {
        workspace: tempfile::TempDir,
        repo: git2::Repository,
        seed_head: git2::Oid,
        task_id: i64,
        run: String,
        cursor: i64,
        content: &'static str,
    }

    /// Commit one root file with the libgit2 path used by the shipped
    /// `git2::Repository::init` fixtures in this module.
    fn commit_file(
        repo: &git2::Repository,
        name: &str,
        message: &str,
        parent: Option<git2::Oid>,
    ) -> git2::Oid {
        let signature = git2::Signature::now("arcana tests", "arcana@test.local").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(name)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        match parent {
            Some(parent) => {
                let parent = repo.find_commit(parent).unwrap();
                repo.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    message,
                    &tree,
                    &[&parent],
                )
                .unwrap()
            }
            None => repo
                .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])
                .unwrap(),
        }
    }

    fn conflict_fixture(session: &str) -> ConflictFixture {
        use crate::db::tasks::{CreateTaskInput, TasksDb};
        let workspace = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(workspace.path()).unwrap();
        std::fs::write(workspace.path().join("seed.txt"), b"seed content").unwrap();
        let seed_head = commit_file(&repo, "seed.txt", "seed commit", None);
        let tasks = TasksDb::open().unwrap();
        let task = tasks
            .create(CreateTaskInput {
                title: "conflict refusal".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "refuse a changed workspace".into(),
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
        let run = runs.begin_worker_tick("worker-backend", session).unwrap();
        // The same pre-state path the executor's begin_function uses: the
        // snapshot captures the live Git state (head, branch digest, index
        // digest) before the intent is journalled.
        let content = "recovered canary content";
        let snapshot = crate::agent::recovery_snapshot::RecoverySnapshot::capture(
            task.id,
            session,
            workspace.path(),
            Some((
                "target/generated.txt",
                &crate::agent::recovery_snapshot::RecoverySnapshot::file_expectation(content),
                false,
            )),
        )
        .unwrap();
        let cursor = runs
            .record_operation_intent_with_snapshot(
                &run,
                Some(task.id),
                "uncertain-write",
                "write_file",
                "Write",
                &["target/generated.txt".to_owned()],
                Some(&snapshot),
                None,
            )
            .unwrap();
        // Interruption handoff: the tick ends `interrupted` and the task is
        // held in `needs_info` with no live claim — exactly the lifecycle
        // `verify_state` accepts for a manual continuation.
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
        ConflictFixture {
            workspace,
            repo,
            seed_head,
            task_id: task.id,
            run,
            cursor,
            content,
        }
    }

    fn supervisor_working(dir: &std::path::Path) -> ToolContext {
        let mut ctx = context("supervisor");
        ctx.working_dir = dir.to_string_lossy().into_owned();
        ctx
    }

    fn error_envelope(result: &ToolResult) -> Value {
        serde_json::from_str(&result.output)
            .unwrap_or_else(|error| panic!("tool result is not a structured envelope: {error}"))
    }

    #[tokio::test]
    async fn git_state_change_between_intent_and_handoff_refuses_reconciliation() {
        use crate::agent::builtin_tools;
        use crate::agent::permission;
        use crate::agent::tool_executor::ToolExecutor;
        use crate::agent::tool_registry::ToolRegistry;
        use crate::agent::tool_trait::AgentIdentity;
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        let f = conflict_fixture("conflict-session");
        let reconciled_request = json!({
            "run_id": f.run,
            "intent_cursor": f.cursor,
            "decision": "reconciled",
            "reason_code": "effect_verified_complete"
        });
        let retry_request = json!({
            "run_id": f.run,
            "intent_cursor": f.cursor,
            "decision": "retry_safe",
            "reason_code": "idempotent_retry_verified"
        });

        // Tamper: advance HEAD past the snapshot's Git state.
        std::fs::write(f.workspace.path().join("tamper.txt"), b"head tamper").unwrap();
        let tamper_head = commit_file(&f.repo, "tamper.txt", "tamper commit", Some(f.seed_head));
        assert_ne!(tamper_head, f.seed_head);

        for request in [&reconciled_request, &retry_request] {
            let refused = ReconcileWorkerOperationTool
                .execute(request.clone(), &supervisor_working(f.workspace.path()))
                .await;
            assert!(
                refused.is_error,
                "changed Git state must refuse: {refused:?}"
            );
            let envelope = error_envelope(&refused);
            assert_eq!(
                envelope["error"]["code"],
                json!("operation_reconciliation_rejected")
            );
            assert_eq!(envelope["error"]["category"], json!("recovery_conflict"));
        }

        // The unknown intent stays unresolved; the journal record is untouched.
        let runs = RunsDb::open().unwrap();
        let page = runs.operation_journal_page(&f.run, 0, 10).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert!(page.entries[0].before_snapshot.is_some());
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::Unresolved
        );
        assert_eq!(page.entries[0].reconciliation_actor.as_deref(), None);
        assert_eq!(page.entries[0].reconciliation_reason.as_deref(), None);
        assert_eq!(
            runs.unresolved_task_side_effect(f.task_id)
                .unwrap()
                .unwrap()
                .2,
            f.cursor
        );

        // The one-use retry barrier still blocks a same-task write.
        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, crate::agent::PAMode::FastDevelopment);
        let write_tool = registry.get("write_file").unwrap();
        let replacement_run = runs
            .begin_worker_tick("worker-backend", "barrier-session")
            .unwrap();
        let mut plain_executor = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "barrier-session".into(),
            f.workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(replacement_run.clone()));
        plain_executor.set_task_id(Some(f.task_id));
        let plain = plain_executor
            .execute_batch(vec![(
                write_tool.clone(),
                json!({"path":"barrier.txt", "content":"blocked fixture"}),
                "barrier-write".into(),
            )])
            .await
            .unwrap();
        assert!(
            plain[0].1.is_error,
            "unresolved intent must block: {:#?}",
            plain[0].1.output
        );
        let envelope = error_envelope(&plain[0].1);
        assert_eq!(
            envelope["error"]["code"],
            json!("operation_reconciliation_required")
        );
        assert_eq!(
            envelope["error"]["category"],
            json!("reconciliation_required")
        );
        assert!(!f.workspace.path().join("barrier.txt").exists());
        assert_eq!(
            runs.operation_journal_page(&replacement_run, 0, 10)
                .unwrap()
                .entries
                .len(),
            0,
            "refused writes append no mutation intent"
        );

        // The same barrier holds when the cursor itself is produced: the
        // refused reconcile never issued a retry grant, so the exact cursor is
        // unusable. The write still must not run. No reconcile decision
        // succeeded, so the actual refusal is the grant-absent
        // `operation_intent_unavailable` envelope, not the journal barrier
        // (`operation_reconciliation_required`).
        let mut cursor_executor = ToolExecutor::new(
            permission::PermissionMode::Autonomous,
            permission::new_pending_permissions(),
            "cursor-barrier-session".into(),
            f.workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(replacement_run.clone()));
        cursor_executor.set_task_id(Some(f.task_id));
        let cursor_refused = cursor_executor
            .execute_batch(vec![(
                write_tool.clone(),
                json!({
                    "path": "target/generated.txt",
                    "content": f.content,
                    "recovery_intent_cursor": f.cursor
                }),
                "cursor-barrier-write".into(),
            )])
            .await
            .unwrap();
        assert!(
            cursor_refused[0].1.is_error,
            "an unresolved intent with no issued grant must not run: {:#?}",
            cursor_refused[0].1.output
        );
        let envelope = error_envelope(&cursor_refused[0].1);
        assert_eq!(
            envelope["error"]["code"],
            json!("operation_intent_unavailable")
        );
        assert_eq!(envelope["error"]["category"], json!("durability"));
        assert!(!f.workspace.path().join("target/generated.txt").exists());
        assert_eq!(
            runs.operation_journal_page(&replacement_run, 0, 10)
                .unwrap()
                .entries
                .len(),
            0
        );

        // Benign control inside the same fixture: returning the workspace to
        // the snapshot's exact Git state clears the conflict. The refusal was
        // specifically the Git change, not the fixture shape.
        let snapshot_head = f.repo.revparse_single(&f.seed_head.to_string()).unwrap();
        f.repo
            .reset(&snapshot_head, git2::ResetType::Hard, None)
            .unwrap();
        let granted = ReconcileWorkerOperationTool
            .execute(retry_request, &supervisor_working(f.workspace.path()))
            .await;
        assert!(
            !granted.is_error,
            "restored Git state accepts one explicit host-verified retry: {}",
            granted.output
        );
        let envelope = error_envelope(&granted);
        assert_eq!(envelope["retry_permitted"], json!(true));
        assert_eq!(envelope["operation_complete"], json!(false));
        let granted_page = RunsDb::open()
            .unwrap()
            .operation_journal_page(&f.run, 0, 10)
            .unwrap();
        assert_eq!(
            granted_page.entries[0].reconciliation,
            OperationReconciliation::RetrySafe
        );
        assert_eq!(
            granted_page.entries[0].reconciliation_actor.as_deref(),
            Some("supervisor")
        );
        assert_eq!(
            granted_page.entries[0].reconciliation_reason.as_deref(),
            Some("idempotent_retry_verified")
        );
        assert!(
            RunsDb::open()
                .unwrap()
                .unresolved_task_side_effect(f.task_id)
                .unwrap()
                .is_some(),
            "a grant authorises one retry; it does not resolve the uncertainty"
        );
        assert!(
            !f.workspace.path().join("target/generated.txt").exists(),
            "authorization itself must not replay the write"
        );
    }

    #[tokio::test]
    async fn untampered_workspace_accepts_supervisor_verified_reconciled_decision() {
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        let f = conflict_fixture("benign-session");
        // Benign control: without any Git tampering, the same reconcile flow
        // behaves as shipped — the Supervisor may resolve the unknown effect
        // once the verified file actually matches the journalled expectation.
        std::fs::create_dir_all(f.workspace.path().join("target")).unwrap();
        std::fs::write(f.workspace.path().join("target/generated.txt"), f.content).unwrap();
        let reconciled = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": f.run,
                    "intent_cursor": f.cursor,
                    "decision": "reconciled",
                    "reason_code": "effect_verified_complete"
                }),
                &supervisor_working(f.workspace.path()),
            )
            .await;
        assert!(
            !reconciled.is_error,
            "untampered state must reconcile: {}",
            reconciled.output
        );
        let envelope = error_envelope(&reconciled);
        assert_eq!(envelope["operation_complete"], json!(true));
        assert_eq!(envelope["retry_permitted"], json!(false));
        let page = RunsDb::open()
            .unwrap()
            .operation_journal_page(&f.run, 0, 10)
            .unwrap();
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::Reconciled
        );
        assert_eq!(
            page.entries[0].reconciliation_actor.as_deref(),
            Some("supervisor")
        );
        assert_eq!(
            page.entries[0].reconciliation_reason.as_deref(),
            Some("effect_verified_complete")
        );
        assert!(RunsDb::open()
            .unwrap()
            .unresolved_task_side_effect(f.task_id)
            .unwrap()
            .is_none());
        let duplicate = ReconcileWorkerOperationTool
            .execute(
                json!({
                    "run_id": f.run,
                    "intent_cursor": f.cursor,
                    "decision": "reconciled",
                    "reason_code": "effect_verified_complete"
                }),
                &supervisor_working(f.workspace.path()),
            )
            .await;
        assert!(
            duplicate.is_error,
            "one durable decision only: {:#?}",
            duplicate.output
        );
        let duplicate_envelope = error_envelope(&duplicate);
        assert_eq!(
            duplicate_envelope["error"]["category"],
            json!("recovery_conflict")
        );
    }

    /// TG4-27 names the repository state explicitly: HEAD, branch/worktree,
    /// staged and unstaged content, and *relevant* untracked files.
    /// `RecoverySnapshot` covers those through two arms — `GitSnapshot`
    /// (head + branch digest + index digest) and `workspace_fingerprint`
    /// (every workspace input, generated directories excluded). Each arm needs
    /// its own negative case, plus the scoping control that proves the exclusion
    /// is deliberate rather than accidental blindness.
    #[tokio::test]
    async fn repository_worktree_and_index_changes_refuse_before_handoff() {
        async fn decide(f: &ConflictFixture, decision: &str, reason: &str) -> ToolResult {
            ReconcileWorkerOperationTool
                .execute(
                    json!({
                        "run_id": f.run,
                        "intent_cursor": f.cursor,
                        "decision": decision,
                        "reason_code": reason
                    }),
                    &supervisor_working(f.workspace.path()),
                )
                .await
        }

        /// Complete the journalled effect on disk, so a clean workspace would
        /// reconcile. Any refusal after that is caused by the tamper alone.
        fn complete_effect(f: &ConflictFixture) {
            std::fs::create_dir_all(f.workspace.path().join("target")).unwrap();
            std::fs::write(f.workspace.path().join("target/generated.txt"), f.content).unwrap();
        }

        async fn assert_reconciled_refused(f: &ConflictFixture, case: &str) {
            let refused = decide(f, "reconciled", "effect_verified_complete").await;
            assert!(
                refused.is_error,
                "{case}: a changed repository must refuse a verified completion: {}",
                refused.output
            );
            let envelope = error_envelope(&refused);
            assert_eq!(
                envelope["error"]["code"],
                json!("operation_reconciliation_rejected"),
                "{case}"
            );
            assert_eq!(
                envelope["error"]["category"],
                json!("recovery_conflict"),
                "{case}"
            );
            // The unknown effect must remain unresolved after every refusal.
            assert_eq!(
                RunsDb::open()
                    .unwrap()
                    .unresolved_task_side_effect(f.task_id)
                    .unwrap()
                    .expect("effect still unresolved")
                    .2,
                f.cursor,
                "{case}: a refusal must not resolve the uncertainty"
            );
        }

        // (1) Unstaged worktree edit of a tracked file. The index still matches
        // HEAD and the effect is complete, so only the workspace-fingerprint arm
        // can refuse this.
        {
            let home = tempfile::tempdir().unwrap();
            let _home = crate::db::test_support::with_home(home.path());
            let f = conflict_fixture("unstaged-session");
            complete_effect(&f);
            std::fs::write(f.workspace.path().join("seed.txt"), b"seed content edited").unwrap();
            assert_eq!(
                f.repo
                    .index()
                    .unwrap()
                    .get_path(std::path::Path::new("seed.txt"), 0)
                    .unwrap()
                    .id,
                f.repo
                    .find_commit(f.seed_head)
                    .unwrap()
                    .tree()
                    .unwrap()
                    .get_name("seed.txt")
                    .unwrap()
                    .id(),
                "the index must still match HEAD, otherwise this case proves the index arm"
            );
            assert_reconciled_refused(&f, "unstaged worktree edit").await;
        }

        // (1b) The same tamper must also refuse the retry-admission path, whose
        // fingerprint comparison is separate from the completion comparison.
        {
            let home = tempfile::tempdir().unwrap();
            let _home = crate::db::test_support::with_home(home.path());
            let f = conflict_fixture("unstaged-retry-session");
            std::fs::write(f.workspace.path().join("seed.txt"), b"seed content edited").unwrap();
            let refused = decide(&f, "retry_safe", "idempotent_retry_verified").await;
            assert!(
                refused.is_error,
                "a changed workspace must refuse retry admission: {}",
                refused.output
            );
            assert_eq!(
                error_envelope(&refused)["error"]["category"],
                json!("recovery_conflict")
            );
        }

        // (2) Index-only change: worktree content and HEAD are untouched, so
        // only the index digest can refuse this. Restoring the recorded mode
        // accepts, which makes the refusal differential rather than blanket.
        {
            let home = tempfile::tempdir().unwrap();
            let _home = crate::db::test_support::with_home(home.path());
            let f = conflict_fixture("staged-session");
            complete_effect(&f);
            let mut entry = {
                let mut index = f.repo.index().unwrap();
                let mut entry = index.get_path(std::path::Path::new("seed.txt"), 0).unwrap();
                entry.mode = 0o100755;
                index.add(&entry).unwrap();
                index.write().unwrap();
                entry
            };
            assert_eq!(
                std::fs::read(f.workspace.path().join("seed.txt")).unwrap(),
                b"seed content",
                "the worktree content must be untouched, otherwise this case proves the fingerprint arm"
            );
            assert_reconciled_refused(&f, "staged index change").await;

            entry.mode = 0o100644;
            let mut index = f.repo.index().unwrap();
            index.add(&entry).unwrap();
            index.write().unwrap();
            let accepted = decide(&f, "reconciled", "effect_verified_complete").await;
            assert!(
                !accepted.is_error,
                "restoring the recorded index must accept: {}",
                accepted.output
            );
        }

        // (3) Relevant untracked file: not generated output, so it is a
        // workspace input and must block the handoff.
        {
            let home = tempfile::tempdir().unwrap();
            let _home = crate::db::test_support::with_home(home.path());
            let f = conflict_fixture("untracked-session");
            complete_effect(&f);
            std::fs::write(f.workspace.path().join("notes.txt"), b"untracked input").unwrap();
            assert!(
                f.repo
                    .statuses(None)
                    .unwrap()
                    .iter()
                    .any(|status| status.path() == Some("notes.txt")),
                "the tamper must be untracked, otherwise this case proves the index arm"
            );
            assert_reconciled_refused(&f, "relevant untracked file").await;
        }

        // (4) Scoping control: untracked content inside a generated dependency
        // tree is deliberately excluded, and the completed effect still
        // reconciles. Without this case the refusals above would be
        // indistinguishable from "any new file blocks everything".
        {
            let home = tempfile::tempdir().unwrap();
            let _home = crate::db::test_support::with_home(home.path());
            let f = conflict_fixture("generated-session");
            complete_effect(&f);
            std::fs::create_dir_all(f.workspace.path().join("node_modules/pkg")).unwrap();
            std::fs::write(
                f.workspace.path().join("node_modules/pkg/index.js"),
                b"generated dependency tree",
            )
            .unwrap();
            let accepted = decide(&f, "reconciled", "effect_verified_complete").await;
            assert!(
                !accepted.is_error,
                "generated untracked content must not block a verified completion: {}",
                accepted.output
            );
            assert_eq!(error_envelope(&accepted)["operation_complete"], json!(true));
        }
    }
}
