// set_task_budget — Supervisor MCP tool for assigning token/cost budgets to tasks.
//
// Called by the Supervisor recipe before or during task assignment so that the
// AgentEngine enforces per-task spending limits via the Budget Circuit Breaker.
// Exposes the three controllable axes: max_tokens, max_usd, max_turns.
//
// Registered for the Supervisor in `register_for_mode` (P8-50 deleted the MCP
// MCP-connected agent that needs budget management). NOT available in Delegate mode.
//
// Ruflo Feature 4 — Budget Circuit Breaker (4.5 Supervisor Budget Assignment).

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::budget;
use crate::db::tasks::TasksDb;

pub struct SetTaskBudgetTool;

#[async_trait]
impl Tool for SetTaskBudgetTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "set_task_budget"
    }

    fn description(&self) -> &str {
        "Set token and cost budget for a task before assigning it to a worker. \
         The worker's agent loop will halt and escalate to Supervisor when any limit is exceeded \
         rather than silently burning tokens. Omitted task-specific limits leave the worker's \
         configured run limits in effect. To resolve an exceeded-budget hold, explicitly set \
         resolve_exceeded=true, provide a reason, and provide larger numeric limits for all axes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "ID of the task to budget"
                },
                "max_tokens": {
                    "type": "integer",
                    "description": "Task-specific maximum total tokens (input + output). If omitted for an ordinary budget edit, the worker run limit applies."
                },
                "max_usd": {
                    "type": "number",
                    "description": "Task-specific maximum USD spend. If omitted for an ordinary budget edit, the worker run limit applies."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Task-specific maximum agent loop turns. If omitted for an ordinary budget edit, the worker run limit applies."
                },
                "resolve_exceeded": {
                    "type": "boolean",
                    "description": "Set true only for an explicit Supervisor decision to resolve an existing budget-exceeded hold. Requires a reason and numeric limits above all recorded usage."
                },
                "resolution_reason": {
                    "type": "string",
                    "description": "Required explanation for an explicit budget-exceeded resolution."
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_id = match input.get("task_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => return ToolResult::error("Missing required field: task_id (integer)"),
        };

        let resolve_exceeded = match input.get("resolve_exceeded") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return ToolResult::error("resolve_exceeded must be a boolean"),
        };
        let resolution_reason = match input.get("resolution_reason") {
            None => None,
            Some(Value::String(reason)) => Some(reason.trim()),
            Some(_) => return ToolResult::error("resolution_reason must be a string"),
        };
        if !resolve_exceeded && resolution_reason.is_some() {
            return ToolResult::error("resolution_reason requires explicit resolve_exceeded=true");
        }
        if resolve_exceeded && ctx.agent_role.trim() != "supervisor" {
            return ToolResult::error("Only the Supervisor can resolve an exceeded task budget");
        }

        let max_tokens = input.get("max_tokens").and_then(|v| v.as_u64());
        let max_usd = input.get("max_usd").and_then(|v| v.as_f64());
        let max_turns = input
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .and_then(|turns| u32::try_from(turns).ok());
        if (input.get("max_tokens").is_some() && max_tokens.is_none())
            || (input.get("max_usd").is_some() && max_usd.is_none())
            || (input.get("max_turns").is_some() && max_turns.is_none())
        {
            return ToolResult::error("Budget limits must be numeric; tokens and turns must be unsigned integers within range");
        }

        // Validate using ADR-097 rules
        if let Err(code) = budget::validate_budget(max_tokens, max_usd, max_turns) {
            return ToolResult::error(&format!("Budget validation failed: {code}"));
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("DB error: {e}")),
        };

        if resolve_exceeded {
            let Some(reason) = resolution_reason.filter(|reason| !reason.is_empty()) else {
                return ToolResult::error(
                    "Explicit budget resolution requires a reason between 1 and 500 bytes",
                );
            };
            if reason.len() > 500 {
                return ToolResult::error(
                    "Explicit budget resolution requires a reason between 1 and 500 bytes",
                );
            }
            let (Some(max_tokens), Some(max_usd), Some(max_turns)) =
                (max_tokens, max_usd, max_turns)
            else {
                return ToolResult::error(
                    "Explicit budget resolution requires numeric max_tokens, max_usd, and max_turns",
                );
            };
            let resolution = match db.resolve_exceeded_task_budget(
                task_id,
                max_tokens,
                max_usd,
                max_turns,
                &ctx.agent_role,
                reason,
            ) {
                Ok(resolution) => resolution,
                Err(error) => {
                    return ToolResult::error(&format!(
                        "Failed to resolve exceeded budget for task {task_id}: {error}"
                    ));
                }
            };
            let audit = match db.get_task_budget_resolution_history(task_id, 1) {
                Ok(rows) => rows
                    .into_iter()
                    .next()
                    .filter(|row| row.id == resolution.id),
                Err(error) => {
                    return ToolResult::error(&format!(
                        "Budget was resolved but its audit readback failed: {error}"
                    ));
                }
            };
            let Some(audit) = audit else {
                return ToolResult::error(
                    "Budget was resolved but the audit reader did not return the new record",
                );
            };
            return ToolResult::success(
                json!({
                    "task_id": task_id,
                    "budget_resolution": audit,
                    "task_status": "needs_info",
                    "message": format!(
                        "Budget hold for task #{task_id} was explicitly resolved and recorded. The task remains needs_info until its other holds are reconciled through normal task lifecycle operations."
                    )
                })
                .to_string(),
            );
        }

        if let Err(e) = db.set_task_budget(task_id, max_tokens, max_usd, max_turns) {
            return ToolResult::error(&format!("Failed to set budget for task {task_id}: {e}"));
        }

        info!("[budget] Set task-specific budget overrides for task {task_id}");

        ToolResult::success(
            json!({
                "task_id": task_id,
                "budget_set": {
                    "max_tokens": max_tokens,
                    "max_usd": max_usd,
                    "max_turns": max_turns,
                },
                "message": format!(
                    "Task-specific budget overrides saved for task #{task_id}. Unset axes inherit the worker's configured run limits."
                )
            })
            .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::db::tasks::{CreateTaskInput, TasksDb};

    fn context(home: &std::path::Path, agent_role: &str) -> ToolContext {
        ToolContext {
            session_id: "task-budget-resolution-tool-test".into(),
            working_dir: home.to_string_lossy().into_owned(),
            call_id: "budget-resolution-tool-call".into(),
            run_id: None,
            run_token_budget: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: agent_role.into(),
        }
    }

    #[tokio::test]
    async fn task_budget_resolution_requires_supervisor_and_reads_back_audit() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        let task = db
            .create(CreateTaskInput {
                title: "budget-held task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "keep usage and remaining holds".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        db.set_task_budget(task.id, Some(100), Some(1.0), Some(4))
            .expect("set initial task overrides");
        db.update_task_budget_usage(task.id, 100, 1.0, 4)
            .expect("record cumulative usage");
        db.mark_task_budget_exceeded(task.id, "saved budget reached")
            .expect("hold exceeded task");

        let resolution = json!({
            "task_id": task.id,
            "max_tokens": 200,
            "max_usd": 2.0,
            "max_turns": 8,
            "resolve_exceeded": true,
            "resolution_reason": "approved additional work after diagnosis"
        });
        let worker_result = SetTaskBudgetTool
            .execute(resolution.clone(), &context(home.path(), "worker-backend"))
            .await;
        assert!(worker_result.is_error);
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().budget_exceeded,
            true,
            "worker self-approval cannot clear the hold"
        );
        assert!(db
            .get_task_budget_resolution_history(task.id, 1)
            .unwrap()
            .is_empty());

        let supervisor_result = SetTaskBudgetTool
            .execute(resolution, &context(home.path(), "supervisor"))
            .await;
        assert!(!supervisor_result.is_error, "{}", supervisor_result.output);
        let output: Value = serde_json::from_str(&supervisor_result.output)
            .expect("parse budget resolution tool output");
        let reported_audit = &output["budget_resolution"];
        assert_eq!(reported_audit["task_id"], task.id);
        assert_eq!(reported_audit["resolved_by"], "supervisor");
        assert_eq!(
            reported_audit["reason"],
            "approved additional work after diagnosis"
        );
        assert_eq!(output["task_status"], "needs_info");

        let task_after = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(task_after.status.as_str(), "needs_info");
        assert!(!task_after.budget_exceeded);
        assert_eq!(task_after.budget_tokens_used, 100);
        assert_eq!(task_after.budget_cost_usd, 1.0);
        assert_eq!(task_after.budget_turns_used, 4);
        let audit = db
            .get_task_budget_resolution_history(task.id, 1)
            .unwrap()
            .pop()
            .expect("bounded audit reader returns the new row");
        assert_eq!(reported_audit["id"], audit.id);
        let reopened_reader = super::super::get_task_tool::GetTaskTool
            .execute(json!({"id": task.id}), &context(home.path(), "supervisor"))
            .await;
        assert!(!reopened_reader.is_error, "{}", reopened_reader.output);
        let detail: Value = serde_json::from_str(&reopened_reader.output).unwrap();
        assert_eq!(detail["budget_tokens_used"], 100);
        assert_eq!(detail["budget_exceeded"], false);
        assert_eq!(detail["budget_resolutions"][0]["id"], audit.id);
        assert_eq!(detail["status"], "needs_info");
    }

    #[tokio::test]
    async fn task_budget_resolution_requires_explicit_reason_and_all_new_limits() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        let task = db
            .create(CreateTaskInput {
                title: "budget-held task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "require explicit resolution input".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        db.update_task_budget_usage(task.id, 5, 0.05, 2)
            .expect("record cumulative usage");
        db.mark_task_budget_exceeded(task.id, "worker run budget reached")
            .expect("hold exceeded task");

        for input in [
            json!({"task_id": task.id, "max_tokens": "invalid"}),
            json!({"task_id": task.id, "max_turns": 4294967296_u64}),
            json!({
                "task_id": task.id,
                "max_tokens": 10,
                "max_usd": 0.10,
                "max_turns": 3,
                "resolve_exceeded": true
            }),
            json!({
                "task_id": task.id,
                "max_tokens": 10,
                "max_usd": 0.10,
                "resolve_exceeded": true,
                "resolution_reason": "missing turns cap"
            }),
        ] {
            let result = SetTaskBudgetTool
                .execute(input, &context(home.path(), "supervisor"))
                .await;
            assert!(result.is_error);
        }
        assert!(db
            .get_task_budget_resolution_history(task.id, 10)
            .unwrap()
            .is_empty());
        let still_held = db.get_by_id(task.id).unwrap().unwrap();
        assert!(still_held.budget_exceeded);
        assert_eq!(still_held.budget_tokens_used, 5);
        assert_eq!(still_held.budget_cost_usd, 0.05);
        assert_eq!(still_held.budget_turns_used, 2);
    }
}
