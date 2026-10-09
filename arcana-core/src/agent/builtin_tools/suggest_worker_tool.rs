// suggest_worker — MCP tool that returns the recommended worker for a task.
// Available to all MCP workers; the Supervisor recipe calls it before assign_task
// to make data-driven routing decisions based on historical performance.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::db::workers::WorkersDb;
use crate::routing::TaskRouter;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct SuggestWorkerTool;

#[async_trait]
impl Tool for SuggestWorkerTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "suggest_worker"
    }

    fn description(&self) -> &str {
        "Get the recommended worker for a task based on historical performance and \
        static keyword patterns. Returns worker_slug, score (0.0-1.0), confidence, \
        and method ('static' | 'historical' | 'default'). Call this before assign_task \
        to make data-driven routing decisions."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "integer",
                    "description": "Numeric task ID from the internal board"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let task_id = match input["task_id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'task_id' must be a positive integer"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks database: {e}")),
        };

        let task = match db.get_by_id(task_id) {
            Ok(Some(t)) => t,
            Ok(None) => return ToolResult::error(format!("Task #{task_id} not found")),
            Err(e) => return ToolResult::error(format!("Failed to fetch task #{task_id}: {e}")),
        };

        match TaskRouter::suggest_worker(&task) {
            Ok(score) => {
                // Bug A — surface `primary_repos` so the field stops being
                // dead metadata. The supervisor (or any caller doing routing)
                // gets to see which repos this worker is configured to own
                // and can use it as a sanity check on the recommendation
                // before assigning. Best-effort: a missing or empty repos
                // list returns an empty array, not an error.
                let primary_repos = WorkersDb::open()
                    .ok()
                    .and_then(|db| db.get_by_slug(&score.worker_slug).ok().flatten())
                    .map(|w| w.primary_repos)
                    .unwrap_or_default();

                ToolResult::success(
                    json!({
                        "task_id":       task_id,
                        "task_title":    task.title,
                        "task_type":     task.task_type.as_str(),
                        "worker_slug":   score.worker_slug,
                        "score":         score.score,
                        "confidence":    score.confidence,
                        "method":        score.method,
                        "primary_repos": primary_repos,
                        "message": format!(
                            "Recommended worker: {} (score={:.2}, method={})",
                            score.worker_slug, score.score, score.method
                        )
                    })
                    .to_string(),
                )
            }
            Err(e) => ToolResult::error(format!("Failed to compute worker suggestion: {e}")),
        }
    }
}

#[cfg(test)]
mod p4_51_executor_tests {
    use super::*;

