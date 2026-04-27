// update_task_status_tool — Native tool for recipe agents to update task board state.
// Used by the Supervisor recipe to assign tasks, mark in_progress, escalate failures, etc.
// Available in FastDevelopment mode — recipe agents run in this mode.
// Eliminates the need for Python scripts to manipulate tasks.db directly.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct UpdateTaskStatusTool;

#[async_trait]
impl Tool for UpdateTaskStatusTool {
    fn name(&self) -> &str { "update_task_status" }

    fn description(&self) -> &str {
        "Update the status and/or assigned worker for a task on the AutoDev board. \
        Use this to: assign a task to a worker (status='assigned', assigned_worker='worker-backend'), \
        mark as in_progress, escalate to failed, report build failures, or mark as done. \
        Valid statuses: ready, assigned, in_progress, qc_review, done, failed, build_failed, cancelled."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "The task ID to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "build_failed", "cancelled"],
                    "description": "New status for the task"
                },
                "assigned_worker": {
                    "type": "string",
                    "description": "Worker slug to assign (e.g. 'worker-backend', 'worker-frontend'). Pass null to unassign."
                }
            },
            "required": ["id", "status"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };
        let status = match input["status"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return ToolResult::error("'status' is required"),
        };
        // null or absent → leave unassigned; string → set assigned_worker
        let assigned_worker = input["assigned_worker"].as_str().map(|s| s.to_string());

        // ── State-transition guardrails ───────────────────────────────────────
        //
        // Reject nonsense combinations before they touch the DB. Historically
        // the Supervisor LLM produced `{status:'ready', assigned_worker:'X'}`
        // (a half-assignment), which left the task stuck in the READY column
        // tagged with a worker while workers ignored it because status != 'assigned'.
        // See `logs/demo.txt` 2026-04-20T12:52:05 for the failure trace.
        //
        // The canonical invariants are:
        //   - `ready`: assigned_worker MUST be null
        //   - `assigned` / `in_progress`: assigned_worker MUST be set
        //
        // For status='ready' with a worker set, we surface a clear error so
        // the LLM retries with `assign_task` or the correct status.
        if status == "ready" && assigned_worker.is_some() {
            return ToolResult::error(
                "Invalid state: status='ready' requires assigned_worker=null. \
                 If you meant to dispatch work, call `assign_task(id, worker_slug)` instead — \
                 it atomically sets status='assigned' AND assigned_worker."
            );
        }
        if matches!(status.as_str(), "assigned" | "in_progress") && assigned_worker.is_none() {
            return ToolResult::error(&format!(
                "Invalid state: status='{status}' requires assigned_worker to be set. \
                 Pass assigned_worker='worker-<slug>' or use `assign_task` for dispatch."
            ));
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        // Verify task exists
        let task = match db.get_by_id(id) {
            Ok(Some(t)) => t,
            Ok(None)    => return ToolResult::error(&format!("Task #{id} not found")),
            Err(e)      => return ToolResult::error(&format!("Failed to fetch task: {e}")),
        };

        if let Err(e) = db.update_status(id, &status, assigned_worker.as_deref()) {
            return ToolResult::error(&format!("Failed to update task #{id}: {e}"));
        }

        // Phase 3.0 R1 — push-based dispatch on status handoffs.
        //   qc_review → wake QC agent immediately
        //   assigned  → wake the worker immediately (stall resets / re-queues)
        // The `working` gate inside kick_worker prevents duplicate ticks if
        // the target is already running.
        match status.as_str() {
            "qc_review" => crate::recipes::scheduler::kick("qc-agent"),
            "assigned"  => {
                if let Some(ref w) = assigned_worker {
                    crate::recipes::scheduler::kick(w);
                }
            }
            _ => {}
        }

        // Phase 3.0 R1 — task-board push event for the UI.
        crate::events::emit_task_changed_global(crate::events::TaskChangedPayload {
            task_id: Some(id),
            kind:    "status".into(),
            status:  Some(status.clone()),
            assigned_worker: assigned_worker.clone(),
        });

        // Fire fleet hooks for task lifecycle events
        let hook_type = match status.as_str() {
            "assigned" => Some(HookType::OnTaskClaim),
            "done" => Some(HookType::OnTaskComplete),
            "failed" => Some(HookType::OnTaskFail),
            "qc_review" => Some(HookType::OnQcReviewStart),
            _ => None,
        };

        if let Some(ht) = hook_type {
            let ctx = HookContext {
                hook_type: ht.as_str().to_string(),
                timestamp: chrono::Utc::now(),
                session_id: Some(_ctx.session_id.clone()),
                agent_mode: Some("Worker".to_string()),
                tool_call: Some(ToolCallContext {
                    id: _ctx.call_id.clone(),
                    name: "update_task_status".to_string(),
                    input: input.clone(),
                    result: Some(json!({
                        "task_id": id,
                        "previous_status": task.status.as_str(),
                        "new_status": status,
                    })),
                    is_error: false,
                }),
                file_path: None,
                task_id: Some(id.to_string()),
                worker_id: assigned_worker.clone(),
                extra: json!({
                    "task_title": task.title,
                    "previous_status": task.status.as_str(),
                    "new_status": status,
                }),
            };

            // Fire hook asynchronously (don't block tool response)
            tokio::spawn(async move {
                HookRegistry::fire(&ht, &ctx).await;
            });
        }

        ToolResult::success(json!({
            "task_id":         id,
            "title":           task.title,
            "previous_status": task.status.as_str(),
            "new_status":      status,
            "assigned_worker": assigned_worker,
            "message":         format!("✅ Task #{id} updated: status={status}")
        }).to_string())
    }
}
