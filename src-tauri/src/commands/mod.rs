// Tauri command modules.
// Commands are thin wrappers that delegate to core modules.
// All commands return Result<T, ArcanaError> which serialises to the frontend.

pub mod agent_commands;
pub mod analytics_commands;
pub mod annotation_commands;
pub mod arcana_instructions_commands;
pub mod browser_commands;
pub mod budget_commands;
pub mod diff_commands;
pub mod file_commands;
pub mod git_commands;
pub mod github_mcp_commands;
pub mod graph_commands;
pub mod hook_commands;
pub mod ide_commands;
pub mod log_commands;
pub mod memory_commands;
pub mod model_config_commands;
pub mod orphan_commands;
pub mod plan_commands;
pub mod plugin_commands;
pub mod recipe_commands;
pub mod routing_commands;
pub mod session_commands;
pub mod settings_commands;
// P8-51/P8-06 — `sidecar_commands` is deleted. Its three MCP-CLIENT
// commands (`list_mcp_tools`, `get_mcp_health`, `force_reconnect_mcp`)
// moved to `mcp_commands`, keeping their names, because they serve a live
// feature and were only ever neighbours of the sidecar code.
pub mod mcp_commands;
pub mod skill_commands;
pub mod startup_commands;
pub mod subagent_commands;
pub mod task_commands;
pub mod updater_commands;
pub mod vault_commands;
pub mod worker_commands;
pub mod workspace_commands;
// Arcana Communication Protocol — Fleet Chat query commands.
pub mod worker_message_commands;
