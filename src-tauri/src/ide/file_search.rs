// File search — quick-open (Ctrl+P) + grep-style content search.
//
// Two entry points:
//   search_files(query, scope, glob?, max) → fuzzy match file & directory names
//   search_contents(query, scope, max)     → literal substring search across files
//
// Both are scoped to `workspace_root` (and optionally a sub-path) and always
// skip the noisy directories enumerated in file_commands.rs / file_tree.rs.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::errors::{AutoDevError, Result};
use super::types::{ContentMatch, ContentSearchResult, FileSearchResult};

const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", ".next", "dist", "build",
    "__pycache__", ".venv", "venv", ".cache", "coverage",
    ".turbo", ".svelte-kit", "out", ".output", ".idea", ".vscode",
];

// ─── Name search ──────────────────────────────────────────────────────────────

/// Fuzzy-match file and directory names against `query`.
/// Empty query returns the first `max_results` entries (dirs first) — useful
/// for "recent files" style placeholders.
pub fn search_files(
    workspace_root: &str,
    query:          &str,
    scope:          Option<&str>,
    glob_pattern:   Option<&str>,
    max_results:    usize,
) -> Result<Vec<FileSearchResult>> {
    let workspace = Path::new(workspace_root);
    let root = resolve_scope(workspace, scope)?;

    let q = query.to_lowercase();
    let globber: Option<glob::Pattern> = glob_pattern
        .filter(|s| !s.is_empty())
        .and_then(|s| glob::Pattern::new(s).ok());

    let mut results: Vec<FileSearchResult> = Vec::new();

    for entry in WalkDir::new(&root)
        .max_depth(12)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| should_walk(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() > 0)
    {
        let abs  = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        let relative = abs
            .strip_prefix(workspace)
            .unwrap_or(abs)
            .to_string_lossy()
            .replace('\\', "/");

        if let Some(p) = &globber {
            if !p.matches(&relative) {
                continue;
            }
        }

        let score = fuzzy_score(&q, &name.to_lowercase(), &relative.to_lowercase());
        let Some(score) = score else { continue };

        results.push(FileSearchResult {
            name,
            relative_path: relative,
            absolute_path: abs.to_string_lossy().into_owned(),
            is_directory:  entry.file_type().is_dir(),
            score,
        });

        if results.len() >= max_results.saturating_mul(4) {
            break;
        }
    }

    // Sort by score asc (better first), dirs before files at equal score, then alphabetical.
    results.sort_by(|a, b| {
        a.score.cmp(&b.score)
            .then(b.is_directory.cmp(&a.is_directory))
            .then(a.name.cmp(&b.name))
    });
    results.truncate(max_results);

    Ok(results)
}

/// Returns `Some(score)` when the query matches; lower is better.
/// `None` means no match.
/// Empty query matches everything with a neutral score.
fn fuzzy_score(query: &str, name_lc: &str, rel_lc: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(1_000);
    }
    if name_lc == query || rel_lc == query {
        return Some(0);
    }
    if name_lc.starts_with(query) {
        return Some(10);
    }
    if rel_lc.starts_with(query) {
        return Some(20);
    }
    if name_lc.contains(query) {
        return Some(50 + (name_lc.len() as i32 - query.len() as i32));
    }
    if rel_lc.contains(query) {
        return Some(100 + (rel_lc.len() as i32 - query.len() as i32));
    }
    // Lightweight subsequence match for fuzzy typing ("utst" → "update_task_status").
    if subsequence(query, name_lc) {
        return Some(250);
    }
    if subsequence(query, rel_lc) {
        return Some(400);
    }
    None
}

fn subsequence(needle: &str, haystack: &str) -> bool {
    let mut hs = haystack.chars();
    'outer: for nc in needle.chars() {
        for hc in hs.by_ref() {
            if hc == nc {
                continue 'outer;
            }
        }
        return false;
    }
    true
}

// ─── Content search ───────────────────────────────────────────────────────────

/// Plain-substring content search (case-insensitive).
/// Returns at most `max_results` files; within a file, match lines are capped at 50.
pub fn search_contents(
    workspace_root: &str,
    query:          &str,
    scope:          Option<&str>,
    max_results:    usize,
) -> Result<Vec<ContentSearchResult>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let workspace = Path::new(workspace_root);
    let root = resolve_scope(workspace, scope)?;
    let needle = query.to_lowercase();

    let mut results: Vec<ContentSearchResult> = Vec::new();

    for entry in WalkDir::new(&root)
        .max_depth(12)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| should_walk(e))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let abs = entry.path();
        // Skip very large files (> 2 MB) and obvious binaries.
        let Ok(meta) = entry.metadata() else { continue };
        if meta.len() > 2 * 1024 * 1024 {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(abs) else { continue };

        let mut matches: Vec<ContentMatch> = Vec::new();
        for (idx, line) in text.lines().enumerate() {
            let lc = line.to_lowercase();
            if let Some(pos) = lc.find(&needle) {
                matches.push(ContentMatch {
                    line_number: idx + 1,
                    line_text:   line.to_string(),
                    match_start: pos,
                    match_end:   pos + query.len(),
                });
                if matches.len() >= 50 {
                    break;
                }
            }
        }

        if !matches.is_empty() {
            let relative = abs
                .strip_prefix(workspace)
                .unwrap_or(abs)
                .to_string_lossy()
                .replace('\\', "/");
            results.push(ContentSearchResult {
                relative_path: relative,
                absolute_path: abs.to_string_lossy().into_owned(),
                matches,
            });
            if results.len() >= max_results {
                break;
            }
        }
    }

    Ok(results)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn should_walk(e: &walkdir::DirEntry) -> bool {
    if e.file_type().is_dir() {
        let name = e.file_name().to_string_lossy();
        !SKIP_DIRS.iter().any(|s| s.eq_ignore_ascii_case(&name))
    } else {
        true
    }
}

fn resolve_scope(workspace: &Path, scope: Option<&str>) -> Result<PathBuf> {
    match scope.filter(|s| !s.is_empty()) {
        None => Ok(workspace.to_path_buf()),
        Some(s) => {
            let p = Path::new(s);
            let abs = if p.is_absolute() { p.to_path_buf() } else { workspace.join(p) };
            let ws_norm = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
            let abs_norm = std::fs::canonicalize(&abs).unwrap_or_else(|_| abs.clone());
            if !abs_norm.starts_with(&ws_norm) {
                return Err(AutoDevError::Agent(format!(
                    "Search scope is outside workspace: {}",
                    abs.display()
                )));
            }
            Ok(abs)
        }
    }
}
