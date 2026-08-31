// AutoDev Rust backend library root.
// Module declarations; main.rs and tauri setup reference this.

pub mod errors;
pub mod logging;
// Doc 08 Phase 2/3 — error_kinds taxonomy + log_retention policy.
pub mod agent;
pub mod commands;
pub mod cost;
pub mod db;
pub mod events;
pub mod git;
pub mod hooks;
pub mod ide;
pub mod mcp;
pub mod observability;
pub mod platform;
pub mod provider;
pub mod recipes;
pub mod runtime_config;
pub mod session;
pub mod sidecar;
pub mod skills;
pub mod state;
pub mod vault;

// Compiled only under test: asserts every .rs file under src/ is reachable from
// the crate root. 918 lines shipped as "COMPLETE" while never being compiled.
pub mod analytics;
pub mod budget;
pub mod diff_analysis;
pub mod graph;
pub mod memory;
pub mod orphan;
pub mod plan;
pub mod plugin;
#[cfg(test)]
mod reachability_gate;

// Phase-P0 spec-invariant gates over repository files (TG0-02/03/04).
#[cfg(test)]
mod ci_gate;
pub mod routing;
pub mod subagent;
pub mod updater;

// Register all Tauri commands on the builder from within the lib crate.
// generate_handler! must run here so the __cmd__* macros (produced by
// #[tauri::command]) are in scope. main.rs calls this instead of using
// generate_handler! directly.
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    use commands::{
        agent_commands::{cancel_agent, respond_to_permission, send_message},
        analytics_commands::{
            export_costs_csv, export_costs_json, get_api_failures, get_cost_analytics,
            get_dashboard_summary, get_system_logs, get_token_usage, get_worker_analytics,
            predict_monthly_cost,
        },
        annotation_commands::{
            create_annotation, delete_annotation, list_annotations_for_file, resolve_annotation,
            update_annotation_response, update_annotation_task_id,
        },
        autodev_instructions_commands::{
            check_autodev_instructions_exists, delete_autodev_instructions,
            get_autodev_instructions, get_autodev_instructions_char_count,
            set_autodev_instructions,
        },
        browser_commands::{check_playwright_available, install_playwright_browsers},
        budget_commands::{
            cmd_set_task_budget, get_daily_budget_summary, get_task_budget_status,
            set_default_worker_budget,
        },
        diff_commands::{analyze_diff, analyze_pr_diff},
        file_commands::search_workspace_files,
        git_commands::{
            compare_checkpoints, create_checkpoint, delete_checkpoint, git_blame,
            git_branch_create, git_branch_delete, git_branch_list, git_checkout, git_clone,
            git_commit, git_diff, git_discard, git_fetch, git_init, git_log, git_open_repo,
            git_pull, git_push, git_stage, git_stash_apply, git_stash_drop, git_stash_list,
            git_stash_push, git_status, git_unstage, git_worktree_add, git_worktree_list,
            git_worktree_remove, list_checkpoints, restore_checkpoint,
        },
        github_mcp_commands::{get_github_mcp_config, set_github_mcp_config},
        graph_commands::{
            build_graph, delete_graph, export_graph, get_god_nodes, get_graph_communities,
            get_graph_edges_for_viz, get_graph_nodes_for_viz, get_graph_report, get_graph_stats,
            get_graph_visualization_data, get_suggested_questions, get_surprising_connections,
            list_repos, merge_all_repos_cmd, open_in_ide, register_repo, search_graph_nodes,
            unregister_repo, update_graph,
        },
        hook_commands::{
            add_hook, disable_hook, enable_hook, get_hook_logs, list_hooks, remove_hook, test_hook,
        },
        ide_commands::{
            create_directory, create_file, delete_file, find_symbol_references, get_file_metadata,
            get_ide_workspace_state, get_symbol_tree, move_file, read_file_content, read_file_tree,
            rename_file, resolve_symbol, search_file_contents, search_files,
            set_ide_workspace_state,
        },
        log_commands::{
            clear_log_files, export_log_file, list_log_files, read_log_file, read_log_file_full,
        },
        memory_commands::{
            deactivate_fact, delete_memory_topic, get_memory_md, get_memory_stats,
            get_memory_topic, get_session_timeline, get_user_preferences, list_memory_topics,
            recall_facts, save_memory_md, save_memory_topic, search_memory, search_procedural,
            search_semantic, store_fact_cmd, summarize_session_cmd, update_fact_cmd,
        },
        model_config_commands::{delete_model_config, get_model_config, set_model_config},
        orphan_commands::{force_release_task, get_orphaned_tasks},
        plan_commands::{
            break_down_epic, claim_plan_item, create_subplan, delete_plan, get_plan, get_plan_tree,
            list_plans, planner_compile, submit_plan_for_qc,
        },
        plugin_commands::{
            generate_setup_token, get_plugin, get_plugin_config, install_plugin,
            install_plugin_from_yaml, list_installed_plugins, list_plugins, run_plugin,
            set_plugin_config, uninstall_plugin, update_plugin, validate_setup_token,
            verify_plugin,
        },
        recipe_commands::{
            create_recipe, delete_recipe, generate_recipe_yaml, get_recipe, list_recipe_backups,
            list_recipes, restore_recipe_backup, update_recipe,
        },
        routing_commands::{get_worker_affinity_stats, suggest_worker_for_task},
        session_commands::{
            create_session, delete_session, get_session, get_session_messages, list_sessions,
            set_session_pa_mode, update_session_title,
        },
        settings_commands::{
            get_active_provider, get_cost_today, get_default_system_prompt, get_system_prompt,
            list_providers, set_active_provider, set_system_prompt, test_provider_connection,
        },
        sidecar_commands::{
            check_sidecar_connectivity,
            force_reconnect_mcp,
            get_full_sidecar_status,
            // Ruflo Feature 6 — MCP Connection Health Monitor
            get_mcp_health,
            get_resource_usage,
            get_sidecar_status,
            get_sidecar_stderr_tail,
            // Phase 5.3 — installer bundling
            get_submodule_paths,
            list_mcp_tools,
            read_sidecar_log,
            restart_sidecar,
            // Phase 3.4b — per-service lifecycle + resource + log viewer
            start_memory_mcp,
            start_orchestrator,
            start_sidecar,
            stop_memory_mcp,
            stop_orchestrator,
            stop_sidecar,
        },
        skill_commands::{
            execute_skill, get_skill, import_skill, list_skills, reload_skills, search_skills,
            skills_by_category, validate_skill_parameters,
        },
        startup_commands::{get_runtime_config, get_startup_phase, set_skip_sidecars},
        subagent_commands::{
            abort_subagent, execute_subtask, get_qc_reviewer_stats, get_subagent_status,
            list_active_subagents, spawn_subagent, terminate_subagent,
        },
        task_commands::{
            clear_all_tasks,
            create_phased_task,
            delete_task,
            get_epic_phase_summary,
            // Ruflo Feature 5 — Phase-Based Task Decomposition
            get_epic_tasks,
            get_qc_queue,
            get_task,
            list_tasks,
            list_worker_requests,
            resolve_worker_request,
            update_task_status,
        },
        updater_commands::{
            check_for_update, download_update, get_cached_update, get_download_progress,
            get_update_history, install_update, skip_version, verify_data_integrity,
        },
        vault_commands::{
            delete_github_pat, delete_provider_key, has_github_pat, has_provider_key,
            list_vault_entries, set_github_pat, set_provider_key,
        },
        worker_commands::{
            bind_worker_recipe, create_worker, delete_worker, get_worker_by_slug, list_workers,
            start_worker_recipe, stop_worker_recipe, update_worker_model, update_worker_model_sync,
            update_worker_repos, update_worker_status,
        },
        worker_message_commands::{
            count_unread_worker_messages_for, list_recent_worker_messages,
            list_worker_messages_for_task,
        },
        workspace_commands::{get_workspace_dir, pick_workspace_dir, set_workspace_dir},
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
        set_provider_key,
        has_provider_key,
        delete_provider_key,
        set_github_pat,
        has_github_pat,
        delete_github_pat,
        list_vault_entries,
        // Tasks (Internal Board)
        list_tasks,
        get_task,
        update_task_status,
        delete_task,
        clear_all_tasks,
        get_qc_queue,
        // Worker Requests (Fleet→Supervisor communication)
        list_worker_requests,
        resolve_worker_request,
        // Ruflo Feature 5 — Epic & Phase commands
        get_epic_tasks,
        get_epic_phase_summary,
        create_phased_task,
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
        export_log_file,
        clear_log_files,
        // Sidecar lifecycle (core-orchestrator)
        get_sidecar_status,
        check_sidecar_connectivity,
        start_sidecar,
        stop_sidecar,
        get_full_sidecar_status,
        get_sidecar_stderr_tail,
        list_mcp_tools,
        // Phase 3.4b
        start_memory_mcp,
        stop_memory_mcp,
        start_orchestrator,
        stop_orchestrator,
        restart_sidecar,
        get_resource_usage,
        read_sidecar_log,
        // Phase 5.3 — installer bundling
        get_submodule_paths,
        // Ruflo Feature 6 — MCP Connection Health Monitor
        get_mcp_health,
        force_reconnect_mcp,
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
        resolve_symbol,
        find_symbol_references,
        create_file,
        create_directory,
        delete_file,
        rename_file,
        move_file,
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
        get_qc_reviewer_stats, // Doc 04 Phase 3.3
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
        search_graph_nodes,
        register_repo,
        unregister_repo,
        list_repos,
        merge_all_repos_cmd,
        delete_graph,
        open_in_ide,
        get_graph_visualization_data,
        // Auto-updater (Phase 5.3)
        check_for_update,
        get_cached_update,
        download_update,
        get_download_progress,
        install_update,
        skip_version,
        get_update_history,
        verify_data_integrity,
        // Memory System (Phase 4.1-4.7)
        get_session_timeline,
        recall_facts,
        store_fact_cmd,
        update_fact_cmd,
        deactivate_fact,
        get_user_preferences,
        search_semantic,
        get_memory_md,
        save_memory_md,
        list_memory_topics,
        get_memory_topic,
        save_memory_topic,
        delete_memory_topic,
        search_procedural,
        search_memory,
        get_memory_stats,
        summarize_session_cmd,
        // Plugin system (Phase 5.2)
        list_plugins,
        list_installed_plugins,
        get_plugin,
        install_plugin,
        install_plugin_from_yaml,
        uninstall_plugin,
        update_plugin,
        verify_plugin,
        run_plugin,
        get_plugin_config,
        set_plugin_config,
        generate_setup_token,
        validate_setup_token,
        // Routing (Ruflo Feature 1 — Semantic Task Routing)
        suggest_worker_for_task,
        get_worker_affinity_stats,
        // Orphan Detection (Ruflo Feature 2 — PID-Based Orphan Detection)
        get_orphaned_tasks,
        force_release_task,
        // Diff Risk Scoring (Ruflo Feature 3)
        analyze_diff,
        analyze_pr_diff,
        // Budget Circuit Breaker (Ruflo Feature 4)
        cmd_set_task_budget,
        get_task_budget_status,
        get_daily_budget_summary,
        set_default_worker_budget,
        // Browser Automation QC Skill (Ruflo Feature 7)
        check_playwright_available,
        install_playwright_browsers,
        // Inline Annotations (IDE Phase 11)
        create_annotation,
        list_annotations_for_file,
        resolve_annotation,
        update_annotation_response,
        update_annotation_task_id,
        delete_annotation,
        // AutoDev Communication Protocol — Fleet Chat
        list_worker_messages_for_task,
        list_recent_worker_messages,
        count_unread_worker_messages_for,
    ])
}
