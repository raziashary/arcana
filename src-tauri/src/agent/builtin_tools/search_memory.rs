// `search_memory` builtin tool — Phase 4.7
// Cross-tier memory search. Queries episodic summaries, semantic facts, and
// procedural topics. Returns a ranked list of matching memory entries.
// Available in all PA modes.

use async_trait::async_trait;
use serde_json::json;
use tracing::debug;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::memory::{recall, RecallRequest};

pub struct SearchMemoryTool;

#[async_trait]
impl Tool for SearchMemoryTool {
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

    fn is_read_only(&self) -> bool {
        true
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

        debug!("[search_memory] query={query:?} limit={limit}");

        let results = match recall::search_all(&req) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Memory search failed: {e}")),
        };

        if results.is_empty() {
            return ToolResult::success(
                serde_json::to_string_pretty(
                    &json!({ "results": [], "message": "No memory entries found for this query." }),
                )
                .unwrap_or_default(),
            );
        }

        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                json!({
                    "tier":       format!("{:?}", r.tier).to_lowercase(),
                    "id":         r.id,
                    "content":    r.content,
                    "score":      format!("{:.2}", r.score),
                    "created_at": r.created_at.to_rfc3339()
                })
            })
            .collect();

        ToolResult::success(
            serde_json::to_string_pretty(&json!({ "results": items, "total": items.len() }))
                .unwrap_or_default(),
        )
    }
}
