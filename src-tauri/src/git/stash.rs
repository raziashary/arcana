// Stash management — list / push / apply / drop.

use git2::{Signature, StashApplyOptions, StashFlags, Time};

use crate::errors::{AutoDevError, Result};

use super::types::StashInfo;

pub fn list(repo_path: &str) -> Result<Vec<StashInfo>> {
    let mut repo = super::repo::open(repo_path)?;
    let mut out = Vec::new();
    repo.stash_foreach(|index, message, oid| {
        out.push(StashInfo {
            index,
            oid:     oid.to_string(),
            message: message.to_string(),
            time:    String::new(), // libgit2's stash_foreach doesn't expose time directly
        });
        true
    })
    .map_err(|e| AutoDevError::Git(format!("stash_foreach: {e}")))?;
    Ok(out)
}

/// Create a stash from the current worktree+index. Returns the new stash's oid.
pub fn push(repo_path: &str, message: Option<&str>, include_untracked: bool) -> Result<String> {
    let mut repo = super::repo::open(repo_path)?;
    let sig = repo
        .signature()
        .or_else(|_| Signature::new("AutoDev", "autodev@local", &Time::new(chrono::Utc::now().timestamp(), 0)))
        .map_err(|e| AutoDevError::Git(format!("signature: {e}")))?;

    let mut flags = StashFlags::DEFAULT;
    if include_untracked {
        flags |= StashFlags::INCLUDE_UNTRACKED;
    }

    let oid = repo
        .stash_save(&sig, message.unwrap_or(""), Some(flags))
        .map_err(|e| AutoDevError::Git(format!("stash_save: {e}")))?;
    Ok(oid.to_string())
}

/// Apply stash at `index`. When `pop` is true, the stash is removed after
/// a successful apply.
pub fn apply(repo_path: &str, index: usize, pop: bool) -> Result<()> {
    let mut repo = super::repo::open(repo_path)?;
    let mut opts = StashApplyOptions::new();
    repo.stash_apply(index, Some(&mut opts))
        .map_err(|e| AutoDevError::Git(format!("stash_apply: {e}")))?;
    if pop {
        repo.stash_drop(index)
            .map_err(|e| AutoDevError::Git(format!("stash_drop: {e}")))?;
    }
    Ok(())
}

pub fn drop(repo_path: &str, index: usize) -> Result<()> {
    let mut repo = super::repo::open(repo_path)?;
    repo.stash_drop(index)
        .map_err(|e| AutoDevError::Git(format!("stash_drop: {e}")))?;
    Ok(())
}
