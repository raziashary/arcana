# Phase 3.1 — Fleet Flow Analysis (Calendar App Re-Run)

**Date:** 22 April 2026
**Log source:** `~/.autodev/logs/autodev-2026-04-22.log` (69,316 lines, ~40 min run)
**Model:** `anthropic/claude-haiku-4-5-20251001` (core-orch) / `claude-4-5-haiku` (Rust fallback — invalid)
**Working dir:** `C:\AIRS Foundry\autodev\testing`
**User prompt:** Same calendar-app prompt as 21-Apr-2026 run

---

## 1. Executive Summary

The 22-Apr run is the **first exercise of the Phase 3.0 push-dispatch fixes** (`worker:status` events, `scheduler::kick`, `task:changed` events, `pa_mode` session persistence). The fixes worked — the log shows **near-instant task assignment** and far less dead-air between state transitions. However, **four new failure modes** surfaced that were masked by the old polling slowness. They are listed below in severity order.

| # | Symptom user saw | Root cause | Severity |
|---|------------------|------------|----------|
| F | Workers still flicker `error → idle` (less often, but still visible) | **Windows silently resets idle HTTP keep-alive connections** (WinError 10054). reqwest (no retry, no pool tuning) picks the dead pooled connection at cron-boundary bursts → `error sending request for url` → fallback to Rust `AgentEngine` → Rust sends the **config key** `claude-4-5-haiku` instead of the **API name** `anthropic/claude-haiku-4-5-20251001` → Anthropic 404 → status `error`. Two bugs chained. | High |
| G | Tasks stuck in `qc_review` on the QC board; qc-agent row shows error | **qc-agent invoke fails for the same transport reason as F** (WinError 10054 → reqwest failure → Rust 404). The "Session worker-qc-agent not found" warnings are a **benign log-only bug** in `ConversationsService.get_session_context()` (line 668) because headless sessions are never inserted into `ConversationsService._sessions`. They affect **all** workers, not just qc-agent. | High |
| H | Frontend build (`testing/index.html`) shows black screen | **`node_modules` never installed** — no `npm install` was executed; additionally `moment` is imported in `App.tsx` but missing from `package.json`; `index.html` references Vite module paths without a dev server | High |
| I | Bash tool returns `exit_code 255` on benign commands | **Windows / Unix command impedance** — LLM generates pipe sequences (`… \| head -20`) that use `head`, which does not exist on Windows CMD/PowerShell | Medium |

**What improved since 21-Apr (do not regress):**

- `create_task` → `scheduler::kick("supervisor")` works — supervisor assign-burst happens within seconds, not 21 s.
- `assign_task` → `scheduler::kick(worker)` works — workers pick up tasks immediately instead of waiting for their next cron tick.
- `worker:status` push events eliminate the *phantom* idle↔error flicker caused by derived UI polling.
- `task:changed` events keep the board fresh without the 60 s `setInterval` being the sole driver.
- `pa_mode` session persistence works — no Mode-1 snap-back observed.

---

## 2. Event Timeline (condensed)

### Phase 3.1 · Boot (03:17:02 → 03:17:11) — 9 s

All 7 agents register with core-orch. `worker:status` events fire immediately.

### Phase 3.2 · PA prompt → task burst → instant assignment (03:17:11 → 03:19:27)

- `create_task` × N (exact count hard to determine because `task:changed` events do not leave a single TRACE line; they are emitted via `events.rs::emit_task_changed_global`).
- Supervisor kicks instantly, assigns tasks to `worker-frontend` and `worker-backend`.
- Tasks move `ready → assigned → in_progress` within the same minute.

### Phase 3.3 · Worker execution (03:19:27 → 03:20:30)

- `worker-backend` executes `bash__autodev` (scaffolding files), `write_file__autodev` × many.
- `worker-frontend` executes `write_file__autodev` (React components).
- **Critical bash failure at 03:19:59** — `npm list 2>&1 | head -20` fails with `exit_code 255` because `head` is not a Windows executable (see L10773). The LLM receives this failure and never attempts `npm install`.

### Phase 3.4 · Transport burst failures at cron boundaries (03:30:00, 03:40:01, 03:44:00, 03:50:01)

At exact cron boundaries reqwest reuses a dead pooled HTTP connection that Windows silently reset (`WinError 10054` / `ProactorBasePipeTransport._call_connection_lost`). reqwest has **no retry**, so the POST dies immediately.

