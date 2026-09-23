// File reader — encoding detection + optional pagination.
//
// Strategy:
//   1. Read the raw bytes.
//   2. Detect binary by looking for NUL bytes in the first 8KB.
//   3. For text: try UTF-8 first, then UTF-16 LE/BE via BOM sniffing,
//      and finally fall back to lossy UTF-8.
//   4. Apply start_line/end_line pagination (1-indexed, inclusive).
//
// Language detection is done via extension → monaco-id mapping.

use std::path::Path;

use super::types::FileContent;
use crate::errors::{ArcanaError, Result};

const BINARY_SNIFF_LEN: usize = 8192;
const MAX_READ_BYTES: u64 = 50 * 1024 * 1024; // 50 MB hard cap

pub fn read_content(
    workspace_root: &str,
    path: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> Result<FileContent> {
    let workspace = Path::new(workspace_root);
    let abs = resolve(workspace, path)?;

    let meta = std::fs::metadata(&abs)
        .map_err(|e| ArcanaError::Agent(format!("metadata {}: {e}", abs.display())))?;
    if meta.is_dir() {
        return Err(ArcanaError::Agent(format!(
            "Path is a directory, not a file: {}",
            abs.display()
        )));
    }
    if meta.len() > MAX_READ_BYTES {
        return Err(ArcanaError::Agent(format!(
            "File too large ({} bytes); limit is {} bytes",
            meta.len(),
            MAX_READ_BYTES
        )));
    }

    let bytes = std::fs::read(&abs)
        .map_err(|e| ArcanaError::Agent(format!("read {}: {e}", abs.display())))?;

    let (content, encoding, is_binary) = decode(&bytes);

    let relative_path = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| abs.to_string_lossy().into_owned());

    let language = detect_language(&abs);

    // Pagination: only for text content.
    let (content, line_count, effective_start, effective_end) = if is_binary {
        (content, 0usize, None, None)
    } else {
        paginate(&content, start_line, end_line)
    };

    Ok(FileContent {
        absolute_path: abs.to_string_lossy().into_owned(),
        relative_path,
        content,
        encoding,
        is_binary,
        line_count,
        size_bytes: meta.len(),
        start_line: effective_start,
        end_line: effective_end,
        language,
    })
}

// ─── Decoding ─────────────────────────────────────────────────────────────────

fn decode(bytes: &[u8]) -> (String, String, bool) {
    // Binary sniff — presence of a NUL byte in the first 8KB.
    let sniff_len = bytes.len().min(BINARY_SNIFF_LEN);
    if bytes[..sniff_len].contains(&0u8) {
        return (
            format!("<binary file, {} bytes>", bytes.len()),
            "binary".into(),
            true,
        );
    }

    // UTF-16 BOM detection.
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return (decode_utf16_le(&bytes[2..]), "utf-16le".into(), false);
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return (decode_utf16_be(&bytes[2..]), "utf-16be".into(), false);
    }
    // UTF-8 BOM — strip it.
    let payload: &[u8] = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        bytes
    };

    match std::str::from_utf8(payload) {
        Ok(s) => (s.to_string(), "utf-8".into(), false),
        Err(_) => (
            String::from_utf8_lossy(payload).into_owned(),
            "utf-8-lossy".into(),
            false,
        ),
    }
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

// ─── Pagination ───────────────────────────────────────────────────────────────

fn paginate(
    text: &str,
    start_line: Option<usize>,
    end_line: Option<usize>,
) -> (String, usize, Option<usize>, Option<usize>) {
    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();

    if start_line.is_none() && end_line.is_none() {
        return (text.to_string(), total, None, None);
    }

    let start = start_line.unwrap_or(1).max(1);
    let end = end_line.unwrap_or(total).min(total);
    if start > end || start > total {
        return (String::new(), total, Some(start), Some(end));
    }

    let slice = &lines[start - 1..end];
    (slice.join("\n"), total, Some(start), Some(end))
}

// ─── Path + Language ──────────────────────────────────────────────────────────

fn resolve(workspace: &Path, path: &str) -> Result<std::path::PathBuf> {
    let p = Path::new(path);
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workspace.join(p)
    };

    let ws_norm = std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    let abs_norm = std::fs::canonicalize(&abs).unwrap_or_else(|_| abs.clone());
    if !abs_norm.starts_with(&ws_norm) {
        return Err(ArcanaError::Agent(format!(
            "Path is outside workspace: {}",
            abs.display()
        )));
    }
    Ok(abs)
}

/// Map file extension to a monaco-editor language id.
/// Falls back to "plaintext" for unknown types.
pub fn detect_language(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let id = match ext.as_str() {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "typescript",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "javascript",
        "py" | "pyw" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "sh" | "bash" | "zsh" => "shell",
        "ps1" | "psm1" => "powershell",
        "lua" => "lua",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "less" => "less",
        "json" => "json",
        "yml" | "yaml" => "yaml",
        "toml" => "toml",
        "xml" => "xml",
        "md" | "markdown" => "markdown",
        "sql" => "sql",
        "dockerfile" => "dockerfile",
        "graphql" | "gql" => "graphql",
        "vue" => "html",
        "svelte" => "html",
        _ => {
            // Special-case bare filenames.
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_lowercase();
            match name.as_str() {
                "dockerfile" => "dockerfile",
                "makefile" => "makefile",
                "cargo.toml" | "cargo.lock" => "toml",
                _ => "plaintext",
            }
        }
    };
    id.to_string()
}

// ─── File Metadata ────────────────────────────────────────────────────────────

use super::types::{FileMetadata, GitStatus};

pub fn read_metadata(workspace_root: &str, path: &str) -> Result<FileMetadata> {
    let workspace = Path::new(workspace_root);
    let abs = resolve(workspace, path)?;
    let meta = std::fs::metadata(&abs)
        .map_err(|e| ArcanaError::Agent(format!("metadata {}: {e}", abs.display())))?;

    let relative_path = abs
        .strip_prefix(workspace)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| abs.to_string_lossy().into_owned());

    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);

    Ok(FileMetadata {
        absolute_path: abs.to_string_lossy().into_owned(),
        relative_path,
        size_bytes: meta.len(),
        modified_ms,
        is_directory: meta.is_dir(),
        is_readonly: meta.permissions().readonly(),
        git_status: lookup_single_git_status(workspace, &abs),
    })
}

fn lookup_single_git_status(workspace: &Path, abs: &Path) -> Option<GitStatus> {
    let repo = git2::Repository::discover(workspace).ok()?;
    let repo_root = repo.workdir()?.to_path_buf();
    let rel = abs.strip_prefix(&repo_root).ok()?;
    let status = repo.status_file(rel).ok()?;
    Some(super::file_tree::map_git_status(status))
}
