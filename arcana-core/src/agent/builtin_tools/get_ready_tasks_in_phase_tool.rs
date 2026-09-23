// get_ready_tasks_in_phase — Ruflo Feature 5. Returns tasks in a specific epic
// phase that are ready or assigned — i.e. available for the Supervisor to dispatch.
// Supervisor calls this after get_epic_progress confirms a phase is unlocked.
// Available in FastDevelopment | Review modes.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GetReadyTasksInPhaseTool;

#[async_trait]
impl Tool for GetReadyTasksInPhaseTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "get_ready_tasks_in_phase"
    }

    fn description(&self) -> &str {
        "List tasks in a specific epic phase that are ready or assigned (available to dispatch). \
         Use after get_epic_progress confirms the phase is unlocked. Returns task IDs, titles, \
         priority, and status so the Supervisor can assign them to workers."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "epic_id": {
                    "type": "string",
                    "description": "ID of the parent Epic task"
                },
                "phase": {
                    "type": "string",
                    "description": "Phase name to query, e.g. 'foundation', 'core', 'integration', 'qa'"
                }
            },
            "required": ["epic_id", "phase"]
        })
    }


    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let epic_id = match input["epic_id"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'epic_id' is required"),
        };
        let phase = match input["phase"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'phase' is required"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let tasks = match db.get_ready_tasks_in_phase(&epic_id, &phase) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(&format!("Failed to get phase tasks: {e}")),
        };

        let tasks_json: Vec<Value> = tasks
            .iter()
            .map(|t| {
                json!({
                    "id":       t.id,
                    "title":    t.title,
                    "priority": t.priority.as_str(),
                    "status":   t.status.as_str(),
                    "phase":    t.phase,
                    "blocked_by": t.blocked_by,
                    "acceptance_criteria": t.acceptance_criteria,
                })
            })
            .collect();

        ToolResult::success(json!({
            "epic_id":   epic_id,
            "phase":     phase,
            "count":     tasks_json.len(),
            "tasks":     tasks_json,
            "message": if tasks_json.is_empty() {
                format!("No ready/assigned tasks in phase '{}' of epic #{}.", phase, epic_id)
            } else {
                format!("{} task(s) ready to assign in phase '{}' of epic #{}.", tasks_json.len(), phase, epic_id)
            }
        }).to_string())
    }
}
