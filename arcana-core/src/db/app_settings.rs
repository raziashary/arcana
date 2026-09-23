// `~/.arcana/config.db` → `app_settings`: the key/value store behind workspace
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

use crate::db::arcana_dir;
use crate::errors::{ArcanaError, Result};

/// The workspace-directory setting key.
pub const WORKSPACE_KEY: &str = "workspace_dir";

/// Open `config.db`. The schema — this table, the two others the file
/// holds, and the ladder — belongs to `db::config_db`, which owns the file.
pub fn open() -> Result<Connection> {
    open_at(&arcana_dir()?.join("config.db"))
}

/// As `open`, against an explicit path. Exists so tests never write to the
/// user's real `~/.arcana/config.db`.
pub fn open_at(path: &std::path::Path) -> Result<Connection> {
    crate::db::config_db::open_at(path)
}

/// Read one setting. `None` when unset.
pub fn get(key: &str) -> Result<Option<String>> {
    get_in(&open()?, key)
}

/// As `get`, on a caller-supplied connection.
pub fn get_in(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| ArcanaError::Agent(e.to_string()))
}

/// Write one setting, replacing any existing value.
pub fn set(key: &str, value: &str) -> Result<()> {
    set_in(&open()?, key, value)
}

/// As `set`, on a caller-supplied connection.
pub fn set_in(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        params![key, value],
    )
    .map_err(|e| ArcanaError::Agent(e.to_string()))?;
    Ok(())
}

/// The configured workspace directory, or an empty string when unset.
///
/// Never fails: callers treat "no workspace configured" and "config.db is
/// unreadable" identically — both mean "fall back to the process directory".
pub fn load_workspace_dir() -> String {
    get(WORKSPACE_KEY).ok().flatten().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("arcana-appsettings-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir.join("config.db")
    }

    #[test]
    fn a_value_written_is_a_value_read_back() {
        let path = temp_db("roundtrip");
        let conn = open_at(&path).expect("open");
        assert_eq!(get_in(&conn, WORKSPACE_KEY).unwrap(), None);

        set_in(&conn, WORKSPACE_KEY, "C:/work").expect("set");
        assert_eq!(
            get_in(&conn, WORKSPACE_KEY).unwrap(),
            Some("C:/work".to_string())
        );

        // Upsert, not a second row.
        set_in(&conn, WORKSPACE_KEY, "C:/other").expect("set again");
        assert_eq!(
            get_in(&conn, WORKSPACE_KEY).unwrap(),
            Some("C:/other".to_string())
        );
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_settings", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "set must upsert, not insert a duplicate");

        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The trigger is part of the schema because the live database has it.
    /// Asserting it exists stops a future edit from quietly dropping it and
    /// re-creating the drift this task removed.
    #[test]
    fn the_updated_at_trigger_is_created_and_fires() {
        let path = temp_db("trigger");
        let conn = open_at(&path).expect("open");

        let present: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='trigger' AND name='update_app_settings_ts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(present, 1, "the updated_at trigger is missing");

        set_in(&conn, "k", "v1").expect("insert");
        let first: String = conn
            .query_row(
                "SELECT updated_at FROM app_settings WHERE key='k'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!first.is_empty(), "updated_at was never populated");

        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_unset_workspace_reads_as_empty_rather_than_failing() {
        // `load_workspace_dir` is called on paths that must not fail; callers
        // treat "unset" and "unreadable" identically.
        let path = temp_db("unset");
        let conn = open_at(&path).expect("open");
        assert_eq!(get_in(&conn, WORKSPACE_KEY).unwrap(), None);
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
