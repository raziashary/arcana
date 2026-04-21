// assign_task — Atomic "assign task to worker" tool for the Supervisor agent.
//
// Motivation:
//   The generic `update_task_status` tool takes both `status` and
//   `assigned_worker` as free parameters, which is ambiguous for the LLM.
//   In practice supervisors were writing `{status:'ready', assigned_worker:'X'}`
//   — a nonsense half-assignment that left the task stuck in the READY column
//   with a worker tag, while workers correctly ignored it (because status
//   was not `assigned`). See `logs/demo.txt` 2026-04-20T12:52:05 for the
//   failure trace.
//
// This tool removes the ambiguity: `assign_task(id, worker_slug)` is a single
// atomic operation that ALWAYS sets status='assigned' AND assigned_worker
// together. The LLM cannot produce an invalid state.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct AssignTaskTool;

/// The set of worker slugs the Supervisor is allowed to assign to.
/// Keeping this explicit prevents the LLM from inventing slugs like
/// `worker-docs` that don't map to a real worker. Changes here must stay
/// in sync with the worker recipes in `recipes/defaults.rs`.
const VALID_WORKERS: &[&str] = &[
    "worker-backend",
    "worker-frontend",
    "worker-platform",
    "worker-mcp",
    "worker-cleanup",
];

#[async_trait]
impl Tool for AssignTaskTool {
    fn name(&self) -> &str { "assign_task" }

    fn description(&self) -> &str {
        "Atomically assign a ready task to a worker. Sets status='assigned' AND \
        assigned_worker in one operation — this is the ONLY correct way for the \
        Supervisor to dispatch work. Prefer this over update_task_status for \
        assignments. Valid workers: worker-backend, worker-frontend, \
        worker-platform, worker-mcp, worker-cleanup."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "The task ID to assign (must currently be in status='ready')"
                },
                "worker_slug": {
                    "type": "string",
                    "enum": VALID_WORKERS,
                    "description": "The worker to dispatch the task to"
                }
            },
            "required": ["id", "worker_slug"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };
        let worker_slug = match input["worker_slug"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'worker_slug' is required"),
        };
        if !VALID_WORKERS.contains(&worker_slug.as_str()) {
            return ToolResult::error(&format!(
                "'{worker_slug}' is not a valid worker. Expected one of: {}",
                VALID_WORKERS.join(", ")
            ));
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        // Verify task exists and is in a dispatchable state. Re-assigning an
        // already-in_progress or already-done task is refused deterministically
        // so the supervisor can't accidentally kick active work.
        let task = match db.get_by_id(id) {
            Ok(Some(t)) => t,
            Ok(None)    => return ToolResult::error(&format!("Task #{id} not found")),
            Err(e)      => return ToolResult::error(&format!("Failed to fetch task: {e}")),
        };

        let prev_status = task.status.as_str().to_string();
        let dispatchable = matches!(
            prev_status.as_str(),
            "ready" | "assigned" | "failed"
        );
        if !dispatchable {
            return ToolResult::error(&format!(
                "Task #{id} is in status='{prev_status}' and cannot be (re)assigned. \
                 Only 'ready', 'assigned', or 'failed' tasks may be assigned."
            ));
        }

        if let Err(e) = db.update_status(id, "assigned", Some(&worker_slug)) {
            return ToolResult::error(&format!("Failed to assign task #{id}: {e}"));
        }

        // Fleet hook — downstream listeners (UI, event bus) react to task
        // lifecycle events. `OnTaskClaim` is the canonical "someone owns this
        // now" signal.
        let ctx = HookContext {
            hook_type:  HookType::OnTaskClaim.as_str().to_string(),
            timestamp:  chrono::Utc::now(),
            session_id: Some(_ctx.session_id.clone()),
            agent_mode: Some("Supervisor".to_string()),
            tool_call: Some(ToolCallContext {
                id:       _ctx.call_id.clone(),
                name:     "assign_task".to_string(),
                input:    input.clone(),
                result:   Some(json!({
                    "task_id":         id,
                    "previous_status": prev_status,
                    "new_status":      "assigned",
                    "assigned_worker": worker_slug,
                })),
                is_error: false,
            }),
            file_path: None,
            task_id:   Some(id.to_string()),
            worker_id: Some(worker_slug.clone()),
            extra: json!({
                "task_title":      task.title,
                "previous_status": prev_status,
                "new_status":      "assigned",
            }),
        };
        tokio::spawn(async move {
            HookRegistry::fire(&HookType::OnTaskClaim, &ctx).await;
        });

        ToolResult::success(json!({
            "task_id":         id,
            "title":           task.title,
            "previous_status": prev_status,
            "new_status":      "assigned",
            "assigned_worker": worker_slug,
            "message":         format!(
                "✅ Task #{id} assigned to {worker_slug} (status=assigned). \
                 Worker will pick it up on its next tick."
            )
        }).to_string())
    }
}
