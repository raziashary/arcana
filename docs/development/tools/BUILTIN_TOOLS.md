# Built-in Tools Implementation

**Status:** ✅ Phase 1 Complete (7 tools)  
**Date:** 2026-04-09  
**Reference:** Analysis vs Claude Code & Goose

## Overview

Implemented 7 built-in tools for PA mode, reaching feature parity with Claude Code and Goose for core development operations. All tools run natively in Rust for low-latency execution.

## Complete Tool List

| # | Tool | Type | Priority | Status | Description |
|---|------|------|----------|--------|-------------|
| 1 | `read_file` | Read | P0 | ✅ Enhanced | Read files with line numbers |
| 2 | `write_file` | Write | P0 | ✅ Existing | Create/overwrite entire files |
| 3 | `edit_file` | Write | **P0** | ✅ **NEW** | **Precise string replacement** |
| 4 | `bash` | Execute | P0 | ✅ Existing | Run shell commands |
| 5 | `list_directory` | Read | P0 | ✅ Existing | Explore project structure |
| 6 | `grep` | Read | **P0** | ✅ **NEW** | **Search file contents (ripgrep)** |
| 7 | `glob` | Read | P1 | ✅ **NEW** | **Find files by pattern** |

---

## New Tools (P0 - Critical)

### 1. `edit_file` — String Replace Tool

**Why Critical:** LLMs are bad at rewriting entire files but excellent at targeted edits. This is the **primary** file modification tool in both Claude Code and Goose.

**Features:**
- Exact string matching with whitespace/indentation awareness
- Safety: Fails if 0 matches or multiple matches (unless `replace_all=true`)
- Provides helpful error messages with file previews and line numbers
- Tracks line count changes

**Input Schema:**
```json
{
  "path": "src/main.rs",
  "old_string": "fn old_function() {\n    // code\n}",
  "new_string": "fn new_function() {\n    // updated code\n}",
  "replace_all": false  // optional
}
```

**Implementation:** `src-tauri/src/agent/builtin_tools/edit_file.rs`

---

### 2. `grep` — Search File Contents (Regex)

**Why Critical:** Agent needs to **find** code before modifying it. `list_directory` only shows file names.

**Features:**
- Built-in regex search (no external dependencies)
- Cross-platform (Windows, macOS, Linux)
- Regex pattern support
- Context lines (default: 2 before/after)
- Case-sensitive or case-insensitive search
- Glob filtering (e.g., `*.rs`)
- Pagination (default max: 100 results)
- Auto-skips noise directories (node_modules, .git, target, etc.)

**Input Schema:**
```json
{
  "pattern": "async fn execute",
  "path": "src/",  // optional, defaults to cwd
  "case_insensitive": true,  // optional
  "glob": "*.rs",  // optional
  "context_lines": 2,  // optional
  "max_results": 100  // optional
}
```

**Output Format:**
```
Found 3 matches:

src/agent/builtin_tools/edit_file.rs:70:    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
--
src/agent/builtin_tools/grep.rs:89:    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
```

**Implementation:** `src-tauri/src/agent/builtin_tools/grep.rs`

**No external dependencies** — uses Rust's `regex` crate

---

### 3. `glob` — Find Files by Pattern

**Why Useful:** More precise than `list_directory` for finding specific file types or patterns.

**Features:**
- Standard glob patterns (`*.rs`, `**/*.py`, `src/**/test_*.rs`)
- Recursive search
- Auto-skips noise directories (node_modules, .git, target, dist, build, .next, .cache)
- Pagination (default max: 200 results)

**Input Schema:**
```json
{
  "pattern": "**/*.rs",
  "path": "src/",  // optional, defaults to cwd
  "max_results": 200  // optional
}
```

**Output Format:**
```
Found 42 files:

src/agent/builtin_tools/bash.rs
src/agent/builtin_tools/edit_file.rs
src/agent/builtin_tools/glob.rs
...
```

**Implementation:** `src-tauri/src/agent/builtin_tools/glob.rs`

**Dependencies:** `glob = "0.3"`, `walkdir = "2"`

---

## Enhanced Existing Tools

### `read_file` — Added Line Numbers

**What Changed:**
- Output now includes line numbers (e.g., `   1 | use async_trait;`)
- Helps with debugging and provides reference for `edit_file`
- Line ranges still work (start_line, end_line)

**Before:**
```
use async_trait;
use serde::Deserialize;
```

**After:**
```
   1 | use async_trait;
   2 | use serde::Deserialize;
```

**Implementation:** Updated `src-tauri/src/agent/builtin_tools/read_file.rs:55-67`

---

## System Prompt Updates

Updated `BASE_SYSTEM_PROMPT` in `src-tauri/src/agent/context.rs` with:

1. **Complete tool list** with categorization:
   - File Reading & Search (4 tools)
   - File Modification (2 tools)
   - Command Execution (1 tool)

2. **Tool Usage Best Practices:**
   - Always read before editing
   - **Prefer edit_file over write_file** for modifications
   - Use grep to find code before editing
   - Explore with list_directory and glob
   - Test changes with bash

