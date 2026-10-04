// IDE workspace state — tab persistence + expansion state.
//
// Stored in ~/.arcana/config.db under two keys in `app_settings`:
//   ide_open_tabs       → JSON array of relative paths currently open
//   ide_expanded_dirs   → JSON array of relative paths that are expanded
//   ide_active_tab      → single relative path of the focused tab
//
// Multi-root workspaces are deferred to Phase 4 per DEVELOPMENT_PLAN.md;
// today the workspace dir is always a single root read from config.db.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::errors::{ArcanaError, Result};

const KEY_OPEN_TABS: &str = "ide_open_tabs";
const KEY_ACTIVE_TAB: &str = "ide_active_tab";
const KEY_EXPANDED_DIRS: &str = "ide_expanded_dirs";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdeWorkspaceState {
    pub open_tabs: Vec<String>,
    pub active_tab: Option<String>,
    pub expanded_dirs: Vec<String>,
}

// ─── Internal DB helpers ──────────────────────────────────────────────────────

// `config.db` is opened through `db::app_settings`, the single owner of the
// `app_settings` table (P1-15). This file used to declare its own
// `CREATE TABLE IF NOT EXISTS` — one of three that disagreed about whether
// the `updated_at` trigger was part of the schema, so first caller won.

fn read_key(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .map_err(|e| ArcanaError::Agent(format!("read {key}: {e}")))
}

fn write_key(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| ArcanaError::Agent(format!("write {key}: {e}")))?;
    Ok(())
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn load() -> Result<IdeWorkspaceState> {
    let operation = crate::observability::timing::OperationTimer::start("ide_workspace_state_load")
        .with_database("config");
    let conn = crate::db::app_settings::open()?;

    let mut degraded = false;
    let open_tabs: Vec<String> = match read_key(&conn, KEY_OPEN_TABS)? {
        Some(serialized) => match serde_json::from_str(&serialized) {
            Ok(paths) => paths,
            Err(_) => {
                degraded = true;
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let expanded_dirs: Vec<String> = match read_key(&conn, KEY_EXPANDED_DIRS)? {
        Some(serialized) => match serde_json::from_str(&serialized) {
            Ok(paths) => paths,
            Err(_) => {
                degraded = true;
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let active_tab: Option<String> = read_key(&conn, KEY_ACTIVE_TAB)?;

    let state = IdeWorkspaceState {
        open_tabs,
        active_tab,
        expanded_dirs,
    };
    let count = state.open_tabs.len() + state.expanded_dirs.len();
    let mut operation = operation.with_result_count(count);
    if degraded {
        operation.set_outcome("fallback");
    } else if count == 0 {
        operation.set_outcome("empty");
    } else {
        operation.succeed();
    }
    Ok(state)
}

pub fn save(state: &IdeWorkspaceState) -> Result<()> {
    let operation = crate::observability::timing::OperationTimer::start("ide_workspace_state_save")
        .with_database("config");
    let conn = crate::db::app_settings::open()?;

    let tabs_json = serde_json::to_string(&state.open_tabs)
        .map_err(|e| ArcanaError::Agent(format!("serialize tabs: {e}")))?;
    let expanded_json = serde_json::to_string(&state.expanded_dirs)
        .map_err(|e| ArcanaError::Agent(format!("serialize expanded: {e}")))?;

    write_key(&conn, KEY_OPEN_TABS, &tabs_json)?;
    write_key(&conn, KEY_EXPANDED_DIRS, &expanded_json)?;

    if let Some(active) = &state.active_tab {
        write_key(&conn, KEY_ACTIVE_TAB, active)?;
    } else {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            params![KEY_ACTIVE_TAB],
        )
        .map_err(|e| ArcanaError::Agent(format!("clear active_tab: {e}")))?;
    }
    operation
        .with_result_count(2 + usize::from(state.active_tab.is_some()))
        .succeed();
    Ok(())
}

#[cfg(test)]
mod observability_tests {
    use super::*;

    #[test]
    fn workspace_state_operations_emit_safe_result_spans() {
        let sandbox = tempfile::tempdir().expect("isolated home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let state = IdeWorkspaceState {
            open_tabs: vec!["private-open-tab-path-marker".into()],
            active_tab: Some("private-active-tab-path-marker".into()),
            expanded_dirs: vec!["private-expanded-directory-marker".into()],
        };

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let empty = load().expect("read initial empty workspace");
            save(&state).expect("persist workspace state");
            let restored = load().expect("read persisted workspace state");
            let conn = crate::db::app_settings::open().expect("open isolated settings DB");
            crate::db::app_settings::set_in(&conn, KEY_OPEN_TABS, "malformed-json-marker")
                .expect("inject malformed saved tab JSON");
            let fallback = load().expect("malformed JSON degrades to empty tabs");
            conn.execute_batch(
                "ALTER TABLE app_settings RENAME TO app_settings_valid;
                 CREATE TABLE app_settings (key TEXT);",
            )
            .expect("install malformed isolated settings schema");
            let failed = load().is_err();
            (empty, restored, fallback, failed)
        });

        assert!(observed.0.open_tabs.is_empty());
        assert_eq!(observed.1.open_tabs, state.open_tabs);
        assert_eq!(observed.1.active_tab, state.active_tab);
        assert_eq!(observed.1.expanded_dirs, state.expanded_dirs);
        assert!(observed.2.open_tabs.is_empty());
        assert!(observed.3);
        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "ide_workspace_state_load"
                && event["fields"]["outcome"] == "empty"
                && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "ide_workspace_state_load"
                && event["fields"]["outcome"] == "fallback"
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "ide_workspace_state_load"
                && event["fields"]["outcome"] == "error"
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "ide_workspace_state_save"
                && event["fields"]["outcome"] == "success"
                && event["fields"]["result_count"] == 3
        }));
        let serialized = serde_json::to_string(&operations).expect("serialize workspace spans");
        for private_value in [
            "private-open-tab-path-marker",
            "private-active-tab-path-marker",
            "private-expanded-directory-marker",
            "malformed-json-marker",
            sandbox.path().to_string_lossy().as_ref(),
        ] {
            assert!(
                !serialized.contains(private_value),
                "workspace telemetry leaked {private_value}"
            );
        }
    }
}