```
03:30:00.068  WARN  RecipeRunner  LivingAgent invoke failed for 'worker-frontend':
              POST … error sending request for url (http://127.0.0.1:4000/…)
03:30:00.525  ERROR RecipeRunner  Rust AgentEngine failed for 'worker-frontend':
              Provider error: Anthropic stream 404 Not Found:
              model: claude-4-5-haiku
```

**Core-orch is alive** during every failure:
- 03:30:00.065 `worker-cleanup` succeeds while 03:30:00.068 `worker-frontend` fails (same core-orch process).
- 03:44:00.062 `supervisor` succeeds while 03:44:00.063 `qc-agent` fails.

The failure is at the **Rust transport layer**, not inside core-orch. After each failure `RecipeRunner` falls back to Rust `AgentEngine`, which 404s on the wrong model string, sets worker status `error`, and recovers on the next tick.

### Phase 3.5 · QC stall (03:44:00 → end of log)

`qc-agent` fails to invoke for the **same transport reason** as other workers (Phase 3.4). It never gets a chance to poll `qc_review` tasks.

Two **separate, non-blocking** log-only issues also appear:

1. **`ConversationsService._sessions` lookup bug** — `Failed to resolve session attachments: Session worker-qc-agent not found` (and identical warnings for `worker-cleanup`, `worker-mcp`, `worker-backend`, `supervisor`). Headless invocations create sessions in `SessionRuntimeManager._runtimes`, but **never** in `ConversationsService._sessions` (line 46, populated only by the UI REST endpoint). The warning is harmless but floods the log.
2. **Memory MCP blip** — `store_user_message[…] failed - connection closed (attempt N/3)` appears for `worker-qc-agent` and `worker-worker-platform`. This is an internal core-orch→memory-mcp retry loop during successful ticks; it does not block execution.

---

## 3. Root Cause Analysis

### 3.1 (F) Connection failure chain: Windows socket reset → reqwest pool reuse → Rust fallback → model name mismatch

**Evidence (log):**

- Core-orch invoke failures happen at **exact cron boundaries** when multiple workers fire simultaneously: 03:30:00, 03:40:01, 03:44:00, 03:50:01 (L24900, L26862, L45517, L51627, L68925).
- At every failure instant, **other workers succeed** against the same core-orch instance:
  - 03:30:00.065 `worker-cleanup` succeeds (L24899) while 03:30:00.068 `worker-frontend` fails (L24900).
  - 03:44:00.062 `supervisor` succeeds (L51626) while 03:44:00.063 `qc-agent` fails (L51627).
- This proves core-orch **is alive and processing** — the failure is at the Rust→core-orch transport layer, not in core-orch itself.

**Evidence (code):**

- `src-tauri/src/sidecar/client.rs:57-59`:
  ```rust
  let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(INVOKE_TIMEOUT_SECS))  // 600 s
      .build()
  ```
  **No pool configuration, no retry policy, no connection-level timeout** separate from request timeout. reqwest reuses pooled HTTP/1.1 keep-alive connections by default.
- `src-tauri/src/sidecar/mod.rs:132-136` (Phase 3.0 filter) documents the underlying Windows mechanism:
  ```rust
  line.contains("ConnectionResetError") && line.contains("WinError 10054")
      || line.contains("_ProactorBasePipeTransport._call_connection_lost")
  ```
  The `ProactorBasePipeTransport` on Windows silently resets idle HTTP keep-alive connections. This was filtered from the UI log view but still happens at the OS level.

**Root cause chain:**

1. Windows `ProactorBasePipeTransport` silently resets an idle keep-alive TCP connection to `127.0.0.1:4000`.
2. reqwest's pooled connection to core-orch becomes dead without reqwest knowing.
3. On the next concurrent burst (cron boundary), reqwest picks the dead connection → `error sending request for url`.
4. reqwest has **no retry** (`client.rs` configures none), so the POST fails immediately.
5. `RecipeRunner` (`src-tauri/src/recipes/runner.rs:198-204`) catches the error and falls back to Rust `AgentEngine`.
6. Rust `AgentEngine` uses a **different model string** than core-orch:
   - Core-orch `model_registry.py:557-558` maps key `"claude-4-5-haiku"` → API name `"anthropic/claude-haiku-4-5-20251001"`. LiteLLM calls the **API name**.
   - Rust `AgentEngine` sends the raw key `claude-4-5-haiku` to Anthropic's API → **404 Not Found** (L26865, L68928).
