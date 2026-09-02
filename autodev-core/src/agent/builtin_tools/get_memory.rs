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
            Ok(v) => ToolResult::success(v.to_string()),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                ToolResult::error(format!("No memory {id} is readable from this workspace."))
            }
            Err(e) => ToolResult::error(format!("Failed to read memory: {e}")),
        }
    }
}
