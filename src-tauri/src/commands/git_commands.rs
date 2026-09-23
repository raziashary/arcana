// Tauri command wrappers for the git module (Phase 3.3).
//
// Each command is a thin async function that delegates to `crate::git::*`
// helpers. Long-running operations (fetch/push/pull/clone) run on a
// blocking thread so they don't stall the UI thread.

use crate::errors::{ArcanaError, Result};
use crate::git::types::{
    BlameLine, BranchInfo, CheckpointInfo, CommitInfo, DiffResult, GitStatusReport, PullResult,
    RepoInfo, RestoreResult, StashInfo, WorktreeInfo,
};
use crate::git::{
    branch, checkpoint, commit, diff, history, remote, repo, stash, status, worktree,
};

// ─── Repository ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_open_repo(path: String) -> Result<RepoInfo> {
    repo::info(&path)
}

#[tauri::command]
pub async fn git_clone(url: String, path: String, branch: Option<String>) -> Result<RepoInfo> {
    tokio::task::spawn_blocking(move || repo::clone_repo(&url, &path, branch.as_deref()))
        .await
        .map_err(|e| ArcanaError::Git(format!("clone join: {e}")))?
}

#[tauri::command]
pub fn git_init(path: String, bare: Option<bool>) -> Result<RepoInfo> {
    repo::init(&path, bare.unwrap_or(false))
}

// ─── Status / Diff ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_status(repo_path: String) -> Result<GitStatusReport> {
    status::status(&repo_path)
}

#[tauri::command]
pub fn git_diff(
    repo_path: String,
    file: Option<String>,
    staged: Option<bool>,
) -> Result<DiffResult> {
    diff::diff(&repo_path, file.as_deref(), staged.unwrap_or(false))
}

// ─── Staging / Commit ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_stage(repo_path: String, files: Vec<String>) -> Result<()> {
    commit::stage(&repo_path, &files)
}

#[tauri::command]
pub fn git_unstage(repo_path: String, files: Vec<String>) -> Result<()> {
    commit::unstage(&repo_path, &files)
}

#[tauri::command]
pub fn git_discard(repo_path: String, files: Vec<String>) -> Result<()> {
    commit::discard_changes(&repo_path, &files)
}

#[tauri::command]
pub fn git_commit(
    repo_path: String,
    message: String,
    author_name: Option<String>,
    author_email: Option<String>,
    trailer: Option<String>,
) -> Result<CommitInfo> {
    let author = match (author_name, author_email) {
        (Some(n), Some(e)) => Some((n, e)),
        _ => None,
    };
    commit::commit(&repo_path, &message, author, trailer)
}

// ─── Remote ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn git_push(
    repo_path: String,
    remote_name: Option<String>,
    branch: Option<String>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        remote::push(&repo_path, remote_name.as_deref(), branch.as_deref())
    })
    .await
    .map_err(|e| ArcanaError::Git(format!("push join: {e}")))?
}

#[tauri::command]
pub async fn git_pull(
    repo_path: String,
    remote_name: Option<String>,
    branch: Option<String>,
) -> Result<PullResult> {
    tokio::task::spawn_blocking(move || {
        remote::pull(&repo_path, remote_name.as_deref(), branch.as_deref())
    })
    .await
    .map_err(|e| ArcanaError::Git(format!("pull join: {e}")))?
}

#[tauri::command]
pub async fn git_fetch(repo_path: String, remote_name: Option<String>) -> Result<()> {
    tokio::task::spawn_blocking(move || remote::fetch(&repo_path, remote_name.as_deref()))
        .await
        .map_err(|e| ArcanaError::Git(format!("fetch join: {e}")))?
}

// ─── Branches ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_branch_list(repo_path: String) -> Result<Vec<BranchInfo>> {
    branch::list(&repo_path)
}

#[tauri::command]
pub fn git_branch_create(repo_path: String, name: String, checkout: Option<bool>) -> Result<()> {
    branch::create(&repo_path, &name, checkout.unwrap_or(false))
}

#[tauri::command]
pub fn git_branch_delete(repo_path: String, name: String, force: Option<bool>) -> Result<()> {
    branch::delete(&repo_path, &name, force.unwrap_or(false))
}

#[tauri::command]
pub fn git_checkout(repo_path: String, refname: String) -> Result<()> {
    branch::checkout(&repo_path, &refname)
}

// ─── Worktrees ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_worktree_list(repo_path: String) -> Result<Vec<WorktreeInfo>> {
    worktree::list(&repo_path)
}

#[tauri::command]
pub fn git_worktree_add(
    repo_path: String,
    path: String,
    branch: Option<String>,
) -> Result<WorktreeInfo> {
    worktree::add(&repo_path, &path, branch.as_deref())
}

#[tauri::command]
pub fn git_worktree_remove(repo_path: String, path: String, force: Option<bool>) -> Result<()> {
    worktree::remove(&repo_path, &path, force.unwrap_or(false))
}

// ─── History ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_log(
    repo_path: String,
    max_count: Option<usize>,
    file: Option<String>,
) -> Result<Vec<CommitInfo>> {
    history::log(&repo_path, max_count, file.as_deref())
}

#[tauri::command]
pub fn git_blame(repo_path: String, file: String, line: Option<usize>) -> Result<Vec<BlameLine>> {
    history::blame(&repo_path, &file, line)
}

// ─── Stash ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn git_stash_list(repo_path: String) -> Result<Vec<StashInfo>> {
    stash::list(&repo_path)
}

#[tauri::command]
pub fn git_stash_push(
    repo_path: String,
    message: Option<String>,
    include_untracked: Option<bool>,
) -> Result<String> {
    stash::push(
        &repo_path,
        message.as_deref(),
        include_untracked.unwrap_or(false),
    )
}

#[tauri::command]
pub fn git_stash_apply(repo_path: String, index: usize, pop: Option<bool>) -> Result<()> {
    stash::apply(&repo_path, index, pop.unwrap_or(false))
}

#[tauri::command]
pub fn git_stash_drop(repo_path: String, index: usize) -> Result<()> {
    stash::drop(&repo_path, index)
}

// ─── Checkpoints ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_checkpoint(
    session_id: String,
    file_path: String,
    reason: String,
) -> Result<Option<CheckpointInfo>> {
    checkpoint::create(&session_id, &file_path, &reason)
}

#[tauri::command]
pub fn restore_checkpoint(checkpoint_id: String) -> Result<RestoreResult> {
    checkpoint::restore(&checkpoint_id)
}

#[tauri::command]
pub fn list_checkpoints(
    file_path: Option<String>,
    session_id: Option<String>,
) -> Result<Vec<CheckpointInfo>> {
    checkpoint::list(file_path.as_deref(), session_id.as_deref())
}

#[tauri::command]
pub fn delete_checkpoint(checkpoint_id: String) -> Result<()> {
    checkpoint::delete(&checkpoint_id)
}

#[tauri::command]
pub fn compare_checkpoints(checkpoint_id_1: String, checkpoint_id_2: String) -> Result<DiffResult> {
    checkpoint::compare(&checkpoint_id_1, &checkpoint_id_2)
}
