// assign_tasks (batch) — Multi-assignment in a single tool call.
//
// Motivation:
//   The supervisor's dispatch tick was bottlenecked on LLM round-trips: for N
//   ready tasks, it had to make N × 2 tool calls (suggest_worker + assign_task).
//   With 11 tasks and ~5s per round-trip, a single tick burned ~110s, during
//   which workers started competing for orch's event loop and the entire
//   fleet would occasionally fall back to Mode 1 because the /health endpoint
//   got starved.
//
//   This tool lets the supervisor dispatch the WHOLE batch in one round-trip.
//   We deduplicate worker kicks so a 5-task assignment to worker-backend
//   doesn't fire 5 wake-ups; one kick is enough. We also collapse routing
//   history writes into the same loop. End-to-end the supervisor goes from
//   N × 2 LLM calls down to 1 — typically a 10× speedup on busy ticks.
//
// Safety:
//   Each individual assignment is validated independently (dispatchable
//   state, valid worker slug). One bad entry doesn't fail the whole batch —
//   we return a per-task result list so the LLM can see exactly what
//   succeeded and what didn't, and decide whether to retry or escalate.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashSet;

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;

pub struct AssignTasksBatchTool;

#[async_trait]
impl Tool for AssignTasksBatchTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "assign_tasks"
    }

    fn description(&self) -> &str {
        "Atomically assign MANY tasks to workers in a single tool call. Use this \
         INSTEAD OF calling assign_task once per task when dispatching a \
         decomposed epic or a post-rejection retry queue. \
         \
         Each assignment in the batch is validated independently; omit \
         worker_slug for deterministic Rust routing, or provide an explicit \
         runnable worker override with a non-empty override_reason. Bad entries \
         (wrong worker slug, task not in a dispatchable state, missing task) are \
         reported in the per-task result list but don't fail the others. After \
         all assignments are written, each affected worker is kicked once \
         (deduped) so a 5-task assignment to worker-backend doesn't fire 5 \
         redundant wake-ups — which is the actual saving: one round trip and \
         one wake-up per worker instead of N of each."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "assignments": {
                    "type": "array",
                    "description": "List of task IDs to dispatch. Arcana selects workers unless an audited override is supplied.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "task_id":     { "type": "integer", "description": "Task ID, must be in dispatchable state (ready/assigned/failed/rejected)." },
                            "worker_slug": {
                                "type": "string",
                                "description": "Optional explicit runnable implementation worker override. Requires override_reason; omit for automatic routing."
                            },
                            "override_reason": {
                                "type": "string",
                                "description": "Required for explicit worker overrides; persisted for audit and excluded from routing learning."
                            }
                        },
                        "required": ["task_id"]
                    },
                    "minItems": 1
                }
            },
            "required": ["assignments"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        if ctx.agent_role.trim() != "supervisor" {
            return ToolResult::error("Only the Supervisor may assign or reassign fleet tasks");
        }
        let assignments = match input.get("assignments").and_then(|v| v.as_array()) {
            Some(a) if !a.is_empty() => a.clone(),
            _ => return ToolResult::error(
                "'assignments' is required and must be a non-empty array of task dispatch requests",
            ),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let workers = match crate::db::workers::WorkersDb::open().and_then(|db| db.list()) {
            Ok(workers) => workers,
            Err(e) => return ToolResult::error(&format!("Failed to read worker roster: {e}")),
        };

        let mut results: Vec<Value> = Vec::with_capacity(assignments.len());
        let mut kicked: HashSet<String> = HashSet::new();
        let mut success_count = 0usize;
        let mut failure_count = 0usize;

        for (idx, entry) in assignments.iter().enumerate() {
            let task_id = entry.get("task_id").and_then(|v| v.as_i64());
            let worker = entry
                .get("worker_slug")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string());
            let override_reason = entry
                .get("override_reason")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|reason| !reason.is_empty());

            // Per-entry validation — failures recorded but don't abort the batch.
            let id = match task_id {
                Some(id) if id > 0 => id,
                _ => {
                    failure_count += 1;
                    results.push(json!({
                        "index":   idx,
                        "ok":      false,
                        "error":   "entry must have a positive integer task_id",
                    }));
                    continue;
                }
            };

            // Load task to verify dispatchable state. Re-using assign_task's
            // safety check: only ready / assigned / failed / rejected can be
            // (re-)dispatched. Anything else (in_progress, qc_review, done)
            // is refused — the supervisor must not kick active work.
            let task = match db.get_by_id(id) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    failure_count += 1;
                    results.push(json!({
                        "index":       idx,
                        "task_id":     id,
                        "worker_slug": worker.as_deref(),
                        "ok":          false,
                        "error":       format!("Task #{id} not found"),
                    }));
                    continue;
                }
                Err(e) => {
                    failure_count += 1;
                    results.push(json!({
                        "index":       idx,
                        "task_id":     id,
                        "worker_slug": worker.as_deref(),
                        "ok":          false,
                        "error":       format!("Failed to fetch task #{id}: {e}"),
                    }));
                    continue;
                }
            };

            let prev_status = task.status.as_str().to_string();
            if task.task_type.as_str() == "epic" {
                failure_count += 1;
                results.push(json!({
                    "index": idx,
                    "task_id": id,
                    "worker_slug": worker.as_deref(),
                    "previous_status": prev_status,
                    "ok": false,
                    "error": format!("Task #{id} is an epic and cannot be assigned to a worker."),
                }));
                continue;
            }
            if !matches!(
                prev_status.as_str(),
                "ready" | "assigned" | "failed" | "rejected"
            ) {
                failure_count += 1;
                results.push(json!({
                    "index":           idx,
                    "task_id":         id,
                    "worker_slug":     worker.as_deref(),
                    "previous_status": prev_status,
                    "ok":              false,
                    "error":           format!(
                        "Task #{id} is in status='{prev_status}' and cannot be (re)assigned. \
                         Only 'ready', 'assigned', 'failed', or 'rejected' tasks may be assigned."
                    ),
                }));
                continue;
            }

            let (worker_slug, override_reason) = match worker.as_deref() {
                Some(slug) if !slug.is_empty() => {
                    let Some(reason) = override_reason else {
                        failure_count += 1;
                        results.push(json!({
                            "index": idx,
                            "task_id": id,
                            "worker_slug": slug,
                            "ok": false,
                            "error": "an explicit worker override requires a non-empty override_reason",
                        }));
                        continue;
                    };
                    let Some(candidate) = workers.iter().find(|candidate| candidate.slug == slug)
                    else {
                        failure_count += 1;
                        results.push(json!({
                            "index": idx,
                            "task_id": id,
                            "worker_slug": slug,
                            "ok": false,
                            "error": format!("'{slug}' is not a registered worker"),
                        }));
                        continue;
                    };
                    if !crate::routing::TaskRouter::is_runnable_implementation_worker(candidate) {
                        failure_count += 1;
                        results.push(json!({
                            "index": idx,
                            "task_id": id,
                            "worker_slug": slug,
                            "ok": false,
                            "error": format!("'{slug}' is not a started, runnable implementation worker"),
                        }));
                        continue;
                    }
                    (slug.to_string(), Some(reason.to_string()))
                }
                Some(_) => {
                    failure_count += 1;
                    results.push(json!({"index": idx, "task_id": id, "ok": false, "error": "worker_slug cannot be empty"}));
                    continue;
                }
                None if override_reason.is_some() => {
                    failure_count += 1;
                    results.push(json!({"index": idx, "task_id": id, "ok": false, "error": "override_reason requires an explicit worker_slug"}));
                    continue;
                }
                None => match crate::routing::TaskRouter::select_worker_for_dispatch(&task) {
                    Ok(selected) => (selected.worker_slug, None),
                    Err(error) => {
                        failure_count += 1;
                        results.push(json!({"index": idx, "task_id": id, "ok": false, "error": format!("automatic routing failed: {error}")}));
                        continue;
                    }
                },
            };

            let routing_entry = crate::routing::TaskRouter::assignment_entry(&task, &worker_slug);
            let assignment = match override_reason.as_deref() {
                Some(reason) => {
                    db.assign_with_routing_override(id, &worker_slug, &routing_entry, reason)
                }
                None => db.assign_with_routing_history(id, &worker_slug, &routing_entry),
            };
            if let Err(e) = assignment {
                failure_count += 1;
                results.push(json!({
                    "index":       idx,
                    "task_id":     id,
                    "worker_slug": worker_slug,
                    "ok":          false,
                    "error":       format!("Failed to assign: {e}"),
                }));
                continue;
            }

            // UI push event per assignment (same shape as single assign_task).
            crate::events::emit_task_changed(crate::events::TaskChangedPayload {
                task_id: Some(id),
                kind: "assigned".into(),
                status: Some("assigned".into()),
                assigned_worker: Some(worker_slug.clone()),
            });

            kicked.insert(worker_slug.clone());
            success_count += 1;
            results.push(json!({
                "index":           idx,
                "task_id":         id,
                "title":           task.title,
                "previous_status": prev_status,
                "new_status":      "assigned",
                "worker_slug":     worker_slug,
                "ok":              true,
            }));
        }

        // One kick per unique worker, even if we assigned 8 tasks to it.
        // Push-based dispatch is per-worker; multiple kicks just trigger the
        // "already working" skip log so they're wasted noise.
        for slug in &kicked {
            crate::recipes::scheduler::kick(slug);
        }

        let summary = format!(
            "Batch assigned {success}/{total} task(s) across {worker_count} worker(s).{failed_suffix}",
            success      = success_count,
            total        = assignments.len(),
            worker_count = kicked.len(),
            failed_suffix = if failure_count > 0 {
                format!(" {failure_count} failed — see per-task results.")
            } else {
                String::new()
            },
        );

        ToolResult::success_with_hint(
            json!({
                "summary":         summary,
                "success_count":   success_count,
                "failure_count":   failure_count,
                "workers_kicked":  kicked.iter().cloned().collect::<Vec<_>>(),
                "results":         results,
            })
            .to_string(),
            summary,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_trait::ToolContext;
    use crate::db::tasks::{CreateTaskInput, TaskStatus};

    #[tokio::test]
    async fn worker_cannot_reassign_through_batch_tool() {
        let context = ToolContext {
            session_id: "batch-assignment-security-test".into(),
            working_dir: std::env::temp_dir().to_string_lossy().into_owned(),
            call_id: "worker-batch-assignment-call".into(),
            run_id: None,
            task_id: Some(1),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = AssignTasksBatchTool
            .execute(json!({"assignments": []}), &context)
            .await;
        assert!(result.is_error);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        assert!(output["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Only the Supervisor"));
    }

    fn create_task(db: &TasksDb, title: &str) -> crate::db::tasks::Task {
        db.create(CreateTaskInput {
            title: title.into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "task is assigned correctly".into(),
            created_by: Some("test".into()),
        })
        .expect("create task")
    }

    #[tokio::test]
    async fn loaded_worker_failure_is_reported_and_later_batch_entry_succeeds() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana data root");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated task database");
        let workers = crate::db::workers::WorkersDb::open().expect("seed workers");
        for slug in ["worker-backend", "worker-frontend"] {
            let worker = workers
                .get_by_slug(slug)
                .expect("read worker")
                .expect("seeded worker exists");
            workers
                .update_status(worker.id, crate::db::workers::WorkerStatus::Idle)
                .expect("make fixture worker runnable");
        }
        let active = create_task(&db, "already loaded task");
        let refused = create_task(&db, "entry for busy worker");
        let accepted = create_task(&db, "entry for free worker");

        db.update_status(active.id, "assigned", Some("worker-backend"))
            .expect("assign loaded worker's existing task");
        db.claim_task_with_pid(active.id, "worker-backend", 901)
            .expect("load worker's existing task");

        let context = ToolContext {
            session_id: "batch-test-session".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "batch-test-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let result = AssignTasksBatchTool
            .execute(
                json!({
                    "assignments": [
                        {"task_id": refused.id, "worker_slug": "worker-backend", "override_reason": "backend task"},
                        {"task_id": accepted.id, "worker_slug": "worker-frontend", "override_reason": "frontend task"}
                    ]
                }),
                &context,
            )
            .await;

        assert!(
            !result.is_error,
            "partial entry refusal must not fail the tool"
        );
        let output: Value = serde_json::from_str(&result.output).expect("parse batch response");
        assert_eq!(output["success_count"], 1);
        assert_eq!(output["failure_count"], 1);
        assert_eq!(output["results"][0]["ok"], false);
        assert!(output["results"][0]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("in-progress task"));
        assert_eq!(output["results"][1]["ok"], true);
        assert_eq!(output["workers_kicked"].as_array().unwrap().len(), 1);
        assert_eq!(output["workers_kicked"][0], "worker-frontend");

        let refused_row = db.get_by_id(refused.id).unwrap().unwrap();
        assert_eq!(refused_row.status, TaskStatus::Ready);
        assert!(refused_row.assigned_worker.is_none());
        let accepted_row = db.get_by_id(accepted.id).unwrap().unwrap();
        assert_eq!(accepted_row.status, TaskStatus::Assigned);
        assert_eq!(
            accepted_row.assigned_worker.as_deref(),
            Some("worker-frontend")
        );
    }

    #[tokio::test]
    async fn automatic_batch_dispatch_uses_recorded_outcomes_without_model_worker_choice() {
        let sandbox = tempfile::tempdir().expect("create isolated Arcana data root");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated task database");
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        for slug in ["worker-backend", "worker-frontend"] {
            let worker = workers
                .get_by_slug(slug)
                .expect("read worker")
                .expect("seeded worker exists");
            workers
                .update_status(worker.id, crate::db::workers::WorkerStatus::Idle)
                .expect("make fixture worker runnable");
        }

        for (slug, outcome) in [
            ("worker-backend", "failure"),
            ("worker-backend", "failure"),
            ("worker-backend", "failure"),
            ("worker-frontend", "success"),
            ("worker-frontend", "success"),
            ("worker-frontend", "success"),
        ] {
            let prior = create_task(&db, "prior ambiguous task");
            let entry = crate::routing::TaskRouter::assignment_entry(&prior, slug);
            db.assign_with_routing_history(prior.id, slug, &entry)
                .expect("persist automatic route");
            db.update_latest_routing_outcome_for_worker(&prior.id.to_string(), slug, outcome)
                .expect("record route outcome");
        }

        let task = create_task(&db, "new unfamiliar task needs implementation");
        let context = ToolContext {
            session_id: "auto-route-session".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "auto-route-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let result = AssignTasksBatchTool
            .execute(json!({"assignments": [{"task_id": task.id}]}), &context)
            .await;
        assert!(!result.is_error, "automatic dispatch succeeds: {result:?}");
        let row = db.get_by_id(task.id).unwrap().unwrap();
        assert_eq!(row.status, TaskStatus::Assigned);
        assert_eq!(row.assigned_worker.as_deref(), Some("worker-frontend"));
    }
}
