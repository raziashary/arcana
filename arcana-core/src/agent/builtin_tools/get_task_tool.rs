// get_task_tool — Native tool to fetch task detail or explicitly requested
// retained budget-resolution audit history by the task's numeric ID.

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct GetTaskTool;

#[async_trait]
impl Tool for GetTaskTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "get_task"
    }

    fn description(&self) -> &str {
        "Fetch a single task by its numeric ID. Returns the full task record including \
        rejection_reason (set by QC when status=rejected) and retry_count. \
        Use this after list_tasks to get full details on a specific task. Set \
        include_deleted_audit=true only to read retained budget-resolution history \
        when the task was deleted; that response contains audit identity and history only."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "integer",
                    "description": "The numeric task ID to fetch"
                },
                "include_deleted_audit": {
                    "type": "boolean",
                    "description": "When true, return retained budget-resolution audit history for a deleted task. Defaults to false; a deleted or missing task remains an error unless history exists and this is true."
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(id) if id > 0 => id,
            _ => return ToolResult::error("'id' must be a positive integer task ID"),
        };
        let include_deleted_audit = match input.get("include_deleted_audit") {
            None => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return ToolResult::error("'include_deleted_audit' must be a boolean"),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(_) => return ToolResult::error("Failed to open tasks database"),
        };

        match db.get_by_id(id) {
            Ok(Some(t)) => {
                let budget_resolutions = match db.get_task_budget_resolution_history(id, 20) {
                    Ok(rows) => rows,
                    Err(_) => return ToolResult::error("Failed to read task budget history"),
                };
                ToolResult::success(
                    json!({
                        "id":                  t.id,
                        "task_identity":       t.task_identity,
                        "title":               t.title,
                        "description":         t.description,
                        "priority":            t.priority.as_str(),
                        "task_type":           t.task_type.as_str(),
                        "status":              t.status.as_str(),
                        "acceptance_criteria": t.acceptance_criteria,
                        "assigned_worker":     t.assigned_worker,
                        "rejection_reason":    t.rejection_reason,
                        "retry_count":         t.retry_count,
                        "created_by":          t.created_by,
                        "created_at":          t.created_at,
                        "updated_at":          t.updated_at,
                        "completed_at":        t.completed_at,
                        "blocked_by":          t.blocked_by,
                        "budget_max_tokens":   t.budget_max_tokens,
                        "budget_max_usd":      t.budget_max_usd,
                        "budget_max_turns":    t.budget_max_turns,
                        "budget_tokens_used":  t.budget_tokens_used,
                        "budget_cost_usd":     t.budget_cost_usd,
                        "budget_turns_used":   t.budget_turns_used,
                        "budget_exceeded":     t.budget_exceeded,
                        "budget_exceeded_reason": t.budget_exceeded_reason,
                        "budget_resolutions":  budget_resolutions,
                    })
                    .to_string(),
                )
            }
            Ok(None) if include_deleted_audit => {
                let budget_resolutions = match db.get_task_budget_resolution_history(id, 20) {
                    Ok(rows) => rows,
                    Err(_) => return ToolResult::error("Failed to read task budget history"),
                };
                let Some(task_identity) = budget_resolutions
                    .first()
                    .map(|resolution| resolution.task_identity.as_str())
                else {
                    return ToolResult::error(format!(
                        "Task #{id} not found and no retained budget-resolution history exists"
                    ));
                };
                if budget_resolutions
                    .iter()
                    .any(|resolution| resolution.task_identity != task_identity)
                {
                    return ToolResult::error(
                        "Retained task budget history has inconsistent identity",
                    );
                }
                ToolResult::success(
                    json!({
                        "task_deleted": true,
                        "id": id,
                        "task_identity": task_identity,
                        "budget_resolutions": budget_resolutions,
                    })
                    .to_string(),
                )
            }
            Ok(None) => ToolResult::error(format!("Task #{id} not found")),
            Err(_) => ToolResult::error(format!("Failed to fetch task #{id}")),
        }
    }
}

