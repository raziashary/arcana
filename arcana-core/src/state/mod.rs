// Application-wide shared state.
// Arc<Mutex<AppState>> is managed by Tauri and accessible from all commands.

pub mod app_state;
pub use app_state::AppState;

pub mod lock;
pub mod runtime_registry;
