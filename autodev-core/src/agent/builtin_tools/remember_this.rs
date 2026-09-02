// `remember_this` builtin tool — Phase 4.7
// Stores a user-explicit fact with confidence = 1.0 into semantic memory (Tier 2).
// The agent calls this when the user says something like:
//   "Remember I prefer X over Y"
//   "Note that we always use rusqlite, not sea-orm"
//   "Keep in mind that my name is [X]"
// Available in all PA modes.

use async_trait::async_trait;
use serde_json::json;
use tracing::info;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::memory::{FactCategory, SemanticStore};

pub struct RememberThisTool;

#[async_trait]
impl Tool for RememberThisTool {
    // `Board`: it writes durable state that other runs read back. Neither
    // `Write` (the workspace) nor `Read` fits memory exactly — and this tool is
    // removed entirely in `P3-39`, so it gets the fail-safe of the two rather
    // than a new variant invented for something scheduled for deletion.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "remember_this"
    }

    fn description(&self) -> &str {
        "Store an important fact, preference, or learning into long-term memory so it \
         persists across sessions. Use this when the user explicitly asks you to remember \
         something, or when you learn a critical project/user preference that should \
         influence all future interactions.\n\n\
         ⚠ Stored facts are injected only into sessions that carry a session \
         id. Recipe-mode runs, subagents and the MCP server have none, so a \
         fact stored there is persisted but will NOT appear in those prompts."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "fact": {
                    "type": "string",
                    "description": "The fact, preference, or learning to remember (be specific and concise)"
                },
                "category": {
                    "type": "string",
                    "enum": ["preference", "architecture", "pattern", "reference", "user_profile"],
                    "description": "Category of the fact:\n\
                      - preference: user/style preferences\n\
                      - architecture: technology/design decisions\n\
                      - pattern: coding patterns to follow\n\
                      - reference: important file or symbol locations\n\
                      - user_profile: facts about the user themselves",
                    "default": "preference"
                },
                "tags": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tags for easier retrieval (e.g. [\"rust\", \"testing\"])"
                }
            },
            "required": ["fact"]
        })
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let fact = match input["fact"].as_str() {
            Some(f) => f.trim().to_string(),
            None => return ToolResult::error("Missing required field: fact"),
        };

        if fact.is_empty() {
            return ToolResult::error("fact cannot be empty");
        }

        let category = FactCategory::from_str(input["category"].as_str().unwrap_or("preference"));

        let tags: Vec<String> = input["tags"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        info!(
            "[remember_this] storing fact category={} session={}",
            category.as_str(),
            ctx.session_id
        );

        let id = match SemanticStore::store_fact(
            &fact,
            category,
            1.0,
            "user_explicit",
            Some(&ctx.session_id),
            Some(&ctx.working_dir),
            &tags,
        ) {
            Ok(id) => id,
            Err(e) => return ToolResult::error(format!("Failed to store memory: {e}")),
        };

        let display = if fact.len() > 120 {
            // P3-04: `&fact[..120]` panicked on any fact whose 120th byte
            // fell inside a character — and a "fact" is free text the model
            // wrote, so non-ASCII is routine.
            crate::security::truncate::truncate_bytes(&fact, 120)
        } else {
            &fact
        };
        ToolResult::success(
            serde_json::to_string_pretty(
                &json!({ "stored": true, "id": id, "message": format!("Remembered: \"{display}\"") })
            ).unwrap_or_default()
        )
    }
}
