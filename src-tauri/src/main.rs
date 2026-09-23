#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use arcana_core::state::lock::LockTolerant;
use arcana_lib::agent::builtin_tools;
use arcana_lib::agent::permission::new_pending_permissions;
use arcana_lib::agent::ToolRegistry;
use arcana_lib::mcp::health::McpHealthMonitor;
use arcana_lib::recipes::runner::RecipeRunner;
use arcana_lib::recipes::scheduler::RecipeScheduler;
use arcana_lib::session::manager::SessionManager;
use arcana_lib::skills::SkillRegistry;
use arcana_lib::state::AppState;
use arcana_lib::updater::{start_background_check, UpdateChannel, UpdaterState};
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    // 🔴 `P8-64`. A `dotenvy::dotenv()` call stood here, in the SHIPPED binary.
    //
    // It existed to read two path variables that pointed at the deleted
    // subsystem — and **nothing reads either of them today**, verified across
    // the tree. But `dotenv()` loads the WHOLE file, and the repo `.env`
    // declares three provider keys and AWS credentials beside those two paths.
    // So a running application carried live secrets in its process
    // environment, inherited by every child the agent spawns, including
    // `bash.rs`, which runs commands the MODEL chooses. Reading a key needed
    // no vulnerability beyond `echo $ANTHROPIC_API_KEY`.
    //
    // ⚠ The scrub below is NOT redundant now — it is more defensible. It no
    // longer cleans up after this process's own load; it guards what the
    // user's shell exported before Arcana started, which is the case it was
    // always really for. Keys come from the OS credential store (BYOK), never
    // from the environment.

    // ─── Scrub credentials out of the process environment (F-8) ──────────────
    // This runs before anything can spawn a child process. BYOK reads keys
    // from the vault; the opt-in live smoke reads `.env` directly from disk.
    //
    // Names are logged, never values.
    {
        let removed = arcana_core::security::env_scrub::scrub_process_env();
        if !removed.is_empty() {
            eprintln!(
                "[Arcana] Removed {} credential-shaped variable(s) from the process \
                 environment: {}",
                removed.len(),
                removed.join(", ")
            );
        }
    }

    // Capture panics and write them to both stderr and the tracing log file
    // before the process exits. This makes the "app restart" root cause visible.
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        eprintln!("[PANIC] {msg}");
        tracing::error!("[PANIC] {msg}");
    }));

    // Phase 7.3 — hold the WorkerGuard alive for the entire process lifetime.
    // Dropping it early would close the background log-writer thread, causing
    // silent log loss from that point on.
    let _log_guard = arcana_lib::logging::init();

    // Doc 08 Phase 3 — prune logs older than the configured retention cap
    // (default 14d, override via ARCANA_LOG_RETENTION_DAYS env var).
    // Idempotent and best-effort; failures log a warning but never block
    // startup. Runs AFTER logging::init so the prune result lands in the
    // new daily file.
    if let Some(home) = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
    {
        let log_dir = std::path::PathBuf::from(home).join(".arcana").join("logs");
        arcana_lib::observability::log_retention::prune(&log_dir);
    }

    // Initialise recipes (writes default system recipes if absent)
    if let Err(e) = arcana_lib::recipes::init() {
        eprintln!("Warning: Failed to initialise recipes: {e}");
    }

    // Initialise skills (writes default builtin skills to ~/.arcana/skills/)
    if let Err(e) = arcana_lib::skills::ensure_default_skills() {
        eprintln!("Warning: Failed to initialise skills: {e}");
    }
    let skill_registry = Arc::new(SkillRegistry::new());
    if let Err(e) = skill_registry.reload() {
        eprintln!("Warning: Failed to load skills: {e}");
    }

    // Canonical builtin_tools registry.
    //
    // This used to be handed to an in-process MCP server on 127.0.0.1:8767 so
    // agents in a separate process could call Arcana's tools. That server was
    // an unauthenticated local endpoint exposing bash, write_file and
    // spawn_subagent to any process on the machine, and was deleted in P0
    // (ADR-03-02). The registry survives because `list_mcp_tools` reads it to
    // show the user which tools exist.
    let mut mcp_registry = ToolRegistry::new();
    builtin_tools::register_all(&mut mcp_registry);
    let mcp_registry = Arc::new(mcp_registry);

    let app_state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::new()));

    // Publish it for core-side callers that hold no state of their own.
    // `analyze_diff_tool` builds a provider straight from the vault, and a tool
    // receives a ToolContext rather than application state — without this it
    // would be the one path a provider call could take without meeting the
    // daily ceiling (P2-21). TG-P2-04 asserts this line exists.
    arcana_lib::state::app_state::set_global(app_state.clone());

    // P2-26. Restore the user's provider and model choice. Without this the
    // selection lived only in memory: they picked a model, used it, quit, and
    // came back to an empty picker with no indication why.
    //
    // A saved model that has since been retired is discarded rather than
    // restored — see `hydrate_active_provider`.
    // S2-06. Restore today's spend BEFORE anything can call a provider.
    //
    // These counters lived only in memory, so the daily ceiling saw zero on
    // every launch: a user who reached the cap could close the app, reopen it,
    // and carry on spending. A cap that resets on restart is a per-session cap
    // describing itself as daily.
    {
        let mut s = app_state.lock_tolerant();
        s.hydrate_today();
    }

    arcana_lib::commands::settings_commands::hydrate_active_provider(&app_state);
    let skill_registry_managed = skill_registry.clone();
    let skill_registry_for_watch = skill_registry.clone();

    let session_mgr: Arc<SessionManager> =
        Arc::new(SessionManager::new().expect("Failed to initialise session database"));

    // Shared pending permissions: call_id → oneshot sender.
    // Used by AgentEngine (tool executor) and respond_to_permission command.
    let pending_permissions = new_pending_permissions();


    // Subagent registry. Subagents run in-process on a nested AgentEngine, so
    // the manager needs app state and the permission channel rather than an
    // HTTP client (ADR-LOG-RESTRUCT-0001). Installed globally so the spawn
    // builtin tools can reach it without Tauri `State<>` plumbing.
    let subagent_mgr = arcana_lib::subagent::SubagentManager::new(
        arcana_lib::subagent::native_runner::RunnerDeps {
            state: app_state.clone(),
            pending: pending_permissions.clone(),
        },
    );
    arcana_lib::subagent::manager::install_global(subagent_mgr.clone());

    // ─── Analytics singleton ─────────────────────────────────────────────────
    // The Dashboard, the agent cost recorder and every worker tick all read
    // and write through `analytics::global()`. Without this `install_global`
    // call, that accessor panics (intentionally — silent in-memory fallbacks
    // have already cost one audit cycle, see docs/endtoendrun/14may2026/02).
    //
    // `AnalyticsManager::new()` is async (opens & migrates ~/.arcana/analytics.db)
    // but we're still on the synchronous main thread before Tauri's runtime
    // starts, so block until the DB is ready. After install, kick the hourly +
    // daily rollup background thread.
    let analytics_mgr =
        tauri::async_runtime::block_on(arcana_lib::analytics::AnalyticsManager::new())
            .expect("Failed to initialise analytics DB at ~/.arcana/analytics.db");
    let analytics_mgr = Arc::new(analytics_mgr);
    arcana_lib::analytics::install_global(analytics_mgr.clone());
    analytics_mgr.start_aggregation_job();
    eprintln!("[Arcana] analytics: installed global manager + started rollup job");

    // Phase 5.3 — auto-updater state
    let updater_state = UpdaterState::new();

    // Ruflo Feature 6 — MCP Connection Health Monitor.
    // Polls registered external MCP servers every 30 s and emits
    // mcp:health-update / mcp:connection-lost / mcp:reconnected events.
    let mcp_health_monitor = Arc::new(McpHealthMonitor::new(30, 3, 10));

    // Clone Arcs for the setup hook (setup runs after app handle is available)
    let app_state_for_setup = app_state.clone();
    let session_mgr_for_setup = session_mgr.clone();
    let pending_for_setup = pending_permissions.clone();
    let subagent_mgr_for_setup = subagent_mgr.clone();
    let updater_state_for_setup = updater_state.clone();
    let mcp_health_for_setup = mcp_health_monitor.clone();

    // register_commands is defined in the lib crate so generate_handler! runs
    // where the #[tauri::command] __cmd__* macros are in scope.
    let app = arcana_lib::register_commands(
        tauri::Builder::default()
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_dialog::init())
            .manage(app_state)
            .manage(session_mgr)
            .manage(pending_permissions)
            .manage(subagent_mgr)
            .manage(skill_registry_managed)
            .manage(mcp_registry.clone())   // Phase 3.4b — exposed so list_mcp_tools can iterate
            .manage(updater_state)          // Phase 5.3 — auto-updater state
            .manage(mcp_health_monitor)     // Ruflo Feature 6 — MCP Connection Health Monitor
            .setup(move |app| {
                // Startup progress state machine — drives the loading overlay.
                // Attached to SidecarManager so spawn/health transitions + log
                // lines from both Python sidecars all feed the same FSM.
                // Install the event sink before anything can emit. Core's
                // agent runtime, RecipeRunner, hooks and task-board tools all
                // emit through `EventSink`; until this runs they emit into a
                // null sink, which is correct for a headless host but would
                // silently drop the desktop's startup events.
                if !arcana_lib::events::install_app_handle(app.handle().clone()) {
                    // A sink was already installed. In a desktop process that
                    // means setup ran twice, and the second window's events
                    // would go to the first window's channel.
                    tracing::error!(
                        "[events] a sink was already installed before setup() —                          events may be delivered to a stale window"
                    );
                }

                // Doc 07 Phase 3 — scan src-tauri/ + src/ for stray
                // worker-generated files left behind by past runs. Emits
                // a `workspace:strays-detected` event so the SidecarCrashBanner
                // can render dismissible "delete" affordances per file.
                // Best-effort: scan failures never block startup.
                {
                    let strays = arcana_lib::observability::stray_files::scan(None);
                    if !strays.is_empty() {
                        arcana_lib::events::emit_workspace_strays(
                            arcana_lib::events::WorkspaceStraysPayload { strays },
                        );
                    }
                }

                // Doc 07 Phase 4 — compute current_dir vs configured
                // workspace mismatch and emit a one-shot event. Frontend
                // banner subscribes and shows a "Set workspace" CTA.
                {
                    let configured = arcana_lib::commands::workspace_commands::load_workspace_dir();
                    let current = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let normalised_eq = !configured.trim().is_empty()
                        && !current.trim().is_empty()
                        && std::path::Path::new(&current)
                            .canonicalize()
                            .map(|c| c == std::path::Path::new(&configured)
                                .canonicalize().unwrap_or_default())
                            .unwrap_or(false);
                    if !configured.trim().is_empty() && !normalised_eq {
                        arcana_lib::events::emit_workspace_mismatch(
                            arcana_lib::events::WorkspaceMismatchPayload {
                                current_dir: current,
                                configured_workspace: configured,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            }
                        );
                    }
                }


                // Ruflo Feature 6 — Start MCP Connection Health Monitor.
                // Registers the GitHub MCP server (if configured + enabled) and any other
                // user-configured external MCP servers, then starts the background ping loop.
                {
                    let monitor_ref  = mcp_health_for_setup.clone();
                    let app_for_mcp  = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        // Register GitHub MCP server if the user has it configured
                        if let Ok(gh_cfg) = arcana_lib::commands::github_mcp_commands::get_github_mcp_config() {
                            if gh_cfg.enabled {
                                if let Some(url) = &gh_cfg.server_url {
                                    monitor_ref.register_server("github-mcp", url).await;
                                }
                                // stdio variant — no URL but still registered for display
                                if gh_cfg.server_url.is_none() {
                                    monitor_ref.register_server("github-mcp", "stdio://github-mcp").await;
                                }
                            }
                        }
                        // Start background ping loop (runs forever)
                        monitor_ref.start_monitoring(app_for_mcp);
                    });
                }

                // P8-08/P8-52 — the `StartupProgress` FSM is gone with the
                // sidecar. Eight of its thirteen phases narrated the boot of a
                // Python process — memory store, vector DB, routers, agents —
                // so what remained could not describe a startup that no longer
                // happens.
                // The overlay goes with it — a spinner that never advances is
                // worse than no spinner.
                //
                // P8-53 — Demo Mode's flag is gone too: it existed to NOT spawn
                // the sidecars, and there are none.

                // Build RecipeRunner + RecipeScheduler now that we have AppHandle
                let runner = Arc::new(RecipeRunner::new(
                    app_state_for_setup,
                    session_mgr_for_setup,
                    pending_for_setup,
                ));
                let scheduler = Arc::new(RecipeScheduler::new(runner));

                // Phase 3.0 R1 — expose the scheduler to the task-board tools
                // via a module-level OnceLock so `create_task`, `assign_task`,
                // and `update_task_status` can fire push-based kicks without
                // needing Tauri `State<>` access.
                arcana_lib::recipes::scheduler::install_global(scheduler.clone());

                // P8-08 — the sidecar auto-start block is DELETED: the
                // watchdog, both spawn paths, the health polling, and the
                // thirteen-phase startup narration that went with them.
                //
                // 🔴 **One thing inside it was NOT sidecar work and is kept
                // below.** `subagent_mgr.cleanup_orphans()` sweeps ephemeral
                // subagents that outlived a previous Arcana process (Phase
                // 4.9). It sat inside the fleet-ready branch only because that
                // was the point in the boot where "everything is up" was known
                // — not because it had anything to do with Python. Deleting
                // the block wholesale would have taken it silently, and
                // nothing would have failed to compile.
                //
                // ⚠ That is the THIRD time in this phase that live code was
                // found nested inside something being deleted for an unrelated
                // reason (`P8-09`'s updater `.sig` client, `P8-53`'s worker
                // budgets, this). The tell each time was the same: the task
                // named a container, not a behaviour.
                let subagent_mgr_for_cleanup = subagent_mgr_for_setup.clone();
                tauri::async_runtime::spawn(async move {
                    // Runs on its own now. It used to wait for the fleet to be
                    // healthy; there is no fleet to wait for, and an orphan
                    // from a previous process is stale the moment we start.
                    subagent_mgr_for_cleanup.cleanup_orphans().await;
                });


                // Auto-start every worker that has a recipe bound and is not paused/deleted.
                // Deferred via async_runtime::spawn to ensure the Tokio runtime is active.
                // Note: the StartupPhase::Workers transition is NOT emitted from here
                // because the scheduler spawn fires at a fixed +500ms delay, while the
                // sidecar fleet takes ~15s to become healthy; advancing to Workers too
                // early would lock out the later MemoryReady/OrchReady transitions
                // (advance() is monotonic). Workers is emitted from the sidecar
                // auto-start path once both sidecars are healthy.
                let scheduler_clone = scheduler.clone();
                tauri::async_runtime::spawn(async move {
                    // Small delay to ensure the Tauri event loop is fully running
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if let Ok(db) = arcana_lib::db::workers::WorkersDb::open() {
                        for worker in db.list().unwrap_or_default() {
                            let has_recipe = worker.recipe_id.is_some();
                            let is_active  = worker.status != "paused" && worker.status != "deleted";
                            if has_recipe && is_active {
                                if let Err(e) = scheduler_clone.start_worker(&worker.slug) {
                                    eprintln!("[Arcana] Auto-start worker '{}' failed: {e}", worker.slug);
                                } else {
                                    eprintln!("[Arcana] Auto-started worker '{}'", worker.slug);
                                }
                            }
                        }
                    }
                });

                app.manage(scheduler);

                // Phase 3.1 — start skills file watcher.
                // Watches ~/.arcana/skills/ for .md changes and auto-reloads the
                // SkillRegistry, emitting "skills:reloaded" with the new skill count.
                {
                    let skills_dir = arcana_lib::db::arcana_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("skills");
                    arcana_lib::skills::start_skill_watcher(
                        skill_registry_for_watch,
                        skills_dir,
                        app.handle().clone(),
                    );
                }

                // Phase 5.3 — start background update checker (every 4 hours).
                // Runs on the Tokio async runtime; won't block the UI thread.
                let updater_state_bg = updater_state_for_setup.clone();
                let app_handle_bg    = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    start_background_check(updater_state_bg, app_handle_bg, UpdateChannel::Stable).await;
                });

                Ok(())
            }),
    )
    .build(tauri::generate_context!())
    .expect("Error building Arcana");

    // The run/audit ledger (P1-18). Opening it here rather than in `.setup()`
    // so the run is closed on the way out: `RunEvent::Exit` fires after setup's
    // scope has gone.
    //
    // `reap_abandoned` first — a crash leaves a run open forever, and an open
    // run reads as "still running" to anything querying the ledger.
    let run_id = match arcana_lib::db::runs::RunsDb::open() {
        Ok(runs) => {
            let _ = runs.reap_abandoned();
            match runs.begin(arcana_lib::db::runs::kind::APP, None) {
                Ok(id) => Some(id),
                Err(e) => {
                    tracing::warn!("[runs] could not record this launch: {e}");
                    None
                }
            }
        }
        Err(e) => {
            // The ledger is an audit trail, not a dependency. Failing to open
            // it must not stop the application starting.
            tracing::warn!("[runs] ledger unavailable: {e}");
            None
        }
    };

    app.run(move |_handle, event| {
        if let tauri::RunEvent::Exit = event {
            if let Some(id) = &run_id {
                if let Ok(runs) = arcana_lib::db::runs::RunsDb::open() {
                    if let Err(e) = runs.finish(id, arcana_lib::db::runs::outcome::OK) {
                        tracing::warn!("[runs] could not close run {id}: {e}");
                    }
                }
            }
        }
    });
}
