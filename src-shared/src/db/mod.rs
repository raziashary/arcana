pub mod tasks;
pub mod workers;
pub mod plans;
pub mod analytics;

use std::path::PathBuf;
use crate::errors::{Result, SharedError};

/// Returns `~/.autodev/`, creating it if needed.
pub fn autodev_dir() -> Result<PathBuf> {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| SharedError::Other("Cannot determine home directory".into()))?;
    let dir = base.join(".autodev");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
