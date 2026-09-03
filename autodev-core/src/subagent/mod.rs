// Subagent module — Phase 4.9.
//
// Ephemeral subagents, run in-process on the same `AgentEngine` as every
// other agent. Spawned, awaited and dropped within one parent turn.
//
// See: docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md
//      docs/development/DEVELOPMENT_PLAN.md (Phase 4.9)
//
// Module map:
//   types.rs    — DTOs (config, handle, status, event, result)
//   slug.rs     — unique slug generator + reserved prefix
//   bridge.rs   — stateless lifecycle runner (HTTP + SSE parser)
//   pool.rs     — concurrency limits (global / per-parent / depth)
//   manager.rs  — registry + event pump + orphan cleanup

pub mod manager;
pub mod native_runner;
pub mod pool;
pub mod slug;
pub mod types;

pub use manager::SubagentManager;
pub use pool::{SubagentPool, MAX_GLOBAL_CONCURRENT, MAX_NESTING_DEPTH, MAX_PER_PARENT};
pub use types::{
    SpawnParallelSubagentsInput, SpawnSubagentInput, SubagentConfig, SubagentEvent, SubagentHandle,
    SubagentResult, SubagentStatus, SubagentSummary,
};
