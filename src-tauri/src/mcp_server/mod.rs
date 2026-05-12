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
