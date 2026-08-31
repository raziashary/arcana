// AUTODEV.md commands — Tauri commands for the SQLite-backed AUTODEV.md instruction store.
// Scope: "global" (no repo_path) or "repo" (absolute path required).
// See: DEVELOPMENT_PLAN.md Phase 2.1.2 — AUTODEV.md System

use crate::db::autodev_instructions::AutodevInstructionsDb;
use crate::errors::Result;

/// Get AUTODEV.md content for the given scope.
/// Returns empty string if no entry exists.
#[tauri::command]
pub fn get_autodev_instructions(scope: String, repo_path: Option<String>) -> Result<String> {
    let db = AutodevInstructionsDb::open()?;
    db.get(&scope, repo_path.as_deref())
}

/// Set (insert or update) AUTODEV.md content.
/// Enforces 40,000 character limit.
#[tauri::command]
pub fn set_autodev_instructions(
    scope: String,
    repo_path: Option<String>,
    content: String,
) -> Result<()> {
    let db = AutodevInstructionsDb::open()?;
    db.set(&scope, repo_path.as_deref(), &content)
}

/// Soft-delete repo-scoped instructions (marks is_active=0).
#[tauri::command]
pub fn delete_autodev_instructions(repo_path: String) -> Result<()> {
    let db = AutodevInstructionsDb::open()?;
    db.delete_repo(&repo_path)
}

/// Check if instructions exist for a scope.
#[tauri::command]
pub fn check_autodev_instructions_exists(scope: String, repo_path: Option<String>) -> Result<bool> {
    let db = AutodevInstructionsDb::open()?;
    db.exists(&scope, repo_path.as_deref())
}

/// Get character count for a scope (for live counter in editor UI).
#[tauri::command]
pub fn get_autodev_instructions_char_count(
    scope: String,
    repo_path: Option<String>,
) -> Result<usize> {
    let db = AutodevInstructionsDb::open()?;
    db.char_count(&scope, repo_path.as_deref())
}