7. The worker status flips to `error`. On the next tick (60 s later), core-orch is fine again → status returns to `idle`. This is the flicker.

**Affects:** worker roster flicker, wasted API quota on 404 calls, noise in the error log. The model mismatch is a **secondary** failure — the primary issue is the transport reset.

---

### 3.2 (G) qc-agent "session not found" is a ConversationsService lookup bug — not orphaning

**Evidence (logs):**

- `Failed to resolve session attachments: Session worker-qc-agent not found` appears 35+ times (L3550, L6284, L8750, L11340, L13872, L15926, …).
- **Critically, the same warning appears for ALL workers**, not just qc-agent: `worker-worker-cleanup`, `worker-worker-mcp`, `worker-worker-backend`, `worker-supervisor` all show identical `Session … not found` warnings in `api_gateway.log`.
- `store_user_message[worker-qc-agent] failed - connection closed` also appears for `worker-worker-platform` (L68923) — this is a **general** Memory MCP connectivity blip, not qc-agent specific.

**Evidence (code — actual root cause):**

The warning is emitted from `api_gateway/modules/agents/service.py:1576`:
```python
except Exception as e:
    logger.warning(f"Failed to resolve session attachments: {e}")
```

The exception originates at `api_gateway/modules/conversations/service.py:668-671`:
```python
session = self._sessions.get(session_id)
uid = (user or {}).get("user_id") if user else None
if not session:
    raise ValueError(f"Session {session_id} not found")
```

**The bug:** `ConversationsService._sessions` (line 46, a class-level `Dict[str, Dict[str, Any]] = {}`) is populated **only** by `ConversationsService.create_session()` (line 73), which is called exclusively from the **UI REST endpoint** (`POST /api/sessions`).

Headless recipe invocations (`invoke_agent` at `agents/service.py:1310`) create sessions via:
1. `agents/service.py:1341`: `session_id = f"worker-{agent_id}"` — synthetic ID for headless mode.
2. `agents/service.py:1359-1364`: `runtime_manager.get_runtime(agent_id, session_id, user_id, create_if_missing=True)` — creates the runtime and its internal `SessionLifecycleFSM`.
3. `runtime_manager.py:403-407`: `SessionLifecycleFSM(session_id).create(agent_id=agent_id)` — FSM emits `session.created` event.

**BUT** `runtime_manager.get_runtime()` **never** calls `ConversationsService.create_session()`. The session exists in `SessionRuntimeManager._runtimes`, but **`ConversationsService._sessions` remains empty** for every headless worker session.

Therefore `self._sessions.get(session_id)` at line 668 **always** returns `None` for headless invocations, triggering the `ValueError` at line 671. The exception is caught at line 1576 and logged as a WARNING, then execution continues. It is **benign** (headless invocations have no file attachments to resolve), but it floods the log.

**Why qc-agent tasks are actually stuck:**

The qc-agent invokes fail for the **same transport reason** as other workers (section 3.1). When qc-agent's tick fails:
1. `RecipeRunner` fallback to Rust `AgentEngine` triggers (section 3.1, chain steps 4-6).
2. Rust fallback 404s on the bad model name → worker status `error`.
3. qc-agent never gets a chance to poll `qc_review` tasks.
4. The `store_user_message` retry warnings are a **separate** Memory MCP blip that happens during successful qc-agent ticks, not the blocker.

**Affects:** QC board backlog is caused by qc-agent invoke failures (same as other workers), not by session orphaning. The `Session … not found` warnings are harmless log noise affecting all workers.

---

### 3.3 (H) Missing dependency installation + missing `moment` in package.json

**Evidence:**

- `C:\AIRS Foundry\autodev\testing\node_modules` **does not exist** on disk (verified by direct directory listing).
- Log search for literal `npm install`, `npm i`, `pnpm install`, `yarn install` across the entire 69 K-line file: **zero hits**.
- The only npm-related bash call is `npm list 2>&1 | head -20`, which fails on `head` (L10773). The LLM likely interprets this failure as "npm is broken" and never tries `npm install`.
- `testing/package.json` dependencies do **not** include `moment`, but `testing/src/App.tsx` line 3 does `import moment from 'moment'`.
- `testing/index.html` references `<script type="module" src="/src/main.tsx">` — this requires a Vite dev server or a pre-build step. Opening the HTML file directly in a browser produces a blank screen because the browser cannot resolve bare modules or TSX.

**Affects:** user sees a completely black screen when opening the generated app. The frontend code is written but not runnable.

---

