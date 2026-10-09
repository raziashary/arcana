// `get_memory` — fetch one memory by id (`P3-39`).
//
// The third of the gateway's four surfaces. It exists so an agent can follow up
// on something `search_memory` returned in truncated form without widening its
// search, and so provenance is inspectable: *"where did I learn this?"* is a
// question the system should be able to answer about itself.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct GetMemoryTool;

#[async_trait]
impl Tool for GetMemoryTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "get_memory"
    }

    fn description(&self) -> &str {
        "Fetch one memory in full by its id, with its provenance — who recorded \
         it, how confident they were, and the file it came from where one \
         applies. Use it to follow up on a truncated search result.\n\n\
         Only memories in scope for this workspace are readable; an id from \
         another project returns not-found rather than its contents."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The memory id, as returned by search_memory or list_memories."
                }
            },
            "required": ["id"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let mut timing = crate::observability::timing::OperationTimer::start("memory_tool_get")
            .with_database("memory")
            .with_session_id(&ctx.session_id)
            .with_task_id(ctx.task_id)
            .with_call_context(&ctx.call_id, &ctx.agent_role);
        if let Some(run_id) = ctx.run_id.as_deref() {
            timing = timing.with_run_id(run_id);
        }
        let id = match input["id"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'id' is required"),
        };

        let db = match crate::memory::db::MemoryDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("memory unavailable: {e}")),
        };

        // ⚠ Scope is enforced HERE too, not only in search. A tool that could
        // fetch any id by guessing it would be a way around the filter the
        // recall path enforces — another project's memory through a different
        // door.
        let scopes = crate::memory::recall::scope_keys_for(&ctx.working_dir);
        let placeholders = (0..scopes.len())
            .map(|i| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT content, tier, scope, scope_ref, source, source_file, source_line,
                    confidence, importance, access_count, created_at
             FROM memories WHERE id = ?1 AND scope_key IN ({placeholders})"
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(id.clone())];
        for s in &scopes {
            params.push(Box::new(s.clone()));
        }
        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

        let row = db.conn.query_row(sql.as_str(), refs.as_slice(), |r| {
            Ok(json!({
                "id": id,
                "content": r.get::<_, String>(0)?,
                "tier": r.get::<_, String>(1)?,
                "scope": r.get::<_, String>(2)?,
                "scope_ref": r.get::<_, String>(3)?,
                "source": r.get::<_, String>(4)?,
                "source_file": r.get::<_, Option<String>>(5)?,
                "source_line": r.get::<_, Option<i64>>(6)?,
                "confidence": r.get::<_, f64>(7)?,
                "importance": r.get::<_, f64>(8)?,
                "access_count": r.get::<_, i64>(9)?,
                "created_at": r.get::<_, String>(10)?,
            }))
        });

        match row {
            Ok(v) => {
                timing.with_result_count(1).succeed();
                ToolResult::success(v.to_string())
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                timing.with_result_count(0).set_outcome("not_found");
                ToolResult::error(format!("No memory {id} is readable from this workspace."))
            }
            Err(e) => ToolResult::error(format!("Failed to read memory: {e}")),
        }
    }
}