#[cfg(test)]
mod deleted_audit_reader_tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::AgentIdentity;
    use crate::db::tasks::{CreateTaskInput, TasksDb};
    use serde_json::Value;

    async fn execute_registered(
        executor: &mut ToolExecutor,
        registry: &ToolRegistry,
        input: Value,
        call_id: &str,
    ) -> crate::agent::tool_trait::ToolResult {
        let tool = registry.get("get_task").expect("registered get_task tool");
        executor
            .execute_batch(vec![(tool, input, call_id.to_owned())])
            .await
            .expect("execute production tool batch")
            .pop()
            .expect("single tool result")
            .1
    }

    fn resolve_budget(db: &TasksDb, title: &str) -> crate::db::tasks::Task {
        let task = db
            .create(CreateTaskInput {
                title: title.into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "preserve deleted audit provenance".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        db.set_task_budget(task.id, Some(100), Some(1.0), Some(4))
            .expect("set task budget");
        db.update_task_budget_usage(task.id, 100, 1.0, 4)
            .expect("record task budget usage");
        db.mark_task_budget_exceeded(task.id, "budget reached")
            .expect("mark exceeded budget");
        db.resolve_exceeded_task_budget(
            task.id,
            200,
            2.0,
            8,
            "supervisor",
            "approved bounded continuation",
        )
        .expect("persist immutable budget resolution");
        task
    }

    #[tokio::test]
    async fn registered_executor_reads_deleted_and_cleared_audit_with_original_identity() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        let deleted_task = resolve_budget(&db, "deleted resolved task");
        let cleared_task = resolve_budget(&db, "cleared resolved task");
        db.delete(deleted_task.id).expect("delete resolved task");

        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "p4-39-deleted-audit".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );

        let default_missing = execute_registered(
            &mut executor,
            &registry,
            json!({"id": deleted_task.id}),
            "default-deleted",
        )
        .await;
        assert!(
            default_missing.is_error,
            "default behavior must remain an error"
        );

        let deleted_result = execute_registered(
            &mut executor,
            &registry,
            json!({"id": deleted_task.id, "include_deleted_audit": true}),
            "deleted-audit",
        )
        .await;
        assert!(!deleted_result.is_error, "{}", deleted_result.output);
        let deleted: Value = serde_json::from_str(&deleted_result.output).unwrap();
        assert!(uuid::Uuid::parse_str(&deleted_task.task_identity).is_ok());
        assert_eq!(deleted["task_deleted"], true);
        assert_eq!(deleted["id"], deleted_task.id);
        assert_eq!(deleted["task_identity"], deleted_task.task_identity);
        assert_eq!(deleted["budget_resolutions"][0]["task_id"], deleted_task.id);
        assert_eq!(
            deleted["budget_resolutions"][0]["task_identity"],
            deleted_task.task_identity
        );
        assert!(deleted.get("status").is_none());
        assert!(deleted.get("title").is_none());

        let no_history = execute_registered(
            &mut executor,
            &registry,
            json!({"id": i64::MAX, "include_deleted_audit": true}),
            "no-audit",
        )
        .await;
        assert!(
            no_history.is_error,
            "missing tasks without history remain errors"
        );

        db.clear_all()
            .expect("clear board after creating audit row");
        let cleared_result = execute_registered(
            &mut executor,
            &registry,
            json!({"id": cleared_task.id, "include_deleted_audit": true}),
            "cleared-audit",
        )
        .await;
        assert!(!cleared_result.is_error, "{}", cleared_result.output);
        let cleared: Value = serde_json::from_str(&cleared_result.output).unwrap();
        assert!(uuid::Uuid::parse_str(&cleared_task.task_identity).is_ok());
        assert_eq!(cleared["task_deleted"], true);
        assert_eq!(cleared["id"], cleared_task.id);
        assert_eq!(cleared["task_identity"], cleared_task.task_identity);
        assert_eq!(cleared["budget_resolutions"][0]["task_id"], cleared_task.id);
        assert_eq!(
            cleared["budget_resolutions"][0]["task_identity"],
            cleared_task.task_identity
        );
        assert!(cleared.get("status").is_none());
    }

    #[tokio::test]
    async fn active_task_output_includes_immutable_identity() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated task database");
        let task = db
            .create(CreateTaskInput {
                title: "active identity task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "return current immutable task identity".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "p4-39-active-identity".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        let result = execute_registered(
            &mut executor,
            &registry,
            json!({"id": task.id}),
            "active-task",
        )
        .await;
        assert!(!result.is_error, "{}", result.output);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output["id"], task.id);
        assert_eq!(output["task_identity"], task.task_identity);
        assert_eq!(output["status"], task.status.as_str());
    }
}
