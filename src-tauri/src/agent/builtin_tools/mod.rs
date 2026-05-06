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
pub mod get_task_tool;
pub mod update_task_status_tool;
pub mod assign_task;
pub mod planner;
pub mod spawn_subagent;
pub mod spawn_parallel_subagents;
pub mod request_supervisor_action;
pub mod search_memory;
pub mod remember_this;

use std::sync::Arc;
use super::{PAMode, tool_registry::ToolRegistry};

/// Register all tools (all modes) — used when no PAMode context is available.
pub fn register_all(registry: &mut ToolRegistry) {
    register_for_mode(registry, PAMode::FastDevelopment);
}

/// Register the restricted tool set exposed via the MCP server to fleet workers.
/// Workers need file/execution tools and task-board read + status-update access.
/// Excluded: assign_task, create_task, planner, spawn_subagent, spawn_parallel_subagents.
/// This prevents workers from stealing other workers' tasks or fan-out spawning via MCP.
pub fn register_for_mcp_worker(registry: &mut ToolRegistry) {
    // File & execution tools — workers need these for their core task
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(edit_file::EditFileTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    // Task board — read + own-status updates only
    registry.register(Arc::new(list_tasks_tool::ListTasksTool));
    registry.register(Arc::new(get_task_tool::GetTaskTool));
    registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
    // Worker→Supervisor communication — workers can request action without task-creation power
    registry.register(Arc::new(request_supervisor_action::RequestSupervisorActionTool));
    // NOT exposed: assign_task, create_task, planner, spawn_subagent, spawn_parallel_subagents
}

/// Register tools filtered by PA mode:
/// - FastDevelopment: all file/execution tools + task board tools + planner
/// - Review:          same as FastDevelopment (permission layer is at execution time)
/// - Delegate:        create_task + planner — zero file tools
///
/// Task board tools (list_tasks, update_task_status, create_task) and planner are available
/// in all modes so that recipe agents (Supervisor, Workers) can manage tasks and plans
/// without resorting to writing Python scripts.
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
            registry.register(Arc::new(get_task_tool::GetTaskTool));
            registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
            registry.register(Arc::new(assign_task::AssignTaskTool));
            registry.register(Arc::new(create_task::CreateTaskTool));
            // Worker→Supervisor communication — available in all modes
            registry.register(Arc::new(request_supervisor_action::RequestSupervisorActionTool));
            // Planner tool — available in all modes for plan management
            registry.register(Arc::new(planner::PlannerTool));
            // Subagent tools — Phase 4.9. Available to PA, Workers, and Supervisor
            // for fan-out / context-isolation work. Not exposed in Delegate mode
            // (that mode exists to keep PA from doing direct execution).
            registry.register(Arc::new(spawn_subagent::SpawnSubagentTool));
            registry.register(Arc::new(spawn_parallel_subagents::SpawnParallelSubagentsTool));
            // Memory tools (Phase 4.7) — available in all non-delegate modes
            registry.register(Arc::new(search_memory::SearchMemoryTool));
            registry.register(Arc::new(remember_this::RememberThisTool));
        }
        PAMode::Delegate => {
            registry.register(Arc::new(create_task::CreateTaskTool));
            // Planner is task management, not file I/O — safe in Delegate mode
            registry.register(Arc::new(planner::PlannerTool));
            // Memory tools available in Delegate mode — read-only and write facts
            registry.register(Arc::new(search_memory::SearchMemoryTool));
            registry.register(Arc::new(remember_this::RememberThisTool));
        }
    }
}
