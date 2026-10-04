// Built-in tool implementations. Every tool runs natively in Rust, in this
// process, for every agent kind — assistant, fleet worker and subagent.
// Which tools an agent gets varies by PAMode — see register_for_mode().

pub mod assign_task;
pub mod assign_tasks_batch;
pub mod bash;
pub mod create_task;
pub mod edit_file;
pub mod get_task_tool;
pub mod git_commit;
pub mod glob;
pub mod graph_prompts;
pub mod graph_tools;
pub mod grep;
pub mod list_directory;
pub mod list_tasks_tool;
pub mod planner;
pub mod read_file;
pub mod spawn_parallel_subagents;
pub mod spawn_subagent;
pub mod update_task_status_tool;
pub mod write_file;
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
pub mod append_file;
pub mod check_file;
pub mod e2e_probe;
pub mod external_mcp;
pub mod get_memory;
pub mod list_memories;
pub mod project_command;
pub mod propose_memory;
pub mod reconcile_worker_operation;
pub mod request_supervisor_action;
pub mod search_memory;
pub mod validate;
// Path-jail helper used by write_file / edit_file / append_file / bash to
// refuse any operation whose resolved absolute path escapes the worker's
// configured workspace. Load-bearing safety net — see
// docs/endtoendrun/14may2026/07-workspace-escape-rootcause.md.
pub mod path_jail;
// Process-wide per-file async mutex registry. Used by write_file, edit_file,
// and append_file, so every writer contends on the same lock when
// targeting the same path. See path_lock.rs for the design notes.
pub mod path_lock;
// Doc 09 T2.4 — module-level tracker of recent bash invocations that look
// like build / test / smoke commands. update_task_status_tool reads from
// it to refuse qc_review transitions that have no proof a build was run.
pub mod analyze_diff_tool;
pub mod annotate_file_tool;
pub mod build_evidence;
pub mod claim_task_tool;
pub mod set_task_budget_tool;
pub mod suggest_worker_tool;
// Ruflo Feature 5 — Phase-Based Task Decomposition
pub mod create_epic_tool;
pub mod create_phased_task_tool;
pub mod get_epic_progress_tool;
pub mod get_ready_tasks_in_phase_tool;
// Supervisor escalation queue tools — required by the Supervisor recipe Step 1a.
pub mod list_worker_requests_tool;
pub mod resolve_worker_request_tool;
// Arcana Communication Protocol (ACP) — structured worker↔worker messaging.
// Inbox lives in `worker_messages` SQLite table; reads/writes guarded by the
// existing budget / orphan / build-evidence machinery because the target
// worker reads its inbox during its own scheduled tick, not out-of-band.
pub mod acknowledge_worker_message;
pub mod list_stale_worker_messages;
pub mod list_worker_health;
pub mod list_worker_messages;
pub mod send_worker_message;

use super::{tool_registry::ToolRegistry, PAMode};
use std::sync::Arc;

/// Register all tools (all modes) — used when no PAMode context is available.
pub fn register_all(registry: &mut ToolRegistry) {
    register_for_mode(registry, PAMode::FastDevelopment);
}

// P8-50 — `register_for_mcp_worker` is DELETED (~80 lines).
//
// It registered a tool subset for a fleet worker reached over MCP,
// reached over MCP. It had **zero callers**, and being `pub` it
// produced no dead-code warning — so it compiled cleanly for months
// while describing a dispatch path that no longer existed.

