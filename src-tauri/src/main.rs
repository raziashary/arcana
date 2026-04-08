#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use autodev_lib::session::manager::SessionManager;
use autodev_lib::state::AppState;
use autodev_lib::agent::permission::new_pending_permissions;

fn main() {
    autodev_lib::logging::init();

    let app_state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::new()));

    let session_mgr: Arc<SessionManager> = Arc::new(
        SessionManager::new().expect("Failed to initialise session database"),
    );

    // Shared pending permissions: call_id → oneshot sender.
    // Used by AgentEngine (tool executor) and respond_to_permission command.
    let pending_permissions = new_pending_permissions();

    // register_commands is defined in the lib crate so generate_handler! runs
    // where the #[tauri::command] __cmd__* macros are in scope.
    autodev_lib::register_commands(
        tauri::Builder::default()
            .plugin(tauri_plugin_shell::init())
            .manage(app_state)
            .manage(session_mgr)
            .manage(pending_permissions),
    )
    .run(tauri::generate_context!())
    .expect("Error running AutoDev");
}
