// create_task — Delegate Mode only. Writes a new task to ~/.arcana/tasks.db.
// PA calls this when in Delegate Mode instead of directly editing files.
// The task enters the board with status="ready" for Supervisor to pick up.
// See: DEVELOPMENT_PLAN.md Phase 2.1 — PA Mode Toggle / create_task Tool

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::{CreateTaskInput, TasksDb};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct CreateTaskTool;

#[async_trait]
impl Tool for CreateTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "create_task"
    }

    fn description(&self) -> &str {
        "Create a new task on the internal Arcana task board. Use this in Delegate Mode to assign work to the Worker fleet. The task enters with status 'ready' and will be picked up by the Supervisor within 2 minutes. Always include specific, testable acceptance criteria."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short, specific task title (max 120 chars). E.g. 'Add GET /agents/health endpoint'"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of what needs to be done and why (optional)"
                },
                "priority": {
                    "type": "string",
                    "enum": ["p0", "p1", "p2", "p3"],
                    "description": "p0=critical/blocking, p1=high, p2=medium (default), p3=low"
                },
                "task_type": {
                    "type": "string",
                    "enum": ["feature", "bug", "refactor", "docs", "chore"],
                    "description": "Type of work. feature=new functionality, bug=fix existing, refactor=restructure, docs=documentation, chore=maintenance"
                },
                "acceptance_criteria": {
                    "type": "string",
                    "description": "Specific, verifiable criteria that define when this task is done. List each criterion on a new line. Be precise — the QC agent will check these."
                }
            },
            "required": ["title", "priority", "task_type", "acceptance_criteria"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let title = match input["title"].as_str() {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => return ToolResult::error("'title' is required and must not be empty"),
        };
        if title.len() > 120 {
            return ToolResult::error("title must be 120 characters or less");
        }

        let priority = input["priority"].as_str().unwrap_or("p2").to_string();
        let task_type = input["task_type"].as_str().unwrap_or("feature").to_string();
        let acceptance_criteria = match input["acceptance_criteria"].as_str() {
            Some(ac) if !ac.trim().is_empty() => ac.trim().to_string(),
            _ => {
                return ToolResult::error("'acceptance_criteria' is required and must not be empty")
            }
        };
        let description = input["description"].as_str().map(|s| s.to_string());

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(&format!("Failed to open tasks database: {e}")),
        };

        let task = match db.create(CreateTaskInput {
            title: title.clone(),
            description,
            priority: priority.clone(),
            task_type: task_type.clone(),
            acceptance_criteria: acceptance_criteria.clone(),
            created_by: Some("pa".to_string()),
        }) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(&format!("Failed to create task: {e}")),
        };

        // Phase 3.0 R1 — push-based dispatch. Wake the Supervisor immediately
        // so it dispatches this task on the next event-loop tick instead of
        // waiting up to one cron cycle (previously ~21 s dead window).
        crate::recipes::scheduler::kick("supervisor");

        // Phase 3.0 R1 — notify the UI (Arcana Panel kanban) that the board
        // changed without requiring a poll. The frontend `taskStore` listens
        // to `task:changed` and re-fetches `list_tasks` on any event.
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(task.id),
            kind: "created".into(),
            status: Some("ready".into()),
            assigned_worker: None,
        });

        ToolResult::success(json!({
            "task_id": task.id,
            "title": task.title,
            "priority": priority,
            "task_type": task_type,
            "status": "ready",
            "message": format!(
                "✅ Task #{} created on the Arcana board. Status: ready. The Supervisor will pick this up within 2 minutes and assign it to a worker.",
                task.id
            )
        }).to_string())
    }
}

