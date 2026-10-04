//! Exact Arcana-owned telemetry labels. Unknown values retain secret redaction.

pub(super) fn is_registered(field: &str, value: &str) -> bool {
    let labels = match field {
        "event_type" => EVENT_TYPES,
        "operation" => OPERATIONS,
        _ => return false,
    };
    labels.binary_search(&value).is_ok()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;
    use syn::visit::{self, Visit};

    fn is_test(attributes: &[syn::Attribute]) -> bool {
        attributes.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && matches!(&attribute.meta, syn::Meta::List(list) if list.tokens.to_string().replace(' ', "") == "test")
        })
    }

    #[derive(Default)]
    struct Producers(BTreeSet<(&'static str, String)>);

    impl<'ast> Visit<'ast> for Producers {
        fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
            if !is_test(&item.attrs) {
                visit::visit_item_mod(self, item);
            }
        }

        fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
            if !is_test(&item.attrs) {
                visit::visit_item_fn(self, item);
            }
        }

        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(function) = call.func.as_ref() {
                let path: Vec<_> = function
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect();
                let timing_helper = path.last().is_some_and(|name| {
                    matches!(
                        name.as_str(),
                        "consent_timing"
                            | "timed_io"
                            | "observe_analytics_read"
                            | "observe_database_read"
                    )
                });
                if path.ends_with(&["OperationTimer".into(), "start".into()]) || timing_helper {
                    if let Some(syn::Expr::Lit(argument)) = call.args.first() {
                        if let syn::Lit::Str(label) = &argument.lit {
                            self.0.insert(("operation", label.value()));
                        }
                    }
                }
            }
            visit::visit_expr_call(self, call);
        }

        fn visit_local(&mut self, local: &'ast syn::Local) {
            // token_by_group selects a fixed operation name through a match.
            if let syn::Pat::Ident(binding) = &local.pat {
                if binding.ident == "operation_name" {
                    if let Some(initializer) = &local.init {
                        if let syn::Expr::Match(selection) = initializer.expr.as_ref() {
                            for arm in &selection.arms {
                                if let syn::Expr::Lit(value) = arm.body.as_ref() {
                                    if let syn::Lit::Str(label) = &value.lit {
                                        self.0.insert(("operation", label.value()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            visit::visit_local(self, local);
        }

        fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
            // Macro fields are token trees, rather than parsed Rust Exprs.
            // Only literal fields are catalogued; dynamic values stay redacted.
            if invocation
                .path
                .segments
                .first()
                .is_some_and(|segment| segment.ident == "tracing")
            {
                let tokens: Vec<_> = invocation.tokens.clone().into_iter().collect();
                for window in tokens.windows(3) {
                    if window[1].to_string() != "=" {
                        continue;
                    }
                    let field = match window[0].to_string().as_str() {
                        "event_type" => "event_type",
                        "operation" => "operation",
                        _ => continue,
                    };
                    if let Ok(label) = syn::parse_str::<syn::LitStr>(&window[2].to_string()) {
                        self.0.insert((field, label.value()));
                    }
                }
            }
        }
    }

    fn read_sources(directory: &Path, producers: &mut Producers) {
        for entry in std::fs::read_dir(directory).expect("read production source directory") {
            let path = entry.expect("source entry").path();
            if path.is_dir() {
                read_sources(&path, producers);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).expect("read Rust source");
                let file = syn::parse_file(&source)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                producers.visit_file(&file);
            }
        }
    }

    #[test]
    fn fixed_production_telemetry_labels_are_registered_and_sorted() {
        for labels in [super::EVENT_TYPES, super::OPERATIONS] {
            assert!(
                labels.windows(2).all(|pair| pair[0] < pair[1]),
                "binary-search catalogue must be sorted and unique"
            );
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut producers = Producers::default();
        for directory in [
            "arcana-core/src",
            "src-tauri/src",
            "src-cli/src",
            "src-shared/src",
        ] {
            read_sources(&root.join(directory), &mut producers);
        }
        assert!(
            producers.0.len() > 100,
            "source scan must find real producers"
        );
        let missing: Vec<_> = producers
            .0
            .iter()
            .filter(|(field, label)| !super::is_registered(field, label))
            .collect();
        assert!(
            missing.is_empty(),
            "fixed production labels are missing: {missing:?}"
        );
        assert!(!super::is_registered(
            "operation",
            "unknown_untrusted_operation_label_12345"
        ));
        assert!(!super::is_registered(
            "api_key",
            "scheduler_recoverable_work_check"
        ));
    }
}

const EVENT_TYPES: &[&str] = &[
    "build_evidence_persistence",
    "build_retry_approval_consumed",
    "daily_budget_pause_failed",
    "daily_budget_settlement_failed",
    "database_backup_created",
    "database_migration_backup_created",
    "database_statement",
    "fleet_cli_run_blocked",
    "fleet_cli_run_completed",
    "fleet_cli_run_started",
    "fleet_cli_run_timeout",
    "fleet_cli_supervisor_failed",
    "fleet_worker_tick",
    "graph_build_phase",
    "graph_build_result",
    "mcp_call",
    "mcp_connector_lifecycle",
    "mcp_health_check",
    "mcp_reconnect_attempt",
    "memory_procedural_result",
    "memory_recall_result",
    "memory_search_result",
    "memory_semantic_extract_result",
    "operation_journal_intent_failed",
    "operation_journal_outcome_failed",
    "operation_reconciliation_required",
    "operation_timing",
    "permission_unavailable",
    "project_command_denied",
    "project_command_result",
    "project_command_stage",
    "project_command_start",
    "provider_attempt",
    "provider_call",
    "provider_failure_persist_failed",
    "provider_retry",
    "python_ruff_install",
    "python_runtime_cache",
    "python_runtime_cache_race",
    "python_runtime_prepare",
    "run_token_reservation",
    "run_token_reservation_settled",
    "sandbox_profile_cleanup_failed",
    "task_board_backup_created",
    "tool_call_result",
    "tool_executor_failed",
    "trace_span",
    "worker_progress_persist_failed",
    "worker_startup_retry",
    "workspace_fingerprint",
];

const OPERATIONS: &[&str] = &[
    "agent_tool_batch",
    "agent_tool_call",
    "agent_turn",
    "checkpoint_before_write",
    "checkpoint_compare",
    "checkpoint_create",
    "checkpoint_delete",
    "checkpoint_list",
    "checkpoint_restore",
    "clear_board_backup",
    "cli_analytics_cost_summary_read",
    "cli_analytics_export_all",
    "cli_analytics_export_api_failures_read",
    "cli_analytics_export_cost_summary_read",
    "cli_analytics_export_hook_logs_read",
    "cli_analytics_export_system_events_read",
    "cli_analytics_export_token_usage_read",
    "cli_analytics_export_worker_stats_read",
    "cli_analytics_token_usage_read",
    "cli_analytics_worker_stats_read",
    "compaction",
    "config_instruction_delete",
    "config_instruction_exists",
    "config_instruction_length",
    "config_instruction_read",
    "config_instruction_write",
    "config_model_delete",
    "config_model_read",
    "config_model_write",
    "config_setting_delete",
    "config_setting_read",
    "config_setting_write",
    "daily_budget_provider_call_reserve",
    "daily_budget_provider_call_settle",
    "daily_cap_save",
    "database_analytics_retention_prune",
    "database_annotation_create",
    "database_annotation_delete",
    "database_annotation_list_for_file",
    "database_annotation_read",
    "database_annotation_resolve",
    "database_annotation_response_update",
    "database_annotation_status_update",
    "database_annotation_task_link",
    "database_api_failure_insert",
    "database_api_failures_recent_read",
    "database_backup",
    "database_build_attempt_batch_record",
    "database_build_evidence_read",
    "database_build_retry_approval_create",
    "database_build_retry_request_create",
    "database_connection_open",
    "database_daily_analytics_rollup",
    "database_daily_budget_summary_read",
    "database_daily_cost_insert",
    "database_daily_cost_range_read",
    "database_daily_cost_read",
    "database_epic_create",
    "database_epic_tasks_read",
    "database_hook_delete",
    "database_hook_enable_update",
    "database_hook_execution_log_insert",
    "database_hook_execution_logs_read",
    "database_hook_insert",
    "database_hook_logs_retained_export_read",
    "database_hook_logs_retention_prune",
    "database_hook_read",
    "database_hooks_by_type_read",
    "database_hooks_list",
    "database_migration_backup",
    "database_phase_ready_tasks_read",
    "database_phase_summary_read",
    "database_phased_task_create",
    "database_plan_create",
    "database_plan_delete",
    "database_plan_dependency_add",
    "database_plan_dependency_check",
    "database_plan_exists",
    "database_plan_item_claim",
    "database_plan_item_create",
    "database_plan_item_list",
    "database_plan_item_read",
    "database_plan_item_status_update",
    "database_plan_list",
    "database_plan_read",
    "database_plan_status_update",
    "database_project_build_batch_begin",
    "database_project_build_batch_finish",
    "database_qc_claim_release",
    "database_qc_claim_try",
    "database_qc_claim_upgrade",
    "database_qc_claims_list",
    "database_qc_cost_add",
    "database_routing_affinity_stats_read",
    "database_routing_history_claim_outcome_update",
    "database_routing_history_insert",
    "database_routing_history_outcome_update",
    "database_routing_history_read",
    "database_routing_success_rates_read",
    "database_run_begin",
    "database_run_finish",
    "database_run_read",
    "database_runs_open_read",
    "database_runs_reap_abandoned",
    "database_runs_recent_read",
    "database_schema_migration_ladder",
    "database_session_delete",
    "database_session_list",
    "database_session_message_insert",
    "database_session_message_update",
    "database_session_messages_read",
    "database_session_pa_mode_update",
    "database_session_read_with_messages",
    "database_session_schema_migrate",
    "database_session_schema_version_read",
    "database_session_store_open",
    "database_session_upsert",
    "database_system_event_insert",
    "database_system_events_recent_read",
    "database_task_assign_with_routing",
    "database_task_budget_exceeded",
    "database_task_budget_read",
    "database_task_budget_set",
    "database_task_budget_usage_update",
    "database_task_claim_assigned",
    "database_task_claim_next_ready",
    "database_task_create",
    "database_task_delete",
    "database_task_dependency_reconcile",
    "database_task_diff_analysis_write",
    "database_task_list",
    "database_task_messages_read",
    "database_task_orphan_candidates_read",
    "database_task_orphan_release",
    "database_task_read",
    "database_task_recovery_quarantine",
    "database_task_retry_count_read",
    "database_task_retry_increment",
    "database_task_status_transition",
    "database_task_unresolved_side_effect_read",
    "database_tasks_clear_all",
    "database_today_activity_read",
    "database_token_usage_by_agent_read",
    "database_token_usage_by_model_read",
    "database_token_usage_by_provider_read",
    "database_token_usage_by_unsupported_group_read",
    "database_token_usage_date_read",
    "database_token_usage_insert",
    "database_token_usage_range_read",
    "database_token_usage_session_read",
    "database_token_usage_top_sessions_read",
    "database_token_usage_totals_read",
    "database_worker_active_operation_read",
    "database_worker_claimed_task_read",
    "database_worker_create",
    "database_worker_deactivate",
    "database_worker_delete",
    "database_worker_heartbeat",
    "database_worker_inbox_read",
    "database_worker_message_acknowledge",
    "database_worker_message_insert",
    "database_worker_message_read",
    "database_worker_messages_recent_read",
    "database_worker_model_update",
    "database_worker_progress_recent_read",
    "database_worker_progress_record",
    "database_worker_read_by_id",
    "database_worker_read_by_slug",
    "database_worker_recipe_bind",
    "database_worker_repos_update",
    "database_worker_request_create",
    "database_worker_request_read",
    "database_worker_request_update",
    "database_worker_requests_list",
    "database_worker_roster_read",
    "database_worker_run_owner_latest_read",
    "database_worker_slug_exists",
    "database_worker_stale_messages_read",
    "database_worker_stats_insert",
    "database_worker_stats_point_read",
    "database_worker_stats_range_read",
    "database_worker_stats_rollup",
    "database_worker_status_update",
    "database_worker_task_list",
    "database_worker_tick_begin",
    "database_worker_tick_finish",
    "database_worker_tick_heartbeat",
    "database_worker_tick_latest_read",
    "database_worker_tick_phase_update",
    "database_worker_ticks_interrupt",
    "database_worker_ticks_latest_read",
    "database_worker_unacknowledged_count",
    "database_worker_unread_count",
    "force_reconnect_requested",
    "github_mcp_config_read",
    "github_mcp_config_write",
    "graph_build",
    "graph_communities_batch_update",
    "graph_community_update",
    "graph_counts_read",
    "graph_edge_upsert",
    "graph_edges_batch_upsert",
    "graph_edges_page_read",
    "graph_edges_upsert",
    "graph_file_edges_delete",
    "graph_file_hash_read",
    "graph_file_nodes_delete",
    "graph_file_nodes_read",
    "graph_flush",
    "graph_fts_rebuild",
    "graph_node_edges_read",
    "graph_node_read",
    "graph_node_search",
    "graph_node_upsert",
    "graph_nodes_batch_upsert",
    "graph_nodes_page_read",
    "graph_nodes_upsert",
    "graph_schema_version_read",
    "graph_snapshot_insert",
    "graph_snapshot_read",
    "graph_stats_read",
    "graph_store_open",
    "graph_wal_flush",
    "ide_workspace_state_load",
    "ide_workspace_state_save",
    "memory_access_record",
    "memory_database_initialize",
    "memory_database_migrate",
    "memory_embedding",
    "memory_episodic_count_sessions",
    "memory_episodic_count_total",
    "memory_episodic_summaries_read",
    "memory_episodic_summary",
    "memory_episodic_timeline_read",
    "memory_episodic_turn_count",
    "memory_episodic_write",
    "memory_get_stats",
    "memory_procedural_count_total",
    "memory_procedural_index_read",
    "memory_procedural_index_refresh",
    "memory_procedural_index_write",
    "memory_procedural_recall",
    "memory_procedural_search",
    "memory_procedural_topic_delete",
    "memory_procedural_topic_read",
    "memory_procedural_topics_list",
    "memory_procedural_write",
    "memory_proposal",
    "memory_recall",
    "memory_search_all",
    "memory_semantic_count_total",
    "memory_semantic_deactivate",
    "memory_semantic_extract",
    "memory_semantic_preferences_read",
    "memory_semantic_recall",
    "memory_semantic_search",
    "memory_semantic_update",
    "memory_semantic_write",
    "memory_tool_get",
    "memory_tool_list",
    "observability_bundle_export",
    "orphan_candidate_scan",
    "orphan_reconciliation",
    "provider_generation",
    "provider_input_token_preflight",
    "provider_stream",
    "python_runtime.copy_dlls",
    "python_runtime.copy_interpreter_files",
    "python_runtime.copy_pytest_closure",
    "python_runtime.copy_stdlib",
    "python_runtime.fingerprint_source",
    "python_runtime.fingerprint_staged_runtime",
    "python_runtime.inspect_interpreter",
    "python_runtime.probe_cached_runtime",
    "python_runtime.probe_staged_runtime",
    "python_runtime.validate_interpreter",
    "python_runtime.verify_cached_runtime",
    "python_runtime.verify_concurrent_runtime",
    "registered",
    "scheduler_recoverable_work_check",
    "scheduler_worker_dispatch_gate",
    "unregistered",
    "workers_database_initialize",
    "workspace_egress_authorize_audit",
    "workspace_egress_consent_grant",
    "workspace_egress_consent_read",
    "workspace_egress_consent_revoke",
];
