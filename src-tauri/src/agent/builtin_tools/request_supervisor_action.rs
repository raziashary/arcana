// request_supervisor_action — Worker→Supervisor communication tool.
// Available to fleet workers via MCP (register_for_mcp_worker) so that Mode 2
// LivingAgents can send structured requests to the Supervisor without needing
// create_task or assign_task privileges.
//
// Use cases:
//   - decompose_audit:  Worker completed an audit and discovered follow-up tasks
//   - reassign:         Task was mis-assigned, needs different worker
//   - needs_info:       Worker needs clarification before proceeding
//   - report_issue:     Task encountered a blocking issue (dependency, bug, etc.)
//   - request_review:   Worker believes task is done but wants QC to verify early

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::{CreateWorkerRequestInput, TasksDb};
use crate::events::{emit_worker_request_global, WorkerRequestPayload};
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct RequestSupervisorActionTool;

#[async_trait]
impl Tool for RequestSupervisorActionTool {
    fn name(&self) -> &str { "request_supervisor_action" }

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

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let request_type = match input["request_type"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'request_type' is required"),
        };

        let valid_types = ["decompose_audit", "reassign", "needs_info", "report_issue", "request_review"];
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

        // Infer worker_slug from the agent role in context, or fail gracefully.
        let worker_slug = ctx.agent_role.as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

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
        emit_worker_request_global(WorkerRequestPayload {
            id: request.id,
            worker_slug: request.worker_slug.clone(),
            task_id: request.task_id,
            request_type: request.request_type.clone(),
            status: request.status.as_str().to_string(),
            payload: request.payload.clone(),
        });

        ToolResult::success(json!({
            "request_id": request.id,
            "status": "pending",
            "message": format!(
                "✅ Worker request #{} sent to Supervisor (type: {}). \
                 The Supervisor will process it on its next tick.",
                request.id, request_type
            )
        }).to_string())
    }
}
