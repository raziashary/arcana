// Database module — houses all SQLite repositories beyond the session store.
// Each sub-module manages its own connection and migration.
// Databases:
//   ~/.autodev/tasks.db            — Internal task board (Supervisor ↔ Workers)
//   ~/.autodev/config.db           — AUTODEV.md instructions + per-model configs
//   ~/.autodev/workers.db          — Worker registry (Phase 2.2)
//
// Session data lives in the existing ~/.airs/autodev.db (see session/storage.rs).

pub mod annotations;
pub mod autodev_instructions;
pub mod backup;
pub mod model_configs;
pub mod plans;
pub mod tasks;
pub mod workers;

use crate::errors::{AutoDevError, Result};
use std::path::PathBuf;

/// Returns the ~/.autodev directory, creating it if needed.
pub fn autodev_dir() -> Result<PathBuf> {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| AutoDevError::Agent("Cannot determine home directory".into()))?;
    let dir = base.join(".autodev");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
