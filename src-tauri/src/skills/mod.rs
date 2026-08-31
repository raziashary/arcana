// Skills system module — Phase 3.1
// Provides reusable .md workflow templates invocable via slash commands
// or programmatically by Fleet workers.
//
// ~/.autodev/skills/{category}/{skill-name}.md

pub mod defaults;
pub mod engine;
pub mod loader;
pub mod registry;
pub mod types;
pub mod watcher;

pub use defaults::ensure_default_skills;
pub use engine::SkillEngine;
pub use registry::SkillRegistry;
pub use watcher::start_skill_watcher;
