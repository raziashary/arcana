// list_worker_requests — Supervisor reads the fleet escalation queue every tick.
// The supervisor recipe (Step 1a) MUST call this to see pending worker requests
// (decompose_audit, reassign, needs_info, report_issue, request_review).
// Without this tool, the Supervisor cannot drain the queue and workers' escalations
// pile up indefinitely.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ListWorkerRequestsTool;

#[async_trait]
impl Tool for ListWorkerRequestsTool {
    fn name(&self) -> &str {
        "list_worker_requests"
    }

    fn description(&self) -> &str {
        "List worker→Supervisor requests from the fleet escalation queue. \
         Supervisor calls this every tick (Step 1a) to drain pending requests. \
         Filter by status='pending' to see only unresolved items. \
         Parse the `payload` JSON field for structured request data (suggested_tasks, target_worker, issue_description, etc.)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status. Omit to list all.",
                    "enum": ["pending", "acknowledged", "completed", "rejected"]
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

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let reqs = match db.list_worker_requests(status_filter) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(&format!("Failed to list worker requests: {e}")),
        };

        let result = reqs
            .iter()
            .map(|r| {
                json!({
                    "id":              r.id,
                    "worker_slug":     r.worker_slug,
                    "task_id":         r.task_id,
                    "request_type":    r.request_type,
                    "payload":         r.payload,
                    "status":          r.status.as_str(),
                    "resolution_note": r.resolution_note,
                    "created_at":      r.created_at,
                    "resolved_at":     r.resolved_at,
                })
            })
            .collect::<Vec<_>>();

        let count = result.len();
        let payload = json!({
            "count":    count,
            "filter":   status_filter.unwrap_or("all"),
            "requests": result,
        })
        .to_string();

        if count == 0 && status_filter == Some("pending") {
            ToolResult::success_with_hint(
                payload,
                "No pending worker requests. Proceed to Step 2 — dispatch READY tasks.",
            )
        } else {
            ToolResult::success(payload)
        }
    }
}
