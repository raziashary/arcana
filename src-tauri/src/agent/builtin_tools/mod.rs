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
pub mod assign_tasks_batch;
pub mod planner;
pub mod spawn_subagent;
pub mod spawn_parallel_subagents;
// Doc 04 Phase 3 — per-task QC reviewer fan-out. Only the qc-dispatcher
// recipe calls this. Atomically claims the task (`under_review_by`),
// spawns a sonnet-backed ephemeral reviewer, schedules cleanup +
// cost-attribution on terminal status.
pub mod spawn_qc_review;
// Doc 09 T3.1 — backend writes INTERFACE.md before qc_review on routing
// tasks. Frontend reads it before writing any consumer.
pub mod write_interface_spec;
// Doc 09 T4.2 — fleet-level smoke probe: bootstraps backend + frontend,
// curls an endpoint, looks for fingerprint text in the rendered page.
pub mod e2e_probe;
pub mod request_supervisor_action;
pub mod search_memory;
pub mod remember_this;
pub mod validate;
pub mod check_file;
pub mod append_file;
// Path-jail helper used by write_file / edit_file / append_file / bash to
// refuse any operation whose resolved absolute path escapes the worker's
// configured workspace. Load-bearing safety net — see
// docs/endtoendrun/14may2026/07-workspace-escape-rootcause.md.
pub mod path_jail;
// Process-wide per-file async mutex registry. Used by write_file, edit_file,
// append_file, and the MCP tool_adapter so PA-mode and Mode 2 callers
// contend on the same lock when targeting the same path. See
// path_lock.rs for the design notes.
pub mod path_lock;
// Doc 09 T2.4 — module-level tracker of recent bash invocations that look
// like build / test / smoke commands. update_task_status_tool reads from
// it to refuse qc_review transitions that have no proof a build was run.
pub mod build_evidence;
pub mod suggest_worker_tool;
pub mod claim_task_tool;
pub mod analyze_diff_tool;
pub mod annotate_file_tool;
pub mod set_task_budget_tool;
// Ruflo Feature 5 — Phase-Based Task Decomposition
pub mod create_epic_tool;
pub mod create_phased_task_tool;
pub mod get_epic_progress_tool;
pub mod get_ready_tasks_in_phase_tool;
// Supervisor escalation queue tools — required by the Supervisor recipe Step 1a.
pub mod list_worker_requests_tool;
pub mod resolve_worker_request_tool;

use std::sync::Arc;
use super::{PAMode, tool_registry::ToolRegistry};

/// Register all tools (all modes) — used when no PAMode context is available.
pub fn register_all(registry: &mut ToolRegistry) {
    register_for_mode(registry, PAMode::FastDevelopment);
}

