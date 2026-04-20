// IDE Tauri commands — thin wrappers around the `ide/` module.
// All commands resolve against the configured workspace_dir (see
// workspace_commands::load_workspace_dir).

use crate::commands::workspace_commands::load_workspace_dir;
use crate::errors::{AutoDevError, Result};
use crate::ide::{
    file_reader, file_search, file_tree, symbols,
    types::{ContentSearchResult, FileContent, FileMetadata, FileSearchResult, FileTreeNode, SymbolTree},
    workspace::{self, IdeWorkspaceState},
};

// ─── Helper ───────────────────────────────────────────────────────────────────

fn require_workspace() -> Result<String> {
    let ws = load_workspace_dir();
    if ws.is_empty() {
        return Err(AutoDevError::Agent(
            "Workspace folder is not configured. Set one in Settings first.".into(),
        ));
    }
    Ok(ws)
}

// ─── File Tree ────────────────────────────────────────────────────────────────

/// Read the file tree at `path` (defaults to workspace root when empty).
/// `depth` caps recursion: 0 = only immediate children, 1 = two levels, …
#[tauri::command]
pub fn read_file_tree(
    path:            Option<String>,
    depth:           Option<usize>,
    ignore_patterns: Option<Vec<String>>,
) -> Result<FileTreeNode> {
    let workspace = require_workspace()?;
    let target    = path.unwrap_or_default();
    let root      = if target.is_empty() { workspace.clone() } else { target };
    let depth     = depth.unwrap_or(1);
    let ignore    = ignore_patterns.unwrap_or_default();

    file_tree::read_tree(&workspace, &root, depth, &ignore)
}

// ─── File Content ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn read_file_content(
    path:       String,
    start_line: Option<usize>,
    end_line:   Option<usize>,
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
    query:        String,
    path:         Option<String>,
    glob:         Option<String>,
    max_results:  Option<usize>,
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
    query:       String,
    path:        Option<String>,
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

// ─── Workspace state (open tabs, expanded dirs) ───────────────────────────────

#[tauri::command]
pub fn get_ide_workspace_state() -> Result<IdeWorkspaceState> {
    workspace::load()
}

#[tauri::command]
pub fn set_ide_workspace_state(state: IdeWorkspaceState) -> Result<()> {
    workspace::save(&state)
}