/// Register tools filtered by PA mode:
/// - FastDevelopment: all file/execution tools + task board tools + planner
/// - Review:          **the same arm**, byte for byte. The difference is
///                    posture, set by the prompt — not capability, and not the
///                    permission policy either: `agent_commands.rs:246` passes
///                    `ManualApproval` as a literal for both (`P8-57`).
/// - Delegate:        `create_task`, `planner` and the four memory tools —
///                    zero file and execution tools.
///
/// ⚠ The sentence that used to close this comment was **false**: *"Task board
/// tools (list_tasks, update_task_status, create_task) and planner are
/// available in all modes."* `Delegate` registers `create_task` and never
/// registered `list_tasks` or `update_task_status`, so a reader planning a
/// Delegate-mode feature around reading the board would have built against a
/// tool that is not there. It also justified itself by a subsystem that no
/// longer exists (*"without resorting to writing Python scripts"*).
pub fn register_for_mode(registry: &mut ToolRegistry, mode: PAMode) {
    match mode {
        PAMode::FastDevelopment | PAMode::Review => {
            // File & execution tools
            registry.register(Arc::new(read_file::ReadFileTool));
            registry.register(Arc::new(write_file::WriteFileTool));
            registry.register(Arc::new(edit_file::EditFileTool));
            registry.register(Arc::new(bash::BashTool));
            registry.register(Arc::new(git_commit::GitCommitTool));
            registry.register(Arc::new(project_command::ProjectCommandTool));
            registry.register(Arc::new(external_mcp::McpListToolsTool));
            registry.register(Arc::new(external_mcp::McpCallTool));
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
            registry.register(Arc::new(
                request_supervisor_action::RequestSupervisorActionTool,
            ));
            // Planner tool — available in all modes for plan management
            registry.register(Arc::new(planner::PlannerTool));
            // Subagent tools — Phase 4.9. Available to PA, Workers, and Supervisor
            // for fan-out / context-isolation work. Not exposed in Delegate mode
            // (that mode exists to keep PA from doing direct execution).
            registry.register(Arc::new(spawn_subagent::SpawnSubagentTool));
            registry.register(Arc::new(
                spawn_parallel_subagents::SpawnParallelSubagentsTool,
            ));
            // Doc 04 Phase 3 — QC fan-out tool (parity with MCP registration).
            registry.register(Arc::new(spawn_qc_review::SpawnQcReviewTool));
            // Doc 09 T3.1 / T4.2 — interface spec + e2e smoke probe (PA parity).
            registry.register(Arc::new(write_interface_spec::WriteInterfaceSpecTool));
            registry.register(Arc::new(e2e_probe::E2eProbeTool));
            // ── The memory gateway (P3-39) ──────────────────────────
            // `remember_this` was DELETED, not deprecated. It gave any agent an
            // unrestricted write at `confidence = 1.0` labelled `user_explicit`
            // — so a fleet worker could put a sentence at the top of the user's
            // "What I know about you". An agent proposes now; the gateway
            // decides, and assigns provenance from the runtime's own identity.
            registry.register(Arc::new(search_memory::SearchMemoryTool));
            registry.register(Arc::new(get_memory::GetMemoryTool));
            registry.register(Arc::new(list_memories::ListMemoriesTool));
            registry.register(Arc::new(propose_memory::ProposeMemoryTool));
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
            registry.register(Arc::new(
                get_ready_tasks_in_phase_tool::GetReadyTasksInPhaseTool,
            ));
            // Supervisor escalation queue tools — also available in PA modes
            // so Razi can manually drain the queue from chat if needed.
            registry.register(Arc::new(list_worker_requests_tool::ListWorkerRequestsTool));
            registry.register(Arc::new(
                resolve_worker_request_tool::ResolveWorkerRequestTool,
            ));
            // ACP tools available in PA so Razi can manually inspect / send
            // worker messages from the chat tab for debugging the fleet.
            registry.register(Arc::new(send_worker_message::SendWorkerMessageTool));
            registry.register(Arc::new(list_worker_messages::ListWorkerMessagesTool));
            registry.register(Arc::new(
                list_stale_worker_messages::ListStaleWorkerMessagesTool,
            ));
            registry.register(Arc::new(list_worker_health::ListWorkerHealthTool));
            registry.register(Arc::new(
                reconcile_worker_operation::ReconcileWorkerOperationTool,
            ));
            registry.register(Arc::new(
                acknowledge_worker_message::AcknowledgeWorkerMessageTool,
            ));
        }
        PAMode::Delegate => {
            registry.register(Arc::new(create_task::CreateTaskTool));
            // Planner is task management, not file I/O — safe in Delegate mode
            registry.register(Arc::new(planner::PlannerTool));
            // ── The memory gateway (P3-39) ──────────────────────────
            // `remember_this` was DELETED, not deprecated. It gave any agent an
            // unrestricted write at `confidence = 1.0` labelled `user_explicit`
            // — so a fleet worker could put a sentence at the top of the user's
            // "What I know about you". An agent proposes now; the gateway
            // decides, and assigns provenance from the runtime's own identity.
            registry.register(Arc::new(search_memory::SearchMemoryTool));
            registry.register(Arc::new(get_memory::GetMemoryTool));
            registry.register(Arc::new(list_memories::ListMemoriesTool));
            registry.register(Arc::new(propose_memory::ProposeMemoryTool));
        }
    }
}
