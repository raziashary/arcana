// AutoDev Rust backend library root.
// Module declarations; main.rs and tauri setup reference this.

pub mod errors;
pub mod logging;
pub mod events;
pub mod state;
pub mod vault;
pub mod session;
pub mod provider;
pub mod cost;
pub mod agent;
pub mod commands;
pub mod db;
pub mod recipes;

// Register all Tauri commands on the builder from within the lib crate.
// generate_handler! must run here so the __cmd__* macros (produced by
// #[tauri::command]) are in scope. main.rs calls this instead of using
// generate_handler! directly.
pub fn register_commands(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    use commands::{
        agent_commands::{cancel_agent, respond_to_permission, send_message},
        session_commands::{create_session, delete_session, get_session, get_session_messages, list_sessions, update_session_title},
        settings_commands::{
            get_active_provider, get_cost_today, get_default_system_prompt, get_system_prompt,
            list_providers, set_active_provider, set_system_prompt, test_provider_connection,
        },
        vault_commands::{
            delete_github_pat, delete_provider_key, get_github_pat, get_provider_key,
            store_github_pat, store_provider_key,
        },
        task_commands::{
            list_tasks, get_task, update_task_status, delete_task,
        },
        autodev_instructions_commands::{
            get_autodev_instructions, set_autodev_instructions, delete_autodev_instructions,
            check_autodev_instructions_exists, get_autodev_instructions_char_count,
        },
        recipe_commands::{list_recipes, get_recipe},
    };
    builder.invoke_handler(tauri::generate_handler![
        // Agent
        send_message,
        cancel_agent,
        respond_to_permission,
        // Sessions
        list_sessions,
        get_session,
        get_session_messages,
        create_session,
        update_session_title,
        delete_session,
        // Settings / providers
        list_providers,
        test_provider_connection,
        set_active_provider,
        get_active_provider,
        get_cost_today,
        get_system_prompt,
        set_system_prompt,
        get_default_system_prompt,
        // Vault
        store_provider_key,
        get_provider_key,
        delete_provider_key,
        store_github_pat,
        get_github_pat,
        delete_github_pat,
        // Tasks (Internal Board)
        list_tasks,
        get_task,
        update_task_status,
        delete_task,
        // AUTODEV.md Instructions
        get_autodev_instructions,
        set_autodev_instructions,
        delete_autodev_instructions,
        check_autodev_instructions_exists,
        get_autodev_instructions_char_count,
        // Recipes
        list_recipes,
        get_recipe,
    ])
}
