// Tauri IPC commands for the auto-updater system.
// Frontend calls these to check, download, and install updates.

use std::sync::Arc;
use tauri::{State, Emitter};

use crate::errors::AutoDevError;
use crate::updater::{
    self,
    check::{UpdateChannel, UpdateInfo},
    download::{DownloadProgress, DownloadStage},
    install::{InstallResult, UpdateRecord},
    persistence::IntegrityReport,
    UpdaterState,
};

// ─── Check ────────────────────────────────────────────────────────────────────

/// Check for an available update. Returns None if current version is latest.
#[tauri::command]
pub async fn check_for_update(
    state: State<'_, Arc<UpdaterState>>,
    channel: Option<String>,
) -> Result<Option<UpdateInfo>, AutoDevError> {
    let ch = match channel.as_deref() {
        Some("beta")    => UpdateChannel::Beta,
        Some("nightly") => UpdateChannel::Nightly,
        _               => UpdateChannel::Stable,
    };

    match updater::check_for_update(updater::APP_VERSION, ch).await {
        Ok(Some(info)) if !updater::is_skipped(&info.version) => {
            *state.latest.lock().await = Some(info.clone());
            Ok(Some(info))
        }
        Ok(_) => Ok(None),
        Err(e) => Err(AutoDevError::from(e)),
    }
}

/// Get the last cached update info (from background check), without network call.
#[tauri::command]
pub async fn get_cached_update(
    state: State<'_, Arc<UpdaterState>>,
) -> Result<Option<UpdateInfo>, AutoDevError> {
    Ok(state.latest.lock().await.clone())
}

// ─── Download ─────────────────────────────────────────────────────────────────

/// Start downloading an update in the background.
/// Progress events are sent as Tauri events: "updater:progress".
#[tauri::command]
pub async fn download_update(
    version:   String,
    app_handle: tauri::AppHandle,
    state:      State<'_, Arc<UpdaterState>>,
) -> Result<DownloadProgress, AutoDevError> {
    // Get cached update info
    let info = state.latest.lock().await.clone()
        .ok_or_else(|| AutoDevError::Agent("No update info cached — call check_for_update first".into()))?;

    if info.version != version {
        return Err(AutoDevError::Agent(format!(
            "Version mismatch: cached={}, requested={version}", info.version
        )));
    }

    let asset = info.asset_for_current_platform()
        .ok_or_else(|| AutoDevError::Agent("No update available for this platform".into()))?
        .clone();

    // Create progress channel
    let (tx, mut rx) = tokio::sync::mpsc::channel::<DownloadProgress>(32);
    *state.progress_tx.lock().await = Some(tx.clone());

    // Initial progress
    let initial = DownloadProgress {
        version:          version.clone(),
        bytes_downloaded: 0,
        total_bytes:      asset.full_size,
        percent:          0.0,
        speed_bps:        0,
        stage:            DownloadStage::Downloading,
        path:             None,
    };
    *state.progress.lock().await = Some(initial.clone());

    let state_clone  = Arc::clone(&state);
    let app_handle2  = app_handle.clone();
    let version2     = version.clone();
    let checksum_url = asset.checksum.clone();
    let sig_url      = asset.signature.clone();

    // Spawn download in background
    tokio::spawn(async move {
        // Forward progress events to frontend
        let state_fwd = Arc::clone(&state_clone);
        let ah_fwd    = app_handle2.clone();
        tokio::spawn(async move {
            while let Some(progress) = rx.recv().await {
                *state_fwd.progress.lock().await = Some(progress.clone());
                let _ = ah_fwd.emit("updater:progress", &progress);
            }
        });

        match updater::download_update(&version2, &asset, tx).await {
            Ok(path) => {
                // Fetch the published checksum, then verify.
                //
                // A failed fetch is a FAILED VERIFICATION, not an absent one.
                // This used to be `.unwrap_or_default()`, which turned any
                // network error into an empty string — and an empty checksum
                // was then accepted by verify_checksum as "nothing to compare
                // against". A transient DNS failure was sufficient to install
                // an unverified binary.
                let verify_result = match updater::download::download_checksum(&checksum_url).await {
                    Ok(expected) => updater::verify_update(&path, &expected, &sig_url).await,
                    Err(e)       => Err(e.context("Could not fetch the published checksum")),
                };

                let ready_progress = DownloadProgress {
                    version:          version2.clone(),
                    bytes_downloaded: asset.full_size,
                    total_bytes:      asset.full_size,
                    percent:          100.0,
                    speed_bps:        0,
                    stage:            if verify_result.is_ok() {
                        DownloadStage::Ready
                    } else {
                        DownloadStage::Failed
                    },
                    path:             Some(path.to_string_lossy().into_owned()),
                };
                *state_clone.progress.lock().await = Some(ready_progress.clone());
                let _ = app_handle2.emit("updater:progress", &ready_progress);

                if verify_result.is_ok() {
                    let _ = app_handle2.emit("updater:ready", &version2);
                } else {
                    let _ = app_handle2.emit("updater:error", verify_result.err().map(|e| e.to_string()).unwrap_or_default());
                }
            }
            Err(e) => {
                let fail = DownloadProgress {
                    version:          version2,
                    bytes_downloaded: 0,
                    total_bytes:      0,
                    percent:          0.0,
                    speed_bps:        0,
                    stage:            DownloadStage::Failed,
                    path:             None,
                };
                *state_clone.progress.lock().await = Some(fail.clone());
                let _ = app_handle2.emit("updater:error", e.to_string());
            }
        }
    });

    Ok(initial)
}

