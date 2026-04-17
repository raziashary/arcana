// Skills system module — Phase 3.1
// Provides reusable .md workflow templates invocable via slash commands
// or programmatically by Fleet workers.
//
// ~/.autodev/skills/{category}/{skill-name}.md

pub mod types;
pub mod loader;
pub mod engine;
pub mod registry;
pub mod defaults;

pub use registry::SkillRegistry;
pub use engine::SkillEngine;
pub use defaults::ensure_default_skills;
