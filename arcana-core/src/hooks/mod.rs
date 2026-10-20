// Hooks system module — Phase 3.1
// Event-driven extensibility at key agent/tool/file/git/fleet lifecycle points.
// Hooks are stored in ~/.arcana/config.db and executed at each named event.
//
// Usage:
//   HookRegistry::fire(&HookType::PostFileWrite, &ctx).await;
//   let gate = HookRegistry::fire_gate(&HookType::PreToolUse, &ctx).await;
//   let (logs, gate) = HookRegistry::fire_with_logs(&HookType::PreToolUse, &ctx).await;
// `fire` answers "may I continue"; `fire_gate` also says whether a refusal was a
// policy decision or the hooks store being unreadable.

pub mod conditions;
pub mod db;
pub mod ecosystem;
pub mod executor;
pub mod registry;
pub mod types;

pub use ecosystem::Ecosystem;
pub use registry::HookRegistry;
pub use types::{HookContext, HookType, ToolCallContext};
