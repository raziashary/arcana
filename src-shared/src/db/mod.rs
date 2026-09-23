// `tasks`, `workers` and `plans` were hand-maintained mirrors of tables that
// `src-tauri` owns and writes. They are gone (P1-09): `arcanactl` now reads and
// writes those tables through `arcana_core::db`, the single owner.
//
// The mirrors were not a theoretical risk. `workers.rs` selected `last_seen`
// and `notes`; the live `~/.arcana/workers.db` written by the app has neither,
// so its own `CREATE TABLE IF NOT EXISTS` was a silent no-op against the
// already-existing table and every `arcanactl worker list/status/heartbeat` call
// failed with *no such column: last_seen*.
//
// `analytics` stays. It is a pure reader — it holds no `CREATE`, `INSERT`,
// `UPDATE` or `DELETE` — so it cannot drift the schema, and it is not one of
// the four tables P1-09 names. Consolidating it means porting aggregation
// semantics that decide the numbers users are shown, which is its own task.
pub mod analytics;

use crate::errors::{Result, SharedError};
use std::path::PathBuf;

/// Returns `~/.arcana/`, creating it if needed.
pub fn arcana_dir() -> Result<PathBuf> {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| SharedError::Other("Cannot determine home directory".into()))?;
    let dir = base.join(".arcana");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
