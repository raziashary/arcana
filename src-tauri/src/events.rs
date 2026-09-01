// The desktop app's event surface.
//
// Every payload type, event-name constant and `emit_*` function now lives in
// `autodev-core::events`, which routes through the `EventSink` trait instead of
// a `tauri::AppHandle`. This module re-exports them so the 255 commands and the
// frontend contract are unchanged, and supplies the one thing that genuinely
// belongs to the host: a sink that forwards to a Tauri window.
//
// See ADR-LOG-RESTRUCT-0011 and ADR-LOG-RESTRUCT-0015.

pub use autodev_core::events::*;

use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Delivers core's events to the frontend over Tauri's event channel.
struct TauriEventSink {
    app: AppHandle,
}

impl EventSink for TauriEventSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        // Emission failure means no window is listening. That is normal during
        // shutdown and before the frontend mounts, and it must not propagate
        // into the agent turn that produced the event.
        let _ = self.app.emit(name, payload);
    }
}

/// Install the Tauri-backed sink. Called once from `lib.rs`'s `.setup()`.
///
/// Returns whether this call installed it. `false` means a sink was already
/// present, which in a desktop process is a double-initialisation bug rather
/// than something to swallow — the caller logs it.
pub fn install_app_handle(app: AppHandle) -> bool {
    install_sink(Arc::new(TauriEventSink { app }))
}