### 3.4 (I) Windows bash impedance

**Evidence:**

- `cd "C:\AIRS Foundry\autodev\testing" && npm list 2>&1 | head -20` → `stderr: "'head' is not recognized as an internal or external command"`, `exit_code: 255` (L10773).

The `bash` tool on Windows is implemented via `std::process::Command` (likely targeting PowerShell or CMD). The LLM, trained predominantly on Unix examples, emits pipes and utilities that do not exist in the Windows shell. The tool reports a hard failure (255) instead of a soft "command not found", which the agent interprets as a blocking problem.

**Affects:** any workflow that requires `npm`, `git`, `curl`, `jq`, etc. in a piped or Unix-specific form will fail on Windows, causing the agent to abandon the step.

---

## 4. Recommended Fixes (ordered by impact)

### Fix 6 — Harden Rust→core-orch transport + unify model name (kills F)

**Problem:** Two chained bugs. (1) Windows silently resets pooled connections; reqwest has no retry → sporadic invoke failures. (2) When fallback triggers, Rust `AgentEngine` sends the config key `claude-4-5-haiku` instead of the API name `anthropic/claude-haiku-4-5-20251001`.

**Approach — Transport layer (primary):**
1. In `client.rs:57-59`, configure reqwest with **connection pool tuning** and **automatic retry**:
   ```rust
   let client = reqwest::Client::builder()
       .timeout(Duration::from_secs(INVOKE_TIMEOUT_SECS))
       .pool_max_idle_per_host(1)  // force fresh connections per host
       .pool_idle_timeout(Duration::from_secs(30))  // reap idle before Windows does
       .build()
   ```
2. Add a **single retry** in `invoke_worker()` (`client.rs:458-514`) — if `send().await` fails with a connection error, wait 100 ms and retry once with a fresh request before returning `Err`.
3. Alternatively, disable connection pooling entirely for invoke requests (use a separate `Client` per request or set `pool_max_idle_per_host(0)`) since invoke volume is low and latency is dominated by LLM token generation, not TCP handshake.

**Approach — Model name layer (secondary):**
4. Align Rust `AgentEngine` with core-orch's `model_registry.py:557-558` convention. The Rust provider must use the **full API model name** (`anthropic/claude-haiku-4-5-20251001`), not the short config key (`claude-4-5-haiku`).
   - **Preferred:** Rust calls the existing `GET /api/llm/models` endpoint (`api_gateway/modules/llm/router.py:48-93`) at boot. The response already contains both `registry_key` (short name) and `name` (full API name) for every model. Cache the mapping and use it during fallback.
   - **Alternative:** store the canonical API name in `~/.autodev/config.json` and have both sides read it.

**Immediate band-aid (30 s):** hard-code `anthropic/claude-haiku-4-5-20251001` in the Rust `AnthropicProvider` fallback path. This alone eliminates the visible error flicker until the transport fix lands.

---

### Fix 7 — Fix ConversationsService headless session lookup + fix qc-agent invoke path (kills G)

**Problem:** Two independent issues.

**Issue 7a — Log noise (benign but confusing):**
`ConversationsService.get_session_context()` fails for every headless session because `runtime_manager.get_runtime()` never populates `ConversationsService._sessions`.

**Approach:**
1. In `api_gateway/modules/agents/service.py:1483-1486`, skip `get_session_context()` entirely for headless worker sessions (`session_id.startswith("worker-")`). Headless invocations never have file attachments, so the call is unnecessary. Return `attachments = []` directly for synthetic sessions.
2. **Why not `SessionManager._store`:** `SessionManager` is not instantiated in the running app (`GatewayContext.initialize()` deleted it with the comment "SessionManager DELETED - was orphaned"). Headless sessions live in `SessionRuntimeManager._runtimes`, not in any `FastSessionStore`.

**Issue 7b — qc-agent stuck tasks (real blocker):**
qc-agent cannot review tasks because its invokes fail for the **same transport reason** as all other workers (Fix 6, transport layer). There is no qc-agent-specific session loss.

**Approach:**
Apply Fix 6 (reqwest retry + pool tuning). Once transport is stable, qc-agent will successfully poll `qc_review` tasks and complete reviews.

**No separate qc-agent workaround is needed.** The "session not found" warnings and `store_user_message` retries are non-blocking log artifacts.

---

### Fix 8 — Auto-install dependencies and validate build after scaffold (kills H)

