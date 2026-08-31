// Auto-updater module — Phase 5.3.
// Orchestrates update checks, downloads, verification, and installation.
// Architecture:
//   check.rs      — queries GitHub Releases API, compares semver
//   download.rs   — streams installer with progress events
//   verify.rs     — SHA-256 + Ed25519 signature checks (both mandatory)
//   install.rs    — atomic OS-specific installation scheduling
//   persistence.rs — data integrity checks before/after update

pub mod check;
pub mod download;
pub mod install;
pub mod persistence;
pub mod verify;

pub use check::{UpdateInfo, UpdateChannel, PlatformAsset, check_for_update};
pub use download::{DownloadProgress, DownloadStage, DownloadState, download_update};
pub use install::{InstallResult, UpdateRecord, schedule_install, get_update_history, skip_version, is_skipped};
pub use persistence::{IntegrityReport, verify_data_integrity, create_restore_point, cleanup_after_update};
pub use verify::verify_update;

use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;

/// Shared updater state registered in Tauri's managed state.
#[derive(Default)]
pub struct UpdaterState {
    /// Most recent update check result.
    pub latest: Mutex<Option<UpdateInfo>>,
    /// Current download progress (None = not downloading).
    pub progress: Mutex<Option<DownloadProgress>>,
    /// Channel for sending progress updates to the command handler.
    pub progress_tx: Mutex<Option<tokio::sync::mpsc::Sender<DownloadProgress>>>,
}

impl UpdaterState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            latest:      Mutex::new(None),
            progress:    Mutex::new(None),
            progress_tx: Mutex::new(None),
        })
    }
}

/// Current app version, read from Cargo.toml at compile time.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Background update check — runs every 4 hours via a Tokio timer.
/// Sends a "updater:available" Tauri event if a newer version is found.
pub async fn start_background_check(
    state: Arc<UpdaterState>,
    app_handle: tauri::AppHandle,
    channel: UpdateChannel,
) {
    let interval = std::time::Duration::from_secs(4 * 3600);
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // First tick fires immediately
    loop {
        ticker.tick().await;

        match check_for_update(APP_VERSION, channel.clone()).await {
            Ok(Some(info)) if !is_skipped(&info.version) => {
                // Cache the update info
                *state.latest.lock().await = Some(info.clone());
                // Notify frontend
                let _ = app_handle.emit("updater:available", &info);
                if info.critical {
                    let _ = app_handle.emit("updater:critical", &info);
                }
            }
            Ok(_) => { /* No update or skipped */ }
            Err(e) => {
                // Log but don't crash the background loop
                eprintln!("[updater] Background check error: {e}");
            }
        }
    }
}
