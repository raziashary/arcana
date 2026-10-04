// list_worker_messages — Arcana Communication Protocol inbox-read tool.
//
// Workers call this as Step 1.5 in their recipe (after list_tasks, before
// claim). Unacknowledged messages remain deliverable after a failed run, with
// the same stable ID so a consumer can recognize a replay.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;

pub struct ListWorkerMessagesTool;

#[async_trait]
impl Tool for ListWorkerMessagesTool {
    // `Board`: reading the inbox marks messages read, so it mutates shared
    // state despite the name.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "list_worker_messages"
    }

    fn description(&self) -> &str {
        "Read your ACP inbox. Returns unacknowledged messages addressed to you, \
         oldest first. A message may be delivered again after a failed or \
         interrupted run, always with the same stable message ID. Treat that \
         ID as the idempotency key: check current task/project state before \
         repeating effects, and acknowledge only after handling it. Reading \
         transitions 'pending' to 'read' but does not remove it from delivery."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unread_only": {
                    "type":        "boolean",
                    "default":     true,
                    "description": "When true (default), return pending and read messages until acknowledged. When false, also include acknowledged messages for inbox history."
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

        // Always present now (see AgentIdentity); guards a blank role only.
        let to_slug = ctx.agent_role.trim().to_string();
        if to_slug.is_empty() {
            return ToolResult::error(
                "list_worker_messages refused: the calling agent has no role. Every reader must identify themselves."
            );
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::AgentIdentity;
    use std::sync::Arc;

    #[tokio::test]
    async fn production_inbox_consumer_replays_stable_id_and_ack_stops_delivery() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated tasks database");
        let sent = db
            .insert_worker_message(
                "worker-backend",
                "worker-frontend",
                "Please wire the endpoint.",
                "request",
                None,
            )
            .expect("persist message");
        drop(db);

        let ctx = ToolContext {
            session_id: "acp-replay-test".into(),
            working_dir: home.path().to_string_lossy().into_owned(),
            call_id: "acp-replay-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-frontend".into(),

            run_token_budget: None,
        };
        let mut worker = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            ctx.session_id.clone(),
            ctx.working_dir.clone(),
            AgentIdentity::worker("worker-frontend"),
        );
        let read_tool = Arc::new(ListWorkerMessagesTool);
        let first = worker
            .execute_batch(vec![(read_tool.clone(), json!({}), "first-read".into())])
            .await
            .expect("execute inbox through production ToolExecutor");
        assert!(
            !first[0].1.is_error,
            "first delivery failed: {}",
            first[0].1.output
        );
        let first: Value = serde_json::from_str(&first[0].1.output).expect("first payload");
        assert_eq!(first["messages"][0]["id"], sent.id);
        assert_eq!(first["messages"][0]["status"], "pending");
        drop(worker); // consumer exits before acknowledging

        let mut recovered_worker = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "worker-session-recovered".into(),
            ctx.working_dir.clone(),
            AgentIdentity::worker("worker-frontend"),
        );

        let replay = recovered_worker
            .execute_batch(vec![(read_tool, json!({}), "replay-read".into())])
            .await
            .expect("replay inbox through production ToolExecutor");
        assert!(
            !replay[0].1.is_error,
            "replay failed: {}",
            replay[0].1.output
        );
        let replay: Value = serde_json::from_str(&replay[0].1.output).expect("replay payload");
        assert_eq!(replay["messages"][0]["id"], sent.id);

        let ack = recovered_worker
            .execute_batch(vec![(
                Arc::new(super::super::acknowledge_worker_message::AcknowledgeWorkerMessageTool),
                json!({"id": sent.id}),
                "ack-message".into(),
            )])
            .await;
        let ack = ack.expect("ack through production ToolExecutor");
        assert!(
            !ack[0].1.is_error,
            "acknowledgement failed: {}",
            ack[0].1.output
        );
        let after_ack = recovered_worker
            .execute_batch(vec![(
                Arc::new(ListWorkerMessagesTool),
                json!({}),
                "read-after-ack".into(),
            )])
            .await
            .expect("read after acknowledgement through production ToolExecutor");
        let after_ack: Value =
            serde_json::from_str(&after_ack[0].1.output).expect("post-ack payload");
        assert_eq!(after_ack["messages"].as_array().unwrap().len(), 0);

        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        for tool in [
            "send_worker_message",
            "list_worker_messages",
            "acknowledge_worker_message",
        ] {
            assert!(
                registry.get(tool).is_some(),
                "missing FastDevelopment ACP tool {tool}"
            );
        }
    }

    #[tokio::test]
    async fn persistent_qc_agent_reads_and_acknowledges_its_inbox_without_review_work() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = TasksDb::open().expect("open isolated tasks database");
        let message = db
            .insert_worker_message(
                "worker-backend",
                "qc-agent",
                "Please review the test report when available.",
                "fyi",
                None,
            )
            .expect("persist inbox-only message");

        let mut qc_agent = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "qc-inbox-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("qc-agent"),
        );
        let read = qc_agent
            .execute_batch(vec![(
                Arc::new(ListWorkerMessagesTool),
                json!({}),
                "qc-read".into(),
            )])
            .await
            .expect("run persistent QC inbox reader");
        assert!(
            !read[0].1.is_error,
            "QC inbox read failed: {}",
            read[0].1.output
        );
        let payload: Value = serde_json::from_str(&read[0].1.output).expect("inbox JSON");
        assert_eq!(payload["messages"][0]["id"], message.id);

        let ack = qc_agent
            .execute_batch(vec![(
                Arc::new(super::super::acknowledge_worker_message::AcknowledgeWorkerMessageTool),
                json!({"id": message.id}),
                "qc-ack".into(),
            )])
            .await
            .expect("acknowledge QC inbox message");
        assert!(
            !ack[0].1.is_error,
            "QC inbox ack failed: {}",
            ack[0].1.output
        );
        assert_eq!(
            db.list_worker_messages_for("qc-agent", true).unwrap().len(),
            0,
            "an inbox-only QC tick must drain the addressed message"
        );
    }
}
