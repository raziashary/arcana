// claim_task — Atomic "begin working on a task" tool for fleet workers.
//
// Replaces calling update_task_status with status=in_progress. Records the
// worker's process PID so OrphanDetector can release the task if the worker
// process dies unexpectedly (Ruflo Feature 2 — PID-based orphan detection).

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ClaimTaskTool;

#[async_trait]
impl Tool for ClaimTaskTool {
    fn name(&self) -> &str { "claim_task" }

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

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_id = match input["task_id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'task_id' must be a positive integer"),
        };

        // Worker identity comes from the __agent_role passed by the MCP client.
        // Fall back to "unknown" to avoid a hard failure, but log for visibility.
        let worker_slug = ctx.agent_role.as_deref().unwrap_or("unknown").to_string();
        if worker_slug == "unknown" {
            tracing::warn!(
                "[claim_task] __agent_role not set for task #{task_id} — \
                 orphan detection will use 'unknown' as worker slug"
            );
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        // Verify task exists and is in 'assigned' state
        let task = match db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None)    => return ToolResult::error(format!("Task #{task_id} not found")),
            Err(e)      => return ToolResult::error(format!("DB error fetching task #{task_id}: {e}")),
        };

        if task.status.as_str() != "assigned" {
            return ToolResult::error(format!(
                "Task #{task_id} is in status='{}', not 'assigned'. \
                 Only assigned tasks can be claimed.",
                task.status.as_str()
            ));
        }

        let pid = std::process::id();

        if let Err(e) = db.claim_task_with_pid(task_id, &worker_slug, pid) {
            return ToolResult::error(format!("Failed to claim task #{task_id}: {e}"));
        }

        // Emit task-changed so the UI kanban moves the card
        crate::events::emit_task_changed_global(crate::events::TaskChangedPayload {
            task_id:         Some(task_id),
            kind:            "status".into(),
            status:          Some("in_progress".into()),
            assigned_worker: Some(worker_slug.clone()),
        });

        ToolResult::success(json!({
            "task_id":     task_id,
            "title":       task.title,
            "status":      "in_progress",
            "worker_slug": worker_slug,
            "pid":         pid,
            "message": format!(
                "✅ Task #{task_id} claimed by {worker_slug} (PID={pid}, status=in_progress). \
                 OrphanDetector will monitor this PID."
            )
        }).to_string())
    }
}