/// Register the restricted tool set exposed via the MCP server to fleet workers.
/// Workers need file/execution tools, task-board read + status-update access,
/// and subagent spawning for the Subagent Protocol documented in their recipes.
///
/// Excluded: assign_task, create_task, planner.
/// - assign_task / create_task: Supervisor-only operations. Workers that bypass
///   the task-board routing would undermine the QC gate and work-stealing prevention.
/// - planner: plan compilation is a PA/Supervisor operation, not a per-worker concern.
///
/// spawn_subagent / spawn_parallel_subagents ARE included because:
/// - All worker recipes document a Subagent Protocol that depends on them.
/// - The SubagentPool hard caps (MAX_GLOBAL=16, MAX_PER_PARENT=10) and the
///   depth guard (MAX_NESTING_DEPTH=3) in each tool already prevent unbounded
///   fan-out — recipe-level "use sparingly" is a cost advisory, not the safety net.
pub fn register_for_mcp_worker(registry: &mut ToolRegistry) {
    // File & execution tools — workers need these for their core task
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(edit_file::EditFileTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    registry.register(Arc::new(check_file::CheckFileTool));
    registry.register(Arc::new(append_file::AppendFileTool));
    // Task board — read + own-status updates only
    registry.register(Arc::new(list_tasks_tool::ListTasksTool));
    registry.register(Arc::new(get_task_tool::GetTaskTool));
    registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
    // Worker→Supervisor communication — workers can request action without task-creation power
    registry.register(Arc::new(request_supervisor_action::RequestSupervisorActionTool));
    // Routing — Supervisor uses this before assign_task; safe to expose to all workers
    registry.register(Arc::new(suggest_worker_tool::SuggestWorkerTool));
    // Orphan detection — workers call this instead of update_task_status(in_progress)
    registry.register(Arc::new(claim_task_tool::ClaimTaskTool));
    // Diff analysis — QC Agent uses this to score and classify PR diffs before review
    registry.register(Arc::new(analyze_diff_tool::AnalyzeDiffTool));
    // Inline annotations — QC Agent (and workers) write review notes pinned to specific lines
    registry.register(Arc::new(annotate_file_tool::AnnotateFileTool));
    // Budget management — Supervisor uses this before assigning tasks (Ruflo Feature 4)
    registry.register(Arc::new(set_task_budget_tool::SetTaskBudgetTool));
    // Ruflo Feature 5 — Epic read tools exposed so workers can check phase progress
    registry.register(Arc::new(get_epic_progress_tool::GetEpicProgressTool));
    registry.register(Arc::new(get_ready_tasks_in_phase_tool::GetReadyTasksInPhaseTool));
    // Subagent tools — workers document Subagent Protocols in their recipes.
    // Pool hard caps + depth guard enforce safety; recipes provide cost-advisory guidance.
    // NOTE: subagent MCP callers must pass __depth on every call so the depth guard
    // in SpawnSubagentTool reflects actual nesting (tool_adapter.rs reads __depth).
    registry.register(Arc::new(spawn_subagent::SpawnSubagentTool));
    registry.register(Arc::new(spawn_parallel_subagents::SpawnParallelSubagentsTool));
    // Doc 04 Phase 3 — QC fan-out tool, called only by qc-dispatcher recipe.
    // Exposed at the MCP-worker layer because Mode 2 LivingAgents reach all
    // tools through the MCP server; recipe-level "only the dispatcher" is
    // enforced by the recipe instructions, not by the registry.
    registry.register(Arc::new(spawn_qc_review::SpawnQcReviewTool));
    // Doc 09 T3.1 / T4.2 — interface spec + e2e smoke probe tools.
    registry.register(Arc::new(write_interface_spec::WriteInterfaceSpecTool));
    registry.register(Arc::new(e2e_probe::E2eProbeTool));
    // Task dispatch — Supervisor MUST have assign_task and create_task.
    // Domain workers (backend, frontend, …) are instructed in their recipes to
    // use request_supervisor_action(decompose_audit) instead of create_task directly.
    // assign_task validates worker_slug against VALID_WORKERS and atomically
    // sets status='assigned', so domain workers calling it would still produce valid
    // state — the recipe instruction is the authoritatively enforced constraint.
    registry.register(Arc::new(assign_task::AssignTaskTool));
    registry.register(Arc::new(assign_tasks_batch::AssignTasksBatchTool));
    registry.register(Arc::new(create_task::CreateTaskTool));
    // Planner — designed for all fleet agents (see planner.rs comment).
    // Supervisor: breakDownEpic, Workers: claimItem / createSubplan / submitForQC.
    registry.register(Arc::new(planner::PlannerTool));
    // Supervisor escalation queue — read pending requests + mark them resolved.
    // The supervisor recipe Step 1a calls these every tick; without them the
    // Supervisor cannot drain the queue and worker escalations pile up forever.
    // Safe to expose to all workers — reading and resolving requests is idempotent
    // and the resolve action requires an explicit request id.
    registry.register(Arc::new(list_worker_requests_tool::ListWorkerRequestsTool));
    registry.register(Arc::new(resolve_worker_request_tool::ResolveWorkerRequestTool));
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
            registry.register(Arc::new(check_file::CheckFileTool));
            registry.register(Arc::new(append_file::AppendFileTool));
            // Task board tools — recipe agents (Supervisor, Workers) use these directly
            registry.register(Arc::new(list_tasks_tool::ListTasksTool));
            registry.register(Arc::new(get_task_tool::GetTaskTool));
            registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
            registry.register(Arc::new(assign_task::AssignTaskTool));
            registry.register(Arc::new(assign_tasks_batch::AssignTasksBatchTool));
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
            // Doc 04 Phase 3 — QC fan-out tool (parity with MCP registration).
            registry.register(Arc::new(spawn_qc_review::SpawnQcReviewTool));
            // Doc 09 T3.1 / T4.2 — interface spec + e2e smoke probe (PA parity).
            registry.register(Arc::new(write_interface_spec::WriteInterfaceSpecTool));
            registry.register(Arc::new(e2e_probe::E2eProbeTool));
            // Memory tools (Phase 4.7) — available in all non-delegate modes
            registry.register(Arc::new(search_memory::SearchMemoryTool));
            registry.register(Arc::new(remember_this::RememberThisTool));
            // Routing — suggest_worker for PA-mode routing awareness
            registry.register(Arc::new(suggest_worker_tool::SuggestWorkerTool));
            // Orphan detection — claim_task records PID for stale-task recovery
            registry.register(Arc::new(claim_task_tool::ClaimTaskTool));
            // Diff analysis — available in PA and review modes for manual QC workflows
            registry.register(Arc::new(analyze_diff_tool::AnalyzeDiffTool));
            // Inline annotations — PA can pin review notes to code lines
            registry.register(Arc::new(annotate_file_tool::AnnotateFileTool));
            // Budget management — available in PA/review so manual workflows can budget tasks
            registry.register(Arc::new(set_task_budget_tool::SetTaskBudgetTool));
            // Ruflo Feature 5 — Epic & Phase tools (Supervisor uses these to decompose work)
            registry.register(Arc::new(create_epic_tool::CreateEpicTool));
            registry.register(Arc::new(create_phased_task_tool::CreatePhasedTaskTool));
            registry.register(Arc::new(get_epic_progress_tool::GetEpicProgressTool));
            registry.register(Arc::new(get_ready_tasks_in_phase_tool::GetReadyTasksInPhaseTool));
            // Supervisor escalation queue tools — also available in PA modes
            // so Razi can manually drain the queue from chat if needed.
            registry.register(Arc::new(list_worker_requests_tool::ListWorkerRequestsTool));
            registry.register(Arc::new(resolve_worker_request_tool::ResolveWorkerRequestTool));
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