#[cfg(test)]
mod p4_51_board_matrix_tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::{AgentIdentity, ToolResult};
    use crate::db::runs::RunsDb;
    use crate::db::tasks::{CreateTaskInput, TaskStatus, TasksDb};
    use crate::db::workers::{WorkerStatus, WorkersDb};
    const SESSION: &str = "p4-51-board-live-matrix";

    fn live_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        registry
    }

    fn executor(
        home: &std::path::Path,
        identity: AgentIdentity,
        run_id: &str,
        task_id: Option<i64>,
    ) -> ToolExecutor {
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            SESSION.into(),
            home.to_string_lossy().into_owned(),
            identity,
        )
        .with_run_id(Some(run_id.into()));
        executor.set_task_id(task_id);
        executor
    }

    async fn invoke(
        executor: &mut ToolExecutor,
        registry: &ToolRegistry,
        name: &str,
        input: Value,
        call: &str,
    ) -> ToolResult {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("FastDevelopment registry is missing {name}"));
        let results = executor
            .execute_batch(vec![(tool, input, call.into())])
            .await
            .expect("execute registered board tool through ToolExecutor");
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].0, call,
            "executor preserves provider tool-use ID"
        );
        results.into_iter().next().unwrap().1
    }

    fn assert_success(result: &ToolResult, name: &str) -> Value {
        assert!(!result.is_error, "{name} failed: {}", result.output);
        serde_json::from_str(&result.output)
            .unwrap_or_else(|error| panic!("{name} returned invalid success JSON: {error}"))
    }

    fn assert_failure(
        result: &ToolResult,
        name: &str,
        call: &str,
        agent: &str,
        run_id: &str,
        task_id: i64,
        private_markers: &[&str],
    ) -> Value {
        assert!(
            result.is_error,
            "{name} returned success-shaped failure: {}",
            result.output
        );
        assert!(result.output.len() <= 8192, "{name} error exceeded bound");
        for marker in private_markers {
            assert!(
                !result.output.contains(marker),
                "{name} error reflected private marker {marker}"
            );
        }
        let envelope: Value = serde_json::from_str(&result.output)
            .unwrap_or_else(|error| panic!("{name} failure was not JSON: {error}"));
        assert_eq!(envelope["schema_version"], 1, "{name}: {envelope}");
        assert_eq!(envelope["ok"], false, "{name}: {envelope}");
        assert_eq!(envelope["tool"], name, "{name}: {envelope}");
        assert_eq!(envelope["operation"], format!("tool.{name}"));
        assert!(!envelope["error_code"]
            .as_str()
            .unwrap_or_default()
            .is_empty());
        assert!(!envelope["category"].as_str().unwrap_or_default().is_empty());
        assert!(envelope["retryable"].is_boolean());
        assert!(envelope["retry_after_ms"].is_null() || envelope["retry_after_ms"].is_u64());
        assert!(envelope["message"].as_str().unwrap_or_default().len() <= 512);
        assert!(envelope["cause"].as_str().unwrap_or_default().len() <= 512);
        assert!(
            envelope["suggested_action"]
                .as_str()
                .unwrap_or_default()
                .len()
                <= 512
        );
        assert_eq!(envelope["context"]["session_id"], SESSION);
        assert_eq!(envelope["context"]["agent_id"], agent);
        assert_eq!(envelope["context"]["task_id"], task_id);
        assert_eq!(
            envelope["context"]["tool_call_id"],
            format!("{SESSION}-{call}")
        );
        assert_eq!(envelope["context"]["run_id"], run_id);
        assert!(envelope["timestamp"].as_str().is_some());
        assert!(envelope["error"].is_object());
        envelope
    }

    fn fixture_task(db: &TasksDb, title: &str) -> crate::db::tasks::Task {
        db.create(CreateTaskInput {
            title: title.into(),
            description: Some("isolated board matrix fixture".into()),
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "the expected board state is persisted".into(),
            created_by: Some("matrix-test".into()),
        })
        .expect("create isolated fixture task")
    }

    #[tokio::test]
    async fn board_live_fastdevelopment_registry_has_success_and_failure_proof() {
        let home = tempfile::tempdir().expect("isolated Arcana home and workspace");
        let _home = crate::db::test_support::with_home(home.path());
        let registry = live_registry();
        for name in [
            "create_task",
            "list_tasks",
            "assign_task",
            "assign_tasks",
            "update_task_status",
            "claim_task",
        ] {
            assert!(
                registry.get(name).is_some(),
                "missing registered tool {name}"
            );
        }

        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(
                "Board matrix worker",
                "worker-board-matrix",
                "backend",
                "fixture-model",
                &[],
                120,
                None,
            )
            .expect("create implementation worker");
        workers
            .update_status(worker_id, WorkerStatus::Idle)
            .expect("mark fixture worker runnable");

        let runs = RunsDb::open().expect("open isolated runs DB");
        let run_id = runs
            .begin_worker_tick("worker-board-matrix", "board-matrix-session")
            .expect("create correlated worker run");
        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let mut supervisor = executor(home.path(), AgentIdentity::supervisor(), &run_id, None);
        let mut worker = executor(
            home.path(),
            AgentIdentity::worker("worker-board-matrix"),
            &run_id,
            None,
        );

        // create_task's production handler calls the existing scheduler kick
        // boundary. This test does not install or start a global scheduler;
        // kick is a documented no-op when none is installed.
        let created = invoke(
            &mut supervisor,
            &registry,
            "create_task",
            json!({
                "title":"Board matrix task created by tool",
                "description":"synthetic description",
                "priority":"p1",
                "task_type":"feature",
                "acceptance_criteria":"the create result is visible from list_tasks"
            }),
            "create-success",
        )
        .await;
        let created = assert_success(&created, "create_task");
        let created_id = created["task_id"].as_i64().expect("created task id");
        supervisor.set_task_id(Some(created_id));
        assert_eq!(created["status"], "ready");
        let persisted = tasks
            .get_by_id(created_id)
            .expect("read created task")
            .expect("created task row");
        assert_eq!(persisted.title, "Board matrix task created by tool");
        assert_eq!(persisted.status, TaskStatus::Ready);

        let listed = invoke(
            &mut supervisor,
            &registry,
            "list_tasks",
            json!({"status":"ready", "compact":false}),
            "list-success",
        )
        .await;
        let listed = assert_success(&listed, "list_tasks");
        assert!(listed["tasks"].as_array().unwrap().iter().any(|task| {
            task["id"] == created_id
                && task["title"] == "Board matrix task created by tool"
                && task["acceptance_criteria"] == "the create result is visible from list_tasks"
        }));

        let batch_task = fixture_task(&tasks, "Batch assignment fixture");
        let assign_failure_task = fixture_task(&tasks, "Single assignment failure fixture");
        let batch_failure_task = fixture_task(&tasks, "Batch assignment failure fixture");
        let assign_failed_id = assign_failure_task.id;
        let batch_failed_id = batch_failure_task.id;

        let assigned = invoke(
            &mut supervisor,
            &registry,
            "assign_task",
            json!({
                "id":created_id,
                "worker_slug":"worker-board-matrix",
                "override_reason":"isolated deterministic matrix assignment"
            }),
            "assign-success",
        )
        .await;
        let assigned = assert_success(&assigned, "assign_task");
        assert_eq!(assigned["task_id"], created_id);
        assert_eq!(assigned["new_status"], "assigned");
        let assigned_row = tasks.get_by_id(created_id).unwrap().unwrap();
        assert_eq!(assigned_row.status, TaskStatus::Assigned);
        assert_eq!(
            assigned_row.assigned_worker.as_deref(),
            Some("worker-board-matrix")
        );

        let batch = invoke(
            &mut supervisor,
            &registry,
            "assign_tasks",
            json!({"assignments":[
                {"task_id":batch_task.id, "worker_slug":"worker-board-matrix", "override_reason":"isolated batch assignment"},
                {"task_id":0}
            ]}),
            "assign-batch-success-and-item-failure",
        )
        .await;
        assert!(
            !batch.is_error,
            "per-item batch failure must retain batch success result"
        );
        let batch: Value = serde_json::from_str(&batch.output).expect("batch result JSON");
        assert_eq!(batch["success_count"], 1);
        assert_eq!(batch["failure_count"], 1);
        assert_eq!(batch["results"][0]["ok"], true);
        assert_eq!(batch["results"][1]["ok"], false);
        let batch_row = tasks.get_by_id(batch_task.id).unwrap().unwrap();
        assert_eq!(batch_row.status, TaskStatus::Assigned);
        assert_eq!(
            batch_row.assigned_worker.as_deref(),
            Some("worker-board-matrix")
        );

        // A DB abort is a handler failure for each write path. `assign_tasks`
        // deliberately converts per-entry DB failures into result rows, so
        // its actual is_error=true case is also exercised below by the empty
        // batch validation and final tasks.db obstruction.
        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_task_insert BEFORE INSERT ON tasks
                 WHEN NEW.title='BOARD_PRIVATE_CREATE_TITLE'
                 BEGIN SELECT RAISE(ABORT, 'injected task insert failure'); END;",
            )
            .expect("inject isolated task creation failure");
        let create_failed = invoke(
            &mut supervisor,
            &registry,
            "create_task",
            json!({
                "title":"BOARD_PRIVATE_CREATE_TITLE",
                "description":"BOARD_PRIVATE_DESCRIPTION",
                "priority":"p2",
                "task_type":"feature",
                "acceptance_criteria":"BOARD_PRIVATE_ACCEPTANCE"
            }),
            "create-database-failure",
        )
        .await;
        assert_failure(
            &create_failed,
            "create_task",
            "create-database-failure",
            "supervisor",
            &run_id,
            created_id,
            &[
                "BOARD_PRIVATE_CREATE_TITLE",
                "BOARD_PRIVATE_DESCRIPTION",
                "BOARD_PRIVATE_ACCEPTANCE",
            ],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_task_insert;")
            .expect("remove isolated task creation failure");

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_single_assignment BEFORE UPDATE OF status ON tasks
                 WHEN NEW.id=(SELECT id FROM tasks WHERE title='Single assignment failure fixture')
                   AND NEW.status='assigned'
                 BEGIN SELECT RAISE(ABORT, 'injected task assignment failure'); END;",
            )
            .expect("inject isolated assignment failure");
        let assign_failed = invoke(
            &mut supervisor,
            &registry,
            "assign_task",
            json!({
                "id":assign_failed_id,
                "worker_slug":"worker-board-matrix",
                "override_reason":"BOARD_PRIVATE_ASSIGNMENT_REASON"
            }),
            "assign-database-failure",
        )
        .await;
        assert_failure(
            &assign_failed,
            "assign_task",
            "assign-database-failure",
            "supervisor",
            &run_id,
            created_id,
            &["BOARD_PRIVATE_ASSIGNMENT_REASON"],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_single_assignment;")
            .expect("remove isolated assignment failure");
        assert_eq!(
            tasks.get_by_id(assign_failed_id).unwrap().unwrap().status,
            TaskStatus::Ready,
            "failed assignment must not partially dispatch the task"
        );

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_batch_assignment BEFORE UPDATE OF status ON tasks
                 WHEN NEW.id=(SELECT id FROM tasks WHERE title='Batch assignment failure fixture')
                   AND NEW.status='assigned'
                 BEGIN SELECT RAISE(ABORT, 'injected batch assignment failure'); END;",
            )
            .expect("inject isolated batch assignment failure");
        let batch_failed = invoke(
            &mut supervisor,
            &registry,
            "assign_tasks",
            json!({"assignments":[{
                "task_id":batch_failed_id,
                "worker_slug":"worker-board-matrix",
                "override_reason":"isolated batch failure"
            }]}),
            "assign-batch-item-failure",
        )
        .await;
        assert!(
            !batch_failed.is_error,
            "batch DB failures are represented per entry"
        );
        let batch_failed: Value =
            serde_json::from_str(&batch_failed.output).expect("batch failure JSON");
        assert_eq!(batch_failed["failure_count"], 1);
        assert_eq!(batch_failed["results"][0]["ok"], false);
        assert!(batch_failed["results"][0]["error"].as_str().is_some());
        assert_eq!(
            tasks.get_by_id(batch_failed_id).unwrap().unwrap().status,
            TaskStatus::Ready,
            "failed batch entry must not partially dispatch the task"
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_batch_assignment;")
            .expect("remove isolated batch assignment failure");
        let empty_batch = invoke(
            &mut supervisor,
            &registry,
            "assign_tasks",
            json!({"assignments":[]}),
            "assign-empty-batch-failure",
        )
        .await;
        assert_failure(
            &empty_batch,
            "assign_tasks",
            "assign-empty-batch-failure",
            "supervisor",
            &run_id,
            created_id,
            &[],
        );

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_claim BEFORE UPDATE OF status ON tasks
                 WHEN NEW.id=(SELECT id FROM tasks WHERE title='Batch assignment fixture')
                   AND NEW.status='in_progress'
                 BEGIN SELECT RAISE(ABORT, 'injected task claim failure'); END;",
            )
            .expect("inject isolated claim failure");
        worker.set_task_id(Some(batch_task.id));
        let claim_failed = invoke(
            &mut worker,
            &registry,
            "claim_task",
            json!({"task_id":batch_task.id}),
            "claim-database-failure",
        )
        .await;
        assert_failure(
            &claim_failed,
            "claim_task",
            "claim-database-failure",
            "worker-board-matrix",
            &run_id,
            batch_task.id,
            &[],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_claim;")
            .expect("remove isolated claim failure");

        let claimed = invoke(
            &mut worker,
            &registry,
            "claim_task",
            json!({"task_id":batch_task.id}),
            "claim-success",
        )
        .await;
        let claimed = assert_success(&claimed, "claim_task");
        assert_eq!(claimed["task_id"], batch_task.id);
        assert_eq!(claimed["status"], "in_progress");
        assert_eq!(claimed["worker_slug"], "worker-board-matrix");
        assert_eq!(claimed["pid"], std::process::id());
        let claimed_row = tasks.get_by_id(batch_task.id).unwrap().unwrap();
        assert_eq!(claimed_row.status, TaskStatus::InProgress);
        assert_eq!(
            claimed_row.assigned_worker.as_deref(),
            Some("worker-board-matrix")
        );
        assert_eq!(
            claimed_row.claimed_by_pid,
            Some(i64::from(std::process::id()))
        );
        assert!(claimed_row
            .claim_host
            .as_deref()
            .is_some_and(|host| !host.is_empty()));

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_status_update BEFORE UPDATE OF status ON tasks
                 WHEN NEW.id=(SELECT id FROM tasks WHERE title='Batch assignment fixture')
                   AND NEW.status='failed'
                 BEGIN SELECT RAISE(ABORT, 'injected task status update failure'); END;",
            )
            .expect("inject isolated status update failure");
        worker.set_task_id(Some(batch_task.id));
        let status_failed = invoke(
            &mut worker,
            &registry,
            "update_task_status",
            json!({"id":batch_task.id, "status":"failed", "rejection_reason":"BOARD_PRIVATE_STATUS_REASON"}),
            "status-database-failure",
        )
        .await;
        assert_failure(
            &status_failed,
            "update_task_status",
            "status-database-failure",
            "worker-board-matrix",
            &run_id,
            batch_task.id,
            &["BOARD_PRIVATE_STATUS_REASON"],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_status_update;")
            .expect("remove isolated status update failure");

        let updated = invoke(
            &mut worker,
            &registry,
            "update_task_status",
            json!({"id":batch_task.id, "status":"failed"}),
            "status-success",
        )
        .await;
        assert_eq!(
            assert_success(&updated, "update_task_status")["new_status"],
            "failed"
        );
        let failed_row = tasks.get_by_id(batch_task.id).unwrap().unwrap();
        assert_eq!(failed_row.status, TaskStatus::Failed);
        assert_eq!(failed_row.assigned_worker, None);
        assert_eq!(failed_row.claimed_by_pid, None);

        // A wrong worker identity is a separate claim policy failure and must
        // leave the single-assigned task untouched.
        let mut wrong_worker = executor(
            home.path(),
            AgentIdentity::worker("worker-board-other"),
            &run_id,
            Some(created_id),
        );
        let wrong_claim = invoke(
            &mut wrong_worker,
            &registry,
            "claim_task",
            json!({"task_id":created_id}),
            "claim-wrong-worker-failure",
        )
        .await;
        assert_failure(
            &wrong_claim,
            "claim_task",
            "claim-wrong-worker-failure",
            "worker-board-other",
            &run_id,
            created_id,
            &[],
        );
        assert_eq!(
            tasks.get_by_id(created_id).unwrap().unwrap().status,
            TaskStatus::Assigned
        );

        drop(tasks);
        drop(runs);
        worker.set_task_id(Some(created_id));
        let task_db_path = home.path().join(".arcana").join("tasks.db");
        std::fs::remove_file(&task_db_path).expect("remove only isolated temp-home tasks.db");
        std::fs::create_dir(&task_db_path).expect("obstruct only isolated temp-home tasks.db");

        for (name, call, input, agent, context_task_id) in [
            (
                "create_task",
                "create-open-failure",
                json!({"title":"BOARD_PRIVATE_OBSTRUCTED_CREATE", "priority":"p2", "task_type":"feature", "acceptance_criteria":"synthetic"}),
                "supervisor",
                created_id,
            ),
            (
                "list_tasks",
                "list-open-failure",
                json!({}),
                "supervisor",
                created_id,
            ),
            (
                "assign_task",
                "assign-open-failure",
                json!({"id":created_id, "worker_slug":"worker-board-matrix", "override_reason":"synthetic"}),
                "supervisor",
                created_id,
            ),
            (
                "assign_tasks",
                "batch-open-failure",
                json!({"assignments":[{"task_id":assign_failed_id}]}),
                "supervisor",
                created_id,
            ),
            (
                "update_task_status",
                "status-open-failure",
                json!({"id":created_id, "status":"failed"}),
                "supervisor",
                created_id,
            ),
            (
                "claim_task",
                "claim-open-failure",
                json!({"task_id":created_id}),
                "worker-board-matrix",
                created_id,
            ),
        ] {
            let executor = if agent == "supervisor" {
                &mut supervisor
            } else {
                &mut worker
            };
            let result = invoke(executor, &registry, name, input, call).await;
            assert_failure(
                &result,
                name,
                call,
                agent,
                &run_id,
                context_task_id,
                &["BOARD_PRIVATE_OBSTRUCTED_CREATE"],
            );
        }
    }

    #[tokio::test]
    async fn batch_assignment_intents_journal_the_registered_tool_identity() {
        let home = tempfile::tempdir().expect("isolated Arcana home and workspace");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(
                "Journal identity worker",
                "worker-journal-identity",
                "backend",
                "fixture-model",
                &[],
                120,
                None,
            )
            .expect("create implementation worker");
        workers
            .update_status(worker_id, WorkerStatus::Idle)
            .expect("mark fixture worker runnable");
        let runs = RunsDb::open().expect("open isolated runs DB");
        let run_id = runs
            .begin_worker_tick("worker-journal-identity", SESSION)
            .expect("create correlated worker run");
        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task = fixture_task(&tasks, "Journal identity assignment");
        let registry = live_registry();
        let mut supervisor = executor(home.path(), AgentIdentity::supervisor(), &run_id, None);

        let batch = invoke(
            &mut supervisor,
            &registry,
            "assign_tasks",
            json!({"assignments":[{
                "task_id":task.id,
                "worker_slug":"worker-journal-identity",
                "override_reason":"isolated journal identity check"
            }]}),
            "assign-journal-identity",
        )
        .await;
        assert!(!batch.is_error, "batch assignment: {}", batch.output);

        let journal = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read run-scoped operation journal");
        assert_eq!(
            journal.entries.len(),
            1,
            "one journaled side effect for the batch assignment"
        );
        assert_eq!(
            journal.entries[0].operation, "assign_tasks",
            "batch assignments journal the registered tool name, not a retired alias"
        );
    }
}
