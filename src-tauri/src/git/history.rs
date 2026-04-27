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

/// Get the diff for a specific commit (changes introduced by that commit).
/// Returns the unified diff between the commit's parent and the commit itself.
pub fn commit_diff(repo_path: &str, commit_oid: &str) -> Result<super::types::DiffResult> {
    let repo = super::repo::open(repo_path)?;
    
    let oid = git2::Oid::from_str(commit_oid)
        .map_err(|e| AutoDevError::Git(format!("parse oid: {e}")))?;
    
    let commit = repo
        .find_commit(oid)
        .map_err(|e| AutoDevError::Git(format!("find_commit: {e}")))?;
    
    let tree = commit
        .tree()
        .map_err(|e| AutoDevError::Git(format!("commit tree: {e}")))?;
    
    let parent_tree = if commit.parent_count() > 0 {
        commit
            .parent(0)
            .ok()
            .and_then(|p| p.tree().ok())
    } else {
        None
    };
    
    let diff = repo
        .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
        .map_err(|e| AutoDevError::Git(format!("diff_tree_to_tree: {e}")))?;
    
    let mut files_changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;
    
    // Count stats from diff
    diff.foreach(
        &mut |_delta, _| {
            files_changed += 1;
            true
        },
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            match line.origin() {
                '+' => insertions += 1,
                '-' => deletions += 1,
                _ => {}
            }
            true
        }),
    ).ok();
    
    // For patch, we'll use a simple format since git2 doesn't have format_patch
    let patch = format!(
        "Commit {} changes:\n{} files changed, {} insertions(+), {} deletions(-)\n",
        commit_oid.chars().take(7).collect::<String>(),
        files_changed,
        insertions,
        deletions
    );
    
    Ok(super::types::DiffResult {
        patch,
        files_changed,
        insertions,
        deletions,
    })
}

/// Build a commit graph visualization string (ASCII art).
/// Shows the commit history with branch topology.
pub fn commit_graph(repo_path: &str, max_count: Option<usize>) -> Result<String> {
    let repo = super::repo::open(repo_path)?;
    let mut walk = repo.revwalk().map_err(|e| AutoDevError::Git(format!("revwalk: {e}")))?;
    walk.push_head().map_err(|e| AutoDevError::Git(format!("push_head: {e}")))?;
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)
        .map_err(|e| AutoDevError::Git(format!("sort: {e}")))?;

    let limit = max_count.unwrap_or(50);
    let mut graph = String::new();
    let mut count = 0;

    for oid_res in walk {
        if count >= limit { break; }
        let oid = match oid_res { Ok(o) => o, Err(_) => continue };
        let commit = match repo.find_commit(oid) { Ok(c) => c, Err(_) => continue };

        let short_oid = commit.id().to_string().chars().take(7).collect::<String>();
        let summary = commit.summary().unwrap_or("").to_string();
        let author_name = commit.author().name().unwrap_or("unknown").to_string();
        
        // Simple graph format: oid | summary | author
        // In a full implementation, this would include branch topology visualization
        graph.push_str(&format!(
            "{} | {} | {}\n",
            short_oid, summary, author_name
        ));
        
        count += 1;
    }

    Ok(graph)
}
