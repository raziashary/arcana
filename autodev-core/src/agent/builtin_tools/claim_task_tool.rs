// claim_task — Atomic "begin working on a task" tool for fleet workers.
//
// Replaces calling update_task_status with status=in_progress. Records the
// worker's process PID so OrphanDetector can release the task if the worker
// process dies unexpectedly (Ruflo Feature 2 — PID-based orphan detection).

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct ClaimTaskTool;

#[async_trait]
impl Tool for ClaimTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "claim_task"
    }

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


    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_id = match input["task_id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'task_id' must be a positive integer"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        // Verify task exists and is in 'assigned' state
        let task = match db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None) => return ToolResult::error(format!("Task #{task_id} not found")),
            Err(e) => return ToolResult::error(format!("DB error fetching task #{task_id}: {e}")),
        };

        if task.status.as_str() != "assigned" {
            return ToolResult::error(format!(
                "Task #{task_id} is in status='{}', not 'assigned'. \
                 Only assigned tasks can be claimed.",
                task.status.as_str()
            ));
        }

        // Worker identity resolution.
        //
        // Source of truth ORDER:
        //   1. `task.assigned_worker` — set atomically by assign_task /
        //      assign_tasks, definitively names who owns this task.
        //   2. `ctx.agent_role` — the MCP request's `__agent_role` field,
        //      passed by the LivingAgent. The Python bridge doesn't always
        //      forward this through to the rmcp call (the source of the
        //      "__agent_role not set" warnings in the logs).
        //
        // Previous behaviour preferred (2) and overwrote `assigned_worker`
        // with "unknown" when MCP didn't pass it — causing the kanban to
        // show "unknown" on in_progress cards even though the Ready column
        // had named the worker correctly a second earlier. That cascaded
        // into the worker-affinity panel and live monitor losing the link
        // between agent and task. Prefer the DB value; only fall back to
        // ctx if the task somehow has no assigned_worker (shouldn't happen
        // after assign_task ran, but guard anyway).
        let worker_slug = match task.assigned_worker.clone() {
            Some(w) => w,
            None => {
                // Falls back to the caller's own role, which is now always a
                // real slug. This used to fall through to the literal string
                // "unknown" whenever the MCP bridge dropped __agent_role,
                // which then broke orphan detection.
                tracing::warn!(
                    "[claim_task] task #{task_id} has no assigned_worker; attributing the claim to the caller '{}'",
                    ctx.agent_role
                );
                ctx.agent_role.clone()
            }
        };

        let pid = std::process::id();

        if let Err(e) = db.claim_task_with_pid(task_id, &worker_slug, pid) {
            return ToolResult::error(format!("Failed to claim task #{task_id}: {e}"));
        }

        // Emit task-changed so the UI kanban moves the card
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(task_id),
            kind: "status".into(),
            status: Some("in_progress".into()),
            assigned_worker: Some(worker_slug.clone()),
        });

        ToolResult::success(
            json!({
                "task_id":     task_id,
                "title":       task.title,
                "status":      "in_progress",
                "worker_slug": worker_slug,
                "pid":         pid,
                "message": format!(
                    "✅ Task #{task_id} claimed by {worker_slug} (PID={pid}, status=in_progress). \
                     OrphanDetector will monitor this PID."
                )
            })
            .to_string(),
        )
    }
}
