// Diff parser — converts raw unified diff text into structured change data.
// Handles standard `diff --git` format produced by git2 and CLI git.

use crate::errors::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDiff {
    pub files: Vec<ChangedFile>,
    pub total_lines_added: usize,
    pub total_lines_deleted: usize,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub extension: String,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub is_new_file: bool,
    pub is_deleted_file: bool,
    pub is_renamed: bool,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub added_lines: Vec<String>,
    pub deleted_lines: Vec<String>,
}

pub fn parse_unified_diff(diff_text: &str) -> Result<ParsedDiff> {
    let mut files: Vec<ChangedFile> = Vec::new();

    // Split on `diff --git` section boundaries.
    let sections: Vec<&str> = diff_text.split("\ndiff --git ").collect();

    for (idx, section) in sections.iter().enumerate() {
        // First segment may or may not start with "diff --git "
        let section = if idx == 0 && section.starts_with("diff --git ") {
            &section["diff --git ".len()..]
        } else {
            section
        };

        if section.trim().is_empty() {
            continue;
        }

        let mut path = String::new();
        let mut is_new_file = false;
        let mut is_deleted_file = false;
        let mut is_renamed = false;
        let mut hunks: Vec<DiffHunk> = Vec::new();
        let mut current_hunk: Option<DiffHunk> = None;

        for line in section.lines() {
            if line.starts_with("a/ ") || line.starts_with("b/ ") {
                // Header line like "a/foo.rs b/foo.rs" — extract path from b/ side
                continue;
            }
            if line.starts_with("--- a/") {
                // keep going
                continue;
            }
            if line.starts_with("+++ b/") {
                path = line["+++ b/".len()..].to_string();
                continue;
            }
            if line.starts_with("+++ /dev/null") {
                is_deleted_file = true;
                continue;
            }
            if line.starts_with("--- /dev/null") {
                is_new_file = true;
                continue;
            }
            if line.starts_with("new file mode") {
                is_new_file = true;
                continue;
            }
            if line.starts_with("deleted file mode") {
                is_deleted_file = true;
                continue;
            }
            if line.starts_with("rename ") {
                is_renamed = true;
                continue;
            }
            if line.starts_with("similarity index") {
                is_renamed = true;
                continue;
            }

            // Hunk header
            if line.starts_with("@@") {
                if let Some(h) = current_hunk.take() {
                    hunks.push(h);
                }
                current_hunk = Some(DiffHunk {
                    header: line.to_string(),
                    added_lines: Vec::new(),
                    deleted_lines: Vec::new(),
                });
                continue;
            }

            if let Some(ref mut hunk) = current_hunk {
                if let Some(rest) = line.strip_prefix('+') {
                    hunk.added_lines.push(rest.to_string());
                } else if let Some(rest) = line.strip_prefix('-') {
                    hunk.deleted_lines.push(rest.to_string());
                }
            }
        }

        // If path is still empty, try to parse from the section header "a/foo b/foo"
        if path.is_empty() {
            if let Some(first_line) = section.lines().next() {
                // "a/path/to/file b/path/to/file"
                let parts: Vec<&str> = first_line.splitn(2, " b/").collect();
                if parts.len() == 2 {
                    path = parts[1].to_string();
                } else {
                    path = first_line.to_string();
                }
            }
        }

        if let Some(h) = current_hunk {
            hunks.push(h);
        }

        let lines_added: usize = hunks.iter().map(|h| h.added_lines.len()).sum();
        let lines_deleted: usize = hunks.iter().map(|h| h.deleted_lines.len()).sum();

        let extension = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        if !path.is_empty() {
            files.push(ChangedFile {
                path,
                extension,
                lines_added,
                lines_deleted,
                is_new_file,
                is_deleted_file,
                is_renamed,
                hunks,
            });
        }
    }

    let total_lines_added: usize = files.iter().map(|f| f.lines_added).sum();
    let total_lines_deleted: usize = files.iter().map(|f| f.lines_deleted).sum();
    let file_count = files.len();

    Ok(ParsedDiff {
        files,
        total_lines_added,
        total_lines_deleted,
        file_count,
    })
}