    #[tokio::test]
    async fn suggest_worker_live_registry_executor_matrix_uses_persisted_runnable_roster() {
        use crate::agent::{
            builtin_tools,
            permission::{new_pending_permissions, PermissionMode},
            tool_executor::ToolExecutor,
            tool_registry::ToolRegistry,
            tool_trait::AgentIdentity,
            PAMode,
        };
        use crate::db::tasks::{CreateTaskInput, TaskStatus, TasksDb};
        use crate::db::workers::{WorkerStatus, WorkersDb};

        let home = tempfile::tempdir().expect("isolated Arcana home for task and worker DBs");
        let _home = crate::db::test_support::with_home(home.path());

        let tasks = TasksDb::open().expect("open isolated task database");
        let task = tasks
            .create(CreateTaskInput {
                title: "Implement an API endpoint for artifact routing".into(),
                description: Some("Persist a backend route contract".into()),
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "The backend endpoint is documented".into(),
                created_by: Some("p4-51 test".into()),
            })
            .expect("persist task through the production task store");
        let persisted_task = TasksDb::open()
            .expect("reopen production task reader")
            .get_by_id(task.id)
            .expect("read task by returned database ID")
            .expect("persisted task is visible");
        assert_eq!(
            persisted_task.title,
            "Implement an API endpoint for artifact routing"
        );
        assert_eq!(persisted_task.status, TaskStatus::Ready);

        let workers = WorkersDb::open().expect("open isolated seeded worker roster");
        let backend = workers
            .get_by_slug("worker-backend")
            .expect("read seeded backend worker")
            .expect("seeded backend worker exists");
        workers
            .update_status(backend.id, WorkerStatus::Idle)
            .expect("mark the backend worker runnable through the production API");
        for slug in [
            "worker-frontend",
            "worker-platform",
            "worker-mcp",
            "worker-cleanup",
        ] {
            let worker = workers
                .get_by_slug(slug)
                .expect("read seeded implementation worker")
                .unwrap_or_else(|| panic!("seeded worker {slug} is present"));
            workers
                .update_status(worker.id, WorkerStatus::Paused)
                .expect("exclude non-target implementation worker from runnable roster");
        }
        drop(workers);

        let roster = WorkersDb::open()
            .expect("reopen production worker-roster reader")
            .list()
            .expect("read persisted worker roster");
        let persisted_backend = roster
            .iter()
            .find(|worker| worker.slug == "worker-backend")
            .expect("backend remains registered");
        assert_eq!(persisted_backend.status, WorkerStatus::Idle);
        assert!(
            [
                "worker-frontend",
                "worker-platform",
                "worker-mcp",
                "worker-cleanup"
            ]
            .iter()
            .all(|slug| roster
                .iter()
                .find(|worker| worker.slug == *slug)
                .is_some_and(|worker| worker.status == WorkerStatus::Paused)),
            "only the static-pattern backend worker is runnable"
        );

        let mut registry = ToolRegistry::new();
        builtin_tools::register_for_mode(&mut registry, PAMode::FastDevelopment);
        let tool = registry
            .get("suggest_worker")
            .expect("suggest_worker is registered in FastDevelopment");
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "p4-51-routing-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        executor.set_task_id(Some(task.id));

        let successes = executor
            .execute_batch(vec![(
                tool.clone(),
                json!({"task_id": task.id}),
                "routing-success".into(),
            )])
            .await
            .expect("registered routing tool returns a result");
        assert_eq!(successes.len(), 1);
        let (success_call, success) = successes.into_iter().next().expect("one result");
        assert_eq!(success_call, "routing-success");
        assert!(!success.is_error, "routing success: {}", success.output);
        let recommendation: Value = serde_json::from_str(&success.output).unwrap_or_else(|error| {
            panic!(
                "successful model-facing route JSON: {error}; output={}",
                success.output
            )
        });
        assert_eq!(recommendation["task_id"], task.id);
        assert_eq!(recommendation["task_title"], persisted_task.title);
        assert_eq!(recommendation["task_type"], "feature");
        assert_eq!(recommendation["worker_slug"], "worker-backend");
        assert_eq!(recommendation["method"], "static");
        assert!((recommendation["confidence"].as_f64().unwrap_or(-1.0) - 0.85).abs() < 0.0001);
        assert_eq!(
            recommendation["primary_repos"],
            json!(persisted_backend.primary_repos)
        );
        assert!((0.0..=1.0).contains(&recommendation["score"].as_f64().unwrap_or(-1.0)));
        assert_eq!(
            TasksDb::open()
                .expect("read task after suggestion")
                .get_by_id(task.id)
                .expect("reload task")
                .expect("task remains present")
                .assigned_worker,
            None,
            "suggestion reports a route without assigning the task"
        );

        let failure_input = json!({
            "task_id":"PRIVATE_ROUTING_TASK_ID_MARKER",
            "prompt":"PRIVATE_ROUTING_PROMPT_MARKER",
            "source":"PRIVATE_ROUTING_SOURCE_MARKER"
        });
        let failures = executor
            .execute_batch(vec![(tool, failure_input, "routing-failure".into())])
            .await
            .expect("malformed registered routing call becomes a tool result");
        assert_eq!(failures.len(), 1);
        let (failure_call, failure) = failures.into_iter().next().expect("one result");
        assert_eq!(failure_call, "routing-failure");
        assert!(
            failure.is_error,
            "malformed route input must remain a failed result"
        );
        assert!(failure.output.len() <= 8 * 1024, "routing error is bounded");
        let envelope: Value =
            serde_json::from_str(&failure.output).expect("versioned routing error envelope");
        assert_eq!(envelope["schema_version"], 1);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["error"]["tool"], "suggest_worker");
        assert_eq!(envelope["error"]["operation"], "tool.suggest_worker");
        assert_eq!(
            envelope["error"]["context"]["session_id"],
            "p4-51-routing-session"
        );
        assert_eq!(envelope["error"]["context"]["agent_id"], "supervisor");
        assert_eq!(envelope["error"]["context"]["task_id"], task.id);
        assert_eq!(
            envelope["error"]["context"]["tool_call_id"],
            "p4-51-routing-session-routing-failure"
        );
        for field in [
            "error_code",
            "category",
            "message",
            "cause",
            "suggested_action",
        ] {
            assert!(
                envelope["error"][field].as_str().is_some(),
                "missing {field}: {envelope}"
            );
        }
        for field in ["message", "cause", "suggested_action"] {
            assert!(envelope["error"][field].as_str().unwrap_or_default().len() <= 512);
        }
        assert!(envelope["error"]["retryable"].is_boolean());
        assert!(
            envelope["error"]["retry_after_ms"].is_null()
                || envelope["error"]["retry_after_ms"].is_u64()
        );
        assert!(envelope["error"]["timestamp"].as_str().is_some());
        for private in [
            "PRIVATE_ROUTING_TASK_ID_MARKER",
            "PRIVATE_ROUTING_PROMPT_MARKER",
            "PRIVATE_ROUTING_SOURCE_MARKER",
        ] {
            assert!(
                !failure.output.contains(private),
                "leaked private argument {private}"
            );
        }
    }
}
