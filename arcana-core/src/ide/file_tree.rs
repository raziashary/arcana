// IDE file tree generation with git-status overlay.
//
// Design:
//   - Walk the workspace directory up to `depth` levels.
//   - Skip noisy directories (.git, node_modules, target, …) AND any custom
//     ignore patterns supplied by the caller.
//   - When the directory sits inside a git repo, overlay git status flags
//     produced by git2 on each returned node.
//
// Lazy loading: if a directory sits past `depth`, we still emit a node for it
// but leave `children` empty and set `has_more=true`. The frontend calls
// `read_file_tree` again with that directory as the new root to expand.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use git2::{Repository, Status, StatusOptions};

use super::types::{FileTreeNode, GitStatus};
use crate::errors::{ArcanaError, Result};

/// Directory names that are always pruned from the tree.
const DEFAULT_SKIP_DIRS: &[&str] = &[
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
    ".idea",
    ".vscode",
];

// ─── Public API ───────────────────────────────────────────────────────────────

/// Read a file tree rooted at `root` (must live inside `workspace_root`).
/// `depth` caps recursion: 0 = only immediate children, 1 = children + grandchildren, …
/// `extra_ignore` is merged with DEFAULT_SKIP_DIRS.
pub fn read_tree(
    workspace_root: &str,
    root: &str,
    depth: usize,
    extra_ignore: &[String],
) -> Result<FileTreeNode> {
    let workspace = Path::new(workspace_root);
    let abs_root = resolve_inside_workspace(workspace, root)?;

    if !abs_root.exists() {
        return Err(ArcanaError::Agent(format!(
            "Path does not exist: {}",
            abs_root.display()
        )));
    }

    // Discover git repo once; reused for every node's status lookup.
    let git_index = build_git_status_index(workspace).ok();

    let ignore: Vec<&str> = DEFAULT_SKIP_DIRS
        .iter()
        .copied()
        .chain(extra_ignore.iter().map(|s| s.as_str()))
        .collect();

    let node = build_node(&abs_root, workspace, depth, &ignore, git_index.as_ref())?;
    Ok(node)
}

// ─── Node construction ────────────────────────────────────────────────────────

fn build_node(
    abs: &Path,
    workspace: &Path,
    depth: usize,
    ignore: &[&str],
    git_index: Option<&HashMap<PathBuf, GitStatus>>,
) -> Result<FileTreeNode> {
    let meta = std::fs::metadata(abs)
        .map_err(|e| ArcanaError::Agent(format!("metadata {}: {e}", abs.display())))?;
    let is_dir = meta.is_dir();

    let name = abs
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| abs.to_string_lossy().into_owned());

    let relative_path = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();

    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);

    let git_status = git_index.and_then(|idx| idx.get(&normalize(abs)).copied());

    let (children, has_more) = if is_dir {
        read_children(abs, workspace, depth, ignore, git_index)
    } else {
        (Vec::new(), false)
    };

    Ok(FileTreeNode {
        name,
        relative_path,
        absolute_path: abs.to_string_lossy().into_owned(),
        is_directory: is_dir,
        size_bytes: if is_dir { None } else { Some(meta.len()) },
        modified_ms,
        git_status,
        children,
        has_more,
    })
}

fn read_children(
    dir: &Path,
    workspace: &Path,
    depth: usize,
    ignore: &[&str],
    git_index: Option<&HashMap<PathBuf, GitStatus>>,
) -> (Vec<FileTreeNode>, bool) {
    let reader = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return (Vec::new(), false),
    };

    let mut entries: Vec<std::fs::DirEntry> = reader
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            !ignore.iter().any(|i| i.eq_ignore_ascii_case(&s))
        })
        .collect();

    // Dirs before files, alphabetical within group.
    entries.sort_by(|a, b| {
        let a_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.file_name().cmp(&b.file_name()))
    });

    let mut children = Vec::with_capacity(entries.len());
    for entry in entries {
        let abs = entry.path();

        if depth == 0 && entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            // Emit a placeholder so the frontend can show the folder and lazy-load.
            children.push(placeholder_dir(&abs, workspace, git_index));
            continue;
        }

        let next_depth = depth.saturating_sub(1);
        if let Ok(node) = build_node(&abs, workspace, next_depth, ignore, git_index) {
            children.push(node);
        }
    }

    (children, false)
}

fn placeholder_dir(
    abs: &Path,
    workspace: &Path,
    git_index: Option<&HashMap<PathBuf, GitStatus>>,
) -> FileTreeNode {
    let name = abs
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let relative_path = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    let git_status = git_index.and_then(|idx| idx.get(&normalize(abs)).copied());

    FileTreeNode {
        name,
        relative_path,
        absolute_path: abs.to_string_lossy().into_owned(),
        is_directory: true,
        size_bytes: None,
        modified_ms: None,
        git_status,
        children: Vec::new(),
        has_more: true,
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn resolve_inside_workspace(workspace: &Path, target: &str) -> Result<PathBuf> {
    let p = Path::new(target);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workspace.join(p)
    };

    // Security: prevent escaping workspace via ".." components.
    let normalized = normalize(&abs);
    let ws_norm = normalize(workspace);
    if !normalized.starts_with(&ws_norm) {
        return Err(ArcanaError::Agent(format!(
            "Path is outside workspace: {}",
            abs.display()
        )));
    }
    Ok(abs)
}

fn normalize(p: &Path) -> PathBuf {
    // Canonicalize when possible; fall back to the path as-is so missing paths
    // can still be reported with a useful error message.
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

// ─── Git status overlay ───────────────────────────────────────────────────────

fn build_git_status_index(
    workspace: &Path,
) -> std::result::Result<HashMap<PathBuf, GitStatus>, git2::Error> {
    let repo = Repository::discover(workspace)?;
    let repo_root = repo
        .workdir()
        .ok_or_else(|| git2::Error::from_str("bare repositories are not supported"))?
        .to_path_buf();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .include_ignored(false)
        .include_unmodified(false)
        .recurse_untracked_dirs(true);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();

    for entry in statuses.iter() {
        let Some(rel) = entry.path() else { continue };
        let abs = repo_root.join(rel);
        let abs = normalize(&abs);
        let status = map_status(entry.status());
        map.insert(abs, status);
    }

    Ok(map)
}

/// Map a git2 `Status` flag set onto our simplified `GitStatus` enum.
/// Exposed for reuse by other IDE modules (file_reader, etc.).
pub fn map_git_status(s: Status) -> GitStatus {
    map_status(s)
}

fn map_status(s: Status) -> GitStatus {
    if s.is_conflicted() {
        GitStatus::Conflicted
    } else if s.contains(Status::INDEX_NEW)
        || s.contains(Status::INDEX_MODIFIED)
        || s.contains(Status::INDEX_RENAMED)
        || s.contains(Status::INDEX_TYPECHANGE)
    {
        GitStatus::Staged
    } else if s.contains(Status::WT_DELETED) || s.contains(Status::INDEX_DELETED) {
        GitStatus::Deleted
    } else if s.contains(Status::WT_NEW) {
        GitStatus::Untracked
    } else if s.contains(Status::IGNORED) {
        GitStatus::Ignored
    } else if s.contains(Status::WT_MODIFIED)
        || s.contains(Status::WT_RENAMED)
        || s.contains(Status::WT_TYPECHANGE)
    {
        GitStatus::Modified
    } else {
        GitStatus::Clean
    }
}
