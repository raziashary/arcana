use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::tool_trait::{Tool, ToolContext, ToolResult};

type ProgressEvents = Vec<(String, String, String)>;

fn read_operation_journal(
    run_id: Option<&str>,
    after_cursor: i64,
    limit: i64,
    read: impl FnOnce(&str, i64, i64) -> crate::errors::Result<crate::db::runs::OperationJournalPage>,
) -> Value {
    let Some(run_id) = run_id else {
        return json!({
            "state": "not_applicable",
            "entries": [],
            "next_cursor": after_cursor,
            "has_more": false,
        });
    };

    match read(run_id, after_cursor, limit) {
        Ok(page) => {
            let entries = page
                .entries
                .iter()
                .map(|entry| {
                    let outcome = match entry.outcome {
                        crate::db::runs::OperationOutcome::Ok => "ok",
                        crate::db::runs::OperationOutcome::Error => "error",
                        crate::db::runs::OperationOutcome::Unknown => "unknown",
                    };
                    let reconciliation = match entry.reconciliation {
                        crate::db::runs::OperationReconciliation::Unresolved => "unresolved",
                        crate::db::runs::OperationReconciliation::Reconciled => "reconciled",
                        crate::db::runs::OperationReconciliation::RetrySafe => "retry_safe",
                        crate::db::runs::OperationReconciliation::DoNotRetry => "do_not_retry",
                    };
                    json!({
                        "cursor": entry.cursor,
                        "run_id": entry.run_id,
                        "task_id": entry.task_id,
                        "operation": entry.operation,
                        "effect": entry.effect,
                        "target_paths": entry.target_paths,
                        "intent_at": entry.intent_at,
                        "outcome": outcome,
                        "changed_paths": entry.changed_paths,
                        "outcome_at": entry.outcome_at,
                        "reconciliation": reconciliation,
                        "reconciliation_actor": entry.reconciliation_actor,
                        "reconciliation_reason": entry.reconciliation_reason,
                        "retry_worker": entry.retry_worker,
                        "retry_grant_cursor": entry.retry_grant_cursor,
                        "retry_consumed_by_cursor": entry.retry_consumed_by_cursor,
                        "recovery_required": outcome == "unknown"
                            && reconciliation != "reconciled"
                            && entry.retry_consumed_by_cursor.is_none(),
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "state": "known",
                "entries": entries,
                "next_cursor": page.next_cursor,
                "has_more": page.has_more,
            })
        }
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "journal_store",
            };
            json!({
                "state": "unknown",
                "entries": Value::Null,
                "next_cursor": after_cursor,
                "has_more": Value::Null,
                "error_class": error_class,
            })
        }
    }
}

fn read_recent_progress(
    run_id: Option<&str>,
    read: impl FnOnce(&str, i64) -> crate::errors::Result<ProgressEvents>,
) -> (ProgressEvents, Value) {
    let Some(run_id) = run_id else {
        return (Vec::new(), json!({ "state": "not_applicable" }));
    };
    match read(run_id, 10) {
        Ok(events) => (events, json!({ "state": "known" })),
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "progress_store",
            };
            (
                Vec::new(),
                json!({ "state": "unknown", "error_class": error_class }),
            )
        }
    }
}

fn read_active_operation(
    run_id: Option<&str>,
    read: impl FnOnce(&str) -> crate::errors::Result<Option<crate::db::runs::ActiveWorkerOperation>>,
) -> Value {
    let Some(run_id) = run_id else {
        return json!({ "state": "not_applicable" });
    };
    match read(run_id) {
        Ok(Some(operation)) => json!({
            "state": "active",
            "operation": operation.operation,
            "started_at": operation.started_at,
            "elapsed_ms": operation.elapsed_ms,
        }),
        Ok(None) => json!({ "state": "idle" }),
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "run_store",
            };
            json!({ "state": "unknown", "error_class": error_class })
        }
    }
}

