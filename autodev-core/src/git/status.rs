// Git status — categorises working-tree and index changes for the frontend.
//
// Produces a GitStatusReport with staged / unstaged / untracked / conflicted
// buckets plus ahead/behind counts vs. upstream.

use git2::{BranchType, Repository, Status, StatusOptions};

use crate::errors::{AutoDevError, Result};

use super::types::{FileStatus, GitStatusReport, StatusEntry};

/// Produce a GitStatusReport for the repository at `repo_path`.
pub fn status(repo_path: &str) -> Result<GitStatusReport> {
    let repo = super::repo::open(repo_path)?;

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);

    let statuses = repo
        .statuses(Some(&mut opts))
        .map_err(|e| AutoDevError::Git(format!("statuses: {e}")))?;

    let mut staged: Vec<StatusEntry> = Vec::new();
    let mut unstaged: Vec<StatusEntry> = Vec::new();
    let mut untracked: Vec<StatusEntry> = Vec::new();
    let mut conflicted: Vec<StatusEntry> = Vec::new();

    for entry in statuses.iter() {
        let s = entry.status();
        let path = entry
            .path()
            .map(|p| p.replace('\\', "/"))
            .unwrap_or_default();

        if path.is_empty() {
            continue;
        }

        if s.contains(Status::CONFLICTED) {
            conflicted.push(StatusEntry {
                path,
                status: FileStatus::Conflicted,
                has_worktree_changes: false,
                old_path: None,
            });
            continue;
        }

        // Index side (staged).
        let index_status = if s.contains(Status::INDEX_NEW) {
            Some(FileStatus::IndexNew)
        } else if s.contains(Status::INDEX_MODIFIED) {
            Some(FileStatus::IndexModified)
        } else if s.contains(Status::INDEX_DELETED) {
            Some(FileStatus::IndexDeleted)
        } else if s.contains(Status::INDEX_RENAMED) {
            Some(FileStatus::IndexRenamed)
        } else {
            None
        };

        let has_worktree_changes = s.contains(Status::WT_NEW)
            || s.contains(Status::WT_MODIFIED)
            || s.contains(Status::WT_DELETED)
            || s.contains(Status::WT_RENAMED);

        if let Some(fs) = index_status {
            staged.push(StatusEntry {
                path: path.clone(),
                status: fs,
                has_worktree_changes,
                old_path: entry.head_to_index().and_then(|d| {
                    d.old_file()
                        .path()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                }),
            });
        }

        // Worktree side (unstaged / untracked).
        if s.contains(Status::WT_NEW) {
            untracked.push(StatusEntry {
                path,
                status: FileStatus::WorktreeNew,
                has_worktree_changes: true,
                old_path: None,
            });
        } else if s.contains(Status::WT_MODIFIED) {
            unstaged.push(StatusEntry {
                path,
                status: FileStatus::WorktreeModified,
                has_worktree_changes: true,
                old_path: None,
            });
        } else if s.contains(Status::WT_DELETED) {
            unstaged.push(StatusEntry {
                path,
                status: FileStatus::WorktreeDeleted,
                has_worktree_changes: true,
                old_path: None,
            });
        } else if s.contains(Status::WT_RENAMED) {
            unstaged.push(StatusEntry {
                path,
                status: FileStatus::WorktreeModified,
                has_worktree_changes: true,
                old_path: None,
            });
        }
    }

    let (head_branch, ahead, behind) = branch_tracking(&repo);

    Ok(GitStatusReport {
        head_branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
        conflicted,
    })
}

/// Resolve HEAD branch name plus ahead/behind counts vs. its upstream.
fn branch_tracking(repo: &Repository) -> (Option<String>, usize, usize) {
    let head = match repo.head() {
        Ok(r) => r,
        Err(_) => return (None, 0, 0),
    };
    let name = head.shorthand().map(|s| s.to_string());

    let branch_name = match &name {
        Some(n) => n.clone(),
        None => return (name, 0, 0),
    };

    let branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return (name, 0, 0),
    };

    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return (name, 0, 0),
    };

    let local_oid = branch.get().target();
    let upstream_oid = upstream.get().target();

    if let (Some(lo), Some(uo)) = (local_oid, upstream_oid) {
        if let Ok((ahead, behind)) = repo.graph_ahead_behind(lo, uo) {
            return (name, ahead, behind);
        }
    }
    (name, 0, 0)
}
