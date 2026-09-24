// request_supervisor_action — Worker→Supervisor communication tool.
// Available to fleet workers through the in-process registry, so a worker
// can send a structured request to the Supervisor without holding
// create_task or assign_task privileges.
//
// ⚠ This sentence was left truncated mid-clause by `P8-50`'s edit and
// repaired in `P8-60`.
//
// Use cases:
//   - decompose_audit:  Worker completed an audit and discovered follow-up tasks
//   - reassign:         Task was mis-assigned, needs different worker
//   - needs_info:       Worker needs clarification before proceeding
//   - report_issue:     Task encountered a blocking issue (dependency, bug, etc.)
//   - request_review:   Worker believes task is done but wants QC to verify early

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::{CreateWorkerRequestInput, TasksDb};
use crate::events::{emit_worker_request, WorkerRequestPayload};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct RequestSupervisorActionTool;

#[async_trait]
impl Tool for RequestSupervisorActionTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "request_supervisor_action"
    }

    fn description(&self) -> &str {
        "Send a structured request to the Supervisor. Workers use this to escalate \
         audit results, request re-assignment, report blockers, or ask for review. \
         The Supervisor polls these requests every tick and acts on them."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "The ID of the task that triggered this request (optional, but strongly recommended for context)."
                },
                "request_type": {
                    "type": "string",
                    "enum": ["decompose_audit", "reassign", "needs_info", "report_issue", "request_review"],
                    "description": "Type of request. \
                        decompose_audit = audit found multiple sub-tasks; \
                        reassign = task should go to a different worker; \
                        needs_info = need clarification before continuing; \
                        report_issue = blocking issue/bug discovered; \
                        request_review = task is ready for QC early."
                },
                "payload": {
                    "type": "object",
                    "description": "Structured payload. Schema depends on request_type. \
                        For decompose_audit, include 'audit_summary' and 'suggested_tasks' array with full task payloads. \
                        For reassign, include 'target_worker' and 'reason'. \
                        For needs_info, include 'question' and 'what_you_tried'. \
                        For report_issue, include 'issue_description' and 'severity'. \
                        For request_review, include 'completion_summary' and 'files_changed'."
                }
            },
            "required": ["request_type", "payload"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let request_type = match input["request_type"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'request_type' is required"),
        };

        let valid_types = [
            "decompose_audit",
            "reassign",
            "needs_info",
            "report_issue",
            "request_review",
        ];
        if !valid_types.contains(&request_type.as_str()) {
            return ToolResult::error(&format!(
                "Invalid request_type '{}'. Valid: {:?}",
                request_type, valid_types
            ));
        }

        let task_id = input["task_id"].as_i64();

        let payload = match &input["payload"] {
            Value::Object(_) => input["payload"].to_string(),
            _ => return ToolResult::error("'payload' must be a JSON object"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        // Worker identity resolution — same pattern as claim_task.
        // Source of truth ORDER:
        //   1. `task.assigned_worker` from the task this request is *about*
        //      (passed as task_id). Authoritative — assign_task wrote it.
        //   2. `ctx.agent_role` — MCP __agent_role; the Python bridge
        //      occasionally drops this on the floor.
        //   3. "unknown" — last resort, logged for visibility.
        let task_assigned_worker = task_id
            .and_then(|tid| db.get_by_id(tid).ok().flatten())
            .and_then(|t| t.assigned_worker);
        let worker_slug = task_assigned_worker.unwrap_or_else(|| {
            // The caller's role is always a real slug now, so a request can no
            // longer be recorded against "unknown".
            tracing::warn!(
                "[request_supervisor_action] no assigned_worker on the task; attributing the request to the caller '{}'",
                ctx.agent_role
            );
            ctx.agent_role.clone()
        });

        let request = match db.create_worker_request(&CreateWorkerRequestInput {
            worker_slug: worker_slug.clone(),
            task_id,
            request_type: request_type.clone(),
            payload: payload.clone(),
        }) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(&format!("Failed to create worker request: {e}")),
        };

        // Wake the Supervisor immediately so it acts on this request on its next tick.
        crate::recipes::scheduler::kick("supervisor");

        // Push event to the frontend so the Worker Requests widget updates live.
        emit_worker_request(WorkerRequestPayload {
            id: request.id,
            worker_slug: request.worker_slug.clone(),
            task_id: request.task_id,
            request_type: request.request_type.clone(),
            status: request.status.as_str().to_string(),
            payload: request.payload.clone(),
        });

        ToolResult::success(
            json!({
                "request_id": request.id,
                "status": "pending",
                "message": format!(
                    "✅ Worker request #{} sent to Supervisor (type: {}). \
                     The Supervisor will process it on its next tick.",
                    request.id, request_type
                )
            })
            .to_string(),
        )
    }
}
