// Auto-updater version check module.
// Queries the update server (GitHub Releases API by default) and returns
// update metadata if a newer version is available.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use semver::Version;

/// The update server base URL. Can be overridden by AUTODEV_UPDATE_URL env var.
const DEFAULT_UPDATE_URL: &str =
    "https://api.github.com/repos/AIRS-Foundry/autodev/releases/latest";

/// Which update channel to use: stable, beta, nightly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
    Nightly,
}

impl UpdateChannel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Stable  => "stable",
            Self::Beta    => "beta",
            Self::Nightly => "nightly",
        }
    }
}

/// Per-platform download information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub full_url:   String,
    pub delta_url:  Option<String>,
    pub full_size:  u64,
    pub delta_size: Option<u64>,
    /// Base64-encoded Ed25519 signature over the file bytes.
    pub signature:  String,
    pub checksum:   String, // SHA-256 hex of the full file
}

/// Update information returned from the update server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version:       String,
    pub release_date:  String,
    pub release_notes: String,
    /// true = security update — enforce installation.
    pub critical:      bool,
    pub channel:       UpdateChannel,
    pub platforms:     std::collections::HashMap<String, PlatformAsset>,
}

impl UpdateInfo {
    /// Return the asset for the current OS/arch, or None.
    pub fn asset_for_current_platform(&self) -> Option<&PlatformAsset> {
        let key = current_platform_key();
        self.platforms.get(&key)
    }
}

/// Check for updates. Returns `None` if current version is up to date.
pub async fn check_for_update(
    current_version: &str,
    channel: UpdateChannel,
) -> Result<Option<UpdateInfo>> {
    let url = std::env::var("AUTODEV_UPDATE_URL")
        .unwrap_or_else(|_| DEFAULT_UPDATE_URL.to_string());

    let url = match channel {
        UpdateChannel::Stable  => url,
        UpdateChannel::Beta    => url.replace("releases/latest", "releases?per_page=5"),
        UpdateChannel::Nightly => url.replace("releases/latest", "releases?per_page=10"),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("AutoDev/{current_version} (auto-updater)"))
        .build()
        .context("Failed to build HTTP client")?;

    let resp = client
        .get(&url)
        .send()
        .await
        .context("Update check request failed")?;

    if !resp.status().is_success() {
        // Non-200 → treat as "no update available" (offline / server error).
        return Ok(None);
    }

    // GitHub returns a release object; convert to our UpdateInfo format.
    let release: serde_json::Value = resp.json().await.context("Failed to parse update response")?;

    let latest_tag = release["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if latest_tag.is_empty() {
        return Ok(None);
    }

    let current = Version::parse(current_version).unwrap_or(Version::new(0, 0, 0));
    let latest  = Version::parse(latest_tag).unwrap_or(Version::new(0, 0, 0));

    if latest <= current {
        return Ok(None); // Already up to date
    }

    // Check channel filter (skip pre-release for stable channel)
    let prerelease = release["prerelease"].as_bool().unwrap_or(false);
    if channel == UpdateChannel::Stable && prerelease {
        return Ok(None);
    }

    // Build asset map from GitHub release assets
    let mut platforms = std::collections::HashMap::new();
    if let Some(assets) = release["assets"].as_array() {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            let url  = asset["browser_download_url"].as_str().unwrap_or("").to_string();
            let size = asset["size"].as_u64().unwrap_or(0);

            let key = platform_key_from_filename(name);
            if let Some(key) = key {
                platforms.insert(key, PlatformAsset {
                    full_url:   url,
                    delta_url:  None,
                    full_size:  size,
                    delta_size: None,
                    signature:  String::new(), // Loaded separately from .sig asset
                    checksum:   String::new(), // Loaded separately from .sha256 asset
                });
            }
        }

        // Second pass: load .sig and .sha256 sidecar files
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            let url  = asset["browser_download_url"].as_str().unwrap_or("").to_string();

            if name.ends_with(".sig") {
                let base = name.trim_end_matches(".sig");
                if let Some(key) = platform_key_from_filename(base) {
                    if let Some(entry) = platforms.get_mut(&key) {
                        entry.signature = url;
                    }
                }
            } else if name.ends_with(".sha256") {
                let base = name.trim_end_matches(".sha256");
                if let Some(key) = platform_key_from_filename(base) {
                    if let Some(entry) = platforms.get_mut(&key) {
                        entry.checksum = url;
                    }
                }
            }
        }
    }

    let critical = release["body"]
        .as_str()
        .unwrap_or("")
        .to_lowercase()
        .contains("[critical]");

    let release_notes = release["body"].as_str().unwrap_or("No release notes.").to_string();
    let release_date  = release["published_at"].as_str().unwrap_or("").to_string();

    Ok(Some(UpdateInfo {
        version: latest_tag.to_string(),
        release_date,
        release_notes,
        critical,
        channel,
        platforms,
    }))
}

// ─── Platform helpers ─────────────────────────────────────────────────────────

pub fn current_platform_key() -> String {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "windows-x64".to_string();
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "macos-x64".to_string();
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "macos-arm64".to_string();
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux-x64".to_string();
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux-arm64".to_string();
    #[allow(unreachable_code)]
    "unknown".to_string()
}

fn platform_key_from_filename(name: &str) -> Option<String> {
    let name_lower = name.to_lowercase();
    if name_lower.contains("_x64.msi") || name_lower.contains("_x64-setup.exe") {
        Some("windows-x64".to_string())
    } else if name_lower.contains("_x64.dmg") || name_lower.contains("_amd64.dmg") {
        Some("macos-x64".to_string())
    } else if name_lower.contains("_aarch64.dmg") || name_lower.contains("_arm64.dmg") {
        Some("macos-arm64".to_string())
    } else if name_lower.contains("_amd64.appimage") || name_lower.contains("_x86_64.appimage") {
        Some("linux-x64".to_string())
    } else if name_lower.contains("_amd64.deb") {
        Some("linux-x64".to_string())
    } else if name_lower.contains("_aarch64.appimage") || name_lower.contains("_arm64.appimage") {
        Some("linux-arm64".to_string())
    } else {
        None
    }
}

/// Parse a version string, defaulting to 0.0.0 on failure.
pub fn parse_version(s: &str) -> Version {
    Version::parse(s.trim_start_matches('v')).unwrap_or(Version::new(0, 0, 0))
}
