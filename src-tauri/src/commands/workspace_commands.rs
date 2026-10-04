// Workspace directory commands — get/set/pick the active workspace folder.
// The workspace dir is the root folder for ALL Arcana agents:
//   - PA chat sessions default their working_dir to workspace_dir
//   - Workers + Supervisor run their recipes against workspace_dir
//   - @-mentions in chat are resolved relative to workspace_dir
//
// Stored as a plain path string in ~/.arcana/config.db (app_settings table).
// See: DEVELOPMENT_PLAN.md — Workspace Folder feature

use crate::errors::Result;

// ─── Storage helpers ──────────────────────────────────────────────────────────

// `config.db` is opened through `db::app_settings`, the single owner of the
// `app_settings` table (P1-15). This file used to declare its own
// `CREATE TABLE IF NOT EXISTS` — one of three that disagreed about whether
// the `updated_at` trigger was part of the schema, so first caller won.

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Return the currently-configured workspace directory.
/// Returns an empty string if none is set.
#[tauri::command]
pub fn get_workspace_dir() -> Result<String> {
    Ok(crate::db::app_settings::get(crate::db::app_settings::WORKSPACE_KEY)?.unwrap_or_default())
}

/// Persist the workspace directory path.
/// Pass an empty string to clear the workspace.
#[tauri::command]
pub fn set_workspace_dir(path: String) -> Result<()> {
    if path.is_empty() {
        crate::db::app_settings::delete(crate::db::app_settings::WORKSPACE_KEY)?;
    } else {
        let resolved = arcana_core::platform::workspace::resolve_workspace_dir(Some(&path))?;
        let canonical_path = resolved.to_string_lossy();
        crate::db::app_settings::set(
            crate::db::app_settings::WORKSPACE_KEY,
            canonical_path.as_ref(),
        )?;
    }
    Ok(())
}

/// Open a native OS folder-picker dialog and return the selected path.
/// Returns None if the user cancelled.
#[tauri::command]
pub async fn pick_workspace_dir() -> Result<Option<String>> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Select Arcana Workspace Folder")
        .pick_folder()
        .await;

    Ok(handle.map(|f| f.path().to_string_lossy().into_owned()))
}

// ─── Compatibility helper for UI display/search consumers ────────────────────

/// The configured workspace directory, or an empty string when unset.
///
/// Delegates to `arcana_core::db::app_settings`, the single owner of
/// `config.db`'s `app_settings` table. This function used to hold its own copy
/// of the query, which is how a database accessor came to live in the command
/// layer and put `recipes/` behind `commands/` (ADR-LOG-RESTRUCT-0015). Agent
/// execution must use the strict platform workspace resolver instead.
pub fn load_workspace_dir() -> String {
    arcana_core::db::app_settings::load_workspace_dir()
}
