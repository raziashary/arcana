// File search commands — workspace file + folder autocomplete for @-mention in chat.
// Triggered when user types @ followed by a query in the chat textarea.
// Returns both files and directories, sorted by match quality.
// Skips common generated/irrelevant dirs (node_modules, target, .git, etc.)

use crate::commands::workspace_commands::load_workspace_dir;
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub absolute_path: String,
    pub relative_path: String,
    /// True when this entry is a directory (shown with folder icon + file count).
    pub is_dir: bool,
    /// For directories: number of files contained (recursive, capped at 999).
    pub children_count: u32,
}

// Directories to skip entirely during workspace walk
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".next",
    "dist",
    "build",
    "__pycache__",
    ".venv",
    "venv",
    ".cache",
    "coverage",
    ".turbo",
    ".svelte-kit",
    "out",
    ".output",
];

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Count files recursively inside a directory (max depth 6, capped at 999).
fn count_files_in_dir(dir: &std::path::Path) -> u32 {
    WalkDir::new(dir)
        .max_depth(6)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let n = e.file_name().to_string_lossy();
                !SKIP_DIRS.contains(&n.as_ref())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .take(1000)
        .count()
        .min(999) as u32
}

// ─── Command ──────────────────────────────────────────────────────────────────

/// Search workspace files AND directories matching a query (case-insensitive).
/// Returns ≤15 results sorted: exact > starts-with > contains; dirs before files at equal score.
/// Returns empty vec if workspace dir is not configured.
#[tauri::command]
pub fn search_workspace_files(query: String) -> Result<Vec<FileEntry>> {
    let workspace = load_workspace_dir();
    if workspace.is_empty() {
        return Ok(vec![]);
    }

    let q = query.to_lowercase();
    let ws_path = std::path::Path::new(&workspace);
    let mut results: Vec<(u8, FileEntry)> = vec![];

    for entry in WalkDir::new(&workspace)
        .max_depth(10)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                !SKIP_DIRS.contains(&name.as_ref())
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
        // Skip the workspace root itself
        .filter(|e| e.depth() > 0)
    {
        let abs = entry.path();
        let is_dir = entry.file_type().is_dir();
        let name = entry.file_name().to_string_lossy().to_string();

        let relative = abs
            .strip_prefix(ws_path)
            .unwrap_or(abs)
            .to_string_lossy()
            .replace('\\', "/");

        // Match against name, relative path, and (for dir queries with /) the path prefix
        let name_lc = name.to_lowercase();
        let rel_lc = relative.to_lowercase();

        let score: u8 = if q.is_empty() {
            // Empty query: show dirs first (score 0), then files (score 1)
            if is_dir {
                0
            } else {
                1
            }
        } else if name_lc == q || rel_lc == q {
            0
        } else if name_lc.starts_with(&q) || rel_lc.starts_with(&q) {
            1
        } else if name_lc.contains(&q) || rel_lc.contains(&q) {
            2
        } else {
            continue;
        };

        let children_count = if is_dir { count_files_in_dir(abs) } else { 0 };

        results.push((
            score,
            FileEntry {
                name,
                absolute_path: abs.to_string_lossy().into_owned(),
                relative_path: relative,
                is_dir,
                children_count,
            },
        ));

        if results.len() >= 30 {
            break;
        }
    }

    // Sort: score asc, then dirs before files, then alphabetical
    results.sort_by(|(sa, a), (sb, b)| {
        sa.cmp(sb)
            .then(b.is_dir.cmp(&a.is_dir))
            .then(a.name.cmp(&b.name))
    });

    Ok(results.into_iter().take(15).map(|(_, e)| e).collect())
}
