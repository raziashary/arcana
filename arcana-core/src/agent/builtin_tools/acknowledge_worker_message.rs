// acknowledge_worker_message — ACP inbox cleanup tool.
//
// Workers call this when they've acted on a message (sent an answer,
// fixed the thing the sender flagged, etc.). The row transitions to
// 'acknowledged' and disappears from the unread queue.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;

pub struct AcknowledgeWorkerMessageTool;

#[async_trait]
impl Tool for AcknowledgeWorkerMessageTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "acknowledge_worker_message"
    }

    fn description(&self) -> &str {
        "Mark an ACP inbox message as acknowledged. Call this after you've \
         acted on the message (e.g. sent an answer back, fixed the thing the \
         sender flagged). The Supervisor uses unacknowledged messages older \
         than a few minutes as a 'stuck conversation' signal during its \
         escalation drain."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type":        "integer",
                    "description": "Worker message id to acknowledge. Must be addressed to YOU (the calling agent's slug)."
                }
            },
            "required": ["id"]
        })
    }


    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let id = match input["id"].as_i64() {
            Some(n) if n > 0 => n,
            _ => return ToolResult::error("'id' must be a positive integer message id"),
        };

        // Identity is Rust-authored now and always present, so the former
        // "no __agent_role" refusal cannot fire. The emptiness check remains
        // as a guard against an agent constructed with a blank role.
        let caller = ctx.agent_role.trim().to_string();
        if caller.is_empty() {
            return ToolResult::error(
                "acknowledge_worker_message refused: the calling agent has no role. Every ack must identify the caller."
            );
        }

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks DB: {e}")),
        };

        // Ownership check: the calling agent must be the addressee, otherwise
        // someone other than the recipient could quietly silence inboxes.
        let msg = match db.get_worker_message(id) {
            Ok(Some(m)) => m,
            Ok(None) => return ToolResult::error(format!("Message #{id} not found.")),
            Err(e) => return ToolResult::error(format!("Failed to fetch message #{id}: {e}")),
        };
        if msg.to_slug != caller {
            return ToolResult::error(format!(
                "Message #{id} is addressed to '{}', not to '{caller}'. Only \
                 the addressee may acknowledge it.",
                msg.to_slug
            ));
        }

        if let Err(e) = db.acknowledge_worker_message(id) {
            return ToolResult::error(format!("Failed to acknowledge #{id}: {e}"));
        }

        ToolResult::success(
            json!({
                "message_id": id,
                "status":     "acknowledged",
                "message":    format!("✅ Message #{id} acknowledged."),
            })
            .to_string(),
        )
    }
}
