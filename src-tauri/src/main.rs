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
use autodev_lib::skills::SkillRegistry;

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

    // Clone Arcs for the setup hook (setup runs after app handle is available)
    let app_state_for_setup    = app_state.clone();
    let session_mgr_for_setup  = session_mgr.clone();
    let pending_for_setup      = pending_permissions.clone();
    let orch_client_for_setup  = orch_client.clone();
    let sidecar_mgr_for_setup  = sidecar_mgr.clone();

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
            .manage(skill_registry_managed)
            .setup(move |app| {
                // Build RecipeRunner + RecipeScheduler now that we have AppHandle
                let runner = Arc::new(RecipeRunner::new(
                    app.handle().clone(),
                    app_state_for_setup,
                    session_mgr_for_setup,
                    pending_for_setup,
                    orch_client_for_setup.clone(),
                ));
                let scheduler = Arc::new(RecipeScheduler::new(runner));

                // Auto-start the sidecar fleet on app launch.
                // Non-blocking — fires and forgets on a background task.
                let orch_probe  = orch_client_for_setup;
                let sidecar_ref = sidecar_mgr_for_setup;
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

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
                                            break;
                                        }
                                    }
                                }
                            } else {
                                eprintln!("[AutoDev] Could not resolve core-orchestrator path; set AUTODEV_SIDECAR_PATH");
                            }
                        }
                    }
                });

                // Auto-start every worker that has a recipe bound and is not paused/deleted
                // Deferred via async_runtime::spawn to ensure the Tokio runtime is active
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
