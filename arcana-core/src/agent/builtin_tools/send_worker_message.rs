// send_worker_message — Arcana Communication Protocol (ACP) outbound tool.
//
// One worker sends a structured message to another worker (or the supervisor).
// The message lands in `worker_messages` and an immediate tick is requested
// for a scheduled target. Delivery stays at-least-once until the target's
// recipe consumer acknowledges the stable message ID; without an active
// consumer, the inbox row remains pending.
//
// Why this exists (vs. the orchestrator's DELEGATE[@agent] syntax):
//   * Async, not out-of-band. The target reads its inbox during its own
//     scheduled tick, when budget / orphan / build-evidence guards are active.
//   * Mediated, not direct. Every message persists; the Supervisor sees the
//     transcript and can intervene.
//   * Auditable. Every cross-worker conversation lives in SQLite forever.
//   * Visible. Frontend `worker:message` event drives a Fleet Chat panel.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::super::tool_trait::{Tool, ToolContext, ToolResult};
use crate::db::tasks::TasksDb;
use crate::events::{emit_worker_message, WorkerMessagePayload};
use crate::recipes::scheduler;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_trait::AgentIdentity;
    use std::sync::Arc;

    #[test]
    fn qc_dispatcher_recipe_id_is_not_an_addressable_worker_recipient() {
        let registered = vec![
            "worker-backend".to_string(),
            "supervisor".to_string(),
            "qc-agent".to_string(),
        ];
        assert!(is_known_recipient("qc-agent", &registered));
        assert!(!is_known_recipient("qc-dispatcher", &registered));
        let broadcast = broadcast_targets(registered, "supervisor");
        assert!(broadcast.contains(&"qc-agent".to_string()));
        assert!(!broadcast.contains(&"qc-dispatcher".to_string()));
        assert!(!is_known_recipient("qc-agent", &["worker-backend".into()]));
        assert!(
            !broadcast_targets(vec!["worker-backend".into()], "supervisor")
                .contains(&"qc-agent".to_string())
        );
    }

    #[test]
    fn deleted_system_worker_is_not_a_message_recipient_or_broadcast_target() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = crate::db::workers::WorkersDb::open().expect("open isolated worker roster");
        // Default workers are installed separately from recipe defaults. If
        // the shipped QC worker is active here, verify it cannot survive in
        // the addressable roster after soft deletion.
        if workers
            .registered_slugs()
            .expect("read initial roster")
            .contains(&"qc-agent".to_string())
        {
            workers
                .deactivate("qc-agent")
                .expect("deactivate shipped QC worker");
        }
        // Install a unique QC-like worker so this test proves that
        // active-roster membership, not a hard-coded system slug, controls
        // message addressing.
        let qc_slug = "qc-agent-test";
        let qc_agent_id = workers
            .create("QC", qc_slug, "qc", "gemini-2.5-flash", &[], 120, None)
            .expect("create QC worker fixture");
        assert!(is_known_recipient(
            qc_slug,
            &workers.registered_slugs().expect("active roster")
        ));
        workers.deactivate(qc_slug).expect("deactivate QC worker");
        let active = workers
            .registered_slugs()
            .expect("read active worker roster");
        assert!(!active.contains(&qc_slug.to_string()));
        assert!(!is_known_recipient(qc_slug, &active));
        assert!(!is_known_recipient("qc-agent", &active));
        assert!(!broadcast_targets(active.clone(), "supervisor").contains(&"qc-agent".to_string()));
        assert!(!broadcast_targets(active, "supervisor").contains(&qc_slug.to_string()));
        assert!(
            workers.get_by_id(qc_agent_id).unwrap().is_none(),
            "deleted workers are excluded from normal recipient lookup"
        );
    }

    #[tokio::test]
    async fn native_tool_executor_supplies_sender_identity_and_refuses_self_messages() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().to_string_lossy().into_owned();
        let tool = Arc::new(SendWorkerMessageTool);

        let mut worker = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "worker-session".into(),
            workspace.clone(),
            AgentIdentity::worker("worker-backend"),
        );
        let sent = worker
            .execute_batch(vec![(
                tool.clone(),
                json!({"to":"supervisor", "body":"Endpoint is ready."}),
                "call-send".into(),
            )])
            .await
            .expect("execute native tool path");
        assert_eq!(sent.len(), 1);
        assert!(!sent[0].1.is_error, "send failed: {}", sent[0].1.output);
        let payload: Value = serde_json::from_str(&sent[0].1.output).expect("send result");
        let message_id = payload["persisted_message_ids"][0]
            .as_i64()
            .expect("persisted message id");
        drop(worker);

        let db = TasksDb::open().expect("reopen persisted task board");
        let message = db
            .get_worker_message(message_id)
            .expect("read persisted message")
            .expect("message row");
        assert_eq!(message.from_slug, "worker-backend");
        assert_eq!(message.to_slug, "supervisor");

        let mut supervisor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "supervisor-session".into(),
            workspace.clone(),
            AgentIdentity::supervisor(),
        );
        let self_send = supervisor
            .execute_batch(vec![(
                tool.clone(),
                json!({"to":"supervisor", "body":"This must be refused."}),
                "call-self".into(),
            )])
            .await
            .expect("execute self-message path");
        assert!(self_send[0].1.is_error);
        assert!(self_send[0].1.output.contains("yourself"));

        let mut limited_worker = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "worker-session-2".into(),
            workspace,
            AgentIdentity::worker("worker-backend"),
        );
        let worker_broadcast = limited_worker
            .execute_batch(vec![(
                tool.clone(),
                json!({"to":"*", "body":"this must not fan out"}),
                "call-worker-broadcast".into(),
            )])
            .await
            .expect("execute bounded-broadcast refusal");
        assert!(worker_broadcast[0].1.is_error);
        assert!(worker_broadcast[0].1.output.contains("supervisor-only"));

        let oversized = limited_worker
            .execute_batch(vec![(
                tool,
                json!({"to":"worker-frontend", "body":"x".repeat(MAX_BODY_BYTES + 1)}),
                "call-oversized".into(),
            )])
            .await
            .expect("execute message-size refusal");
        assert!(oversized[0].1.is_error);
        assert!(oversized[0].1.output.contains("maximum is 4096"));
        assert_eq!(db.list_recent_worker_messages(10).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn message_to_unbound_worker_reports_durable_pending_delivery() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = crate::db::workers::WorkersDb::open().expect("open isolated worker roster");
        let worker_id = workers
            .create(
                "Unbound worker",
                "worker-unbound-test",
                "backend",
                "gemini-3.8-flash",
                &[],
                120,
                None,
            )
            .expect("create unbound worker");
        assert_eq!(
            workers.get_by_id(worker_id).unwrap().unwrap().recipe_id,
            None,
            "fixture should have no recipe consumer"
        );

        let mut supervisor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "supervisor-session".into(),
            home.path().to_string_lossy().into_owned(),
            AgentIdentity::supervisor(),
        );
        let results = supervisor
            .execute_batch(vec![(
                Arc::new(SendWorkerMessageTool),
                json!({"to":"worker-unbound-test", "body":"Please review the task."}),
                "call-unbound-message".into(),
            )])
            .await
            .expect("execute message through production tool executor");
        let result = &results[0].1;
        assert!(!result.is_error, "durable inbox should accept the message");
        let payload: Value = serde_json::from_str(&result.output).expect("message result JSON");
        assert!(payload["message"]
            .as_str()
            .unwrap()
            .contains("acknowledgement is pending"));
        let message_id = payload["persisted_message_ids"][0]
            .as_i64()
            .expect("persisted message id");

        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let inbox = tasks
            .list_worker_messages_for("worker-unbound-test", true)
            .expect("read durable inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, message_id);
        assert_eq!(inbox[0].status, "pending");
        assert_eq!(
            tasks
                .get_worker_message(message_id)
                .unwrap()
                .unwrap()
                .status,
            "read",
            "the read marker is separate from acknowledgement"
        );
        tasks
            .acknowledge_worker_message(message_id)
            .expect("acknowledge message");
        assert!(tasks
            .list_worker_messages_for("worker-unbound-test", true)
            .expect("read acknowledged inbox")
            .is_empty());
    }
}

