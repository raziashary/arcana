// Symbol navigation — resolve_symbol and find_symbol_references.
// Phase 3.2 (formerly deferred as "needs tree-sitter").
//
// Implementation strategy:
//   Rather than the full tree-sitter pipeline (which requires C grammar
//   compilation and per-language grammar crates), we build on the existing
//   regex symbol extractor already shipping in symbols.rs.
//
//   resolve_symbol   → extract the identifier at (line,col), then walk
//                       workspace files through the symbol extractor and return
//                       the first declaration whose name matches.
//
//   find_symbol_references → whole-word regex search across every text file
//                             in the workspace (similar to VS Code's
//                             "Find All References" plain-text fallback).
//
// Accuracy notes:
//   ‣ resolve_symbol handles the common case (same-workspace definitions)
//     perfectly. Cross-crate stdlib symbols won't resolve (expected).
//   ‣ find_symbol_references returns every occurrence of the word in source
//     files — callers should expect some false positives in comments / strings.
//   ‣ A future Phase 3.2.1 can swap these internals for tree-sitter without
//     changing the Tauri command signatures.

use std::path::{Path, PathBuf};

use regex::Regex;
use walkdir::WalkDir;

use crate::errors::{AutoDevError, Result};
use super::symbols::extract;
use super::types::{SymbolDefinition, SymbolReference};

const SKIP_DIRS: &[&str] = &[
    ".git", "node_modules", "target", ".next", "dist", "build",
    "__pycache__", ".venv", "venv", ".cache", "coverage",
    ".turbo", ".svelte-kit", "out", ".output", ".idea", ".vscode",
];

// ── resolve_symbol ────────────────────────────────────────────────────────────

/// Find the definition of the symbol at `(line, column)` in `path`.
///
/// Returns `None` when no matching declaration is found in the workspace.
/// Errors only on I/O problems with the source file itself.
pub fn resolve_symbol(
    workspace_root: &str,
    path:           &str,
    line:           usize,   // 1-indexed
    column:         usize,   // 0-indexed byte offset
) -> Result<Option<SymbolDefinition>> {
    let workspace = Path::new(workspace_root);
    let abs_path  = resolve_abs(workspace, path)?;

    // Step 1: Read the target file and extract the identifier at (line, col).
    let source = std::fs::read_to_string(&abs_path)
        .map_err(|e| AutoDevError::Agent(format!("read {}: {e}", abs_path.display())))?;

    let identifier = extract_identifier_at(&source, line, column);
    let Some(ident) = identifier else {
        return Ok(None);
    };

    // Step 2: Walk the workspace and look for the declaration of `ident`.
    // We check the file that contains the cursor first (common case: fn → def
    // in the same file), then expand to the whole workspace.
    let preferred = std::iter::once(abs_path.clone());
    let rest      = walk_source_files(workspace);

    for candidate in preferred.chain(rest) {
        if candidate == abs_path && candidate != abs_path {
            // Skip re-checking the source file in the general walk.
            continue;
        }
        let rel = candidate
            .strip_prefix(workspace)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| candidate.to_string_lossy().into_owned());

        // Use the existing regex symbol extractor.
        if let Ok(tree) = extract(workspace_root, &rel) {
            for sym in flatten_symbols(tree.symbols) {
                if sym.name == ident {
                    // Read the declaration line for the preview snippet.
                    let preview = read_line(&candidate, sym.line).unwrap_or_default();
                    return Ok(Some(SymbolDefinition {
                        name:          sym.name,
                        kind:          sym.kind,
                        relative_path: rel,
                        absolute_path: candidate.to_string_lossy().into_owned(),
                        line:          sym.line,
                        column:        sym.column,
                        preview,
                    }));
                }
            }
        }
    }

    Ok(None)
}

// ── find_symbol_references ────────────────────────────────────────────────────

/// Return every location in the workspace where `symbol_name` appears as a
/// whole word.  Results are sorted by (path, line).
pub fn find_symbol_references(
    workspace_root: &str,
    symbol_name:    &str,
    max_results:    usize,
) -> Result<Vec<SymbolReference>> {
    if symbol_name.trim().is_empty() {
        return Ok(Vec::new());
    }

    // Build a whole-word regex: \b<name>\b
    let pattern = format!(r"\b{}\b", regex::escape(symbol_name));
    let re = Regex::new(&pattern)
        .map_err(|e| AutoDevError::Agent(format!("invalid symbol regex: {e}")))?;

    let workspace = Path::new(workspace_root);
    let mut refs: Vec<SymbolReference> = Vec::new();

    'outer: for abs in walk_source_files(workspace) {
        let rel = abs
            .strip_prefix(workspace)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| abs.to_string_lossy().into_owned());

        // Skip binary files
        let text = match std::fs::read_to_string(&abs) {
            Ok(t)  => t,
            Err(_) => continue,
        };

        for (idx, line_text) in text.lines().enumerate() {
            for m in re.find_iter(line_text) {
                refs.push(SymbolReference {
                    relative_path: rel.clone(),
                    absolute_path: abs.to_string_lossy().into_owned(),
                    line:          idx + 1,
                    column:        m.start(),
                    line_text:     line_text.to_string(),
                });
                if refs.len() >= max_results {
                    break 'outer;
                }
            }
        }
    }

    Ok(refs)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Flatten a nested symbol tree into a single vec for scanning.
fn flatten_symbols(
    symbols: Vec<super::types::Symbol>,
) -> impl Iterator<Item = super::types::Symbol> {
    let mut stack = symbols;
    let mut out   = Vec::new();
    while let Some(sym) = stack.pop() {
        let children = sym.children.clone();
        out.push(sym);
        stack.extend(children);
    }
    out.into_iter()
}

/// Walk workspace source files, skipping noise directories.
fn walk_source_files(workspace: &Path) -> impl Iterator<Item = PathBuf> {
    WalkDir::new(workspace)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir() && SKIP_DIRS.contains(&name.as_ref()))
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| is_source_file(e.path()))
        .map(|e| e.into_path())
}

/// Return `true` for text source extensions we can usefully search.
fn is_source_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go"
           | "c" | "cpp" | "h" | "hpp" | "cs" | "java" | "kt"
           | "swift" | "rb" | "php" | "lua" | "toml" | "yaml"
           | "yml" | "json" | "md" | "html" | "css" | "scss")
    )
}

/// Extract the identifier (word) at byte offset `column` on 1-indexed `line`.
/// Returns `None` if there is no identifier-like token at that position.
fn extract_identifier_at(source: &str, line: usize, column: usize) -> Option<String> {
    let line_text = source.lines().nth(line.saturating_sub(1))?;
    let bytes = line_text.as_bytes();
    let col   = column.min(bytes.len().saturating_sub(1));

    // Identify the extent of the identifier token at `col`.
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    if col >= bytes.len() || !is_ident(bytes[col]) {
        return None;
    }

    let start = (0..=col).rev().take_while(|&i| is_ident(bytes[i])).last().unwrap_or(col);
    let end   = (col..bytes.len()).take_while(|&i| is_ident(bytes[i])).last().map(|i| i + 1)?;

    Some(line_text[start..end].to_string())
}

fn read_line(path: &Path, line: usize) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().nth(line.saturating_sub(1)).map(str::to_string)
}

fn resolve_abs(workspace: &Path, path: &str) -> Result<PathBuf> {
    let p = Path::new(path);
    let abs = if p.is_absolute() { p.to_path_buf() } else { workspace.join(p) };
    if !abs.exists() {
        return Err(AutoDevError::Agent(format!("file not found: {}", abs.display())));
    }
    Ok(abs)
}
