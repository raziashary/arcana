// `list_memories` — enumerate what is remembered (`P3-39`).
//
// The fourth gateway surface. `search_memory` answers *"what is relevant to
// this?"*; this answers *"what do you know at all?"* — the question a user asks
// when they want to check or correct something, and one a relevance search
// cannot answer.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ListMemoriesTool;

const DEFAULT_LIMIT: i64 = 25;
const MAX_LIMIT: i64 = 100;

#[async_trait]
impl Tool for ListMemoriesTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "list_memories"
    }

    fn description(&self) -> &str {
        "List memories readable from this workspace, newest first, with their \
         provenance and confidence. Use it to review what is stored — \
         search_memory answers 'what is relevant to this question', this \
         answers 'what do you know at all'.\n\n\
         Only global memories and this workspace's own are listed. Results are \
         capped; the response carries truncated=true when more exist."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tier": {
                    "type": "string",
                    "enum": ["episodic", "semantic", "procedural"],
                    "description": "Optional tier filter."
                },
                "limit": {
                    "type": "integer",
                    "description": "How many to return. Default 25, capped at 100."
                }
            }
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let mut timing = crate::observability::timing::OperationTimer::start("memory_tool_list")
            .with_database("memory")
            .with_session_id(&ctx.session_id)
            .with_task_id(ctx.task_id)
            .with_call_context(&ctx.call_id, &ctx.agent_role);
        if let Some(run_id) = ctx.run_id.as_deref() {
            timing = timing.with_run_id(run_id);
        }
        let db = match crate::memory::db::MemoryDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("memory unavailable: {e}")),
        };

        let limit = input["limit"]
            .as_i64()
            .unwrap_or(DEFAULT_LIMIT)
            .clamp(1, MAX_LIMIT);

        let scopes = crate::memory::recall::scope_keys_for(&ctx.working_dir);
        let placeholders = (0..scopes.len())
            .map(|i| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(",");

        // The tier is checked against a fixed set before it reaches the SQL —
        // it is model-supplied, and an `enum` in a schema is a suggestion to a
        // language model rather than a constraint on one.
        let tier_clause = match input["tier"].as_str() {
            Some(t) if ["episodic", "semantic", "procedural"].contains(&t) => {
                format!(" AND tier = '{t}'")
            }
            _ => String::new(),
        };

        // Ask for one more than requested, so "there are more" is measured
        // rather than inferred from a full page.
        let sql = format!(
            "SELECT id, tier, content, source, confidence, created_at
             FROM memories
             WHERE scope_key IN ({placeholders}){tier_clause}
             ORDER BY julianday(created_at) DESC, id DESC
             LIMIT ?{}",
            scopes.len() + 1
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        for s in &scopes {
            params.push(Box::new(s.clone()));
        }
        params.push(Box::new(limit + 1));
        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

        let mut stmt = match db.conn.prepare(&sql) {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to list memories: {e}")),
        };

        let rows = stmt.query_map(refs.as_slice(), |r| {
            let content: String = r.get(2)?;
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "tier": r.get::<_, String>(1)?,
                "content": crate::security::truncate::truncate_bytes(&content, 240).to_string(),
                "source": r.get::<_, String>(3)?,
                "confidence": r.get::<_, f64>(4)?,
                "created_at": r.get::<_, String>(5)?,
            }))
        });

        let mut skipped_rows = 0usize;
        let mut raw_rows_seen = 0usize;
        let mut items: Vec<Value> = Vec::new();
        match rows {
            Ok(it) => {
                for row in it {
                    raw_rows_seen += 1;
                    match row {
                        Ok(item) => items.push(item),
                        Err(_) => skipped_rows += 1,
                    }
                }
            }
            Err(e) => return ToolResult::error(format!("Failed to list memories: {e}")),
        }

        // The SQL lookahead counts persisted rows, including malformed ones.
        // A corrupt row must not make a partially returned page look complete.
        let truncated = raw_rows_seen as i64 > limit;
        items.truncate(limit as usize);
        let count = items.len();
        if skipped_rows == 0 {
            timing.with_result_count(count).succeed();
        } else {
            timing.with_result_count(count).set_outcome("degraded");
        }

        ToolResult::success(
            json!({
                "memories": items,
                "count": count,
                // Named plainly: a caller that cannot tell a full page from the
                // end of the list will read the first as the second.
                "truncated": truncated,
                "degraded": skipped_rows > 0,
                "malformed_count": skipped_rows,
                "scopes_searched": scopes,
            })
            .to_string(),
        )
    }
}

