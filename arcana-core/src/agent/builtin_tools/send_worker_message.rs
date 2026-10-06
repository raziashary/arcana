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
                Err(_) => {
                    return ToolResult::error(
                        "Local worker storage could not load the recipient roster.",
                    )
                }
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
            Err(_) => {
                return ToolResult::error(
                    "Local task storage is unavailable; the message was not sent.",
                )
            }
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
                Err(_) => {
                    return ToolResult::categorized_error(
                        "worker_message_persist_failed",
                        "database",
                        "Local task storage rejected the message write; its acknowledgement is not recorded.",
                        "Check task storage and the referenced task, then inspect recipient inboxes before retrying a broadcast.",
                    )
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

#[cfg(test)]
mod acp_matrix_tests {
    use super::*;
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::{AgentIdentity, ToolResult};
    use crate::db::runs::RunsDb;
    use crate::db::tasks::TasksDb;
    use crate::db::workers::WorkersDb;
    use std::sync::Arc;

    const SESSION: &str = "p4-51-acp-live-matrix";

    fn live_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        registry
    }

    fn executor(
        task_id: i64,
        home: &std::path::Path,
        identity: AgentIdentity,
        run_id: &str,
    ) -> ToolExecutor {
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            SESSION.into(),
            home.to_string_lossy().into_owned(),
            identity,
        )
        .with_run_id(Some(run_id.to_owned()));
        executor.set_task_id(Some(task_id));
        executor
    }

    async fn invoke(
        executor: &mut ToolExecutor,
        registry: &ToolRegistry,
        name: &str,
        input: Value,
        call: &str,
    ) -> ToolResult {
        let tool = registry
            .get(name)
            .unwrap_or_else(|| panic!("FastDevelopment registry is missing {name}"));
        let results = executor
            .execute_batch(vec![(tool, input, call.into())])
            .await
            .expect("execute registered ACP tool through ToolExecutor");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, call);
        results.into_iter().next().unwrap().1
    }

    fn assert_success(result: &ToolResult, name: &str) -> Value {
        assert!(!result.is_error, "{name} failed: {}", result.output);
        serde_json::from_str(&result.output)
            .unwrap_or_else(|error| panic!("{name} returned invalid success JSON: {error}"))
    }

    fn assert_failure(
        expected_task_id: i64,
        expected_run_id: &str,
        result: &ToolResult,
        name: &str,
        call: &str,
        agent: &str,
        private_markers: &[&str],
    ) -> Value {
        assert!(
            result.is_error,
            "{name} returned success-shaped failure: {}",
            result.output
        );
        assert!(result.output.len() <= 8192, "{name} error exceeded bound");
        for marker in private_markers {
            assert!(
                !result.output.contains(marker),
                "{name} error reflected private marker {marker}"
            );
        }
        let envelope: Value = serde_json::from_str(&result.output)
            .unwrap_or_else(|error| panic!("{name} failure was not JSON: {error}"));
        assert_eq!(envelope["schema_version"], 1, "{name}: {envelope}");
        assert_eq!(envelope["ok"], false, "{name}: {envelope}");
        assert_eq!(envelope["tool"], name, "{name}: {envelope}");
        assert_eq!(envelope["operation"], format!("tool.{name}"));
        assert!(!envelope["error_code"]
            .as_str()
            .unwrap_or_default()
            .is_empty());
        assert!(!envelope["category"].as_str().unwrap_or_default().is_empty());
        assert!(envelope["retryable"].is_boolean());
        assert!(envelope["retry_after_ms"].is_null() || envelope["retry_after_ms"].is_u64());
        assert!(envelope["message"].as_str().unwrap_or_default().len() <= 512);
        assert!(envelope["cause"].as_str().unwrap_or_default().len() <= 512);
        assert!(
            envelope["suggested_action"]
                .as_str()
                .unwrap_or_default()
                .len()
                <= 512
        );
        assert_eq!(envelope["context"]["session_id"], SESSION);
        assert_eq!(envelope["context"]["agent_id"], agent);
        assert_eq!(envelope["context"]["task_id"], expected_task_id);
        assert_eq!(envelope["context"]["run_id"], expected_run_id);
        assert_eq!(
            envelope["context"]["tool_call_id"],
            format!("{SESSION}-{call}")
        );
        assert!(envelope["timestamp"].as_str().is_some());
        assert!(envelope["error"].is_object());
        envelope
    }

    #[tokio::test]
    async fn acp_live_fastdevelopment_registry_has_success_and_failure_proof() {
        let home = tempfile::tempdir().expect("isolated Arcana home and workspace");
        let _home = crate::db::test_support::with_home(home.path());
        let registry = live_registry();
        let expected = [
            "send_worker_message",
            "list_worker_messages",
            "list_stale_worker_messages",
            "list_worker_health",
            "acknowledge_worker_message",
            "list_worker_requests",
            "resolve_worker_request",
            "request_supervisor_action",
            "reconcile_worker_operation",
        ];
        for name in expected {
            assert!(
                registry.get(name).is_some(),
                "missing registered tool {name}"
            );
        }

        let workers = WorkersDb::open().expect("open isolated workers DB");
        workers
            .create(
                "ACP matrix worker",
                "worker-acp-matrix",
                "backend",
                "fixture-model",
                &[],
                120,
                None,
            )
            .expect("seed isolated worker roster");
        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task_id = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "ACP matrix task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "isolated communication proof".into(),
                created_by: Some("test".into()),
            })
            .expect("seed real referenced task")
            .id;
        let runs = RunsDb::open().expect("open isolated worker run journal");
        let health_run = runs
            .begin_worker_tick("worker-acp-matrix", "acp-matrix-health-session")
            .expect("write worker health row");
        let intent_run = runs
            .begin("worker_tick", Some("worker-acp-matrix"))
            .expect("write operation run");
        let intent_cursor = runs
            .record_operation_intent(
                &intent_run,
                Some(task_id),
                "acp-matrix-operation",
                "write_file",
                "FileSystem",
                &[],
            )
            .expect("write unresolved operation intent");

        let mut supervisor = executor(
            task_id,
            home.path(),
            AgentIdentity::supervisor(),
            &health_run,
        );
        let mut worker = executor(
            task_id,
            home.path(),
            AgentIdentity::worker("worker-acp-matrix"),
            &health_run,
        );

        let sent = invoke(
            &mut supervisor,
            &registry,
            "send_worker_message",
            json!({"to":"worker-acp-matrix", "body":"ACP matrix durable message", "kind":"question", "task_id":task_id}),
            "send-success",
        )
        .await;
        let sent = assert_success(&sent, "send_worker_message");
        let message_id = sent["persisted_message_ids"][0]
            .as_i64()
            .expect("persisted message id");
        let persisted = tasks
            .get_worker_message(message_id)
            .expect("read sent message")
            .expect("sent message row");
        assert_eq!(persisted.from_slug, "supervisor");
        assert_eq!(persisted.to_slug, "worker-acp-matrix");
        assert_eq!(persisted.body, "ACP matrix durable message");

        let injected_send_detail = format!(
            "ACP_PRIVATE_DATABASE_MARKER {}",
            "oversized database diagnostic ".repeat(96)
        );
        let trigger = format!(
            "CREATE TRIGGER p4_51_fail_acp_send BEFORE INSERT ON worker_messages
             BEGIN SELECT RAISE(ABORT, '{}'); END;",
            injected_send_detail.replace('\'', "''")
        );
        tasks
            .conn
            .execute_batch(&trigger)
            .expect("inject long isolated message-write failure");
        let send_failed = invoke(
            &mut supervisor,
            &registry,
            "send_worker_message",
            json!({"to":"worker-acp-matrix", "body":"ACP_PRIVATE_BODY_VALUE"}),
            "send-database-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &send_failed,
            "send_worker_message",
            "send-database-failure",
            "supervisor",
            &["ACP_PRIVATE_BODY_VALUE", "ACP_PRIVATE_DATABASE_MARKER"],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_acp_send;")
            .expect("remove isolated message-write failure");

        let inbox = invoke(
            &mut worker,
            &registry,
            "list_worker_messages",
            json!({"unread_only":true}),
            "inbox-success",
        )
        .await;
        assert_eq!(
            assert_success(&inbox, "list_worker_messages")["messages"][0]["id"],
            message_id
        );
        assert_eq!(
            tasks
                .get_worker_message(message_id)
                .unwrap()
                .unwrap()
                .status,
            "read"
        );
        tasks
            .conn
            .execute(
                "UPDATE worker_messages SET created_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','-10 minutes') WHERE id=?1",
                rusqlite::params![message_id],
            )
            .expect("age isolated pending message for stale reader");
        let stale = invoke(
            &mut supervisor,
            &registry,
            "list_stale_worker_messages",
            json!({"older_than_minutes":1}),
            "stale-success",
        )
        .await;
        assert_eq!(
            assert_success(&stale, "list_stale_worker_messages")["messages"][0]["id"],
            message_id
        );

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_acp_ack BEFORE UPDATE OF status ON worker_messages
                 WHEN NEW.status='acknowledged'
                 BEGIN SELECT RAISE(ABORT, 'injected acknowledgement database failure'); END;",
            )
            .expect("inject acknowledgement database failure");
        let ack_failed = invoke(
            &mut worker,
            &registry,
            "acknowledge_worker_message",
            json!({"id":message_id}),
            "ack-database-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &ack_failed,
            "acknowledge_worker_message",
            "ack-database-failure",
            "worker-acp-matrix",
            &[],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_acp_ack;")
            .expect("remove isolated acknowledgement failure");
        let acked = invoke(
            &mut worker,
            &registry,
            "acknowledge_worker_message",
            json!({"id":message_id}),
            "ack-success",
        )
        .await;
        assert_eq!(
            assert_success(&acked, "acknowledge_worker_message")["status"],
            "acknowledged"
        );
        assert_eq!(
            tasks
                .get_worker_message(message_id)
                .unwrap()
                .unwrap()
                .status,
            "acknowledged"
        );

        let request = invoke(
            &mut worker,
            &registry,
            "request_supervisor_action",
            json!({"task_id":task_id, "request_type":"report_issue", "payload":{"issue_description":"fixture issue", "severity":"low"}}),
            "request-success",
        )
        .await;
        let request = assert_success(&request, "request_supervisor_action");
        let request_id = request["request_id"].as_i64().expect("request id");
        let requests = invoke(
            &mut supervisor,
            &registry,
            "list_worker_requests",
            json!({"status":"pending"}),
            "requests-success",
        )
        .await;
        assert_eq!(
            assert_success(&requests, "list_worker_requests")["requests"][0]["id"],
            request_id
        );

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_worker_request_resolution BEFORE UPDATE OF status ON worker_requests
                 BEGIN SELECT RAISE(ABORT, 'injected request resolution database failure'); END;",
            )
            .expect("inject request-resolution database failure");
        let resolve_failed = invoke(
            &mut supervisor,
            &registry,
            "resolve_worker_request",
            json!({"id":request_id, "action":"completed", "note":"ACP_PRIVATE_RESOLUTION_NOTE"}),
            "resolve-database-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &resolve_failed,
            "resolve_worker_request",
            "resolve-database-failure",
            "supervisor",
            &["ACP_PRIVATE_RESOLUTION_NOTE"],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_worker_request_resolution;")
            .expect("remove isolated resolution failure");
        let resolved = invoke(
            &mut supervisor,
            &registry,
            "resolve_worker_request",
            json!({"id":request_id, "action":"completed", "note":"fixture handled"}),
            "resolve-success",
        )
        .await;
        assert_eq!(
            assert_success(&resolved, "resolve_worker_request")["action"],
            "completed"
        );
        assert_eq!(
            tasks
                .get_worker_request_by_id(request_id)
                .unwrap()
                .unwrap()
                .status
                .as_str(),
            "completed"
        );

        tasks
            .conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_worker_request_insert BEFORE INSERT ON worker_requests
                 BEGIN SELECT RAISE(ABORT, 'injected request create failure'); END;",
            )
            .expect("inject worker request creation failure");
        let action_failed = invoke(
            &mut worker,
            &registry,
            "request_supervisor_action",
            json!({"request_type":"needs_info", "payload":{"question":"ACP_PRIVATE_REQUEST_VALUE"}}),
            "request-database-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &action_failed,
            "request_supervisor_action",
            "request-database-failure",
            "worker-acp-matrix",
            &["ACP_PRIVATE_REQUEST_VALUE"],
        );
        tasks
            .conn
            .execute_batch("DROP TRIGGER p4_51_fail_worker_request_insert;")
            .expect("remove isolated request creation failure");

        let reconciliation = invoke(
            &mut supervisor,
            &registry,
            "reconcile_worker_operation",
            json!({"run_id":intent_run, "intent_cursor":intent_cursor, "decision":"do_not_retry", "reason_code":"unsafe_to_repeat"}),
            "reconcile-success",
        )
        .await;
        assert_eq!(
            assert_success(&reconciliation, "reconcile_worker_operation")["decision"],
            "do_not_retry"
        );
        let journal = runs
            .operation_journal_page(&intent_run, 0, 10)
            .expect("read reconciled journal");
        assert_eq!(journal.entries[0].reconciliation.as_str(), "do_not_retry");
        assert_eq!(
            journal.entries[0].reconciliation_actor.as_deref(),
            Some("supervisor")
        );

        runs.conn
            .execute_batch(
                "CREATE TRIGGER p4_51_fail_acp_reconciliation BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type='reconciliation'
                 BEGIN SELECT RAISE(ABORT, 'ACP_PRIVATE_RUN_DETAIL injected journal failure'); END;",
            )
            .expect("inject operation reconciliation database failure");
        let second_intent = runs
            .record_operation_intent(
                &intent_run,
                None,
                "acp-matrix-operation-2",
                "append_file",
                "FileSystem",
                &[],
            )
            .expect("write second unresolved intent");
        let reconcile_failed = invoke(
            &mut supervisor,
            &registry,
            "reconcile_worker_operation",
            json!({"run_id":intent_run, "intent_cursor":second_intent, "decision":"retry_safe", "reason_code":"idempotent_retry_verified"}),
            "reconcile-database-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &reconcile_failed,
            "reconcile_worker_operation",
            "reconcile-database-failure",
            "supervisor",
            &["ACP_PRIVATE_RUN_DETAIL"],
        );
        runs.conn
            .execute_batch("DROP TRIGGER p4_51_fail_acp_reconciliation;")
            .expect("remove isolated reconciliation failure");

        let health = invoke(
            &mut supervisor,
            &registry,
            "list_worker_health",
            json!({"limit":20, "journal_limit":10}),
            "health-success",
        )
        .await;
        let health = assert_success(&health, "list_worker_health");
        assert!(health["workers"].as_array().unwrap().iter().any(|row| {
            row["slug"] == "worker-acp-matrix" && row["run"]["run_id"] == health_run
        }));

        for (name, call, input, markers) in [
            (
                "send_worker_message",
                "send-invalid-recipient",
                json!({"to":"ACP_PRIVATE_RECIPIENT", "body":"fixture message"}),
                vec!["ACP_PRIVATE_RECIPIENT"],
            ),
            (
                "acknowledge_worker_message",
                "ack-invalid-id",
                json!({"id":0}),
                vec![],
            ),
            (
                "resolve_worker_request",
                "resolve-invalid-action",
                json!({"id":request_id, "action":"ACP_PRIVATE_ACTION"}),
                vec!["ACP_PRIVATE_ACTION"],
            ),
            (
                "request_supervisor_action",
                "request-invalid-type",
                json!({"request_type":"ACP_PRIVATE_TYPE", "payload":{}}),
                vec!["ACP_PRIVATE_TYPE"],
            ),
        ] {
            let (result, agent) =
                if name == "acknowledge_worker_message" || name == "request_supervisor_action" {
                    (
                        invoke(&mut worker, &registry, name, input, call).await,
                        "worker-acp-matrix",
                    )
                } else {
                    (
                        invoke(&mut supervisor, &registry, name, input, call).await,
                        "supervisor",
                    )
                };
            assert_failure(task_id, &health_run, &result, name, call, agent, &markers);
        }

        let worker_stale_denied = invoke(
            &mut worker,
            &registry,
            "list_stale_worker_messages",
            json!({"older_than_minutes":1}),
            "stale-role-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &worker_stale_denied,
            "list_stale_worker_messages",
            "stale-role-failure",
            "worker-acp-matrix",
            &[],
        );
        let worker_health_denied = invoke(
            &mut worker,
            &registry,
            "list_worker_health",
            json!({}),
            "health-role-failure",
        )
        .await;
        assert_failure(
            task_id,
            &health_run,
            &worker_health_denied,
            "list_worker_health",
            "health-role-failure",
            "worker-acp-matrix",
            &[],
        );

        drop(tasks);
        drop(workers);
        drop(runs);
        let task_db_path = home.path().join(".arcana").join("tasks.db");
        std::fs::remove_file(&task_db_path).expect("remove only isolated temp-home task DB");
        std::fs::create_dir(&task_db_path).expect("obstruct only isolated temp-home task DB");
        for (name, call, input) in [
            ("list_worker_messages", "inbox-database-failure", json!({})),
            (
                "list_stale_worker_messages",
                "stale-database-failure",
                json!({}),
            ),
            (
                "list_worker_requests",
                "requests-database-failure",
                json!({}),
            ),
        ] {
            let result = invoke(&mut supervisor, &registry, name, input, call).await;
            assert_failure(task_id, &health_run, &result, name, call, "supervisor", &[]);
        }
    }
}