**Problem:** The agent writes package manifests (`package.json`, `requirements.txt`, `Cargo.toml`, `pyproject.toml`, etc.) and source files, but never runs the corresponding install command. It also skips post-install validation (`cargo check`, `npm run build`, `pytest`, etc.), so compilation or runtime errors are only discovered later — or never, because the worker never attempts them.

**Approach:**
1. **Ecosystem detection in `post_scaffold` hook:** After all file writes finish, scan the working directory for known manifest files and run the matching install + validate sequence:

   | Manifest | Install command | Validate command |
   |----------|-----------------|------------------|
   | `package.json` | `npm install` (or `pnpm install` / `yarn` if lockfile present) | `npm run build` (or `vite build` / `tsc --noEmit`) |
   | `requirements.txt` | `pip install -r requirements.txt` | `python -m pytest` (if `tests/` exists) or `python -c "import <main_module>"` |
   | `pyproject.toml` | `poetry install` or `pip install -e .[dev]` | `poetry run pytest` |
   | `Cargo.toml` | `cargo build` | `cargo check` then `cargo test` |
   | `go.mod` | `go mod tidy` | `go build ./...` then `go test ./...` |

2. **Worker recipe update:** Add an explicit final step to *every* default worker recipe: *"After all file writes are complete, run the ecosystem-specific install and validate commands. If any command exits non-zero, treat it as a build failure: capture the error output, append it to the task artifact as `build.log`, and move the task to `qc_review` with status `build_failed` so qc-agent can inspect it."*

3. **Tool update:** The `bash` tool (Fix 10) must handle Windows↔Unix command rewriting so that install/validate commands do not fail spuriously on Windows (e.g. `cargo test 2>&1 | head -20` must not abort because of `head`).

4. **Missing-dependency guard:** Before a worker marks a task `done`, assert that the expected artifact directories exist (`node_modules/`, `.venv/`, `target/debug/`, etc.) by checking for a sentinel file (`.package-lock.json`, `.venv/pyvenv.cfg`, `target/.rustc_info.json`). If the sentinel is missing, re-run the install step once. This prevents tasks from being marked complete when dependencies were never actually installed.

---

### Fix 9 — Expand worker status states + toast pipeline (UX improvement)

**Current state machine:**

```
idle ──► working ──► idle
         │
         ▼
        error ──► idle
```

**Proposed expanded states:**

| State | Color | Meaning | Toast? |
|-------|-------|---------|--------|
| `idle` | Grey | Waiting for task | No |
| `polling` | Blue | Tick fired, querying core-orch | No (too noisy) |
| `working` | Green | Executing tools / LLM loop | Yes — "🛠️ Worker started `<task_title>`" |
| `pausing` | Amber | Sidecar connection blip, retrying | No (transient < 3 s) |
| `error` | Red | Hard failure (model 404, session lost, 3× retry exhausted) | Yes — "⚠️ Worker failed: `<reason>`" |
| `cooldown` | Orange | Backoff after error, next tick in N s | Yes — "⏳ Worker cooling down (retry in 30 s)" |
| `offline` | Dark grey | Worker recipe disabled or sidecar permanently down | Yes — "🔌 Worker offline" |

**Toast pipeline:**

1. **Backend:** `events.rs` adds new payloads:
   - `WorkerStatusEvent { slug, status, previous_status, reason: Option<String>, cooldown_seconds: Option<u32> }`
2. **Frontend:** `workerStore` listens to `worker:status`. When `status` transitions into or out of `error`, `cooldown`, or `offline`, it calls a new `toastStore.notify({ type: 'worker', slug, message, severity })`.
3. **UI:** A lightweight toast stack (top-right, 4 s auto-dismiss) shows worker-level events so the user sees *why* a worker turned red instead of just watching it flicker.
4. **Rate limit:** identical `{slug, status}` pairs within 10 s are deduplicated to prevent spam during retry storms.

---

### Fix 10 — Windows shell compatibility layer (kills I)

**Problem:** LLM generates Unix commands on Windows.

**Approach:**
1. In `bash.rs` (the Rust tool implementation), prepend a platform hint to the LLM system prompt:
   - *"The shell is Windows PowerShell. Use `Select-Object -First N` instead of `head -n`, `Out-File` instead of `>`, and avoid Unix pipes when possible."*
2. In `bash.rs` command preprocessing, detect common Unix-only utilities in piped commands and rewrite them to Windows equivalents **before** spawning the process:
   - `… | head -20` → `… | Select-Object -First 20`
   - `… 2>&1 | …` — PowerShell handles redirection natively, but the `2>&1` syntax actually works in PS. The real issue is `head`.
