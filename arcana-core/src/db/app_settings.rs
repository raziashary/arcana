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
    let mut timing = crate::observability::timing::OperationTimer::start("config_setting_read")
        .with_database("config");
    let result = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| ArcanaError::Agent(e.to_string()));
    match &result {
        Ok(Some(_)) => {
            timing = timing.with_result_count(1);
            timing.succeed();
        }
        Ok(None) => {
            timing = timing.with_result_count(0);
            timing.succeed();
        }
        Err(_) => {}
    }
    result
}

/// Write one setting, replacing any existing value.
pub fn set(key: &str, value: &str) -> Result<()> {
    set_in(&open()?, key, value)
}

/// Delete one setting. `false` means the key was already absent.
pub fn delete(key: &str) -> Result<bool> {
    delete_in(&open()?, key)
}

/// As `delete`, on a caller-supplied connection.
pub fn delete_in(conn: &Connection, key: &str) -> Result<bool> {
    let mut timing = crate::observability::timing::OperationTimer::start("config_setting_delete")
        .with_database("config");
    let result = conn
        .execute("DELETE FROM app_settings WHERE key = ?1", params![key])
        .map(|rows| rows != 0)
        .map_err(|e| ArcanaError::Agent(e.to_string()));
    if let Ok(deleted) = &result {
        timing = timing.with_result_count(usize::from(*deleted));
        timing.succeed();
    }
    result
}

/// As `set`, on a caller-supplied connection.
pub fn set_in(conn: &Connection, key: &str, value: &str) -> Result<()> {
    let mut timing = crate::observability::timing::OperationTimer::start("config_setting_write")
        .with_database("config");
    let updated_at = crate::db::timestamps::now_rfc3339();
    conn.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, updated_at],
    )
    .map_err(|e| ArcanaError::Agent(e.to_string()))?;
    timing = timing.with_result_count(1);
    timing.succeed();
    Ok(())
}

/// Compatibility accessor for UI display/search code.
///
/// Returns an empty string when unset or unreadable. Agent execution paths
/// must use `platform::workspace::resolve_workspace_dir` so database errors
/// and missing projects are reported instead of becoming a directory fallback.
pub fn load_workspace_dir() -> String {
    get(WORKSPACE_KEY).ok().flatten().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "arcana-appsettings-test-{tag}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&dir).expect("create unique isolated test directory");
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

    #[test]
    fn deleting_a_setting_reports_whether_a_row_was_removed() {
        let path = temp_db("delete");
        let conn = open_at(&path).expect("open");
        set_in(&conn, WORKSPACE_KEY, "C:/work").expect("write setting");
        assert!(delete_in(&conn, WORKSPACE_KEY).expect("delete setting"));
        assert_eq!(get_in(&conn, WORKSPACE_KEY).unwrap(), None);
        assert!(!delete_in(&conn, WORKSPACE_KEY).expect("repeat delete"));
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// The trigger is part of the schema because the live database has it.
    /// Asserting it exists stops a future edit from quietly dropping it and
    /// re-creating the drift this task removed.
    #[test]
    fn the_updated_at_trigger_is_created_and_fires() {
        let path = temp_db("trigger");
        let _home = crate::db::test_support::with_home(path.parent().unwrap());
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
        assert_eq!(first.len(), 24, "expected fixed millisecond UTC: {first}");
        assert!(first.ends_with('Z'), "expected UTC Z suffix: {first}");
        assert!(chrono::DateTime::parse_from_rfc3339(&first).is_ok());

        set_in(&conn, "k", "v2").expect("update");
        let updated: String = conn
            .query_row(
                "SELECT updated_at FROM app_settings WHERE key='k'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            updated.len(),
            24,
            "expected fixed millisecond UTC: {updated}"
        );
        assert!(updated.ends_with('Z'), "expected UTC Z suffix: {updated}");

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

    #[test]
    fn setting_operations_record_results_without_setting_keys_or_values() {
        let path = temp_db("telemetry");
        let conn = open_at(&path).expect("open");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            set_in(&conn, "private-setting-key", "private-setting-value").expect("write");
            assert_eq!(
                get_in(&conn, "private-setting-key")
                    .expect("read")
                    .as_deref(),
                Some("private-setting-value")
            );
            assert!(delete_in(&conn, "private-setting-key").expect("delete"));
            assert_eq!(
                get_in(&conn, "missing-setting").expect("missing read"),
                None
            );
        });

        let _ = result;
        let operation_events = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect::<Vec<_>>();
        assert_eq!(operation_events.len(), 4);
        for event in &operation_events {
            assert_eq!(event["fields"]["database"], "config");
            assert_eq!(event["fields"]["outcome"], "success");
            assert_eq!(event["fields"]["result_count_observed"], true);
        }
        assert!(operation_events
            .iter()
            .any(
                |event| event["fields"]["operation"] == "config_setting_read"
                    && event["fields"]["result_count"] == 0
            ));
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("private-setting-key"));
        assert!(!serialized.contains("private-setting-value"));
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn setting_write_failure_records_error_without_echoing_input() {
        let path = temp_db("telemetry-failure");
        let conn = open_at(&path).expect("open");
        conn.execute_batch(
            "CREATE TRIGGER reject_settings BEFORE INSERT ON app_settings
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .expect("install failure trigger");
        let (result, events) = crate::observability::timing::capture_json_events(|| {
            set_in(&conn, "private-setting-key", "private-setting-value")
        });

        assert!(result.is_err());
        let event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "config_setting_write")
            .expect("failed setting write event");
        assert_eq!(event["fields"]["outcome"], "error");
        assert_eq!(event["fields"]["result_count_observed"], false);
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains("private-setting-key"));
        assert!(!serialized.contains("private-setting-value"));
        assert!(!serialized.contains("injected failure"));
        drop(conn);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
