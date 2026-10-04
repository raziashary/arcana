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
