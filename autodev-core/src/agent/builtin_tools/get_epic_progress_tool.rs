// get_epic_progress — Ruflo Feature 5. Returns per-phase progress summaries
// for an epic, including task counts, completion state, and unlock gating.
// Supervisor uses this to decide which phase's tasks can be assigned next.
// Available in FastDevelopment | Review modes.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GetEpicProgressTool;

#[async_trait]
impl Tool for GetEpicProgressTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "get_epic_progress"
    }

    fn description(&self) -> &str {
        "Get phase-by-phase progress for an epic. Returns each phase with task counts \
         (total, done, in_progress, ready, blocked), whether the phase is complete, \
         and whether it is unlocked (previous phase done). Use this to determine which \
         tasks can be assigned next."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "epic_id": {
                    "type": "string",
                    "description": "ID of the Epic task to inspect"
                }
            },
            "required": ["epic_id"]
        })
    }


    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let epic_id = match input["epic_id"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'epic_id' is required"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let summaries = match db.get_phase_summary(&epic_id) {
            Ok(s) => s,
            Err(e) => return ToolResult::error(&format!("Failed to get epic progress: {e}")),
        };

        if summaries.is_empty() {
            return ToolResult::success(json!({
                "epic_id": epic_id,
                "phases":  [],
                "message": format!("No phases found for epic #{}. Add subtasks via create_phased_task.", epic_id)
            }).to_string());
        }

        let total_tasks: i32 = summaries.iter().map(|s| s.total_tasks).sum();
        let total_done: i32 = summaries.iter().map(|s| s.done_tasks).sum();
        let all_complete = summaries.iter().all(|s| s.is_complete);

        let phases_json: Vec<Value> = summaries
            .iter()
            .map(|s| {
                json!({
                    "phase":           s.phase,
                    "phase_order":     s.phase_order,
                    "milestone":       s.milestone,
                    "total_tasks":     s.total_tasks,
                    "done_tasks":      s.done_tasks,
                    "in_progress":     s.in_progress_tasks,
                    "ready_tasks":     s.ready_tasks,
                    "blocked_tasks":   s.blocked_tasks,
                    "is_complete":     s.is_complete,
                    "is_unlocked":     s.is_unlocked,
                })
            })
            .collect();

        ToolResult::success(
            json!({
                "epic_id":      epic_id,
                "total_tasks":  total_tasks,
                "total_done":   total_done,
                "all_complete": all_complete,
                "phases":       phases_json,
            })
            .to_string(),
        )
    }
}