3. If a command exits 255 with "not recognized", the `bash` tool returns a structured error with `suggestion: "This command uses a Unix utility not available on Windows. Try …"` so the LLM can self-correct in the next iteration.

---

## 5. Counter-intuitive Findings

1. **The push-dispatch fixes worked as designed.** The 21-Apr "dead window" (21 s) is gone. Tasks are assigned within seconds of creation. The remaining slowness is purely execution time (LLM token generation).
2. **The `error` status is now *real*, not phantom.** In the 21-Apr run, `error` was a UI artifact caused by derived polling. In the 22-Apr run, `error` corresponds to actual RecipeRunner failures — but the **primary** trigger is Windows silently resetting idle pooled HTTP connections (WinError 10054), not a model problem. The model 404 is a **secondary** failure that only happens because reqwest has no retry and the Rust fallback uses the wrong model string. Fix the transport (retry + pool tuning) and the visible flicker disappears even before the model name is aligned.
3. **`worker-cleanup` and `worker-platform` still never received tasks** in this epic. They remain idle the entire run. This is correct behaviour (no cleanup/platform work in a calendar app) but worth noting for cost attribution.
4. **`task #1` reached `failed` state** (L10232: `task #1 in_progress → failed`). This means a worker encountered an unrecoverable error during execution (likely the bash/npm failure) and correctly escalated the task to `failed` instead of silently dropping it. The task-board state machine is working.

---
### Fix 6 — Harden Rust→core-orch transport + unify model name (kills F)

**Goal:** Eliminate `error sending request for url` at cron boundaries (reqwest dead-pool reuse) and stop the Anthropic 404 fallback.

#### 6a — reqwest connection-pool tuning
- [x] **`src-tauri/src/sidecar/client.rs:55-68`** (modify)
  - Add `.pool_max_idle_per_host(1)` and `.pool_idle_timeout(Duration::from_secs(30))` to `Client::builder()`.
  - Keep existing `.timeout(Duration::from_secs(INVOKE_TIMEOUT_SECS))`.
  - Verify `health_check()` and `invoke_worker()` still compile; no behaviour change except fewer dead pooled connections.

#### 6b — Single retry on connection-level errors in `invoke_worker()`
- [x] **`src-tauri/src/sidecar/client.rs:460-520`** (modify)
  - Wrap the `self.client.post(...).send().await` call in a `match`.
  - On `reqwest::Error` where `.is_connect()` or `.is_timeout()` is true, wait `Duration::from_millis(100)` and retry once with a fresh request builder.
  - If the retry also fails, return the original `Err` so `RecipeRunner` can still fall back to Mode 1 if core-orch is truly down.

#### 6c — Band-aid: hard-code valid Anthropic API model name in Rust fallback
- [x] **`src-tauri/src/provider/anthropic.rs:36-64`** (modify)
  - In `models_static()`, add a new `ModelInfo` entry with `id: "anthropic/claude-haiku-4-5-20251001"` (the exact LiteLLM API name from `model_registry.py:557-558`).
  - **OR** modify `build_body()` so that when `req.model` contains the config key `claude-4-5-haiku`, it is rewritten to `"anthropic/claude-haiku-4-5-20251001"` before the API call.
  - Verify by triggering a Rust `AgentEngine` fallback tick; the Anthropic API must return 200 instead of 404.

#### 6d — Proper: single canonical model config source
- [x] **`src-tauri/src/sidecar/client.rs`** (new helper method `resolve_model_name`)
  - On first call (lazy), calls the existing `GET /api/llm/models` endpoint (`api_gateway/modules/llm/router.py:48-93`).
  - The response is `List[LLMModel]` where each item has `registry_key: str` (short name) and `name: str` (full API name).
  - Cache the `registry_key → name` map in `model_registry_cache: Mutex<HashMap<String,String>>`.
  - `RecipeRunner` or `AgentEngine` can resolve the raw `model_id` through this cache before falling back to Rust.
  - **Note:** No DB schema change required — canonical mapping lives in core-orchestrator's model registry.

---

### Fix 7 — Fix ConversationsService headless session lookup (cleans G log noise)

**Goal:** Remove the harmless `"Session worker-{slug} not found"` flood from `api_gateway.log` for every headless worker.

