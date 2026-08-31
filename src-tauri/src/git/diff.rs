// Diff generation — unified diff (patch) between HEAD, index, and workdir.
//
// Three scopes:
//   - unstaged: workdir vs. index  (what "git diff" shows)
//   - staged:   index vs. HEAD     (what "git diff --staged" shows)
//   - file-scoped variants filter the diff to a single path.

use git2::{Diff, DiffOptions, Repository};

use crate::errors::{AutoDevError, Result};

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
        .map_err(|e| AutoDevError::Git(format!("diff_index_to_workdir: {e}")))
}

fn index_to_head<'a>(repo: &'a Repository, opts: &mut DiffOptions) -> Result<Diff<'a>> {
    // If no HEAD yet (fresh repo), diff against empty tree.
    let head_tree = match repo.head().ok().and_then(|h| h.peel_to_tree().ok()) {
        Some(t) => Some(t),
        None => None,
    };
    repo.diff_tree_to_index(head_tree.as_ref(), None, Some(opts))
        .map_err(|e| AutoDevError::Git(format!("diff_tree_to_index: {e}")))
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
    .map_err(|e| AutoDevError::Git(format!("print_patch: {e}")))?;

    let stats = diff
        .stats()
        .map_err(|e| AutoDevError::Git(format!("diff_stats: {e}")))?;

    Ok(DiffResult {
        patch,
        files_changed: stats.files_changed(),
        insertions: stats.insertions(),
        deletions: stats.deletions(),
    })
}
