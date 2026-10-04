// IDE Tauri commands — thin wrappers around the `ide/` module.
// All commands resolve against the configured workspace_dir (see
// workspace_commands::load_workspace_dir).

use crate::errors::{ArcanaError, Result};
use crate::ide::{
    file_reader, file_search, file_tree, symbol_nav, symbols,
    types::{
        ContentSearchResult, FileContent, FileMetadata, FileSearchResult, FileTreeNode,
        SymbolDefinition, SymbolReference, SymbolTree,
    },
    workspace::{self, IdeWorkspaceState},
};

// ─── Helper ───────────────────────────────────────────────────────────────────

fn require_workspace() -> Result<String> {
    arcana_core::platform::workspace::resolve_workspace_dir(None)
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| ArcanaError::Agent(format!("Workspace is unavailable: {error}")))
}

// ─── File Tree ────────────────────────────────────────────────────────────────

/// Read the file tree at `path` (defaults to workspace root when empty).
/// `depth` caps recursion: 0 = only immediate children, 1 = two levels, …
#[tauri::command]
pub fn read_file_tree(
    path: Option<String>,
    depth: Option<usize>,
    ignore_patterns: Option<Vec<String>>,
) -> Result<FileTreeNode> {
    let workspace = require_workspace()?;
    let target = path.unwrap_or_default();
    let root = if target.is_empty() {
        workspace.clone()
    } else {
        target
    };
    let depth = depth.unwrap_or(1);
    let ignore = ignore_patterns.unwrap_or_default();

    file_tree::read_tree(&workspace, &root, depth, &ignore)
}

// ─── File Content ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_file_content(
    path: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<FileContent> {
    let workspace = require_workspace()?;
    file_reader::read_content(&workspace, &path, start_line, end_line)
}

#[tauri::command]
pub fn get_file_metadata(path: String) -> Result<FileMetadata> {
    let workspace = require_workspace()?;
    file_reader::read_metadata(&workspace, &path)
}

// ─── Search ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_files(
    query: String,
    path: Option<String>,
    glob: Option<String>,
    max_results: Option<usize>,
) -> Result<Vec<FileSearchResult>> {
    let workspace = require_workspace()?;
    file_search::search_files(
        &workspace,
        &query,
        path.as_deref(),
        glob.as_deref(),
        max_results.unwrap_or(50),
    )
}

#[tauri::command]
pub fn search_file_contents(
    query: String,
    path: Option<String>,
    max_results: Option<usize>,
) -> Result<Vec<ContentSearchResult>> {
    let workspace = require_workspace()?;
    file_search::search_contents(
        &workspace,
        &query,
        path.as_deref(),
        max_results.unwrap_or(100),
    )
}

// ─── Symbols ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_symbol_tree(path: String) -> Result<SymbolTree> {
    let workspace = require_workspace()?;
    symbols::extract(&workspace, &path)
}

// ─── File Mutations ───────────────────────────────────────────────────────────

/// Create a new file at `path` (relative to workspace) with optional initial content.
#[tauri::command]
pub fn create_file(path: String, content: Option<String>) -> Result<()> {
    let workspace = require_workspace()?;
    let abs = resolve_safe(&workspace, &path)?;
    if let Some(parent) = abs.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ArcanaError::Agent(format!("mkdir: {e}")))?;
    }
    std::fs::write(&abs, content.unwrap_or_default())
        .map_err(|e| ArcanaError::Agent(format!("create file: {e}")))?;
    Ok(())
}

/// Create a new directory at `path` (relative to workspace).
#[tauri::command]
pub fn create_directory(path: String) -> Result<()> {
    let workspace = require_workspace()?;
    let abs = resolve_safe(&workspace, &path)?;
    std::fs::create_dir_all(&abs).map_err(|e| ArcanaError::Agent(format!("mkdir: {e}")))?;
    Ok(())
}

/// Delete the file (or empty directory) at `path` (relative to workspace).
#[tauri::command]
pub fn delete_file(path: String) -> Result<()> {
    let workspace = require_workspace()?;
    let abs = resolve_safe(&workspace, &path)?;
    if abs.is_dir() {
        std::fs::remove_dir_all(&abs).map_err(|e| ArcanaError::Agent(format!("rmdir: {e}")))?;
    } else {
        std::fs::remove_file(&abs).map_err(|e| ArcanaError::Agent(format!("rm: {e}")))?;
    }
    Ok(())
}

