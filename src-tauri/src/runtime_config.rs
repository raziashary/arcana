// runtime_config.rs — Tiny on-disk JSON file read at app startup.
//
// Holds flags that need to be known *before* Tauri state and sidecar spawn
// are wired up — i.e. things that can't live in localStorage (which is only
// readable from the webview, after the webview has loaded).
//
// Today the only flag is `skip_sidecars`, which puts AutoDev into Demo Mode:
//   - memory-mcp-server and core-orchestrator are NOT spawned at startup
//   - the StartupProgress FSM fast-forwards to `Ready`
//   - the Chat tab and Worker recipe scheduler continue to function via the
//     pure-Rust `agent::engine::AgentEngine` path (which calls the LLM
//     provider HTTP API directly and never touches the Python sidecars).
//
// Storage path: ~/.autodev/runtime_config.json
// Schema:       {"skip_sidecars": false}
//
// The file is created lazily on first write; missing-file returns defaults.
// Settings UI writes via `set_skip_sidecars` and the user must restart the
// app for the change to take effect (because sidecar spawn happens once at
// startup).

use std::fs;
use std::io::Write;
use std::path::PathBuf;

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
    /// When true, AutoDev runs in Demo Mode:
    /// - sidecars are not spawned on launch
    /// - the loading overlay fast-forwards to Ready
    /// - LivingAgent / mem0 features are unavailable, but Chat + Workers work
    #[serde(default)]
    pub skip_sidecars: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { skip_sidecars: false }
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

/// Read the runtime config. Missing file → defaults. Malformed JSON →
/// defaults + warning (don't crash the app over a bad config).
pub fn read() -> RuntimeConfig {
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

/// Atomically write the runtime config to disk.
pub fn write(cfg: &RuntimeConfig) -> std::io::Result<()> {
    let path = config_path();
    let json = serde_json::to_string_pretty(cfg)
        .unwrap_or_else(|_| "{}".into());

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
    Ok(())
}