#[cfg(test)]
mod observability_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn production_list_memory_reports_partial_rows_as_degraded_without_content() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = crate::memory::db::MemoryDb::open().expect("open migrated MemoryDb");
        for (id, content, confidence) in [
            ("valid-id", "safe result content", "0.8"),
            ("corrupt-id", "private corrupt row", "invalid-confidence"),
        ] {
            db.conn
                .execute(
                    "INSERT INTO memories (id, tier, scope, scope_ref, scope_key, content, source, confidence, created_at, updated_at)
                     VALUES (?1, 'semantic', 'global', '', 'global', ?2, 'test', ?3, '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                    rusqlite::params![id, content, confidence],
                )
                .expect("seed memory row");
        }
        drop(db);

        let context = ToolContext {
            session_id: "session-fixture".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-fixture".into(),
            run_id: Some("run-fixture".into()),
            task_id: Some(18),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(ListMemoriesTool.execute(json!({"limit":10}), &context))
        });
        assert!(!result.is_error);
        let output: Value = serde_json::from_str(&result.output).expect("tool JSON result");
        assert_eq!(output["count"], 1);
        assert_eq!(output["degraded"], true);
        assert_eq!(output["malformed_count"], 1);
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_tool_list")
            .expect("production tool operation event");
        assert_eq!(event["fields"]["database"], "memory");
        assert_eq!(event["fields"]["run_id"], "run-fixture");
        assert_eq!(event["fields"]["outcome"], "degraded");
        assert_eq!(event["fields"]["result_count"], 1);
        let telemetry = serde_json::to_string(&events).expect("serialize safe events");
        assert!(!telemetry.contains("safe result content"));
        assert!(!telemetry.contains("private corrupt row"));
        assert!(!telemetry.contains("invalid-confidence"));
    }

    #[test]
    fn malformed_lookahead_does_not_hide_that_more_raw_rows_exist() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = crate::memory::db::MemoryDb::open().expect("open migrated MemoryDb");
        for (id, content, confidence, created_at) in [
            (
                "newest-corrupt",
                "private corrupt row",
                "invalid",
                "2026-10-03T03:00:00Z",
            ),
            ("valid-one", "first valid", "0.8", "2026-10-03T02:00:00Z"),
            ("valid-two", "second valid", "0.8", "2026-10-03T01:00:00Z"),
        ] {
            db.conn
                .execute(
                    "INSERT INTO memories (id, tier, scope, scope_ref, scope_key, content, source, confidence, created_at, updated_at)
                     VALUES (?1, 'semantic', 'global', '', 'global', ?2, 'test', ?3, ?4, ?4)",
                    rusqlite::params![id, content, confidence, created_at],
                )
                .expect("seed memory row");
        }
        drop(db);

        let context = ToolContext {
            session_id: "session-fixture".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-fixture".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let result = runtime.block_on(ListMemoriesTool.execute(json!({"limit":1}), &context));
        let output: Value = serde_json::from_str(&result.output).expect("tool JSON result");

        assert!(!result.is_error);
        assert_eq!(output["count"], 1);
        assert_eq!(output["memories"][0]["id"], "valid-one");
        assert_eq!(output["truncated"], true);
        assert_eq!(output["degraded"], true);
        assert_eq!(output["malformed_count"], 1);
    }

    #[test]
    fn production_list_tool_orders_mixed_timestamps_by_instant() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = crate::memory::db::MemoryDb::open().expect("open migrated MemoryDb");
        for (id, created_at) in [
            ("canonical-0900", "2026-09-02T09:00:00Z"),
            ("legacy-1200", "2026-09-02 12:00:00"),
        ] {
            db.conn
                .execute(
                    "INSERT INTO memories (id, tier, scope, scope_ref, scope_key, content, source, confidence, created_at, updated_at)
                     VALUES (?1, 'semantic', 'global', '', 'global', ?1, 'test', 0.8, ?2, ?2)",
                    rusqlite::params![id, created_at],
                )
                .expect("seed mixed-format Memory row");
        }
        drop(db);

        let context = ToolContext {
            session_id: "session-fixture".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "call-fixture".into(),
            run_id: None,
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),
            run_token_budget: None,
        };
        let runtime = tokio::runtime::Runtime::new().expect("test runtime");
        let result = runtime.block_on(ListMemoriesTool.execute(json!({"limit":10}), &context));
        let output: Value = serde_json::from_str(&result.output).expect("tool JSON result");

        assert!(!result.is_error);
        assert_eq!(output["memories"][0]["id"], "legacy-1200");
        assert_eq!(output["memories"][1]["id"], "canonical-0900");
    }
}
