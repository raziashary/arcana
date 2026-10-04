// list_stale_worker_messages — Supervisor-only recovery view for aged ACP mail.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;

pub struct ListStaleWorkerMessagesTool;

#[async_trait]
impl Tool for ListStaleWorkerMessagesTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "list_stale_worker_messages"
    }

    fn description(&self) -> &str {
        "Supervisor-only recovery view of ACP messages that remain pending or read without acknowledgement past the age threshold. Use it to nudge the addressee or route stuck work; reading never acknowledges or deletes a message."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "older_than_minutes": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1440,
                    "description": "Minimum age for unacknowledged messages; defaults to 5 minutes."
                }
            },
            "additionalProperties": false
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let older_than_minutes = match input.get("older_than_minutes") {
            None => 5,
            Some(value) => match value.as_i64() {
                Some(minutes @ 1..=1440) => minutes,
                _ => {
                    return ToolResult::error(
                        "'older_than_minutes' must be an integer from 1 through 1440",
                    )
                }
            },
        };
        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(error) => return ToolResult::error(format!("Failed to open tasks DB: {error}")),
        };
        list_stale_worker_messages_in(&db, &ctx.agent_role, older_than_minutes)
    }
}

fn list_stale_worker_messages_in(
    db: &TasksDb,
    agent_role: &str,
    older_than_minutes: i64,
) -> ToolResult {
    if agent_role != "supervisor" {
        return ToolResult::error(
            "list_stale_worker_messages is restricted to the Supervisor because it reads fleet-wide message bodies",
        );
    }
    if !(1..=1440).contains(&older_than_minutes) {
        return ToolResult::error("age threshold must be between 1 and 1440 minutes");
    }
    match db.list_stale_worker_messages(older_than_minutes) {
        Ok(messages) => {
            ToolResult::success(json!({"count": messages.len(), "messages": messages}).to_string())
        }
        Err(error) => ToolResult::error(format!("Failed to read stale worker messages: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tasks::TasksDb;
    use rusqlite::params;

    #[test]
    fn stale_message_reader_is_supervisor_only_and_preserves_unacknowledged_rows() {
        let path = std::env::temp_dir().join(format!(
            "arcana-stale-worker-messages-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = TasksDb::open_at(&path).expect("open isolated task DB");
        let message = db
            .insert_worker_message(
                "worker-backend",
                "qc-agent",
                "Please review this task result",
                "question",
                None,
            )
            .expect("insert ACP message");
        db.list_worker_messages_for("qc-agent", true)
            .expect("read QC inbox");
        db.conn
            .execute(
                "UPDATE worker_messages SET created_at = datetime('now', '-10 minutes') WHERE id = ?1",
                params![message.id],
            )
            .expect("age fixture message");

        let denied = list_stale_worker_messages_in(&db, "worker-backend", 5);
        assert!(denied.is_error);
        assert!(denied.output.contains("restricted to the Supervisor"));

        let result = list_stale_worker_messages_in(&db, "supervisor", 5);
        assert!(!result.is_error, "unexpected error: {}", result.output);
        let payload: Value = serde_json::from_str(&result.output).expect("stale response JSON");
        assert_eq!(payload["count"], 1);
        assert_eq!(payload["messages"][0]["id"], message.id);
        assert_eq!(payload["messages"][0]["status"], "read");
        assert_eq!(db.count_unacknowledged_for("qc-agent").unwrap(), 1);
    }
}
