// The Tool trait — uniform interface for all built-in tools.
// Mirrors Claude Code's Tool interface and Goose's Tool trait.
// Each tool declares: name, description, JSON schema, read-only flag, execute().

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output:   String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(output: impl Into<String>) -> Self {
        Self { output: output.into(), is_error: false }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self { output: message.into(), is_error: true }
    }
}

/// Context passed to every tool execution — working directory, session, etc.
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id:  String,
    pub working_dir: String,
    pub call_id:     String,
    /// K.11 — Subagent nesting depth (0 = top-level agent, n+1 = subagent
    /// spawned by an agent at depth n). Spawn tools read this to set the
    /// child's depth and guard against unbounded recursion.
    pub depth:       u32,
    /// Agent role (e.g., "supervisor", "worker-backend", "worker-frontend")
    /// Used by tools like request_supervisor_action to identify the worker.
    pub agent_role:  Option<String>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;

    /// Read-only tools can run concurrently; write tools run serially.
    /// Pattern from Claude Code's isConcurrencySafe().
    fn is_read_only(&self) -> bool { false }

    /// Requires explicit user approval before execution.
    fn requires_permission(&self) -> bool { !self.is_read_only() }

    async fn execute(
        &self,
        input: serde_json::Value,
        ctx:   &ToolContext,
    ) -> ToolResult;
}
