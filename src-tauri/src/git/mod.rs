// Git module — Phase 3.3.
//
// Thin, synchronous wrapper around git2-rs (libgit2) providing every
// operation the AutoDev UI and agents need:
//   - Repository discovery / init / clone     (repo.rs)
//   - File status (staged / unstaged / etc.)  (status.rs)
//   - Unified diff generation                 (diff.rs)
//   - Stage / unstage / commit with trailers  (commit.rs)
//   - Branch list / create / delete / switch  (branch.rs)
//   - Fetch / push / pull (fast-forward only) (remote.rs)
//   - Merge and conflict resolution           (merge.rs)
//   - Worktree list / add / remove            (worktree.rs)
//   - History (log) and blame annotations     (history.rs)
//   - Stash list / push / apply / drop        (stash.rs)
//   - File checkpointing (snapshots + undo)   (checkpoint.rs)
//
// Errors are surfaced as AutoDevError::Git(String); commands in
// commands/git_commands.rs serialise them for the frontend.

pub mod types;
pub mod repo;
pub mod status;
pub mod diff;
pub mod commit;
pub mod branch;
pub mod remote;
pub mod merge;
pub mod worktree;
pub mod history;
pub mod stash;
pub mod checkpoint;
