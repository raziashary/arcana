// resolve_worker_request — Supervisor marks a worker request as completed/
// acknowledged/rejected after acting on it. Required by the Supervisor recipe
// Step 1a: each branch ends with "Mark request completed/acknowledged".
// Without this tool, requests sit in the queue forever and the Supervisor
// re-processes them every tick (infinite work loop).

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ResolveWorkerRequestTool;

#[async_trait]
impl Tool for ResolveWorkerRequestTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "resolve_worker_request"
    }

    fn description(&self) -> &str {
        "Mark a worker request as completed, acknowledged, or rejected after the \
         Supervisor has acted on it. ALWAYS call this after handling a request from \
         `list_worker_requests` — otherwise the request stays pending and the \
         Supervisor re-processes it every tick. \
         Use action='completed' when the request was fulfilled (e.g., decompose_audit → tasks created), \
         'acknowledged' when no further action is needed (e.g., needs_info awaiting human), \
         'rejected' when the request was invalid or refused."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "The worker request ID returned by list_worker_requests."
                },
                "action": {
                    "type": "string",
                    "enum": ["completed", "acknowledged", "rejected"],
                    "description": "Resolution. completed=fulfilled, acknowledged=seen-no-action, rejected=declined."
                },
                "note": {
                    "type": "string",
                    "description": "Optional one-line resolution note (e.g., 'Created tasks 142-146 from decompose_audit')."
                }
            },
            "required": ["id", "action"]
        })
    }


    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(v) => v,
            None => return ToolResult::error("'id' is required (integer worker request ID)"),
        };
        let action = match input["action"].as_str() {
            Some(s) if matches!(s, "completed" | "acknowledged" | "rejected") => s,
            Some(s) => {
                return ToolResult::error(&format!(
                    "Invalid action '{s}'. Must be one of: completed, acknowledged, rejected."
                ))
            }
            None => return ToolResult::error("'action' is required"),
        };
        let note = input["note"].as_str();

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        if let Err(e) = db.update_worker_request_status(id, action, note) {
            return ToolResult::error(&format!("Failed to resolve worker request {id}: {e}"));
        }

        ToolResult::success(
            json!({
                "id":     id,
                "action": action,
                "note":   note,
                "message": format!("Worker request #{id} marked as {action}."),
            })
            .to_string(),
        )
    }
}
