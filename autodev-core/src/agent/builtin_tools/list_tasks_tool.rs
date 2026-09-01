// list_tasks_tool — Native tool for recipe agents to read the AutoDev task board.
// Available in FastDevelopment mode so the Supervisor recipe can poll tasks directly
// without needing to write Python scripts.
// Returns JSON array of Task objects, optionally filtered by status.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ListTasksTool;

#[async_trait]
impl Tool for ListTasksTool {
    fn name(&self) -> &str {
        "list_tasks"
    }

    fn description(&self) -> &str {
        "List tasks on the AutoDev internal board. Optionally filter by status. \
        In compact mode (default) returns only {id, title, status, assigned_worker, priority, retry_count} \
        to avoid token bloat. Use get_task(id) for full details on a specific task."
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
                },
                "compact": {
                    "type": "boolean",
                    "description": "When true (default), return minimal fields only. Set false to include description and acceptance_criteria.",
                    "default": true
                }
            },
            "required": []
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let status_filter = input["status"].as_str();
        // compact=true by default — avoids token bloat from description + acceptance_criteria.
        // Agents should call get_task(id) when they need full detail on a specific task.
        let compact = input["compact"].as_bool().unwrap_or(true);

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let tasks = match db.list(status_filter) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(&format!("Failed to list tasks: {e}")),
        };

        let result = tasks
            .iter()
            .map(|t| {
                if compact {
                    // Compact: decision-relevant fields only (~80 chars per task vs ~500+)
                    json!({
                        "id":              t.id,
                        "title":           t.title,
                        "priority":        t.priority.as_str(),
                        "task_type":       t.task_type.as_str(),
                        "status":          t.status.as_str(),
                        "assigned_worker": t.assigned_worker,
                        "rejection_reason": t.rejection_reason,
                        "retry_count":     t.retry_count,
                    })
                } else {
                    // Full: everything including description and acceptance_criteria
                    json!({
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
                    })
                }
            })
            .collect::<Vec<_>>();

        let payload = json!({
            "count":   result.len(),
            "filter":  status_filter.unwrap_or("all"),
            "compact": compact,
            "tasks":   result
        })
        .to_string();

        if compact {
            ToolResult::success_with_hint(
                payload,
                "Compact list — call get_task(id) for full description/acceptance_criteria on any task.",
            )
        } else {
            ToolResult::success(payload)
        }
    }
}