/// Poll the current download progress.
#[tauri::command]
pub async fn get_download_progress(
    state: State<'_, Arc<UpdaterState>>,
) -> Result<Option<DownloadProgress>, AutoDevError> {
    Ok(state.progress.lock().await.clone())
}

// ─── Install ──────────────────────────────────────────────────────────────────

/// Schedule update installation. The app will restart to apply.
#[tauri::command]
pub async fn install_update(
    version:    String,
    app_handle: tauri::AppHandle,
    state:      State<'_, Arc<UpdaterState>>,
) -> Result<InstallResult, AutoDevError> {
    let progress = state.progress.lock().await.clone()
        .ok_or_else(|| AutoDevError::Agent("No download in progress — call download_update first".into()))?;

    if progress.stage != DownloadStage::Ready {
        return Err(AutoDevError::Agent(
            format!("Download not ready (stage: {:?})", progress.stage)
        ));
    }

    let installer_path = progress.path
        .as_ref()
        .map(std::path::PathBuf::from)
        .ok_or_else(|| AutoDevError::Agent("Installer path missing".into()))?;

    // The download command's verification proves nothing about this file now.
    // It sits in a user-writable directory (~/.autodev/updates) and install is a
    // SEPARATE IPC call, so anything able to write there between the two calls
    // gets its binary installed. Re-verify against the published checksum and
    // signature immediately before scheduling.
    let info = state.latest.lock().await.clone()
        .ok_or_else(|| AutoDevError::Agent(
            "No update info cached — cannot verify the installer before install".into()
        ))?;

    // The caller supplies `version` independently of the download that actually
    // happened; without this the update history could record a version that was
    // never the one installed.
    if info.version != version || progress.version != version {
        return Err(AutoDevError::Agent(format!(
            "Version mismatch at install: requested={version}, cached={}, downloaded={}",
            info.version, progress.version
        )));
    }

    let asset = info.asset_for_current_platform()
        .ok_or_else(|| AutoDevError::Agent("No update available for this platform".into()))?;

    let expected_checksum = updater::download::download_checksum(&asset.checksum)
        .await
        .map_err(|e| AutoDevError::Agent(format!(
            "Install refused: could not fetch the published checksum: {e}"
        )))?;

    updater::verify_update(&installer_path, &expected_checksum, &asset.signature)
        .await
        .map_err(|e| AutoDevError::Agent(format!("Install refused: {e}")))?;

    // Pre-update: verify data integrity and create restore point
    updater::create_restore_point()
        .map_err(|e| AutoDevError::Agent(format!("Restore point creation failed: {e}")))?;

    updater::verify_data_integrity()
        .map_err(|e| AutoDevError::Agent(format!("Data integrity check failed: {e}")))?;

    let result = updater::schedule_install(&installer_path, &version, updater::APP_VERSION)
        .map_err(AutoDevError::from)?;

    if result.requires_restart {
        // Give frontend time to show "restarting" UI
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        app_handle.restart();
    }

    Ok(result)
}

// ─── Skip / History ───────────────────────────────────────────────────────────

/// Mark a version as skipped — don't prompt again.
#[tauri::command]
pub async fn skip_version(
    version: String,
    state:   State<'_, Arc<UpdaterState>>,
) -> Result<(), AutoDevError> {
    updater::skip_version(&version).map_err(AutoDevError::from)?;
    // Clear cached update if it's the skipped version
    let mut latest = state.latest.lock().await;
    if latest.as_ref().map(|u| u.version == version).unwrap_or(false) {
        *latest = None;
    }
    Ok(())
}

/// Get the history of installed updates.
#[tauri::command]
pub async fn get_update_history() -> Result<Vec<UpdateRecord>, AutoDevError> {
    Ok(updater::get_update_history())
}

// ─── Data integrity ───────────────────────────────────────────────────────────

/// Verify that all user data is intact (pre or post update).
#[tauri::command]
pub async fn verify_data_integrity() -> Result<IntegrityReport, AutoDevError> {
    updater::verify_data_integrity().map_err(AutoDevError::from)
}
