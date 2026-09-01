pub mod analytics;
pub mod plans;
pub mod tasks;
pub mod workers;

use crate::errors::{Result, SharedError};
use std::path::PathBuf;

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
