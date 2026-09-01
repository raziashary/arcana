// Hooks system module — Phase 3.1
// Event-driven extensibility at key agent/tool/file/git/fleet lifecycle points.
// Hooks are stored in ~/.autodev/config.db and executed at each named event.
//
// Usage:
//   HookRegistry::fire(&HookType::PostFileWrite, &ctx).await;
//   let logs = HookRegistry::fire_with_logs(&HookType::PreToolUse, &ctx).await;

pub mod conditions;
pub mod db;
pub mod ecosystem;
pub mod executor;
pub mod registry;
pub mod types;

pub use ecosystem::Ecosystem;
pub use registry::HookRegistry;
pub use types::{HookContext, HookType, ToolCallContext};
