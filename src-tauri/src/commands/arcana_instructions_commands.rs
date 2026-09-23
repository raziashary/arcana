// ARCANA.md commands — Tauri commands for the SQLite-backed ARCANA.md instruction store.
// Scope: "global" (no repo_path) or "repo" (absolute path required).
// See: DEVELOPMENT_PLAN.md Phase 2.1.2 — ARCANA.md System

use crate::db::arcana_instructions::ArcanaInstructionsDb;
use crate::errors::Result;

/// Get ARCANA.md content for the given scope.
/// Returns empty string if no entry exists.
#[tauri::command]
pub fn get_arcana_instructions(scope: String, repo_path: Option<String>) -> Result<String> {
    let db = ArcanaInstructionsDb::open()?;
    db.get(&scope, repo_path.as_deref())
}

/// Set (insert or update) ARCANA.md content.
/// Enforces 40,000 character limit.
#[tauri::command]
pub fn set_arcana_instructions(
    scope: String,
    repo_path: Option<String>,
    content: String,
) -> Result<()> {
    let db = ArcanaInstructionsDb::open()?;
    db.set(&scope, repo_path.as_deref(), &content)
}

/// Soft-delete repo-scoped instructions (marks is_active=0).
#[tauri::command]
pub fn delete_arcana_instructions(repo_path: String) -> Result<()> {
    let db = ArcanaInstructionsDb::open()?;
    db.delete_repo(&repo_path)
}

/// Check if instructions exist for a scope.
#[tauri::command]
pub fn check_arcana_instructions_exists(scope: String, repo_path: Option<String>) -> Result<bool> {
    let db = ArcanaInstructionsDb::open()?;
    db.exists(&scope, repo_path.as_deref())
}

/// Get character count for a scope (for live counter in editor UI).
#[tauri::command]
pub fn get_arcana_instructions_char_count(
    scope: String,
    repo_path: Option<String>,
) -> Result<usize> {
    let db = ArcanaInstructionsDb::open()?;
    db.char_count(&scope, repo_path.as_deref())
}
