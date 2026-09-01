// `~/.autodev/config.db` → `app_settings`: the key/value store behind workspace
// selection and assorted UI preferences.
//
// Why this module exists: `load_workspace_dir` used to live in
// `commands/workspace_commands.rs`, where its own comment described it as an
// "internal helper (used by RecipeRunner + agent_commands)" — a database
// accessor that was never a command, sitting in the command layer. That
// placement put `recipes/` behind `commands/`, one of the crate cycles that
// made the core extraction impossible (ADR-LOG-RESTRUCT-0015).
//
// ⚠ `app_settings` currently has three independent creators (P1-15). This
// module is the intended single owner; the remaining two are repointed here in
// that task, which also settles whether the `updated_at` trigger — created
// today by `github_mcp_commands.rs` alone, so its existence depends on which
// caller ran first — is part of the schema.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};

/// The workspace-directory setting key.
pub const WORKSPACE_KEY: &str = "workspace_dir";

/// Open `config.db` and ensure `app_settings` exists.
pub fn open() -> Result<Connection> {
    let path = autodev_dir()?.join("config.db");
    let conn =
        Connection::open(&path).map_err(|e| AutoDevError::Agent(format!("config.db open: {e}")))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
             key        TEXT PRIMARY KEY,
             value      TEXT NOT NULL,
             updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );",
    )
    .map_err(|e| AutoDevError::Agent(format!("app_settings migrate: {e}")))?;
    Ok(conn)
}

/// Read one setting. `None` when unset.
pub fn get(key: &str) -> Result<Option<String>> {
    let conn = open()?;
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| AutoDevError::Agent(e.to_string()))
}

/// Write one setting, replacing any existing value.
pub fn set(key: &str, value: &str) -> Result<()> {
    let conn = open()?;
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| AutoDevError::Agent(e.to_string()))?;
    Ok(())
}

/// The configured workspace directory, or an empty string when unset.
///
/// Never fails: callers treat "no workspace configured" and "config.db is
/// unreadable" identically — both mean "fall back to the process directory".
pub fn load_workspace_dir() -> String {
    get(WORKSPACE_KEY).ok().flatten().unwrap_or_default()
}