fn read_task_health(
    task_id: Option<i64>,
    db: &crate::errors::Result<crate::db::tasks::TasksDb>,
) -> Value {
    let Some(task_id) = task_id else {
        return json!({ "state": "not_applicable" });
    };
    let db = match db {
        Ok(db) => db,
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "task_store",
            };
            return json!({ "state": "unknown", "error_class": error_class });
        }
    };
    match db.get_by_id(task_id) {
        Ok(Some(task)) => json!({
            "state": "known",
            "status": task.status.as_str(),
            "retry_count": task.retry_count,
            "assigned_worker": task.assigned_worker,
            "epic_id": task.epic_id,
            "phase": task.phase,
            "phase_order": task.phase_order,
            "blocked_by": task.blocked_by,
            "budget_exceeded": task.budget_exceeded,
        }),
        Ok(None) => json!({ "state": "not_found" }),
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "task_store",
            };
            json!({ "state": "unknown", "error_class": error_class })
        }
    }
}

fn read_session_usage(
    session_id: Option<&str>,
    analytics: &crate::errors::Result<crate::analytics::db::AnalyticsDb>,
) -> Value {
    let Some(session_id) = session_id else {
        return json!({ "state": "not_applicable" });
    };
    let analytics = match analytics {
        Ok(analytics) => analytics,
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "analytics_store",
            };
            return json!({ "state": "unknown", "error_class": error_class });
        }
    };
    match analytics.get_token_usage_for_session(session_id) {
        Ok(summary) => json!({
            "state": "known",
            "request_count": summary.request_count,
            "prompt_tokens": summary.prompt_tokens,
            "completion_tokens": summary.completion_tokens,
            "total_tokens": summary.total_tokens,
            "thinking_tokens": summary.thinking_tokens,
            "cost_usd": summary.cost_usd,
        }),
        Err(error) => {
            let error_class = match error {
                crate::errors::ArcanaError::Database(_) => "database",
                crate::errors::ArcanaError::Io(_) => "io",
                _ => "analytics_store",
            };
            json!({ "state": "unknown", "error_class": error_class })
        }
    }
}

pub struct ListWorkerHealthTool;

