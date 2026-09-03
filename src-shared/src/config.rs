// ~/.airs/config.toml — airsctl configuration file types and I/O.

use crate::errors::{Result, SharedError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved path to `~/.airs/config.toml`.
pub fn config_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| SharedError::Other("Cannot determine home directory".into()))?;
    let dir = PathBuf::from(home).join(".autodev");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CoreConfig {
    /// Default working directory for agent operations.
    pub working_dir: Option<String>,
    /// Default LLM provider name.
    pub default_provider: Option<String>,
}

// 🔴 `P8-64`. `CoreConfig` also carried `auto_start_sidecars` and
// `orchestrator_url`. Both are gone: there is nothing to auto-start and no
// second process to address.
//
// ⚠ Removing them is backwards compatible in the direction that matters.
// `CliConfigFile` derives `Deserialize` without `deny_unknown_fields`, so an
// existing `~/.autodev/config.toml` carrying either key still parses — serde
// ignores a key with no field. `config set core.orchestrator_url ...` now
// returns "Unknown key", which is the correct answer and better than accepting
// a value nothing will read.

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliConfig {
    pub output_format: Option<String>, // "table" | "json" | "plain"
    pub interactive: Option<bool>,
    pub history_size: Option<u32>,
    pub color: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkerConfig {
    pub worker_id: Option<String>,
    pub worker_type: Option<String>,
    pub heartbeat_interval: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CliConfigFile {
    pub core: Option<CoreConfig>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    pub cli: Option<CliConfig>,
    pub worker: Option<WorkerConfig>,
}

impl CliConfigFile {
    /// Load from `~/.airs/config.toml`, returning defaults if the file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let cfg: Self = toml::from_str(&content)?;
        Ok(cfg)
    }

    /// Persist to `~/.airs/config.toml`.
    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Create a default config file at `~/.airs/config.toml`.
    pub fn init_default() -> Result<Self> {
        let mut cfg = Self::default();
        cfg.core = Some(CoreConfig {
            working_dir: None,
            default_provider: Some("anthropic".into()),
        });
        let mut anthropic = ProviderConfig::default();
        anthropic.model = Some("claude-haiku-4-5".into());
        cfg.providers.insert("anthropic".into(), anthropic);
        cfg.cli = Some(CliConfig {
            output_format: Some("table".into()),
            interactive: Some(true),
            history_size: Some(1000),
            color: Some(true),
        });
        cfg.save()?;
        Ok(cfg)
    }

    /// Get a flat key value. Key format: "section.field" (e.g. "core.default_provider").
    pub fn get_key(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.splitn(2, '.').collect();
        match parts.as_slice() {
            ["core", field] => {
                let core = self.core.as_ref()?;
                match *field {
                    "working_dir" => core.working_dir.clone(),
                    "default_provider" => core.default_provider.clone(),
                    _ => None,
                }
            }
            ["cli", field] => {
                let cli = self.cli.as_ref()?;
                match *field {
                    "output_format" => cli.output_format.clone(),
                    "interactive" => cli.interactive.map(|v| v.to_string()),
                    "history_size" => cli.history_size.map(|v| v.to_string()),
                    "color" => cli.color.map(|v| v.to_string()),
                    _ => None,
                }
            }
            ["worker", field] => {
                let w = self.worker.as_ref()?;
                match *field {
                    "worker_id" => w.worker_id.clone(),
                    "worker_type" => w.worker_type.clone(),
                    "heartbeat_interval" => w.heartbeat_interval.map(|v| v.to_string()),
                    _ => None,
                }
            }
            ["providers", rest] => {
                let parts2: Vec<&str> = rest.splitn(2, '.').collect();
                if parts2.len() == 2 {
                    let provider = self.providers.get(parts2[0])?;
                    match parts2[1] {
                        "api_key" => provider.api_key.clone(),
                        "model" => provider.model.clone(),
                        "base_url" => provider.base_url.clone(),
                        "enabled" => provider.enabled.map(|v| v.to_string()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Set a flat key. Key format: "section.field".
    pub fn set_key(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.splitn(2, '.').collect();
        match parts.as_slice() {
            ["core", field] => {
                let core = self.core.get_or_insert_with(Default::default);
                match *field {
                    "working_dir" => core.working_dir = Some(value.into()),
                    "default_provider" => core.default_provider = Some(value.into()),
                    k => return Err(SharedError::Other(format!("Unknown key: core.{k}"))),
                }
            }
            ["cli", field] => {
                let cli = self.cli.get_or_insert_with(Default::default);
                match *field {
                    "output_format" => cli.output_format = Some(value.into()),
                    "interactive" => cli.interactive = Some(value.parse().unwrap_or(true)),
                    "history_size" => cli.history_size = Some(value.parse().unwrap_or(1000)),
                    "color" => cli.color = Some(value.parse().unwrap_or(true)),
                    k => return Err(SharedError::Other(format!("Unknown key: cli.{k}"))),
                }
            }
            ["worker", field] => {
                let w = self.worker.get_or_insert_with(Default::default);
                match *field {
                    "worker_id" => w.worker_id = Some(value.into()),
                    "worker_type" => w.worker_type = Some(value.into()),
                    "heartbeat_interval" => {
                        w.heartbeat_interval = Some(value.parse().unwrap_or(300))
                    }
                    k => return Err(SharedError::Other(format!("Unknown key: worker.{k}"))),
                }
            }
            _ => return Err(SharedError::Other(format!("Unknown config key: {key}"))),
        }
        Ok(())
    }

    /// Flatten all config keys to a vec of (key, value) pairs.
    pub fn list_all(&self) -> Vec<(String, String)> {
        let mut pairs = vec![];
        if let Some(c) = &self.core {
            if let Some(v) = &c.working_dir {
                pairs.push(("core.working_dir".into(), v.clone()));
            }
            if let Some(v) = &c.default_provider {
                pairs.push(("core.default_provider".into(), v.clone()));
            }
        }
        if let Some(c) = &self.cli {
            if let Some(v) = &c.output_format {
                pairs.push(("cli.output_format".into(), v.clone()));
            }
            if let Some(v) = c.interactive {
                pairs.push(("cli.interactive".into(), v.to_string()));
            }
            if let Some(v) = c.history_size {
                pairs.push(("cli.history_size".into(), v.to_string()));
            }
            if let Some(v) = c.color {
                pairs.push(("cli.color".into(), v.to_string()));
            }
        }
        if let Some(w) = &self.worker {
            if let Some(v) = &w.worker_id {
                pairs.push(("worker.worker_id".into(), v.clone()));
            }
            if let Some(v) = &w.worker_type {
                pairs.push(("worker.worker_type".into(), v.clone()));
            }
            if let Some(v) = w.heartbeat_interval {
                pairs.push(("worker.heartbeat_interval".into(), v.to_string()));
            }
        }
        for (name, p) in &self.providers {
            if let Some(v) = &p.model {
                pairs.push((format!("providers.{name}.model"), v.clone()));
            }
            if let Some(v) = &p.base_url {
                pairs.push((format!("providers.{name}.base_url"), v.clone()));
            }
            if let Some(v) = p.enabled {
                pairs.push((format!("providers.{name}.enabled"), v.to_string()));
            }
            // never emit api_key — security
        }
        pairs
    }

    pub fn output_format(&self) -> String {
        self.cli
            .as_ref()
            .and_then(|c| c.output_format.clone())
            .unwrap_or_else(|| "table".into())
    }
}
