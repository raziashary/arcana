// `propose_memory` — the gateway's write surface (`P3-39`).
//
// Replaces `remember_this`, which gave any agent an **unrestricted write** into
// the user's long-term memory at `confidence = 1.0` labelled `user_explicit`.
// Combined with a recall path that had no scope predicate and ordered by
// confidence, an autonomous fleet worker could put a sentence at the top of the
// user's *"What I know about you and this project"* — indistinguishable from
// something the user had said.
//
// The name is the contract: an agent **proposes**, the gateway **decides**.

use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::memory::gateway::{self, Proposal, Proposer, Verdict};
use crate::memory::types::MemoryTier;

pub struct ProposeMemoryTool;

#[async_trait]
impl Tool for ProposeMemoryTool {
    /// `Board`: durable state other runs read back.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Board
    }

    fn name(&self) -> &str {
        "propose_memory"
    }

    fn description(&self) -> &str {
        "Propose a fact for long-term memory. A deterministic gateway decides \
         whether to keep it — checking length, duplication against existing \
         memories in the same scope, and how much confidence your role is \
         permitted to assert. The result tells you what was decided and why.\n\n\
         Set user_stated=true ONLY when the user has just told you this in so \
         many words. It records the memory as the user's own statement, at full \
         confidence, and everything else is stored as your observation at a \
         lower one.\n\n\
         Scope determines who can recall it: 'project' (this workspace only) or \
         'global' (everywhere). Prefer 'project' — a global memory is recalled \
         in every other repository too."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The fact, in one sentence. Must carry real content — trivial strings are refused."
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "global"],
                    "description": "'project' = this workspace only (preferred). 'global' = recalled everywhere."
                },
                "user_stated": {
                    "type": "boolean",
                    "description": "True only if the user just stated this explicitly. Governs provenance and confidence."
                },
                "confidence": {
                    "type": "number",
                    "description": "How sure you are, 0.0–1.0. Capped by what your role may assert; you may ask for less."
                },
                "source_file": {
                    "type": "string",
                    "description": "Optional file this was learned from, so it can be checked later."
                },
                "source_line": {
                    "type": "integer",
                    "description": "Optional line within source_file."
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let content = match input["content"].as_str() {
            Some(s) if !s.trim().is_empty() => s.to_string(),
            _ => return ToolResult::error("'content' is required and must be a non-empty string"),
        };

        let scope = match input["scope"].as_str() {
            Some("global") => "global",
            _ => "project",
        };
        let scope_ref = if scope == "global" {
            String::new()
        } else {
            ctx.working_dir.clone()
        };

        // ⚠ The proposer is DERIVED from the identity the runtime already
        // threads through `ToolContext`, never taken from the input. A model
        // that could name its own role could elect to be the user, and the
        // ceiling below would mean nothing.
        let user_stated = input["user_stated"].as_bool().unwrap_or(false);
        let proposer = Proposer::from_agent_role(&ctx.agent_role, user_stated);

        let claimed = input["confidence"].as_f64().unwrap_or(0.8) as f32;

        let proposal = Proposal {
            content,
            tier: MemoryTier::Semantic,
            scope: scope.to_string(),
            scope_ref,
            proposer,
            claimed_confidence: claimed,
            source_file: input["source_file"].as_str().map(String::from),
            source_line: input["source_line"].as_i64(),
        };

        let db = match crate::memory::db::MemoryDb::open() {
            Ok(db) => db,
            Err(e) => return ToolResult::error(format!("memory unavailable: {e}")),
        };

        match gateway::propose(&db.conn, &proposal) {
            Ok(Verdict::Stored { id, confidence }) => {
                info!(
                    "[propose_memory] stored {id} scope={scope} proposer={proposer:?} confidence={confidence:.2}"
                );
                ToolResult::success(format!(
                    "Stored as {id} (scope: {scope}, recorded as {}, confidence {confidence:.2}).",
                    proposal_source_label(proposer)
                ))
            }
            // ⚠ A refusal is a SUCCESS with a reason, not an error. The gateway
            // deciding not to keep something is the system working; returning an
            // error would teach the model to retry a policy decision.
            Ok(Verdict::Rejected { reason }) => ToolResult::success(format!(
                "Not stored: {reason}. Nothing is wrong — the gateway decides what is worth keeping."
            )),
            Err(e) => ToolResult::error(format!("Failed to evaluate the proposal: {e}")),
        }
    }
}

fn proposal_source_label(p: Proposer) -> &'static str {
    match p {
        Proposer::User => "something you told me",
        Proposer::Assistant => "my own observation",
        Proposer::AutonomousAgent => "an agent's observation",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(role: &str) -> ToolContext {
        ToolContext {
            session_id: "s1".into(),
            working_dir: "/proj".into(),
            call_id: "c1".into(),
            depth: 0,
            agent_role: role.into(),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
        }
    }

    #[test]
    fn the_schema_does_not_let_the_model_name_its_own_provenance() {
        // 🔴 The defect this tool replaces: `remember_this` stored every write
        // as `user_explicit` at confidence 1.0. There is deliberately no
        // `source` or `proposer` field here — provenance is derived from the
        // runtime's own identity, so a model cannot elect to be the user.
        let schema = ProposeMemoryTool.input_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(!props.contains_key("source"), "provenance must not be caller-supplied");
        assert!(!props.contains_key("proposer"), "the proposer must not be caller-supplied");
        assert!(!props.contains_key("importance"), "importance is derived, not claimed");
    }

    #[test]
    fn a_worker_role_becomes_an_autonomous_proposer() {
        let c = ctx("worker-backend");
        assert_eq!(
            Proposer::from_agent_role(&c.agent_role, false),
            Proposer::AutonomousAgent
        );
        // …and even claiming the user said it does not change the ROLE ceiling
        // unless the flag is set by the tool from a real user statement.
        assert_eq!(
            Proposer::from_agent_role(&c.agent_role, true),
            Proposer::User
        );
    }

    #[test]
    fn the_tool_declares_a_board_effect_so_it_is_prompted() {
        // A durable write into the user's memory is not a silent act.
        assert!(matches!(
            ProposeMemoryTool.effect(),
            crate::agent::tool_effect::ToolEffect::Board
        ));
        use crate::agent::permission::PermissionMode;
        assert!(ProposeMemoryTool
            .effect()
            .requires_prompt(&PermissionMode::ManualApproval));
    }
}
