// send_worker_message — AutoDev Communication Protocol (ACP) outbound tool.
//
// One worker sends a structured message to another worker (or the supervisor).
// The message lands in `worker_messages` SQLite table and is pushed to the
// target's bound recipe via `recipes::scheduler::kick`, so the target picks
// it up on its next tick (sub-second under push, ≤ cron cadence otherwise).
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
use crate::recipes::defaults::FLEET_WORKERS;
use crate::recipes::scheduler;

pub struct SendWorkerMessageTool;

const MAX_BODY_BYTES: usize = 4096;
const VALID_KINDS: &[&str] = &["nudge", "question", "answer", "fyi", "broadcast"];

/// Recipients the protocol accepts. `*` is the broadcast wildcard, supervisor-only.
fn is_known_recipient(slug: &str) -> bool {
    slug == "supervisor"
        || slug == "qc-agent"
        || slug == "qc-dispatcher"
        || slug == "*"
        || FLEET_WORKERS.contains(&slug)
}

#[async_trait]
impl Tool for SendWorkerMessageTool {
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

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        // ── Validate `to` ────────────────────────────────────────────────────
        let to = match input["to"].as_str() {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'to' is required (worker slug or '*')"),
        };
        if !is_known_recipient(&to) {
            return ToolResult::error(format!(
                "'{to}' is not a known recipient. Valid: {}, supervisor, qc-agent, qc-dispatcher, or '*' (broadcast).",
                FLEET_WORKERS.join(", ")
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
        // ctx.agent_role is the canonical sender identity, set from
        // __agent_role at the MCP boundary. PA / interactive callers won't
        // have one; we refuse those so the inbox stays fleet-only and the
        // From column in Fleet Chat is always an agent slug, never "anon".
        // Identity is Rust-authored and always present now, and the
        // interactive assistant is "supervisor" — the role the shipped recipes
        // already give it (recipes/defaults.rs:217), and the one that dispatches
        // to workers. The old refusal was a proxy for "has an identity", which
        // interactive callers lacked; they no longer do, so this is a deliberate
        // widening (ADR-LOG-RESTRUCT-0007). "anon" remains impossible.
        let from = ctx.agent_role.trim().to_string();
        if from.is_empty() {
            return ToolResult::error(
                "send_worker_message refused: the calling agent has no role, so                  the message would have no identifiable sender."
            );
        }

        // ── Refuse self-addressed ────────────────────────────────────────────
        if from == to {
            return ToolResult::error(format!(
                "Cannot send a message to yourself ('{from}'). If you need a \
                 note-to-self, write to AUTODEV_PROGRESS.md instead."
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
            // Every fleet worker + supervisor's helpers, EXCLUDING the sender
            // (supervisor doesn't broadcast to itself).
            let mut all: Vec<String> = FLEET_WORKERS.iter().map(|s| s.to_string()).collect();
            all.push("qc-agent".into());
            all.push("qc-dispatcher".into());
            all.retain(|s| s != &from);
            all
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

            // Push the target's recipe so they read the inbox on the very
            // next event-loop tick instead of waiting up to a cron cadence.
            // Safe no-op for the qc-agent / qc-dispatcher slugs if they
            // aren't bound to a scheduled recipe.
            scheduler::kick(target);

            delivered.push(msg.id);
        }

        let summary = if targets.len() == 1 {
            format!("Message #{} delivered to '{}'.", delivered[0], targets[0])
        } else {
            format!(
                "Broadcast delivered to {} recipients (ids: {}).",
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
                "delivered_message_ids": delivered,
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
