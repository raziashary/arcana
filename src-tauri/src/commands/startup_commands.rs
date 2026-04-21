// startup_commands.rs — Tauri commands for the startup overlay + Demo Mode.
//
// These two concerns are colocated because they both pertain to app-launch
// behaviour and both hook into the StartupProgress FSM:
//
//   get_startup_phase  — frontend pull-on-mount fallback for events emitted
//                        before the React tree subscribed (Tauri does NOT
//                        replay missed events).
//   get_runtime_config — read on-disk demo flags.
//   set_skip_sidecars  — write the demo flag (requires app restart to apply).

use std::sync::Arc;

use crate::runtime_config::{self, RuntimeConfig};
use crate::sidecar::startup_progress::{StartupProgress, StartupProgressPayload};

// ── Startup phase snapshot ───────────────────────────────────────────────────

/// Return the current StartupProgress phase as a plain payload.
///
/// The `StartupOverlay` calls this on mount, immediately after registering
/// its `startup:progress` event listener, to recover any phase transitions
/// the listener missed (Tauri events emitted before listen() is registered
/// are dropped).
#[tauri::command]
pub fn get_startup_phase(
    progress: tauri::State<'_, Arc<StartupProgress>>,
) -> StartupProgressPayload {
    progress.snapshot()
}

// ── Runtime config (Demo Mode) ───────────────────────────────────────────────

/// Read the on-disk runtime config.
/// Used by Settings UI to render the current state of the toggles.
#[tauri::command]
pub fn get_runtime_config() -> RuntimeConfig {
    runtime_config::read()
}

/// Persist the `skip_sidecars` flag and return the new full config.
///
/// The change does NOT take effect until the next app launch — the sidecar
/// auto-start task only runs once during `setup()`. The Settings UI surfaces
/// this with an inline notice asking the user to restart.
#[tauri::command]
pub fn set_skip_sidecars(skip: bool) -> Result<RuntimeConfig, String> {
    let mut cfg = runtime_config::read();
    cfg.skip_sidecars = skip;
    runtime_config::write(&cfg).map_err(|e| e.to_string())?;
    Ok(cfg)
}
