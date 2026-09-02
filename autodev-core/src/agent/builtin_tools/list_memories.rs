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
             ORDER BY created_at DESC
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

        let mut items: Vec<Value> = match rows {
            Ok(it) => it.filter_map(|r| r.ok()).collect(),
            Err(e) => return ToolResult::error(format!("Failed to list memories: {e}")),
        };

        let truncated = items.len() as i64 > limit;
        items.truncate(limit as usize);
        let count = items.len();

        ToolResult::success(
            json!({
                "memories": items,
                "count": count,
                // Named plainly: a caller that cannot tell a full page from the
                // end of the list will read the first as the second.
                "truncated": truncated,
                "scopes_searched": scopes,
            })
            .to_string(),
        )
    }
}
