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
        "Supervisor-only: after inspecting the exact task, workspace/checkpoint and Git state, persist one decision for an unknown worker side effect. Use reconciled when the effect is verified complete, retry_safe only when repeating it is proven safe, and do_not_retry when uncertainty remains unsafe. This records the decision; it does not independently inspect or verify the workspace."
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
        ) {
            Ok(decision_cursor) => ToolResult::success(
                json!({
                    "run_id": run_id,
                    "intent_cursor": intent_cursor,
                    "decision_cursor": decision_cursor,
                    "decision": decision.as_str(),
                    "reason_code": reason_code,
                    "retry_permitted": matches!(
                        decision,
                        OperationReconciliation::Reconciled | OperationReconciliation::RetrySafe
                    )
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
        assert!(
            !denied.is_error,
            "Supervisor action should succeed: {}",
            denied.output
        );
        assert_eq!(runs.unresolved_task_side_effect(77).unwrap(), None);
        let page = runs.operation_journal_page(&run_id, 0, 10).unwrap();
        assert_eq!(
            page.entries[0].reconciliation,
            OperationReconciliation::RetrySafe
        );
        assert_eq!(
            page.entries[0].reconciliation_actor.as_deref(),
            Some("supervisor")
        );
        assert_eq!(
            page.entries[0].reconciliation_reason.as_deref(),
            Some("idempotent_retry_verified")
        );
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
                    "decision": "retry_safe",
                    "reason_code": "idempotent_retry_verified"
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
}
