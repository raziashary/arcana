// Stage / unstage / commit — index manipulation and HEAD advancement.
//
// Commits always receive an AutoDev attribution trailer when `trailer` is
// supplied, per the Phase 3.3 spec:
//     Generated-by: AutoDev/<agent-role>

use std::path::Path;

use git2::{IndexAddOption, Repository, Signature, Time};

use crate::errors::{AutoDevError, Result};

use super::types::CommitInfo;

/// Stage the given paths (add to the index).
pub fn stage(repo_path: &str, files: &[String]) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let mut index = repo.index().map_err(|e| AutoDevError::Git(e.to_string()))?;
    index
        .add_all(
            files.iter().map(|s| s.as_str()),
            IndexAddOption::DEFAULT,
            None,
        )
        .map_err(|e| AutoDevError::Git(format!("stage add_all: {e}")))?;
    index
        .write()
        .map_err(|e| AutoDevError::Git(format!("index write: {e}")))?;
    Ok(())
}

/// Unstage the given paths (reset index entries back to HEAD).
pub fn unstage(repo_path: &str, files: &[String]) -> Result<()> {
    let repo = super::repo::open(repo_path)?;

    // When HEAD exists, reset the paths against HEAD commit.
    if let Ok(head) = repo.head() {
        if let Ok(obj) = head.peel(git2::ObjectType::Commit) {
            let paths: Vec<&str> = files.iter().map(|s| s.as_str()).collect();
            repo.reset_default(Some(&obj), paths.iter())
                .map_err(|e| AutoDevError::Git(format!("unstage reset: {e}")))?;
            return Ok(());
        }
    }

    // No HEAD yet (initial commit) — remove from index directly.
    let mut index = repo.index().map_err(|e| AutoDevError::Git(e.to_string()))?;
    for f in files {
        let _ = index.remove_path(Path::new(f));
    }
    index
        .write()
        .map_err(|e| AutoDevError::Git(format!("index write: {e}")))?;
    Ok(())
}

/// Create a commit from the current index. If `trailer` is supplied
/// (e.g. "AutoDev/PA"), appends a `Generated-by:` trailer to the message.
pub fn commit(
    repo_path: &str,
    message: &str,
    author: Option<(String, String)>,
    trailer: Option<String>,
) -> Result<CommitInfo> {
    let repo = super::repo::open(repo_path)?;

    let sig = match author {
        Some((name, email)) => {
            Signature::new(&name, &email, &Time::new(chrono::Utc::now().timestamp(), 0))
                .map_err(|e| AutoDevError::Git(format!("signature: {e}")))?
        }
        None => repo
            .signature()
            .map_err(|e| AutoDevError::Git(format!("default signature: {e}")))?,
    };

    let full_message = match trailer {
        Some(t) if !t.trim().is_empty() => {
            // Ensure one blank line between body and trailers per git convention.
            let trimmed = message.trim_end();
            format!("{trimmed}\n\nGenerated-by: {t}\n")
        }
        _ => message.to_string(),
    };

    let mut index = repo.index().map_err(|e| AutoDevError::Git(e.to_string()))?;
    let tree_oid = index
        .write_tree()
        .map_err(|e| AutoDevError::Git(format!("write_tree: {e}")))?;
    let tree = repo
        .find_tree(tree_oid)
        .map_err(|e| AutoDevError::Git(format!("find_tree: {e}")))?;

    let parents: Vec<git2::Commit> = match repo.head().ok().and_then(|h| h.peel_to_commit().ok()) {
        Some(c) => vec![c],
        None => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, &full_message, &tree, &parent_refs)
        .map_err(|e| AutoDevError::Git(format!("commit: {e}")))?;

    let commit = repo
        .find_commit(oid)
        .map_err(|e| AutoDevError::Git(e.to_string()))?;
    Ok(to_commit_info(&commit))
}

pub(crate) fn to_commit_info(commit: &git2::Commit) -> CommitInfo {
    let summary = commit.summary().unwrap_or("").to_string();
    let body = commit.body().unwrap_or("").to_string();
    let author = commit.author();
    let time = chrono::DateTime::<chrono::Utc>::from_timestamp(author.when().seconds(), 0)
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();

    CommitInfo {
        oid: commit.id().to_string(),
        short_oid: commit.id().to_string().chars().take(8).collect(),
        summary,
        body,
        author_name: author.name().unwrap_or("").to_string(),
        author_email: author.email().unwrap_or("").to_string(),
        time,
        parents: commit.parent_ids().map(|o| o.to_string()).collect(),
    }
}

/// Discard worktree changes for the given paths — restores them from HEAD.
pub fn discard_changes(repo_path: &str, files: &[String]) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.force();
    for f in files {
        opts.path(f);
    }
    repo.checkout_head(Some(&mut opts))
        .map_err(|e| AutoDevError::Git(format!("checkout_head discard: {e}")))?;
    Ok(())
}

/// Helper used by higher-level code (e.g. recipe runner) that does not
/// require a full Repository handle.
pub(crate) fn _noop_sig_check(_repo: &Repository) -> Result<()> {
    Ok(())
}
