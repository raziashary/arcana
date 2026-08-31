// Workspace directory commands — get/set/pick the active workspace folder.
// The workspace dir is the root folder for ALL AutoDev agents:
//   - PA chat sessions default their working_dir to workspace_dir
//   - Workers + Supervisor run their recipes against workspace_dir
//   - @-mentions in chat are resolved relative to workspace_dir
//
// Stored as a plain path string in ~/.autodev/config.db (app_settings table).
// See: DEVELOPMENT_PLAN.md — Workspace Folder feature

use crate::db::autodev_dir;
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};

const WORKSPACE_KEY: &str = "workspace_dir";

// ─── Storage helpers ──────────────────────────────────────────────────────────

fn open_config_db() -> Result<Connection> {
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

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Return the currently-configured workspace directory.
/// Returns an empty string if none is set.
#[tauri::command]
pub fn get_workspace_dir() -> Result<String> {
    let conn = open_config_db()?;
    let row: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![WORKSPACE_KEY],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| AutoDevError::Agent(format!("get_workspace_dir: {e}")))?;

    Ok(row.unwrap_or_default())
}

/// Persist the workspace directory path.
/// Pass an empty string to clear the workspace.
#[tauri::command]
pub fn set_workspace_dir(path: String) -> Result<()> {
    let conn = open_config_db()?;
    if path.is_empty() {
        conn.execute(
            "DELETE FROM app_settings WHERE key = ?1",
            params![WORKSPACE_KEY],
        )
        .map_err(|e| AutoDevError::Agent(format!("clear_workspace_dir: {e}")))?;
    } else {
        conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![WORKSPACE_KEY, &path],
        )
        .map_err(|e| AutoDevError::Agent(format!("set_workspace_dir: {e}")))?;
    }
    Ok(())
}

/// Open a native OS folder-picker dialog and return the selected path.
/// Returns None if the user cancelled.
#[tauri::command]
pub async fn pick_workspace_dir() -> Result<Option<String>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Select AutoDev Workspace Folder")
        .pick_folder()
        .await;

    Ok(handle.map(|f| f.path().to_string_lossy().into_owned()))
}

// ─── Internal helper (used by RecipeRunner + agent_commands) ──────────────────

/// Load workspace_dir from config.db synchronously.
/// Returns empty string if not set. Never fails.
pub fn load_workspace_dir() -> String {
    open_config_db()
        .and_then(|conn| {
            conn.query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![WORKSPACE_KEY],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| AutoDevError::Agent(e.to_string()))
        })
        .ok()
        .flatten()
        .unwrap_or_default()
}
