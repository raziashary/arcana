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
pub mod ide;
pub mod git;
pub mod runtime_config;
pub mod mcp_server;
pub mod plan;
pub mod subagent;
pub mod analytics;
pub mod graph;

// Register all Tauri commands on the builder from within the lib crate.
// generate_handler! must run here so the __cmd__* macros (produced by
// #[tauri::command]) are in scope. main.rs calls this instead of using
// generate_handler! directly.
pub fn register_commands(
    builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    use commands::{
        agent_commands::{cancel_agent, respond_to_permission, send_message},
        session_commands::{create_session, delete_session, get_session, get_session_messages, list_sessions, set_session_pa_mode, update_session_title},
        settings_commands::{
            get_active_provider, get_cost_today, get_default_system_prompt, get_system_prompt,
            list_providers, set_active_provider, set_system_prompt, test_provider_connection,
        },
        vault_commands::{
            delete_github_pat, delete_provider_key, get_github_pat, get_provider_key,
            store_github_pat, store_provider_key,
        },
        task_commands::{
            list_tasks, get_task, update_task_status, delete_task, clear_all_tasks, get_qc_queue,
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
            update_worker_model, update_worker_model_sync, update_worker_repos, bind_worker_recipe,
            delete_worker, start_worker_recipe, stop_worker_recipe,
        },
        workspace_commands::{get_workspace_dir, set_workspace_dir, pick_workspace_dir},
        file_commands::search_workspace_files,
        log_commands::{list_log_files, read_log_file, read_log_file_full, clear_log_files},
        sidecar_commands::{
            get_sidecar_status, check_sidecar_connectivity, start_sidecar, stop_sidecar,
            get_full_sidecar_status, list_mcp_tools,
            // Phase 3.4b — per-service lifecycle + resource + log viewer
            start_memory_mcp, stop_memory_mcp,
            start_orchestrator, stop_orchestrator,
            get_resource_usage, read_sidecar_log,
        },
        skill_commands::{
            list_skills, get_skill, search_skills, skills_by_category,
            execute_skill, validate_skill_parameters, reload_skills, import_skill,
        },
        hook_commands::{
            list_hooks, enable_hook, disable_hook, add_hook, remove_hook,
            test_hook, get_hook_logs,
        },
        ide_commands::{
            read_file_tree, read_file_content, get_file_metadata,
            search_files, search_file_contents, get_symbol_tree,
            get_ide_workspace_state, set_ide_workspace_state,
        },
        git_commands::{
            git_open_repo, git_clone, git_init, git_status, git_diff,
            git_stage, git_unstage, git_discard, git_commit,
            git_push, git_pull, git_fetch,
            git_branch_list, git_branch_create, git_branch_delete, git_checkout,
            git_worktree_list, git_worktree_add, git_worktree_remove,
            git_log, git_blame,
            git_stash_list, git_stash_push, git_stash_apply, git_stash_drop,
            create_checkpoint, restore_checkpoint, list_checkpoints,
            delete_checkpoint, compare_checkpoints,
        },
        startup_commands::{get_startup_phase, get_runtime_config, set_skip_sidecars},
        plan_commands::{
            get_plan, list_plans, get_plan_tree, delete_plan, planner_compile,
            break_down_epic, claim_plan_item, create_subplan, submit_plan_for_qc,
        },
        subagent_commands::{
            spawn_subagent, execute_subtask, get_subagent_status,
            terminate_subagent, abort_subagent, list_active_subagents,
        },
        analytics_commands::{
            get_dashboard_summary, get_cost_analytics, get_token_usage,
            get_api_failures, predict_monthly_cost, export_costs_csv, export_costs_json,
            get_worker_analytics, get_system_logs,
        },
        graph_commands::{
            build_graph, update_graph, get_graph_stats, export_graph,
            get_graph_communities, get_graph_report, get_graph_nodes_for_viz,
            get_graph_edges_for_viz, get_god_nodes, get_surprising_connections,
            get_suggested_questions, register_repo, unregister_repo, list_repos,
            merge_all_repos_cmd, delete_graph, open_in_ide, get_graph_visualization_data,
        },
    };
    builder.invoke_handler(tauri::generate_handler![
        // Agent
        send_message,
        cancel_agent,
        respond_to_permission,
        // Plan Management
        planner_compile,
        get_plan,
        list_plans,
        get_plan_tree,
        delete_plan,
        // Fleet Plan Commands
        break_down_epic,
        claim_plan_item,
        create_subplan,
        submit_plan_for_qc,
        // Sessions
        list_sessions,
        get_session,
        get_session_messages,
        create_session,
        update_session_title,
        set_session_pa_mode,
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
        get_qc_queue,
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
        update_worker_model_sync,
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
        read_log_file_full,
        clear_log_files,
        // Sidecar lifecycle (core-orchestrator)
        get_sidecar_status,
        check_sidecar_connectivity,
        start_sidecar,
        stop_sidecar,
        get_full_sidecar_status,
        list_mcp_tools,
        // Phase 3.4b
        start_memory_mcp,
        stop_memory_mcp,
        start_orchestrator,
        stop_orchestrator,
        get_resource_usage,
        read_sidecar_log,
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
        // IDE (Phase 3.2)
        read_file_tree,
        read_file_content,
        get_file_metadata,
        search_files,
        search_file_contents,
        get_symbol_tree,
        get_ide_workspace_state,
        set_ide_workspace_state,
        // Git (Phase 3.3) — repo / status / diff
        git_open_repo,
        git_clone,
        git_init,
        git_status,
        git_diff,
        // Git — staging / commit
        git_stage,
        git_unstage,
        git_discard,
        git_commit,
        // Git — remote
        git_push,
        git_pull,
        git_fetch,
        // Git — branches
        git_branch_list,
        git_branch_create,
        git_branch_delete,
        git_checkout,
        // Git — worktrees
        git_worktree_list,
        git_worktree_add,
        git_worktree_remove,
        // Git — history / blame
        git_log,
        git_blame,
        // Git — stash
        git_stash_list,
        git_stash_push,
        git_stash_apply,
        git_stash_drop,
        // Git — checkpoints
        create_checkpoint,
        restore_checkpoint,
        list_checkpoints,
        delete_checkpoint,
        compare_checkpoints,
        // Startup overlay + Demo Mode (Phase 3.3.1)
        get_startup_phase,
        get_runtime_config,
        set_skip_sidecars,
        // Subagent System (Phase 4.9)
        spawn_subagent,
        execute_subtask,
        get_subagent_status,
        terminate_subagent,
        abort_subagent,
        list_active_subagents,
        // Analytics & Dashboard (Phase 5.4)
        get_dashboard_summary,
        get_cost_analytics,
        get_token_usage,
        get_api_failures,
        predict_monthly_cost,
        export_costs_csv,
        export_costs_json,
        get_worker_analytics,
        get_system_logs,
        // Graph Engine (Phase 6)
        build_graph,
        update_graph,
        get_graph_stats,
        export_graph,
        get_graph_communities,
        get_graph_report,
        get_graph_nodes_for_viz,
        get_graph_edges_for_viz,
        get_god_nodes,
        get_surprising_connections,
        get_suggested_questions,
        register_repo,
        unregister_repo,
        list_repos,
        merge_all_repos_cmd,
        delete_graph,
        open_in_ide,
        get_graph_visualization_data,
    ])
}
