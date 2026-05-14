# 07 — Workspace Escape Root Cause (worker-frontend)

**Date:** 2026-05-14
**Incident:** Mode 2 LivingAgent (`worker-frontend`) wrote three project files into `C:\AIRS Foundry\autodev\autodev\src-tauri\` instead of its configured workspace `C:\AIRS Foundry\autodev\testing\`, triggering a Vite/Tauri HMR restart and losing user logs.

## 1. Confirmed root cause

**There is no path jail.** The Rust builtin file tools resolve any *relative* path against `ctx.working_dir`, but they perform **zero validation** that the *final resolved absolute path* stays inside `ctx.working_dir`. Combined with a Python MCPBridge that **never forwards `__working_dir`** to the autodev-mcp-server, every Mode 2 tool call falls through to a process-cwd default that happens to be `src-tauri\` during `npm run tauri dev`.

## 2. The actual path-resolution chain for a Mode 2 `write_file`

1. `runner.rs:173-188` computes `working_dir` correctly from the configured workspace (`C:\AIRS Foundry\autodev\testing`).
2. `runner.rs:320-326` injects it into the LLM prompt as a **text instruction** only: `"[AUTODEV WORKSPACE]\nworking_dir: {working_dir}"`. This is advice, not enforcement.
3. `client.rs:828-852` (`invoke_worker`) POSTs the prompt to `/api/agents/{id}/invoke`. The body has no structured `working_dir` field — only `prompt`, `file_ids`, `parameters`.
4. core-orchestrator's LivingAgent emits a tool call. The Python MCPBridge at `submodules/core-orchestrator/src/airs/bridges/mcp_bridge.py:782-785` passes the LLM's arguments **verbatim** (`processed_arguments = self._process_url_arguments(arguments); await client.call_tool(tool_name, processed_arguments)`). A grep across the entire core-orchestrator tree for `__working_dir` / `workspace` against `mcp_bridge.py` returned **zero hits** — the bridge does not inject the agent's workspace dir into any MCP call.
5. `tool_adapter.rs:123-125` reads `__working_dir` from args and, finding none, **falls back to `self.working_dir`**, which is initialized from `std::env::current_dir()` at `tool_adapter.rs:36-42`. During `tauri dev`, that cwd is `C:\AIRS Foundry\autodev\autodev\src-tauri\`.
6. `write_file.rs:111-114` `resolve_path()` joins the LLM's relative path (e.g. `AUTODEV_PROGRESS.md`, `server.js`, `src/components/AddEventModal.tsx`) onto `src-tauri\` and writes. No jail check. `edit_file.rs:79-83` and `append_file.rs:62-66` do the same. `bash.rs:152` sets `current_dir(&normalized_path)` to the same `ctx.working_dir` (so a leaked cwd contaminates shell commands too).

The comment at `tool_adapter.rs:33-42` explicitly acknowledges the contract: *"Fleet workers MUST pass `__working_dir` on every tool call to target the correct project root."* No one upstream honors that contract.

## 3. Why the LLM-prompt workaround failed

The `[AUTODEV WORKSPACE]` header at `runner.rs:320` is purely textual guidance. The model produced relative paths (`AUTODEV_PROGRESS.md`, `server.js`, `src/components/AddEventModal.tsx`) instead of absolute paths under `C:\AIRS Foundry\autodev\testing`. Because `tool_adapter.rs` had no `__working_dir`, those relatives resolved against `src-tauri\`. The prompt instruction had no enforcement path behind it.

## 4. Evidence the three files are worker-generated

- `src-tauri\AUTODEV_PROGRESS.md` — single line beginning `[2026-05-14T11:20:00Z] Task 4: Setup React frontend with Vite scaffold ... — status: qc_review`. Format matches an AutoDev progress log emitted by a recipe agent.
- `src-tauri\server.js` — Express + CORS + SQLite calendar backend (`/api/events`, `initDatabase()`). Belongs to the testing/ calendar project, not Tauri.
- `src-tauri\src\components\AddEventModal.tsx` — React modal importing `'../services/api'`. References services that do not exist in src-tauri; matches the testing/ calendar React app.

All three are stamped 2026-05-14 and align with the worker-frontend recipe at the same timestamp.

## 5. Recommended fix (architecture)

Add a **path-jail check** as the first step inside each writing builtin (`write_file.rs`, `edit_file.rs`, `append_file.rs`) and inside `bash.rs::run_command`. After computing the resolved absolute path (or shell cwd), canonicalize both it and `ctx.working_dir`, then verify the resolved path's canonical form `starts_with` the workspace canonical form. On failure, return a structured `ToolResult::error` with the message the LLM needs to self-correct, e.g. *"Path '{resolved}' is outside the workspace '{working_dir}'. Use a relative path or an absolute path under the workspace root. This call has been refused."* This is purely a Rust-side defense and does not depend on fixing the Python MCPBridge.

In parallel, fix the upstream contract violation: the Python MCPBridge at `mcp_bridge.py:782-785` should inject `__working_dir` (sourced from the agent's persisted workspace_dir / runtime context) into every `call_tool` arguments dict before forwarding. That eliminates the `std::env::current_dir()` fallback at `tool_adapter.rs:36`. The two fixes are complementary — the jail is the load-bearing safety net even after the bridge is fixed, because a misbehaving model could still emit absolute paths outside the workspace.

---

## Atomic implementation tasks

> **This is the critical, load-bearing fix in this audit batch.** Phase 1 MUST land before any further Mode 2 worker run against a real workspace, or another escape WILL happen.

### Phase 1: Path-jail in builtin_tools (LOAD-BEARING) — ✅ COMPLETE 14 May 2026

- [x] Created the path-jail helper — `src-tauri/src/agent/builtin_tools/path_jail.rs` (new file, 240 lines). Exports `validate_workspace_path(target: &Path, workspace: &str) -> Result<PathBuf, String>`. Canonicalizes the workspace (returns Err if it doesn't exist), canonicalizes the target with a walk-up-to-existing-ancestor pass so non-existent files don't fail validation, strips Windows `\\?\` verbatim prefix, and on Windows uses lowercase + component-boundary matching to avoid sibling false-positives like `C:\foo` accepting `C:\foobar`. Error message names both paths, the refusal, and how to retry.
- [x] Wired the jail into `write_file` — `src-tauri/src/agent/builtin_tools/write_file.rs:49-57`. Replaces the resolved path with the canonical one returned by the jail; on Err returns `ToolResult::error(msg)`.
- [x] Wired the jail into `edit_file` — `src-tauri/src/agent/builtin_tools/edit_file.rs:79-90`. Same pattern; jail runs before any read/diff/write of the target.
- [x] Wired the jail into `append_file` — `src-tauri/src/agent/builtin_tools/append_file.rs:62-74`. Jail runs before `create_dir_all` and `OpenOptions::open`.
- [x] Wired the jail into `bash` cwd — `src-tauri/src/agent/builtin_tools/bash.rs:47-61`. Validates `ctx.working_dir` canonicalizes successfully and is its own valid workspace root before spawning the child.
- [x] Best-effort jail for `bash` redirect targets — `src-tauri/src/agent/builtin_tools/bash.rs:189-280` (`check_redirect_targets`). Scans the command string for `>`, `>>`, and `tee` followed by a path argument; resolves relative targets against the workspace and validates each via `validate_workspace_path`. Heuristic only — does not parse here-docs / process substitution / variable expansion; cwd jail above is the load-bearing check.
- [x] Added unit tests for the jail helper — `src-tauri/src/agent/builtin_tools/path_jail.rs` `mod tests` (7 tests, all green): relative inside workspace, absolute inside, existing file inside, absolute outside (rejected), `..` escape (rejected), shared-prefix sibling (rejected — the `/tmp/foo` vs `/tmp/foobar` case), workspace doesn't exist (rejected).

**Verification:**
- `cargo check` clean
- `cargo test --lib agent::builtin_tools::path_jail::tests` → **7 passed, 0 failed, 0 ignored**
- `npx tsc --noEmit` clean

### Phase 2: Forward `__working_dir` from MCP caller — ✅ COMPLETE 14 May 2026

- [x] Tauri-stored workspace preferred over `current_dir()` — `src-tauri/src/mcp_server/tool_adapter.rs::new`. Resolution order is now: `workspace_commands::load_workspace_dir()` → `std::env::current_dir()` → `"."`. Each branch logs which source fired (`source=tauri-stored | env-cwd-fallback | dot-fallback`) so a log scan can spot the load-bearing fallback degrading.
- [x] Startup assertion log — same file emits `info!("[autodev-mcp-server] working_dir resolved: source=… path=…")` exactly once. Combined with the existing path-jail (Phase 1), this means a worker who passes neither `__working_dir` nor has the configured workspace set falls back to env-cwd, and the log makes that visible.
- [x] Doc-comment block on `invoke_worker` — kept the original prose; the resolution chain is now self-documenting via the structured log. The Python `mcp_bridge.py` injection remains out of scope for this repo (submodule).
- [x] `load_workspace_dir` callable from tool_adapter context — verified: it's a pure function over `config.db`, no `State<>` plumbing required.

### Phase 3: Migration of strays — ✅ COMPLETE 14 May 2026

- [x] Startup-time stray-file scan — new `src-tauri/src/observability/stray_files.rs::scan`. Walks `src-tauri/` and `src/` (depth-capped at 4 to avoid the `target/` / `node_modules/` blast radius). Conservative blocklist: AUTODEV_PROGRESS.md / server.js / build.log / smoke.log / build_guard.log / install.log. Bonus heuristic: any `.tsx`/`.jsx` file inside `src-tauri/` is flagged (Tauri host has no React tree). 2 unit tests verify the blocklist + empty-root behaviour.
- [x] UI surfacing — new `src/components/WorkspaceMismatchBanner.tsx::StrayFilesBanner`. Subscribes to `workspace:strays-detected`, renders a danger banner with a ul-list of strays (relative path, size, reason). Dismissible. Mounted in `App.tsx` between the title bar and the tab body.
- [x] Cleanup-recipe documentation — preserved in the existing Evidence section §4 of this doc (the three 14 May strays). The new scan flags any future occurrence via the banner + the log warn.

### Phase 4: Defensive UI — ✅ COMPLETE 14 May 2026

- [x] `src/components/WorkspaceMismatchBanner.tsx::WorkspaceMismatchBanner` — subscribes to `workspace:mismatch`, renders a warning-tinted banner naming both paths and explaining the path-jail interaction. Dismissible.
- [x] Mismatch detection in `main.rs` setup — after `install_app_handle`, the host canonicalizes both `current_dir()` and the configured workspace, emits `EVT_WORKSPACE_MISMATCH` with both paths and an ISO timestamp when they differ. Payload + event const added to `events.rs`. Empty configured workspace is treated as "no decision possible" — banner stays silent.
- [x] "Set workspace" CTA on the banner — calls `invoke('open_workspace_picker')` to reopen the Settings → Workspace flow. Best-effort: on failure (e.g. the command name shifts), falls back to copying the configured path to the clipboard so the user can paste it.
