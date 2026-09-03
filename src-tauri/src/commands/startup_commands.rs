// startup_commands.rs — reading the on-disk runtime config.
//
// 🔴 **`P8-52`/`P8-53` emptied this file of everything else.**
//
// `get_startup_phase` returned the `StartupProgress` FSM's current phase so
// the overlay could recover transitions emitted before React subscribed. That
// FSM lived in `src-tauri/src/sidecar/startup_progress.rs` and **8 of its 13
// phases narrated Python boot** — polling `:8000`, waiting on the
// orchestrator. With the sidecar gone the overlay had nothing left to narrate,
// so the command, the FSM and the overlay go together (`P8-52`). Deleting the
// FSM alone would have left a spinner that never advances.
//
// `set_skip_sidecars` wrote Demo Mode's flag: "do not spawn the sidecars".
// There are none (`P8-53`).
//
// ⚠ `get_runtime_config` STAYS. `runtime_config.json` still holds the default
// worker budgets, which is why the file it reads was not deleted with the
// flag — see the header of `runtime_config.rs`.

use crate::runtime_config::{self, RuntimeConfig};

/// Read the on-disk runtime config.
///
/// Used by the Settings UI to render the current budget defaults.
#[tauri::command]
pub fn get_runtime_config() -> RuntimeConfig {
    runtime_config::read()
}
