// Built-in tool implementations for Phase 1.
// These run natively in Rust for PA mode; Worker mode uses AutoDev tools
// passed as __autodev-suffixed tools to core-orchestrator.

pub mod read_file;
pub mod write_file;
pub mod bash;
pub mod list_directory;

use std::sync::Arc;
use super::tool_registry::ToolRegistry;

/// Register all Phase 1 built-in tools into a ToolRegistry.
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
}
