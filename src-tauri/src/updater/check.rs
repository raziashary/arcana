// Auto-updater version check module.
// Queries the update server (GitHub Releases API by default) and returns
// update metadata if a newer version is available.

use anyhow::{Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};

/// Arcana has no published release feed yet. A feed must be opted into explicitly.

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
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Nightly => "nightly",
        }
    }
}

/// Per-platform download information.
///
/// `signature` and `checksum` are **URLs of sidecar assets**, not the values
/// themselves — they are fetched and compared at verification time. (The field
/// docs previously described them as the values; they never held those.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub full_url: String,
    pub full_size: u64,
    /// URL of the detached Ed25519 `.sig` for `full_url`.
    pub signature: String,
    /// URL of the `.sha256` sidecar for `full_url`.
    pub checksum: String,
}

/// Update information returned from the update server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub release_date: String,
    pub release_notes: String,
    /// true = security update — enforce installation.
    pub critical: bool,
    pub channel: UpdateChannel,
    pub platforms: std::collections::HashMap<String, PlatformAsset>,
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
    let url = match std::env::var("ARCANA_UPDATE_URL") {
        Ok(url) if !url.trim().is_empty() => url,
        _ => return Ok(None),
    };

    // The manifest endpoint is overridable, so it is guarded like any other
    // update endpoint rather than trusted because it has a default.
    super::download::require_https(&url).context("Update manifest URL rejected")?;

    let url = match channel {
        UpdateChannel::Stable => url,
        UpdateChannel::Beta => url.replace("releases/latest", "releases?per_page=5"),
        UpdateChannel::Nightly => url.replace("releases/latest", "releases?per_page=10"),
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .https_only(true)
        .user_agent(format!("Arcana/{current_version} (auto-updater)"))
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
    let payload: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse update response")?;

    // The beta and nightly channels rewrite the URL to the *list* endpoint,
    // which returns an array. The previous code went straight on to read
    // `tag_name` off that array — always null — so neither channel could ever
    // return an update. Select the newest release the channel accepts instead.
    let release = match payload.as_array() {
        Some(releases) => match releases.iter().find(|r| channel_accepts(&channel, r)) {
            Some(r) => r.clone(),
            None => return Ok(None),
        },
        None => payload,
    };

    let latest_tag = release["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if latest_tag.is_empty() {
        return Ok(None);
    }

    let current = Version::parse(current_version).unwrap_or(Version::new(0, 0, 0));
    let latest = Version::parse(latest_tag).unwrap_or(Version::new(0, 0, 0));

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
    // Linux publishes both an AppImage and a .deb under one platform key; without
    // a rank the winner would depend on the order GitHub happens to return.
    let mut ranks: std::collections::HashMap<String, u8> = std::collections::HashMap::new();
    if let Some(assets) = release["assets"].as_array() {
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            let url = asset["browser_download_url"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let size = asset["size"].as_u64().unwrap_or(0);

            if let Some(key) = platform_key_from_filename(name) {
                let rank = asset_preference(name);
                if rank > ranks.get(&key).copied().unwrap_or(0) {
                    ranks.insert(key.clone(), rank);
                    platforms.insert(
                        key,
                        PlatformAsset {
                            full_url: url,
                            full_size: size,
                            signature: String::new(), // Loaded separately from .sig asset
                            checksum: String::new(),  // Loaded separately from .sha256 asset
                        },
                    );
                }
            }
        }

        // Second pass: attach .sig and .sha256 sidecars to the asset they
        // actually belong to.
        //
        // Matching on the platform key alone is not sufficient: Linux publishes
        // sidecars for BOTH the AppImage and the .deb and both map to
        // `linux-x64`, so whichever sidecar was processed last would win and the
        // .deb's checksum could end up attached to the AppImage — refusing every
        // update as a mismatch. Require the sidecar's base name to be the asset
        // that was actually selected.
        for asset in assets {
            let name = asset["name"].as_str().unwrap_or("");
            let url = asset["browser_download_url"]
                .as_str()
                .unwrap_or("")
                .to_string();

            let (base, is_signature) = if let Some(b) = name.strip_suffix(".sig") {
                (b, true)
            } else if let Some(b) = name.strip_suffix(".sha256") {
                (b, false)
            } else {
                continue;
            };

            let Some(key) = platform_key_from_filename(base) else {
                continue;
            };
            let Some(entry) = platforms.get_mut(&key) else {
                continue;
            };
            if !entry.full_url.ends_with(base) {
                continue;
            }

            if is_signature {
                entry.signature = url;
            } else {
                entry.checksum = url;
            }
        }
    }

    let critical = release["body"]
        .as_str()
        .unwrap_or("")
        .to_lowercase()
        .contains("[critical]");

    let release_notes = release["body"]
        .as_str()
        .unwrap_or("No release notes.")
        .to_string();
    let release_date = release["published_at"].as_str().unwrap_or("").to_string();

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
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "windows-arm64".to_string();
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

/// Whether a release from the GitHub *list* endpoint is eligible for a channel.
///
/// Drafts are never offered: their assets are not publicly downloadable, so a
/// draft would produce an update the user cannot install.
fn channel_accepts(channel: &UpdateChannel, release: &serde_json::Value) -> bool {
    if release["draft"].as_bool().unwrap_or(false) {
        return false;
    }
    let prerelease = release["prerelease"].as_bool().unwrap_or(false);
    match channel {
        UpdateChannel::Stable => !prerelease,
        // Beta and nightly deliberately accept pre-releases; that is the point
        // of the channel.
        UpdateChannel::Beta | UpdateChannel::Nightly => true,
    }
}

/// Map a release asset filename to a platform key.
///
/// Matches on **extension plus architecture**, not on exact infix strings. The
/// previous version required `_x64.msi` or `_x64-setup.exe` for Windows while
/// `installers/wix/build-msi.ps1:222` actually ships
/// `arcana_<version>[_channel]_x64_en-US.exe`, and required `_amd64.deb` while
/// `scripts/build-linux.sh:90` ships `arcana_<version>_x86_64.deb`. Both
/// therefore matched nothing, so Windows and the Linux `.deb` resolved to no
/// asset at all and no update could ever be offered on those platforms.
fn platform_key_from_filename(name: &str) -> Option<String> {
    let n = name.to_lowercase();
    let arm = n.contains("aarch64") || n.contains("arm64");

    if n.ends_with(".msi") || n.ends_with(".exe") {
        Some(if arm { "windows-arm64" } else { "windows-x64" }.to_string())
    } else if n.ends_with(".dmg") {
        Some(if arm { "macos-arm64" } else { "macos-x64" }.to_string())
    } else if n.ends_with(".appimage") || n.ends_with(".deb") {
        Some(if arm { "linux-arm64" } else { "linux-x64" }.to_string())
    } else {
        None
    }
}

/// Preference when two assets claim the same platform key.
///
/// Linux ships both an AppImage and a `.deb` under `linux-x64`; without a rank
/// the winner depends on the order GitHub returns assets in. AppImage wins
/// because it needs no package manager and no root.
fn asset_preference(name: &str) -> u8 {
    let n = name.to_lowercase();
    if n.ends_with(".appimage") {
        2
    } else if n.ends_with(".msi")
        || n.ends_with(".exe")
        || n.ends_with(".dmg")
        || n.ends_with(".deb")
    {
        1
    } else {
        0
    }
}

/// Parse a version string, defaulting to 0.0.0 on failure.
pub fn parse_version(s: &str) -> Version {
    Version::parse(s.trim_start_matches('v')).unwrap_or(Version::new(0, 0, 0))
}