#### 7a — Skip `get_session_context()` for headless worker sessions
- [x] **`submodules/core-orchestrator/api_gateway/modules/agents/service.py:1483-1486`** (modify)
  - Wrap the `get_session_context` call in a guard that skips attachment resolution for synthetic worker sessions:
    ```python
    # Headless worker sessions (worker-{slug}) have no file attachments
    if not session_id.startswith("worker-"):
        attachments = await conversations_service.get_session_context(session_id, user)
```

---

### Fix 8 — Auto-install dependencies and validate build after scaffold (kills H)

**Goal:** Ensure any generated project (JS, Python, Rust, Go) has its dependencies installed and passes a basic build/test check before the worker marks the task `done`.

#### 8a — Ecosystem detector module (Rust side)
- [x] **`src-tauri/src/hooks/ecosystem.rs`** (create)
  - Define `Ecosystem` enum: `Node`, `PythonPip`, `PythonPoetry`, `RustCargo`, `GoMod`.
  - Implement `detect(working_dir: &Path) -> Option<Ecosystem>` that scans for manifest files in priority order: `package.json` → `requirements.txt` / `pyproject.toml` → `Cargo.toml` → `go.mod`.
  - Implement `install_cmd(&self) -> &'static str` and `validate_cmd(&self) -> &'static str` returning the platform-appropriate shell command string (e.g. `"npm install"` / `"npm run build"`).
  - Added `sentinel_path(&self) -> PathBuf` for Fix 8c sentinel-file checks.
- [x] **`src-tauri/src/hooks/mod.rs`** (modify)
  - Added `pub mod ecosystem;` and re-export `Ecosystem`, `detect`.

#### 8b — Worker recipe update: mandatory install + validate step
- [x] **`src-tauri/src/recipes/defaults.rs`** (modify)
  - Appended a `## Build Validation (MANDATORY)` section to the instructions string of every worker recipe (`worker-backend`, `worker-frontend`, `worker-platform`, `worker-mcp`, `worker-cleanup`).
  - Section enumerates ecosystem-specific install + validate commands (npm, pip, poetry, cargo, go) and instructs the worker to mark `build_failed` on validation failure.
    > "After all file writes are complete, run the ecosystem-specific install command, then run the validate command. If either exits non-zero, append the full stderr to a `build.log` artifact and mark the task status as `build_failed` instead of `done`."
  - For `worker-frontend`, also explicitly mention: "Run `npm install` after writing `package.json`."

#### 8c — Sentinel-file guard before `done` transition
- [ ] **`src-tauri/src/recipes/runner.rs`** (modify, deferred)
  - Ecosystem detector (8a) and `build_failed` status (8d) are ready. Wiring into `runner.rs` post-tick flow requires deeper runner refactoring to avoid overlapping with existing stranded-task reconciliation. Deferred to next phase to keep changes minimal and verifiable.
  - After a worker tick succeeds (Mode 2 or Mode 1), before calling `set_worker_status(worker_slug, "idle")`:
    - Call `ecosystem::detect(&working_dir)`.
    - If an ecosystem is detected, check for the sentinel file:
      - Node → `node_modules/.package-lock.json` (or `pnpm-lock.yaml`, `yarn.lock`)
      - Python pip → `.venv/pyvenv.cfg` (or `requirements.txt` hash marker)
      - Rust → `target/.rustc_info.json`
      - Go → `go.sum`
    - If sentinel is missing, run `install_cmd()` once, then `validate_cmd()`.
    - If validate still fails, write `build.log` to the workspace and transition the active task to `build_failed` instead of `done`.

#### 8d — Task state machine extension for `build_failed`
- [x] **`src-tauri/src/db/tasks.rs`** (modify schema + helpers)
  - Added `BuildFailed` variant to `TaskStatus` enum with `"build_failed"` serialization.
  - Updated `as_str()` and `from_str()` to handle the new status.
- [ ] **`src/tabs/AutoDevPanel.tsx` or `src/stores/taskStore.ts`** (modify)
  - Frontend colour mapping for `"build_failed"` not yet wired (deferred with 8c).

---

### Fix 9 — Expand worker status states + toast pipeline (UX)

**Goal:** Give the user visibility into *why* a worker turned red, and distinguish transient blips from hard failures.

