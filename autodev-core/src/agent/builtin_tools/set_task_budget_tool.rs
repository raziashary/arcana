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
         rather than silently burning tokens. Omit a field to leave it unlimited."
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
                    "description": "Max total tokens (input + output) for this task. Default: 100,000."
                },
                "max_usd": {
                    "type": "number",
                    "description": "Max USD spend for this task. Default: $0.50."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max agent loop turns for this task. Default: 20."
                }
            },
            "required": ["task_id"]
        })
    }


    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let task_id = match input.get("task_id").and_then(|v| v.as_i64()) {
            Some(id) => id,
            None => return ToolResult::error("Missing required field: task_id (integer)"),
        };

        let max_tokens = input.get("max_tokens").and_then(|v| v.as_u64());
        let max_usd = input.get("max_usd").and_then(|v| v.as_f64());
        let max_turns = input
            .get("max_turns")
            .and_then(|v| v.as_u64())
            .map(|t| t as u32);

        // Validate using ADR-097 rules
        if let Err(code) = budget::validate_budget(max_tokens, max_usd, max_turns) {
            return ToolResult::error(&format!("Budget validation failed: {code}"));
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("DB error: {e}")),
        };

        if let Err(e) = db.set_task_budget(task_id, max_tokens, max_usd, max_turns) {
            return ToolResult::error(&format!("Failed to set budget for task {task_id}: {e}"));
        }

        let effective_tokens = max_tokens.unwrap_or(100_000);
        let effective_usd = max_usd.unwrap_or(0.50);
        let effective_turns = max_turns.unwrap_or(20);

        info!(
            "[budget] Set budget for task {task_id}: {effective_tokens} tokens / \
             ${effective_usd:.2} USD / {effective_turns} turns"
        );

        ToolResult::success(
            json!({
                "task_id": task_id,
                "budget_set": {
                    "max_tokens": effective_tokens,
                    "max_usd": effective_usd,
                    "max_turns": effective_turns,
                },
                "message": format!(
                    "✅ Budget set for task #{task_id}: {effective_tokens} tokens / \
                     ${effective_usd:.2} USD / {effective_turns} turns max."
                )
            })
            .to_string(),
        )
    }
}