/// Rename or move a file/directory from `old_path` to `new_path` (both relative).
#[tauri::command]
pub fn rename_file(old_path: String, new_path: String) -> Result<()> {
    let workspace = require_workspace()?;
    let abs_old = resolve_safe(&workspace, &old_path)?;
    let abs_new = resolve_safe(&workspace, &new_path)?;
    if let Some(parent) = abs_new.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ArcanaError::Agent(format!("mkdir for rename: {e}")))?;
    }
    std::fs::rename(&abs_old, &abs_new).map_err(|e| ArcanaError::Agent(format!("rename: {e}")))?;
    Ok(())
}

/// Move a file/directory from `from_path` to a destination directory `to_dir`,
/// preserving the original filename.  Both paths are relative to workspace.
#[tauri::command]
pub fn move_file(from_path: String, to_dir: String) -> Result<()> {
    let workspace = require_workspace()?;
    let abs_from = resolve_safe(&workspace, &from_path)?;
    let file_name = abs_from
        .file_name()
        .ok_or_else(|| ArcanaError::Agent("invalid from_path".into()))?;
    let abs_to_dir = resolve_safe(&workspace, &to_dir)?;
    std::fs::create_dir_all(&abs_to_dir).map_err(|e| ArcanaError::Agent(format!("mkdir: {e}")))?;
    let abs_dest = resolve_safe(&workspace, &abs_to_dir.join(file_name).to_string_lossy())?;
    std::fs::rename(&abs_from, &abs_dest).map_err(|e| ArcanaError::Agent(format!("move: {e}")))?;
    Ok(())
}

/// Resolve a workspace-relative path to an absolute path, rejecting traversals
/// that escape the workspace root.
fn resolve_safe(workspace: &str, path: &str) -> Result<std::path::PathBuf> {
    let candidate = std::path::Path::new(path);
    let candidate = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        std::path::Path::new(workspace).join(candidate)
    };
    arcana_core::agent::builtin_tools::path_jail::validate_workspace_path(&candidate, workspace)
        .map_err(ArcanaError::Agent)
}

#[cfg(test)]
mod path_safety_tests {
    use super::resolve_safe;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ide_mutation_paths_refuse_traversal_and_accept_new_workspace_paths() {
        let workspace = TempDir::new().expect("workspace");
        let root = workspace.path().to_string_lossy().to_string();

        let inside = resolve_safe(&root, "nested/new-file.ts").expect("inside path");
        assert!(inside.starts_with(workspace.path()));
        assert!(resolve_safe(&root, "../outside.ts").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn ide_mutation_paths_refuse_symlink_escape() {
        use std::os::unix::fs::symlink;

        let workspace = TempDir::new().expect("workspace");
        let outside = TempDir::new().expect("outside");
        let link = workspace.path().join("outside-link");
        symlink(outside.path(), &link).expect("create symlink");
        let root = workspace.path().to_string_lossy().to_string();

        assert!(resolve_safe(&root, "outside-link/new-file.ts").is_err());
    }

    #[test]
    fn ide_mutation_paths_accept_existing_file_inside_workspace() {
        let workspace = TempDir::new().expect("workspace");
        let file = workspace.path().join("existing.ts");
        fs::write(&file, "const value = 1;\n").expect("write fixture");
        let root = workspace.path().to_string_lossy().to_string();

        let resolved = resolve_safe(&root, "existing.ts").expect("safe file");
        assert!(resolved.is_file());
        assert_eq!(
            fs::read_to_string(resolved).expect("read safe file"),
            "const value = 1;\n"
        );
    }
}

// ─── Symbol Navigation ────────────────────────────────────────────────────────

/// Find the definition site of the symbol at `(line, column)` in `path`.
/// `line` is 1-indexed; `column` is the 0-indexed byte offset within the line.
/// Returns `null` on the frontend when no definition is found.
#[tauri::command]
pub fn resolve_symbol(
    path: String,
    line: usize,
    column: usize,
) -> Result<Option<SymbolDefinition>> {
    let workspace = require_workspace()?;
    symbol_nav::resolve_symbol(&workspace, &path, line, column)
}

/// Find all locations in the workspace where `symbol_name` appears as a whole
/// word.  Searches source files only; binary files are skipped.
#[tauri::command]
pub fn find_symbol_references(
    symbol_name: String,
    max_results: Option<usize>,
) -> Result<Vec<SymbolReference>> {
    let workspace = require_workspace()?;
    symbol_nav::find_symbol_references(&workspace, &symbol_name, max_results.unwrap_or(200))
}

// ─── Workspace state (open tabs, expanded dirs) ───────────────────────────────

#[tauri::command]
pub fn get_ide_workspace_state() -> Result<IdeWorkspaceState> {
    workspace::load()
}

#[tauri::command]
pub fn set_ide_workspace_state(state: IdeWorkspaceState) -> Result<()> {
    workspace::save(&state)
}
