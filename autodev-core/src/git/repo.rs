// Repository operations — open, init, clone.
//
// Thin wrappers around git2::Repository that produce AutoDevError on failure
// and return serialisable RepoInfo structs for the frontend.

use std::path::{Path, PathBuf};

use git2::{Repository, RepositoryInitOptions};

use crate::errors::{AutoDevError, Result};

use super::types::RepoInfo;

/// Open an existing repository at `path` (or at any parent of it — libgit2
/// performs a walk-up discovery by default).
pub fn open(path: &str) -> Result<Repository> {
    Repository::discover(path).map_err(|e| AutoDevError::Git(format!("open({path}): {e}")))
}

/// Return a serialisable snapshot of the repository at `path`.
pub fn info(path: &str) -> Result<RepoInfo> {
    let repo = open(path)?;
    build_info(&repo)
}

/// Initialise a new repository at `path`. `bare=false` creates a working
/// directory repo (the usual case).
pub fn init(path: &str, bare: bool) -> Result<RepoInfo> {
    let mut opts = RepositoryInitOptions::new();
    opts.bare(bare);
    opts.mkdir(true);
    opts.mkpath(true);

    let repo = Repository::init_opts(path, &opts)
        .map_err(|e| AutoDevError::Git(format!("init({path}): {e}")))?;
    build_info(&repo)
}

/// Clone a repository from `url` to `into_path`. If `branch` is supplied
/// the clone checks out that branch after fetch.
pub fn clone_repo(url: &str, into_path: &str, branch: Option<&str>) -> Result<RepoInfo> {
    let mut builder = git2::build::RepoBuilder::new();
    if let Some(b) = branch {
        builder.branch(b);
    }
    let repo = builder
        .clone(url, Path::new(into_path))
        .map_err(|e| AutoDevError::Git(format!("clone({url} -> {into_path}): {e}")))?;
    build_info(&repo)
}

/// Build a RepoInfo DTO from an open Repository.
pub(crate) fn build_info(repo: &Repository) -> Result<RepoInfo> {
    let path = repo.path().to_string_lossy().to_string();
    let workdir = repo
        .workdir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_bare = repo.is_bare();
    let is_empty = repo.is_empty().unwrap_or(false);

    let (head_branch, head_oid) = match repo.head() {
        Ok(r) => (
            r.shorthand().map(|s| s.to_string()),
            r.target().map(|o| o.to_string()),
        ),
        Err(_) => (None, None),
    };

    let remote_url = repo
        .find_remote("origin")
        .ok()
        .and_then(|r| r.url().map(|s| s.to_string()));

    Ok(RepoInfo {
        path,
        workdir,
        is_bare,
        is_empty,
        head_branch,
        head_oid,
        remote_url,
    })
}

/// Resolve a path inside the workdir to a repository-relative path. If the
/// path is absolute it is made relative to `workdir`; if already relative
/// it is returned as-is (normalised to forward slashes).
pub(crate) fn to_repo_relative(repo: &Repository, path: &str) -> Result<String> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| AutoDevError::Git("repository has no workdir".into()))?;

    let p = PathBuf::from(path);
    let rel: PathBuf = if p.is_absolute() {
        p.strip_prefix(workdir)
            .map_err(|e| AutoDevError::Git(format!("strip_prefix: {e}")))?
            .to_path_buf()
    } else {
        p
    };

    Ok(rel.to_string_lossy().replace('\\', "/"))
}