#[cfg(test)]
mod observability_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn production_get_memory_emits_correlated_count_without_memory_content() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = crate::memory::db::MemoryDb::open().expect("open migrated MemoryDb");
        db.conn
            .execute(
                "INSERT INTO memories (id, tier, scope, scope_ref, scope_key, content, source, created_at, updated_at)
                 VALUES ('private-id', 'semantic', 'global', '', 'global', 'private memory contents', 'test', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                [],
            )
            .expect("seed scoped memory");
        drop(db);

        let context = ToolContext {
            session_id: "session-fixture".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-fixture".into(),
            run_id: Some("run-fixture".into()),
            task_id: Some(17),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(GetMemoryTool.execute(json!({"id":"private-id"}), &context))
        });
        assert!(!result.is_error);
        assert!(result.output.contains("private memory contents"));
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_tool_get")
            .expect("production tool operation event");
        assert_eq!(event["fields"]["database"], "memory");
        assert_eq!(event["fields"]["run_id"], "run-fixture");
        assert_eq!(event["fields"]["task_id"], "17");
        assert_eq!(event["fields"]["result_count"], 1);
        let telemetry = serde_json::to_string(&events).expect("serialize safe events");
        assert!(!telemetry.contains("private memory contents"));
        assert!(!telemetry.contains("private-id"));
    }

    #[test]
    fn production_get_memory_marks_not_found_as_an_observed_zero() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        crate::memory::db::MemoryDb::open().expect("open migrated MemoryDb");
        let context = ToolContext {
            session_id: "session-fixture".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-fixture".into(),
            run_id: Some("run-fixture".into()),
            task_id: Some(19),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(GetMemoryTool.execute(json!({"id":"absent-id"}), &context))
        });

        assert!(result.is_error);
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_tool_get")
            .expect("production not-found operation event");
        assert_eq!(event["fields"]["outcome"], "not_found");
        assert_eq!(event["fields"]["result_count"], 0);
        assert_eq!(event["fields"]["result_count_observed"], true);
    }
}

