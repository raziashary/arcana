#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use autodev_lib::agent::builtin_tools;
use autodev_lib::agent::permission::new_pending_permissions;
use autodev_lib::agent::ToolRegistry;
use autodev_lib::mcp::health::McpHealthMonitor;
use autodev_lib::recipes::runner::RecipeRunner;
use autodev_lib::recipes::scheduler::RecipeScheduler;
use autodev_lib::session::manager::SessionManager;
use autodev_lib::sidecar::client::OrchestratorClient;
use autodev_lib::sidecar::startup_progress::{StartupPhase, StartupProgress};
use autodev_lib::sidecar::{start_watchdog, SidecarManager};
use autodev_lib::skills::SkillRegistry;
use autodev_lib::state::AppState;
use autodev_lib::updater::{start_background_check, UpdateChannel, UpdaterState};
use std::sync::{Arc, Mutex};
use tauri::Manager;

fn main() {
    // ─── .env loading ────────────────────────────────────────────────────────
    // Load `.env` from the npm/project root so AUTODEV_SIDECAR_PATH and
    // AUTODEV_MEMORY_PATH (single source of truth for sidecar paths in dev)
    // are available to every subsystem before any path resolution runs.
    //
    // dotenvy walks up from CWD looking for `.env`. With `cargo tauri dev`
    // CWD is the project root, which is where `.env` lives.
    let dotenv_loaded = dotenvy::dotenv().ok();
    if let Some(ref p) = dotenv_loaded {
        eprintln!("[AutoDev] Loaded .env from: {}", p.display());

        // Canonicalize the sidecar path env vars so that downstream callers
        // never depend on the CWD at the moment of resolution. Relative paths
        // in `.env` (e.g. `./submodules/core-orchestrator`) are resolved
        // against `.env`'s parent directory — the project root.
        if let Some(env_dir) = p.parent() {
            for key in &["AUTODEV_SIDECAR_PATH", "AUTODEV_MEMORY_PATH"] {
                if let Ok(val) = std::env::var(key) {
                    let pb = std::path::PathBuf::from(&val);
                    let resolved = if pb.is_relative() {
                        env_dir.join(&pb)
                    } else {
                        pb
                    };
                    match resolved.canonicalize() {
                        Ok(abs) => {
                            // Strip Windows \\?\ verbatim prefix so paths render cleanly in logs.
                            let abs_str = abs
                                .to_string_lossy()
                                .trim_start_matches(r"\\?\")
                                .to_string();
                            std::env::set_var(key, &abs_str);
                            eprintln!("[AutoDev] {key}={abs_str}");
                        }
                        Err(e) => {
                            eprintln!("[AutoDev] WARN {key}='{val}' could not be resolved: {e}");
                        }
                    }
                }
            }
        }
    } else {
        eprintln!("[AutoDev] No .env file found — relying on system environment variables");
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
    let _log_guard = autodev_lib::logging::init();

    // Doc 08 Phase 3 — prune logs older than the configured retention cap
    // (default 14d, override via AUTODEV_LOG_RETENTION_DAYS env var).
    // Idempotent and best-effort; failures log a warning but never block
    // startup. Runs AFTER logging::init so the prune result lands in the
    // new daily file.
    if let Some(home) = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
    {
        let log_dir = std::path::PathBuf::from(home).join(".autodev").join("logs");
        autodev_lib::observability::log_retention::prune(&log_dir);
    }

    // Initialise recipes (writes default system recipes if absent)
    if let Err(e) = autodev_lib::recipes::init() {
        eprintln!("Warning: Failed to initialise recipes: {e}");
    }

    // Initialise skills (writes default builtin skills to ~/.autodev/skills/)
    if let Err(e) = autodev_lib::skills::ensure_default_skills() {
        eprintln!("Warning: Failed to initialise skills: {e}");
    }
    let skill_registry = Arc::new(SkillRegistry::new());
    if let Err(e) = skill_registry.reload() {
        eprintln!("Warning: Failed to load skills: {e}");
    }

    // Canonical builtin_tools registry.
    //
    // This used to be handed to an in-process MCP server on 127.0.0.1:8767 so
    // core-orchestrator LivingAgents could call AutoDev's tools. That server was
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
    autodev_lib::state::app_state::set_global(app_state.clone());

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
        let mut s = app_state.lock().unwrap();
        s.hydrate_today();
    }

    autodev_lib::commands::settings_commands::hydrate_active_provider(&app_state);
    let skill_registry_managed = skill_registry.clone();
    let skill_registry_for_watch = skill_registry.clone();

    let session_mgr: Arc<SessionManager> =
        Arc::new(SessionManager::new().expect("Failed to initialise session database"));

    // Shared pending permissions: call_id → oneshot sender.
    // Used by AgentEngine (tool executor) and respond_to_permission command.
    let pending_permissions = new_pending_permissions();

    // Sidecar: core-orchestrator lifecycle manager + HTTP client.
    // Both are Arc-wrapped so they can be cloned across threads.
    let sidecar_mgr = Arc::new(SidecarManager::new());
    let orch_client = Arc::new(OrchestratorClient::new());

    // Subagent registry. Subagents run in-process on a nested AgentEngine, so
    // the manager needs app state and the permission channel rather than an
    // HTTP client (ADR-LOG-RESTRUCT-0001). Installed globally so the spawn
    // builtin tools can reach it without Tauri `State<>` plumbing.
    let subagent_mgr = autodev_lib::subagent::SubagentManager::new(
        autodev_lib::subagent::native_runner::RunnerDeps {
            state: app_state.clone(),
            pending: pending_permissions.clone(),
        },
    );
    autodev_lib::subagent::manager::install_global(subagent_mgr.clone());

    // ─── Analytics singleton ─────────────────────────────────────────────────
    // The Dashboard, Mode-1 cost recorder, and every Mode-2 tick all read
    // and write through `analytics::global()`. Without this `install_global`
    // call, that accessor panics (intentionally — silent in-memory fallbacks
    // have already cost one audit cycle, see docs/endtoendrun/14may2026/02).
    //
    // `AnalyticsManager::new()` is async (opens & migrates ~/.autodev/analytics.db)
    // but we're still on the synchronous main thread before Tauri's runtime
    // starts, so block until the DB is ready. After install, kick the hourly +
    // daily rollup background thread.
    let analytics_mgr =
        tauri::async_runtime::block_on(autodev_lib::analytics::AnalyticsManager::new())
            .expect("Failed to initialise analytics DB at ~/.autodev/analytics.db");
    let analytics_mgr = Arc::new(analytics_mgr);
    autodev_lib::analytics::install_global(analytics_mgr.clone());
    analytics_mgr.start_aggregation_job();
    eprintln!("[AutoDev] analytics: installed global manager + started rollup job");

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
    let orch_client_for_setup = orch_client.clone();
    let sidecar_mgr_for_setup = sidecar_mgr.clone();
    let subagent_mgr_for_setup = subagent_mgr.clone();
    let updater_state_for_setup = updater_state.clone();
    let mcp_health_for_setup = mcp_health_monitor.clone();

    // register_commands is defined in the lib crate so generate_handler! runs
    // where the #[tauri::command] __cmd__* macros are in scope.
    let app = autodev_lib::register_commands(
        tauri::Builder::default()
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_dialog::init())
            .manage(app_state)
            .manage(session_mgr)
            .manage(pending_permissions)
            .manage(sidecar_mgr)
            .manage(orch_client)
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
                if !autodev_lib::events::install_app_handle(app.handle().clone()) {
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
                    let strays = autodev_lib::observability::stray_files::scan(None);
                    if !strays.is_empty() {
                        autodev_lib::events::emit_workspace_strays(
                            autodev_lib::events::WorkspaceStraysPayload { strays },
                        );
                    }
                }

                // Doc 07 Phase 4 — compute current_dir vs configured
                // workspace mismatch and emit a one-shot event. Frontend
                // banner subscribes and shows a "Set workspace" CTA.
                {
                    let configured = autodev_lib::commands::workspace_commands::load_workspace_dir();
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
                        autodev_lib::events::emit_workspace_mismatch(
                            autodev_lib::events::WorkspaceMismatchPayload {
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
                        if let Ok(gh_cfg) = autodev_lib::commands::github_mcp_commands::get_github_mcp_config() {
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

                let progress = Arc::new(StartupProgress::new(app.handle().clone()));
                progress.advance(StartupPhase::Init);
                // Skills were already loaded synchronously above — emit now that
                // the AppHandle is available.
                progress.advance(StartupPhase::Skills);
                sidecar_mgr_for_setup.set_progress(progress.clone());
                // Expose progress as Tauri state so `get_startup_phase` can
                // serve a snapshot on demand. This is the recovery path for
                // events emitted before the React listener registered (Tauri
                // does NOT replay missed events).
                app.manage(progress.clone());

                // Read the on-disk Demo Mode flag once. Used to decide
                // whether to spawn the Python sidecars below.
                let runtime_cfg = autodev_lib::runtime_config::read();
                if runtime_cfg.skip_sidecars {
                    eprintln!("[AutoDev] Demo Mode active — sidecars will NOT be started.");
                    eprintln!("[AutoDev] Chat + Worker fleet still work via the native AgentEngine.");
                    eprintln!("[AutoDev] To re-enable sidecars: Settings → Demo Mode → off, then restart.");
                }

                // Build RecipeRunner + RecipeScheduler now that we have AppHandle
                let runner = Arc::new(RecipeRunner::new(
                    app_state_for_setup,
                    session_mgr_for_setup,
                    pending_for_setup,
                    orch_client_for_setup.clone(),
                ));
                let scheduler = Arc::new(RecipeScheduler::new(runner));

                // Phase 3.0 R1 — expose the scheduler to the task-board tools
                // via a module-level OnceLock so `create_task`, `assign_task`,
                // and `update_task_status` can fire push-based kicks without
                // needing Tauri `State<>` access.
                autodev_lib::recipes::scheduler::install_global(scheduler.clone());

                // Phase 3.4B — start sidecar watchdog (auto-restart with exponential backoff).
                // The watchdog waits 90s before monitoring so the initial startup can complete.
                // Uses clones — the originals are consumed by the sidecar auto-start block below.
                start_watchdog(sidecar_mgr_for_setup.clone(), orch_client_for_setup.clone());

                // Auto-start the sidecar fleet on app launch.
                // Non-blocking — fires and forgets on a background task.
                // Skipped entirely in Demo Mode; in that case we fast-forward
                // the StartupProgress FSM straight to Ready so the loading
                // overlay dismisses cleanly.
                let orch_probe        = orch_client_for_setup;
                let sidecar_ref       = sidecar_mgr_for_setup;
                let skip_sidecars     = runtime_cfg.skip_sidecars;
                let progress_for_demo = progress.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

                    if skip_sidecars {
                        // Demo Mode: walk the FSM through every "would have"
                        // phase so the UI shows a quick scripted boot rather
                        // than jumping straight to Ready. Each step is < 200ms
                        // so total perceived boot time is ~1.4s.
                        for phase in [
                            StartupPhase::MemoryStarting,
                            StartupPhase::VectorDb,
                            StartupPhase::MemoryReady,
                            StartupPhase::OrchStarting,
                            StartupPhase::Routers,
                            StartupPhase::Agents,
                            StartupPhase::LivingAgents,
                            StartupPhase::OrchReady,
                            StartupPhase::Workers,
                            StartupPhase::FleetReady,
                            StartupPhase::Ready,
                        ] {
                            progress_for_demo.advance(phase);
                            tokio::time::sleep(std::time::Duration::from_millis(140)).await;
                        }
                        eprintln!("[AutoDev] Demo Mode boot complete (sidecars skipped).");
                        return;
                    }

                    // ── Phase 1: Check if sidecars are already running externally ──────────
                    // Use the STRICT health probe here so a previous-instance
                    // ghost socket or a grace-window optimistic check doesn't
                    // make us think a dead orchestrator is running — that
                    // caused workers to queue auth-login calls against a port
                    // that never accepted them, with no path to recovery.
                    let mem_already_up = orch_probe.is_memory_healthy().await;
                    let orch_already_up = orch_probe.is_healthy_strict().await;

                    if mem_already_up {
                        sidecar_ref.mark_memory_running();
                        eprintln!("[AutoDev] memory-mcp-server already running on :8000");
                    }
                    if orch_already_up {
                        sidecar_ref.mark_running();
                        eprintln!("[AutoDev] core-orchestrator already running on :4000 — Mode 2 active");
                    }

                    // ── Phase 2: If not already running, auto-start the fleet ──────────────
                    if !mem_already_up || !orch_already_up {
                        eprintln!("[AutoDev] Auto-starting sidecar fleet...");

                        // Start memory-mcp-server first
                        if !mem_already_up {
                            if let Ok(mem_cwd) = autodev_lib::sidecar::resolve_memory_path(None) {
                                if let Err(e) = sidecar_ref.start_memory(&mem_cwd) {
                                    eprintln!("[AutoDev] Failed to start memory-mcp-server: {e}");
                                } else {
                                    // Poll memory health: up to 20 × 1s attempts
                                    for attempt in 1..=20u32 {
                                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                        if orch_probe.is_memory_healthy().await {
                                            sidecar_ref.mark_memory_running();
                                            eprintln!("[AutoDev] memory-mcp-server ready after {attempt}s");
                                            break;
                                        }
                                    }
                                }
                            } else {
                                eprintln!("[AutoDev] Could not resolve memory-mcp-server path; set AUTODEV_MEMORY_PATH");
                            }
                        }

                        // Start core-orchestrator after memory is ready
                        if !orch_already_up {
                            if let Ok(orch_cwd) = autodev_lib::sidecar::resolve_orchestrator_path(None) {
                                if let Err(e) = sidecar_ref.start(&orch_cwd) {
                                    eprintln!("[AutoDev] Failed to start core-orchestrator: {e}");
                                } else {
                                    // Poll orchestrator health: up to 30 × 1s attempts
                                    for attempt in 1..=30u32 {
                                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                        if orch_probe.is_healthy().await {
                                            sidecar_ref.mark_running();
                                            orch_probe.clear_cache();
                                            eprintln!("[AutoDev] core-orchestrator ready after {attempt}s (Mode 2 is inert — see ADR-LOG-RESTRUCT-0008)");
                                            break;
                                        }
                                    }
                                }
                            } else {
                                eprintln!("[AutoDev] Could not resolve core-orchestrator path; set AUTODEV_SIDECAR_PATH");
                            }
                        }

                        // ── Phase 3: Fleet verification ─────────────────────────────────────────
                        // After both sidecars are healthy, wait for RecipeScheduler to start
                        // workers and then verify all 7 fleet agents are registered.
                        let progress_fleet = progress.clone();
                        let orch_client_fleet = orch_probe.clone();
                        let subagent_mgr_for_cleanup = subagent_mgr_for_setup.clone();
                        tauri::async_runtime::spawn(async move {
                            // Give RecipeScheduler time to start workers (+3s after orch ready)
                            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                            progress_fleet.advance(StartupPhase::Workers);

                            // Poll fleet verification for up to 60 seconds
                            let mut verified = false;
                            for attempt in 1..=60u32 {
                                let (ready, missing) = orch_client_fleet.verify_fleet_ready().await;
                                if ready {
                                    verified = true;
                                    eprintln!("[AutoDev] Fleet verification complete — all 7 agents registered after {attempt}s");
                                    break;
                                }
                                if attempt % 10 == 0 {
                                    eprintln!("[AutoDev] Fleet verification waiting — missing: {:?}", missing);
                                }
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            }

                            if !verified {
                                let (_, missing) = orch_client_fleet.verify_fleet_ready().await;
                                eprintln!("[AutoDev] Fleet verification timeout — proceeding with missing agents: {:?}", missing);
                            }

                            progress_fleet.advance(StartupPhase::FleetReady);

                            // Phase 4.9 — sweep ephemeral subagents that
                            // outlived a previous AutoDev process. Best-effort;
                            // failures are logged inside the manager.
                            subagent_mgr_for_cleanup.cleanup_orphans().await;

                            progress_fleet.advance(StartupPhase::Ready);
                        });
                    }
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
                    if let Ok(db) = autodev_lib::db::workers::WorkersDb::open() {
                        for worker in db.list().unwrap_or_default() {
                            let has_recipe = worker.recipe_id.is_some();
                            let is_active  = worker.status != "paused" && worker.status != "deleted";
                            if has_recipe && is_active {
                                if let Err(e) = scheduler_clone.start_worker(&worker.slug) {
                                    eprintln!("[AutoDev] Auto-start worker '{}' failed: {e}", worker.slug);
                                } else {
                                    eprintln!("[AutoDev] Auto-started worker '{}'", worker.slug);
                                }
                            }
                        }
                    }
                });

                app.manage(scheduler);

                // Phase 3.1 — start skills file watcher.
                // Watches ~/.autodev/skills/ for .md changes and auto-reloads the
                // SkillRegistry, emitting "skills:reloaded" with the new skill count.
                {
                    let skills_dir = autodev_lib::db::autodev_dir()
                        .unwrap_or_else(|_| std::path::PathBuf::from("."))
                        .join("skills");
                    autodev_lib::skills::start_skill_watcher(
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
    .expect("Error building AutoDev");

    // The run/audit ledger (P1-18). Opening it here rather than in `.setup()`
    // so the run is closed on the way out: `RunEvent::Exit` fires after setup's
    // scope has gone.
    //
    // `reap_abandoned` first — a crash leaves a run open forever, and an open
    // run reads as "still running" to anything querying the ledger.
    let run_id = match autodev_lib::db::runs::RunsDb::open() {
        Ok(runs) => {
            let _ = runs.reap_abandoned();
            match runs.begin(autodev_lib::db::runs::kind::APP, None) {
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
                if let Ok(runs) = autodev_lib::db::runs::RunsDb::open() {
                    if let Err(e) = runs.finish(id, autodev_lib::db::runs::outcome::OK) {
                        tracing::warn!("[runs] could not close run {id}: {e}");
                    }
                }
            }
        }
    });
}
