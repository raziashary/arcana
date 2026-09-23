// Plugin registry — local disk scanner + remote registry stub.
// The local registry lives at ~/.arcana/plugins/<name>/manifest.yaml.
// The remote registry is a future-facing struct ready for an HTTP source.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::builtins::all_builtins;
use super::manifest::{InstalledPlugin, PluginInfo, PluginManifest};

/// Root directory for locally installed plugins.
pub fn plugin_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    let dir = PathBuf::from(home).join(".arcana").join("plugins");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Directory for a specific plugin.
pub fn plugin_install_dir(name: &str) -> Result<PathBuf> {
    Ok(plugin_dir()?.join(name))
}

// ─── Local registry ──────────────────────────────────────────────────────────

/// Scan `~/.arcana/plugins/` and return all installed plugin manifests.
pub fn list_installed() -> Vec<InstalledPlugin> {
    let Ok(dir) = plugin_dir() else {
        return vec![];
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| load_installed_from_dir(&e.path()).ok())
        .collect()
}

/// Load a single installed plugin from its directory.
fn load_installed_from_dir(dir: &std::path::Path) -> Result<InstalledPlugin> {
    // Try manifest.yaml first, then manifest.yml
    let yaml_path = dir.join("manifest.yaml");
    let yaml_path = if yaml_path.exists() {
        yaml_path
    } else {
        dir.join("manifest.yml")
    };

    let yaml = std::fs::read_to_string(&yaml_path)
        .with_context(|| format!("Cannot read {}", yaml_path.display()))?;
    let manifest: PluginManifest = serde_yaml::from_str(&yaml)
        .with_context(|| format!("Cannot parse {}", yaml_path.display()))?;

    // Try reading install metadata
    let meta_path = dir.join(".install_meta.json");
    let (installed_at, config) = if meta_path.exists() {
        let meta_str = std::fs::read_to_string(&meta_path).unwrap_or_default();
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap_or_default();
        let at = meta["installed_at"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let cfg = meta.get("config").cloned();
        (at, cfg)
    } else {
        ("unknown".into(), None)
    };

    Ok(InstalledPlugin {
        install_path: dir.to_string_lossy().into_owned(),
        manifest,
        installed_at,
        config,
    })
}

/// Check if a plugin name is currently installed on disk.
pub fn is_installed(name: &str) -> bool {
    plugin_dir()
        .map(|d| {
            d.join(name).join("manifest.yaml").exists()
                || d.join(name).join("manifest.yml").exists()
        })
        .unwrap_or(false)
}

/// Write a manifest and optional install metadata to `~/.arcana/plugins/<name>/`.
pub fn persist_install(
    manifest: &PluginManifest,
    config: Option<&serde_json::Value>,
) -> Result<PathBuf> {
    let dir = plugin_install_dir(&manifest.name)?;
    std::fs::create_dir_all(&dir)?;

    // Write manifest
    let yaml = serde_yaml::to_string(manifest)?;
    std::fs::write(dir.join("manifest.yaml"), yaml)?;

    // Write metadata
    let now = chrono_now();
    let meta = serde_json::json!({
        "installed_at": now,
        "config": config,
    });
    std::fs::write(
        dir.join(".install_meta.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    Ok(dir)
}

/// Remove a plugin directory from `~/.arcana/plugins/<name>/`.
pub fn remove_install(name: &str) -> Result<()> {
    let dir = plugin_install_dir(name)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .with_context(|| format!("Cannot remove {}", dir.display()))?;
    }
    Ok(())
}

// ─── Remote registry ─────────────────────────────────────────────────────────

/// A remote registry entry (used for future marketplace HTTP source).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePluginEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub icon: String,
    pub tags: Vec<String>,
    pub download_url: String,
    pub checksum: Option<String>,
}

/// Fetch plugins from a remote registry URL.
/// Returns an empty vec on failure (graceful degradation).
pub async fn fetch_remote(registry_url: &str) -> Vec<RemotePluginEntry> {
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    else {
        return vec![];
    };

    match client.get(registry_url).send().await {
        Ok(resp) if resp.status().is_success() => resp
            .json::<Vec<RemotePluginEntry>>()
            .await
            .unwrap_or_default(),
        _ => vec![],
    }
}

// ─── Marketplace listing ─────────────────────────────────────────────────────

/// Build a combined PluginInfo list: built-ins + installed, with `installed` flag.
pub fn list_marketplace() -> Vec<PluginInfo> {
    let installed = list_installed();
    let installed_names: std::collections::HashSet<&str> =
        installed.iter().map(|p| p.manifest.name.as_str()).collect();

    let mut out: Vec<PluginInfo> = vec![];

    // Built-ins first
    for m in all_builtins() {
        let mut info = PluginInfo::from(&m);
        info.installed = installed_names.contains(m.name.as_str()) || m.builtin;
        out.push(info);
    }

    // User-installed plugins not already in builtins
    let all_b = all_builtins();
    let builtin_names: std::collections::HashSet<&str> =
        all_b.iter().map(|b| b.name.as_str()).collect();

    for ip in &installed {
        if !builtin_names.contains(ip.manifest.name.as_str()) {
            let mut info = PluginInfo::from(&ip.manifest);
            info.installed = true;
            out.push(info);
        }
    }

    out
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn chrono_now() -> String {
    // Produce a simple ISO-8601 timestamp without pulling in chrono.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let y400 = days / 146097;
    let rem = days % 146097;
    let y100 = rem / 36524;
    let rem2 = rem % 36524;
    let y4 = rem2 / 1461;
    let rem3 = rem2 % 1461;
    let y1 = rem3 / 365;
    let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1970;
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mdays: [u64; 12] = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut doy = rem3 % 365 + if y1 == 4 { 365 } else { 0 };
    let mut month = 1u64;
    for md in &mdays {
        if doy < *md {
            break;
        }
        doy -= md;
        month += 1;
    }
    let day = doy + 1;
    format!("{year}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}
