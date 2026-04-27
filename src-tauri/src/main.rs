#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use autodev_lib::session::manager::SessionManager;
use autodev_lib::state::AppState;
use autodev_lib::agent::permission::new_pending_permissions;
use tauri::Manager;
use autodev_lib::recipes::runner::RecipeRunner;
use autodev_lib::recipes::scheduler::RecipeScheduler;
use autodev_lib::sidecar::SidecarManager;
use autodev_lib::sidecar::client::OrchestratorClient;
use autodev_lib::sidecar::startup_progress::{StartupPhase, StartupProgress};
use autodev_lib::skills::SkillRegistry;
use autodev_lib::agent::ToolRegistry;
use autodev_lib::agent::builtin_tools;
use autodev_lib::mcp_server::server::start_mcp_server;

fn main() {
    // Capture panics and write them to both stderr and the tracing log file
    // before the process exits. This makes the "app restart" root cause visible.
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        eprintln!("[PANIC] {msg}");
        tracing::error!("[PANIC] {msg}");
    }));

    autodev_lib::logging::init();

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

    // Phase 3.4 — start AutoDev MCP Server in-process.
    // All canonical builtin_tools are registered and exposed over Streamable HTTP
    // on 127.0.0.1:8767 so core-orchestrator LivingAgents can discover them via MCP.
    let mut mcp_registry = ToolRegistry::new();
    builtin_tools::register_all(&mut mcp_registry);
    let mcp_registry = Arc::new(mcp_registry);

    let app_state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::new()));
    let skill_registry_managed = skill_registry.clone();

    let session_mgr: Arc<SessionManager> = Arc::new(
        SessionManager::new().expect("Failed to initialise session database"),
    );

    // Shared pending permissions: call_id → oneshot sender.
    // Used by AgentEngine (tool executor) and respond_to_permission command.
    let pending_permissions = new_pending_permissions();

    // Sidecar: core-orchestrator lifecycle manager + HTTP client.
    // Both are Arc-wrapped so they can be cloned across threads.
    let sidecar_mgr    = Arc::new(SidecarManager::new());
    let orch_client    = Arc::new(OrchestratorClient::new());

    // Phase 4.9 — Subagent registry. Shares the same OrchestratorClient so
    // it benefits from the cached JWT and connection pool. Also installed
    // globally so builtin tools (`spawn_subagent`, `spawn_parallel_subagents`)
    // can reach it without Tauri `State<>` plumbing.
    let subagent_mgr = autodev_lib::subagent::SubagentManager::new(orch_client.clone());
    autodev_lib::subagent::manager::install_global(subagent_mgr.clone());

    // Clone Arcs for the setup hook (setup runs after app handle is available)
    let app_state_for_setup    = app_state.clone();
    let session_mgr_for_setup  = session_mgr.clone();
    let pending_for_setup      = pending_permissions.clone();
    let orch_client_for_setup  = orch_client.clone();
    let sidecar_mgr_for_setup  = sidecar_mgr.clone();
    let subagent_mgr_for_setup = subagent_mgr.clone();

    // register_commands is defined in the lib crate so generate_handler! runs
    // where the #[tauri::command] __cmd__* macros are in scope.
    autodev_lib::register_commands(
        tauri::Builder::default()
            .plugin(tauri_plugin_shell::init())
            .manage(app_state)
            .manage(session_mgr)
            .manage(pending_permissions)
            .manage(sidecar_mgr)
            .manage(orch_client)
            .manage(subagent_mgr)
            .manage(skill_registry_managed)
            .manage(mcp_registry.clone())  // Phase 3.4b — exposed so list_mcp_tools can iterate
            .setup(move |app| {
                // Startup progress state machine — drives the loading overlay.
                // Attached to SidecarManager so spawn/health transitions + log
                // lines from both Python sidecars all feed the same FSM.
                // Phase 3.0 R1/R2 — install the process-wide AppHandle so
                // background subsystems (task-board tools, RecipeRunner,
                // hooks) can emit Tauri events without State plumbing.
                autodev_lib::events::install_app_handle(app.handle().clone());

                // Phase 3.4 — start AutoDev MCP Server as an async task.
                // The server exposes all builtin_tools over Streamable HTTP on 127.0.0.1:8767
                // so core-orchestrator LivingAgents can discover them via MCP.
                let mcp_registry_for_server = mcp_registry.clone();
                tauri::async_runtime::spawn(async move {
                    let _mcp_handle = start_mcp_server(mcp_registry_for_server);
                    // Keep the handle alive for the lifetime of the app
                    // (it will be dropped when the app shuts down)
                });

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
                    app.handle().clone(),
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
                    let mem_already_up = orch_probe.is_memory_healthy().await;
                    let orch_already_up = orch_probe.is_healthy().await;

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
                            if let Ok(orch_cwd) = autodev_lib::sidecar::resolve_sidecar_path(None) {
                                if let Err(e) = sidecar_ref.start(&orch_cwd) {
                                    eprintln!("[AutoDev] Failed to start core-orchestrator: {e}");
                                } else {
                                    // Poll orchestrator health: up to 30 × 1s attempts
                                    for attempt in 1..=30u32 {
                                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                                        if orch_probe.is_healthy().await {
                                            sidecar_ref.mark_running();
                                            orch_probe.clear_cache();
                                            eprintln!("[AutoDev] core-orchestrator ready after {attempt}s — Mode 2 active");
                                            // Phase 3.4 — register autodev-mcp-server so LivingAgents discover tools
                                            if let Err(e) = orch_probe.register_mcp_server().await {
                                                eprintln!("[AutoDev] Failed to register autodev-mcp-server: {e}");
                                            } else {
                                                eprintln!("[AutoDev] autodev-mcp-server registered with core-orchestrator");
                                            }
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
                Ok(())
            }),
    )
    .run(tauri::generate_context!())
    .expect("Error running AutoDev");
}
