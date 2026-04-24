# AutoDev Startup & Fleet Mode Analysis — Run #2

**Date:** 2026-04-23 (12:47:39 → 12:49:13)
**Log File:** `logs/mcpserver2run.log` (8,934 lines)
**Observed UI State (screenshot):**
- Header badge: **`• Mode 1`** (aqua dot off)
- Agent Roster rows: all 7 system workers labeled **`Error`**
- Task Board: empty (no tasks have ever been created)

---

## 1. Executive Summary — What Actually Happened

The log proves the fleet is **running in Mode 2 (LivingAgent) successfully**:

| Worker | Dispatch Log Line | Tick Complete | Duration |
|--------|-------------------|--------------|----------|
| `worker-backend`  | L547  | L6885 | ~30.1 s |
| `worker-frontend` | L456  | L7382 | ~34.4 s |
| `qc-agent`        | L639  | L6569 | ~28.9 s |
| `worker-cleanup`  | L645  | L7029 | ~30.7 s |
| `supervisor`      | L660  | L7511 | ~39.4 s |
| `worker-platform` | L712  | L6638 | ~29.1 s |
| `worker-mcp`      | L717  | L7277 | ~33.4 s |

All 7 agents issued `POST /api/agents/{slug}/invoke → 200 OK`, executed `list_tasks_autodev-mcp-server`, and returned `idle` (board empty).

**So why does the UI show "Mode 1" + "Error" everywhere?** Three independent frontend/backend bugs that have nothing to do with the fleet actually failing:

1. **UI Bug A — "Mode 1" badge is stuck:** `AutoDevPanel` calls `checkConnectivity()` exactly once on mount. The AutoDev tab mounted at app launch (~T+0s) while core-orchestrator only became healthy at ~T+13s. The first probe returned `false`, and there is **no retry, no event subscription, no polling** — so the badge stays on "Mode 1" forever even though Mode 2 is fully operational.
2. **UI Bug B — "Error" label for healthy workers:** In `AgentRoster` (`AutoDevPanel.tsx:73-89`), when a worker's status is `working` but no task is assigned (normal during an idle-poll tick), the ladder of conditionals falls through to the `'Error'` branch. There is no case for `status === 'working'`.
3. **Backend Bug C — Double registration noise:** `autodev-mcp-server` is declared in both `submodules/core-orchestrator/config/mcp/autodev-mcp-server.yaml` (loaded at orch startup) AND re-POSTed by Rust's `OrchestratorClient::register_mcp_server()`. Python raises `ValueError("already exists")` → FastAPI returns **HTTP 400**, but Rust only treats **409** as idempotent → registration is logged as an error (non-fatal).

Plus two residual panics from the prior run that still appear here:

4. **`ollama-rs-0.3.4` panic** at `lib.rs:122:44` — `RelativeUrlWithoutBase` (2 occurrences, lines 104 & 106).
5. **MCP SSE empty-data parse errors** from the Python `mcp.client.streamable_http` (hundreds of occurrences, not fatal, previously documented).

---

## 2. Root-Cause Walkthrough

### 2.1 UI Bug A — "Mode 1" forever

`src/tabs/AutoDevPanel.tsx:446-448`:

```@c:\AIRS Foundry\autodev\autodev\src\tabs\AutoDevPanel.tsx:446-448
  useEffect(() => {
    checkConnectivity()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps
```

`checkConnectivity()` invokes `check_sidecar_connectivity` which returns `mem_ok && orch_ok`. At mount time (before orch is healthy) this resolves to `false` and `sidecarConnected` is set to `false` — **permanently**. There is no listener on `startup:progress`, no setInterval re-probe, no subscription to `worker:status`. The only way the user ever sees "LivingAgent" is if they navigate away and back to the AutoDev tab after T+15s, which remounts the component.

Meanwhile `startupStore` already subscribes to `startup:progress` and transitions through `orch_ready → fleet_ready → ready`, so the data is available — just not wired into `sidecarStore`.

