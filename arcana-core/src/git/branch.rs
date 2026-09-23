// Branch management — list, create, delete, checkout.

use git2::{BranchType, Repository};

use crate::errors::{ArcanaError, Result};

use super::types::BranchInfo;

/// List all local + remote branches.
pub fn list(repo_path: &str) -> Result<Vec<BranchInfo>> {
    let repo = super::repo::open(repo_path)?;
    let head_oid = repo.head().ok().and_then(|r| r.target());

    let mut out = Vec::new();
    let branches = repo
        .branches(None)
        .map_err(|e| ArcanaError::Git(format!("branches: {e}")))?;

    for item in branches {
        let (branch, btype) = match item {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        let name = branch.name().ok().flatten().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }

        let oid = branch.get().target();
        let is_head =
            matches!((btype, oid, head_oid), (BranchType::Local, Some(a), Some(b)) if a == b);

        let (upstream, ahead, behind) = match branch.upstream() {
            Ok(up) => {
                let up_name = up.name().ok().flatten().map(|s| s.to_string());
                let (a, b) = match (oid, up.get().target()) {
                    (Some(lo), Some(uo)) => repo.graph_ahead_behind(lo, uo).unwrap_or((0, 0)),
                    _ => (0, 0),
                };
                (up_name, a, b)
            }
            Err(_) => (None, 0, 0),
        };

        out.push(BranchInfo {
            name,
            is_remote: matches!(btype, BranchType::Remote),
            is_head,
            upstream,
            ahead,
            behind,
            last_commit: oid.map(|o| o.to_string()),
        });
    }
    Ok(out)
}

/// Create a branch at HEAD. If `checkout` is true, also switch the
/// working tree to the new branch.
pub fn create(repo_path: &str, name: &str, checkout: bool) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let head = repo
        .head()
        .and_then(|r| r.peel_to_commit())
        .map_err(|e| ArcanaError::Git(format!("head commit: {e}")))?;

    repo.branch(name, &head, false)
        .map_err(|e| ArcanaError::Git(format!("branch create: {e}")))?;

    if checkout {
        checkout_ref(&repo, &format!("refs/heads/{name}"))?;
    }
    Ok(())
}

/// Delete a local branch.
pub fn delete(repo_path: &str, name: &str, force: bool) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let mut b = repo
        .find_branch(name, BranchType::Local)
        .map_err(|e| ArcanaError::Git(format!("find_branch: {e}")))?;

    if !force {
        // Guard against deleting the current HEAD.
        if b.is_head() {
            return Err(ArcanaError::Git(format!(
                "cannot delete current branch '{name}'"
            )));
        }
    }
    b.delete()
        .map_err(|e| ArcanaError::Git(format!("branch delete: {e}")))?;
    Ok(())
}

/// Checkout a branch or revision by ref name / oid.
pub fn checkout(repo_path: &str, refname: &str) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    checkout_ref(&repo, refname)
}

/// Checkout a specific commit by OID (detached HEAD state).
pub fn checkout_commit(repo_path: &str, commit_oid: &str) -> Result<()> {
    let repo = super::repo::open(repo_path)?;

    let oid = git2::Oid::from_str(commit_oid)
        .map_err(|e| ArcanaError::Git(format!("parse oid: {e}")))?;

    let commit = repo
        .find_commit(oid)
        .map_err(|e| ArcanaError::Git(format!("find_commit: {e}")))?;

    repo.checkout_tree(commit.as_object(), None)
        .map_err(|e| ArcanaError::Git(format!("checkout_tree: {e}")))?;

    repo.set_head_detached(oid)
        .map_err(|e| ArcanaError::Git(format!("set_head_detached: {e}")))?;

    Ok(())
}

/// Create a new branch from a specific commit.
pub fn create_from_commit(
    repo_path: &str,
    branch_name: &str,
    commit_oid: &str,
    checkout: bool,
) -> Result<()> {
    let repo = super::repo::open(repo_path)?;

    let oid = git2::Oid::from_str(commit_oid)
        .map_err(|e| ArcanaError::Git(format!("parse oid: {e}")))?;

    let commit = repo
        .find_commit(oid)
        .map_err(|e| ArcanaError::Git(format!("find_commit: {e}")))?;

    repo.branch(branch_name, &commit, false)
        .map_err(|e| ArcanaError::Git(format!("branch create: {e}")))?;

    if checkout {
        checkout_ref(&repo, &format!("refs/heads/{branch_name}"))?;
    }

    Ok(())
}

fn checkout_ref(repo: &Repository, refname: &str) -> Result<()> {
    // If the user passed a short branch name, expand to refs/heads/… first.
    let full_ref = if refname.starts_with("refs/") {
        refname.to_string()
    } else if repo.find_branch(refname, BranchType::Local).is_ok() {
        format!("refs/heads/{refname}")
    } else {
        refname.to_string()
    };

    let (obj, reference) = repo
        .revparse_ext(&full_ref)
        .map_err(|e| ArcanaError::Git(format!("revparse {full_ref}: {e}")))?;

    repo.checkout_tree(&obj, None)
        .map_err(|e| ArcanaError::Git(format!("checkout_tree: {e}")))?;

    match reference {
        Some(r) => repo
            .set_head(r.name().unwrap_or(&full_ref))
            .map_err(|e| ArcanaError::Git(format!("set_head: {e}")))?,
        None => repo
            .set_head_detached(obj.id())
            .map_err(|e| ArcanaError::Git(format!("set_head_detached: {e}")))?,
    };
    Ok(())
}
