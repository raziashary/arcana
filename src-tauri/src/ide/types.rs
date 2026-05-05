// IDE shared types — DTOs serialized to the frontend.
// Mirror these in src/stores/ideStore.ts so the TS types stay aligned.

use serde::{Deserialize, Serialize};

// ─── File Tree ────────────────────────────────────────────────────────────────

/// A single node in the file tree. Directories carry their children inline
/// up to the depth requested by the caller; deeper directories arrive empty
/// and are lazy-loaded by a follow-up `read_file_tree` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTreeNode {
    pub name:           String,
    /// Path relative to the workspace root (forward-slashes, no leading slash).
    pub relative_path:  String,
    /// Absolute OS path.
    pub absolute_path:  String,
    pub is_directory:   bool,
    pub size_bytes:     Option<u64>,
    /// Unix ms of mtime. None if not available.
    pub modified_ms:    Option<i64>,
    /// Git status for this path (if inside a git repo). None otherwise.
    pub git_status:     Option<GitStatus>,
    /// Children of this directory node (empty for files and lazy-loaded dirs).
    pub children:       Vec<FileTreeNode>,
    /// True when the directory's children were not loaded (depth cut-off).
    pub has_more:       bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitStatus {
    /// Staged for commit (index changes).
    Staged,
    /// Modified in the working tree.
    Modified,
    /// New file that is not tracked by git.
    Untracked,
    /// Deleted in the working tree.
    Deleted,
    /// File is ignored by .gitignore.
    Ignored,
    /// Merge conflict.
    Conflicted,
    /// File is clean / unchanged.
    Clean,
}

// ─── File Content ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub absolute_path: String,
    pub relative_path: String,
    /// UTF-8 decoded content. Binary files return a placeholder string and `is_binary=true`.
    pub content:       String,
    pub encoding:      String,
    pub is_binary:     bool,
    pub line_count:    usize,
    /// Total file size on disk in bytes.
    pub size_bytes:    u64,
    /// First line included when pagination was applied (1-indexed, None = whole file).
    pub start_line:    Option<usize>,
    /// Last line included when pagination was applied (1-indexed, None = whole file).
    pub end_line:      Option<usize>,
    /// Detected language ID (monaco-compatible: "rust", "typescript", …).
    pub language:      String,
}

// ─── File Metadata ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub absolute_path: String,
    pub relative_path: String,
    pub size_bytes:    u64,
    pub modified_ms:   Option<i64>,
    pub is_directory:  bool,
    pub is_readonly:   bool,
    pub git_status:    Option<GitStatus>,
}

// ─── Search ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub name:          String,
    pub relative_path: String,
    pub absolute_path: String,
    pub is_directory:  bool,
    /// Fuzzy-match score (lower = better). Useful for stable sorting.
    pub score:         i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSearchResult {
    pub relative_path: String,
    pub absolute_path: String,
    pub matches:       Vec<ContentMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentMatch {
    /// 1-indexed line number.
    pub line_number: usize,
    pub line_text:   String,
    /// 0-indexed byte offset of the match start within `line_text`.
    pub match_start: usize,
    /// 0-indexed byte offset of the match end (exclusive) within `line_text`.
    pub match_end:   usize,
}

// ─── Symbols ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolTree {
    pub relative_path: String,
    pub language:      String,
    pub symbols:       Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name:        String,
    pub kind:        SymbolKind,
    /// 1-indexed line where the symbol is declared.
    pub line:        usize,
    /// 0-indexed column of the declaration name.
    pub column:      usize,
    pub children:    Vec<Symbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
    Type,
}

// ─── Symbol Navigation (resolve_symbol / find_symbol_references) ──────────────

/// Result of resolve_symbol — the definition site of a named symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolDefinition {
    pub name:          String,
    pub kind:          SymbolKind,
    pub relative_path: String,
    pub absolute_path: String,
    /// 1-indexed declaration line.
    pub line:          usize,
    /// 0-indexed declaration column.
    pub column:        usize,
    /// The raw declaration line text (useful for tooltip previews).
    pub preview:       String,
}

/// A single location where a symbol name appears (may be a call-site, import,
/// type annotation, etc. — not just definitions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolReference {
    pub relative_path: String,
    pub absolute_path: String,
    /// 1-indexed.
    pub line:          usize,
    /// 0-indexed byte start of the match within `line_text`.
    pub column:        usize,
    pub line_text:     String,
}
