// browser_commands.rs — Ruflo Feature 7.
// Playwright installation detection and browser setup helpers.
// Used by Settings > Browser Testing section to report Playwright availability
// and trigger browser installation for the QC Agent's browser automation skill.

use serde::{Deserialize, Serialize};
use tokio::process::Command;

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserInfo {
    pub name:      String,
    pub installed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BrowserAvailability {
    pub playwright_installed: bool,
    pub playwright_version:   Option<String>,
    pub browsers:             Vec<BrowserInfo>,
    pub npx_available:        bool,
}

// ── Commands ───────────────────────────────────────────────────────────────────

/// Detect whether Playwright and its browsers are available on this machine.
/// Called from the Settings > Browser Testing card on mount.
#[tauri::command]
pub async fn check_playwright_available() -> Result<BrowserAvailability, String> {
    // Step 1 — is npx present?
    let npx_ok = Command::new("npx")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !npx_ok {
        return Ok(BrowserAvailability {
            playwright_installed: false,
            playwright_version:   None,
            browsers:             Vec::new(),
            npx_available:        false,
        });
    }

    // Step 2 — is @playwright/test installed?
    let pw = Command::new("npx")
        .args(["playwright", "--version"])
        .output()
        .await;

    let (installed, version) = match pw {
        Ok(out) if out.status.success() => {
            // stdout is e.g. "Version 1.44.0\n"
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let ver = raw.split_whitespace().last().map(|s| s.to_string());
            (true, ver)
        }
        _ => (false, None),
    };

    if !installed {
        return Ok(BrowserAvailability {
            playwright_installed: false,
            playwright_version:   None,
            browsers:             Vec::new(),
            npx_available:        true,
        });
    }

    // Step 3 — check each browser via dry-run.
    // `npx playwright install --dry-run <name>` exits 0 and prints nothing when
    // the browser is already cached; it prints a download plan when it is not.
    let mut browsers = Vec::new();
    for &name in &["chromium", "firefox", "webkit"] {
        let res = Command::new("npx")
            .args(["playwright", "install", "--dry-run", name])
            .output()
            .await;
        let browser_installed = res
            .map(|o| o.status.success() && o.stdout.is_empty())
            .unwrap_or(false);
        browsers.push(BrowserInfo { name: name.to_string(), installed: browser_installed });
    }

    Ok(BrowserAvailability {
        playwright_installed: true,
        playwright_version:   version,
        browsers,
        npx_available:        true,
    })
}

/// Run `npx playwright install <browser>` to download the specified browser binary.
/// `browser` must be one of: "chromium", "firefox", "webkit", "all".
#[tauri::command]
pub async fn install_playwright_browsers(browser: String) -> Result<(), String> {
    let output = Command::new("npx")
        .args(["playwright", "install", &browser])
        .output()
        .await
        .map_err(|e| format!("Failed to launch playwright install: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("playwright install {browser} failed: {stderr}"))
    }
}