#[cfg(test)]
mod p4_51_memory_executor_matrix_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::{AgentIdentity, ToolResult};
    use serde_json::{json, Value};

    const SESSION_ID: &str = "p4-51-memory-matrix-session";
    const AGENT_ID: &str = "worker-memory-matrix";
    const TASK_ID: i64 = 51251;

    fn invoke(
        runtime: &tokio::runtime::Runtime,
        executor: &mut ToolExecutor,
        registry: &ToolRegistry,
        name: &str,
        input: Value,
        use_id: &str,
    ) -> ToolResult {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("FastDevelopment registry omitted {name}"));
        let mut results = runtime
            .block_on(executor.execute_batch(vec![(tool, input, use_id.to_string())]))
            .expect("production ToolExecutor should return the completed batch");
        assert_eq!(
            results.len(),
            1,
            "one registered call should yield one result"
        );
        let (returned_use_id, result) = results.pop().expect("one tool result");
        assert_eq!(returned_use_id, use_id, "the model call id must survive");
        result
    }

    fn assert_bounded_correlated_failure(
        result: &ToolResult,
        name: &str,
        use_id: &str,
        run_id: &str,
        planted: &[&str],
    ) {
        assert!(result.is_error, "{name} failure was returned as success");
        assert!(
            result.output.len() <= 4096,
            "{name} envelope exceeded 4 KiB"
        );
        let envelope: Value =
            serde_json::from_str(&result.output).expect("failed tool result must be JSON");
        assert_eq!(envelope["schema_version"], 1, "{name}");
        assert_eq!(envelope["ok"], false, "{name}");
        assert_eq!(envelope["tool"], name, "{name}");
        assert_eq!(envelope["operation"], format!("tool.{name}"), "{name}");
        assert_eq!(envelope["error"]["tool"], name, "{name}");
        assert_eq!(envelope["error"]["operation"], format!("tool.{name}"));
        assert_eq!(envelope["error"]["error_code"], envelope["error_code"]);
        assert_eq!(envelope["error"]["category"], envelope["category"]);
        assert!(envelope["error_code"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(envelope["category"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(envelope["message"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(envelope["cause"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(envelope["retryable"].is_boolean());
        assert!(envelope["retry_after_ms"].is_null() || envelope["retry_after_ms"].is_u64());
        assert!(envelope["suggested_action"]
            .as_str()
            .is_some_and(|s| !s.is_empty()));
        assert!(envelope["message"].as_str().unwrap().len() <= 512);
        assert!(envelope["cause"].as_str().unwrap().len() <= 512);
        assert!(envelope["suggested_action"].as_str().unwrap().len() <= 512);

        let timestamp = envelope["timestamp"].as_str().expect("timestamp");
        chrono::DateTime::parse_from_rfc3339(timestamp).expect("RFC3339 timestamp");
        assert_eq!(envelope["run_id"], run_id);
        assert_eq!(envelope["agent_id"], AGENT_ID);
        assert_eq!(envelope["task_id"], TASK_ID);
        assert_eq!(envelope["context"]["session_id"], SESSION_ID);
        assert_eq!(envelope["context"]["run_id"], run_id);
        assert_eq!(envelope["context"]["agent_id"], AGENT_ID);
        assert_eq!(envelope["context"]["task_id"], TASK_ID);
        assert!(envelope["run_id"].as_str().unwrap().len() <= 128);
        assert!(envelope["agent_id"].as_str().unwrap().len() <= 80);
        assert!(envelope["context"]["session_id"].as_str().unwrap().len() <= 128);
        assert!(envelope["context"]["tool_call_id"].as_str().unwrap().len() <= 128);
        assert_eq!(
            envelope["context"]["tool_call_id"],
            format!("{SESSION_ID}-{use_id}")
        );
        for marker in planted {
            assert!(
                !result.output.contains(marker),
                "{name} reflected planted private value {marker:?}: {}",
                result.output
            );
        }
        for success_field in ["results", "memories", "count", "id"] {
            assert!(
                envelope.get(success_field).is_none(),
                "{name} failure carried success-shaped field {success_field}"
            );
        }
    }

    fn planted_input_fields() -> Value {
        json!({
            "api_key": "P4_51_PRIVATE_CREDENTIAL_CANARY",
            "prompt": "P4_51_PRIVATE_PROMPT_CANARY",
            "source_text": "P4_51_PRIVATE_SOURCE_CANARY",
            "raw_argument": "P4_51_PRIVATE_ARGUMENT_CANARY",
            "long_os_error": format!(
                "P4_51_PRIVATE_OS_ERROR_CANARY {}",
                "Windows error detail ".repeat(64)
            ),
        })
    }

    #[test]
    fn memory_registered_executor_success_failure_matrix() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let workspace = tempfile::tempdir().expect("isolated memory-tool workspace");
        let _home_guard = crate::db::test_support::with_home(home.path());

        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        for name in [
            "search_memory",
            "get_memory",
            "list_memories",
            "propose_memory",
        ] {
            assert!(
                registry.get(name).is_some(),
                "missing real registration: {name}"
            );
        }

        // The executor's operation journal requires a real run row. It is
        // created under the redirected home, alongside the isolated memory DB.
        let run_id = crate::db::runs::RunsDb::open()
            .expect("open isolated RunsDb")
            .begin(crate::db::runs::kind::AGENT_TURN, Some(AGENT_ID))
            .expect("create isolated agent run");
        let identity = AgentIdentity::worker(AGENT_ID);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            SESSION_ID.to_string(),
            workspace.path().to_string_lossy().into_owned(),
            identity,
        )
        .with_run_id(Some(run_id.clone()));
        executor.set_task_id(Some(TASK_ID));
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");

        // Exercise the registered writer, then all three registered reader
        // surfaces against its persisted result.
        let content =
            "Synthetic matrix memory records a copper lantern beside the northern archive.";
        let proposal = invoke(
            &runtime,
            &mut executor,
            &registry,
            "propose_memory",
            json!({"content": content, "scope": "project", "confidence": 0.8}),
            "memory-propose-success",
        );
        assert!(!proposal.is_error, "proposal failed: {}", proposal.output);
        let memory_id = proposal
            .output
            .strip_prefix("Stored as ")
            .and_then(|rest| rest.split_whitespace().next())
            .expect("proposal output should return the persisted memory ID")
            .to_string();
        assert!(
            memory_id.starts_with("mem-"),
            "unexpected memory id {memory_id}"
        );

        let fetched = invoke(
            &runtime,
            &mut executor,
            &registry,
            "get_memory",
            json!({"id": memory_id.clone()}),
            "memory-get-success",
        );
        assert!(!fetched.is_error, "get_memory failed: {}", fetched.output);
        let fetched_json: Value = serde_json::from_str(&fetched.output).expect("get JSON");
        assert_eq!(fetched_json["content"], content);
        assert_eq!(fetched_json["source"], "agent_observation");

        let listed = invoke(
            &runtime,
            &mut executor,
            &registry,
            "list_memories",
            json!({"limit": 10, "tier": "semantic"}),
            "memory-list-success",
        );
        assert!(!listed.is_error, "list_memories failed: {}", listed.output);
        let listed_json: Value = serde_json::from_str(&listed.output).expect("list JSON");
        assert!(listed_json["memories"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == memory_id)));

        let searched = invoke(
            &runtime,
            &mut executor,
            &registry,
            "search_memory",
            json!({"query": "copper lantern northern archive", "tier": "semantic"}),
            "memory-search-success",
        );
        assert!(
            !searched.is_error,
            "search_memory failed: {}",
            searched.output
        );
        let searched_json: Value = serde_json::from_str(&searched.output).expect("search JSON");
        assert!(searched_json["results"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == memory_id)));
        assert!(searched.output.contains(content));

        // Three tools reject malformed required values before touching the
        // database. List has no required input, so its deterministic failure
        // comes from a schema obstruction in this throwaway database.
        let planted = [
            "P4_51_PRIVATE_CREDENTIAL_CANARY",
            "P4_51_PRIVATE_PROMPT_CANARY",
            "P4_51_PRIVATE_SOURCE_CANARY",
            "P4_51_PRIVATE_ARGUMENT_CANARY",
            "P4_51_PRIVATE_OS_ERROR_CANARY",
        ];
        let mut search_input = planted_input_fields();
        search_input["query"] = json!({"invalid": "P4_51_PRIVATE_ARGUMENT_CANARY"});
        let search_failure = invoke(
            &runtime,
            &mut executor,
            &registry,
            "search_memory",
            search_input,
            "memory-search-malformed",
        );
        assert_bounded_correlated_failure(
            &search_failure,
            "search_memory",
            "memory-search-malformed",
            &run_id,
            &planted,
        );

        let mut get_input = planted_input_fields();
        get_input["id"] = json!({"invalid": "P4_51_PRIVATE_ARGUMENT_CANARY"});
        let get_failure = invoke(
            &runtime,
            &mut executor,
            &registry,
            "get_memory",
            get_input,
            "memory-get-malformed",
        );
        assert_bounded_correlated_failure(
            &get_failure,
            "get_memory",
            "memory-get-malformed",
            &run_id,
            &planted,
        );

        let mut propose_input = planted_input_fields();
        propose_input["content"] = json!({"invalid": "P4_51_PRIVATE_ARGUMENT_CANARY"});
        let propose_failure = invoke(
            &runtime,
            &mut executor,
            &registry,
            "propose_memory",
            propose_input,
            "memory-propose-malformed",
        );
        assert_bounded_correlated_failure(
            &propose_failure,
            "propose_memory",
            "memory-propose-malformed",
            &run_id,
            &planted,
        );

        let db = crate::memory::db::MemoryDb::open().expect("reopen only the isolated MemoryDb");
        db.conn
            .execute_batch("DROP TABLE memories;")
            .expect("obstruct only the isolated memory table");
        drop(db);
        let list_failure = invoke(
            &runtime,
            &mut executor,
            &registry,
            "list_memories",
            planted_input_fields(),
            "memory-list-database-failure",
        );
        assert_bounded_correlated_failure(
            &list_failure,
            "list_memories",
            "memory-list-database-failure",
            &run_id,
            &planted,
        );
    }
}
