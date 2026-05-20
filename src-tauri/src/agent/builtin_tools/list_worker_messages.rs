// list_worker_messages — AutoDev Communication Protocol inbox-read tool.
//
// Workers call this as Step 1.5 in their recipe (after list_tasks, before
// claim). Returns every message addressed to the calling worker. By default
// unread_only=true so the agent only sees fresh nudges.
//
// Side-effect: returning messages marks the pending ones as `read` and
// stamps `read_at`. The supervisor uses this to surface "read but never
// acknowledged" cross-worker conversations as stuck-conversation
// escalations.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::db::tasks::TasksDb;
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ListWorkerMessagesTool;

#[async_trait]
impl Tool for ListWorkerMessagesTool {
    fn name(&self) -> &str { "list_worker_messages" }

    fn description(&self) -> &str {
        "Read your ACP inbox. Returns every message addressed to you, oldest \
         first. By default only unread messages are returned. Calling this \
         transitions each returned message from 'pending' to 'read', so the \
         Supervisor knows you've seen them; call acknowledge_worker_message \
         when you've acted on a message so it disappears from the unread \
         queue."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unread_only": {
                    "type":        "boolean",
                    "default":     true,
                    "description": "When true (default), only return pending messages. When false, also include 'read' and 'acknowledged' messages — useful for re-reading the full inbox."
                }
            }
        })
    }

    fn is_read_only(&self) -> bool {
        // Technically false because we transition status pending → read, but
        // the side effect is metadata-only (no file I/O). Marking as
        // read-only would let the tool executor batch it concurrently with
        // other reads; the side effect under that scheduler is still
        // serialised inside SQLite so it's safe. Erring on the cautious
        // side: mark as not-read-only so it goes through the serial write
        // partition just like the other status-changing tools.
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let unread_only = input["unread_only"].as_bool().unwrap_or(true);

        let to_slug = match ctx.agent_role.as_ref() {
            Some(r) if !r.trim().is_empty() => r.trim().to_string(),
            _ => return ToolResult::error(
                "list_worker_messages refused: no __agent_role on the call. \
                 The ACP is fleet-only — every reader must identify themselves."
            ),
        };

        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks DB: {e}")),
        };

        let messages = match db.list_worker_messages_for(&to_slug, unread_only) {
            Ok(m) => m,
            Err(e) => return ToolResult::error(format!("Failed to read inbox: {e}")),
        };

        ToolResult::success(json!({
            "count":      messages.len(),
            "to":         to_slug,
            "unread_only": unread_only,
            "messages":   messages,
            "message":    if messages.is_empty() {
                "Inbox empty.".to_string()
            } else {
                format!(
                    "{} message(s) in inbox. Reply with send_worker_message(kind='answer', task_id=…) or call acknowledge_worker_message(id) once handled.",
                    messages.len()
                )
            },
        }).to_string())
    }
}
