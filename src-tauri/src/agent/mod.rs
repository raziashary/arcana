// Agent module — core agentic loop for PA mode.
// Architecture: AgentEngine drives a streaming LLM loop with tool execution.
// Pattern adopted from Goose's agent.rs reply_internal() and
// Claude Code's query.ts runQuery().
// See DEVELOPMENT_PLAN.md Section 9.4 for state machine.

pub mod tool_trait;
pub mod tool_registry;
pub mod tool_executor;
pub mod permission;
pub mod context;
pub mod engine;
pub mod compaction;
pub mod builtin_tools;

pub use engine::AgentEngine;
pub use tool_trait::{Tool, ToolResult as ToolOutput, ToolContext};
pub use tool_registry::ToolRegistry;
pub use permission::{PermissionMode, PermissionRequest, PermissionResponse};
