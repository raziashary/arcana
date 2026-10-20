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

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct AssignTaskTool;

#[async_trait]
impl Tool for AssignTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "assign_task"
    }

    fn description(&self) -> &str {
        "Atomically dispatch a task. Omit worker_slug and Arcana selects a runnable \
        implementation worker from task signals and automatic outcome history. To \
        override that choice, provide a runnable worker_slug and a non-empty \
        override_reason; overrides are audited and excluded from routing learning."
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
                    "description": "Optional explicit override. Must name a runnable implementation worker and requires override_reason. Omit to let Arcana route deterministically."
                },
                "override_reason": {
                    "type": "string",
                    "description": "Required when worker_slug is supplied; persisted for audit and excluded from automatic routing learning."
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        if ctx.agent_role.trim() != "supervisor" {
            return ToolResult::error("Only the Supervisor may assign or reassign fleet tasks");
        }
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };
        let workers = match crate::db::workers::WorkersDb::open().and_then(|db| db.list()) {
            Ok(workers) => workers,
            Err(e) => return ToolResult::error(&format!("Failed to read worker roster: {e}")),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        // Verify task exists and is in a dispatchable state. Re-assigning an
        // already-in_progress or already-done task is refused deterministically
        // so the supervisor can't accidentally kick active work.
        let task = match db.get_by_id(id) {
            Ok(Some(t)) => t,
            Ok(None) => return ToolResult::error(&format!("Task #{id} not found")),
            Err(e) => return ToolResult::error(&format!("Failed to fetch task: {e}")),
        };

        let prev_status = task.status.as_str().to_string();
        if task.task_type.as_str() == "epic" {
            return ToolResult::error(format!(
                "Task #{id} is an epic. Epic records track progress and cannot be assigned to a worker."
            ));
        }
        // K.9 — Added 'rejected' to the dispatchable states. Without this, the
        // supervisor calling assign_task on a QC-rejected task would get a hard
        // error, breaking the entire Phase A retry path at the tool level.
        let dispatchable = matches!(
            prev_status.as_str(),
            "ready" | "assigned" | "failed" | "rejected"
        );
        if !dispatchable {
            return ToolResult::error(&format!(
                "Task #{id} is in status='{prev_status}' and cannot be (re)assigned. \
                 Only 'ready', 'assigned', 'failed', or 'rejected' tasks may be assigned."
            ));
        }

        let override_reason = input["override_reason"].as_str().map(str::trim);
        let explicit_worker = input["worker_slug"].as_str().map(str::trim);
        let (worker_slug, override_reason) = match explicit_worker {
            Some(slug) if !slug.is_empty() => {
                let Some(reason) = override_reason.filter(|reason| !reason.is_empty()) else {
                    return ToolResult::error(
                        "An explicit worker_slug requires a non-empty override_reason",
                    );
                };
                let Some(worker) = workers.iter().find(|worker| worker.slug == slug) else {
                    return ToolResult::error(format!("'{slug}' is not a registered worker"));
                };
                if !crate::routing::TaskRouter::is_runnable_implementation_worker(worker) {
                    return ToolResult::error(format!(
                        "'{slug}' is not a started, runnable implementation worker"
                    ));
                }
                (slug.to_string(), Some(reason.to_string()))
            }
            Some(_) => return ToolResult::error("'worker_slug' cannot be empty"),
            None => {
                if override_reason.is_some() {
                    return ToolResult::error("'override_reason' requires an explicit worker_slug");
                }
                let selected = match crate::routing::TaskRouter::select_worker_for_dispatch(&task) {
                    Ok(selected) => selected,
                    Err(error) => {
                        return ToolResult::error(&format!("Automatic routing failed: {error}"))
                    }
                };
                (selected.worker_slug, None)
            }
        };

        let routing_entry = crate::routing::TaskRouter::assignment_entry(&task, &worker_slug);
        let assignment = match override_reason.as_deref() {
            Some(reason) => {
                db.assign_with_routing_override(id, &worker_slug, &routing_entry, reason)
            }
            None => db.assign_with_routing_history(id, &worker_slug, &routing_entry),
        };
        if let Err(e) = assignment {
            return ToolResult::error(&format!("Failed to assign task #{id}: {e}"));
        }

        // Phase 3.0 R1 — push-based dispatch. The supervisor just handed work
        // to a worker; wake that worker *now* instead of waiting up to its
        // cron cadence. This is the fix for the "serialised pickup" symptom
        // where 8 assigned tasks rolled in one at a time.
        crate::recipes::scheduler::kick(&worker_slug);

        // Phase 3.0 R1 — task-board push event for the UI.
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(id),
            kind: "assigned".into(),
            status: Some("assigned".into()),
            assigned_worker: Some(worker_slug.clone()),
        });

        // Fleet hook — downstream listeners (UI, event bus) react to task
        // lifecycle events. `OnTaskClaim` is the canonical "someone owns this
        // now" signal.
        let ctx = HookContext {
            hook_type: HookType::OnTaskClaim.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some(ctx.session_id.clone()),
            agent_mode: Some("Supervisor".to_string()),
            tool_call: Some(ToolCallContext {
                id: ctx.call_id.clone(),
                name: "assign_task".to_string(),
                input: input.clone(),
                result: Some(json!({
                    "task_id":         id,
                    "previous_status": prev_status,
                    "new_status":      "assigned",
                    "assigned_worker": worker_slug,
                })),
                is_error: false,
            }),
            file_path: None,
            task_id: Some(id.to_string()),
            worker_id: Some(worker_slug.clone()),
            workspace_root: Some(ctx.working_dir.clone()),
            extra: json!({
                "task_title":      task.title,
                "previous_status": prev_status,
                "new_status":      "assigned",
            }),
        };
        tokio::spawn(async move {
            HookRegistry::fire(&HookType::OnTaskClaim, &ctx).await;
        });

        ToolResult::success(
            json!({
                "task_id":         id,
                "title":           task.title,
                "previous_status": prev_status,
                "new_status":      "assigned",
                "assigned_worker": worker_slug,
                "message":         format!(
                    "✅ Task #{id} assigned to {worker_slug} (status=assigned). \
                     Worker will pick it up on its next tick."
                )
            })
            .to_string(),
        )
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use crate::agent::{permission::PermissionMode, tool_trait::ToolContext};

    #[tokio::test]
    async fn worker_cannot_call_assignment_tool() {
        let context = ToolContext {
            session_id: "assignment-security-test".into(),
            working_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            call_id: "worker-assignment-call".into(),
            run_id: None,
            task_id: Some(1),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = AssignTaskTool.execute(json!({"id": 2}), &context).await;
        assert!(result.is_error);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Only the Supervisor"));
    }
}