#### 9a — Expand `WorkerStatusPayload` in Rust
- [x] **`src-tauri/src/events.rs:149-154`** (modify)
  - Extended `WorkerStatusPayload` with `previous_status`, `reason`, `cooldown_seconds`.
  - Updated `emit_worker_status_global()` call sites (`runner.rs::set_worker_status`) to populate new fields.
    ```rust
    pub struct WorkerStatusPayload {
        pub slug:               String,
        pub status:             String,
        pub previous_status:    Option<String>,
        pub reason:             Option<String>,
        pub cooldown_seconds:   Option<u32>,
        pub timestamp:          String,
    }
    ```
  - Update `emit_worker_status_global()` signature if needed (it takes the payload struct, so likely no change).

#### 9b — Wire `workerStore.ts` → toast pipeline
- [x] **`src/stores/workerStore.ts`** (modify)
  - `initEventListeners` now tracks `_lastToastTs` per worker slug (dedup window = 5 s).
  - On `status === 'error'`, calls `useToastStore.getState().addToast()` with title, message (from `reason`), `workerSlug`, and `durationMs` (from `cooldown_seconds`).
  - Only emits one toast per worker per dedup window to prevent spam.

#### 9c — Implement `ToastStack.tsx`
- [x] **`src/components/ToastStack.tsx`** (create)
  - Toast stack component: top-right fixed position, auto-dismiss after `durationMs`.
  - Severity colours mapped to WiC theme: `error` = coral (`#FF6363`), `warning` = amber (`#FFCB8B`), `info` = periwinkle (`#82AAFF`), `success` = green (`#ADDB67`).
  - Escape key clears all toasts.
- [x] **`src/stores/toastStore.ts`** (create)
  - Zustand store with `toasts: Toast[]`, `addToast()`, `removeToast(id)`, `clearAll()`.
  - Toast interface: `id`, `title`, `message`, `type`, `workerSlug?`, `durationMs`, `createdAt`.
- [x] **`src/App.tsx`** (modify)
  - Mount `<ToastStack />` at root level pending UI integration (component is ready).

---

### Fix 10 — Windows shell compatibility layer (kills I)

**Goal:** Stop `exit_code 255` when the LLM emits Unix-only pipes (`head`, `grep`, `jq`) on Windows.

#### 10a — Platform hint in `BashTool` description
- [x] **`src-tauri/src/agent/builtin_tools/bash.rs:20-22`** (modify)
  - `description()` now returns a Windows-specific hint when `cfg!(target_os = "windows")` is active, warning the LLM to avoid Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes) and use PowerShell-compatible alternatives.
    > " **Platform note:** The shell on this machine is Windows CMD. Avoid Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes). Use PowerShell-compatible alternatives or run commands separately."

#### 10b — Unix→Windows command pre-processor
- [x] **`src-tauri/src/agent/builtin_tools/bash.rs:44-95`** (modify)
  - Added `rewrite_for_windows(cmd: &str) -> String` function that rewrites common Unix pipe patterns:
    - ` | head -n N` → ` | Select-Object -First N`
    - ` | tail -n N` → ` | Select-Object -Last N`
    - ` | grep PATTERN` → ` | Select-String PATTERN`
  - Applied only when `cfg!(target_os = "windows")`.
  - Rewritten command logged at `debug!` level.
    ```rust
    fn rewrite_for_windows(cmd: &str) -> String {
        cmd.replace(" | head -", " | Select-Object -First ")
           .replace(" | tail -",  " | Select-Object -Last ")
           // add more patterns as needed
    }
    ```
  - Apply only when `cfg!(target_os = "windows")`.
  - Log the rewritten command at `debug!` level so devs can verify.

#### 10c — Structured suggestion on exit 255
- [x] **`src-tauri/src/agent/builtin_tools/bash.rs:66-93`** (modify)
  - In the `Ok(Ok((stdout, stderr, exit_code)))` branch, when `exit_code == 255`, `cfg!(target_os = "windows")`, and output contains `"is not recognized"`:
    - Returns `ToolResult::error` with a structured `[SUGGESTION]` block explaining the Unix→PowerShell mismatch and suggesting `Select-Object`, `Out-File`, etc.
    - Gives the LLM enough context to self-correct in the next reasoning step.
    - Return `ToolResult::error(format!("Command failed (exit 255): {stderr}\n\n[SUGGESTION] This command uses a Unix utility not available on Windows. Try rewriting it with PowerShell-compatible syntax."))`.
  - This gives the LLM enough context to self-correct in the next reasoning step.

---

- [ ] **`docs/development/runtime-fixes/phase3/fleet-flow-analysis.md`** (modify)
  - Copy the updated Fix 8 scope (ecosystem-aware install + validate) and the new atomic Next Actions table into the parent doc so both files stay in sync.