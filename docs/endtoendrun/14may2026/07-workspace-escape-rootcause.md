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

### Phase 1: Path-jail in builtin_tools (LOAD-BEARING)

- [ ] Create the path-jail helper — new file `src-tauri/src/agent/builtin_tools/path_jail.rs` — export `pub fn validate_workspace_path(target: &Path, workspace: &Path) -> Result<PathBuf, ToolError>` that canonicalizes both inputs (handling non-existent target by canonicalizing its existing parent then re-joining the final component), verifies `canonical_target.starts_with(canonical_workspace)`, and returns the canonical target on success. On failure return a structured error with a self-correcting message: `"Path '{resolved}' is outside the workspace '{workspace}'. Use a workspace-relative path (e.g. 'src/foo.ts') or an absolute path under '{workspace}'. This call has been refused."` Handle Windows path-case insensitivity (`eq_ignore_ascii_case` after lowercase normalization of drive letter) and UNC prefixes (`\\?\`).
- [ ] Wire the jail into `write_file` — `src-tauri/src/agent/builtin_tools/write_file.rs:111-114` — immediately after `resolve_path()` returns, call `validate_workspace_path(&resolved, &ctx.working_dir)?` and use its returned canonical path for the actual `fs::write`. Return the structured error from `ToolResult::error` on failure.
- [ ] Wire the jail into `edit_file` — `src-tauri/src/agent/builtin_tools/edit_file.rs:79-83` — same pattern; call `validate_workspace_path` before any read/diff/write of the target file.
- [ ] Wire the jail into `append_file` — `src-tauri/src/agent/builtin_tools/append_file.rs:62-66` — same pattern; call `validate_workspace_path` before opening the file for append.
- [ ] Wire the jail into `bash` for `cwd` resolution — `src-tauri/src/agent/builtin_tools/bash.rs:152` (the `current_dir(&normalized_path)` call) — validate that the resolved cwd is inside `ctx.working_dir` before spawning the child; reject the call with the structured error if not.
- [ ] Best-effort jail for `bash` redirect targets — `src-tauri/src/agent/builtin_tools/bash.rs` (argument parsing) — scan the command string for `>` / `>>` / `tee` redirect targets and, when statically extractable, validate each via `validate_workspace_path`. Note in code-comment that this is heuristic; non-extractable cases fall through (covered by the cwd jail).
- [ ] Add unit tests for the jail helper — `src-tauri/src/agent/builtin_tools/path_jail.rs` (`#[cfg(test)] mod tests`) — cover: relative path inside workspace (ok), `..` escape attempt (rejected), absolute path inside workspace (ok), absolute path outside workspace (rejected), symlink that points outside (rejected after canonicalize), non-existent target with valid parent (ok), Windows case-difference on drive letter (ok), UNC vs non-UNC normalization.

### Phase 2: Forward `__working_dir` from MCP caller

- [ ] Document the workspace-injection contract — `src-tauri/src/sidecar/client.rs:828-852` (`invoke_worker`) — add a doc-comment block describing that downstream MCP calls SHOULD receive `__working_dir`; note that fixing the Python `mcp_bridge.py:782-785` to inject it is out of this repo's scope (submodule) but the contract is recorded here.
- [ ] Prefer Tauri-stored workspace over `current_dir()` in tool_adapter — `src-tauri/src/mcp_server/tool_adapter.rs:36-42` (and the `__working_dir` lookup near `:123-125`) — when `__working_dir` is missing from args, look up the configured workspace via `workspace_commands::load_workspace_dir()` (or the equivalent `SidecarManager` getter) and use it as the fallback. Only fall through to `std::env::current_dir()` if both sources are unavailable, and emit a `tracing::warn!` when that fallback is hit.
- [ ] Add a startup assertion log line — `src-tauri/src/mcp_server/tool_adapter.rs` (construction) — log the resolved `working_dir` and its source (`"args" | "tauri-stored" | "env-cwd-fallback"`) so log scans like Doc 08's can spot when the load-bearing fallback degrades.
- [ ] Plumb the workspace through the Tauri command if needed — `src-tauri/src/commands/workspace_commands.rs` — verify `load_workspace_dir()` is callable from `mcp_server::tool_adapter` context; if not (lifetime / state-handle scoping), expose it through a `WorkspaceProvider` trait stored alongside the adapter.

### Phase 3: Migration of strays

- [ ] Add a startup-time stray-file scan — `src-tauri/src/sidecar/startup.rs` (or wherever the host's startup phase lives) — walk `src-tauri/` for files NOT in `.gitignore`; warn (`tracing::warn!`) when names match worker-output heuristics (`AUTODEV_PROGRESS.md`, `server.js` at the repo root level, `src/components/*.tsx` if package is Rust-only, etc.).
- [ ] Surface the warning to the UI — `src/components/SidecarCrashBanner.tsx` (or a dedicated startup-warnings panel) — render a dismissible info banner listing each stray with a "move to workspace" or "delete" affordance.
- [ ] Document the cleanup recipe — `docs/endtoendrun/14may2026/07-workspace-escape-rootcause.md` (this doc, in the existing Evidence section) — keep a running list of stray files observed in test runs so future regressions are obvious.

### Phase 4: Defensive UI

- [ ] Add a "workspace mismatch" banner — new component `src/components/WorkspaceMismatchBanner.tsx` — render at the top of the sidecar panel when the Rust-side `current_dir()` differs from the configured workspace at startup.
- [ ] Emit the mismatch state from Rust — `src-tauri/src/sidecar/startup.rs` and `src-tauri/src/events.rs` — compute `current_dir == configured_workspace` at startup and emit a one-shot `workspace:mismatch` event with both paths; frontend subscribes and renders the banner.
- [ ] Add a "Set workspace" CTA on the banner — `src/components/WorkspaceMismatchBanner.tsx` — clicking opens the existing workspace-selection flow so users can fix it without restarting.
