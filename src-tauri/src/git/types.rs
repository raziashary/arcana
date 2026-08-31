// Git shared types — DTOs serialized to the frontend.
// Mirror these in src/stores/gitStore.ts for type parity.

use serde::{Deserialize, Serialize};

// ─── Repository ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub path: String,
    pub workdir: String,
    pub is_bare: bool,
    pub is_empty: bool,
    pub head_branch: Option<String>,
    pub head_oid: Option<String>,
    pub remote_url: Option<String>,
}

// ─── Status ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    /// Staged/indexed new file.
    IndexNew,
    /// Staged modification.
    IndexModified,
    /// Staged deletion.
    IndexDeleted,
    /// Staged rename.
    IndexRenamed,
    /// Worktree new (untracked).
    WorktreeNew,
    /// Worktree modification.
    WorktreeModified,
    /// Worktree deletion.
    WorktreeDeleted,
    /// Merge conflict.
    Conflicted,
    /// Ignored.
    Ignored,
    /// No change.
    Clean,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEntry {
    pub path: String,
    pub status: FileStatus,
    /// True when the file also has worktree changes beyond the indexed ones.
    pub has_worktree_changes: bool,
    /// Original path when renamed; None otherwise.
    pub old_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatusReport {
    pub head_branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Vec<StatusEntry>,
    pub unstaged: Vec<StatusEntry>,
    pub untracked: Vec<StatusEntry>,
    pub conflicted: Vec<StatusEntry>,
}

// ─── Diff ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    /// Unified diff string (patch format).
    pub patch: String,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
}

// ─── Commit ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub oid: String,
    pub short_oid: String,
    pub summary: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    /// Author time (ISO-8601 UTC).
    pub time: String,
    pub parents: Vec<String>,
}

// ─── Branch ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_remote: bool,
    pub is_head: bool,
    pub upstream: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub last_commit: Option<String>,
}

// ─── Worktree ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub is_main: bool,
    pub locked: bool,
}

// ─── Stash ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StashInfo {
    pub index: usize,
    pub oid: String,
    pub message: String,
    pub time: String,
}

// ─── Remote / Pull ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullResult {
    pub fetched: bool,
    pub fast_forward: bool,
    pub conflicts: Vec<String>,
    pub merge_commit: Option<String>,
    pub files_changed: usize,
}

// ─── Blame ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameLine {
    pub line_number: usize,
    pub line: String,
    pub commit_oid: String,
    pub author_name: String,
    pub author_email: String,
    pub time: String,
}

// ─── Checkpoints ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub id: String,
    pub session_id: String,
    pub file_path: String,
    /// Unix ms timestamp when the snapshot was taken.
    pub created_ms: i64,
    pub reason: String,
    pub size_bytes: u64,
    /// Absolute path to the snapshot file on disk.
    pub snapshot_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreResult {
    pub file_path: String,
    pub restored_from: String,
    pub bytes_written: u64,
}
