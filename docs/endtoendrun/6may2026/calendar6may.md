# AutoDev End-to-End Run — 6 May 2026

**Session type**: Live fleet run analysis  
**Log file**: `autodev/logs/autodev-.2026-05-06..log`  
**Analysis date**: 6 May 2026  
**Status**: Issues documented — no code changes applied in this session

---

## Table of Contents

1. [Issue A — Empty Model String in `spawn_parallel_subagents`](#issue-a--empty-model-string-in-spawn_parallel_subagents)
2. [Issue B — MCP Server 401 Unauthorized on Every Tick](#issue-b--mcp-server-401-unauthorized-on-every-tick)
3. [Issue C — Redis / Memurai Connection Refused](#issue-c--redis--memurai-connection-refused)
4. [Issue D — QC Agent UI Verbosity — Toast Notifications Are Silent](#issue-d--qc-agent-ui-verbosity--toast-notifications-are-silent)
5. [Issue E — Rejected Tasks Stuck in Panel — Supervisor Re-routing Gap](#issue-e--rejected-tasks-stuck-in-panel--supervisor-re-routing-gap)
6. [Issue F — Dashboard Auto-Refresh Too Slow / task:changed Not Wired](#issue-f--dashboard-auto-refresh-too-slow--taskchanged-not-wired)
7. [Issue G — High LLM Token Usage per Tick (Extended Prompts)](#issue-g--high-llm-token-usage-per-tick-extended-prompts)
8. [Issue H — AIRSClientManager Unclosed + anyio Cancel Scope Crash](#issue-h--airsclientmanager-unclosed--anyio-cancel-scope-crash)
9. [Issue I — AgentConfigFSM Invalid Transition on Every Invocation](#issue-i--agentconfigfsm-invalid-transition-on-every-invocation)
10. [Atomic Task Breakdown](#atomic-task-breakdown)

---

## Issue A — Empty Model String in `spawn_parallel_subagents`

### Severity: **Critical**

### Observed Behavior
When the QC agent or any worker calls `spawn_parallel_subagents` and the LLM omits the `model` field in a subtask's JSON, the subagent creation call to core-orchestrator returns a 500 error with the message:

```
Model '' not found in registry
```

This causes the spawned subagent to immediately fail with `SubagentStatus::Failed` and the parent agent's multi-file review aborts silently from the caller's perspective.

### Root Cause

In `src-tauri/src/agent/builtin_tools/spawn_parallel_subagents.rs` (line 136):

```rust
model: st.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
```

The `unwrap_or("")` hard-codes an empty string when the LLM does not emit a `model` key for a subtask. The same pattern exists in `spawn_subagent.rs`.

The `SubagentConfig` doc comment at `src-tauri/src/subagent/types.rs` line 33 reads:  
`"LLM model name (registry key, e.g. "claude-4-5-haiku"). Falls back to the default fleet model when empty."`

That fallback is supposed to happen in `manager.rs` or `bridge.rs`, but it is **never implemented** — the empty string is forwarded raw to `client.create_ephemeral_agent()` → `POST /api/agents` → core-orchestrator rejects it at the registry lookup.

### Files Affected
- `src-tauri/src/agent/builtin_tools/spawn_parallel_subagents.rs` (line 136)
- `src-tauri/src/agent/builtin_tools/spawn_subagent.rs` (line 127)
- `src-tauri/src/subagent/manager.rs` (lines 80–88 — other defaults are here but model is missing)

---

## Issue B — MCP Server 401 Unauthorized on Every Tick

### Severity: **Critical**

### Observed Behavior
Every time a fleet agent (supervisor, worker-backend, worker-frontend, worker-mcp, etc.) is invoked via core-orchestrator, the `autodev-mcp-server` connection attempt returns:

```
httpx.HTTPStatusError: Client error '401 Unauthorized' for url 'http://127.0.0.1:8767/mcp'
```

This is followed by:

```
RuntimeError: Attempted to exit cancel scope in a different task than it was entered in
BaseExceptionGroup: unhandled errors in a TaskGroup (2 sub-exceptions)
```

The cascade means: every fleet agent is invoked without any access to AutoDev's MCP tools (`list_tasks`, `create_task`, `assign_task`, etc.). **Critically, the core-orchestrator LivingAgent has zero built-in tools of its own** — `grep` across `core-orchestrator/src/**/*.py` found no native `read_file`, `bash`, `write_file`, `list_tasks`, `create_task`, or `assign_task` implementations. Everything was offloaded to the MCP server. When the 401 hits, the LivingAgent's tool registry is **empty**. It receives instructions to call `list_tasks`, tries to discover tools from MCP, gets 401, and returns a text hallucination like "I don't have access to list_tasks."

`runner.rs` sees `Ok(response)` — HTTP 200 from core-orchestrator — so it **never falls back** to Mode 1 (Rust AgentEngine). The tick is logged as "success" with thousands of milliseconds elapsed. Tokens are burned, nothing is accomplished. **Mode 1 (Rust AgentEngine) is the ONLY working path** — `builtin_tools/mod.rs` registers the full tool set natively — but the fleet never reaches it because the fallback condition (`Err(e)`) is never met when the 401 occurs inside the LivingAgent session.

### Root Cause — Three Layers

**Layer 1 — Unnecessary auth causing token mismatch:**  
The autodev-mcp-server (`src-tauri/src/mcp_server/server.rs`) has a custom `auth_middleware` that validates `Authorization: Bearer <key>` against a UUID stored in `~/.autodev/mcp.key`. The core-orchestrator's Python MCP client constructs the HTTP request but the header is either missing or mismatched. However, **this auth is entirely unnecessary** — the MCP server binds to `127.0.0.1:8767` (localhost only), is an in-process Tokio task inside the Tauri app, and its only client is core-orchestrator's LivingAgent (also a local child process). There is no network boundary to protect. The correct fix is to remove the auth middleware entirely, eliminating the key file, YAML config writing, and the 401 class of errors completely. The token-propagation approach (writing `config/mcp/autodev-mcp-server.yaml` with the bearer key) is a workaround that perpetuates unnecessary complexity.

**Layer 2 — anyio cancel scope bug:**  
When the 401 is raised inside the `streamablehttp_client` async context manager, the MCP Python client library's `anyio` task group teardown crosses task boundaries (`Attempted to exit cancel scope in a different task than it was entered in`). This is a known incompatibility between `anyio` + `httpx` StreamableHTTP transport when the initial request fails with a non-2xx response. The error is unrecoverable per tick — the entire MCP bridge for that invocation is abandoned.

**Layer 3 — runner.rs fallback to Mode 1 never triggers:**  
`runner.rs` only falls back to Mode 1 (Rust AgentEngine) when `orch_client.ensure_agent()` or `orch_client.invoke_worker()` returns `Err(e)` — i.e., the HTTP call to core-orchestrator fails outright (connection refused, timeout). The 401 happens **inside** the LivingAgent session, so `invoke_worker()` returns `Ok(response)` — a valid HTTP 200 with the LivingAgent's text response. The runner logs "LivingAgent tick complete" and considers the tick a success. The fleet agent is completely paralyzed: zero tools, zero task board access, zero file access, burning tokens on every tick.

### Files Affected
- `src-tauri/src/mcp_server/server.rs` — `auth_middleware` (lines 116–138), `load_or_create_mcp_key()`, `write_mcp_server_yaml()` — the source of the 401
- `src-tauri/src/main.rs` — calls `write_mcp_server_yaml()` before spawning core-orchestrator
- `submodules/core-orchestrator/config/mcp/autodev-mcp-server.yaml` — YAML config with `Authorization: Bearer <key>` header (consumed by `MCPClientManager`)
- `submodules/core-orchestrator/src/airs/mcp/` — `MCPClientManager` / `MCPClient` transport layer — where the header should be (but apparently isn't) injected into HTTP requests

---

## Issue C — Redis / Memurai Connection Refused

### Severity: **High**

### Observed Behavior
On every agent invocation, core-orchestrator logs:

```
[REDIS] Failed to connect, proceeding without warm tier (dev mode): Timeout connecting to server
redis.exceptions.ConnectionError: Error 22 connecting to 127.0.0.1:6379.
The remote computer refused the network connection.
```

This means:
1. The warm memory tier (Redis) is unavailable — every agent starts cold.
2. Extended prompt caching is disabled — the 8 assembled extended prompts are rebuilt from disk on every tick.
3. The `AgentFactory` falls back to `Using registry config for claude-4-5-haiku: max_tokens=None, context_window=200000` — which is correct but unoptimized.

### Root Cause

The Memurai/Redis service is not running when AutoDev starts. The auto-start code in `src-tauri/src/sidecar/mod.rs::try_start_memurai_service()` tries:

1. `net start Memurai`
2. `sc start Memurai`
3. `net start redis`

…but this only runs when `check_sidecar_connectivity` detects the `redis_status()` is `SidecarStatus::Stopped`. The auto-start is **fire-and-forget** and logs a warning if all three attempts fail. If Memurai is not installed as a Windows service (e.g. installed as a standalone binary, not registered with SCM), all three commands fail silently and the warm tier is abandoned for the entire session.

### Files Affected
- `src-tauri/src/sidecar/mod.rs` (lines 630–681) — `try_start_memurai_service`
- `src-tauri/src/commands/sidecar_commands.rs` (lines 60–66) — the caller
- `docs/ENVIRONMENT_SETUP.md` — Memurai setup instructions need to address standalone vs service install

---

## Issue D — QC Agent UI Verbosity — Toast Notifications Are Silent

### Severity: **Medium**

### Observed Behavior
When the QC agent fires (every 1 minute), scans tasks in `qc_review`, approves or rejects them, the user sees **nothing** in the UI — no toast, no status badge change, no brief indicator. The only feedback is the next time the Kanban board auto-refreshes (every 3 minutes) and the task changes column.

### Root Cause

The `workerStore.ts` event listener at lines 240–310 only emits a toast when a worker transitions **to** `status: 'error'`. The `worker:status` event from the Rust scheduler (`src-tauri/src/recipes/scheduler.rs`) emits:
- `working` — when a recipe tick fires
- `idle` — when the recipe finishes

Neither of these carries QC-specific context (task ID reviewed, decision: approved/rejected, rejection reason). The toast deduplication uses `workerSlug + status` as a key, so repeated `idle` transitions are silently deduplicated.

The `AgentRoster` in `AutoDevPanel.tsx` updates `taskLabel` on a 1-second tick and shows the worker's current activity string, but this only updates while the recipe is in-flight. Once the recipe completes (idle), the label reverts to "Idle".

### Files Affected
- `src/stores/workerStore.ts` (lines 240–310) — event listener, toast emit logic
- `src/stores/toastStore.ts` — toast shape (needs `detail` field for QC decisions)
- `src/components/ToastStack.tsx` — visual rendering
- `src/tabs/AutoDevPanel.tsx` — `taskLabel` in `AgentRoster`
- `src-tauri/src/events.rs` — `worker:status` event payload (needs QC activity metadata)
- `src-tauri/src/recipes/runner.rs` — where `set_worker_status` is called; QC decision context not emitted

---

## Issue E — Rejected Tasks Stuck in Panel — Supervisor Re-routing Gap

### Severity: **High**

### Observed Behavior
Tasks that the QC agent rejects (`status = 'rejected'`) remain visible in the `Rejected` Kanban column indefinitely and are not automatically re-dispatched within a reasonable timeframe (observed waiting multiple minutes with no re-assignment).

### Root Cause

**The supervisor recipe DOES have the correct logic** (lines 120–123 of `defaults.rs`):

```
**Rejected tasks**
`status='rejected'` → re-dispatch to the SAME worker that originally owned the task.
Call `assign_task(id, original_worker_slug)`.
```

However, three compounding issues prevent this from working reliably:

1. **MCP 401 (Issue B)** — The supervisor LivingAgent session has **zero tools** because the MCP 401 leaves the tool registry empty. The supervisor cannot call `list_tasks`, `assign_task`, or any other tool. It returns a text hallucination. `runner.rs` sees `Ok(response)` and **never falls back** to Mode 1 (Rust AgentEngine), so the supervisor tick burns tokens while doing absolutely nothing. The supervisor never sees rejected tasks, never re-dispatches them.

2. **Mode 1 fallback is dead code for the fleet** — Even if the `invoke_worker()` call returned `Err`, the Rust AgentEngine fallback exists in `runner.rs` lines 365–498. But the fallback is only reachable when core-orchestrator is down (`is_healthy() == false`) or `invoke_worker()` returns an HTTP-level error. The 401 happens inside the LivingAgent session, so neither condition is met. The fleet is permanently stuck in a tool-less Mode 2.

3. **Re-dispatch of rejected tasks relies on `assigned_worker` being preserved** — `update_status_with_reason()` in `tasks.rs` DOES preserve `assigned_worker` in the rejected state. This is correct. The supervisor recipe correctly reads this. The gap is that the supervisor must fire, see the task, and call `assign_task` — which depends on the MCP/LivingAgent path working cleanly (Issue B).

### Files Affected
- `src-tauri/src/recipes/defaults.rs` (supervisor recipe, lines 120–123) — logic is correct
- `src-tauri/src/recipes/runner.rs` — LivingAgent fallback behavior
- `src-tauri/src/db/tasks.rs` (lines 252–275) — `update_status_with_reason` — data is correct
- `src/tabs/AutoDevPanel.tsx` (line 484) — rejected tasks filtered correctly into column
- **Blocked by Issue B** (MCP 401 causes supervisor ticks to be unreliable)

---

## Issue F — Dashboard Auto-Refresh Too Slow / task:changed Not Wired

### Severity: **Medium**

### Observed Behavior
After a worker completes a task or QC approves/rejects, the Kanban board does not update immediately. The user must wait up to 3 minutes (or manually click Refresh) to see the new state.

### Root Cause

**Two separate problems:**

1. **Auto-refresh interval is 180 seconds (3 min), not 60 seconds:**  
   In `src/tabs/AutoDevPanel.tsx` (line 474):
   ```ts
   const id = setInterval(doRefresh, 180_000)
   ```
   This was previously 60 seconds (`60_000`) per the Phase 2.6 memory. It was increased to 180 seconds at some point. For an active fleet run, 3 minutes means a task can go through multiple status transitions before the UI catches up.

2. **`task:changed` event is subscribed in `taskStore.ts` but `initEventListeners` may not be called:**  
   The event-driven path exists (`taskStore.ts::initEventListeners` subscribes to `task:changed`), which should force an immediate `loadTasks()` on every task mutation. If this is wired correctly, the 180s polling is just a safety net. However, in the log, board state appears stale between ticks — suggesting `task:changed` either is not firing or is being swallowed before the React store hydrates.

   The `emit_task_changed` Rust helper in `events.rs` fires from `update_task_status`, `assign_task`, `create_task`. If the Tauri `AppHandle` is not installed (e.g., during a recipe run where the engine runs in a background thread), `emit_*_global()` returns early silently with no log warning.

### Files Affected
- `src/tabs/AutoDevPanel.tsx` (line 474) — interval constant `180_000`
- `src/stores/taskStore.ts` — `initEventListeners` / `task:changed` subscription
- `src-tauri/src/events.rs` — `emit_task_changed_global()` — silent fail if AppHandle not installed
- `src/App.tsx` — `initEventListeners` call on mount (must verify it's called before first tick fires)

---

## Issue G — High LLM Token Usage per Tick (Extended Prompts)

### Severity: **Medium**

### Observed Behavior
From the log (line 1026), every agent invocation logs:
```
Using registry config for claude-4-5-haiku: max_tokens=None, context_window=200000
```

And at lines 1093–1101:
```
Agent 'supervisor' (tags=[...]) → 8 prompts attached:
['agent_identity', 'minimal_environmental_context', 'base_guidelines', 'safety_guidelines',
 'memory_awareness', 'agent_to_agent_communication', 'tool_usage_guidelines', 'output_format']

└─ PREPEND: agent_identity          (457 chars)
└─ PREPEND: minimal_environmental_context (202 chars)
└─ PREPEND: base_guidelines         (606 chars)
└─ PREPEND: safety_guidelines       (457 chars)
└─ PREPEND: memory_awareness        (275 chars)
└─ APPEND:  agent_to_agent_communication (3450 chars)
└─ APPEND:  tool_usage_guidelines   (490 chars)
└─ APPEND:  output_format           (183 chars)
```

Total extended prompt overhead per tick: ~6,120 chars ≈ **~1,500 tokens** added to every invocation, on top of the recipe instructions. With 7 agents firing per minute each contributing this overhead: **~10,500 token overhead/minute** from extended prompts alone, before any actual task context or tool results.

The `agent_to_agent_communication` block alone is 3,450 chars (~860 tokens) and is appended to every fleet agent, even agents like `worker-cleanup` that never communicate with other agents.

Combined with `context_window=200000` and `max_tokens=None`, agents are not bounded on output length, which means they can emit up to the full context window in a single response if not stopped by the `Stop Contract` in the recipe.

### Root Cause
- Extended prompts are assembled from `config/extended_prompts.yaml` — no per-agent tag filtering to suppress irrelevant prompts for simpler workers.
- `max_tokens=None` means no hard output cap beyond the model's default.
- Redis being down (Issue C) means extended prompts are rebuilt from YAML on every tick rather than being served from cache.
- The `base_tokens=100` in SynopsisService (`SynopsisService started (model=gpt-4o-mini, base_tokens=100)`) uses `gpt-4o-mini` for memory synopsis — this is a second LLM call per tick (to OpenAI) that adds cost and latency.

### Files Affected
- `submodules/core-orchestrator/config/extended_prompts.yaml` — `a2a_communication` prompt tag filtering
- `submodules/core-orchestrator/config/synopsis_config.yaml` — `base_tokens=100` and model selection
- `submodules/core-orchestrator/src/airs/config/extended_prompts_loader.py` — tag-based filtering logic
- Issue C (Redis down) exacerbates this — warm cache eliminates per-tick YAML rebuild cost

---

## Issue H — AIRSClientManager Unclosed + anyio Cancel Scope Crash

### Severity: **Medium** (noise-level in logs, but indicative of resource leak)

### Observed Behavior
On every supervisor and worker-mcp invocation:

```
WARNING src.airs.a2a.client: AIRSClientManager was not closed properly. Use 'async with' or call close() explicitly.
```

This warning fires **6 times per supervisor tick**. The same tick then produces the `anyio cancel scope` `RuntimeError` (Issue B). These are linked but distinct:

- The `AIRSClientManager` is being instantiated inside the agent factory without proper async context management — it is not placed in an `async with` block and not explicitly `close()`d before the LivingAgent session ends.
- The `anyio` cancel scope crash occurs specifically in `streamable_http.py` when the MCP 401 causes the `handle_request_async` coroutine to raise `HTTPStatusError`, propagating into a task group that was entered in a different async task context.

### Files Affected
- `submodules/core-orchestrator/src/airs/a2a/client.py` — `AIRSClientManager` lifecycle
- `submodules/core-orchestrator/` — any code that instantiates `AIRSClientManager` without `async with`
- MCP Python package `streamable_http.py` — upstream bug (anyio incompatibility on 401)

---

## Issue I — AgentConfigFSM Invalid Transition on Every Invocation

### Severity: **Low** (recovers gracefully but logs noise)

### Observed Behavior
```
ERROR src.airs.events.v2.event_state_machine: ❌ [FSM] agent_config:supervisor FAILED:
  applying → validating | error=AgentConfigFSM(supervisor): Invalid transition applying → validating
WARNING src.airs.agent.factory: Config FSM validate transition failed for supervisor:
  AgentConfigFSM(supervisor): Invalid transition applying → validating
```

This fires on every invocation of every fleet agent.

### Root Cause
The `AgentConfigFSM` state machine does not allow `applying → validating` as a transition. The factory calls `validate()` while still in the `applying` state. The factory catches the warning and continues (graceful degradation), but the FSM transition is wrong — `applying` should proceed to `applied` before `validating` can be triggered. This is a core-orchestrator bug.

### Files Affected
- `submodules/core-orchestrator/src/airs/events/v2/event_state_machine.py` — FSM transition table
- `submodules/core-orchestrator/src/airs/agent/factory.py` — state machine call sequence

---

## Atomic Task Breakdown

---

### BUG-001 — Fix empty model fallback in subagent spawn tools `[RUST]`

**Module Structure:**
- [x] `src-tauri/src/subagent/manager.rs` — `SubagentManager::spawn()` lines 79–88 — add model default
- [x] `src-tauri/src/agent/builtin_tools/spawn_parallel_subagents.rs` — line 136 (keep `unwrap_or("")`, canonical fallback moves to manager)
- [x] `src-tauri/src/agent/builtin_tools/spawn_subagent.rs` — line 127 (same)

**Code change in `manager.rs`:**
```rust
// After existing defaults block (lines 80–88):
if config.model.is_empty() {
    config.model = self.resolve_default_model();
}
```

Where `resolve_default_model()` reads the active model from `AppState` (or workers.db / settings) rather than hardcoding:
```rust
fn resolve_default_model(&self) -> String {
    // Priority: 1) active_model from AppState, 2) "claude-haiku-4-5" hard fallback
    // Re-use the same resolution logic already present in recipes/runner.rs lines 111–123
    "claude-haiku-4-5".into() // TODO: wire to AppState.active_model
}
```

**Tauri Commands:** None (internal subagent spawn path only).

**Acceptance:** Subagent created with omitted `model` field succeeds; log shows non-empty model in `create_ephemeral_agent` POST body.

---

### BUG-002 — Remove unnecessary auth from autodev-mcp-server `[RUST]`

**Module Structure:**
- [x] `src-tauri/src/mcp_server/server.rs` — delete `auth_middleware` (lines 116–138)
- [x] `src-tauri/src/mcp_server/server.rs` — delete `load_or_create_mcp_key()` (lines 44–61)
- [x] `src-tauri/src/mcp_server/server.rs` — delete `write_mcp_server_yaml()` (lines 66–113)
- [x] `src-tauri/src/main.rs` — remove `write_mcp_server_yaml` call (line 246)
- [x] `src-tauri/src/mcp_server/server.rs` — remove `.route_layer(middleware::from_fn_with_state(...))` (line 186)

**Code change in `server.rs::start_mcp_server()`:**

Current:
```rust
let api_key = load_or_create_mcp_key();
// ...
let router = axum::Router::new()
    .route_service("/mcp", service)
    .route_layer(middleware::from_fn_with_state(api_key, auth_middleware));
```

Replace with:
```rust
let router = axum::Router::new()
    .route_service("/mcp", service);
// No auth — localhost-only, in-process, zero external exposure.
```

**Code change in `main.rs` line 246:**

Delete:
```rust
autodev_lib::mcp_server::server::write_mcp_server_yaml(&orch_cwd);
```

**Alternative (if auth must stay for future external exposure):**
Keep `auth_middleware` but fix token propagation:
- Verify core-orchestrator's `MCPClientManager` reads `config/mcp/autodev-mcp-server.yaml` and injects `Authorization: Bearer <key>` into every `POST /mcp` request header map.
- The 401 in logs means the header is either not read from YAML or not forwarded by the Python `streamablehttp_client` transport.

**Tauri Commands:** None.

**Acceptance:** Zero `401 Unauthorized` for `http://127.0.0.1:8767/mcp` in core-orchestrator logs. `list_tools` discovery returns the Rust builtin tool set.

---

### BUG-002b — Add pre-flight tool capability check in `runner.rs` (safety net) `[RUST]`

**Module Structure:**
- [x] `src-tauri/src/recipes/runner.rs` — after `ensure_agent()` success, before `invoke_worker()`, query tool count
- [x] `src-tauri/src/recipes/runner.rs` — if tool count == 0, force Mode 1 fallback immediately

**Code change in `runner.rs` lines 236–256:**

After:
```rust
match self.orch_client.ensure_agent(worker_slug, &model).await {
    Ok(agent_id) => {
```

Add:
```rust
        // Pre-flight: if LivingAgent has zero tools (MCP 401, broken bridge),
        // skip the expensive invoke and fall back to Mode 1 immediately.
        let tools = self.orch_client.get_agent_tools(&agent_id).await.unwrap_or_default();
        if tools.is_empty() {
            warn!("[RecipeRunner] LivingAgent '{}' has zero tools — forcing Mode 1 fallback", worker_slug);
            emit_worker_tick_end(&self.app, WorkerTickEndPayload {
                slug: worker_slug.to_string(),
                outcome: "mode1_forced".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
            // Fall through to the Mode 1 block below.
        } else {
            // Proceed with invoke_worker as normal.
            match self.orch_client.invoke_worker(&agent_id, ...).await { ... }
            return Ok(());
        }
    }
```

**Tauri Commands:** None.

**Acceptance:** When MCP is broken, runner logs "forcing Mode 1 fallback" and executes the Rust AgentEngine path instead of burning tokens on a tool-less LivingAgent.

---

### BUG-003 — Fix Redis/Memurai auto-start for standalone binary installs `[RUST]`

**Module Structure:**
- [x] `src-tauri/src/sidecar/mod.rs` — `try_start_memurai_service()` lines 630–681 — add binary launch fallback
- [x] `src-tauri/src/sidecar/mod.rs` — `SidecarManager` — add `redis_process: Mutex<Option<Child>>` field to track spawned binary

**Code change in `sidecar/mod.rs::try_start_memurai_service()`:**

After the `#[cfg(target_os = "windows")]` service attempts block (lines 643–663), add:

```rust
// Attempt 4: standalone binary paths
let binary_paths = [
    r"C:\Program Files\Memurai\memurai.exe",
    r"C:\ProgramData\chocolatey\bin\redis-server.exe",
];
for path in binary_paths {
    if std::path::Path::new(path).exists() {
        match std::process::Command::new(path).spawn() {
            Ok(child) => {
                // Store child in SidecarManager so it is reaped on shutdown
                // *self.redis_process.lock().unwrap() = Some(child);
                return true;
            }
            Err(_) => continue,
        }
    }
}
```

**Acceptance:** On standalone Memurai install, log shows `[SidecarManager] Redis/Memurai service started successfully` (or binary path) instead of the `warn!` fallback.

---

### BUG-004 — Emit QC decision events for UI toast verbosity `[RUST]` `[TS]`

**Module Structure (Rust):**
- [x] `src-tauri/src/events.rs` — add `pub const EVT_QC_DECISION: &str = "qc:decision"` (after line 56)
- [x] `src-tauri/src/events.rs` — add `QcDecisionPayload` struct + `emit_qc_decision()` helper
- [x] `src-tauri/src/recipes/runner.rs` — after QC agent `update_status_with_reason()`, emit `qc:decision`

**Code change in `events.rs`:**
```rust
pub const EVT_QC_DECISION: &str = "qc:decision";

#[derive(Debug, Serialize, Clone)]
pub struct QcDecisionPayload {
    pub task_id: i64,
    pub task_title: String,
    pub decision: String, // "approved" | "rejected"
    pub rejection_reason: Option<String>,
    pub worker_slug: String,
}

pub fn emit_qc_decision(app: &AppHandle, payload: QcDecisionPayload) {
    let _ = app.emit(EVT_QC_DECISION, payload);
}
```

**Code change in `recipes/runner.rs` (or QC tool executor):**
After `db.update_status_with_reason(id, "done", ...)` or `"rejected"`:
```rust
if let Some(app) = crate::events::app_handle() {
    let _ = app.emit("qc:decision", QcDecisionPayload {
        task_id: task.id,
        task_title: task.title.clone(),
        decision: status.clone(),
        rejection_reason: task.rejection_reason.clone(),
        worker_slug: "qc-agent".into(),
    });
}
```

**Module Structure (TypeScript):**
- [x] `src/stores/toastStore.ts` — extend `Toast` type with `detail?: string`
- [x] `src/stores/workerStore.ts` — add `listen<QcDecisionPayload>('qc:decision', ...)` handler (inside `initEventListeners`)

**Code change in `workerStore.ts`:**
```typescript
const unlistenQc = await listen<QcDecisionPayload>('qc:decision', (event) => {
    const { task_id, task_title, decision, rejection_reason } = event.payload
    const { addToast } = require('../stores/toastStore')
    if (decision === 'approved') {
        addToast({
            title: `✅ QC passed — Task #${task_id} done`,
            message: task_title,
            type: 'success',
            durationMs: 5000,
        })
    } else {
        addToast({
            title: `⛔ QC rejected — Task #${task_id}`,
            message: rejection_reason?.split('\n')[0] ?? 'See board for details',
            type: 'error',
            durationMs: 10000,
        })
    }
})
_unlistenFns.push(unlistenQc)
```

**Tauri Commands:** None (pure event-driven).

**Acceptance:** Every QC approve/reject fires a visible toast within 2s of DB write.

---

### BUG-005 — Fix dashboard auto-refresh interval + verify event chain `[TS]` `[RUST]`

**Module Structure:**
- [x] `src/tabs/AutoDevPanel.tsx` — line 474 — change `180_000` → `60_000`
- [x] `src-tauri/src/events.rs` — `emit_task_changed_global()` line 215 — add `warn!` when AppHandle missing
- [x] `src/App.tsx` — verify `initEventListeners()` is called in first `useEffect` before any recipe tick (confirmed: `initWorkerEvents()`, `initTaskEvents()`, `initSidecarEvents()` called at lines 61–63 in first `useEffect`)

**Code change in `AutoDevPanel.tsx`:**
```typescript
// Before:
const id = setInterval(doRefresh, 180_000)
// After:
const id = setInterval(doRefresh, 60_000)
```

**Code change in `events.rs`:**
```rust
pub fn emit_task_changed_global(payload: TaskChangedPayload) {
    if let Some(app) = app_handle() {
        emit_task_changed(app, payload);
    } else {
        // Previously silent — now logged so we can detect race conditions
        // where a task DB write happens before the frontend listener mounts.
        tracing::warn!(
            "emit_task_changed_global skipped: AppHandle not installed yet. \
             Task change may not appear in UI until next poll."
        );
    }
}
```

**Acceptance:** Board refreshes at most 60s after any transition. `task:changed` events drive hydration within 500ms when frontend is mounted.

---

### BUG-006 — Fix `AIRSClientManager` lifecycle (unclosed async context) `[PYTHON]`

**Module Structure:**
- [ ] `submodules/core-orchestrator/src/airs/a2a/client.py` — add `__aenter__` / `__aexit__` to `AIRSClientManager`
- [ ] `submodules/core-orchestrator/src/airs/agent/factory.py` — wrap instantiation in `async with`

**Code change (conceptual, verify exact class name in `client.py`):**
```python
class AIRSClientManager:
    async def __aenter__(self):
        return self

    async def __aexit__(self, exc_type, exc, tb):
        await self.close()
```

**Usage site in `factory.py` (or wherever instantiated):**
```python
async with AIRSClientManager(...) as mgr:
    # ... agent invocation ...
    pass
# close() is guaranteed even on exception
```

**Acceptance:** Zero `AIRSClientManager was not closed properly` warnings per tick.

---

### BUG-007 — Fix `AgentConfigFSM` invalid `applying → validating` transition `[PYTHON]`

**Module Structure:**
- [ ] `submodules/core-orchestrator/src/airs/events/v2/event_state_machine.py` — FSM transition table
- [ ] `submodules/core-orchestrator/src/airs/agent/factory.py` — state machine call sequence

**Code change in `factory.py` (preferred):**
Ensure the sequence is `applying → applied → validating → validated`.

```python
# Before (broken):
await config_fsm.applying()
await config_fsm.validate()  # raises Invalid transition applying → validating

# After (fixed):
await config_fsm.applying()
await config_fsm.applied()   # new intermediate state
await config_fsm.validate()
await config_fsm.validated()
```

**Acceptance:** Zero `Invalid transition applying → validating` errors in the log.

---

### PERF-001 — Tag-filter `a2a_communication` prompt for non-communicating workers `[CONFIG]`

**Module Structure:**
- [ ] `submodules/core-orchestrator/config/extended_prompts.yaml` — verify `a2a_communication` block has tag `a2a_communication`
- [ ] `submodules/core-orchestrator/src/airs/config/extended_prompts_loader.py` — ensure tag-based filtering is active

**Change:**
The `agent_to_agent_communication` prompt (3,450 chars) is already tagged. Verify that `worker-cleanup` and `qc-agent` agent configs do NOT include `a2a_communication` in their `capabilities` / `prompt_tags` list. The loader should skip any prompt whose tag is absent from the agent's tag set.

**Acceptance:** `worker-cleanup` and `qc-agent` logs show 6 extended prompts (not 8), saving ~860 tokens/tick.

---

### PERF-002 — Set `max_tokens` hard cap in LLM registry config `[CONFIG]`

**Module Structure:**
- [ ] `submodules/core-orchestrator/config/llm_config.yaml` (or `src/airs/config/llm_registry.yaml`) — set `max_tokens: 4096` for fleet models

**Change:**
```yaml
models:
  claude-haiku-4-5:
    provider: anthropic
    context_window: 200000
    max_tokens: 4096   # <-- add this
```

**Acceptance:** Log shows `max_tokens=4096` instead of `max_tokens=None` for all fleet agents.

---

### DOCS-001 — Update ENVIRONMENT_SETUP.md with Memurai standalone vs service install `[DOCS]`

**Module Structure:**
- [ ] `docs/ENVIRONMENT_SETUP.md` — add "Redis / Memurai Setup" section

**Content:**
1. **Windows Service install** (recommended): `memurai.exe --service-install` → `try_start_memurai_service` auto-starts.
2. **Standalone binary install**: Download portable Memurai or use `choco install redis-64`. Auto-start will attempt binary launch fallback (BUG-003).
3. **Verify**: `redis-cli ping` → `PONG`.
4. **Impact**: Without Redis, extended prompts are rebuilt from YAML on every tick (~1,500 token overhead).

---

## Summary Matrix

| Task | Severity | Layer | Files | One-line fix |
|------|----------|-------|-------|--------------|
| BUG-001 | Critical | Rust | `subagent/manager.rs` | Add `if config.model.is_empty() { config.model = default }` in `spawn()` |
| BUG-002 | Critical | Rust | `mcp_server/server.rs`, `main.rs` | Delete `auth_middleware`, `.route_layer(...)`, `write_mcp_server_yaml` call |
| BUG-002b | Critical | Rust | `recipes/runner.rs` | Pre-flight `get_agent_tools()`; if empty, force Mode 1 fallback |
| BUG-003 | High | Rust | `sidecar/mod.rs` | Add binary path fallback after `net start` / `sc start` attempts |
| BUG-004 | Medium | Rust+TS | `events.rs`, `workerStore.ts` | Add `qc:decision` event + frontend listener → `addToast` |
| BUG-005 | Medium | TS+Rust | `AutoDevPanel.tsx`, `events.rs` | `180_000` → `60_000`; add `warn!` when AppHandle missing |
| BUG-006 | Medium | Python | `a2a/client.py`, `agent/factory.py` | Wrap `AIRSClientManager` in `async with` |
| BUG-007 | Low | Python | `events/v2/event_state_machine.py`, `agent/factory.py` | Insert `applied()` state between `applying()` and `validate()` |
| PERF-001 | Medium | Config | `config/extended_prompts.yaml` | Tag-filter `a2a_communication` for `worker-cleanup` / `qc-agent` |
| PERF-002 | Medium | Config | `config/llm_config.yaml` | Set `max_tokens: 4096` for fleet models |
| DOCS-001 | Low | Docs | `docs/ENVIRONMENT_SETUP.md` | Add Memurai standalone vs service section |

**Fix priority order:**
1. **BUG-002** (remove MCP auth) — single biggest fix; unblocks entire fleet tool access
2. **BUG-002b** (runner.rs pre-flight) — safety net if BUG-002 takes time
3. **BUG-001** (empty model) — unblocks subagent spawning
4. **BUG-003** (Redis binary fallback) — reduces token overhead from cold prompts
5. **BUG-005** (dashboard refresh) — one-line change, immediate UX win
6. **BUG-004** (QC toast) — requires new event but straightforward
7. **PERF-001 + PERF-002** — config-only, low risk
8. **BUG-006 + BUG-007** — Python hygiene, log noise
9. **DOCS-001** — documentation update
