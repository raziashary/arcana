// AutoDev MCP Server — exposes canonical Rust builtin_tools via MCP protocol.
//
// Architecture: The existing `Tool` trait implementations in `agent/builtin_tools/`
// are the single source of truth. This module bridges them into the Model Context
// Protocol so core-orchestrator LivingAgents (fleet workers) can discover and
// call them over Streamable HTTP instead of using the duplicated Python
// `autodev_handler.py`.
//
// See DEVELOPMENT_PLAN.md Phase 3.4 for full rationale.

pub mod tool_adapter;
pub mod server;
pub mod health_monitor;
pub mod port_status;

// Re-export the small port-resolution surface so other modules can target
// the actually-bound port without depending on the server module's internals.
pub use server::{current_mcp_port, is_ready};
pub use port_status::{McpPortStatus, snapshot as port_snapshot};
