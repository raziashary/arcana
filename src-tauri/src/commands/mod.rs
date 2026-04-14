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
