// IDE workspace state — tab persistence + expansion state.
//
// Stored in ~/.autodev/config.db under two keys in `app_settings`:
//   ide_open_tabs       → JSON array of relative paths currently open
//   ide_expanded_dirs   → JSON array of relative paths that are expanded
//   ide_active_tab      → single relative path of the focused tab
//
// Multi-root workspaces are deferred to Phase 4 per DEVELOPMENT_PLAN.md;
// today the workspace dir is always a single root read from config.db.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};

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
    .map_err(|e| AutoDevError::Agent(format!("read {key}: {e}")))
}

fn write_key(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| AutoDevError::Agent(format!("write {key}: {e}")))?;
    Ok(())
}

// ─── Public API ───────────────────────────────────────────────────────────────

pub fn load() -> Result<IdeWorkspaceState> {
    let conn = crate::db::app_settings::open()?;

    let open_tabs: Vec<String> = read_key(&conn, KEY_OPEN_TABS)?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let expanded_dirs: Vec<String> = read_key(&conn, KEY_EXPANDED_DIRS)?
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let active_tab: Option<String> = read_key(&conn, KEY_ACTIVE_TAB)?;

    Ok(IdeWorkspaceState {
        open_tabs,
        active_tab,
        expanded_dirs,
    })
}

pub fn save(state: &IdeWorkspaceState) -> Result<()> {
    let conn = crate::db::app_settings::open()?;

    let tabs_json = serde_json::to_string(&state.open_tabs)
        .map_err(|e| AutoDevError::Agent(format!("serialize tabs: {e}")))?;
    let expanded_json = serde_json::to_string(&state.expanded_dirs)
        .map_err(|e| AutoDevError::Agent(format!("serialize expanded: {e}")))?;

    write_key(&conn, KEY_OPEN_TABS, &tabs_json)?;
    write_key(&conn, KEY_EXPANDED_DIRS, &expanded_json)?;

    if let Some(active) = &state.active_tab {
        write_key(&conn, KEY_ACTIVE_TAB, active)?;
    } else {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            params![KEY_ACTIVE_TAB],
        )
        .map_err(|e| AutoDevError::Agent(format!("clear active_tab: {e}")))?;
    }
    Ok(())
}