#[async_trait]
impl Tool for ListWorkerHealthTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "list_worker_health"
    }

    fn description(&self) -> &str {
        "Read the fleet's durable worker health, latest run phase, current task, last safe completed operation, heartbeat, and failure code. Use this before reassigning or retrying work so you can reconcile task and workspace state first. Available only to the Supervisor."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 50 },
                "journal_limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 },
                "journal_cursors": {
                    "type": "object",
                    "description": "Per-run next cursors returned by prior pages; cursors are independent for each worker run.",
                    "additionalProperties": { "type": "integer", "minimum": 0 }
                }
            }
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        if ctx.agent_role.trim() != "supervisor" {
            return ToolResult::error("list_worker_health is restricted to the Supervisor");
        }
        let limit = input["limit"].as_i64().unwrap_or(50).clamp(1, 100);
        let journal_limit = input["journal_limit"].as_i64().unwrap_or(20).clamp(1, 50);
        let journal_cursors = input.get("journal_cursors").and_then(Value::as_object);
        let workers = match crate::db::workers::WorkersDb::open().and_then(|db| db.list()) {
            Ok(workers) => workers,
            Err(error) => {
                return ToolResult::error(format!("Could not read worker roster: {error}"))
            }
        };
        let runs_db = match crate::db::runs::RunsDb::open() {
            Ok(db) => db,
            Err(error) => {
                return ToolResult::error(format!("Could not open worker run health: {error}"))
            }
        };
        let runs = match runs_db.latest_worker_ticks(limit) {
            Ok(runs) => runs,
            Err(error) => {
                return ToolResult::error(format!("Could not read worker run health: {error}"))
            }
        };
        let latest: std::collections::HashMap<_, _> = runs
            .into_iter()
            .map(|run| (run.worker_slug.clone(), run))
            .collect();
        let analytics_db = if latest.is_empty() {
            Err(crate::errors::ArcanaError::Agent(
                "No worker runs are available".into(),
            ))
        } else {
            crate::analytics::db::AnalyticsDb::new().await
        };
        let tasks_db = crate::db::tasks::TasksDb::open();
        let workers = workers
            .into_iter()
            .take(limit as usize)
            .map(|worker| {
                let run = latest.get(&worker.slug);
                let (progress_events, progress_state) = read_recent_progress(
                    run.map(|run| run.run_id.as_str()),
                    |run_id, count| runs_db.recent_worker_progress(run_id, count),
                );
                let active_operation = read_active_operation(
                    run.map(|run| run.run_id.as_str()),
                    |run_id| runs_db.active_worker_operation(run_id),
                );
                let task_health = read_task_health(run.and_then(|run| run.task_id), &tasks_db);
                let token_usage = read_session_usage(
                    run.map(|run| run.session_id.as_str()),
                    &analytics_db,
                );
                let operation_journal = read_operation_journal(
                    run.map(|run| run.run_id.as_str()),
                    run.and_then(|run| {
                        journal_cursors
                            .and_then(|cursors| cursors.get(&run.run_id))
                            .and_then(Value::as_i64)
                    })
                    .unwrap_or(0)
                    .max(0),
                    journal_limit,
                    |run_id, after_cursor, count| {
                        runs_db.operation_journal_page(run_id, after_cursor, count)
                    },
                );
                let checkpoints = match run {
                    Some(run) => match crate::git::checkpoint::list(
                        None,
                        Some(&run.session_id),
                        run.task_id,
                    ) {
                        Ok(checkpoints) => json!({
                            "state": "known",
                            "count": checkpoints.len(),
                            "latest_at_ms": checkpoints.first().map(|checkpoint| checkpoint.created_ms),
                        }),
                        Err(error) => json!({
                            "state": "unknown",
                            "error_class": match error {
                                crate::errors::ArcanaError::Database(_) => "database",
                                crate::errors::ArcanaError::Io(_) => "io",
                                _ => "checkpoint_store",
                            },
                        }),
                    },
                    None => json!({ "state": "not_applicable", "count": 0 }),
                };
                json!({
                    "slug": worker.slug,
                    "status": worker.status.as_str(),
                    "model": worker.model,
                    "last_seen": worker.last_seen,
                    "run": run.map(|run| json!({
                        "run_id": run.run_id,
                        "phase": run.phase,
                        "task_id": run.task_id,
                        "task": task_health,
                        "token_usage": token_usage,
                        "heartbeat_at": run.heartbeat_at,
                        "last_progress_at": run.last_progress_at,
                        "last_progress": run.last_progress,
                        "active_operation": active_operation,
                        "recent_progress_state": progress_state,
                        "operation_journal": operation_journal,
                        "recent_progress": progress_events.iter().map(|(at, operation, outcome)| json!({
                            "at": at,
                            "operation": operation,
                            "outcome": outcome,
                        })).collect::<Vec<_>>(),
                        "checkpoints": checkpoints,
                        "failure_code": run.failure_code,
                        "startup_retry_count": run.startup_retry_count,
                        "outcome": run.outcome,
                        "started_at": run.started_at,
                    }))
                })
            })
            .collect::<Vec<_>>();
        let count = workers.len();
        ToolResult::success(json!({
            "schema_version": 1,
            "workers": workers,
            "count": count,
            "guidance": "Before retry or reassignment, inspect task status, current workspace and Git state, and checkpoints. Reconcile every unknown outcome or unresolved journal entry before retry or reassignment. Never automatically replay a journaled operation."
        }).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tool_trait::AgentIdentity;

    #[test]
    fn progress_read_failure_is_reported_as_unknown_not_empty_history() {
        let (events, state) = read_recent_progress(Some("run-health"), |_, _| {
            Err(crate::errors::ArcanaError::Database(
                rusqlite::Error::InvalidQuery,
            ))
        });

        assert!(events.is_empty());
        assert_eq!(state["state"], "unknown");
        assert_eq!(state["error_class"], "database");
    }

    #[test]
    fn operation_journal_read_failure_is_unknown_without_database_details() {
        let journal = read_operation_journal(Some("run-health"), 7, 1, |_, _, _| {
            Err(crate::errors::ArcanaError::Database(
                rusqlite::Error::InvalidQuery,
            ))
        });

        assert_eq!(journal["state"], "unknown");
        assert_eq!(journal["error_class"], "database");
        assert!(journal["entries"].is_null());
        assert_eq!(journal["next_cursor"], 7);
        assert!(journal["has_more"].is_null());
        assert!(!journal.to_string().contains("InvalidQuery"));
    }

    #[test]
    fn task_health_reports_retry_and_epic_relationship_without_task_text() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let tasks = crate::db::tasks::TasksDb::open().expect("open task database");
        let epic = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "private-epic-title-marker".into(),
                description: Some("private-epic-description-marker".into()),
                priority: "p1".into(),
                task_type: "epic".into(),
                acceptance_criteria: "private-epic-criteria-marker".into(),
                created_by: Some("supervisor".into()),
            })
            .expect("create parent epic");
        let task = tasks
            .create_phased_task(crate::db::tasks::CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "implementation".into(),
                phase_order: 2,
                title: "private-task-title-marker".into(),
                description: Some("private-task-description-marker".into()),
                priority: "p1".into(),
                task_type: "feature".into(),
                acceptance_criteria: "private-task-criteria-marker".into(),
                blocked_by: vec!["17".into()],
                milestone: Some("private-milestone-marker".into()),
                created_by: Some("supervisor".into()),
            })
            .expect("create phased child");
        tasks.increment_retry(task.id).expect("record retry one");
        tasks.increment_retry(task.id).expect("record retry two");

        let health = read_task_health(Some(task.id), &Ok(tasks));
        assert_eq!(health["state"], "known");
        assert_eq!(health["status"], "needs_info");
        assert_eq!(health["retry_count"], 2);
        assert_eq!(health["epic_id"], epic.id.to_string());
        assert_eq!(health["phase"], "implementation");
        assert_eq!(health["phase_order"], 2);
        assert_eq!(health["blocked_by"][0], "17");
        let serialized = health.to_string();
        for private in [
            "private-epic-title-marker",
            "private-epic-description-marker",
            "private-epic-criteria-marker",
            "private-task-title-marker",
            "private-task-description-marker",
            "private-task-criteria-marker",
            "private-milestone-marker",
        ] {
            assert!(
                !serialized.contains(private),
                "task health leaked {private}"
            );
        }
    }

    #[test]
    fn task_health_store_failure_is_unknown_without_database_details() {
        let unavailable = Err(crate::errors::ArcanaError::Database(
            rusqlite::Error::InvalidQuery,
        ));
        let health = read_task_health(Some(42), &unavailable);
        assert_eq!(health["state"], "unknown");
        assert_eq!(health["error_class"], "database");
        assert!(!health.to_string().contains("InvalidQuery"));
        assert_eq!(
            read_task_health(None, &unavailable)["state"],
            "not_applicable"
        );
    }

    #[tokio::test]
    async fn session_usage_failure_is_unknown_without_database_details_or_session_id() {
        let unavailable = Err(crate::errors::ArcanaError::Database(
            rusqlite::Error::InvalidQuery,
        ));
        let usage = read_session_usage(Some("private-session-marker"), &unavailable);
        assert_eq!(usage["state"], "unknown");
        assert_eq!(usage["error_class"], "database");
        assert!(!usage.to_string().contains("InvalidQuery"));
        assert!(!usage.to_string().contains("private-session-marker"));
        assert_eq!(
            read_session_usage(None, &unavailable)["state"],
            "not_applicable"
        );
    }

    #[tokio::test]
    async fn supervisor_reads_roster_and_latest_worker_run_state() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let worker = workers
            .get_by_slug("worker-backend")
            .expect("query")
            .expect("seed worker");
        let runs = crate::db::runs::RunsDb::open().expect("open runs DB");
        let run_id = runs
            .begin_worker_tick(&worker.slug, "test-session")
            .expect("begin run");
        runs.record_worker_progress(&run_id, Some(42), "tool_project_command_ok")
            .expect("record progress");
        let completed_cursor = runs
            .record_operation_intent(
                &run_id,
                Some(42),
                "reconciled-operation-id",
                "write_file",
                "write_file",
                &["src/result.rs".to_string()],
            )
            .expect("append completed operation intent");
        runs.record_operation_outcome(
            &run_id,
            completed_cursor,
            crate::db::runs::OperationOutcome::Unknown,
            &["src/result.rs".to_string()],
        )
        .expect("append completed operation outcome");
        runs.record_operation_reconciliation(
            &run_id,
            completed_cursor,
            crate::db::runs::OperationReconciliation::Reconciled,
        )
        .expect("append completed operation reconciliation");
        let unresolved_cursor = runs
            .record_operation_intent(
                &run_id,
                Some(42),
                "private-operation-id-that-must-not-be-exposed",
                "write_file",
                "write_file",
                &["src/generated.rs".to_string()],
            )
            .expect("append interrupted operation intent");
        let analytics = crate::analytics::db::AnalyticsDb::new()
            .await
            .expect("open isolated analytics database");
        analytics
            .insert_token_usage(&crate::analytics::types::TokenUsageRecord {
                id: None,
                timestamp: chrono::Utc::now(),
                session_id: "test-session".into(),
                agent_type: "worker".into(),
                provider: "google".into(),
                model: "gemini-3.8-flash".into(),
                prompt_tokens: 101,
                completion_tokens: 37,
                total_tokens: 138,
                thinking_tokens: Some(9),
                cost_usd: 0.002,
                request_type: "chat".into(),
            })
            .expect("persist worker provider usage");
        analytics
            .insert_token_usage(&crate::analytics::types::TokenUsageRecord {
                id: None,
                timestamp: chrono::Utc::now(),
                session_id: "other-session".into(),
                agent_type: "worker".into(),
                provider: "google".into(),
                model: "gemini-3.8-flash".into(),
                prompt_tokens: 999,
                completion_tokens: 999,
                total_tokens: 1998,
                thinking_tokens: None,
                cost_usd: 1.0,
                request_type: "chat".into(),
            })
            .expect("persist unrelated session usage");

        let ctx = ToolContext {
            session_id: "supervisor-session".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-health".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,

            run_token_budget: None,
        };
        let result = ListWorkerHealthTool
            .execute(json!({ "journal_limit": 1 }), &ctx)
            .await;
        assert!(!result.is_error, "{}", result.output);
        let output: Value =
            serde_json::from_str(&result.output).expect("structured health response");
        let worker = output["workers"]
            .as_array()
            .expect("worker list")
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .expect("backend worker");
        assert_eq!(worker["run"]["run_id"], run_id);
        assert_eq!(worker["model"], "gemini-3.8-flash");
        assert_eq!(worker["run"]["task"]["state"], "not_found");
        assert_eq!(worker["run"]["token_usage"]["state"], "known");
        assert_eq!(worker["run"]["token_usage"]["request_count"], 1);
        assert_eq!(worker["run"]["token_usage"]["prompt_tokens"], 101);
        assert_eq!(worker["run"]["token_usage"]["completion_tokens"], 37);
        assert_eq!(worker["run"]["token_usage"]["total_tokens"], 138);
        assert_eq!(worker["run"]["token_usage"]["thinking_tokens"], 9);
        assert_eq!(worker["run"]["token_usage"]["cost_usd"], 0.002);
        assert_eq!(worker["run"]["task_id"], 42);
        assert_eq!(worker["run"]["last_progress"], "tool_project_command_ok");
        assert_eq!(worker["run"]["active_operation"]["state"], "active");
        assert_eq!(worker["run"]["active_operation"]["operation"], "write_file");
        assert!(worker["run"]["active_operation"]["elapsed_ms"]
            .as_u64()
            .is_some());
        assert_eq!(worker["run"]["checkpoints"]["state"], "known");
        let journal = &worker["run"]["operation_journal"];
        assert_eq!(journal["state"], "known");
        assert_eq!(journal["entries"].as_array().unwrap().len(), 1);
        assert_eq!(journal["entries"][0]["cursor"], completed_cursor);
        assert_eq!(journal["entries"][0]["run_id"], run_id);
        assert_eq!(journal["entries"][0]["task_id"], 42);
        assert_eq!(journal["entries"][0]["outcome"], "unknown");
        assert_eq!(journal["entries"][0]["reconciliation"], "reconciled");
        assert_eq!(journal["entries"][0]["reconciliation_actor"], "supervisor");
        assert_eq!(
            journal["entries"][0]["reconciliation_reason"],
            "effect_verified_complete"
        );
        assert_eq!(journal["entries"][0]["recovery_required"], false);
        assert_eq!(journal["entries"][0]["target_paths"][0], "src/result.rs");
        assert_eq!(journal["has_more"], true);
        let first_page_cursor = journal["next_cursor"].as_i64().expect("page cursor");
        assert!(!output
            .to_string()
            .contains("private-operation-id-that-must-not-be-exposed"));
        assert!(!output.to_string().contains("test-session"));
        assert!(!output.to_string().contains("other-session"));
        assert!(!output.to_string().contains("1998"));
        assert!(!output
            .to_string()
            .contains(home.path().to_string_lossy().as_ref()));

        let next_result = ListWorkerHealthTool
            .execute(
                json!({
                    "journal_limit": 1,
                    "journal_cursors": { run_id.clone(): first_page_cursor }
                }),
                &ctx,
            )
            .await;
        assert!(!next_result.is_error, "{}", next_result.output);
        let next_output: Value = serde_json::from_str(&next_result.output).expect("next page JSON");
        let next_worker = next_output["workers"]
            .as_array()
            .expect("worker list")
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .expect("backend worker");
        let unresolved = &next_worker["run"]["operation_journal"];
        assert_eq!(unresolved["entries"].as_array().unwrap().len(), 1);
        assert_eq!(unresolved["entries"][0]["cursor"], unresolved_cursor);
        assert_eq!(unresolved["entries"][0]["outcome"], "unknown");
        assert_eq!(unresolved["entries"][0]["reconciliation"], "unresolved");
        assert_eq!(unresolved["entries"][0]["recovery_required"], true);
        assert_eq!(
            unresolved["entries"][0]["target_paths"][0],
            "src/generated.rs"
        );
        assert_eq!(unresolved["has_more"], false);

        assert!(runs
            .active_worker_operation(&run_id)
            .expect("read remaining active operation")
            .is_some());
        runs.record_operation_outcome(
            &run_id,
            unresolved_cursor,
            crate::db::runs::OperationOutcome::Unknown,
            &[],
        )
        .expect("finish active intent as unknown");
        assert!(runs
            .active_worker_operation(&run_id)
            .expect("read closed active operation")
            .is_none());
    }

    #[tokio::test]
    async fn journal_cursors_are_independent_per_worker_run() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let backend = workers
            .get_by_slug("worker-backend")
            .expect("query backend")
            .expect("backend worker");
        let frontend = workers
            .get_by_slug("worker-frontend")
            .expect("query frontend")
            .expect("frontend worker");
        let runs = crate::db::runs::RunsDb::open().expect("open runs DB");
        let backend_run = runs
            .begin_worker_tick(&backend.slug, "backend-session")
            .expect("begin backend run");
        let backend_first = runs
            .record_operation_intent(&backend_run, Some(1), "b-1", "write", "write_file", &[])
            .expect("append first backend intent");
        runs.record_operation_outcome(
            &backend_run,
            backend_first,
            crate::db::runs::OperationOutcome::Unknown,
            &[],
        )
        .expect("record an ambiguous first backend intent");
        runs.record_operation_reconciliation(
            &backend_run,
            backend_first,
            crate::db::runs::OperationReconciliation::Reconciled,
        )
        .expect("reconcile first backend intent");
        let frontend_run = runs
            .begin_worker_tick(&frontend.slug, "frontend-session")
            .expect("begin frontend run");
        let frontend_first = runs
            .record_operation_intent(&frontend_run, Some(2), "f-1", "write", "write_file", &[])
            .expect("append first frontend intent");
        let backend_second = runs
            .record_operation_intent(&backend_run, Some(1), "b-2", "edit", "edit_file", &[])
            .expect("append second backend intent");

        let ctx = ToolContext {
            session_id: "supervisor-session".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-journal-cursors".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,

            run_token_budget: None,
        };
        let result = ListWorkerHealthTool
            .execute(
                json!({
                    "journal_limit": 1,
                    "journal_cursors": { (backend_run.clone()): backend_first }
                }),
                &ctx,
            )
            .await;
        assert!(!result.is_error, "{}", result.output);
        let output: Value = serde_json::from_str(&result.output).expect("health JSON");
        let workers = output["workers"].as_array().expect("worker array");
        let backend_page = workers
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .expect("backend state")["run"]["operation_journal"]
            .clone();
        let frontend_page = workers
            .iter()
            .find(|worker| worker["slug"] == "worker-frontend")
            .expect("frontend state")["run"]["operation_journal"]
            .clone();
        assert_eq!(backend_page["entries"][0]["cursor"], backend_second);
        assert_eq!(frontend_page["entries"][0]["cursor"], frontend_first);
        assert_eq!(backend_page["next_cursor"], backend_second);
        assert_eq!(frontend_page["next_cursor"], frontend_first);
    }

    #[tokio::test]
    async fn non_supervisor_cannot_read_fleet_health() {
        let ctx = ToolContext {
            session_id: "worker-session".into(),
            working_dir: ".".into(),
            call_id: "call-health".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = ListWorkerHealthTool.execute(json!({}), &ctx).await;
        assert!(result.is_error);
        let output: Value = serde_json::from_str(&result.output).expect("structured failure");
        assert_eq!(output["error"]["code"], "tool_operation_failed");
    }

    #[tokio::test]
    async fn checkpoint_store_failure_is_reported_as_unknown_not_empty() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let worker = workers
            .get_by_slug("worker-backend")
            .expect("query")
            .expect("seed worker");
        let runs = crate::db::runs::RunsDb::open().expect("open runs DB");
        runs.begin_worker_tick(&worker.slug, "checkpoint-failure-session")
            .expect("begin run");
        std::fs::create_dir_all(home.path().join(".arcana/config.db"))
            .expect("make checkpoint DB unavailable");

        let ctx = ToolContext {
            session_id: "supervisor-session".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-health".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,

            run_token_budget: None,
        };
        let result = ListWorkerHealthTool.execute(json!({}), &ctx).await;
        assert!(
            !result.is_error,
            "health data remains available: {}",
            result.output
        );
        let output: Value = serde_json::from_str(&result.output).expect("health JSON");
        let worker = output["workers"]
            .as_array()
            .expect("worker list")
            .iter()
            .find(|worker| worker["slug"] == "worker-backend")
            .expect("backend worker");
        assert_eq!(worker["run"]["checkpoints"]["state"], "unknown");
        assert!(worker["run"]["checkpoints"]["count"].is_null());
    }
}
