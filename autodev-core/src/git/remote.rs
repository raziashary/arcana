// Remote operations — fetch, push, pull.
//
// Credential handling uses the ssh-agent / default credential helper chain
// that libgit2 can negotiate automatically. More advanced auth (PAT via
// keyring) can be wired later through `RemoteCallbacks::credentials`.

use git2::{FetchOptions, PushOptions, RemoteCallbacks, Repository};

use crate::errors::{AutoDevError, Result};

use super::types::PullResult;

/// Fetch from `remote_name` (defaults to "origin").
pub fn fetch(repo_path: &str, remote_name: Option<&str>) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let name = remote_name.unwrap_or("origin");
    let mut remote = repo
        .find_remote(name)
        .map_err(|e| AutoDevError::Git(format!("find_remote({name}): {e}")))?;

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(default_callbacks());
    let refspecs: Vec<String> = remote
        .fetch_refspecs()
        .map_err(|e| AutoDevError::Git(format!("fetch_refspecs: {e}")))?
        .iter()
        .filter_map(|s| s.map(|s| s.to_string()))
        .collect();

    remote
        .fetch(&refspecs, Some(&mut fo), None)
        .map_err(|e| AutoDevError::Git(format!("fetch: {e}")))?;
    Ok(())
}

/// Push the current branch (or `branch` if supplied) to `remote_name`.
pub fn push(repo_path: &str, remote_name: Option<&str>, branch: Option<&str>) -> Result<()> {
    let repo = super::repo::open(repo_path)?;
    let name = remote_name.unwrap_or("origin");
    let mut remote = repo
        .find_remote(name)
        .map_err(|e| AutoDevError::Git(format!("find_remote({name}): {e}")))?;

    let branch_name = match branch {
        Some(b) => b.to_string(),
        None => repo
            .head()
            .ok()
            .and_then(|r| r.shorthand().map(|s| s.to_string()))
            .ok_or_else(|| AutoDevError::Git("no current branch for push".into()))?,
    };
    let refspec = format!("refs/heads/{b}:refs/heads/{b}", b = branch_name);

    let mut po = PushOptions::new();
    po.remote_callbacks(default_callbacks());
    remote
        .push(&[&refspec], Some(&mut po))
        .map_err(|e| AutoDevError::Git(format!("push: {e}")))?;
    Ok(())
}

/// Pull = fetch + merge (fast-forward only). On non-ff divergence we return
/// an error so the caller can surface it; a full merge helper lives in the
/// branch module if/when we need it.
pub fn pull(
    repo_path: &str,
    remote_name: Option<&str>,
    branch: Option<&str>,
) -> Result<PullResult> {
    fetch(repo_path, remote_name)?;
    let repo = super::repo::open(repo_path)?;

    let head_ref = repo
        .head()
        .map_err(|e| AutoDevError::Git(format!("head: {e}")))?;
    let branch_name = match branch {
        Some(b) => b.to_string(),
        None => head_ref
            .shorthand()
            .ok_or_else(|| AutoDevError::Git("HEAD has no shorthand".into()))?
            .to_string(),
    };
    let remote = remote_name.unwrap_or("origin");

    let upstream_ref = format!("refs/remotes/{remote}/{branch_name}");
    let upstream_oid = repo
        .refname_to_id(&upstream_ref)
        .map_err(|e| AutoDevError::Git(format!("refname_to_id({upstream_ref}): {e}")))?;
    let upstream_annotated = repo
        .find_annotated_commit(upstream_oid)
        .map_err(|e| AutoDevError::Git(format!("annotated commit: {e}")))?;

    let (analysis, _) = repo
        .merge_analysis(&[&upstream_annotated])
        .map_err(|e| AutoDevError::Git(format!("merge_analysis: {e}")))?;

    if analysis.is_up_to_date() {
        return Ok(PullResult {
            fetched: true,
            fast_forward: true,
            conflicts: vec![],
            merge_commit: None,
            files_changed: 0,
        });
    }

    if analysis.is_fast_forward() {
        // Fast-forward: move HEAD ref to upstream oid.
        let refname = format!("refs/heads/{branch_name}");
        let mut r = repo
            .find_reference(&refname)
            .map_err(|e| AutoDevError::Git(format!("find_reference: {e}")))?;
        r.set_target(upstream_oid, "pull: fast-forward")
            .map_err(|e| AutoDevError::Git(format!("set_target: {e}")))?;
        repo.set_head(&refname)
            .map_err(|e| AutoDevError::Git(format!("set_head: {e}")))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
            .map_err(|e| AutoDevError::Git(format!("checkout_head: {e}")))?;
        return Ok(PullResult {
            fetched: true,
            fast_forward: true,
            conflicts: vec![],
            merge_commit: Some(upstream_oid.to_string()),
            files_changed: 0,
        });
    }

    // Non-fast-forward merge is not attempted automatically; surface to caller.
    Err(AutoDevError::Git(format!(
        "pull requires non-ff merge on '{branch_name}'; merge manually via `git merge {remote}/{branch_name}`"
    )))
}

fn default_callbacks<'a>() -> RemoteCallbacks<'a> {
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|url, username_from_url, allowed_types| {
        // Try ssh-agent first if SSH is allowed.
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            let user = username_from_url.unwrap_or("git");
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(user) {
                return Ok(cred);
            }
        }
        // Try default credential helper (covers HTTPS PATs stored in OS credential manager).
        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            let cfg = git2::Config::open_default()
                .map_err(|e| git2::Error::from_str(&format!("config: {e}")))?;
            if let Ok(cred) = git2::Cred::credential_helper(&cfg, url, username_from_url) {
                return Ok(cred);
            }
        }
        Err(git2::Error::from_str("no credentials available"))
    });
    cb
}

#[allow(dead_code)]
fn _keep_linked(_r: &Repository) {}
