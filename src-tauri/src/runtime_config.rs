// runtime_config.rs — Tiny on-disk JSON file read at app startup.
//
// Holds settings the Rust side needs without a round-trip to the webview —
// i.e. things that cannot live in localStorage, which is only readable after
// the webview has loaded.
//
// Today that is the **default worker budgets** (Ruflo Feature 4), applied when
// a Supervisor assigns a task without an explicit budget. `budget_commands.rs`
// writes them; the recipe runner reads them.
//
// Storage path: ~/.autodev/runtime_config.json
// The file is created lazily on first write; a missing file returns defaults.
//
// 🔴 **`skip_sidecars` was removed in `P8-11`/`P8-53`.** It was Demo Mode: a
// flag that stopped the Python sidecars spawning. With no sidecars to skip,
// the feature had nothing left to mean.
//
// ⚠ **This file was itself nearly deleted.** `P8-53` said its *"only field is
// `skip_sidecars`"* and prescribed deleting it. There were six, and five are
// the live budgets above — removing them would have taken the Supervisor's
// default budgets with it, silently. Removing one field is backwards
// compatible: every field carries `#[serde(default)]` and serde ignores the
// now-unknown key, so an existing config file still parses.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

const FILE_NAME: &str = "runtime_config.json";

/// On-disk runtime configuration.
///
/// `serde(default)` on every field means missing-key reads succeed and the
/// default value is used — so adding new fields in the future is backwards
/// compatible with old config files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    // Ruflo Feature 4 — Default budgets applied when Supervisor assigns a task
    // without an explicit budget. Stored here (not localStorage) so the Rust
    // recipe runner can read them without a round-trip to the webview.
    #[serde(default = "default_budget_tokens")]
    pub default_worker_budget_tokens: u64,
    #[serde(default = "default_budget_usd")]
    pub default_worker_budget_usd: f64,
    #[serde(default = "default_budget_turns")]
    pub default_worker_budget_turns: u32,
    #[serde(default = "default_budget_warn_pct")]
    pub budget_warn_pct: u8, // warn at X% of budget; default 80
}

fn default_budget_tokens() -> u64 {
    100_000
}
fn default_budget_usd() -> f64 {
    0.50
}
fn default_budget_turns() -> u32 {
    20
}
fn default_budget_warn_pct() -> u8 {
    80
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_worker_budget_tokens: default_budget_tokens(),
            default_worker_budget_usd: default_budget_usd(),
            default_worker_budget_turns: default_budget_turns(),
            budget_warn_pct: default_budget_warn_pct(),
        }
    }
}

/// Resolve the on-disk path: `<HOME>/.autodev/runtime_config.json`.
fn config_path() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = home.join(".autodev");
    let _ = fs::create_dir_all(&dir);
    dir.join(FILE_NAME)
}

// Doc 08 P1 — cached runtime config singleton.
//
// The 14 May log showed `[runtime_config] loaded …` repeated 4× per run
// (L24, L131–138, L818, L956–959) because main.rs and several command
// handlers each independently called `read()`. The values never change at
// runtime — writes go through `write()` which then refreshes the cache
// (see `write()` below) — so a single-load + cached pattern is correct.
//
// `OnceLock<Mutex<…>>` is used (not just `OnceLock<RuntimeConfig>`) so
// `write()` can replace the cached value in place without rebuilding the
// lock; readers always see the latest committed config.
static CACHE: OnceLock<Mutex<RuntimeConfig>> = OnceLock::new();

fn cache() -> &'static Mutex<RuntimeConfig> {
    CACHE.get_or_init(|| Mutex::new(load_from_disk()))
}

/// Force a re-read from disk. Useful only for tests or in-place edits
/// performed outside the running process (which the app doesn't support).
/// Production code paths never call this — `write()` keeps the cache in
/// lockstep with disk on every write.
#[allow(dead_code)]
pub fn invalidate() {
    if let Some(m) = CACHE.get() {
        if let Ok(mut g) = m.lock() {
            *g = load_from_disk();
        }
    }
}

/// Disk read + parse — the operation that used to be `read()` directly.
/// Called once per process from `cache()`; the info log fires exactly once
/// per host lifetime instead of once per caller.
fn load_from_disk() -> RuntimeConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<RuntimeConfig>(&text) {
            Ok(cfg) => {
                info!("[runtime_config] loaded {:?}: {:?}", path, cfg);
                cfg
            }
            Err(e) => {
                warn!(
                    "[runtime_config] {:?} is malformed ({}); using defaults",
                    path, e
                );
                RuntimeConfig::default()
            }
        },
        Err(_) => {
            // Missing file is the normal first-run case; not noisy.
            RuntimeConfig::default()
        }
    }
}

/// Read the runtime config. Missing file → defaults. Malformed JSON →
/// defaults + warning (don't crash the app over a bad config).
///
/// Doc 08 P1 — this is now a cheap clone of the process-wide cached
/// value; the disk read + info log fire only on the first call.
pub fn read() -> RuntimeConfig {
    cache().lock().map(|g| g.clone()).unwrap_or_default()
}

/// Atomically write the runtime config to disk.
pub fn write(cfg: &RuntimeConfig) -> std::io::Result<()> {
    let path = config_path();
    let json = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());

    // Write to a sibling temp file then rename — prevents a half-written file
    // if the process is killed mid-write.
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(json.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, &path)?;
    info!("[runtime_config] wrote {:?}: {:?}", path, cfg);
    // Doc 08 P1 — keep the cache in lockstep with the disk write so
    // subsequent `read()` calls reflect the new value.
    if let Some(m) = CACHE.get() {
        if let Ok(mut g) = m.lock() {
            *g = cfg.clone();
        }
    }
    Ok(())
}
