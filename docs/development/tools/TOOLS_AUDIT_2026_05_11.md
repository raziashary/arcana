# AutoDev Builtin Tools & MCP Wrapper — Full Audit

**Date:** 2026-05-11  
**Audited by:** Claude (post-Phase-4.9 full read, cross-referenced with run logs from 5 May and 6 May 2026)  
**Last updated:** 2026-05-11 — fixes applied in same session; see [§9 Fixes Applied](#9-fixes-applied)  
**Files audited:**
- `src-tauri/src/agent/builtin_tools/` — all 19 modules
- `src-tauri/src/mcp_server/` — `mod.rs`, `server.rs`, `tool_adapter.rs`
- `docs/development/analysis/token-explosion-analysis.md`
- `docs/endtoendrun/6may2026/calendar6may.md`

**Purpose:** Root-cause the three known production failure modes:
1. Token blowup (observed: 44K–616K tokens on iteration 4)
2. Fleet producing major code errors (wrong class init, missing requirements.txt packages)
3. Excessive iterations for trivial tasks (15+ tool calls for a single markdown file)

---

## Implementation Status

> All 6 Critical and all 7 High priority items from this audit have been fixed and verified (`cargo check` clean, `npx tsc --noEmit` clean). The new `check_file` tool and `validate.rs` module also close the two Code Quality critical gaps (C-01, C-02, C-03). See [§9 Fixes Applied](#9-fixes-applied) for the full changelog with file-level details.

| Priority tier | Items | Fixed | Remaining |
|---------------|-------|-------|-----------|
| 🔴 Critical (1–6) | 6 | **6/6** ✅ | 0 |
| 🟠 High (7–13) | 7 | **7/7** ✅ | 0 |
| 🟡 Medium (14–22) | 9 | **9/9** ✅ | 0 |
| Cross-cutting proposals | 5 | **5/5** ✅ | 0 |

**All items complete.**

---

## Table of Contents

1. [Summary Matrix — All Bugs Ranked](#1-summary-matrix--all-bugs-ranked)
2. [Token Blowup — Root Causes](#2-token-blowup--root-causes)
3. [Code Quality Failures — Root Causes](#3-code-quality-failures--root-causes)
4. [Excessive Iterations — Root Causes](#4-excessive-iterations--root-causes)
5. [Per-Tool Audit](#5-per-tool-audit)
6. [MCP Wrapper Audit](#6-mcp-wrapper-audit)
7. [Cross-Cutting Issues](#7-cross-cutting-issues)
8. [Recommended Fixes by Priority](#8-recommended-fixes-by-priority)

---

## 1. Summary Matrix — All Bugs Ranked

`✅ Fixed` = resolved and verified in this session. `⬜ Open` = still outstanding.

| ID | File | Severity | Symptom | Category | Status |
|----|------|----------|---------|----------|--------|
| T-01 | `list_tasks_tool.rs` | 🔴 Critical | Returns full task records including description + acceptance_criteria — 50+ tasks = 50K+ chars | Token Blowup | ✅ Fixed |
| T-02 | `read_file.rs` | 🔴 Critical | No file-size guard — reads entire file into memory; a 10MB log file = ~2.5M tokens | Token Blowup | ✅ Fixed |
| T-03 | `grep.rs` | 🔴 Critical | `to_string_pretty` output with context arrays per match — 100 matches × 5 lines × JSON overhead ≈ 10–15K tokens per call | Token Blowup | ✅ Fixed |
| T-04 | `glob.rs` | 🟠 High | 200 results × JSON with size_bytes + modified_at metadata — rarely needed by LLM, adds ~3K tokens per call | Token Blowup | ✅ Fixed |
| T-05 | `list_directory.rs` | 🟠 High | Recursive listing (depth=3) has no output size cap on large repos | Token Blowup | ⬜ Open |
| T-06 | `bash.rs` | 🟠 High | Truncation showed head of output (build progress) instead of tail (actual errors) | Token Blowup | ✅ Fixed |
| C-01 | (none — no tool) | 🔴 Critical | No syntax validator tool — agent writes broken Python then gets `ToolResult::success`; continues thinking code is correct | Code Quality | ✅ Fixed — `validate.rs` + `check_file` tool + Option A in write/edit |
| C-02 | (none — no tool) | 🔴 Critical | No dependency validator — no tool to check if an import exists in `requirements.txt` or `package.json` before writing code that uses it | Code Quality | ✅ Fixed — `check_file` Tier 2 import cross-reference |
| C-03 | `write_file.rs` | 🔴 Critical | `ToolResult::success("Wrote X bytes")` on any write — no validation step; broken code is treated as successfully written | Code Quality | ✅ Fixed — Option A Tier 1 lint appended inline |
| C-04 | `spawn_subagent.rs` | 🟠 High | Default system prompt for subagents has NO working_dir when `task_context` is None AND `system_prompt` is None — subagent operates with wrong/missing root path | Code Quality | ✅ Fixed |
| C-05 | `spawn_parallel_subagents.rs` | 🟠 High | `DEFAULT_SUBAGENT_SYSTEM_PROMPT` has NO working_dir context — parallel subagents have no idea where to write files | Code Quality | ✅ Fixed |
| I-01 | `planner.rs` | 🔴 Critical | `executeTemplate` action listed in description but NOT implemented in `compile()` match — agents calling it get "Unknown action" error and retry | Excessive Iterations | ✅ Fixed — removed from description and `list_actions()` |
| I-02 | `planner.rs` | 🟠 High | `updateStep` uses 0-based `step_index` not `step_id` — agent must count positions; after any step insertion the indices shift and all subsequent calls fail | Excessive Iterations | ✅ Fixed — `step_id` UUID alternative added |
| I-03 | `edit_file.rs` | 🟠 High | Error message on not-found includes first 500 chars of file in the error response — these 500 chars re-enter the context on every failed edit attempt | Excessive Iterations | ✅ Fixed |
| I-04 | `request_supervisor_action.rs` | 🟠 High | `worker_slug` from `ctx.agent_role` is always `None` for MCP calls — all fleet worker requests report `worker_slug = "unknown"`, breaking Supervisor routing | Excessive Iterations | ✅ Fixed — `__agent_role` meta-arg now extracted in MCP adapter |
| I-05 | `update_task_status_tool.rs` | 🟡 Medium | Rejecting a task without `assigned_worker` skips the worker kick; rejected worker is never woken, task sits in Rejected column | Excessive Iterations | ✅ Fixed — reads existing DB value as fallback |
| M-01 | `mcp_server/server.rs` | 🔴 Critical | `tokio::spawn` inside `start_mcp_server` — called from Tauri's synchronous `.setup()` callback — will panic with "no reactor running" (same bug as sidecar/mod.rs:737) | MCP | ✅ Fixed — `tauri::async_runtime::spawn` |
| M-02 | `mcp_server/tool_adapter.rs` | 🔴 Critical | `AutodevMcpServer::new()` sets `working_dir` to `$HOME/.autodev` — this is the config dir, not a project workspace; fleet workers without `__working_dir` silently operate on wrong root | MCP | ✅ Fixed — falls back to `current_dir()` with warn! |
| M-03 | `mcp_server/tool_adapter.rs` | 🟠 High | `agent_role: None` in synthetic `ToolContext` for all MCP calls — `request_supervisor_action` always reports `worker_slug = "unknown"` via MCP | MCP | ✅ Fixed — `__agent_role` support added |
| B-01 | `spawn_subagent.rs` / `spawn_parallel_subagents.rs` | 🔴 Critical | Empty model string forwarded to core-orchestrator → "Model '' not found in registry" → 500 error → SubagentStatus::Failed (documented as BUG-001 in 6may log) | Already documented | ⬜ Open (pre-existing, tracked separately) |
| P-01 | `grep.rs`, `glob.rs` | 🟡 Medium | `serde_json::to_string_pretty` used everywhere — 30–40% larger than compact JSON, purely wasted tokens | Token Blowup | ✅ Fixed — both switched to `to_string()` |
| P-02 | `grep.rs` | 🟡 Medium | Only captures first regex match per line (`regex.find()` not `find_iter()`) — agents miss multi-match lines, re-run grep with adjusted patterns, extra turns | Excessive Iterations | ✅ Fixed — `find_iter()` used |
| P-03 | `planner.rs` | 🟡 Medium | Description string is 790+ chars (the `r#"..."#` block) — injected as tool description on every LLM call; no other tool comes close in description size | Token Blowup | ⬜ Open |
| G-01 | `graph_tools.rs` | 🟡 Medium | `SemanticSearchTool` name is misleading — it does keyword substring matching, not vector/semantic search; agents expecting semantic similarity matching will loop on poor results | Code Quality | ⬜ Open |
| G-02 | `graph_tools.rs` | 🟡 Medium | Every graph tool calls `store.get_all_nodes_for_graph(i64::MAX, 0)?` — loads ALL nodes on every invocation, no pagination, O(N) on graph size | Performance | ⬜ Open |
| G-03 | `graph_tools.rs` | 🟡 Medium | `FindPathTool` uses DFS (stack pop) but comment says BFS — may return non-shortest paths; agents may re-run looking for "shorter" path | Code Quality | ⬜ Open |
| G-04 | `graph_tools.rs` | 🟡 Medium | `BuildGraphTool` does not build anything — it just reads current stats; the name implies construction, leading agents to call it expecting a rebuild | Code Quality | ⬜ Open |

---

## 2. Token Blowup — Root Causes

### 2.1 `list_tasks_tool.rs` — Full Record Verbosity (T-01)

Every call to `list_tasks` returns the FULL task record per task:
```rust
json!({
    "id":                  t.id,
    "title":               t.title,
    "description":         t.description,          // Can be hundreds of words
    "priority":            t.priority.as_str(),
    "task_type":           t.task_type.as_str(),
    "status":              t.status.as_str(),
    "acceptance_criteria": t.acceptance_criteria,   // Can be 200+ words
    "assigned_worker":     t.assigned_worker,
    "rejection_reason":    t.rejection_reason,
    "retry_count":         t.retry_count,
    "created_by":          t.created_by,
    "created_at":          t.created_at,            // Timestamps rarely needed
    "updated_at":          t.updated_at,
    "completed_at":        t.completed_at,
})
```

A board with 20 tasks where each description is 100 words = **~28,000 chars = ~7,000 tokens** from a single `list_tasks` call, before the tool loop even begins. The Supervisor calls `list_tasks` on every tick. At 7 fleet agents × 7K tokens = 49K tokens per fleet cycle just from task listing.

**Fix:** Add a `compact: bool` parameter (default `true`). In compact mode return only `{id, title, status, assigned_worker, priority, retry_count}`. Full details via `get_task(id)`.

### 2.2 `read_file.rs` — No Size Guard (T-02)

```rust
match std::fs::read_to_string(&path) {
    Err(e) => ToolResult::error(...),
    Ok(content) => {
        // slices + formats with line numbers
        let output = selected_lines.iter().enumerate()
            .map(|(i, line)| format!("{:>4} | {}", start_line_num + i, line))
            .collect::<Vec<_>>().join("\n");
        ToolResult::success(output)
    }
}
```

No file-size check before `read_to_string`. An agent reading a compiled binary, a lock file, or a debug log will:
1. Load the entire file into the Rust heap
2. Return the entire content (or slice) as a string
3. Inject it verbatim into the LLM context

For a 5MB `package-lock.json` or Python `__pycache__` file: ~1.25M tokens.

**Fix:** Stat the file first. If `metadata.len() > 256_000` bytes (~64K tokens), refuse with an instructive error: `"File too large to read in full (X MB). Use start_line/end_line to read specific sections, or use grep to search for content."`. The `start_line`/`end_line` mechanism already exists for targeted reads.

### 2.3 `grep.rs` — Verbose JSON Output (T-03)

Each match result includes:
```rust
struct GrepMatch {
    line_number:     usize,
    line_content:    String,
    match_positions: Vec<(usize, usize)>,
    context_before:  Vec<String>,   // default 2 lines
    context_after:   Vec<String>,   // default 2 lines
}
```

With `max_results=100` (default), each match produces:
- 1 matched line
- 2 context_before lines
- 2 context_after lines
- Plus JSON field names, brackets, commas

100 matches at ~5 lines each = 500 lines of source code, pretty-printed in JSON = **roughly 12–18K tokens** from a single `grep` call. Agents doing exploratory searches (e.g., `grep("import")`) will hit this ceiling routinely.

**Fix:** Default `max_results` to 30 (not 100). Default `context_lines` to 1 (not 2). Add an `output_format: "compact" | "full"` option where `compact` returns just `{file, line_number, line_content}` — dropping the `context_before/after` and `match_positions` arrays.

### 2.4 Pretty-Print Overhead (P-01)

Every tool with structured output uses `serde_json::to_string_pretty`:
- `grep.rs` line 250: `serde_json::to_string_pretty(&output)`
- `glob.rs` line 168: `serde_json::to_string_pretty(&output)`
- `list_tasks_tool.rs` line 71: `serde_json::to_string_pretty`... actually uses `.to_string()` on the `json!()` macro output, which is compact. However grep and glob explicitly use `to_string_pretty`.

**Fix:** Switch `grep` and `glob` to `serde_json::to_string()` (compact). Agents don't need human-readable indentation — only the LLM reads this.

### 2.5 `glob.rs` — Unnecessary Metadata (T-04)

```rust
struct GlobFileEntry {
    path: String,
    size_bytes: u64,
    modified_at: String,   // ISO 8601 timestamp
    is_directory: bool,
}
```

LLMs almost never need `size_bytes` or `modified_at` to make decisions. They need `path` and `is_directory`. With 200 results, the timestamp formatting alone adds ~4,000 chars.

**Fix:** Remove `size_bytes` and `modified_at` from default output. Only include them if `include_metadata: bool` is set (default false).

---

## 3. Code Quality Failures — Root Causes

### 3.1 No Code Validation Tools (C-01, C-02)

The most direct cause of the "class initialized inside class", "missing requirements.txt" errors is a structural gap: **no tool validates generated code before marking it as written**.

Currently:
```rust
// write_file.rs
match std::fs::write(&path, content) {
    Ok(_) => ToolResult::success(format!("Wrote {} bytes to '{}'", ...))
    Err(e) => ToolResult::error(...)
}
```

The tool succeeds on any successful filesystem write. It doesn't care if the content is:
- Syntactically invalid Python
- Using undeclared imports
- Duplicating class definitions

The agent receives `✓ Wrote 2,847 bytes to 'app.py'` and continues.

**Missing tools needed:**
1. **`validate_syntax`** — Run `python -c "import ast; ast.parse(open('file').read())"` or `node --check file.js` and return errors as `ToolResult::error` so the agent retries
2. **`check_imports`** — Given a Python file, list imports and cross-reference against `requirements.txt` / installed packages. Return missing packages.

These are simple `bash` wrappers with structured output. They don't require new infrastructure.

**Interim fix (no new tool):** The system prompt / recipe for workers should instruct them to run `python -m py_compile <file>` via `bash` after every `.py` write. This adds ~1 iteration per file but catches the errors before marking task done.

### 3.2 Subagent System Prompt Missing working_dir (C-04, C-05)

In `spawn_subagent.rs`, the system prompt logic:
```rust
let system_prompt = match (
    input.get("task_context").and_then(|v| v.as_str()),
    input.get("system_prompt").and_then(|v| v.as_str()),
) {
    (Some(task_ctx), _) => format!(
        "You are an AutoDev subagent. Working directory: {}.\n...",
        ctx.working_dir, task_ctx         // ✅ working_dir included
    ),
    (None, Some(sp)) => sp.to_string(),   // ❌ no working_dir
    (None, None)     => DEFAULT_SUBAGENT_SYSTEM_PROMPT.to_string(), // ❌ no working_dir
};
```

And the default prompt:
```rust
const DEFAULT_SUBAGENT_SYSTEM_PROMPT: &str =
    "You are an AutoDev subagent — a focused worker spawned by a parent agent \
     to handle a single, well-defined subtask. You have no memory of the \
     parent's conversation. Read the task carefully, do exactly what is \
     asked, and respond with a clear, structured summary...";
```

There is no working directory in this prompt. When a subagent is spawned without `task_context` or a custom `system_prompt`, it has no idea where to resolve file paths. The `bridge.rs` DOES inject `working_dir` as an `[AUTODEV WORKSPACE]` preamble into the system prompt (as fixed in the previous session), but that fix only helps when `config.working_dir` is populated.

In `spawn_parallel_subagents.rs`, the same issue exists — `DEFAULT_SUBAGENT_SYSTEM_PROMPT` never includes working_dir, and the per-subtask `system_prompt` field doesn't get it either.

**Fix:** Unconditionally inject `Working directory: {ctx.working_dir}` into all three branches of the match. The `bridge.rs` preamble is a second-layer safety net, but the system prompt should be authoritative.

### 3.3 `request_supervisor_action` Always Reports `worker_slug = "unknown"` via MCP (I-04, M-03)

In `request_supervisor_action.rs`:
```rust
let worker_slug = ctx.agent_role
    .as_ref()
    .map(|s: &String| s.to_string())
    .unwrap_or_else(|| "unknown".to_string());
```

In `tool_adapter.rs` (the MCP bridge):
```rust
let ctx = ToolContext {
    session_id,
    working_dir,
    call_id: uuid::Uuid::new_v4().to_string(),
    depth:   0,
    agent_role: None,   // ← always None for MCP calls
};
```

Fleet workers calling `request_supervisor_action` via MCP will ALWAYS be stored as `worker_slug = "unknown"`. The Supervisor receiving these requests cannot route them back to the correct worker. The `kick("unknown")` call is a no-op since no recipe is named "unknown".

**Fix:** Fleet workers must pass their slug via `__worker_slug` (similar to `__working_dir` and `__session_id`). Update `tool_adapter.rs::call_tool()` to extract it and populate `ctx.agent_role`.

---

## 4. Excessive Iterations — Root Causes

### 4.1 `planner.rs` — Missing `executeTemplate` Handler (I-01)

The description (the `r#"..."#` block) explicitly lists:
```
- executeTemplate: Create plan from template
```

But the `compile()` match arm has no case for `"executeTemplate"`:
```rust
match action {
    "listActions"       => self.list_actions(),
    "create"            => self.create_plan(&doc),
    // ...
    // NO "executeTemplate" arm
    _ => Err(anyhow::anyhow!(
        "Unknown action: '{action}'. Use 'action: listActions' to discover valid actions."
    )),
}
```

An agent following the tool description, calling `action: executeTemplate`, gets:
```
Planner error: Unknown action: 'executeTemplate'. Use 'action: listActions' to discover valid actions.
```

The agent then calls `listActions`, sees `executeTemplate` listed as valid, and retries. It loops until it gives up or reaches max iterations.

**Fix:** Either implement `executeTemplate` or remove it from the description. Since it's listed in `list_actions()` output too (line 153), both places must be updated.

### 4.2 `planner.rs` — Index-Based Step Update (I-02)

`updateStep` takes a 0-based `step_index`:
```rust
let items = self.db.list_plan_items(&plan_id)?;
let step_index: usize = self.get_usize(doc, "step_index")...;
let item = items.get(step_index).ok_or_else(||...)?;
```

Problems:
1. The LLM must know the exact insertion order of all steps to use the right index
2. If any step was added out of order, the index the LLM uses is wrong
3. No stable identifier — the LLM can't refer to "step added 3 turns ago" reliably

This causes agents to call `getPlan` (get current state), count steps, then call `updateStep`, then verify the right step was updated, then retry if not — easily 4–6 extra iterations per step update.

**Fix:** Add `updateStepById` that takes `step_id` (the UUID from `addStep` output). The `addStep` response includes `item.id` — agents should store that and reference it directly.

### 4.3 `edit_file.rs` — Error Message Injects File Content (I-03)

When `old_string` is not found:
```rust
return ToolResult::error(format!(
    "old_string not found in file. Searched for:\n```\n{}\n```\n\nFile preview (first 500 chars):\n```\n{}\n```",
    params.old_string,
    &content.chars().take(500).collect::<String>()
));
```

The error message includes:
1. The `old_string` the agent was searching for (re-injected into context)
2. 500 chars of the file content (re-injected into context)

On a failed edit, the agent receives all this content. On the next attempt it again provides `old_string` + the tool returns the preview. After 3 failed edits: 3 × (old_string + 500 chars) = ~4,500 additional chars in context.

This is also a subtle feedback loop: the 500-char preview might not contain the section the agent wanted to edit (if the target is beyond char 500). The agent sees the start of the file and may incorrectly conclude the content doesn't exist at all.

**Fix:** Remove the 500-char file preview from the error. Instead, return the line numbers where `old_string` most likely should be (using fuzzy line matching) or suggest using `read_file` with `start_line`/`end_line` to inspect the target area. The agent already has the file if it read it — the preview is redundant.

### 4.4 `update_task_status_tool.rs` — Rejected Task Kick Logic Gap (I-05)

```rust
"rejected"  => {
    if let Some(ref w) = assigned_worker {
        crate::recipes::scheduler::kick(w);
    }
}
```

The `assigned_worker` here is from the CALL INPUT, not from the existing DB record. If the QC agent calls:
```json
{"id": 42, "status": "rejected", "rejection_reason": "Missing error handling"}
```
...without providing `assigned_worker`, the kick is silently skipped. The rejected task sits in the Rejected column indefinitely.

The guardrail at the top requires `assigned_worker` for `assigned` and `in_progress` but NOT for `rejected`. The QC agent's recipe likely doesn't include `assigned_worker` in its rejection call.

**Fix:** For `status="rejected"`, if no `assigned_worker` is provided in the call, look up the existing task from DB and kick whichever worker is already on the record. This makes the kick unconditional for rejections.

---

## 5. Per-Tool Audit

### `read_file.rs`
| Property | Value |
|----------|-------|
| Status | Functional but dangerous for large files |
| Missing | File size guard, binary file detection |
| **Critical Bug** | No size limit before `read_to_string` — can return millions of tokens |
| Minor | `path` description says "Absolute or working-directory-relative" but schema has no constraint — the path must be valid UTF-8, which is not stated |
| Hook | `PreFileRead` fires correctly |

**Immediate fix:** Add before line 76:
```rust
let meta = std::fs::metadata(&path)
    .map_err(|e| ToolResult::error(format!("Cannot stat '{}': {e}", path.display())));
if meta.len() > 256_000 {
    return ToolResult::error(format!(
        "File too large ({} KB). Use start_line/end_line to read specific sections, \
         or use grep to search for content.",
        meta.len() / 1024
    ));
}
```

---

### `write_file.rs`
| Property | Value |
|----------|-------|
| Status | Functional |
| Missing | Content validation, size cap on write |
| Good | Creates parent directories, auto-checkpoints before overwrite |
| Minor | `tokio::spawn` for hook fire is fine in async context (unlike the sidecar case) |

**Recommendation:** Add optional `validate_syntax: bool` parameter. If true, after writing the file, detect language from extension and run a lightweight syntax check via `bash` (e.g., `python -m py_compile file.py`, `node --check file.js`). Return error if syntax check fails — the write is rolled back or flagged.

---

### `edit_file.rs`
| Property | Value |
|----------|-------|
| Status | Functional |
| Bug | Error includes 500-char file preview — re-injects content into context on every failed attempt |
| Good | Multiple-match detection with line numbers is excellent |
| Good | `replace_all` flag is correct |
| Minor | Async `fs::write` after sync `fs::read_to_string` is inconsistent — both should be async |

**Fix:** Remove the `&content.chars().take(500).collect::<String>()` segment from the error message. Replace with:
```
"old_string not found. The file has {line_count} lines. \
 Use read_file(path, start_line=X, end_line=Y) to inspect the target area."
```

---

### `bash.rs`
| Property | Value |
|----------|-------|
| Status | Functional, well-implemented |
| Good | Windows rewriter, structured stdout/stderr labels, exit code feedback |
| Issue | `MAX_OUTPUT_BYTES = 50_000` (50KB ≈ 12K tokens) — a single `npm install` can produce much more; truncation silently cuts important error messages |
| Issue | Truncation indicator `[Output truncated — X bytes total]` is at the END of the truncated string — agents may never see it if they focus on the start of the output |
| Good | Working-dir existence check gives diagnostic error instead of OS gibberish |

**Recommendation:** Move truncation indicator to the BEGINNING of the output, not the end: `[⚠ Output truncated — showing last {MAX_OUTPUT_BYTES} bytes of {total} bytes]\n{tail_of_output}`. The end (errors, test failures) is more useful than the beginning (build progress lines).

---

### `list_directory.rs`
| Property | Value |
|----------|-------|
| Status | Functional |
| Issue | Recursive mode (depth=3) has no output size cap — large monorepos can return thousands of entries |
| Missing | `max_entries` parameter |
| Good | Sorts directories before files, skips noise dirs |
| Minor | `size_bytes` is only included for files (None for dirs) — the schema claims it's always present but it's omitted for dirs |

---

### `grep.rs`
| Property | Value |
|----------|-------|
| Status | Functional but token-expensive |
| Bug | `regex.find()` captures only first match per line — multi-match lines appear as single matches |
| Issue | Default `max_results=100` and `context_lines=2` produces very large output |
| Issue | `serde_json::to_string_pretty` adds ~35% size overhead |
| Good | `(?i)` prefix for case-insensitive (correct approach), truncated flag, skip_dirs |

---

### `glob.rs`
| Property | Value |
|----------|-------|
| Status | Functional |
| Issue | Default `max_results=200` — large result sets |
| Issue | `size_bytes` and `modified_at` metadata in every result — agents rarely need this |
| Issue | `serde_json::to_string_pretty` — unnecessary size inflation |
| Good | Sorts alphabetically, skips noise dirs |

---

### `create_task.rs`
| Property | Value |
|----------|-------|
| Status | ✅ Solid |
| Good | Title length check, push-based dispatch (kicks Supervisor immediately), UI event |
| No issues | — |

---

### `list_tasks_tool.rs`
| Property | Value |
|----------|-------|
| Status | Works but critically verbose |
| **Critical** | Returns `description` + `acceptance_criteria` (both can be 200+ words per task) for every task |
| Missing | `compact` mode, `fields` selection |
| Good | Status enum validation in schema |

---

### `get_task_tool.rs`
| Property | Value |
|----------|-------|
| Status | ✅ Solid |
| Good | Full record is appropriate here — agents call this when they need detail on one specific task |
| No issues | — |

---

### `update_task_status_tool.rs`
| Property | Value |
|----------|-------|
| Status | Mostly solid, one gap |
| Bug | Rejected-task kick depends on input `assigned_worker` being present (not from DB) — QC calling without `assigned_worker` silently skips the wake |
| Good | State-transition guardrails, QC toast event emission, hook system integration |

---

### `assign_task.rs`
| Property | Value |
|----------|-------|
| Status | ✅ Excellent |
| Good | Atomic assign, valid worker validation against `FLEET_WORKERS` constant, push-based dispatch |
| No issues | — |

---

### `planner.rs`
| Property | Value |
|----------|-------|
| Status | Has blocking bugs |
| **Bug** | `executeTemplate` listed in description AND in `list_actions()` output but has NO match arm in `compile()` → "Unknown action" loop |
| **Bug** | `updateStep` uses 0-based index — fragile, no stable identifier |
| Issue | Description string is 790+ chars — largest tool description in the system, injected every tick |
| Issue | `claimItem` and `submitForQC` are fleet-only but available to all modes |
| Good | All other actions work correctly, DB integration is clean |

---

### `request_supervisor_action.rs`
| Property | Value |
|----------|-------|
| Status | Has critical routing bug when called via MCP |
| **Bug** | `worker_slug` from `ctx.agent_role` — always `None` for MCP calls → `worker_slug = "unknown"` |
| Missing | `__worker_slug` parameter support in MCP adapter |
| Good | Clean request types, DB storage, push-based supervisor kick |

---

### `search_memory.rs`
| Property | Value |
|----------|-------|
| Status | Functional |
| Issue | Results include full `content` field which can be large for episodic entries |
| Good | Tier filtering, limit enforcement (max 30) |

---

### `remember_this.rs`
| Property | Value |
|----------|-------|
| Status | ✅ Solid |
| Good | Category tags, confidence=1.0 for user-explicit facts, display truncation |
| No issues | — |

---

### `spawn_subagent.rs`
| Property | Value |
|----------|-------|
| Status | Partially broken |
| **Bug** | Empty model string → core-orchestrator 500 error (BUG-001) |
| **Bug** | Default system prompt has no `working_dir` when `task_context` and `system_prompt` both absent |
| Good | Depth limit (MAX_SUBAGENT_DEPTH=3), structured result return |

---

### `spawn_parallel_subagents.rs`
| Property | Value |
|----------|-------|
| Status | Partially broken |
| **Bug** | Empty model string → BUG-001 |
| **Bug** | `DEFAULT_SUBAGENT_SYSTEM_PROMPT` has no `working_dir` |
| Good | Chunk batching, all-awaited pattern, aggregate summary |

---

### `graph_tools.rs` / `graph_prompts.rs`
| Property | Value |
|----------|-------|
| Status | Not in ToolRegistry — exposed via separate graph MCP pathway |
| Bug | `SemanticSearchTool` does keyword substring matching, not semantic search — misleading name |
| Bug | `BuildGraphTool` reads existing stats, doesn't build anything — misleading name |
| Bug | `FindPathTool` uses DFS (stack.pop) not BFS — comment says BFS |
| Issue | Every tool calls `get_all_nodes_for_graph(i64::MAX, 0)` — loads entire graph on every call |
| Issue | `GetReviewContextTool` calls `get_all_nodes_for_graph(i64::MAX, 0)` N times (once per file) |

---

## 6. MCP Wrapper Audit

### `mcp_server/server.rs`

**Critical — tokio::spawn panic (M-01):**
```rust
pub fn start_mcp_server(registry: Arc<ToolRegistry>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {   // ← PANICS if called from sync .setup() callback
        ...
    })
}
```

`start_mcp_server` uses `tokio::spawn`, which requires an active Tokio runtime context. If this is called from Tauri's synchronous `.setup()` callback (like the `sidecar/mod.rs` watchdog that was just fixed), it will panic with "there is no reactor running".

**Action:** Change to `tauri::async_runtime::spawn` and update the return type to `tauri::async_runtime::JoinHandle<()>`. Verify the call site in `main.rs`.

**Auth middleware:** The current code shows NO auth middleware (comment says "No auth — localhost-only, in-process, zero external exposure"). This matches the BUG-002 fix described in the 6 May log. If this is already cleaned up, confirm and document it as resolved.

---

### `mcp_server/tool_adapter.rs`

**Critical — wrong default working_dir (M-02):**
```rust
impl AutodevMcpServer {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        let working_dir = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(|p| std::path::PathBuf::from(p).join(".autodev").to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string());
        Self { registry, working_dir }
    }
```

This sets `working_dir = "$HOME/.autodev"` (e.g., `C:\Users\Razi\.autodev`). This is the AutoDev config directory, not a user project. Fleet workers that fail to pass `__working_dir` will have all file operations silently directed at the wrong location.

**Fix:** Change fallback to `"."` (process cwd) and add a `warn!` log when the fallback is used:
```rust
.unwrap_or_else(|_| {
    warn!("[autodev-mcp-server] No working_dir configured and $HOME unavailable. \
           Fleet workers MUST pass __working_dir on every tool call.");
    ".".to_string()
});
```

**High — agent_role always None (M-03):**
```rust
let ctx = ToolContext {
    session_id,
    working_dir,
    call_id: uuid::Uuid::new_v4().to_string(),
    depth:   0,
    agent_role: None,    // ← never populated for MCP calls
};
```

All fleet workers calling tools via MCP have `agent_role = None`. The `request_supervisor_action` tool reads this for `worker_slug`. The result: all MCP-originated supervisor requests have `worker_slug = "unknown"`.

**Fix:** Support an `__agent_role` meta-argument (same pattern as `__working_dir` and `__session_id`):
```rust
let agent_role = args.remove("__agent_role")
    .and_then(|v| v.as_str().map(String::from));

let ctx = ToolContext {
    session_id,
    working_dir,
    call_id: uuid::Uuid::new_v4().to_string(),
    depth:   0,
    agent_role,
};
```

---

## 7. Cross-Cutting Issues

### 7.1 No Validation-Write Pattern

The entire tool system has no way to write a file AND validate it atomically. Agents must:
1. Call `write_file` (succeeds, returns "Wrote X bytes")
2. Manually call `bash("python -m py_compile file.py")` 
3. Interpret the output
4. Call `edit_file` if there are errors
5. Repeat

Steps 2–5 are 3+ extra iterations per file. A fleet worker writing 5 Python files = 15+ extra iterations that shouldn't exist.

**Proposed tool:** `write_and_validate(path, content, validator: "python" | "node" | "typescript" | "rust")` — writes the file, runs the appropriate syntax checker, returns success only if both pass. On validation failure, the file is still written (for inspection) but the result is `ToolResult::error`.

### 7.2 No `append_file` Tool

Agents wanting to add a line to `requirements.txt` or `package.json` must:
1. `read_file("requirements.txt")`
2. Construct full new content
3. `write_file("requirements.txt", new_content)`

This is 2 extra turns and a higher risk of accidentally overwriting important content due to context window truncation.

**Proposed tool:** `append_file(path, content)` — appends content to end of file (creating file if needed). Supports a `newline: bool` parameter (default true) to add a trailing newline before appending.

### 7.3 Tool Description Size Comparison

| Tool | Description chars | Estimated tokens |
|------|------------------|-----------------|
| `planner` | ~790 | ~198 |
| `spawn_subagent` | ~380 | ~95 |
| `request_supervisor_action` | ~350 | ~88 |
| `grep` | ~210 | ~53 |
| `bash` (Windows) | ~440 | ~110 |
| `read_file` | ~150 | ~38 |
| `write_file` | ~130 | ~33 |

The `planner` description is nearly 4× the next longest. Since descriptions are injected into every tool discovery context, `planner`'s verbose description adds overhead to every agent tick. Recommend condensing to 200 chars with a pointer to `listActions`.

### 7.4 Missing Tool: `check_requirements`

A tool that takes a list of Python import names and verifies they appear in `requirements.txt` (or are stdlib) would prevent the "importing a library that does not exist in requirements.txt" failures entirely. This is a trivial implementation:

```rust
pub struct CheckRequirementsTool;
// execute: read requirements.txt, parse package names, 
// check each input import against stdlib + requirements list,
// return {missing: [...], found: [...]}
```

### 7.5 All Tool Results Are Treated Equal by the Agent

There is no mechanism for tools to signal "this result is noisy — summarize before using". The LLM receives a 10K-token `grep` result the same way it receives a 10-token `ToolResult::success("✅ Task #42 updated")`. A `hint` field on `ToolResult` that the system prompt instructs the agent to read could help:
```rust
ToolResult {
    output: String,
    is_error: bool,
    hint: Option<String>,   // e.g., "Large result — extract only what you need for the next step"
}
```

---

## 8. Recommended Fixes by Priority

### 🔴 Critical — Fix immediately (blocks fleet functionality or causes panics)

| # | Fix | File | Effort | Status |
|---|-----|------|--------|--------|
| 1 | Fix `tokio::spawn` → `tauri::async_runtime::spawn` in `start_mcp_server` | `mcp_server/server.rs` | 2 min | ✅ Done |
| 2 | Add file size guard in `read_file` before `read_to_string` | `read_file.rs` | 10 min | ✅ Done |
| 3 | Fix `executeTemplate` in `planner.rs` — implement or remove from description AND `list_actions()` | `planner.rs` | 20 min | ✅ Done |
| 4 | Fix `working_dir` fallback in `AutodevMcpServer::new()` — change `$HOME/.autodev` → `"."` with warning | `tool_adapter.rs` | 5 min | ✅ Done |
| 5 | Add `working_dir` to subagent default prompts (all 3 branches in `spawn_subagent.rs`, both tools) | `spawn_subagent.rs`, `spawn_parallel_subagents.rs` | 15 min | ✅ Done |
| 6 | Fix rejected-task kick — read existing `assigned_worker` from DB when input has none | `update_task_status_tool.rs` | 15 min | ✅ Done |

### 🟠 High — Fix in next sprint (directly causes token blowup or iteration loops)

| # | Fix | File | Effort | Status |
|---|-----|------|--------|--------|
| 7 | Add `compact: bool` parameter to `list_tasks` (default true) — return only `{id, title, status, assigned_worker, priority}` | `list_tasks_tool.rs` | 30 min | ✅ Done |
| 8 | Reduce `grep` defaults: `max_results=30`, `context_lines=1`; compact JSON output | `grep.rs` | 45 min | ✅ Done |
| 9 | Remove 500-char file preview from `edit_file` error message | `edit_file.rs` | 5 min | ✅ Done |
| 10 | Add `__agent_role` meta-argument support in `tool_adapter.rs::call_tool()` | `tool_adapter.rs` | 20 min | ✅ Done |
| 11 | Fix `planner.rs::updateStep` — add `step_id` parameter as alternative to `step_index` | `planner.rs` | 45 min | ✅ Done |
| 12 | Remove `size_bytes` + `modified_at` from `glob` default output (add `include_metadata` flag) | `glob.rs` | 15 min | ✅ Done |
| 13 | Fix `grep.rs` — use `find_iter()` instead of `find()` to capture all matches per line | `grep.rs` | 10 min | ✅ Done |

### 🟡 Medium — Fix as part of quality pass

| # | Fix | File | Effort | Status |
|---|-----|------|--------|--------|
| 14 | Switch `grep.rs` and `glob.rs` from `to_string_pretty` to `to_string` | `grep.rs`, `glob.rs` | 5 min | ✅ Done |
| 15 | Move bash truncation to show tail (errors) instead of head (progress noise) | `bash.rs` | 5 min | ✅ Done |
| 16 | Add `append_file` builtin tool | new file | 30 min | ✅ Done |
| 17 | Add syntax validation to `write_file`/`edit_file` + standalone `check_file` tool | `write_file.rs`, `edit_file.rs`, new `check_file.rs`, new `validate.rs` | 60 min | ✅ Done |
| 18 | Condense `planner.rs` description to ≤200 chars | `planner.rs` | 10 min | ✅ Done |
| 19 | Fix `SemanticSearchTool` name → `KeywordSearchTool` | `graph_tools.rs` | 5 min | ✅ Done |
| 20 | Fix `BuildGraphTool` name → `GetGraphStatusTool` | `graph_tools.rs` | 2 min | ✅ Done |
| 21 | Fix `FindPathTool` — implement true BFS (VecDeque + pop_front) for shortest paths | `graph_tools.rs` | 20 min | ✅ Done |
| 22 | Add `max_entries` parameter to `list_directory` | `list_directory.rs` | 10 min | ✅ Done |

### Recipe/System-Prompt mitigations

With the fixes above applied, most of these are now automatically handled. The residual guidance:

1. ~~After every `.py` write, run `bash("python -m py_compile {file}")`~~ — **No longer needed.** `write_file` and `edit_file` now run `py_compile` automatically and surface errors inline.
2. ~~Manually verify imports against `requirements.txt`~~ — **Now handled by `check_file(path, checks=["imports"])`** which cross-references against `requirements.txt`/`package.json` automatically.
3. **When using `planner`:** Do not use `action: executeTemplate` — it has been removed. Use `addStep` + `updateStep` (or `updateStep` with `step_id` for stable references).
4. **Use `get_task(id)` for details:** Call `list_tasks()` (compact by default) to discover tasks, then `get_task(id)` for the full record. `list_tasks(compact=false)` only when you need descriptions for all tasks at once.

---

## 9. Fixes Applied

> Complete changelog of every code change made in this session. All changes verified: `cargo check` → 33 pre-existing warnings, **0 errors**; `npx tsc --noEmit` → **0 errors**.

### `src-tauri/src/mcp_server/server.rs`
- Changed `tokio::spawn` → `tauri::async_runtime::spawn`
- Return type updated from `tokio::task::JoinHandle<()>` → `tauri::async_runtime::JoinHandle<()>`
- **Fixes:** M-01 (panic in `.setup()` callback)

### `src-tauri/src/mcp_server/tool_adapter.rs`
- `AutodevMcpServer::new()`: replaced `$HOME/.autodev` working_dir fallback with `std::env::current_dir()` + `warn!` log
- `call_tool()`: added extraction of `__agent_role` meta-argument and population of `ctx.agent_role`
- Added `use tracing::warn;`
- **Fixes:** M-02 (wrong working_dir), M-03 / I-04 (agent_role always None)

### `src-tauri/src/agent/builtin_tools/read_file.rs`
- Added 256 KB pre-check before `read_to_string` when no `start_line`/`end_line` range is specified
- Returns an instructive error directing agents to use `start_line`/`end_line` or `grep`
- **Fixes:** T-02 (no size guard)

### `src-tauri/src/agent/builtin_tools/edit_file.rs`
- Removed 500-char file preview from "old_string not found" error — replaced with line count + `read_file` hint
- Added `use super::validate;`
- Added Option A post-edit Tier 1 syntax validation: `validate::tier1_syntax()` result appended inline when issues found
- **Fixes:** I-03 (error injects file content), C-03 (no write validation)

### `src-tauri/src/agent/builtin_tools/write_file.rs`
- Added `use super::validate;`
- Added Option A post-write Tier 1 syntax validation: `validate::tier1_syntax()` result appended inline when issues found
- **Fixes:** C-03 (no write validation)

### `src-tauri/src/agent/builtin_tools/planner.rs`
- Removed `executeTemplate` from tool description string
- Removed `executeTemplate` from `list_actions()` output
- Added `step_id` (UUID) as alternative lookup key in `updateStep` — searches items by `item.id` before falling back to `step_index`
- **Fixes:** I-01 (executeTemplate loop), I-02 (index-fragile step updates)

### `src-tauri/src/agent/builtin_tools/list_tasks_tool.rs`
- Added `compact: bool` parameter (default `true`)
- Compact mode returns only `{id, title, priority, task_type, status, assigned_worker, rejection_reason, retry_count}`
- Full mode (existing behavior) returned only when `compact=false`
- Added `hint` field in response pointing to `get_task(id)`
- **Fixes:** T-01 (full record verbosity)

### `src-tauri/src/agent/builtin_tools/grep.rs`
- `max_results` default: 100 → 30
- `context_lines` default: 2 → 1
- `regex.find()` → `regex.find_iter()` to capture all match positions per line
- `serde_json::to_string_pretty` → `serde_json::to_string`
- **Fixes:** T-03 (verbose JSON), P-01 (pretty-print), P-02 (first-match-only)

### `src-tauri/src/agent/builtin_tools/glob.rs`
- `size_bytes` and `modified_at` fields changed to `Option` with `#[serde(skip_serializing_if = "Option::is_none")]`
- Both fields only populated when `include_metadata: bool` is `true` (default `false`)
- `serde_json::to_string_pretty` → `serde_json::to_string`
- **Fixes:** T-04 (unnecessary metadata), P-01 (pretty-print)

### `src-tauri/src/agent/builtin_tools/bash.rs`
- Truncation rewritten to show the **tail** of output (where errors are) instead of the **head** (where progress lines are)
- Truncation indicator moved to the start of the truncated output: `[⚠ Output truncated — showing last N bytes of M bytes total]`
- **Fixes:** T-06 (truncation showed wrong end)

### `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs`
- `"rejected"` kick branch: reads existing `task.assigned_worker` from DB as fallback when `assigned_worker` is absent from input
- **Fixes:** I-05 (rejected task kick silently skipped)

### `src-tauri/src/agent/builtin_tools/spawn_subagent.rs`
- All three branches of the system prompt match now inject `working_dir`:
  - `(Some(task_ctx), _)` — already had it ✓
  - `(None, Some(sp))` — now prepends `[AUTODEV WORKSPACE]\nworking_dir: {working_dir}\n...`
  - `(None, None)` — now prepends `[AUTODEV WORKSPACE]\nworking_dir: {working_dir}\n...`
- **Fixes:** C-04 (subagent missing working_dir)

### `src-tauri/src/agent/builtin_tools/spawn_parallel_subagents.rs`
- `effective_prompt` for every subtask now unconditionally prepends `[AUTODEV WORKSPACE]\nworking_dir: {ctx.working_dir}\n...`
- **Fixes:** C-05 (parallel subagent missing working_dir)

### `src-tauri/src/agent/builtin_tools/validate.rs` *(new file)*
- Shared validation module. Three public tiers:
  - **`tier1_syntax(path, working_dir)`** — async; JSON/YAML/TOML in-process; Python via `py_compile`; JS via `node --check`
  - **`tier2_imports(path, working_dir)`** — sync pure-Rust; Python imports vs `requirements.txt` + stdlib set; JS/TS imports vs `package.json`
  - **`tier3_types(path, working_dir, checker)`** — async subprocess; auto-detects mypy/pyright for Python, tsc for TypeScript, eslint for JS
- Helper functions: `resolve_python()` (venv-aware), `find_upward()` (manifest search), `python_stdlib_set()` (~220 modules), `node_builtin_set()` (~35 modules)
- `LintResult` / `LintDiagnostic` / `ImportResult` types with `format_inline()` for compact tool output
- **Fixes:** C-01, C-02 (no validation tools existed)

### `src-tauri/src/agent/builtin_tools/check_file.rs` *(new file)*
- Option B standalone `check_file` tool registered in both `register_for_mode` and `register_for_mcp_worker`
- Parameters: `path`, `checks` (default `["syntax", "imports"]`), `type_checker` (default `"auto"`)
- Returns `ToolResult::error` when any check finds errors (signals agent to fix), `ToolResult::success` when clean
- **Fixes:** C-01, C-02 (on-demand full diagnostic capability)

### `src-tauri/Cargo.toml`
- Added `toml = "0.8"` dependency for in-process TOML syntax validation in `validate.rs`

### `src-tauri/src/agent/builtin_tools/mod.rs`
- Added `pub mod validate;` and `pub mod check_file;`
- Registered `CheckFileTool` in `register_for_mcp_worker` and in the `FastDevelopment | Review` branch of `register_for_mode`

---

### `src-tauri/src/agent/builtin_tools/append_file.rs` *(new file)*
- Appends content to a file using `tokio::fs::OpenOptions` with `create(true).append(true)`
- Reads only the last byte to detect trailing newlines (avoids reading entire file)
- `newline: bool` parameter (default `true`) ensures content starts on its own line
- Creates parent directories automatically
- Registered in `register_for_mcp_worker` and in `FastDevelopment | Review` branches of `register_for_mode`
- **Fixes:** Item 16 (`append_file` tool gap)

### `src-tauri/src/agent/builtin_tools/planner.rs`
- Description condensed from ~790 chars to ~200 chars — directs agents to `action: listActions` for full schema
- **Fixes:** P-03 (Item 18 — oversized description injected every tick)

### `src-tauri/src/agent/builtin_tools/graph_tools.rs`
- `SemanticSearchTool` → `KeywordSearchTool`; exposed name `semantic_search_nodes` → `keyword_search_nodes`
- `BuildGraphTool` → `GetGraphStatusTool`; exposed name `build_graph` → `get_graph_status`; output keys renamed `nodes_extracted`/`edges_extracted` → `node_count`/`edge_count`
- `FindPathTool`: replaced `Vec + pop()` (DFS) with `VecDeque + pop_front()` (true BFS); comment updated to "BFS — guarantees shortest-hop path"
- **Fixes:** G-01 (Item 19), G-04 (Item 20), G-03 (Item 21)

### `src-tauri/src/agent/builtin_tools/list_directory.rs`
- Added `max_entries: integer` parameter (default 500) to schema
- `collect_entries` bails early when `out.len() >= max_entries`
- Response JSON includes `"truncated": bool` field so agents know output was capped
- **Fixes:** T-05 (Item 22 — unbounded output on large repos)

### `src-tauri/src/agent/builtin_tools/mod.rs`
- Added `pub mod append_file;`
- Registered `AppendFileTool` in `register_for_mcp_worker` and `FastDevelopment | Review` branches

### `src-tauri/src/agent/tool_trait.rs`
- Added `hint: Option<String>` field to `ToolResult` with `#[serde(skip_serializing_if = "Option::is_none")]`
- Added `ToolResult::success_with_hint(output, hint)` convenience constructor
- **Fixes:** Cross-cutting item 7.5 (no mechanism for tools to signal noisy results)

### `src-tauri/src/agent/engine.rs`
- Tool result assembly: when `result.hint` is `Some(hint)`, appends `"\n\n[HINT] {hint}"` to `content` before inserting into LLM history
- Hint is consumed by the LLM as part of the tool result; it does not appear in the raw output stored in the DB or emitted to the UI
- **Fixes:** Cross-cutting item 7.5 (hint wiring into agent loop)

### `src-tauri/src/agent/builtin_tools/grep.rs`
- Attaches `success_with_hint` when result count hits `max_results` cap, directing agent to narrow pattern/path

### `src-tauri/src/agent/builtin_tools/glob.rs`
- Attaches `success_with_hint` when file count hits `max_results` cap

### `src-tauri/src/agent/builtin_tools/list_directory.rs`
- Attaches `success_with_hint` when output is truncated by `max_entries`

### `src-tauri/src/agent/builtin_tools/list_tasks_tool.rs`
- Moved inline JSON `"hint"` key to proper `ToolResult::success_with_hint` channel
- Hint now reaches the LLM via the `[HINT]` inline format, not as a JSON field in the response body

**Verification:** `cargo check` → 33 pre-existing warnings, **0 errors**

---

*End of audit. All line number references are based on the code state as of 2026-05-11. Cross-reference with `docs/endtoendrun/6may2026/calendar6may.md` for runtime confirmation of the MCP 401, model empty-string, and token explosion issues.*
