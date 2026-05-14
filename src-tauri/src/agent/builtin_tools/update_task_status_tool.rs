// update_task_status_tool — Native tool for recipe agents to update task board state.
// Used by the Supervisor recipe to assign tasks, mark in_progress, escalate failures, etc.
// Available in FastDevelopment mode — recipe agents run in this mode.
// Eliminates the need for Python scripts to manipulate tasks.db directly.

use async_trait::async_trait;
use serde_json::{json, Value};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};
use crate::recipes::defaults::FLEET_WORKERS;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct UpdateTaskStatusTool;

#[async_trait]
impl Tool for UpdateTaskStatusTool {
    fn name(&self) -> &str { "update_task_status" }

    fn description(&self) -> &str {
        "Update the status and/or assigned worker for a task on the AutoDev board. \
        Use this to: assign a task to a worker (status='assigned', assigned_worker='worker-backend'), \
        mark as in_progress, escalate to failed, report build failures, or mark as done. \
        Valid statuses: ready, assigned, in_progress, qc_review, done, failed, build_failed, cancelled, rejected. \
        Use rejected when QC finds issues — provide a rejection_reason in that field so the worker knows what to fix."
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
                    "enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "build_failed", "cancelled", "rejected"],
                    "description": "New status for the task"
                },
                "assigned_worker": {
                    "type": "string",
                    "description": "Worker slug to assign (e.g. 'worker-backend', 'worker-frontend'). Pass null to unassign."
                },
                "rejection_reason": {
                    "type": "string",
                    "description": "Required when status='rejected'. Describe exactly what the QC agent found wrong so the worker can fix it."
                }
            },
            "required": ["id", "status"]
        })
    }

    fn is_read_only(&self) -> bool { false }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
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
        let rejection_reason = input["rejection_reason"].as_str().map(|s| s.to_string());

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
        // Validate the worker slug is a known fleet worker when explicitly set.
        // Catches typos like 'backend-worker' before they silently corrupt the board.
        if let Some(ref w) = assigned_worker {
            if !FLEET_WORKERS.contains(&w.as_str()) {
                return ToolResult::error(&format!(
                    "Unknown worker slug '{w}'. Valid workers: {}. \
                     Check for typos — worker slugs use the format 'worker-<domain>'.",
                    FLEET_WORKERS.join(", ")
                ));
            }
        }
        if status == "rejected" {
            let reason = rejection_reason.as_deref().unwrap_or("").trim();
            if reason.is_empty() {
                return ToolResult::error(
                    "status='rejected' requires a non-empty 'rejection_reason'. \
                     Describe what failed so the worker knows what to fix."
                );
            }
        }

        // Doc 09 T2.4 — gate qc_review transitions on recent build evidence.
        //
        // The 14 May calendar run shipped 16 integration defects to qc_review
        // because no worker actually ran `npm run build`. The bash tool now
        // records every recognised build/test/smoke invocation in a per-
        // session ring (`build_evidence::record`). Here we reject the
        // transition if the calling session has produced no entry, or if the
        // most recent recorded build failed.
        //
        // Exemptions:
        //   • QC reviewer subagents (parent_slug = qc-dispatcher) can move
        //     tasks to done/rejected freely — they don't write code.
        //   • Other agents reading qc_review via supervisor reconciliation
        //     touch task status with non-qc_review targets only.
        //   • Tests / unit code may call update_task_status outside a session
        //     where bash isn't available. We allow when the agent_role is
        //     None (raw tool invocation) so non-fleet callers aren't blocked.
        if status == "qc_review" && ctx.agent_role.is_some() {
            // Skip the gate for QC reviewers — they never run a build, they
            // just review someone else's. The qc-reviewer-* slugs all start
            // with a stable prefix; match on that.
            let role = ctx.agent_role.as_deref().unwrap_or("");
            let is_qc_reviewer = role.starts_with("qc-")
                || role.starts_with("subagent-qc-dispatcher");
            if !is_qc_reviewer {
                match super::build_evidence::most_recent_build(&ctx.session_id) {
                    None => return ToolResult::error(
                        "Cannot move task to qc_review without recorded build evidence. \
                         Run your recipe's Build Validation step in the SAME tick as this \
                         status update — e.g. `bash('npm run build 2>&1 | tee build.log; \
                         echo \"EXIT:$?\"')` for Node, `bash('cargo test 2>&1 | tee \
                         build.log; echo \"EXIT:$?\"')` for Rust, or `bash('uv run pytest \
                         2>&1 | tee build.log; echo \"EXIT:$?\"')` for Python. The \
                         supervisor / qc-dispatcher requires proof the artifact builds \
                         before any review can begin."
                    ),
                    Some(ev) if ev.exit_code != 0 => return ToolResult::error(&format!(
                        "Cannot move task to qc_review — the most recent build invocation \
                         exited with code {}. Transition to status='build_failed' instead \
                         (the worker recipe's failure branch). Most recent build: '{}'.",
                        ev.exit_code,
                        ev.command,
                    )),
                    Some(_) => { /* ok — proceed */ }
                }
            }
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

        if let Err(e) = db.update_status_with_reason(
            id, &status, assigned_worker.as_deref(), rejection_reason.as_deref()
        ) {
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
            // Wake the originally assigned worker immediately so they see the rejection
            // and can begin rework without waiting for the next polling cycle.
            // Use the input assigned_worker first; fall back to the pre-existing DB value.
            // This handles QC rejections where the caller omits assigned_worker.
            "rejected"  => {
                let worker_to_kick = assigned_worker.as_deref()
                    .or_else(|| task.assigned_worker.as_deref());
                if let Some(w) = worker_to_kick {
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

        // BUG-004 — QC decision toast event.
        // When a task moves from qc_review → done / rejected, the QC agent has
        // rendered its verdict. Surface it as a toast so the user sees progress.
        if matches!(task.status.as_str(), "qc_review") && matches!(status.as_str(), "done" | "rejected") {
            crate::events::emit_qc_decision_global(crate::events::QcDecisionPayload {
                task_id:          id,
                task_title:       task.title.clone(),
                decision:         status.clone(),
                rejection_reason: rejection_reason.clone(),
                worker_slug:      assigned_worker.clone().unwrap_or_else(|| "qc-agent".into()),
            });
        }

        // Fire fleet hooks for task lifecycle events.
        // K.8 — added build_failed (reuses OnTaskFail) and rejected
        // (reuses OnQcReviewStart) so telemetry and hook consumers observe
        // these transitions. Previously they fired no hooks.
        let hook_type = match status.as_str() {
            "assigned"     => Some(HookType::OnTaskClaim),
            "done"         => Some(HookType::OnTaskComplete),
            "failed"       => Some(HookType::OnTaskFail),
            "build_failed" => Some(HookType::OnTaskFail),      // K.8
            "qc_review"    => Some(HookType::OnQcReviewStart),
            "rejected"     => Some(HookType::OnQcReviewStart), // K.8
            _ => None,
        };

        if let Some(ht) = hook_type {
            let ctx = HookContext {
                hook_type: ht.as_str().to_string(),
                timestamp: chrono::Utc::now(),
                session_id: Some(ctx.session_id.clone()),
                agent_mode: Some("Worker".to_string()),
                tool_call: Some(ToolCallContext {
                    id: ctx.call_id.clone(),
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
            "task_id":          id,
            "title":            task.title,
            "previous_status":  task.status.as_str(),
            "new_status":       status,
            "assigned_worker":  assigned_worker,
            "rejection_reason": rejection_reason,
            "message":          format!("✅ Task #{id} updated: status={status}")
        }).to_string())
    }
}
