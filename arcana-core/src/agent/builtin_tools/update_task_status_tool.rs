// update_task_status_tool — Native tool for recipe agents to update task board state.
// Used by the Supervisor recipe for assignments/recovery and by workers for results.
// Entering in_progress requires the owning worker's atomic claim_task tool.
// Available in FastDevelopment mode — recipe agents run in this mode.
// Eliminates the need for Python scripts to manipulate tasks.db directly.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::warn;

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
        escalate to failed, report build failures, or mark as done. \
        To enter in_progress, the assigned worker must use claim_task; this tool cannot create a claim. \
        Valid statuses: ready, assigned, in_progress, qc_review, done, failed, build_failed, cancelled, rejected, needs_info. \
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
                    "enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "build_failed", "cancelled", "rejected", "needs_info"],
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
                 ten this tool's schema declares; check spelling and case."
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

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };
        let task = match db.get_by_id(id) {
            Ok(Some(task)) => task,
            Ok(None) => return ToolResult::error(&format!("Task #{id} not found")),
            Err(e) => return ToolResult::error(&format!("Failed to fetch task: {e}")),
        };

        let role = ctx.agent_role.trim();
        if role == "supervisor" {
            // The Supervisor owns fleet-wide lifecycle and assignment changes.
        } else if is_qc_reviewer(role) {
            let qc_transition = matches!(
                status.as_str(),
                "done" | "rejected" | "build_failed" | "failed"
            );
            if task.status.as_str() != "qc_review" || !qc_transition {
                return ToolResult::error(
                    "QC agents may only resolve a task currently in qc_review; ask the Supervisor to change other task state",
                );
            }
            if assigned_worker.as_deref() != task.assigned_worker.as_deref() {
                return ToolResult::error(
                    "QC agents cannot change task ownership; preserve the assigned_worker value",
                );
            }
        } else {
            if ctx.task_id != Some(id)
                || task.status.as_str() != "in_progress"
                || task.assigned_worker.as_deref() != Some(role)
            {
                return ToolResult::error(
                    "Workers may update only their own currently claimed in_progress task; ask the Supervisor to route other work",
                );
            }
            if !matches!(
                status.as_str(),
                "done" | "failed" | "build_failed" | "qc_review"
            ) {
                return ToolResult::error(
                    "Workers may only report completion, failure, build failure, or request QC for their own task; ask the Supervisor to requeue, reassign, cancel, or change other lifecycle state",
                );
            }
        }

        // P4-12…14 — gate qc_review transitions on persisted task build evidence.
        //
        // The 14 May calendar run shipped 16 integration defects to qc_review
        // because no worker actually ran `npm run build`. The Rust-owned
        // project_command tool stores a completed process result against the worker's unique in-progress
        // task. Here we reject the transition unless that task's latest build
        // has an attributable, passing exit code. Evidence is intentionally
        // persisted across app restarts and scoped by task ID, not session ID.
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
                match super::build_evidence::qc_gate_error_for_workspace(
                    id,
                    std::path::Path::new(&ctx.working_dir),
                ) {
                    Err(e) => {
                        return ToolResult::error(format!(
                            "Cannot verify task #{id} build evidence because the evidence database could not be read: {e}"
                        ))
                    }
                    Ok(Some(message)) => return ToolResult::error(message),
                    Ok(None) => { /* task-specific passing build; proceed */ }
                }
            }
        }

        let routing_outcome = match status.as_str() {
            "done" => task
                .assigned_worker
                .as_deref()
                .map(|worker| (worker, "success")),
            "failed" => task
                .assigned_worker
                .as_deref()
                .map(|worker| (worker, "failure")),
            "assigned" => task
                .assigned_worker
                .as_deref()
                .filter(|previous| Some(*previous) != assigned_worker.as_deref())
                .map(|worker| (worker, "reassigned")),
            _ => None,
        };
        let update_result = if role != "supervisor" && !is_qc_reviewer(role) {
            db.update_status_for_worker_claim_and_run_id(
                id,
                role,
                &status,
                assigned_worker.as_deref(),
                rejection_reason.as_deref(),
                ctx.run_id.as_deref(),
                (status == "qc_review").then_some(std::path::Path::new(&ctx.working_dir)),
            )
        } else if let Some((worker, outcome)) = routing_outcome {
            db.update_status_with_routing_outcome_and_run_id(
                id,
                task.status.as_str(),
                worker,
                &status,
                assigned_worker.as_deref(),
                rejection_reason.as_deref(),
                outcome,
                ctx.run_id.as_deref(),
            )
        } else {
            if status == "qc_review" {
                db.update_status_with_workspace_and_run_id(
                    id,
                    &status,
                    assigned_worker.as_deref(),
                    rejection_reason.as_deref(),
                    ctx.run_id.as_deref(),
                    std::path::Path::new(&ctx.working_dir),
                )
            } else {
                db.update_status_with_reason_and_run_id(
                    id,
                    &status,
                    assigned_worker.as_deref(),
                    rejection_reason.as_deref(),
                    ctx.run_id.as_deref(),
                )
            }
        };
        if let Err(e) = update_result {
            return ToolResult::error(&format!("Failed to update task #{id}: {e}"));
        }

        let unblocked_task_ids: Vec<i64> = if status == "done" {
            match unblock_completed_task(&db, id, || {
                crate::recipes::scheduler::kick("supervisor");
            }) {
                Ok(unblocked_ids) => unblocked_ids,
                Err(e) => {
                    warn!(
                        "[update_task_status] task #{id} is done, but dependent-task reconciliation failed: {e}"
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

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
                "unblocked_task_ids": unblocked_task_ids,
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
    is_qc_reviewer(role) || role == "supervisor"
}

fn is_qc_reviewer(role: &str) -> bool {
    role.starts_with("qc-") || role.starts_with("subagent-qc-dispatcher")
}

fn unblock_completed_task(
    db: &TasksDb,
    completed_task_id: i64,
    dispatch_supervisor: impl FnOnce(),
) -> crate::errors::Result<Vec<i64>> {
    let unblocked_ids = db.unblock_ready_tasks(completed_task_id)?;
    for task_id in &unblocked_ids {
        crate::events::emit_task_unblocked(*task_id);
    }
    if !unblocked_ids.is_empty() {
        // Newly-ready work is routed by the Supervisor immediately. The
        // five-minute scheduled pass remains a recovery path only.
        dispatch_supervisor();
    }
    Ok(unblocked_ids)
}

#[cfg(test)]
mod qc_gate_tests {
    use super::{promotes_on_behalf_of_others, unblock_completed_task, UpdateTaskStatusTool};
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_trait::{Tool, ToolContext};
    use crate::db::tasks::{CreatePhasedTaskInput, CreateTaskInput, TasksDb};
    use serde_json::json;

    fn isolated_tasks_db() -> TasksDb {
        let root = std::env::temp_dir().join(format!(
            "arcana-autonomous-unblock-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).expect("create unique isolated fixture directory");
        TasksDb::open_at(&root.join("tasks.db")).expect("open isolated task database")
    }

    fn create_test_task(db: &TasksDb, title: &str) -> i64 {
        db.create(CreateTaskInput {
            title: title.into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "test".into(),
            created_by: Some("autonomous-unblock-test".into()),
        })
        .expect("create task")
        .id
    }

    #[test]
    fn worker_status_tool_carries_run_id_into_committed_database_event() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let task_id = create_test_task(&db, "run-correlated status tool task");
        db.update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign test task");
        db.claim_task_with_pid(task_id, "worker-backend", 1_122)
            .expect("claim test task");

        let context = ToolContext {
            session_id: "run-correlated-status-test".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "run-correlated-status-call".into(),
            run_id: Some("worker-run-status-tool".into()),
            task_id: Some(task_id),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build isolated test runtime");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(
                UpdateTaskStatusTool.execute(json!({"id": task_id, "status": "done"}), &context),
            )
        });

        assert!(!result.is_error, "status tool failed: {}", result.output);
        assert_eq!(
            db.get_by_id(task_id).unwrap().unwrap().status.as_str(),
            "done"
        );
        let status = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_task_status_transition")
            .expect("task status event from production tool");
        assert_eq!(status["fields"]["task_id"], task_id.to_string());
        assert_eq!(status["fields"]["worker"], "worker-backend");
        assert_eq!(status["fields"]["run_id"], "worker-run-status-tool");
        assert_eq!(status["fields"]["outcome"], "success");
    }

    #[tokio::test]
    async fn worker_cannot_complete_another_workers_claimed_task() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let task_a = create_test_task(&db, "worker A task");
        let task_b = create_test_task(&db, "worker B task");
        db.update_status(task_a, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task_a, "worker-a", 801).unwrap();
        db.update_status(task_b, "assigned", Some("worker-b"))
            .unwrap();
        db.claim_task_with_pid(task_b, "worker-b", 802).unwrap();

        let context = ToolContext {
            session_id: "cross-worker-status-test".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "cross-worker-status-call".into(),
            run_id: None,
            task_id: Some(task_b),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-b".into(),

            run_token_budget: None,
        };
        let result = UpdateTaskStatusTool
            .execute(json!({"id": task_a, "status": "done"}), &context)
            .await;

        assert!(result.is_error);
        assert_eq!(
            db.get_by_id(task_a).unwrap().unwrap().status.as_str(),
            "in_progress",
            "denial must happen before status, routing, or dependency effects"
        );
    }

    /// TG4-30 boundary: a worker cannot release another worker's dependents.
    /// Completing a task is the only worker route into `unblock_ready_tasks`,
    /// and that route is owner-checked; the dependent survives a denied
    /// cross-worker completion untouched.
    #[tokio::test]
    async fn worker_cannot_release_another_workers_dependency_chain() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let task_a = create_test_task(&db, "worker A prerequisite");
        let epic = db
            .create_epic("cross-worker release guard", None, "test")
            .unwrap();
        let dependent_id = db
            .create_phased_task(crate::db::tasks::CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "next".into(),
                phase_order: 1,
                title: "stays blocked when the completion is denied".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "ready only after worker A completes".into(),
                blocked_by: vec![task_a.to_string()],
                milestone: None,
                created_by: Some("tg4-30-test".into()),
            })
            .unwrap()
            .id;
        db.update_status(task_a, "assigned", Some("worker-a"))
            .unwrap();
        db.claim_task_with_pid(task_a, "worker-a", 811).unwrap();
        let task_b = create_test_task(&db, "worker B task");
        db.update_status(task_b, "assigned", Some("worker-b"))
            .unwrap();
        db.claim_task_with_pid(task_b, "worker-b", 812).unwrap();

        let context = ToolContext {
            session_id: "cross-worker-release-test".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "cross-worker-release-call".into(),
            run_id: None,
            task_id: Some(task_b),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-b".into(),

            run_token_budget: None,
        };
        let result = UpdateTaskStatusTool
            .execute(json!({ "id": task_a, "status": "done" }), &context)
            .await;

        assert!(result.is_error, "worker B cannot complete worker A's task");
        assert_eq!(
            db.get_by_id(task_a).unwrap().unwrap().status.as_str(),
            "in_progress",
            "the prerequisite stays claimed and uncompleted"
        );
        assert_eq!(
            db.get_by_id(dependent_id).unwrap().unwrap().status.as_str(),
            "needs_info",
            "the denied completion must not release the dependent"
        );
    }

    #[tokio::test]
    async fn autonomous_completion_closes_only_the_current_owners_routing_row() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let task_id = create_test_task(&db, "routing outcome task");
        let task = db.get_by_id(task_id).unwrap().unwrap();

        let stale = crate::routing::TaskRouter::assignment_entry(&task, "worker-backend");
        db.assign_with_routing_history(task_id, "worker-backend", &stale)
            .expect("insert earlier automatic still-pending attempt");
        let current = crate::routing::TaskRouter::assignment_entry(&task, "worker-backend");
        db.assign_with_routing_history(task_id, "worker-backend", &current)
            .expect("assign current worker with its routing row");
        db.claim_task_with_pid(task_id, "worker-backend", std::process::id())
            .expect("worker claims task");

        let context = ToolContext {
            session_id: "routing-outcome-test".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "routing-outcome-call".into(),
            run_id: None,
            task_id: Some(task_id),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            // Attribution comes from the owner-checked task row, not a
            // caller-supplied or newly assigned worker name.
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let result = UpdateTaskStatusTool
            .execute(json!({ "id": task_id, "status": "done" }), &context)
            .await;
        assert!(
            !result.is_error,
            "tool completion should succeed: {result:?}"
        );

        let history = db.get_routing_history(10).unwrap();
        let stale_row = history.iter().find(|entry| entry.id == stale.id).unwrap();
        let current_row = history.iter().find(|entry| entry.id == current.id).unwrap();
        assert!(
            stale_row.outcome.is_none(),
            "older attempt must remain pending"
        );
        assert_eq!(current_row.worker_slug, "worker-backend");
        assert_eq!(current_row.outcome.as_deref(), Some("success"));
        assert!(current_row.duration_ms.is_some());
        assert_eq!(
            db.get_by_id(task_id).unwrap().unwrap().status.as_str(),
            "done"
        );

        let untracked_id = create_test_task(&db, "missing routing row stays in progress");
        db.update_status(untracked_id, "assigned", Some("worker-backend"))
            .unwrap();
        db.claim_task_with_pid(untracked_id, "worker-backend", std::process::id())
            .unwrap();
        let untracked_completion = UpdateTaskStatusTool
            .execute(json!({ "id": untracked_id, "status": "done" }), &context)
            .await;
        assert!(!untracked_completion.is_error);
        assert_eq!(
            db.get_by_id(untracked_id).unwrap().unwrap().status.as_str(),
            "done",
            "untracked manual assignments remain completable without an affinity row"
        );
    }

    #[tokio::test]
    async fn shipped_status_tool_completion_unblocks_a_dependent_for_supervisor_dispatch() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated task database");
        let blocker = create_test_task(&db, "completed prerequisite through status tool");
        let epic = db
            .create_epic("autonomous dependency epic", None, "test")
            .unwrap();
        let dependent = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "follow-up".into(),
                phase_order: 1,
                title: "becomes claimable after completion".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "ready after its prerequisite completes".into(),
                blocked_by: vec![blocker.to_string()],
                milestone: None,
                created_by: Some("p4-39-test".into()),
            })
            .unwrap()
            .id;
        let budget_held = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "budget-held follow-up".into(),
                phase_order: 2,
                title: "must stay held after prerequisite completion".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "requires explicit budget resolution".into(),
                blocked_by: vec![blocker.to_string()],
                milestone: None,
                created_by: Some("p4-39-test".into()),
            })
            .unwrap()
            .id;
        db.mark_task_budget_exceeded(budget_held, "saved task budget was reached")
            .expect("persist the explicit budget hold");
        db.update_status(blocker, "assigned", Some("worker-backend"))
            .unwrap();
        db.claim_task_with_pid(blocker, "worker-backend", 902)
            .unwrap();

        let context = ToolContext {
            session_id: "p4-39-status-tool".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "p4-39-complete-prerequisite".into(),
            run_id: None,
            task_id: Some(blocker),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = UpdateTaskStatusTool
            .execute(json!({"id": blocker, "status": "done"}), &context)
            .await;
        assert!(!result.is_error, "status tool failed: {}", result.output);
        let response: serde_json::Value =
            serde_json::from_str(&result.output).expect("status tool response JSON");
        assert_eq!(response["unblocked_task_ids"][0], dependent);
        assert_eq!(response["unblocked_task_ids"].as_array().unwrap().len(), 1);
        assert_eq!(
            db.get_by_id(dependent).unwrap().unwrap().status.as_str(),
            "ready"
        );
        assert!(db
            .list(Some("ready"))
            .unwrap()
            .iter()
            .any(|task| task.id == dependent && task.assigned_worker.is_none()));
        let held = db.get_by_id(budget_held).unwrap().unwrap();
        assert_eq!(held.status.as_str(), "needs_info");
        assert!(held.budget_exceeded);
        assert_eq!(
            held.budget_exceeded_reason.as_deref(),
            Some("saved task budget was reached")
        );
    }

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

    #[test]
    fn autonomous_completion_unblocks_and_wakes_supervisor() {
        let db = isolated_tasks_db();
        let blocker = create_test_task(&db, "completed prerequisite");
        let epic = db.create_epic("dependency epic", None, "test").unwrap();
        let dependent = db
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "core".into(),
                phase_order: 1,
                title: "dependent task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "test".into(),
                blocked_by: vec![blocker.to_string()],
                milestone: None,
                created_by: Some("autonomous-unblock-test".into()),
            })
            .unwrap()
            .id;
        db.update_status(blocker, "assigned", Some("worker-backend"))
            .unwrap();
        db.claim_task_with_pid(blocker, "worker-backend", 902)
            .unwrap();
        db.update_status(blocker, "done", Some("worker-backend"))
            .unwrap();

        let mut supervisor_woken = false;
        let unblocked = unblock_completed_task(&db, blocker, || supervisor_woken = true).unwrap();

        assert_eq!(unblocked, vec![dependent]);
        assert!(supervisor_woken, "newly-ready work must wake Supervisor");
        assert_eq!(
            db.get_by_id(dependent).unwrap().unwrap().status.as_str(),
            "ready"
        );
    }
}
