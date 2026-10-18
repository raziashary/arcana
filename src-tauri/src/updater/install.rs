// Auto-updater install module.
// Handles the atomic installation of a downloaded update package.
// Strategy:
//   - Windows:  Write a .bat launcher that runs the MSI/NSIS installer
//               silently, then restarts the app. Tauri restarts invoke
//               `process::exit(0)` after writing the launcher.
//   - macOS:    Open the .dmg with `open` command; user drags to Applications.
//               For automated: `hdiutil` + `rsync` to update the .app bundle.
//   - Linux:    If AppImage: replace the current AppImage with the downloaded one.
//               If .deb: `pkexec apt install -y <path>` (requires polkit).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub version: String,
    pub message: String,
    pub requires_restart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRecord {
    pub version: String,
    pub installed_at: String,
    pub previous_version: String,
    pub channel: String,
}

/// Prepare the update for installation and schedule it on the next restart.
/// Returns immediately — the actual install happens when the app restarts.
pub fn schedule_install(
    installer_path: &Path,
    version: &str,
    current_version: &str,
) -> Result<InstallResult> {
    #[cfg(target_os = "windows")]
    let result = schedule_windows(installer_path, version)?;
    #[cfg(target_os = "macos")]
    let result = schedule_macos(installer_path, version)?;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let result = schedule_linux(installer_path, version)?;

    // Record the pending update
    record_pending_update(version, current_version).ok();

    Ok(result)
}

// ─── Windows ─────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn schedule_windows(installer_path: &Path, version: &str) -> Result<InstallResult> {
    let installer_str = installer_path.to_string_lossy();
    let app_path = current_exe_path();
    let relaunch_cmd = app_path.to_string_lossy();

    // Write a batch file to TEMP that runs the installer then relaunches the app
    let temp = std::env::var("TEMP").unwrap_or_else(|_| "C:\\Temp".to_string());
    let batch_path = PathBuf::from(&temp).join("arcana_update.bat");

    let ext = installer_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let install_cmd = if ext == "msi" {
        format!("msiexec /i \"{installer_str}\" /qn /norestart")
    } else {
        // NSIS installer
        format!("\"{installer_str}\" /S")
    };

    let batch_content = format!(
        "@echo off\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         {install_cmd}\r\n\
         start \"\" \"{relaunch_cmd}\"\r\n\
         del \"%~f0\"\r\n"
    );

    std::fs::write(&batch_path, batch_content)?;

    // Register the batch to run via registry RunOnce so it survives reboot too
    write_runonce_key(&batch_path)?;

    Ok(InstallResult {
        success: true,
        version: version.to_string(),
        message: "Update ready. Restart Arcana to apply.".into(),
        requires_restart: true,
    })
}

#[cfg(target_os = "windows")]
fn write_runonce_key(batch_path: &Path) -> Result<()> {
    // Use reg.exe to avoid pulling in winreg crate
    let status = std::process::Command::new("reg")
        .args([
            "add",
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
            "/v",
            "ArcanaUpdate",
            "/d",
            &format!("\"{}\"", batch_path.display()),
            "/f",
        ])
        .status()
        .context("Failed to run reg.exe")?;
    if !status.success() {
        // Non-fatal — app will still be able to install manually
        eprintln!("[updater] Failed to set RunOnce registry key");
    }
    Ok(())
}

// ─── macOS ────────────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn schedule_macos(installer_path: &Path, version: &str) -> Result<InstallResult> {
    // Write a shell script to ~/Library/Application Support/Arcana/update.sh
    let support = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Arcana");
    std::fs::create_dir_all(&support)?;

    let script_path = support.join("update.sh");
    let dmg_path = installer_path.to_string_lossy();
    let app_path = current_exe_path().to_string_lossy().into_owned();

    // Find the .app path from the running binary
    let app_bundle = find_app_bundle(&app_path);

    let script = format!(
        r#"#!/usr/bin/env bash
set -e
sleep 2
MOUNT=$(hdiutil attach "{dmg_path}" -nobrowse -mountpoint /tmp/arcana_update 2>/dev/null | tail -1 | awk '{{print $3}}')
if [ -d "$MOUNT/Arcana.app" ]; then
    rsync -a --delete "$MOUNT/Arcana.app/" "{app_bundle}/"
fi
hdiutil detach "$MOUNT" 2>/dev/null || true
open "{app_bundle}"
rm -- "$0"
"#,
        dmg_path = dmg_path,
        app_bundle = app_bundle,
    );

    std::fs::write(&script_path, &script)?;
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // Register as LaunchAgent or just mark for auto-run on next start
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.arcana.desktop.update</string>
    <key>ProgramArguments</key>
    <array><string>{script}</string></array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
        script = script_path.display(),
    );

    let agents_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents_dir)?;
    std::fs::write(agents_dir.join("dev.arcana.desktop.update.plist"), plist)?;

    Ok(InstallResult {
        success: true,
        version: version.to_string(),
        message: "Update ready. Quit Arcana to apply.".into(),
        requires_restart: true,
    })
}

