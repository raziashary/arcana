// Built-in tool implementations.
// PA mode: tools run natively in Rust.
// Worker mode: tools are forwarded to core-orchestrator with __autodev suffix.
// Tool availability varies by PAMode — see register_for_mode().

pub mod read_file;
pub mod write_file;
pub mod edit_file;
pub mod bash;
pub mod list_directory;
pub mod grep;
pub mod glob;
pub mod create_task;

use std::sync::Arc;
use super::{PAMode, tool_registry::ToolRegistry};

/// Register all tools (all modes) — used when no PAMode context is available.
pub fn register_all(registry: &mut ToolRegistry) {
    register_for_mode(registry, PAMode::FastDevelopment);
}

/// Register tools filtered by PA mode:
/// - FastDevelopment: all file/execution tools
/// - Review:          same tools (permission layer is at execution time)
/// - Delegate:        only create_task — zero file tools
pub fn register_for_mode(registry: &mut ToolRegistry, mode: PAMode) {
    match mode {
        PAMode::FastDevelopment | PAMode::Review => {
            registry.register(Arc::new(read_file::ReadFileTool));
            registry.register(Arc::new(write_file::WriteFileTool));
            registry.register(Arc::new(edit_file::EditFileTool));
            registry.register(Arc::new(bash::BashTool));
            registry.register(Arc::new(list_directory::ListDirectoryTool));
            registry.register(Arc::new(grep::GrepTool));
            registry.register(Arc::new(glob::GlobTool));
        }
        PAMode::Delegate => {
            registry.register(Arc::new(create_task::CreateTaskTool));
        }
    }
}