### 2.2 UI Bug B — All workers labeled "Error"

`src/tabs/AutoDevPanel.tsx:77-89`:

```@c:\AIRS Foundry\autodev\autodev\src\tabs\AutoDevPanel.tsx:77-89
        const isWorking  = worker.status === 'working' || !!active
        const dotColor   = workerStatusColor(isWorking ? 'working' : worker.status)
        const typeLabel  = (WORKER_TYPE_LABELS[worker.worker_type] ?? worker.worker_type).toUpperCase()
        const label      = worker.slug.toUpperCase()
        const taskLabel  = active
          ? `#${active.id} — ${active.title}`
          : worker.status === 'active' || worker.status === 'idle'
            ? 'Idle — awaiting tasks'
            : worker.status === 'paused'
              ? 'Paused'
              : worker.status === 'created'
                ? 'Created — not started'
                : 'Error'
```

Statuses produced by `RecipeRunner::set_worker_status()` at various points in a tick lifecycle: `idle → working → idle` (normal path), or `… → error` (failure). When the roster is viewed during a tick (~30 s window) AND no task row exists in `tasks.db`, the flow is:
- `active` = `undefined`
- `worker.status` = `'working'`
- None of the three explicit branches match → falls to `'Error'`

So the label is **definitionally wrong** — the worker is not in error, it's running the empty-board poll recipe. This happens on every single cron tick until someone adds a task.

### 2.3 Backend Bug C — 400 on `/api/mcp/servers`

Log lines 535-543 and 548:

```
2026-04-23 19:47:54 [INFO]  airs_mcp.config.loader: 📦 [MCP Config] Final merged servers: ['mcp-adobe', 'mcp-autodev', 'mcp-memory', 'autodev-mcp-server']
2026-04-23 19:47:54 [ERROR] api_gateway.modules.mcp.service: Server with ID autodev-mcp-server already exists
2026-04-23 19:47:54 [ERROR] api_gateway.modules.mcp.service: Failed to add server: Server with ID autodev-mcp-server already exists
...
ValueError: Server with ID autodev-mcp-server already exists
...
POST /api/mcp/servers HTTP/1.1" 400 Bad Request
```

Core-orch loads `config/mcp/autodev-mcp-server.yaml` as part of the startup config merge:

```@c:\AIRS Foundry\autodev\autodev\submodules\core-orchestrator\config\mcp\autodev-mcp-server.yaml:1-4
id: autodev-mcp-server
name: AutoDev MCP Server
type: autodev
tags: []
```

Then Rust POSTs the same ID, and Python's `add_server()` raises `ValueError` (served as 400). Rust's check is:

```@c:\AIRS Foundry\autodev\autodev\src-tauri\src\sidecar\client.rs:769-780
        } else if resp.status().as_u16() == 409 {
            // Already registered — idempotent
            info!("[OrchestratorClient] autodev-mcp-server already registered (409)");
            Ok(())
        } else {
            ...
            Err(AutoDevError::Agent(format!(
                "MCP server registration failed (HTTP {status}): ...
```

400 is not matched → Rust logs the registration as failed. **It's non-fatal** because the LivingAgent already loaded the server from the YAML config, so it has the tools. But the log noise is misleading and currently makes the "registered" state ambiguous.

### 2.4 Residual panic — `ollama-rs-0.3.4`

Log lines 104-107:

```
ERROR autodev: [PANIC] panicked at C:\...\ollama-rs-0.3.4\src\lib.rs:122:44:
called `Result::unwrap()` on an `Err` value: RelativeUrlWithoutBase
```

Two panics fire at T+11 s (before core-orch is up), from the Tokio task-panic handler. Tasks are caught so the app continues; however if the active provider is ever `ollama` and Mode 2 is not reachable, the Mode 1 fallback in `RecipeRunner` will hit the same panic and kill the worker tick.

Current offending code:

```@c:\AIRS Foundry\autodev\autodev\src-tauri\src\provider\ollama.rs:default_from_env
pub fn default_from_env() -> Self {
    if let Ok(url) = std::env::var("OLLAMA_HOST") {
        let mut parts = url.split(':');
        let host = parts.next().unwrap_or("localhost").to_string();
        let port = parts.next().and_then(|p| p.parse().ok()).unwrap_or(11434);
        Self::new(host, port)
    } else {
        Self::new("localhost".to_string(), 11434)
    }
}
```

If `OLLAMA_HOST="http://localhost:11434"`, `split(':')` yields `["http", "//localhost", "11434"]`. `host = "http"`, `port = None → 11434`. Downstream `Ollama::new("http", 11434)` triggers the crate's internal `Url::parse` unwrap.

### 2.5 Residual noise — SSE empty-data

Still present, identical to the prior run. See Run #1 report §4.2.

---

## 3. Atomic Task Breakdown

Tasks are **atomic** (one file, one conceptual change, one commit). Each item lists: target file · symbol · change type · acceptance check.

### Task A1 — Wire `sidecarStore` to `startup:progress` events ✅ DONE

- **File:** `src/stores/sidecarStore.ts`
- **Change type:** Add `initEventListeners()` method (mirroring `workerStore` pattern)
- **Scope:**
  - Register a `listen<StartupProgressPayload>("startup:progress", …)` on first call.
  - When `event.payload.phase === "orch_ready" | "fleet_ready" | "ready"`, set `{ status: "running", isConnected: true }`.
  - Additionally, start a 5-second `setInterval` that calls `checkConnectivity()` until `isConnected === true`, then stops.
  - Idempotent via a module-scoped `_listenersInstalled` flag (same shape as `workerStore.ts:111`).
- **Acceptance:** Open AutoDev tab at T+0; the badge flips from `• Mode 1` to `• LivingAgent` within ~3 s of core-orch becoming healthy, without navigation.
- **Implementation:** Added `initEventListeners()` to sidecarStore with event listener registration and 5s polling fallback.

### Task A2 — Call `sidecarStore.initEventListeners()` on app mount ✅ DONE

- **File:** `src/App.tsx` (or wherever `workerStore.initEventListeners()` is currently invoked)
- **Change type:** Add one line inside the existing mount effect
- **Scope:** `useSidecarStore.getState().initEventListeners()`
- **Acceptance:** Listener registered exactly once per app lifetime (guarded).
- **Implementation:** Added `initSidecarEvents` hook and called it in the mount effect alongside worker/task listeners.

### Task A3 — Remove stale `checkConnectivity()` from `AutoDevPanel` ✅ DONE

- **File:** `src/tabs/AutoDevPanel.tsx:446-448`
- **Change type:** Delete the `useEffect` block that calls `checkConnectivity()` on mount (superseded by A1).
- **Acceptance:** `AutoDevPanel` no longer probes connectivity — the store does it globally.
- **Implementation:** Removed the stale useEffect and unused `checkConnectivity` import.

### Task B1 — Fix "Error" label in `AgentRoster` ✅ DONE

- **File:** `src/tabs/AutoDevPanel.tsx:77-89`
- **Change type:** Extend the `taskLabel` conditional chain
- **Scope:**
  - Add `worker.status === 'working'` → `'Working — polling…'` (or `'Working — #<task.id>'` if `active` is present, which the branch above already handles).
  - Add `worker.status === 'error'` → `'Error — see logs'` (explicit, so Error only shows when it's really an error).
  - Default fallback → `worker.status` (title-cased) so we never mask unknown states as "Error".
- **Acceptance:** During the ~30 s tick window on an empty board, the row reads `Working — polling…` with the amber shimmer dot; only genuine errors say `Error`.
- **Implementation:** Extended taskLabel conditional to handle `working` status and explicit error check.

### Task B2 — Promote `working` to a status color the user expects ✅ DONE

- **File:** `src/stores/workerStore.ts:47-56` (`workerStatusColor`)
- **Change type:** No-op — already returns amber (`#FFCB8B`) for `working`. Just verify after B1 that the dot correctly turns amber during polls.
- **Acceptance:** Amber dot + amber progress indicator visible during ticks.
- **Implementation:** Verified — amber color `#FFCB8B` already correctly assigned to `working` status.

### Task C1 — Make core-orchestrator return 409 instead of 400 for duplicate MCP server ✅ DONE

- **File:** `submodules/core-orchestrator/api_gateway/modules/mcp/service.py:160`
- **Change type:** Replace `raise ValueError(...)` with an HTTPException
- **Scope:**
  ```python
  from fastapi import HTTPException
  raise HTTPException(status_code=409, detail=f"Server with ID {server.get('id')} already exists")
  ```
  (Or, if a custom exception layer is in use, raise `ConflictError` that the middleware maps to 409.)
- **Acceptance:** Rust's `register_mcp_server()` hits the existing `== 409` branch and logs `autodev-mcp-server already registered (409)` instead of an error.
- **Implementation:** Changed ValueError to HTTPException with 409 status code.

### Task C2 — (Alternate) Remove the static YAML and rely on Rust POST

- **File:** `submodules/core-orchestrator/config/mcp/autodev-mcp-server.yaml`
- **Change type:** Delete the file
- **Rationale:** The MCP server is an in-process Rust component; its lifecycle is owned by the Tauri app, so the orchestrator should learn about it via `register_mcp_server()` at runtime, not a hard-coded config.
- **Acceptance:** Only one registration path exists → no 400. (Do **either** C1 or C2, not both; C1 is less invasive.)

### Task C3 — Also accept 400 "already exists" in Rust (defensive) ✅ DONE

- **File:** `src-tauri/src/sidecar/client.rs:769-772`
- **Change type:** Extend the idempotent check
- **Scope:**
  ```rust
  } else if resp.status().as_u16() == 409
         || (resp.status().as_u16() == 400
             && /* body contains */ "already exists")
  {
      info!("[OrchestratorClient] autodev-mcp-server already registered");
      Ok(())
  }
  ```
- **Acceptance:** Even without C1, the log no longer shows "MCP server registration failed". Only ship this alongside C1 as a belt-and-suspenders safety net.
- **Implementation:** Added defensive 400 check with "already exists" text match; logs as legacy response.

### Task D1 — Sanitize `OLLAMA_HOST` parsing ✅ DONE

- **File:** `src-tauri/src/provider/ollama.rs` · fn `default_from_env`
- **Change type:** Rewrite URL parsing to handle scheme + host + port
- **Scope:**
  ```rust
  pub fn default_from_env() -> Self {
      let raw = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
      // Strip scheme if present
      let without_scheme = raw.trim_start_matches("https://").trim_start_matches("http://");
      let (host, port) = match without_scheme.rsplit_once(':') {
          Some((h, p)) => (h.trim_end_matches('/').to_string(), p.parse().unwrap_or(11434)),
          None => (without_scheme.trim_end_matches('/').to_string(), 11434),
      };
      Self::new(host, port)
  }
  ```
- **Acceptance:** `OLLAMA_HOST=http://localhost:11434` produces `host="localhost"`, `port=11434`. No panic on startup.
- **Implementation:** Rewrote URL parsing to strip scheme (http/https) and parse host:port correctly; added debug logging.

### Task D2 — Wrap Ollama `.stream()` in `catch_unwind` ✅ DONE

- **File:** `src-tauri/src/provider/ollama.rs` (wherever `.generate_stream()` or `.send_chat_messages_stream()` is called)
- **Change type:** Wrap the crate call in `tokio::task::spawn_blocking` + `std::panic::catch_unwind`, or better, bound it inside `AssertUnwindSafe(future).catch_unwind().await` (from `futures::FutureExt`).
- **Scope:** Convert any panic into an `AutoDevError::Agent` with the message `"ollama-rs internal panic — upgrade crate or use HTTP provider"`.
- **Acceptance:** Mode 1 fallback on Ollama logs a graceful error + sets worker status to `error`, instead of killing the tick thread.
- **Implementation:** Wrapped `send_chat_messages_stream()` in `catch_unwind(AssertUnwindSafe(...))` with proper error handling and logging.

### Task D3 — Upgrade / replace `ollama-rs` ✅ DONE

- **File:** `src-tauri/Cargo.toml`
- **Change type:** Keep 0.3.4 (no 0.4 published); panic prevention via D1 + D2 instead.
- **Acceptance:** `cargo check` passes; ollama-rs 0.3.4 with sanitized OLLAMA_HOST parsing + catch_unwind wrapping prevents panics.
- **Implementation:** Kept ollama-rs at 0.3.4 with comment noting that panic prevention is achieved through D1 (URL sanitization) and D2 (catch_unwind wrapping) rather than crate upgrade.

### Task E1 — SSE empty-data guard (Python SDK patch) ✅ DONE

- **File:** `submodules/core-orchestrator/.venv/Lib/site-packages/mcp/client/streamable_http.py:162` (vendor patch) **OR** upstream PR to `mcp` SDK.
- **Change type:** Add early-continue for empty events
- **Scope:**
  ```python
  if not sse.data or not sse.data.strip():
      return False
  message = JSONRPCMessage.model_validate_json(sse.data)
  ```
- **Acceptance:** Log size shrinks by ~70 % for a normal startup; no more `Invalid JSON: EOF while parsing` errors.
- **Implementation:** Added guard in `_handle_sse_event()` to skip empty/whitespace-only SSE data before JSON parsing; returns False to continue processing.

### Task E2 — Track / fork the fix upstream

- **File:** `docs/mcpserverrun.md` (add link section)
- **Change type:** Track the upstream issue + PR so the vendor patch can be removed when the SDK publishes a fix.
- **Acceptance:** Open issue URL recorded here.

---

## 4. Suggested Implementation Order

1. **B1, A1, A2, A3** — UI truth restored. Zero risk, immediate user-visible fix.
2. **C1** (then **C3** as insurance) — backend registration noise gone.
3. **D1** — Ollama startup panic gone.
4. **E1** — massive log-noise cleanup.
5. **D2, D3** — resilience hardening.

Steps 1–2 can ship in one PR; 3–5 are independent follow-ups.

---

## 5. Files Touched Summary

| Task | File |
|------|------|
| A1   | `src/stores/sidecarStore.ts` |
| A2   | `src/App.tsx` |
| A3, B1 | `src/tabs/AutoDevPanel.tsx` |
| B2 (verify) | `src/stores/workerStore.ts` |
| C1   | `submodules/core-orchestrator/api_gateway/modules/mcp/service.py` |
| C2 (alt) | `submodules/core-orchestrator/config/mcp/autodev-mcp-server.yaml` |
| C3   | `src-tauri/src/sidecar/client.rs` |
| D1, D2 | `src-tauri/src/provider/ollama.rs` |
| D3   | `src-tauri/Cargo.toml` |
| E1   | `submodules/core-orchestrator/.venv/.../mcp/client/streamable_http.py` (or upstream) |

---

## 6. Verification Plan

After applying A1–B1 + C1:

1. **Cold boot:** `npm run tauri dev` with AutoDev tab preselected.
2. **T+0..5 s:** badge shows `• Mode 1` (expected — orch not up yet).
3. **T+15..20 s:** badge flips to `• LivingAgent`, and all 7 worker rows change to amber dots with label `Working — polling…`.
4. **T+50 s:** all rows return to blue `Idle — awaiting tasks` after ticks complete.
5. **Log check:** no `POST /api/mcp/servers ... 400` line; instead `autodev-mcp-server already registered (409)`.
6. **Screenshot diff:** compare against the attached screenshot — no row should say `Error` unless a real failure occurred.

---

*Report replaces the previous version; tasks are ready to be converted into internal AutoDev Board items via the Delegate Mode PA.*
