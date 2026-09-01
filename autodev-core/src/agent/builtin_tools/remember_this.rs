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
    fn name(&self) -> &str {
        "remember_this"
    }

    fn description(&self) -> &str {
        "Store an important fact, preference, or learning into long-term memory so it \
         persists across sessions. Use this when the user explicitly asks you to remember \
         something, or when you learn a critical project/user preference that should \
         influence all future interactions. Stored facts appear in every future system prompt."
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
            &fact[..120]
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
