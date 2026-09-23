// Agent module — core agentic loop for PA mode.
// Architecture: AgentEngine drives a streaming LLM loop with tool execution.
// Pattern adopted from Goose's agent.rs reply_internal() and
// Claude Code's query.ts runQuery().
// See DEVELOPMENT_PLAN.md Section 9.4 for state machine.

pub mod builtin_tools;
pub mod compaction;
pub mod context;
pub mod engine;
pub mod permission;
pub mod tool_executor;
pub mod tool_registry;
pub mod terminal_event;
pub mod tool_effect;
pub mod tool_trait;

pub use engine::AgentEngine;
pub use permission::{PermissionMode, PermissionRequest, PermissionResponse};
pub use tool_registry::ToolRegistry;
pub use tool_trait::{Tool, ToolContext, ToolResult as ToolOutput};

/// PA Mode variants for the Mode Toggle (Phase 2)
/// Controls which tools are available and which system prompt is used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PAMode {
    /// Full tool access for direct coding (read_file, write_file, edit_file, bash, grep, glob)
    FastDevelopment,
    /// Same tools but with explicit permission prompts for write operations
    Review,
    /// No file tools, only create_task for delegating to workers
    Delegate,
}

impl PAMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PAMode::FastDevelopment => "fast_development",
            PAMode::Review => "review",
            PAMode::Delegate => "delegate",
        }
    }
}
