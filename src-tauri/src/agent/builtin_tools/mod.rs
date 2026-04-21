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
pub mod list_tasks_tool;
pub mod update_task_status_tool;
pub mod assign_task;

use std::sync::Arc;
use super::{PAMode, tool_registry::ToolRegistry};

/// Register all tools (all modes) — used when no PAMode context is available.
pub fn register_all(registry: &mut ToolRegistry) {
    register_for_mode(registry, PAMode::FastDevelopment);
}

/// Register tools filtered by PA mode:
/// - FastDevelopment: all file/execution tools + task board tools (list_tasks, update_task_status, create_task)
/// - Review:          same as FastDevelopment (permission layer is at execution time)
/// - Delegate:        only create_task — zero file tools
///
/// Task board tools (list_tasks, update_task_status, create_task) are available in FastDevelopment
/// so that recipe agents (Supervisor, Workers) can manage the board without resorting to
/// writing Python scripts. This is the primary fix for workspace pollution.
pub fn register_for_mode(registry: &mut ToolRegistry, mode: PAMode) {
    match mode {
        PAMode::FastDevelopment | PAMode::Review => {
            // File & execution tools
            registry.register(Arc::new(read_file::ReadFileTool));
            registry.register(Arc::new(write_file::WriteFileTool));
            registry.register(Arc::new(edit_file::EditFileTool));
            registry.register(Arc::new(bash::BashTool));
            registry.register(Arc::new(list_directory::ListDirectoryTool));
            registry.register(Arc::new(grep::GrepTool));
            registry.register(Arc::new(glob::GlobTool));
            // Task board tools — recipe agents (Supervisor, Workers) use these directly
            registry.register(Arc::new(list_tasks_tool::ListTasksTool));
            registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
            registry.register(Arc::new(assign_task::AssignTaskTool));
            registry.register(Arc::new(create_task::CreateTaskTool));
        }
        PAMode::Delegate => {
            registry.register(Arc::new(create_task::CreateTaskTool));
        }
    }
}
