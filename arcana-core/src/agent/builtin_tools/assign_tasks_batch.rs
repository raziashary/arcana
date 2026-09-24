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
         Each assignment in the batch is validated independently; bad entries \
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
                    "description": "List of (task_id, worker_slug) pairs to dispatch.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "task_id":     { "type": "integer", "description": "Task ID, must be in dispatchable state (ready/assigned/failed/rejected)." },
                            "worker_slug": {
                                "type": "string",
                                "description": "A currently registered worker slug."
                            }
                        },
                        "required": ["task_id", "worker_slug"]
                    },
                    "minItems": 1
                }
            },
            "required": ["assignments"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let assignments = match input.get("assignments").and_then(|v| v.as_array()) {
            Some(a) if !a.is_empty() => a.clone(),
            _ => return ToolResult::error(
                "'assignments' is required and must be a non-empty array of {task_id, worker_slug}",
            ),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let registered_workers =
            match crate::db::workers::WorkersDb::open().and_then(|db| db.registered_slugs()) {
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

            // Per-entry validation — failures recorded but don't abort the batch.
            let (id, worker_slug) = match (task_id, worker) {
                (Some(id), Some(w)) if id > 0 && !w.is_empty() => (id, w),
                _ => {
                    failure_count += 1;
                    results.push(json!({
                        "index":   idx,
                        "ok":      false,
                        "error":   "entry must have task_id (positive integer) and worker_slug (non-empty string)",
                    }));
                    continue;
                }
            };

            if !registered_workers.iter().any(|slug| slug == &worker_slug) {
                failure_count += 1;
                results.push(json!({
                    "index":       idx,
                    "task_id":     id,
                    "worker_slug": worker_slug,
                    "ok":          false,
                    "error":       format!("'{}' is not a registered worker. Registered workers: {}", worker_slug, registered_workers.join(", ")),
                }));
                continue;
            }

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
                        "worker_slug": worker_slug,
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
                        "worker_slug": worker_slug,
                        "ok":          false,
                        "error":       format!("Failed to fetch task #{id}: {e}"),
                    }));
                    continue;
                }
            };

            let prev_status = task.status.as_str().to_string();
            if !matches!(
                prev_status.as_str(),
                "ready" | "assigned" | "failed" | "rejected"
            ) {
                failure_count += 1;
                results.push(json!({
                    "index":           idx,
                    "task_id":         id,
                    "worker_slug":     worker_slug,
                    "previous_status": prev_status,
                    "ok":              false,
                    "error":           format!(
                        "Task #{id} is in status='{prev_status}' and cannot be (re)assigned. \
                         Only 'ready', 'assigned', 'failed', or 'rejected' tasks may be assigned."
                    ),
                }));
                continue;
            }

            if let Err(e) = db.update_status(id, "assigned", Some(&worker_slug)) {
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

            // Record affinity history for this assignment so the routing
            // scorer keeps learning from batched dispatches too.
            let _ = crate::routing::TaskRouter::record_assignment(&task, &worker_slug);

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
