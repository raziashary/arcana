// update_task_status_tool — Native tool for recipe agents to update task board state.
// Used by the Supervisor recipe to assign tasks, mark in_progress, escalate failures, etc.
// Available in FastDevelopment mode — recipe agents run in this mode.
// Eliminates the need for Python scripts to manipulate tasks.db directly.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct UpdateTaskStatusTool;

#[async_trait]
impl Tool for UpdateTaskStatusTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "update_task_status"
    }

    fn description(&self) -> &str {
        "Update the status and/or assigned worker for a task on the Arcana board. \
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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };
        let status = match input["status"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return ToolResult::error("'status' is required"),
        };
        // 🔴 T-36 — validate against the enum this tool's own schema declares.
        //
        // `status` was accepted as any non-empty string and
        // `update_status_with_reason` interpolates it straight into the UPDATE,
        // so a typo like `"in-progress"` (hyphen) or `"Done"` wrote a value no
        // reader matches. The row then sits in a state the scheduler, the QC
        // queue and the board all skip: not an error, just permanently
        // invisible. The schema advertised nine values and nothing checked them.
        //
        // ⚠ `TaskStatus::from_str` is the authority, not a list retyped here —
        // a second list is the thing `L-19` is about ("two lists that must
        // agree will disagree, and the second is always invisible").
        // The task parser is fallible at every boundary; an unknown value
        // must never silently become `ready` in either the tool or DB reader.
        if crate::db::tasks::TaskStatus::from_str(&status).is_err() {
            return ToolResult::error(format!(
                "'{status}' is not a task status. The recognised values are the \
                 nine this tool's schema declares; check spelling and case."
            ));
        }
        // P8-29 — null or absent → UNASSIGN (clears assigned_worker and the
        // claim fields); string → set assigned_worker. This comment used to say
        // "leave unassigned", which contradicted the tool description twenty
        // lines above it ("Pass null to unassign") and matched the SQL rather
        // than the promise. The SQL is what changed.
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
                 it atomically sets status='assigned' AND assigned_worker.",
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
            let registered_workers = match crate::db::workers::WorkersDb::open()
                .and_then(|db| db.registered_slugs())
            {
                Ok(workers) => workers,
                Err(e) => return ToolResult::error(&format!("Failed to read worker roster: {e}")),
            };
            if !registered_workers.iter().any(|slug| slug == w) {
                return ToolResult::error(&format!(
                    "Unknown worker slug '{w}'. Registered workers: {}. \
                     Check for typos — worker slugs use the format 'worker-<domain>'.",
                    registered_workers.join(", ")
                ));
            }
        }
        if status == "rejected" {
            let reason = rejection_reason.as_deref().unwrap_or("").trim();
            if reason.is_empty() {
                return ToolResult::error(
                    "status='rejected' requires a non-empty 'rejection_reason'. \
                     Describe what failed so the worker knows what to fix.",
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
        //   • This gate used to be skipped entirely whenever agent_role was
        //     None — which, on the in-process path, was ALWAYS, because all
        //     three ToolContext construction sites hardcoded None. The gate
        //     could therefore never fire outside the MCP path. Identity is now
        //     always present, so it applies to every caller except QC
        //     reviewers (ADR-LOG-RESTRUCT-0007).
        if status == "qc_review" {
            // Skip the gate for callers who promote someone ELSE's work and
            // never run a build themselves:
            //   • QC reviewers (`qc-*`, `subagent-qc-dispatcher*`) — they review
            //     a build someone else ran.
            //   • the supervisor — `recipes/defaults.rs` `request_review` has it
            //     call `update_task_status(id, 'qc_review')` on a worker's
            //     behalf after that worker asked for review.
            //
            // The supervisor exemption was missed when this gate was widened in
            // P0: identity became always-present, so the gate started applying
            // to every caller, and `request_review` began failing every time.
            // Found by the P0 post-phase audit (F-22).
            //
            // The gate's purpose is intact: it stops a WORKER claiming
            // qc_review for work it never built.
            if !promotes_on_behalf_of_others(ctx.agent_role.as_str()) {
                match super::build_evidence::most_recent_build(&ctx.session_id) {
                    None => {
                        return ToolResult::error(
                            "Cannot move task to qc_review without recorded build \
                             evidence. Run your build in the SAME tick as this status \
                             update, as a PLAIN command so the exit status is the \
                             build's own: bash('npm run build') for Node, \
                             bash('cargo test --all-targets') for Rust, bash('uv run \
                             pytest') for Python. Do NOT pipe into tee or append \
                             `; echo EXIT:$?` — a shell reports the LAST element's \
                             status, so the build's own result is discarded and cannot \
                             serve as evidence.",
                        )
                    }
                    Some(ev) if !ev.exit_code_attributable => {
                        return ToolResult::error(&format!(
                            "Cannot move task to qc_review — the most recent build ran \
                             as a pipeline or command sequence, so its exit status ({}) \
                             belongs to the last element, not to the build. Re-run the \
                             build as a plain command. Recorded: '{}'.",
                            ev.exit_code, ev.command,
                        ))
                    }
                    Some(ev) if ev.exit_code != 0 => {
                        return ToolResult::error(&format!(
                            "Cannot move task to qc_review — the most recent build \
                             invocation exited with code {}. Transition to \
                             status='build_failed' instead (the worker recipe's failure \
                             branch). Most recent build: '{}'.",
                            ev.exit_code, ev.command,
                        ))
                    }
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
            Ok(None) => return ToolResult::error(&format!("Task #{id} not found")),
            Err(e) => return ToolResult::error(&format!("Failed to fetch task: {e}")),
        };

        if let Err(e) = db.update_status_with_reason(
            id,
            &status,
            assigned_worker.as_deref(),
            rejection_reason.as_deref(),
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
            "assigned" => {
                if let Some(ref w) = assigned_worker {
                    crate::recipes::scheduler::kick(w);
                }
            }
            // Wake the originally assigned worker immediately so they see the rejection
            // and can begin rework without waiting for the next polling cycle.
            // Use the input assigned_worker first; fall back to the pre-existing DB value.
            // This handles QC rejections where the caller omits assigned_worker.
            "rejected" => {
                let worker_to_kick = assigned_worker
                    .as_deref()
                    .or_else(|| task.assigned_worker.as_deref());
                if let Some(w) = worker_to_kick {
                    crate::recipes::scheduler::kick(w);
                }
            }
            _ => {}
        }

        // Phase 3.0 R1 — task-board push event for the UI.
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(id),
            kind: "status".into(),
            status: Some(status.clone()),
            assigned_worker: assigned_worker.clone(),
        });

        // BUG-004 — QC decision toast event.
        // When a task moves from qc_review → done / rejected, the QC agent has
        // rendered its verdict. Surface it as a toast so the user sees progress.
        if matches!(task.status.as_str(), "qc_review")
            && matches!(status.as_str(), "done" | "rejected")
        {
            crate::events::emit_qc_decision(crate::events::QcDecisionPayload {
                task_id: id,
                task_title: task.title.clone(),
                decision: status.clone(),
                rejection_reason: rejection_reason.clone(),
                worker_slug: assigned_worker.clone().unwrap_or_else(|| "qc-agent".into()),
            });
        }

        // Fire fleet hooks for task lifecycle events.
        // K.8 — added build_failed (reuses OnTaskFail) and rejected
        // (reuses OnQcReviewStart) so telemetry and hook consumers observe
        // these transitions. Previously they fired no hooks.
        let hook_type = match status.as_str() {
            "assigned" => Some(HookType::OnTaskClaim),
            "done" => Some(HookType::OnTaskComplete),
            "failed" => Some(HookType::OnTaskFail),
            "build_failed" => Some(HookType::OnTaskFail), // K.8
            "qc_review" => Some(HookType::OnQcReviewStart),
            "rejected" => Some(HookType::OnQcReviewStart), // K.8
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

        ToolResult::success(
            json!({
                "task_id":          id,
                "title":            task.title,
                "previous_status":  task.status.as_str(),
                "new_status":       status,
                "assigned_worker":  assigned_worker,
                "rejection_reason": rejection_reason,
                "message":          format!("✅ Task #{id} updated: status={status}")
            })
            .to_string(),
        )
    }
}

/// Roles that may move a task to `qc_review` without build evidence of their
/// own, because they promote work someone else built.
pub(crate) fn promotes_on_behalf_of_others(role: &str) -> bool {
    role.starts_with("qc-") || role.starts_with("subagent-qc-dispatcher") || role == "supervisor"
}

#[cfg(test)]
mod qc_gate_tests {
    use super::promotes_on_behalf_of_others;

    /// Regression for the P0 post-phase audit finding F-22.
    ///
    /// Widening the build-evidence gate to every caller (identity became
    /// always-present, so `agent_role.is_some()` stopped being a filter) broke
    /// the shipped supervisor recipe: `recipes/defaults.rs` `request_review`
    /// calls `update_task_status(id, 'qc_review')` on a worker's behalf, and
    /// the supervisor never runs a build, so the transition failed every time.
    #[test]
    fn the_supervisor_may_promote_work_it_did_not_build() {
        assert!(promotes_on_behalf_of_others("supervisor"));
    }

    #[test]
    fn qc_reviewers_are_exempt() {
        assert!(promotes_on_behalf_of_others("qc-agent"));
        assert!(promotes_on_behalf_of_others("qc-reviewer-3"));
        assert!(promotes_on_behalf_of_others("subagent-qc-dispatcher-1"));
    }

    /// The gate's whole purpose: a worker must not claim qc_review for work it
    /// never built. If this ever passes, the exemption has swallowed the rule.
    #[test]
    fn workers_are_not_exempt() {
        for role in [
            "worker-backend",
            "worker-frontend",
            "worker-platform",
            "worker-cleanup",
            "fleet-smoke",
            "",
        ] {
            assert!(
                !promotes_on_behalf_of_others(role),
                "{role} must still need build evidence to reach qc_review"
            );
        }
    }
}
