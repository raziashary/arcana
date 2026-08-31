// Worktree management — add, list, remove.
//
// Worktrees let the Fleet (Phase 4) isolate per-task workspaces under
// `.autodev/worktrees/<slug>/` without touching the main working directory.

use std::path::Path;

use git2::{WorktreeAddOptions, WorktreePruneOptions};

use crate::errors::{AutoDevError, Result};

use super::types::WorktreeInfo;

pub fn list(repo_path: &str) -> Result<Vec<WorktreeInfo>> {
    let repo = super::repo::open(repo_path)?;

    let names = repo
        .worktrees()
        .map_err(|e| AutoDevError::Git(format!("worktrees list: {e}")))?;

    let mut out = Vec::new();
    for n in names.iter().flatten() {
        let wt = match repo.find_worktree(n) {
            Ok(w) => w,
            Err(_) => continue,
        };
        let path = wt.path().to_string_lossy().to_string();
        let is_locked = matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Locked(_)));

        // Resolve current branch of the worktree by opening it and reading HEAD.
        let branch: Option<String> = match git2::Repository::open(Path::new(&path)) {
            Ok(r) => {
                let name: Option<String> = r
                    .head()
                    .ok()
                    .and_then(|h| h.shorthand().map(|s| s.to_owned()));
                name
            }
            Err(_) => None,
        };

        out.push(WorktreeInfo {
            name: n.to_string(),
            path,
            branch,
            is_main: false,
            locked: is_locked,
        });
    }

    // Also report the main worktree (repo workdir) for UI completeness.
    if let Some(wd) = repo.workdir() {
        let branch = repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(|s| s.to_string()));
        out.insert(
            0,
            WorktreeInfo {
                name: "(main)".into(),
                path: wd.to_string_lossy().to_string(),
                branch,
                is_main: true,
                locked: false,
            },
        );
    }
    Ok(out)
}

/// Add a new worktree at `path`. If `branch` is supplied a new branch of
/// that name is created (or reused if it already exists).
pub fn add(repo_path: &str, path: &str, branch: Option<&str>) -> Result<WorktreeInfo> {
    let repo = super::repo::open(repo_path)?;

    let mut opts = WorktreeAddOptions::new();
    let reference_guard;
    if let Some(b) = branch {
        if repo.find_branch(b, git2::BranchType::Local).is_err() {
            let head = repo
                .head()
                .and_then(|r| r.peel_to_commit())
                .map_err(|e| AutoDevError::Git(format!("head commit: {e}")))?;
            repo.branch(b, &head, false)
                .map_err(|e| AutoDevError::Git(format!("branch create: {e}")))?;
        }
        reference_guard = repo
            .find_reference(&format!("refs/heads/{b}"))
            .map_err(|e| AutoDevError::Git(format!("find_reference: {e}")))?;
        opts.reference(Some(&reference_guard));
    }

    let name = Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("worktree");

    let wt = repo
        .worktree(name, Path::new(path), Some(&opts))
        .map_err(|e| AutoDevError::Git(format!("worktree add: {e}")))?;

    Ok(WorktreeInfo {
        name: name.to_string(),
        path: wt.path().to_string_lossy().to_string(),
        branch: branch.map(|s| s.to_string()),
        is_main: false,
        locked: false,
    })
}

pub fn remove(repo_path: &str, wt_path: &str, force: bool) -> Result<()> {
    let repo = super::repo::open(repo_path)?;

    // Find the worktree whose path matches `wt_path`.
    let names = repo
        .worktrees()
        .map_err(|e| AutoDevError::Git(format!("worktrees: {e}")))?;
    let wt_path_norm = Path::new(wt_path);

    for n in names.iter().flatten() {
        if let Ok(wt) = repo.find_worktree(n) {
            if wt.path() == wt_path_norm {
                let mut opts = WorktreePruneOptions::new();
                opts.valid(force).locked(force).working_tree(true);
                wt.prune(Some(&mut opts))
                    .map_err(|e| AutoDevError::Git(format!("worktree prune: {e}")))?;

                // Best-effort cleanup of the working directory on disk.
                if force {
                    let _ = std::fs::remove_dir_all(wt_path_norm);
                }
                return Ok(());
            }
        }
    }
    Err(AutoDevError::Git(format!(
        "no worktree registered at '{wt_path}'"
    )))
}