#[cfg(target_os = "macos")]
fn find_app_bundle(binary_path: &str) -> String {
    // Walk up from Contents/MacOS/arcana to find .app bundle
    let mut p = std::path::PathBuf::from(binary_path);
    for _ in 0..4 {
        p.pop();
        if p.to_string_lossy().ends_with(".app") {
            return p.to_string_lossy().into_owned();
        }
    }
    "/Applications/Arcana.app".to_string()
}

// ─── Linux ────────────────────────────────────────────────────────────────────

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn schedule_linux(installer_path: &Path, version: &str) -> Result<InstallResult> {
    let installer_str = installer_path.to_string_lossy();
    let ext = installer_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "appimage" {
        // Replace the current AppImage
        let current_exe = current_exe_path();
        let script_path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("arcana-update.sh");

        let script = format!(
            r#"#!/usr/bin/env bash
set -e
sleep 2
chmod +x "{installer}"
cp -f "{installer}" "{current}"
exec "{current}"
"#,
            installer = installer_str,
            current = current_exe.display(),
        );
        std::fs::write(&script_path, &script)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;

        Ok(InstallResult {
            success: true,
            version: version.to_string(),
            message: "AppImage update ready. Restart Arcana to apply.".into(),
            requires_restart: true,
        })
    } else if ext == "deb" {
        // Schedule deb installation via pkexec
        let script_path = PathBuf::from("/tmp/arcana-update.sh");
        let script =
            format!("#!/usr/bin/env bash\npkexec apt-get install -y \"{installer_str}\"\n");
        std::fs::write(&script_path, &script)?;
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;

        Ok(InstallResult {
            success: true,
            version: version.to_string(),
            message: "Deb package ready. Restart Arcana to install (requires pkexec).".into(),
            requires_restart: true,
        })
    } else {
        Ok(InstallResult {
            success: false,
            version: version.to_string(),
            message: format!("Unsupported installer format: .{ext}"),
            requires_restart: false,
        })
    }
}

// ─── History & records ────────────────────────────────────────────────────────

/// Persist an update record to ~/.arcana/update_history.json
pub fn record_pending_update(version: &str, current_version: &str) -> Result<()> {
    let path = update_history_path();
    let mut records: Vec<UpdateRecord> = if path.exists() {
        let s = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        vec![]
    };

    records.push(UpdateRecord {
        version: version.to_string(),
        // Canonical fixed-millisecond UTC form (`db::timestamps`), like every
        // other persisted timestamp. `iso_now` used to hand-roll a
        // second-precision copy of this here, which is the divergence
        // P4-57/TG4-19 exists to prevent.
        installed_at: arcana_core::db::timestamps::now_rfc3339(),
        previous_version: current_version.to_string(),
        channel: "stable".to_string(),
    });

    // Keep last 20 records
    if records.len() > 20 {
        records.drain(..records.len() - 20);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&records)?)?;
    Ok(())
}

pub fn get_update_history() -> Vec<UpdateRecord> {
    let path = update_history_path();
    if !path.exists() {
        return vec![];
    }
    let s = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&s).unwrap_or_default()
}

pub fn skip_version(version: &str) -> Result<()> {
    let path = skipped_versions_path();
    let mut skipped: Vec<String> = if path.exists() {
        let s = std::fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        vec![]
    };
    if !skipped.contains(&version.to_string()) {
        skipped.push(version.to_string());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string(&skipped)?)?;
    Ok(())
}

pub fn is_skipped(version: &str) -> bool {
    let path = skipped_versions_path();
    if !path.exists() {
        return false;
    }
    let s = std::fs::read_to_string(&path).unwrap_or_default();
    let skipped: Vec<String> = serde_json::from_str(&s).unwrap_or_default();
    skipped.contains(&version.to_string())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn current_exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("arcana"))
}

fn update_history_path() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".arcana")
        .join("update_history.json")
}

fn skipped_versions_path() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".arcana")
        .join("skipped_versions.json")
}
