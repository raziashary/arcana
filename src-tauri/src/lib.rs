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
pub mod sidecar;
pub mod skills;
pub mod hooks;

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
            list_tasks, get_task, update_task_status, delete_task, clear_all_tasks,
        },
        autodev_instructions_commands::{
            get_autodev_instructions, set_autodev_instructions, delete_autodev_instructions,
            check_autodev_instructions_exists, get_autodev_instructions_char_count,
        },
        recipe_commands::{list_recipes, get_recipe, create_recipe, update_recipe, delete_recipe, list_recipe_backups, restore_recipe_backup, generate_recipe_yaml},
        model_config_commands::{get_model_config, set_model_config, delete_model_config},
        github_mcp_commands::{get_github_mcp_config, set_github_mcp_config},
        worker_commands::{
            list_workers, get_worker_by_slug, create_worker, update_worker_status,
            update_worker_model, update_worker_repos, bind_worker_recipe,
            delete_worker, start_worker_recipe, stop_worker_recipe,
        },
        workspace_commands::{get_workspace_dir, set_workspace_dir, pick_workspace_dir},
        file_commands::search_workspace_files,
        log_commands::{list_log_files, read_log_file, clear_log_files},
        sidecar_commands::{get_sidecar_status, check_sidecar_connectivity, start_sidecar, stop_sidecar},
        skill_commands::{
            list_skills, get_skill, search_skills, skills_by_category,
            execute_skill, validate_skill_parameters, reload_skills, import_skill,
        },
        hook_commands::{
            list_hooks, enable_hook, disable_hook, add_hook, remove_hook,
            test_hook, get_hook_logs,
        },
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
        clear_all_tasks,
        // AUTODEV.md Instructions
        get_autodev_instructions,
        set_autodev_instructions,
        delete_autodev_instructions,
        check_autodev_instructions_exists,
        get_autodev_instructions_char_count,
        // Recipes
        list_recipes,
        get_recipe,
        create_recipe,
        update_recipe,
        delete_recipe,
        list_recipe_backups,
        restore_recipe_backup,
        generate_recipe_yaml,
        // Model config (temperature, top_p, reasoning effort, thinking)
        get_model_config,
        set_model_config,
        delete_model_config,
        // GitHub MCP configuration
        get_github_mcp_config,
        set_github_mcp_config,
        // Workers
        list_workers,
        get_worker_by_slug,
        create_worker,
        update_worker_status,
        update_worker_model,
        update_worker_repos,
        bind_worker_recipe,
        delete_worker,
        start_worker_recipe,
        stop_worker_recipe,
        // Workspace directory
        get_workspace_dir,
        set_workspace_dir,
        pick_workspace_dir,
        // File search (for @-mention picker)
        search_workspace_files,
        // Log file reading / management (for Logs tab)
        list_log_files,
        read_log_file,
        clear_log_files,
        // Sidecar lifecycle (core-orchestrator)
        get_sidecar_status,
        check_sidecar_connectivity,
        start_sidecar,
        stop_sidecar,
        // Skills system (Phase 3.1)
        list_skills,
        get_skill,
        search_skills,
        skills_by_category,
        execute_skill,
        validate_skill_parameters,
        reload_skills,
        import_skill,
        // Hooks system (Phase 3.1)
        list_hooks,
        enable_hook,
        disable_hook,
        add_hook,
        remove_hook,
        test_hook,
        get_hook_logs,
    ])
}
