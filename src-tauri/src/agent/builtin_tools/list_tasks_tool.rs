// list_tasks_tool — Native tool for recipe agents to read the AutoDev task board.
// Available in FastDevelopment mode so the Supervisor recipe can poll tasks directly
// without needing to write Python scripts.
// Returns JSON array of Task objects, optionally filtered by status.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ListTasksTool;

#[async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str { "list_tasks" }

    fn description(&self) -> &str {
        "List all tasks on the AutoDev internal board. Optionally filter by status. \
        Leave status empty to list all tasks. Returns an array of task objects including \
        id, title, status, priority, assigned_worker, acceptance_criteria, \
        rejection_reason, and retry_count."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status. Omit to list all.",
                    "enum": ["ready", "assigned", "in_progress", "qc_review", "done",
                             "failed", "build_failed", "cancelled", "rejected"]
                }
            },
            "required": []
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let status_filter = input["status"].as_str();

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let tasks = match db.list(status_filter) {
            Ok(t)  => t,
            Err(e) => return ToolResult::error(&format!("Failed to list tasks: {e}")),
        };

        let result = tasks.iter().map(|t| json!({
            "id":                  t.id,
            "title":               t.title,
            "description":         t.description,
            "priority":            t.priority.as_str(),
            "task_type":           t.task_type.as_str(),
            "status":              t.status.as_str(),
            "acceptance_criteria": t.acceptance_criteria,
            "assigned_worker":     t.assigned_worker,
            "rejection_reason":    t.rejection_reason,   // K.7 — why QC rejected
            "retry_count":         t.retry_count,         // K.7 — how many retries
            "created_by":          t.created_by,
            "created_at":          t.created_at,
            "updated_at":          t.updated_at,
            "completed_at":        t.completed_at,
        })).collect::<Vec<_>>();

        ToolResult::success(json!({
            "count": result.len(),
            "filter": status_filter.unwrap_or("all"),
            "tasks": result
        }).to_string())
    }
}
