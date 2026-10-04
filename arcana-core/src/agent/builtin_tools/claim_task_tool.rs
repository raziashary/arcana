// claim_task — Atomic "begin working on a task" tool for fleet workers.
//
// Replaces calling update_task_status with status=in_progress. Records the
// worker's process PID so OrphanDetector can release the task if the worker
// process dies unexpectedly (Ruflo Feature 2 — PID-based orphan detection).

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ClaimTaskTool;

#[async_trait]
impl Tool for ClaimTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "claim_task"
    }

    fn description(&self) -> &str {
        "Claim an assigned task and begin working on it. Sets status='in_progress' \
        and records this process's PID for orphan detection. Use this instead of \
        update_task_status(status=in_progress) — it enables automatic recovery if \
        the worker process exits unexpectedly."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "Numeric task ID to claim (must currently be status='assigned')"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_id = match input["task_id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'task_id' must be a positive integer"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        Self::claim_from_db(task_id, ctx, &db)
    }
}

impl ClaimTaskTool {
    fn claim_from_db(task_id: i64, ctx: &ToolContext, db: &TasksDb) -> ToolResult {
        // Verify task exists and is in 'assigned' state
        let task = match db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None) => return ToolResult::error(format!("Task #{task_id} not found")),
            Err(e) => return ToolResult::error(format!("DB error fetching task #{task_id}: {e}")),
        };

        if task.status.as_str() != "assigned" {
            return ToolResult::error(format!(
                "Task #{task_id} is in status='{}', not 'assigned'. \
                 Only assigned tasks can be claimed.",
                task.status.as_str()
            ));
        }

        // A row's current assignment is not proof that the caller owns it.
        // Worker identity is supplied by the runtime, and the atomic DB write
        // checks that this exact identity remains the owner at claim time.
        let worker_slug = ctx.agent_role.trim();
        if worker_slug.is_empty() {
            return ToolResult::error("The runtime did not provide a worker identity");
        }
        if task.assigned_worker.as_deref() != Some(worker_slug) {
            return ToolResult::error(format!(
                "Task #{task_id} is assigned to {:?}, not the calling worker '{worker_slug}'",
                task.assigned_worker
            ));
        }

        // A durable intent with an unknown outcome means the previous worker
        // may already have changed files, run a command, or contacted a
        // connector. Do not redispatch until a Supervisor has recorded an
        // explicit reconciliation decision. Board mutations are excluded by
        // RunsDb because their task state is authoritative in TasksDb.
        match crate::db::runs::RunsDb::open()
            .and_then(|runs| runs.unresolved_task_side_effect(task_id))
        {
            Ok(Some((run_id, operation, cursor))) => {
                return ToolResult::categorized_error(
                    "operation_reconciliation_required",
                    "reconciliation_required",
                    format!(
                        "Task #{task_id} has an unresolved '{operation}' side effect from run {run_id} (journal cursor {cursor})."
                    ),
                    "Do not claim or repeat this task yet. The Supervisor must inspect the operation journal, task/workspace/checkpoint state, and Git state, then persist a reconciliation decision.",
                );
            }
            Ok(None) => {}
            Err(_) => {
                return ToolResult::categorized_error(
                    "operation_recovery_unavailable",
                    "durability",
                    format!(
                        "Arcana could not verify the recovery journal for task #{task_id}; the task was not claimed."
                    ),
                    "Check the local run database and retry only after the journal can be read.",
                );
            }
        }

        let pid = std::process::id();

        if let Err(e) = db.claim_task_with_pid(task_id, worker_slug, pid) {
            return ToolResult::error(format!("Failed to claim task #{task_id}: {e}"));
        }

        // The engine is constructed before the worker knows which task it will
        // claim. Return the persisted cap and usage through the live tool result
        // so the engine can activate that cap before its next provider turn.
        let budget = match db.get_by_id(task_id) {
            Ok(Some(claimed)) => json!({
                "max_tokens": claimed.budget_max_tokens,
                "max_usd": claimed.budget_max_usd,
                "max_turns": claimed.budget_max_turns,
                "tokens_used": claimed.budget_tokens_used,
                "cost_usd": claimed.budget_cost_usd,
                "turns_used": claimed.budget_turns_used,
            }),
            Ok(None) => {
                return ToolResult::error(format!("Task #{task_id} disappeared after claim"))
                    .with_unknown_side_effect_outcome()
            }
            Err(e) => {
                return ToolResult::error(format!(
                    "Task #{task_id} claimed, but budget could not be loaded: {e}"
                ))
                .with_unknown_side_effect_outcome()
            }
        };

        // Emit task-changed so the UI kanban moves the card
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(task_id),
            kind: "status".into(),
            status: Some("in_progress".into()),
            assigned_worker: Some(worker_slug.to_string()),
        });

        ToolResult::success(
            json!({
                "task_id":     task_id,
                "title":       task.title,
                "status":      "in_progress",
                "worker_slug": worker_slug,
                "pid":         pid,
                "budget":      budget,
                "message": format!(
                    "✅ Task #{task_id} claimed by {worker_slug} (PID={pid}, status=in_progress). \
                     OrphanDetector will monitor this PID."
                )
            })
            .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(role: &str) -> ToolContext {
        ToolContext {
            session_id: "p4-claim-test".into(),
            working_dir: std::env::temp_dir().to_string_lossy().to_string(),
            call_id: "p4-claim-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: role.into(),

            run_token_budget: None,
        }
    }

    #[test]
    fn shipped_claim_tool_refuses_other_worker_and_claims_for_owner() {
        let path =
            std::env::temp_dir().join(format!("arcana-claim-tool-{}.db", uuid::Uuid::new_v4()));
        let db = TasksDb::open_at(&path).expect("open isolated tasks DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "owner checked claim".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "claimed".into(),
                created_by: Some("test".into()),
            })
            .expect("create");
        db.update_status(task.id, "assigned", Some("worker-a"))
            .expect("assign to A");
        db.set_task_budget(task.id, Some(250), Some(0.2), Some(4))
            .expect("persist task cap");
        db.update_task_budget_usage(task.id, 17, 0.03, 1)
            .expect("persist existing task usage");

        let denied = ClaimTaskTool::claim_from_db(task.id, &context("worker-b"), &db);
        assert!(denied.is_error, "worker B must not claim worker A's task");
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::Assigned
        );

        let allowed = ClaimTaskTool::claim_from_db(task.id, &context("worker-a"), &db);
        assert!(!allowed.is_error, "the assigned worker should claim it");
        let payload: Value = serde_json::from_str(&allowed.output).expect("claim result JSON");
        assert_eq!(payload["budget"]["max_tokens"], 250);
        assert_eq!(payload["budget"]["tokens_used"], 17);
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::InProgress
        );

        drop(db);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn claim_refuses_unresolved_side_effect_until_supervisor_reconciliation() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "reconcile before redispatch".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "unknown writes block a later claim".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");

        let runs = crate::db::runs::RunsDb::open().expect("open isolated runs DB");
        let run_id = runs
            .begin_worker_tick("worker-backend", "interrupted-session")
            .expect("begin previous worker run");
        let cursor = runs
            .record_operation_intent(
                &run_id,
                Some(task.id),
                "private-operation-id",
                "write_file",
                "Write",
                &["src/calendar.rs".into()],
            )
            .expect("persist uncertain write intent");

        let blocked = ClaimTaskTool::claim_from_db(task.id, &context("worker-backend"), &db);
        assert!(blocked.is_error, "an unknown write must block redispatch");
        let blocked_json: Value = serde_json::from_str(&blocked.output).expect("error envelope");
        assert_eq!(
            blocked_json["error"]["code"],
            "operation_reconciliation_required"
        );
        assert!(blocked_json["error"]["message"]
            .as_str()
            .unwrap()
            .contains("write_file"));
        assert!(!blocked.output.contains("calendar.rs"));
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::Assigned,
            "a refused recovery claim must leave its task assignment unchanged"
        );

        runs.record_operation_reconciliation(
            &run_id,
            cursor,
            crate::db::runs::OperationReconciliation::RetrySafe,
        )
        .expect("persist explicit safe-to-continue decision");
        let allowed = ClaimTaskTool::claim_from_db(task.id, &context("worker-backend"), &db);
        assert!(
            !allowed.is_error,
            "an explicitly reconciled operation should permit a new claim: {}",
            allowed.output
        );
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::InProgress
        );
    }
}
