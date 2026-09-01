// The Tool trait — uniform interface for all built-in tools.
// Mirrors Claude Code's Tool interface and Goose's Tool trait.
// Each tool declares: name, description, JSON schema, read-only flag, execute().

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
    /// Optional guidance appended to the tool result content before sending to the LLM.
    /// Use this for large/noisy results to direct the agent on how to consume them,
    /// e.g. "Large result — extract only what you need before proceeding."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            hint: None,
        }
    }

    /// Like `success` but attaches a hint string that is appended inline to the
    /// tool result sent to the LLM. Use for large outputs to guide summarisation.
    pub fn success_with_hint(output: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            hint: Some(hint.into()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            output: message.into(),
            is_error: true,
            hint: None,
        }
    }
}

/// Context passed to every tool execution — working directory, session, etc.
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub working_dir: String,
    pub call_id: String,
    /// K.11 — Subagent nesting depth (0 = top-level agent, n+1 = subagent
    /// spawned by an agent at depth n). Spawn tools read this to guard against
    /// unbounded recursion.
    pub depth: u32,
    /// Who is making this call: "supervisor", "worker-backend", "qc-agent", …
    ///
    /// NOT an Option. Every agent has an identity, and the three worker↔worker
    /// messaging tools hard-refuse without one — so an absent identity was not
    /// a neutral default, it silently disabled capabilities.
    pub agent_role: String,
}

/// Who an agent is, and how deeply nested it is.
///
/// Both fields are authored in Rust and threaded from the spawn site. They were
/// previously carried over the MCP boundary: `subagent/bridge.rs` injected
/// "Include `__depth: {n}` in every tool call" into the child's *prompt* and
/// `mcp_server/tool_adapter.rs` read it back off the arguments. That made the
/// recursion guard depend on the model choosing to report its own nesting depth
/// honestly, and the in-process path — which never went through the adapter —
/// hardcoded `depth: 0` and `agent_role: None` at all three construction sites.
///
/// See ADR-LOG-RESTRUCT-0001.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub role: String,
    pub depth: u32,
}

impl AgentIdentity {
    /// The interactive assistant. "supervisor" matches the role the shipped
    /// recipes already use for it (`recipes/defaults.rs:217`).
    pub fn supervisor() -> Self {
        Self {
            role: "supervisor".to_string(),
            depth: 0,
        }
    }

    /// A top-level fleet worker, identified by its recipe slug.
    pub fn worker(slug: impl Into<String>) -> Self {
        Self {
            role: slug.into(),
            depth: 0,
        }
    }

    /// A subagent spawned by this agent — one level deeper, by construction.
    pub fn child(&self, role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            depth: self.depth + 1,
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;

    /// Read-only tools can run concurrently; write tools run serially.
    /// Pattern from Claude Code's isConcurrencySafe().
    fn is_read_only(&self) -> bool {
        false
    }

    /// Requires explicit user approval before execution.
    fn requires_permission(&self) -> bool {
        !self.is_read_only()
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
}

#[cfg(test)]
mod identity_tests {
    use super::AgentIdentity;

    #[test]
    fn top_level_agents_start_at_depth_zero() {
        assert_eq!(AgentIdentity::supervisor().depth, 0);
        assert_eq!(AgentIdentity::supervisor().role, "supervisor");
        assert_eq!(AgentIdentity::worker("worker-backend").depth, 0);
        assert_eq!(
            AgentIdentity::worker("worker-backend").role,
            "worker-backend"
        );
    }

    #[test]
    fn each_generation_is_exactly_one_level_deeper() {
        let parent = AgentIdentity::worker("worker-backend");
        let child = parent.child("subagent-a");
        let grandchild = child.child("subagent-b");

        assert_eq!(child.depth, 1);
        assert_eq!(grandchild.depth, 2);
        assert_eq!(grandchild.role, "subagent-b");
        // The parent is untouched — depth is not shared mutable state.
        assert_eq!(parent.depth, 0);
    }

    /// The regression this whole change exists for.
    ///
    /// All three `ToolContext` construction sites in `tool_executor.rs` used to
    /// hardcode `depth: 0` and `agent_role: None`. That silently disabled the
    /// three worker-messaging tools, made `claim_task` attribute work to
    /// "unknown", and left the subagent recursion guard unable to fire on the
    /// in-process path — while the only site that set a real value lived in the
    /// MCP adapter that P0 deletes.
    ///
    /// Reading the source is the only way to assert this: the values are struct
    /// literals, so there is no runtime surface to interrogate.
    #[test]
    fn no_tool_context_site_hardcodes_identity() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent/tool_executor.rs");
        let source = std::fs::read_to_string(&path).expect("read tool_executor.rs");

        let sites = source.matches("ToolContext {").count();
        assert!(
            sites >= 3,
            "expected at least 3 ToolContext sites, found {sites}"
        );

        // NOT `!contains("agent_role: None")`: `agent_role` is a `String` now, so
        // that assertion is type-impossible and can never fail — it looks like a
        // guard and is worth nothing. Found by the P0 post-phase audit (F-09).
        //
        // The property that must hold is positive: EVERY construction site takes
        // both fields from the threaded identity. Assert that instead.
        let role_from_identity = source.matches("identity.role").count();
        let depth_from_identity = source.matches("identity.depth").count();

        assert_eq!(
            role_from_identity, sites,
            "{role_from_identity} of {sites} ToolContext sites take agent_role from \
             the threaded AgentIdentity. A site that hardcodes it silently disables \
             the ACP tools and the qc_review gate for that path."
        );
        assert_eq!(
            depth_from_identity, sites,
            "{depth_from_identity} of {sites} ToolContext sites take depth from the \
             threaded AgentIdentity. A hardcoded depth makes the subagent recursion \
             guard unable to fire on that path."
        );

        // A literal depth defeats the guard whatever value it is given.
        for forbidden in ["depth: 0", "depth: 1"] {
            assert!(
                !source.contains(forbidden),
                "a ToolContext is built with a literal `{forbidden}` instead of the \
                 threaded depth"
            );
        }
    }
}
