// `search_memory` builtin tool — Phase 4.7
// Cross-tier memory search. Queries episodic summaries, semantic facts, and
// procedural topics. Returns a ranked list of matching memory entries.
// Available in all PA modes.

use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::memory::RecallRequest;

pub struct SearchMemoryTool;

#[async_trait]
impl Tool for SearchMemoryTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "search_memory"
    }

    fn description(&self) -> &str {
        "Search across all memory tiers (episodic session summaries, semantic facts, \
         procedural topics/patterns). Returns ranked memory entries relevant to the query. \
         Use this to recall what the user said in past sessions, retrieve known preferences, \
         or find learned patterns relevant to the current task."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Free-text search query (keywords or phrases)"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results to return (default 10, max 30)",
                    "default": 10
                },
                "tier": {
                    "type": "string",
                    "enum": ["all", "episodic", "semantic", "procedural"],
                    "description": "Filter to a specific memory tier (default: all)",
                    "default": "all"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let query = match input["query"].as_str() {
            Some(q) => q.to_string(),
            None => return ToolResult::error("Missing required field: query"),
        };

        let limit = input["limit"]
            .as_u64()
            .map(|n| n.min(30) as u32)
            .unwrap_or(10);

        let tier_filter = match input["tier"].as_str().unwrap_or("all") {
            "episodic" => Some(vec![crate::memory::MemoryTier::Episodic]),
            "semantic" => Some(vec![crate::memory::MemoryTier::Semantic]),
            "procedural" => Some(vec![crate::memory::MemoryTier::Procedural]),
            _ => None,
        };

        let req = RecallRequest {
            session_id: ctx.session_id.clone(),
            query: query.clone(),
            working_dir: Some(ctx.working_dir.clone()),
            tier_filter,
            limit: Some(limit),
        };

        debug!(
            query_chars = query.chars().count(),
            limit, "[search_memory] search requested"
        );

        let result = match crate::memory::recall::search_all_detailed_with_context(
            &req,
            ctx.task_id,
            Some(&ctx.call_id),
            ctx.run_id.as_deref(),
            Some(&ctx.agent_role),
        ) {
            Ok(result) => result,
            Err(e) => return ToolResult::error(format!("Memory search failed: {e}")),
        };

        ToolResult::success(render_response(&result))
    }
}

fn render_response(result: &crate::memory::recall::DetailedSearchResult) -> String {
    let items: Vec<serde_json::Value> = result
        .results
        .iter()
        .map(|r| {
            json!({
                "tier":       format!("{:?}", r.tier).to_lowercase(),
                "id":         r.id,
                "content":    r.content,
                "score":      format!("{:.2}", r.score),
                "created_at": crate::db::timestamps::format_utc_timestamp(r.created_at)
            })
        })
        .collect();
    let mut response = json!({
        "results": items,
        "total": items.len(),
        "degraded": result.degraded,
        "degraded_reason": result.degraded_reason,
    });
    if result.results.is_empty() {
        response["message"] = json!("No memory entries found for this query.");
    }
    serde_json::to_string_pretty(&response).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::backend::{Candidate, LexicalBackend, RetrievalBackend, RetrievalQuery};

    struct DegradedLexicalBackend {
        reason: &'static str,
    }

    impl RetrievalBackend for DegradedLexicalBackend {
        fn name(&self) -> &'static str {
            "test-lexical"
        }

        fn is_degraded(&self) -> bool {
            true
        }

        fn degraded_reason(&self) -> Option<String> {
            Some(self.reason.to_string())
        }

        fn search(
            &self,
            conn: &rusqlite::Connection,
            query: &RetrievalQuery,
        ) -> crate::errors::Result<Vec<Candidate>> {
            LexicalBackend.search(conn, query)
        }
    }

    fn test_db() -> rusqlite::Connection {
        crate::memory::vector::register_once();
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(
            &conn,
            "memory.db(search_memory_test)",
            crate::memory::db::MemoryDb::LADDER,
        )
        .unwrap();
        conn
    }

    fn insert_memory(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO memories(id,tier,scope,scope_ref,scope_key,content,created_at,updated_at)
             VALUES ('memory-1','semantic','project','/project','project:/project',
                     'calendar event retention', '2026-09-02T00:00:00Z', '2026-09-02T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn request(query: &str) -> RecallRequest {
        RecallRequest {
            session_id: "session-test".into(),
            query: query.into(),
            working_dir: Some("/project".into()),
            tier_filter: None,
            limit: Some(10),
        }
    }

    fn json_response(conn: &rusqlite::Connection, req: &RecallRequest) -> serde_json::Value {
        let backend = DegradedLexicalBackend {
            reason:
                "Semantic search unavailable; api_key=canary-secret. Keyword search remains active.",
        };
        let result = crate::memory::recall::search_with_backend(conn, req, &backend).unwrap();
        serde_json::from_str(&render_response(&result)).unwrap()
    }

    #[test]
    fn nonempty_keyword_fallback_reports_degradation_without_exposing_secrets() {
        let conn = test_db();
        insert_memory(&conn);

        let response = json_response(&conn, &request("calendar retention"));

        assert_eq!(response["degraded"], true);
        assert_eq!(response["total"], 1);
        assert_eq!(response["results"][0]["id"], "memory-1");
        let reason = response["degraded_reason"].as_str().unwrap();
        assert!(
            reason.contains("[REDACTED]"),
            "reason was not redacted: {reason}"
        );
        assert!(!reason.contains("canary-secret"));
    }

    #[test]
    fn empty_keyword_fallback_still_reports_degradation_and_reason() {
        let conn = test_db();

        let response = json_response(&conn, &request("no matching memory"));

        assert_eq!(response["degraded"], true);
        assert_eq!(
            response["degraded_reason"]
                .as_str()
                .unwrap()
                .contains("[REDACTED]"),
            true
        );
        assert_eq!(response["results"], json!([]));
        assert_eq!(response["total"], 0);
        assert_eq!(
            response["message"],
            "No memory entries found for this query."
        );
    }

    #[tokio::test]
    async fn production_tool_reads_persisted_results_and_detailed_status() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = crate::memory::db::MemoryDb::open().expect("open migrated production database");
        insert_memory(&db.conn);
        drop(db);

        let ctx = ToolContext {
            session_id: "session-memory-tool-test".into(),
            working_dir: "/project".into(),
            call_id: "call-memory-tool-test".into(),
            run_id: Some("run-memory-tool-test".into()),
            task_id: None,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: crate::agent::tool_trait::AgentIdentity::supervisor().role,

            run_token_budget: None,
        };
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            futures::executor::block_on(
                SearchMemoryTool.execute(json!({ "query": "calendar event retention" }), &ctx),
            )
        });

        assert!(!result.is_error, "{}", result.output);
        let response: serde_json::Value =
            serde_json::from_str(&result.output).expect("tool returns JSON");
        assert_eq!(response["degraded"], true);
        let reason = response["degraded_reason"]
            .as_str()
            .expect("fallback reason");
        assert!(reason.contains("keyword-matched only"));
        assert_eq!(response["total"], 1);
        assert_eq!(response["results"][0]["id"], "memory-1");
        assert_eq!(
            response["results"][0]["content"],
            "calendar event retention"
        );
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_search_all")
            .expect("production tool's detailed search timing");
        assert_eq!(event["fields"]["task_id"], "");
        assert_eq!(event["fields"]["call_id"], "call-memory-tool-test");
        assert_eq!(event["fields"]["run_id"], "run-memory-tool-test");
        assert_eq!(event["fields"]["agent_role"], ctx.agent_role);
    }
}
