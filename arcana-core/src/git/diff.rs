// Diff generation — unified diff (patch) between HEAD, index, and workdir.
//
// Three scopes:
//   - unstaged: workdir vs. index  (what "git diff" shows)
//   - staged:   index vs. HEAD     (what "git diff --staged" shows)
//   - file-scoped variants filter the diff to a single path.

use git2::{Diff, DiffOptions, Repository};

use crate::errors::{ArcanaError, Result};

use super::types::DiffResult;

/// Build a unified diff. If `file` is Some(path), restrict to that path.
/// If `staged` is true, compare index↔HEAD (what will be committed);
/// otherwise compare workdir↔index (unstaged changes).
pub fn diff(repo_path: &str, file: Option<&str>, staged: bool) -> Result<DiffResult> {
    let repo = super::repo::open(repo_path)?;

    let mut opts = DiffOptions::new();
    opts.context_lines(3).interhunk_lines(1);
    if let Some(path) = file {
        opts.pathspec(path);
    }

    let diff = if staged {
        index_to_head(&repo, &mut opts)?
    } else {
        workdir_to_index(&repo, &mut opts)?
    };

    render(&diff)
}

fn workdir_to_index<'a>(repo: &'a Repository, opts: &mut DiffOptions) -> Result<Diff<'a>> {
    repo.diff_index_to_workdir(None, Some(opts))
        .map_err(|e| ArcanaError::Git(format!("diff_index_to_workdir: {e}")))
}

fn index_to_head<'a>(repo: &'a Repository, opts: &mut DiffOptions) -> Result<Diff<'a>> {
    // If no HEAD yet (fresh repo), diff against empty tree.
    let head_tree = match repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
        Some(t) => Some(t),
        None => None,
    };
    repo.diff_tree_to_index(head_tree.as_ref(), None, Some(opts))
        .map_err(|e| ArcanaError::Git(format!("diff_tree_to_index: {e}")))
}

/// Build a unified diff between two in-memory byte blobs. Used by the
/// checkpoint comparator which has two snapshot files rather than git
/// objects.
pub fn diff_blobs(old: &[u8], new: &[u8]) -> DiffResult {
    let mut patch = String::new();
    let mut insertions = 0usize;
    let mut deletions = 0usize;

    let res = git2::Patch::from_buffers(old, None, new, None, None);
    if let Ok(Some(mut p)) = res.map(Some) {
        if let Ok(buf) = p.to_buf() {
            if let Ok(s) = std::str::from_utf8(&buf) {
                patch.push_str(s);
            }
        }
        if let Ok(stats) = p.line_stats() {
            // line_stats returns (context, additions, deletions)
            insertions = stats.1;
            deletions = stats.2;
        }
    }

    DiffResult {
        patch,
        files_changed: 1,
        insertions,
        deletions,
    }
}

/// Render a libgit2 Diff to a unified patch string and count stats.
fn render(diff: &Diff) -> Result<DiffResult> {
    let mut patch = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => patch.push(line.origin()),
            _ => {}
        }
        if let Ok(s) = std::str::from_utf8(line.content()) {
            patch.push_str(s);
        }
        true
    })
    .map_err(|e| ArcanaError::Git(format!("print_patch: {e}")))?;

    let stats = diff
        .stats()
        .map_err(|e| ArcanaError::Git(format!("diff_stats: {e}")))?;

    Ok(DiffResult {
        patch,
        files_changed: stats.files_changed(),
        insertions: stats.insertions(),
        deletions: stats.deletions(),
    })
}

/// Unified diff between two branches (or any two revspecs).
///
/// Moved here from `commands/diff_commands.rs` in P1-B. It is a pure git
/// operation with no Tauri or state dependency, and it lived under `commands/`
/// only because a Tauri command wrapped it — which made
/// `agent/builtin_tools/analyze_diff_tool.rs` reach into `crate::commands`,
/// one of the four crate cycles blocking the `arcana-core` extraction
/// (ADR-LOG-RESTRUCT-0011). Relocating it removes the edge outright rather
/// than hiding it behind indirection.
pub fn branch_diff(repo_path: &str, base_branch: &str, head_branch: &str) -> Result<String> {
    let repo = git2::Repository::open(repo_path)
        .map_err(|e| ArcanaError::Git(format!("open repo: {e}")))?;

    let base_obj = repo
        .revparse_single(base_branch)
        .map_err(|e| ArcanaError::Git(format!("revparse '{base_branch}': {e}")))?;
    let head_obj = repo
        .revparse_single(head_branch)
        .map_err(|e| ArcanaError::Git(format!("revparse '{head_branch}': {e}")))?;

    let base_commit = base_obj
        .peel_to_commit()
        .map_err(|e| ArcanaError::Git(format!("peel to commit '{base_branch}': {e}")))?;
    let head_commit = head_obj
        .peel_to_commit()
        .map_err(|e| ArcanaError::Git(format!("peel to commit '{head_branch}': {e}")))?;

    let base_tree = base_commit
        .tree()
        .map_err(|e| ArcanaError::Git(format!("base tree: {e}")))?;
    let head_tree = head_commit
        .tree()
        .map_err(|e| ArcanaError::Git(format!("head tree: {e}")))?;

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3).interhunk_lines(1);

    let diff = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut opts))
        .map_err(|e| ArcanaError::Git(format!("diff_tree_to_tree: {e}")))?;

    let mut patch = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => patch.push(line.origin()),
            _ => {}
        }
        if let Ok(s) = std::str::from_utf8(line.content()) {
            patch.push_str(s);
        }
        true
    })
    .map_err(|e| ArcanaError::Git(format!("print patch: {e}")))?;

    Ok(patch)
}