pub struct SendWorkerMessageTool;

const MAX_BODY_BYTES: usize = 4096;
const VALID_KINDS: &[&str] = &["nudge", "question", "answer", "fyi", "broadcast"];

/// Recipients the protocol accepts. `*` is the broadcast wildcard, supervisor-only.
fn is_known_recipient(slug: &str, registered_workers: &[String]) -> bool {
    slug == "*" || registered_workers.iter().any(|worker| worker == slug)
}

fn broadcast_targets(registered_workers: Vec<String>, sender: &str) -> Vec<String> {
    let mut targets = Vec::with_capacity(registered_workers.len());
    for slug in registered_workers {
        if slug != sender && !targets.contains(&slug) {
            targets.push(slug);
        }
    }
    targets
}

#[async_trait]
impl Tool for SendWorkerMessageTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "send_worker_message"
    }

    fn description(&self) -> &str {
        "Send a structured message to another fleet worker. Asynchronous: the \
         target picks it up on its next tick via list_worker_messages. The \
         message persists in the task board and the Supervisor sees it.\n\n\
         Use this for: peer coordination (\"frontend: the endpoint shape \
         changed\"), answering questions another worker queued for you, \
         volunteering FYI context. NOT for: assigning tasks (Supervisor's \
         job), escalating issues (use request_supervisor_action), or routing \
         human-targeted output.\n\n\
         kinds: nudge | question | answer | fyi | broadcast (supervisor-only). \
         to: a worker slug, or '*' for broadcast (supervisor-only). \
         task_id: optional but strongly recommended so the message threads \
         under its task in the Fleet Chat panel."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "to": {
                    "type":        "string",
                    "description": "Target worker slug (e.g. 'worker-frontend'), or '*' for broadcast."
                },
                "body": {
                    "type":        "string",
                    "description": "Message body. Plain text, max 4096 bytes."
                },
                "kind": {
                    "type":        "string",
                    "enum":        VALID_KINDS,
                    "default":     "nudge",
                    "description": "nudge (default) | question (waiting on answer) | answer (reply chain) | fyi (informational) | broadcast (supervisor-only fan-out)."
                },
                "task_id": {
                    "type":        "integer",
                    "description": "Optional task id this message threads under. Strongly recommended for cross-worker conversations about specific work."
                }
            },
            "required": ["to", "body"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        // ── Validate `to` ────────────────────────────────────────────────────
        let to = match input["to"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'to' is required (worker slug or '*')"),
        };
        let registered_workers =
            match crate::db::workers::WorkersDb::open().and_then(|db| db.registered_slugs()) {
                Ok(workers) => workers,
                Err(e) => return ToolResult::error(&format!("Failed to read worker roster: {e}")),
            };
        if !is_known_recipient(&to, &registered_workers) {
            return ToolResult::error(format!(
                "'{to}' is not a registered recipient. Active workers: {}; '*' is the supervisor-only broadcast address.",
                registered_workers.join(", ")
            ));
        }

        // ── Validate `body` ──────────────────────────────────────────────────
        let body = match input["body"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'body' is required and must be non-empty"),
        };
        if body.len() > MAX_BODY_BYTES {
            return ToolResult::error(format!(
                "Message body is {} bytes; maximum is {}. Trim the message or split \
                 into multiple sends.",
                body.len(),
                MAX_BODY_BYTES
            ));
        }

        // ── Validate `kind` ──────────────────────────────────────────────────
        let kind = input["kind"]
            .as_str()
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "nudge".into());
        if !VALID_KINDS.contains(&kind.as_str()) {
            return ToolResult::error(format!(
                "Invalid kind '{kind}'. Expected one of: {}",
                VALID_KINDS.join(", ")
            ));
        }

        // ── Resolve sender ───────────────────────────────────────────────────
        // ctx.agent_role is the canonical sender identity, set by the Rust
        // ToolExecutor from the running agent's identity. We refuse blank
        // values so the inbox sender is always a real agent slug, never "anon".
        // Identity is Rust-authored and always present now, and the
        // interactive assistant is "supervisor" — the role the shipped recipes
        // already give it (recipes/defaults.rs:217), and the one that dispatches
        // to workers. The old refusal was a proxy for "has an identity", which
        // interactive callers lacked; they no longer do, so this is a deliberate
        // widening (ADR-LOG-RESTRUCT-0007). "anon" remains impossible.
        let from = ctx.agent_role.trim().to_string();
        if from.is_empty() {
            return ToolResult::error(
                "send_worker_message refused: the calling agent has no role, so the message would have no identifiable sender."
            );
        }

        // ── Refuse self-addressed ────────────────────────────────────────────
        if from == to {
            return ToolResult::error(format!(
                "Cannot send a message to yourself ('{from}'). If you need a \
                 note-to-self, write to ARCANA_PROGRESS.md instead."
            ));
        }

        // ── Broadcast policy ─────────────────────────────────────────────────
        // Only the Supervisor can fan a message to everyone. This prevents a
        // misbehaving worker from flooding the inbox.
        if to == "*" && from != "supervisor" {
            return ToolResult::error(format!(
                "Broadcast ('*') is supervisor-only. '{from}' may only send to \
                 a specific recipient slug."
            ));
        }

        // ── Optional task_id (typed as integer in schema) ────────────────────
        let task_id: Option<i64> = input["task_id"].as_i64().filter(|n| *n > 0);

        // ── Open DB + persist ────────────────────────────────────────────────
        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("Failed to open tasks DB: {e}")),
        };

        // Broadcast = N inserts; non-broadcast = 1.
        let targets: Vec<String> = if to == "*" {
            broadcast_targets(registered_workers, &from)
        } else {
            vec![to.clone()]
        };

        let mut delivered = Vec::with_capacity(targets.len());
        for target in &targets {
            let msg = match db.insert_worker_message(&from, target, &body, &kind, task_id) {
                Ok(m) => m,
                Err(e) => {
                    return ToolResult::error(format!(
                        "Failed to persist message to '{target}': {e}"
                    ))
                }
            };

            // Emit Tauri event so the Fleet Chat panel updates in real time.
            emit_worker_message(WorkerMessagePayload {
                message_id: msg.id,
                from: msg.from_slug.clone(),
                to: msg.to_slug.clone(),
                body: msg.body.clone(),
                kind: msg.kind.clone(),
                task_id: msg.task_id,
                timestamp: msg.created_at.clone(),
            });

            // Request an immediate tick so a scheduled target reads the inbox
            // promptly. If no schedule is active (including an unbound worker),
            // the durable message remains pending until a consumer runs.
            scheduler::kick(target);

            delivered.push(msg.id);
        }

        let summary = if targets.len() == 1 {
            format!(
                "Message #{} persisted in '{}''s inbox; acknowledgement is pending.",
                delivered[0], targets[0]
            )
        } else {
            format!(
                "Broadcast persisted to {} inboxes; acknowledgement is pending (ids: {}).",
                delivered.len(),
                delivered
                    .iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        ToolResult::success(
            json!({
                "persisted_message_ids": delivered,
                "from":                  from,
                "to":                    targets,
                "kind":                  kind,
                "task_id":               task_id,
                "message":               summary,
            })
            .to_string(),
        )
    }
}
