// Tauri command modules.
// Commands are thin wrappers that delegate to core modules.
// All commands return Result<T, AutoDevError> which serialises to the frontend.

pub mod agent_commands;
pub mod vault_commands;
pub mod settings_commands;
pub mod session_commands;
pub mod task_commands;
pub mod autodev_instructions_commands;
pub mod recipe_commands;
pub mod worker_commands;
pub mod model_config_commands;
pub mod github_mcp_commands;
pub mod workspace_commands;
pub mod file_commands;
pub mod log_commands;
pub mod sidecar_commands;
pub mod skill_commands;
pub mod hook_commands;
pub mod ide_commands;
pub mod git_commands;
pub mod startup_commands;