This ensures the LLM knows to use `edit_file` for targeted changes instead of rewriting entire files with `write_file`.

---

## Tool Registration

All 7 tools registered in `src-tauri/src/agent/builtin_tools/mod.rs`:

```rust
pub fn register_all(registry: &mut ToolRegistry) {
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(edit_file::EditFileTool));  // NEW
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
    registry.register(Arc::new(grep::GrepTool));  // NEW
    registry.register(Arc::new(glob::GlobTool));  // NEW
}
```

---

## Dependencies Added

Updated `src-tauri/Cargo.toml`:

```toml
# File operations (for new tools)
glob     = "0.3"
walkdir  = "2"
regex    = "1"
```

**Note:** No external command-line tools required. All tools use pure Rust implementations for maximum portability.

---

## Comparison: AutoDev vs Claude Code vs Goose

| Feature | Claude Code | Goose | AutoDev |
|---------|-------------|-------|---------|
| **Core File Operations** |
| Read file with line numbers | ✅ | ❌ | ✅ |
| Write entire file | ✅ | ✅ | ✅ |
| **Edit file (string replace)** | ✅ | ✅ | ✅ |
| **Search & Discovery** |
| Grep (content search) | ✅ | ✅ | ✅ |
| Glob (file patterns) | ✅ | ❌ | ✅ |
| List directory | ✅ | ✅ (tree) | ✅ |
| **Execution** |
| Shell commands | ✅ | ✅ | ✅ |
| **Advanced (Future)** |
| Web search | ✅ | ❌ | 🔲 (via core-orchestrator) |
| Multi-file edit | ✅ | ❌ | 🔲 (future) |
| Undo/checkpoint | ✅ | ❌ | 🔲 (Phase 3) |

**Result:** ✅ AutoDev now has **feature parity** with Claude Code and Goose for core development tools.

---

## Testing Checklist

- [ ] `edit_file` with single match — should succeed
- [ ] `edit_file` with no match — should error with file preview
- [ ] `edit_file` with multiple matches, replace_all=false — should error with line numbers
- [ ] `edit_file` with multiple matches, replace_all=true — should replace all
- [ ] `grep` with pattern match — should return results with context
- [ ] `grep` with no match — should return "No matches found"
- [ ] `grep` with glob filter — should only search matching files
- [ ] `glob` with pattern — should return matching files
- [ ] `glob` with no match — should return "No files found"
- [ ] `read_file` — should show line numbers
- [ ] System prompt includes all 7 tools and best practices

---

## Known Limitations

1. **Regex performance** — For very large files (>100MB), regex matching may be slow. Consider using `edit_file` with exact strings for better performance.
2. **Large file handling** — `read_file` loads entire file into memory. Consider streaming for files >10MB (future enhancement).
3. **No undo yet** — File checkpointing/undo is planned for Phase 3 (git integration).
4. **Bash tool on Windows** — Paths with spaces must be properly quoted by the LLM. The tool uses `cmd /C` which handles most cases.

---

## Future Enhancements (Phase 2+)

| Priority | Enhancement | Description |
|----------|-------------|-------------|
| P2 | web_search | Use core-orchestrator's internal web_search__internal tool |
| P2 | Bash shell selection | Support GOOSE_SHELL env var (bash/zsh/pwsh) |
| P2 | Structured bash output | Return {stdout, stderr, exit_code} separately |
| P3 | File checkpointing | Git-based undo system (per-message checkpoints) |
| P3 | Multi-file edit | Batch edit operations with single undo point |
| P3 | .gitignore support | Use `ignore` crate instead of hardcoded skip list |

---

## Files Changed

**New Files:**
- `src-tauri/src/agent/builtin_tools/edit_file.rs` (135 lines)
- `src-tauri/src/agent/builtin_tools/grep.rs` (155 lines)
- `src-tauri/src/agent/builtin_tools/glob.rs` (143 lines)

**Modified Files:**
- `src-tauri/src/agent/builtin_tools/mod.rs` — Added 3 tool registrations
- `src-tauri/src/agent/builtin_tools/read_file.rs` — Added line numbers to output
- `src-tauri/src/agent/context.rs` — Updated system prompt with tool list and best practices
- `src-tauri/Cargo.toml` — Added `glob` and `walkdir` dependencies

**Total:** +433 lines of new tool implementations

---

## Phase 1 Status

✅ **COMPLETE** — All P0 tools implemented

**Phase 1 Deliverables:**
- [x] PA Mode Chat with streaming
- [x] Multiple LLM providers (Anthropic, OpenAI, Google)
- [x] Session persistence (SQLite)
- [x] Cost tracking
- [x] **7 built-in tools** (read, write, edit, bash, list, grep, glob)
- [x] Tool permissions system
- [x] Context injection (CLAUDE.md, AGENTS.md)

**Next Phase:** Phase 2 — Recipes + AutoDev Panel + GitHub Integration
