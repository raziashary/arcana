// Commit history and blame annotations.

use git2::{BlameOptions, Repository, Sort};

use crate::errors::{AutoDevError, Result};

use super::commit::to_commit_info;
use super::types::{BlameLine, CommitInfo};

/// Walk the commit history starting at HEAD, optionally filtered by file.
pub fn log(repo_path: &str, max_count: Option<usize>, file: Option<&str>) -> Result<Vec<CommitInfo>> {
    let repo = super::repo::open(repo_path)?;
    let mut walk = repo.revwalk().map_err(|e| AutoDevError::Git(format!("revwalk: {e}")))?;
    walk.push_head().map_err(|e| AutoDevError::Git(format!("push_head: {e}")))?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(|e| AutoDevError::Git(format!("sort: {e}")))?;

    let limit = max_count.unwrap_or(100);
    let mut out = Vec::new();

    for oid_res in walk {
        if out.len() >= limit { break; }
        let oid = match oid_res { Ok(o) => o, Err(_) => continue };
        let commit = match repo.find_commit(oid) { Ok(c) => c, Err(_) => continue };

        if let Some(path) = file {
            if !commit_touched_path(&repo, &commit, path) { continue; }
        }
        out.push(to_commit_info(&commit));
    }
    Ok(out)
}

fn commit_touched_path(repo: &Repository, commit: &git2::Commit, path: &str) -> bool {
    let tree = match commit.tree() { Ok(t) => t, Err(_) => return false };
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
        Ok(d) => d,
        Err(_) => return false,
    };
    let mut touched = false;
    let _ = diff.foreach(
        &mut |delta, _| {
            let matched = delta.new_file().path().map(|p| p.to_string_lossy() == path).unwrap_or(false)
                || delta.old_file().path().map(|p| p.to_string_lossy() == path).unwrap_or(false);
            if matched { touched = true; }
            true
        },
        None, None, None,
    );
    touched
}

/// Produce blame annotations for `file`. If `line` is Some(n), the
/// returned vector is restricted to that single line (1-indexed).
pub fn blame(repo_path: &str, file: &str, line: Option<usize>) -> Result<Vec<BlameLine>> {
    let repo = super::repo::open(repo_path)?;

    let mut opts = BlameOptions::new();
    if let Some(n) = line {
        opts.min_line(n).max_line(n);
    }

    let blame = repo
        .blame_file(std::path::Path::new(file), Some(&mut opts))
        .map_err(|e| AutoDevError::Git(format!("blame_file({file}): {e}")))?;

    let content = std::fs::read_to_string(
        repo.workdir()
            .ok_or_else(|| AutoDevError::Git("no workdir".into()))?
            .join(file),
    )
    .unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();

    let mut out = Vec::new();
    for hunk in blame.iter() {
        let start = hunk.final_start_line();
        let count = hunk.lines_in_hunk();
        let sig = hunk.final_signature();

        let time_str = chrono::DateTime::<chrono::Utc>::from_timestamp(sig.when().seconds(), 0)
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();

        for i in 0..count {
            let ln = start + i;
            let text = lines.get(ln.saturating_sub(1)).copied().unwrap_or("").to_string();
            out.push(BlameLine {
                line_number:  ln,
                line:         text,
                commit_oid:   hunk.final_commit_id().to_string(),
                author_name:  sig.name().unwrap_or("").to_string(),
                author_email: sig.email().unwrap_or("").to_string(),
                time:         time_str.clone(),
            });
            if let Some(n) = line { if ln == n { return Ok(out); } }
        }
    }
    Ok(out)
}
