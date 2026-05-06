// get_task_tool — Native tool to fetch a single task by ID from the task board.
// K.7: Complements list_tasks by letting agents fetch full detail (including
// rejection_reason and retry_count) for a specific task without scanning all tasks.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct GetTaskTool;

#[async_trait]
impl Tool for GetTaskTool {
    fn name(&self) -> &str { "get_task" }

    fn description(&self) -> &str {
        "Fetch a single task by its numeric ID. Returns the full task record including \
        rejection_reason (set by QC when status=rejected) and retry_count. \
        Use this after list_tasks to get full details on a specific task."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "The numeric task ID to fetch"
                }
            },
            "required": ["id"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        match db.get_by_id(id) {
            Ok(Some(t)) => ToolResult::success(json!({
                "id":                  t.id,
                "title":               t.title,
                "description":         t.description,
                "priority":            t.priority.as_str(),
                "task_type":           t.task_type.as_str(),
                "status":              t.status.as_str(),
                "acceptance_criteria": t.acceptance_criteria,
                "assigned_worker":     t.assigned_worker,
                "rejection_reason":    t.rejection_reason,
                "retry_count":         t.retry_count,
                "created_by":          t.created_by,
                "created_at":          t.created_at,
                "updated_at":          t.updated_at,
                "completed_at":        t.completed_at,
            }).to_string()),
            Ok(None)  => ToolResult::error(format!("Task #{id} not found")),
            Err(e)    => ToolResult::error(format!("Failed to fetch task #{id}: {e}")),
        }
    }
}
