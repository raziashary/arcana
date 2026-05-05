# AutoDev End-to-End Run Analysis — Calendar App
**Date:** 5 May 2026 · **Session Window:** 13:25 – 15:03 (UTC+7)  
**Prompt:** *"Create a task: I want a calendar app similar to Google Calendar…"*  
**Log file:** `logs/autodev-.2026-05-05..log` (60,671 lines / 11.1 MB)  
**Output directory:** `C:\AIRS Foundry\autodev\testing`

---

## Table of Contents
1. [Executive Summary](#1-executive-summary)
2. [System Startup & Initialization](#2-system-startup--initialization)
3. [Task Decomposition & Planning](#3-task-decomposition--planning)
4. [Agent Execution Flow](#4-agent-execution-flow)
5. [What Worked Correctly](#5-what-worked-correctly)
6. [Issues & Root Cause Analysis](#6-issues--root-cause-analysis)
   - [6.1 Build Failures — Windows Path with Spaces](#61-build-failures--windows-path-with-spaces)
   - [6.2 Dashboard & Cost Metrics Not Updating](#62-dashboard--cost-metrics-not-updating)
   - [6.3 QC Agent Stuck on `working-polling`](#63-qc-agent-stuck-on-working-polling)
   - [6.4 Supervisor Toast Static / No State Transitions](#64-supervisor-toast-static--no-state-transitions)
   - [6.5 QC↔Ready Cycling Loop](#65-qcready-cycling-loop)
   - [6.6 Recurring Tracebacks — `ConnectionResetError` / `ClosedResourceError`](#66-recurring-tracebacks--connectionreseterror--closedresourceerror)
   - [6.7 Memory Recall Latency (Slow Recall Warnings)](#67-memory-recall-latency-slow-recall-warnings)
   - [6.8 Theme Visibility](#68-theme-visibility)
7. [Output Artifact Analysis](#7-output-artifact-analysis)
8. [Cost & Efficiency Analysis](#8-cost--efficiency-analysis)
9. [Systemic Issues vs Isolated Bugs](#9-systemic-issues-vs-isolated-bugs)
10. [Recommendations](#10-recommendations)

---

## 1. Executive Summary

The session **partially succeeded**. The system correctly decomposed the calendar app into ~11 tasks, spawned the full agent fleet (supervisor, worker-backend, worker-frontend, worker-platform, worker-mcp, qc-agent), and produced meaningful backend code (`main.py`, `database.py`, `test_events_crud.py`) and a complete frontend skeleton (`Calendar.tsx`, `EventForm.tsx`, `App.tsx`, `Login.tsx`, `Signup.tsx`).

However, **no deliverable reached a deployable/runnable state** due to:
- Build validation uniformly failing because the Windows path `C:\AIRS Foundry\autodev\testing` contains spaces that broke agent-executed shell commands
- The dashboard, cost panel, and QC/supervisor toast UIs never received live data because the frontend polling loop is either absent or broken
- The QC agent drove tasks into an unresolvable re-dispatch loop, burning unnecessary LLM tokens at >110s per QC invocation
- The memory MCP server experienced recurring connection resets that cascaded into retries and cron-slip warnings

---

## 2. System Startup & Initialization

| Timestamp | Event |
|-----------|-------|
| 13:25:11 | Rust binary starts; skills loaded (16 total) |
| 13:25:32 | core-orchestrator Python process initializes; AnalyticsService, OrchestrationService, UserPreferencesManager all ready |
| 13:25:45 | `OrchestratorClient` health-check failed 59 consecutive times **before** the gateway was ready — watchdog declared unhealthy prematurely |
| 13:25:46 | All 10 agents loaded from YAML; A2A servers started on dedicated ports (supervisor, qc-agent, worker-backend, worker-frontend, worker-mcp, worker-platform, worker-cleanup, etc.) |
| 13:25:53 | All A2A servers transitioned `stopped → initializing → starting → ready` — full startup clean |
| 13:26:00 | RecipeRunner first tick: "No qc_review tasks in DB — skipping qc-agent tick" |
| 13:26:01 | All workers confirmed registered with 14 MCP tools exposed: `create_task`, `assign_task`, `update_task_status`, `list_tasks`, `read_file`, `write_file`, `edit_file`, `glob`, `grep`, `list_directory`, `bash`, `planner`, `spawn_subagent`, `spawn_parallel_subagents` |

**Startup was clean with one anomaly:** The 59 consecutive health-check failures at startup are benign (race between health-check polling cadence and gateway readiness) but they generate noise in the log and could cause the Rust-side watchdog to restart the sidecar on a tight restart threshold.

---

## 3. Task Decomposition & Planning

The supervisor correctly broke the prompt into **11 tasks** (IDs 1–11). Confirmed task titles reconstructed from log memory fragments:

| ID | Title | Assigned Worker |
|----|-------|----------------|
| 1  | Add event colors and styling | worker-frontend |
| 2  | (backend task — build_failed) | worker-backend |
| 3  | (backend task) | worker-backend |
| 4  | Add calendar view filtering and date range queries | worker-backend |
| 5  | (task — build_failed) | worker-backend |
| 6  | Write integration tests for full event lifecycle | worker-backend |
| 7  | Deploy calendar app to production | worker-platform |
| 8  | Implement user authentication UI (login/signup screens) | worker-frontend |
| 9  | Build event creation form | worker-frontend |
| 10 | Implement CRUD API endpoints for events | worker-backend |
| 11 | (additional task) | — |

Planning phase completed correctly at **13:27:51** with all tasks in `ready` state, then supervisor dispatched them via `assign_task` calls.

---

## 4. Agent Execution Flow

### Timeline (key events)
```
13:25 – Startup
13:26 – Worker agents first tick (no tasks yet)
13:27 – Supervisor plans & creates 11 tasks; dispatches to workers
13:28 – Workers begin claiming & executing tasks; first QC tick fires (no tasks)
13:29 – worker-platform tries update_task_status → in_progress without assigned_worker → ERROR
13:31 – QC agent first real invocation; 99,585ms to complete
13:32 – QC agent invocation 2: 110,192ms
13:35 – Tasks 2 and 5 marked build_failed by worker-frontend/backend
13:36 – Task 8 (auth UI) marked build_failed
13:37 – QC invocation 3: 115,906ms; task 7 moved qc_review → assigned (loop begins)
13:39 – No qc_review tasks in DB (loop briefly cleared)
13:39:45 – Push-based kick fires qc-agent immediately
13:40–13:42 – QC agent still working; cron ticks skipped
13:43 – Another QC invocation starts
...
14:53–15:03 – worker-backend, worker-frontend, worker-mcp, supervisor still polling every 10s;
              ClosedResourceError tracebacks firing on every tick
```

---

## 5. What Worked Correctly

- **Agent fleet startup** — All 10 agents initialized and ready; A2A servers on correct ports
- **Task decomposition** — Supervisor created a sensible 11-task breakdown for a calendar app
- **Task dispatching** — `assign_task` correctly transitioned tasks `ready → assigned`; supervisor identified stalled tasks correctly
- **Code generation** — Workers produced meaningful artifacts:
  - `main.py` (FastAPI with full CRUD: `/events`, `/auth/register`, `/auth/login`)
  - `database.py` (SQLAlchemy models: `User`, `Calendar`, `Event`)
  - `test_events_crud.py` (25KB integration test suite)
  - `Calendar.tsx`, `EventForm.tsx`, `Login.tsx`, `Signup.tsx`, `App.tsx`
  - `requirements.txt`, `package.json`, `alembic/env.py`
- **FSM transitions** — The `TaskExecutionFSM` and `ToolExecutionFSM` state machines fired correctly on all successful transitions
- **Memory system** — Mem0 procedural/episodic memory was being written and read correctly for tool outcomes; two-stage context scoring was functioning
- **QC guard-rail** — RecipeRunner correctly skipped qc-agent when no `qc_review` tasks existed
- **Worker stop contract** — worker-frontend correctly self-terminated with "No tasks assigned — idle" when no tasks were in its queue

---

## 6. Issues & Root Cause Analysis

---

### 6.1 Build Failures — Windows Path with Spaces

**Symptom:** Both backend and frontend builds failed. `build.log` contents:
```
Build Validation Failed: Unable to execute npm install and npm run build due to Windows CMD 
path syntax errors.
Error: The filename, directory name, or volume label syntax is incorrect.
Issue: Shell environment unable to properly resolve working directory with spaces in path.
```
Multiple tasks ended in `build_failed` status (tasks 2, 5, 8 confirmed; likely more).

**Root Cause — Precise Location:**

The `bash` MCP tool in `src-tauri/src/agent/builtin_tools/bash.rs` rewrites shell commands for Windows PowerShell. The log confirms this rewrite:
```
[BashTool] rewritten command: cd "C:\AIRS Foundry\autodev\testing" && python -m pytest ...
-> cd "C:\AIRS Foundry\autodev\testing" && python -m pytest ... | Select-Object -First 200
```
The `cd` + `&&` chain in PowerShell **requires the path to be quoted** in the `cd` step *and* the shell invoked as `cmd /c` or properly handled as a script block. When agents build the shell command as a string interpolation without surrounding the path in quotes, PowerShell splits the path at the space, interpreting `Foundry` as a command argument and `autodev\testing` as a second argument.

**Specifically:** The worker agents (worker-frontend, worker-backend) generate the build/install command inline, e.g.:
```bash
cd C:\AIRS Foundry\autodev\testing && npm install && npm run build
```
Without quotes around the path this fails immediately in PowerShell. The `bash` tool rewriter patches some patterns but does not uniformly double-quote all `cd` operands that contain spaces.

**Impact:** Every task that required build validation (`npm run build`, `pip install -r requirements.txt`, `pytest`) failed regardless of code correctness. This is the **primary reason zero tasks reached `qc_review` or `done` status through the normal build→QC path.**

**Secondary Effect:** Tasks were repeatedly marked `build_failed` and then the supervisor had no recovery strategy — those tasks were left terminal without retry logic escalation.

---

### 6.2 Dashboard & Cost Metrics Not Updating

**Symptom:** The AutoDev dashboard values (task counts, cost metrics) are not updating in real time.

**Root Cause:**

The log confirms that `AnalyticsService` initialized at 13:25:32 and the analytics router was registered at `/api/analytics`. However, **zero calls to `/api/analytics` appear anywhere in the 60,671-line log**. The frontend never queries it.

This is a **frontend polling gap**. The React components responsible for the dashboard summary and cost panel are either:
1. Not mounting a `useEffect` poll against `/api/analytics` or the Tauri analytics commands, or
2. Calling a Tauri command that is not forwarding to the orchestrator analytics endpoint

The Rust-side `autodev_lib` does not expose a `get_dashboard_summary` or `get_cost_analytics` Tauri command that bridges to `GET /api/analytics/...`. The frontend therefore never has live data.

**Additionally:** Token counts and cost totals are computed by the core-orchestrator's `litellm_provider` internally but the results are only logged at DEBUG level (`Tool update_task_status_autodev-mcp-server executed via memory-aware executor`). There is no event emission from the Python side to the Rust-side event bus that would push cost deltas to the frontend.

**Impact:** Dashboard shows stale/zero values for the entire session duration. This is a **systemic gap** — not a transient bug.

---

### 6.3 QC Agent Stuck on `working-polling`

**Symptom:** The QC-Agent toast in the frontend remains stuck at `working-polling` state and does not transition to reflect task review outcomes.

**Root Cause — Two parts:**

**Part A (Backend):** The QC agent invocations are extremely long:
- Invocation 1 (13:31 start): **99,585ms** (~1.6 min)
- Invocation 2 (13:37 start): **110,192ms** (~1.8 min)
- Invocation 3 (13:39 start): **115,906ms** (~1.9 min)

During the entire execution window the RecipeScheduler correctly logs `Worker 'qc-agent' still working — skipping tick` but no intermediate state events are emitted. The QC agent operates as a single long-running LLM turn with no streaming state updates back to the frontend.

**Part B (Frontend):** The `working-polling` toast state is a static label set when the agent is dispatched. There is no subscription to `agent.task_assigned`, `agent.task_completed`, or QC-specific events that would transition the toast label. The frontend only sees "dispatched" and "done" — intermediate substates (e.g., "reading task", "verifying files", "issuing verdict") are never surfaced because the QC agent does not emit them as discrete events.

**Impact:** From the user's perspective, the QC panel appears frozen for 1–2 minutes per review cycle with no indication of progress.

---

### 6.4 Supervisor Toast Static / No State Transitions

**Symptom:** The supervisor toast does not reflect dynamic states (assigning tasks, breaking down tasks, board clean, etc.).

**Root Cause:** Same pattern as QC. The supervisor's episodic memory shows rich internal reasoning:
```
Board Analysis:
- Total tasks: 11
- Ready tasks: 0
- Assigned tasks: 5 ...
Board clean — idle.
```
This reasoning is stored in Mem0 episodic memory but **never emitted as a Tauri event**. The supervisor completes its tick and returns a final response string. The Rust `RecipeRunner` logs `LivingAgent tick complete for 'supervisor': <text>` but does not parse or re-emit this as structured frontend events.

The frontend toast therefore only knows "supervisor fired" → "supervisor completed" with no substates in between.

---

### 6.5 QC↔Ready Cycling Loop

**Symptom:** Tasks move between `QC Review` and `Ready` (or `Assigned`) repeatedly.

**Evidence from logs:**

Task #7 ("Deploy calendar app to production") transitioned:
```
ready → assigned (worker-platform, 13:27)
assigned → in_progress (13:32)
in_progress → qc_review (13:32)
qc_review → assigned (worker-platform, 13:32:34)  ← QC agent re-dispatched it
```

The QC agent used `update_task_status` with `status: assigned` directly after reviewing the task, rather than routing through a `ready` state or a structured rejection reason. This means:
- The supervisor's next tick sees the task as `assigned` (not `ready`) and ignores it
- The worker-platform picks it up again and executes without knowing it was rejected
- No rejection reason was recorded in the task record

**Root Cause — Two layers:**

1. **QC agent prompt instruction gap:** The QC agent's YAML config does not specify a structured `reject → ready` vs `approve → done` decision tree with a mandatory rejection reason field. The agent improvises and calls `update_task_status` with `assigned` (re-dispatch) instead of emitting a `rejected` verdict that includes a structured reason.

2. **Missing `rejected` task status:** The `update_task_status` MCP tool's state machine only exposes: `ready`, `assigned`, `in_progress`, `qc_review`, `build_failed`, `done`. There is no `rejected` status. The QC agent therefore cannot express "rejected — reason: X" and instead re-assigns to continue work, creating the loop.

**Cost impact:** Each cycle burns ~110–115 seconds of LLM time for the QC agent alone. If the loop runs N times, cost scales linearly with N.

---

### 6.6 Recurring Tracebacks — `ConnectionResetError` / `ClosedResourceError`

**Symptom:** `ERROR autodev_lib::sidecar: [core-orch] Traceback (most recent call last):` fires dozens of times throughout the session, especially in the final 15 minutes. Example full stack (from ~13:41):

```
File ".../context_service.py", line 165, in _call_memory_with_retry
    result = await self._memory_client.call_tool(...)
File ".../airs_mcp/client.py", line 286, in _tracked_call
    result = await asyncio.wait_for(...)
File ".../mcp/shared/session.py", line 261, in send_request
    await self._write_stream.send(...)
File ".../anyio/streams/memory.py", line 211, in send_nowait
    raise ClosedResourceError
```

**Root Cause:** The memory MCP server (running as a separate sidecar) experiences connection resets triggered by Windows TCP behavior (`WinError 10054` — remote host forcibly closed connection). This is a known Windows networking issue where idle TCP connections are terminated by the OS or an intermediary.

The `ClosedResourceError` propagates because:
1. The anyio `MemoryObjectSendStream` backing the MCP SSE connection is closed by the remote side
2. The `context_service.py`'s `_call_memory_with_retry` using `tenacity` re-raises after exhausting retries
3. The error surfaces as an unhandled exception in the asyncio callback `_ProactorBasePipeTransport._call_connection_lost()` — a Windows ProactorEventLoop-specific hook

The log confirms partial recovery: `Memory MCP reconnected successfully` appears several times, meaning the reconnect logic works but the error still generates traceback noise before reconnecting.

**Late-session escalation (13:57–15:03):** The error frequency increased dramatically — up to 10+ tracebacks per second — indicating the memory MCP server became progressively less stable as the session extended, possibly due to connection pool exhaustion or memory growth.

**Impact:**
- `store_memory` and `search_memories` calls silently dropped → agent learning degraded over time
- `list_tasks` via the MCP bridge also raised `ClosedResourceError` at 13:38:15, meaning agents received tool failures instead of task lists
- Cron slip warnings (`enforcing 10s floor`) cascaded from these failures — agents couldn't complete their tick within the cron window

---

### 6.7 Memory Recall Latency (Slow Recall Warnings)

**Symptom:** `WARNING: Slow recall: Xms (threshold: 1000ms)` fires continuously throughout the session.

**Measured latencies:**
| Time | Recall Latency |
|------|---------------|
| 13:28 | 8,428ms |
| 13:29 | 6,463ms |
| 13:30 | 8,481ms |
| 13:32 | 16,076ms |
| 13:32 | 16,552ms |
| 13:37 | 17,511ms |
| 13:38 | 21,738ms |
| 13:40 | 25,056ms |
| 13:40 | 31,793ms |

**Root Cause:** Mem0 is backed by a vector store (likely FAISS/ChromaDB or similar running locally). On Windows, local vector search degrades significantly as the memory corpus grows within a session because:
1. All agents (supervisor, qc-agent, 5 workers) write memories concurrently to the same Mem0 instance
2. The vector index is not sharded per-agent, causing lock contention
3. Recall times escalate exponentially as the corpus grows (from ~8s at session start to ~31s at session end)

**Impact:** Every agent turn that requires memory context adds 8–31 seconds of latency. With 6 agents polling every 30–60 seconds, this means the system spends more wall-clock time waiting for memory than executing LLM calls or tool operations. This is a **major cost multiplier** — the LLM itself is idle during these waits but session time (and potential serverless compute cost) increases linearly.

---

### 6.8 Theme Visibility

**Symptom:** Current dark theme is too dark and affects visibility.

**Root Cause:** The `--bg-primary` and `--bg-card` CSS variables in `Graph.css` (and the global theme) use near-black values (`#0a0a0f`, `#0d0d12`). Text elements using `var(--text-secondary)` at `opacity: 0.5` become illegible on these backgrounds. This is a pure CSS design issue, not a runtime bug.

---

## 7. Output Artifact Analysis

### Backend (`C:\AIRS Foundry\autodev\testing\`)

| File | Status | Issues |
|------|--------|--------|
| `main.py` | Functional code, **not runnable** | Line 172: `Event.calendar.user_id` — SQLAlchemy does not support attribute access on a join target this way; should be `Calendar.user_id` as a join filter |
| `database.py` | SQLAlchemy models | Not viewed but likely the `Calendar` model's `user_id` FK chain |
| `requirements.txt` | Complete but version-pinned for Python 3.10 | `pydantic==1.10.15` is v1 but `main.py` uses `model_config = {'from_attributes': True}` which is Pydantic v2 syntax — version mismatch |
| `test_events_crud.py` | 25KB, comprehensive CRUD tests | Tests will fail because `main.py` has ORM bugs and no running server |
| `alembic/env.py` | Present but incomplete | No `alembic.ini`, no migration scripts; Alembic cannot run |
| `build.log` | Failure record | Confirms Windows path-with-spaces failure |

### Frontend (`src/`)

| File | Status | Issues |
|------|--------|--------|
| `Calendar.tsx` (14KB) | Rich calendar component | No backend URL configured; likely hardcoded `localhost:8000` |
| `EventForm.tsx` (9.9KB) | Full form component | Requires Calendar.tsx context |
| `App.tsx` (2.5KB) | Router setup | React Router v6 structure |
| `Login.tsx` / `Signup.tsx` | Auth screens | Not viewed in detail |
| `package.json` | Vite + React 18 | Missing `vite.config.js` — `npm run build` would fail even with path fixed |
| `public/` | Empty | No `index.html` — Vite build would fail |

**Critical missing files:** `vite.config.js`, `index.html` in `public/`. These are required for `npm run build` to succeed. The worker-frontend generated the component files but did not scaffold the full Vite project structure.

---

## 8. Cost & Efficiency Analysis

### QC Agent Cost Inefficiency
- 3 confirmed QC invocations × ~110s average = ~330s of LLM streaming for reviews
- Each invocation uses `claude-haiku-4-5` (confirmed via `cache hit 'qc-agent' → qc-agent (model='claude-haiku-4-5')`)
- QC ran on tasks that were `build_failed` — verifying code that wasn't even buildable. The QC gate should short-circuit on `build_failed` tasks before invoking the LLM

### Memory MCP Overhead
- Memory recall averaging 8–31 seconds per operation
- Every agent tick makes 2–4 memory calls (search before tool, store after tool)
- At 6 agents × 2–4 calls × 8–31s per call, **memory IO alone costs 96–744 seconds per polling cycle**
- This dwarfs the actual LLM inference time

### Cron Slip Cascade
- `cron slipped (0ns), enforcing 10s floor` appearing 50+ times, predominantly for `worker-backend` and `worker-frontend`
- This means those workers could not finish their tick within the scheduled window → the 10s floor prevents rapid oscillation but each slip indicates a missed execution opportunity
- The slips are primarily caused by memory MCP latency and `ClosedResourceError` retry overhead

### Dead Tokens
- The `worker-frontend` agent hit a "No tasks assigned — idle" stop-contract multiple times but was still re-fired by the recipe scheduler. Each idle invocation burns tokens on: system prompt, tool list, memory search, and the `list_tasks` call before concluding idle.

---

## 9. Systemic Issues vs Isolated Bugs

### Systemic Issues (architectural / cross-cutting)
| # | Issue | Scope |
|---|-------|-------|
| S1 | No rejection-reason tracking for QC | QC agent YAML + task status schema |
| S2 | No `rejected` task status in state machine | MCP tool definition + DB schema |
| S3 | Frontend has no live data polling loop for dashboard/cost | Frontend React components |
| S4 | Backend analytics events not bridged to Tauri event bus | Rust bridge + Python event emission |
| S5 | Memory MCP grows unbounded within session | Mem0 / vector store sharding |
| S6 | Agent toast states are binary (dispatched / done) | Frontend toast + recipe event emission |
| S7 | Build validation runs even on `build_failed` tasks | QC agent precondition check |

### Isolated Bugs (specific, fixable)
| # | Issue | File / Line |
|---|-------|------------|
| B1 | `Event.calendar.user_id` ORM filter invalid | `testing/main.py` lines 172, 200, 288, 337 |
| B2 | `pydantic==1.10.15` (v1) vs v2 model syntax | `testing/requirements.txt` line 4 |
| B3 | Missing `vite.config.js` and `public/index.html` | `testing/` root — not generated |
| B4 | `alembic.ini` and migration scripts not generated | `testing/alembic/` |
| B5 | Build commands not quoting paths with spaces | `bash` MCP tool path handling |
| B6 | 59 health-check failures logged at startup | `sidecar/client.rs` — startup race condition |
| B7 | Windows `WinError 10054` causes memory MCP disconnects | Windows ProactorEventLoop + anyio SSE |

---

## 10. Recommendations

### Priority 1 — Blockers (Must fix before next run)
1. **Path quoting in bash tool:** The `bash` MCP tool must wrap all generated `cd` paths in double-quotes when the CWD contains spaces. Alternatively, set `WORKSPACE_PATH` as an env var at agent launch and reference it as `$env:WORKSPACE_PATH` in PowerShell.
2. **Add `rejected` status to task state machine:** The MCP `update_task_status` tool must accept `rejected` as a valid target status, and the QC agent YAML must mandate a structured `rejection_reason` field when issuing a rejection.
3. **QC short-circuit on `build_failed`:** The QC recipe should pre-check task status; if `build_failed`, skip LLM invocation entirely and instead re-queue the task as `ready` with a `rebuild_needed` flag.

### Priority 2 — Dashboard & UX
4. **Frontend analytics polling:** Add a `useEffect` polling loop (or WebSocket subscription) in the dashboard components that calls `GET /api/analytics/summary` via a Tauri `invoke` command. Wire the response to the React state driving the dashboard counters.
5. **Agent toast event emission:** The `RecipeRunner` (Rust) should parse the final response text of agent ticks for structured markers (e.g., `[STATE: assigning_task]`, `[STATE: idle]`) and emit them as Tauri events. The Python core-orchestrator's FSM already fires `agent.task_assigned` events — these should be bridged to the frontend.
6. **Theme adjustment:** Raise `--bg-primary` from `#0a0a0f` to approximately `#161622` and `--bg-card` to `#1e1e2e`. Increase `--text-secondary` opacity from 0.5 to 0.7.

### Priority 3 — Reliability & Cost
7. **Memory MCP connection resilience:** Replace the anyio `MemoryObjectSendStream`-based SSE transport with an HTTP-based reconnect that tolerates Windows `WinError 10054`. The existing reconnect logic works but fires too slowly.
8. **Memory index sharding:** Partition the Mem0 vector index by `agent_id` to eliminate cross-agent lock contention and reduce recall latency to sub-second.
9. **Idle agent LLM cost reduction:** Before calling the LLM in a recipe tick, the RecipeRunner should make a lightweight HTTP `GET /api/tasks?status=assigned&worker=<slug>&limit=1` preflight. If the count is zero, skip the LLM invocation entirely.
10. **Generate full Vite scaffold:** The worker-frontend agent prompt should explicitly include the Vite project scaffold requirements: `vite.config.js`, `public/index.html`, `tsconfig.json`.

---

*Report generated from 60,671 log lines and direct artifact inspection.*  
*Analysis performed: 5 May 2026*

---

## 11. Deep Output Artifact Analysis

### `testing/main.py` — Backend (FastAPI)

**Bug B1 — Invalid SQLAlchemy ORM attribute traversal (4 locations)**

All four event-query endpoints use `Event.calendar.user_id` as a filter condition. This is not valid SQLAlchemy — you cannot traverse a relationship attribute and access a column on the related model this way. The correct pattern is a `.join(Calendar)` + `Calendar.user_id ==` filter:

```python
# BROKEN (lines 171-173, 198-200, 286-288, 335-337)
query = db.query(Event).join(Event.calendar).filter(
    Event.calendar.user_id == users_db[current_user]["id"]
)

# CORRECT
query = db.query(Event).join(Calendar, Calendar.id == Event.calendar_id).filter(
    Calendar.user_id == users_db[current_user]["id"]
)
```

**Bug B2 — Pydantic version mismatch**

`requirements.txt` pins `pydantic==1.10.15` (v1). In Pydantic v1, the ORM compatibility setting is `orm_mode = True`. The code at line 69-70 uses:

```python
class Config:
    from_attributes = True  # This is Pydantic v2 syntax
```

In Pydantic v1 this key is silently ignored, meaning `EventResponse` cannot be constructed from SQLAlchemy ORM objects. Either upgrade to `pydantic>=2.0.0` and add `from sqlalchemy.orm import DeclarativeBase`, or change to `orm_mode = True` and stay on v1.

**Bug B3 — Mixed auth: `users_db` dict and SQLAlchemy `DBUser`**

`main.py` imports `User as DBUser` from `database` (line 9) but never queries it. All user lookups go through `users_db` (a plain `dict`). Calendar ownership checks use `users_db[current_user]["id"]` but the `Calendar` model's `user_id` is a FK into the SQLAlchemy `users` table. These two user stores are never synchronized — creating a user via `/auth/register` writes to `users_db` but not to the `users` table, so `Calendar.user_id` will never match any real `users.id` value.

**Bug B4 — Validation dead code**

Lines 222-228 check `if not event_data.title` and `if not event_data.start_time` after Pydantic already enforced that these are non-nullable required fields. Pydantic will 422 before those lines are ever reached. The checks are harmless but indicate the LLM was uncertain and added redundant guards.

**Bug B5 — `datetime.utcnow()` deprecation**

SQLAlchemy column defaults in `database.py` use `default=datetime.utcnow` (Python 3.12 deprecated this — use `datetime.now(timezone.utc)` instead). This doesn't break Python 3.10/3.11 but will generate deprecation warnings on Python 3.12+.

---

### `testing/test_events_crud.py` — Test Suite

**Bug B6 — Async engine used with synchronous SQLAlchemy**

Lines 19-26 use `async with engine.begin() as conn`. The `engine` in `database.py` is a synchronous `create_engine` instance. Calling `async with engine.begin()` on a sync engine raises `AttributeError: 'Engine' object has no attribute '__aenter__'`. The fixture must either use `asyncio.get_event_loop().run_until_complete(...)` or the engine must be replaced with `create_async_engine`.

**Bug B7 — `users_db` not reset between tests**

`setup_database` fixture drops/recreates SQLAlchemy tables but never clears `users_db = {}` in `main.py`. Since `users_db` is a module-level dict, it persists across test function calls within the same process. Tests that register `testuser@example.com` in their setup will fail with 400 "Email already registered" if any prior test left the same email in `users_db`.

**Structural note — test isolation**

The `client` fixture is function-scoped but `setup_database` is `autouse=True` without `scope`. The database is torn down/recreated on each test but the running FastAPI app state (`users_db`) is not. This is a design flaw — the tests would pass only if run in isolation, not in suite.

---

### `testing/requirements.txt`

| Line | Issue |
|------|-------|
| `fastapi==0.95.0` | Released April 2023; does not support the `lifespan` context manager pattern (uses deprecated `on_event`). Not a blocker but outdated. |
| `pydantic==1.10.15` | Version conflict with `from_attributes = True` syntax in `main.py` (see Bug B2). |
| `sqlalchemy==2.0.23` | SQLAlchemy 2.x is fine but the test fixture uses the asyncio API (`engine.begin()`) without declaring an async engine. |
| `email-validator==1.3.1` | Requires `pydantic[email]` extras. Not listed. `pip install` of this alone does not satisfy Pydantic v1's email validation import path. |
| Missing: `passlib[bcrypt]` | `main.py` uses `passlib.context.CryptContext` but only `bcrypt==4.1.1` is listed. `passlib` itself is not listed — will fail at import. |

---

### `testing/package.json`

The `package.json` defines a valid Vite + React project. However:

- `testing/src/` — **directory exists but is empty**. No `main.tsx`, no component files. The workers created standalone `.tsx` files directly in `testing/` but `vite.config.js` (absent) would expect entry at `src/main.tsx`.
- `testing/public/` — **empty**. Vite requires `public/index.html` as the HTML template.
- No `vite.config.js` — Without this, `npm run build` will error: `Could not resolve entry module "index.html"`.
- No `tsconfig.json` — TypeScript compilation for Vite requires this.

Frontend components `Calendar.tsx`, `EventForm.tsx`, `Login.tsx`, `Signup.tsx`, `App.tsx` were generated at the `testing/` root rather than inside `testing/src/`. They are orphaned — Vite cannot discover them.

---

### `testing/alembic/`

**Directory is empty.** The `alembic` package is in `requirements.txt` and `alembic/env.py` was mentioned in build planning, but the directory contains no files. Neither `alembic.ini` nor `env.py` was actually written. `alembic upgrade head` will fail immediately.

---

## 12. Atomic To-Do List

> **Scope rule:** All tasks below touch only `C:\AIRS Foundry\autodev\autodev`. Changes to `submodules/core-orchestrator` or `submodules/memory-mcp-server` are **excluded** — those are tracked separately.

---

### Phase A — Task State Machine: Add `rejected` Status

**Status:** ⬜ Pending  
**Goal:** Give the QC agent a proper `rejected` state to express "failed review with reason" without re-dispatching via `assigned`, which is what caused the QC↔Assigned loop in the May 5 run.

#### A.1 — Add `Rejected` variant to `TaskStatus` enum

**File:** `src-tauri/src/db/tasks.rs` — lines 56–86

```rust
// In the TaskStatus enum, add after BuildFailed:
Rejected,     // QC rejected — worker must rework and resubmit

// In as_str():
Self::Rejected => "rejected",

// In from_str():
"rejected" => Self::Rejected,
```

Also update `update_status()` SQL in `tasks.rs` — no schema change needed since status is stored as TEXT. The migration does not need altering.

#### A.2 — Expose `rejected` in `update_task_status_tool.rs`

**File:** `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — lines 25–44

Add `"rejected"` to the JSON schema enum array and update the description:

```rust
// In input_schema(), change the enum list:
"enum": ["ready", "assigned", "in_progress", "qc_review", "done",
         "failed", "build_failed", "cancelled", "rejected"],

// In description():
"Valid statuses: ready, assigned, in_progress, qc_review, done, failed, \
 build_failed, cancelled, rejected. Use rejected when QC finds issues — \
 provide a rejection_reason in the notes field."
```

#### A.3 — Add `rejection_reason` field to the tool input schema

**File:** `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — lines 25–44

```rust
// Add to input_schema() properties:
"rejection_reason": {
    "type": "string",
    "description": "Required when status='rejected'. Describe exactly what the QC agent found wrong."
}
```

Add a guardrail in `execute()` after the existing `in_progress` check:

```rust
if status == "rejected" {
    let reason = input["rejection_reason"].as_str().unwrap_or("").trim();
    if reason.is_empty() {
        return ToolResult::error(
            "status='rejected' requires a non-empty 'rejection_reason'. \
             Describe what failed so the worker can fix it."
        );
    }
}
```

#### A.4 — Add `rejected` hook type and kick-worker logic

**File:** `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — lines 110–118 (`match status.as_str()`)

```rust
match status.as_str() {
    "qc_review" => crate::recipes::scheduler::kick("qc-agent"),
    "assigned"  => { /* existing */ }
    "rejected"  => {
        // Wake the originally assigned worker so they see the rejection immediately
        if let Some(ref w) = assigned_worker {
            crate::recipes::scheduler::kick(w);
        }
    }
    _ => {}
}
```

#### A.5 — Store `rejection_reason` in the tasks DB

**File:** `src-tauri/src/db/tasks.rs`

Add a `rejection_reason` TEXT column via the migration:

```rust
// In migrate(), add to the CREATE TABLE statement (or add as ALTER TABLE in a v2 migration block):
"rejection_reason" TEXT,

// Add update_rejection_reason() helper method:
pub fn update_status_with_reason(
    &self,
    id: i64,
    status: &str,
    assigned_worker: Option<&str>,
    rejection_reason: Option<&str>,
) -> Result<()> {
    self.conn.execute(
        "UPDATE tasks SET status=?1, assigned_worker=?2, rejection_reason=?3, updated_at=datetime('now') WHERE id=?4",
        params![status, assigned_worker, rejection_reason, id],
    )?;
    Ok(())
}
```

#### A.6 — Surface `rejected` column in AutoDev kanban board

**File:** `src/tabs/AutoDevPanel.tsx`

Add a `REJECTED` column to the kanban board between `BUILD_FAILED` and `DONE` so the user can see which tasks need rework:

```tsx
// In the status columns array / render:
{ status: 'rejected', label: 'Rejected', color: '#FF6B35' },
```

---

### Phase B — Build Command Path Handling (bash.rs)

**Status:** ⬜ Pending  
**Goal:** Prevent agents from generating bare `cd C:\AIRS Foundry\...` chains that break on Windows when the path contains spaces. The tool already sets `current_dir` correctly at the OS level — the fix is preventing redundant and broken `cd` prefixes in the command string.

#### B.1 — Strip redundant `cd <working_dir> &&` from agent commands

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` — `rewrite_for_windows()` function (lines 143–159)

Add a step at the top of `rewrite_for_windows` that removes any `cd "<cwd>" &&` or `cd <cwd> &&` prefix when the working directory is already set by `run_command`:

```rust
fn rewrite_for_windows(cmd: &str, working_dir: &str) -> String {
    let mut out = cmd.to_string();

    // Strip any leading `cd "<working_dir>" &&` since current_dir is already set.
    // Handles both quoted and unquoted forms agents might generate.
    let quoted   = format!("cd \"{}\" && ", working_dir);
    let unquoted = format!("cd {} && ", working_dir);
    if out.starts_with(&quoted) {
        out = out[quoted.len()..].to_string();
    } else if out.starts_with(&unquoted) {
        out = out[unquoted.len()..].to_string();
    }
    // Also handle the PowerShell Set-Location form
    let set_loc = format!("Set-Location \"{}\" ; ", working_dir);
    if out.starts_with(&set_loc) {
        out = out[set_loc.len()..].to_string();
    }

    // ... existing Unix→PowerShell rewrites below
    out = out.replace(" | head -n ", " | Select-Object -First ");
    // ...
    out
}
```

Update the call site in `execute()` to pass `working_dir`:

```rust
// In execute(), change:
let rewritten = rewrite_for_windows(&command);
// To:
let rewritten = rewrite_for_windows(&command, &ctx.working_dir);
```

#### B.2 — Update tool description to reinforce "no cd prefix" rule

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` — `description()` (lines 21–24)

```rust
#[cfg(target_os = "windows")]
return "Execute a shell command in the working directory. Returns stdout and stderr. \
        IMPORTANT: Do NOT prefix commands with `cd <path> &&` — the working directory \
        is already set to the project root. Write commands directly, e.g. `npm install` \
        not `cd C:\\path && npm install`. On Windows, avoid Unix-only utilities \
        (head, tail, jq, grep in pipes). Use PowerShell-compatible alternatives.";
```

---

### Phase C — Dashboard Live Data (No Polling Interval + Wrong Event Names)

**Status:** ⬜ Pending  
**Goal:** Make the dashboard update in real time as agent ticks complete. Two separate bugs to fix: (1) `Dashboard.tsx` never starts the realtime listener, and (2) the event name `analytics:cost-update` in `dashboardStore.ts` does not match the emitted name `cost:update` in `events.rs`.

#### C.1 — Fix event name mismatch in `dashboardStore.ts`

**File:** `src/stores/dashboardStore.ts` — lines 347, 363

The Rust `events.rs` emits `cost:update` (constant `EVT_COST_UPDATE`). The store listens for `analytics:cost-update` — a name that is never emitted anywhere.

```ts
// Change line 347 from:
const costUnlistener = await listen<{ cost: number }>('analytics:cost-update', (event) => {
// To:
const costUnlistener = await listen<CostUpdatePayload>('cost:update', (event) => {

// Change line 363 from:
const failureUnlistener = await listen('analytics:api-failure', () => {
// To:
const failureUnlistener = await listen('agent:done', () => {
  get().refreshDashboard();
});
```

Also update the `CostUpdatePayload` type import to match `events.rs` — the payload has `session_id, tokens_in, tokens_out, cost_usd, total_today`:

```ts
// Update the budget alert comparison in the costUnlistener handler:
if (summary && event.payload.total_today > get().dailyBudget) {
  const alert: BudgetAlert = {
    // ...
    current_cost: event.payload.total_today,
    // ...
  };
}
```

#### C.2 — Call `startRealtimeUpdates()` from `Dashboard.tsx`

**File:** `src/components/dashboard/Dashboard.tsx` — lines 13–35

Add `startRealtimeUpdates` and `stopRealtimeUpdates` to the destructured store values and wire them into the mount/unmount `useEffect`:

```tsx
const {
  // ... existing
  refreshDashboard,
  startRealtimeUpdates,
  stopRealtimeUpdates,
} = useDashboardStore();

useEffect(() => {
  refreshDashboard();
  startRealtimeUpdates();
  return () => {
    stopRealtimeUpdates();
  };
}, []); // empty deps — mount once
```

#### C.3 — Add a 60-second polling interval for task-count freshness

**File:** `src/components/dashboard/Dashboard.tsx` — inside the mount `useEffect`

The `agent:done` event covers PA sessions (Mode 1). Mode 2 (LivingAgent via core-orchestrator) doesn't emit `agent:done` because the Rust side only calls it from `engine.rs`. Add a fallback interval:

```tsx
useEffect(() => {
  refreshDashboard();
  startRealtimeUpdates();

  const interval = setInterval(() => {
    refreshDashboard();
  }, 60_000);

  return () => {
    clearInterval(interval);
    stopRealtimeUpdates();
  };
}, []);
```

#### C.4 — Emit `cost:update` from `runner.rs` after each successful Mode 2 tick

**File:** `src-tauri/src/recipes/runner.rs` — lines 218–230 (after `record_worker_stats` in Mode 2 success path)

After a successful LivingAgent tick, emit a `cost:update` so the dashboard reacts without waiting for the 60s interval:

```rust
// After the record_worker_stats call in the Mode 2 Ok(response) branch:
crate::events::emit_worker_status_global(crate::events::WorkerStatusPayload {
    slug:            worker_slug.to_string(),
    status:          "idle".to_string(),
    previous_status: Some("working".to_string()),
    reason:          None,
    cooldown_seconds: None,
    timestamp:       chrono::Utc::now().to_rfc3339(),
});
// Trigger a dashboard refresh on the frontend side:
if let Some(app) = crate::events::app_handle() {
    let _ = app.emit("task:changed", serde_json::json!({
        "task_id": null,
        "kind": "worker_tick_complete",
        "status": null,
        "assigned_worker": worker_slug,
    }));
}
```

---

### Phase D — Agent Toast State Granularity

**Status:** ⬜ Pending  
**Goal:** Surface rich per-agent substates in the toast/roster UI so users see more than just `working → idle`. The QC agent running for 115 seconds with no feedback is the symptom this fixes.

#### D.1 — Emit `worker:tick-start` and `worker:tick-end` events from `runner.rs`

**File:** `src-tauri/src/recipes/runner.rs`

`emit_worker_tick_start` and `emit_worker_tick_end` are already defined in `events.rs` (lines 186–192) but are never called. Wire them:

```rust
// At the start of Mode 2 dispatch (after the qc pre-flight gate), before the
// invoke_worker call:
crate::events::emit_worker_tick_start(&self.app, crate::events::WorkerTickStartPayload {
    slug:      worker_slug.to_string(),
    mode:      "mode2".to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
});

// In the Ok(response) handler, before set_worker_status("idle"):
crate::events::emit_worker_tick_end(&self.app, crate::events::WorkerTickEndPayload {
    slug:      worker_slug.to_string(),
    outcome:   "success".to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
});

// In the Err(e) fallthrough handler:
crate::events::emit_worker_tick_end(&self.app, crate::events::WorkerTickEndPayload {
    slug:      worker_slug.to_string(),
    outcome:   "fallback".to_string(),
    timestamp: chrono::Utc::now().to_rfc3339(),
});
```

#### D.2 — Add `last_tick_at`, `tick_duration_ms`, `last_outcome` to `WorkerStatusPayload`

**File:** `src-tauri/src/events.rs` — lines 158–166

```rust
#[derive(Debug, Serialize, Clone)]
pub struct WorkerStatusPayload {
    pub slug:               String,
    pub status:             String,
    pub previous_status:    Option<String>,
    pub reason:             Option<String>,
    pub cooldown_seconds:   Option<u32>,
    pub timestamp:          String,
    // New fields:
    pub tick_duration_ms:   Option<u64>,   // how long the last tick took
    pub last_outcome:       Option<String>, // "success" | "fallback" | "error"
}
```

Populate `tick_duration_ms` in `runner.rs` by recording a start `Instant` before `invoke_worker` and computing elapsed before calling `set_worker_status("idle")`.

#### D.3 — Subscribe to `worker:tick-start` and `worker:tick-end` in `workerStore.ts`

**File:** `src/stores/workerStore.ts`

Extend `initEventListeners()` to subscribe to the new events and update per-worker state:

```ts
await listen<WorkerTickStartPayload>('worker:tick-start', (event) => {
  const { slug, timestamp } = event.payload;
  set(state => ({
    workers: state.workers.map(w =>
      w.slug === slug
        ? { ...w, tick_started_at: timestamp }
        : w
    )
  }));
});

await listen<WorkerTickEndPayload>('worker:tick-end', (event) => {
  const { slug, outcome, timestamp } = event.payload;
  set(state => ({
    workers: state.workers.map(w =>
      w.slug === slug
        ? { ...w, last_outcome: outcome, tick_ended_at: timestamp }
        : w
    )
  }));
});
```

---

### Phase E — QC Agent: Build-Failed Short-Circuit + Retry Count

**Status:** ⬜ Pending  
**Goal:** Prevent QC from invoking the LLM on tasks that are `build_failed` — it cannot review code that didn't install. Add an auto-retry mechanism so `build_failed` tasks requeue instead of sitting terminal.

#### E.1 — Add `retry_count` column to `tasks` table

**File:** `src-tauri/src/db/tasks.rs`

Add to the migration:

```rust
// In the CREATE TABLE statement:
"retry_count" INTEGER NOT NULL DEFAULT 0,

// Add helper:
pub fn increment_retry(&self, id: i64) -> Result<()> {
    self.conn.execute(
        "UPDATE tasks SET retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

pub fn get_retry_count(&self, id: i64) -> Result<i64> {
    let count = self.conn.query_row(
        "SELECT retry_count FROM tasks WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    Ok(count)
}
```

Also add `retry_count: i64` to the `Task` struct and the SELECT query.

#### E.2 — Auto-reset `build_failed` tasks to `ready` after grace period (Supervisor recipe instruction)

**File:** `src-tauri/src/recipes/defaults.rs`

The supervisor's system instructions should include a `build_failed` recovery clause. Find the supervisor recipe instructions string and append:

```
## Build-Failed Recovery
When you find tasks with status='build_failed':
- If retry_count < 3: call update_task_status(id, 'ready', assigned_worker=null) to re-queue the task.
  The worker will pick it up on their next tick and retry the build.
- If retry_count >= 3: leave as build_failed and log it as requiring manual intervention.
  Do NOT reset tasks that have failed 3+ times — they need human review.
```

#### E.3 — QC gate: skip `build_failed` tasks even if they leak into `qc_review`

**File:** `src-tauri/src/recipes/runner.rs` — lines 166–186 (qc pre-flight gate)

The current gate skips QC if zero `qc_review` tasks exist. Extend it to also filter out tasks that have `build_failed` as their previous state (i.e., were promoted to `qc_review` by `reconcile_stranded_task` incorrectly):

```rust
// In the qc_trigger_override block, after collecting tasks:
let reviewable: Vec<_> = tasks.iter()
    .filter(|t| !matches!(t.status, crate::db::tasks::TaskStatus::BuildFailed))
    .collect();

if reviewable.is_empty() {
    info!("[RecipeRunner] Only build_failed tasks in qc_review — skipping LLM invocation");
    self.set_worker_status(worker_slug, "idle");
    return Ok(());
}

let titles: Vec<String> = reviewable.iter()
    .map(|t| format!("#{}: {}", t.id, t.title))
    .collect();
```

---

### Phase F — Worker Frontend Scaffold Generation

**Status:** ⬜ Pending  
**Goal:** Ensure worker-frontend generates a complete, runnable Vite project scaffold when creating a new frontend project. The May 5 run generated component files but no `vite.config.js`, `public/index.html`, `tsconfig.json`, or `src/main.tsx` entry point.

#### F.1 — Add scaffold checklist to worker-frontend recipe instructions

**File:** `src-tauri/src/recipes/defaults.rs`

Find the `worker-frontend` recipe instructions and add a **mandatory scaffold section**:

```
## Frontend Project Scaffold (REQUIRED before writing components)

Before writing any React components, verify the following files exist. If any are missing, create them FIRST:

1. `vite.config.js` (or `vite.config.ts`):
```js
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
export default defineConfig({
  plugins: [react()],
  server: { port: 3000 },
})
```

2. `public/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8" /><title>App</title></head>
<body><div id="root"></div><script type="module" src="/src/main.tsx"></script></body>
</html>
```

3. `src/main.tsx`:
```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>)
```

4. `tsconfig.json` with `"jsx": "react-jsx"` and `"moduleResolution": "bundler"`.

Place ALL component files under `src/` (e.g. `src/components/Calendar.tsx`), NOT in the project root.

Only after these four files exist should you write or update component files.
```

#### F.2 — Add backend scaffold checklist to worker-backend recipe instructions

**File:** `src-tauri/src/recipes/defaults.rs`

Similarly, for the `worker-backend` recipe, add a mandatory checklist:

```
## Backend Project Scaffold (REQUIRED)

Before writing route handlers, verify these files exist:

1. `alembic.ini` — Alembic config (point `sqlalchemy.url` to the app's DB_URL).
2. `alembic/env.py` — Import `Base` from your models and set `target_metadata = Base.metadata`.
3. An initial migration: run `alembic revision --autogenerate -m "initial"` via bash tool.

Also verify `requirements.txt` package compatibility:
- If using Pydantic BaseModel with `from_attributes = True`, pin `pydantic>=2.0.0`.
- Always list `passlib[bcrypt]` explicitly (not just `bcrypt`).
- Use `email-validator>=2.0.0` (not `1.3.x`).
```

---

### Phase G — Startup Health-Check Race Condition

**Status:** ⬜ Pending  
**Goal:** Eliminate the 59 consecutive health-check failures logged at startup, which generate log noise and risk triggering the watchdog restart threshold.

#### G.1 — Add startup grace window to `OrchestratorClient::is_healthy()`

**File:** `src-tauri/src/sidecar/client.rs`

The health-check currently fires immediately. The core-orchestrator takes ~18 seconds to fully start. Add a `startup_grace_until` field that suppresses failure-logging during the known boot window:

```rust
pub struct OrchestratorClient {
    base_url: String,
    api_key:  String,
    agent_cache: Mutex<HashMap<String, String>>,
    // New:
    healthy_since:    Mutex<Option<std::time::Instant>>,
    startup_grace_until: std::time::Instant,
}

impl OrchestratorClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url,
            api_key,
            agent_cache: Mutex::new(HashMap::new()),
            healthy_since: Mutex::new(None),
            // 30s grace: don't log WARN for failures in the first 30s after app start
            startup_grace_until: std::time::Instant::now() + std::time::Duration::from_secs(30),
        }
    }
}
```

In `is_healthy()`, only log a WARN when outside the grace window:

```rust
pub async fn is_healthy(&self) -> bool {
    match /* existing health check */ {
        Ok(true) => true,
        _ => {
            if std::time::Instant::now() > self.startup_grace_until {
                warn!("[OrchestratorClient] health check failed — treating as unhealthy");
            }
            // else: silent during grace period
            false
        }
    }
}
```

---

### Phase H — Theme Visibility (CSS)

**Status:** ⬜ Pending  
**Goal:** Fix the near-black dark theme that renders `--text-secondary` illegible. Section 6.8 and Recommendation #6 both identify specific CSS variable values to change.

#### H.1 — Raise `--bg-primary` and `--bg-card` values

**File:** Find the global CSS custom property definitions. Based on Section 6.8 these live in `src/tabs/Graph.css` and in the global theme file. Search for `--bg-primary`:

```css
/* Current (too dark) */
--bg-primary: #0a0a0f;
--bg-card:    #0d0d12;

/* Target */
--bg-primary: #161622;
--bg-card:    #1e1e2e;
```

Apply the same change in every `.css` file that defines these variables (global theme + Graph.css).

#### H.2 — Raise `--text-secondary` opacity

**File:** Same theme files as H.1.

```css
/* Current */
--text-secondary: rgba(255, 255, 255, 0.5);

/* Target — enough contrast on #1e1e2e */
--text-secondary: rgba(255, 255, 255, 0.7);
```

Any element setting `opacity: 0.5` on text elements directly (rather than using the variable) should also be updated to `0.7`.

---

### Phase I — Idle Agent Pre-Flight LLM Cost Reduction

**Status:** ⬜ Pending  
**Goal:** Before invoking the LLM for a worker recipe tick, perform a lightweight SQLite task-count check. If the worker has zero assigned or in-progress tasks, skip the LLM call entirely. This eliminates the "dead token" invocations observed in Section 8 where `worker-frontend` was re-fired repeatedly and immediately exited with "No tasks assigned — idle", burning tokens on the system prompt and `list_tasks` call before concluding.

#### I.1 — Add pre-flight task count check in `runner.rs` before LLM dispatch

**File:** `src-tauri/src/recipes/runner.rs` — before the `invoke_worker` / `AgentEngine::new` call in the worker dispatch path.

```rust
// Before dispatching the worker LLM tick, check if it has any actionable tasks:
let has_work = {
    let db = self.tasks_db.lock().await;
    let assigned = db.list_tasks_by_status_and_worker("assigned", worker_slug).unwrap_or_default();
    let in_prog  = db.list_tasks_by_status_and_worker("in_progress", worker_slug).unwrap_or_default();
    let rejected = db.list_tasks_by_status_and_worker("rejected", worker_slug).unwrap_or_default();
    !assigned.is_empty() || !in_prog.is_empty() || !rejected.is_empty()
};

if !has_work {
    debug!("[RecipeRunner] {} has no actionable tasks — skipping LLM invocation", worker_slug);
    self.set_worker_status(worker_slug, "idle");
    return Ok(());
}
```

Also add `list_tasks_by_status_and_worker(status: &str, worker: &str)` helper to `src-tauri/src/db/tasks.rs`:

```rust
pub fn list_tasks_by_status_and_worker(
    &self,
    status: &str,
    worker: &str,
) -> Result<Vec<Task>> {
    let mut stmt = self.conn.prepare(
        "SELECT * FROM tasks WHERE status = ?1 AND assigned_worker = ?2 ORDER BY id",
    )?;
    let tasks = stmt.query_map(params![status, worker], Task::from_row)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}
```

**Impact:** Eliminates all "No tasks assigned — idle" LLM invocations. On a board with 5 workers polling every 60 seconds and only 1-2 active tasks, this reduces LLM calls by up to 80%.

---

### Phase J — Sidecar Log Noise Filtering

**Status:** ⬜ Pending  
**Goal:** Reduce the ERROR-level log noise from known transient Python connection errors that propagate through the Rust sidecar's log router. Section 6.6 confirms `ClosedResourceError` and `WinError 10054` tracebacks appeared 10+ times per second in the final session phase — these are benign reconnect events that are logged at ERROR by the Python tracebacks but should be DEBUG/WARN on the Rust side.

#### J.1 — Add pattern-based log level downgrade in `sidecar/mod.rs`

**File:** `src-tauri/src/sidecar/mod.rs` — in `spawn_log_router`, where Python stdout lines are forwarded to Rust logs.

```rust
// In the log forwarding loop, classify known transient error patterns before logging:
fn classify_python_log_level(line: &str) -> log::Level {
    // Known transient Windows networking errors — not real errors, part of reconnect flow
    if line.contains("ClosedResourceError")
        || line.contains("WinError 10054")
        || line.contains("ConnectionResetError")
        || line.contains("_call_connection_lost")
        || line.contains("send_nowait")
    {
        return log::Level::Debug;
    }
    // Memory reconnect success — informational
    if line.contains("Memory MCP reconnected successfully") {
        return log::Level::Info;
    }
    // Default: pass through as error (original behaviour)
    log::Level::Error
}

// Then in the loop:
let level = classify_python_log_level(&line);
log::log!(level, "[{}] {}", process_label, line);
```

**Impact:** Reduces log file noise by hundreds of lines per session. Prevents the log file from growing uncontrollably during long runs. Makes genuine errors visible again by not drowning them in reconnect noise.

---

### Phase K — builtin_tools Code Quality Fixes

**Status:** ⬜ Pending  
**Goal:** Fix silent defects in the Rust builtin tools identified in Section 13. These are not recipe changes — they are code correctness fixes that affect all agents regardless of instruction.

#### K.1 — `bash.rs`: Label stdout and stderr separately in output

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` — output concatenation (lines 85–100)

```rust
// Replace bare newline concat with labelled sections:
let mut output = String::new();
if !stdout.is_empty() {
    output.push_str("=== STDOUT ===\n");
    output.push_str(&stdout);
}
if !stderr.is_empty() {
    if !output.is_empty() { output.push('\n'); }
    output.push_str("=== STDERR ===\n");
    output.push_str(&stderr);
}
```

**Impact:** Agents can distinguish build warnings (stderr) from program output (stdout). Eliminates false-positive "build failed" reads when only warnings appear on stderr.

#### K.2 — `bash.rs`: Add `working_dir` existence pre-check before spawn

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` — `run_command()` before `cmd.current_dir()`

```rust
if !normalized_path.exists() {
    return Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("working_dir does not exist: {}", normalized_path.display()),
    ));
}
```

**Impact:** Converts opaque "Failed to spawn command" errors into self-diagnosing messages. Agent can then call `list_directory` to find the actual project root.

#### K.3 — `edit_file.rs`: Fire `PostFileWrite` hook after successful edit

**File:** `src-tauri/src/agent/builtin_tools/edit_file.rs` — after `fs::write` succeeds

`write_file.rs` fires `HookType::PostFileWrite`. `edit_file.rs` does the same logical operation but never fires the hook. Add the same hook dispatch that `write_file.rs` uses, after the successful `fs::write` call.

**Impact:** Graph engine's incremental updater (Phase 6 stubs), telemetry, and all hook consumers will see agent edits. Currently they are blind to all `edit_file` calls.

#### K.4 — `grep.rs`: Fix broken `case_insensitive` flag

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` — regex compilation block (~line 125) and match loop (~line 184)

```rust
// At regex compilation, apply (?i) prefix instead of lowercasing the line at match time:
let pattern_str = if params.case_insensitive {
    format!("(?i){}", params.pattern)
} else {
    params.pattern.clone()
};
let regex = regex::Regex::new(&pattern_str)?;

// Remove the to_lowercase() call in the match loop — match against original line directly.
```

**Impact:** Every agent call with `case_insensitive: true` currently returns zero results for any pattern containing uppercase letters. This is silently broken — agents get empty results and assume the pattern doesn't exist.

#### K.5 — `grep.rs`: Implement `context_lines` in the match loop

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` — match loop (lines 181–204) and `GrepMatch` struct

`context_lines` is deserialized (default: 2) but the loop never uses it. Add context window expansion:

```rust
let lines: Vec<&str> = content.lines().collect();
for (line_num, line) in lines.iter().enumerate() {
    if regex.is_match(line) {
        let ctx_start = line_num.saturating_sub(params.context_lines as usize);
        let ctx_end   = (line_num + params.context_lines as usize + 1).min(lines.len());
        let context_before: Vec<String> = lines[ctx_start..line_num].iter()
            .map(|s| s.to_string()).collect();
        let context_after: Vec<String>  = lines[line_num+1..ctx_end].iter()
            .map(|s| s.to_string()).collect();
        file_matches.push(GrepMatch {
            line_number,
            line_content: line.to_string(),
            match_positions: ...,
            context_before,
            context_after,
        });
    }
}
```

Also add `truncated: bool` to `GrepOutput` (set when `total_matches >= max_results`).

**Impact:** Eliminates 2–4 follow-up `read_file` calls that agents currently need after every grep hit. Context is now inline in the grep result.

#### K.6 — `grep.rs` / `glob.rs`: Fix glob filter to match full relative path, not just filename

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` (~line 156) and `src-tauri/src/agent/builtin_tools/glob.rs`

```rust
// In grep.rs glob check, build the relative path and match against it:
let relative_path = entry.path()
    .strip_prefix(&search_path).unwrap_or(entry.path())
    .to_string_lossy().replace('\\', "/");

if !pattern.matches(&relative_path) &&
   !pattern.matches(entry.file_name().to_string_lossy().as_ref()) {
    continue;
}
```

For `glob.rs`, switch to `globset` crate for `**` support, or use `glob::glob()` anchored at the absolute search path.

**Impact:** `grep(pattern: "import", glob: "src/**/*.tsx")` currently matches ALL `.tsx` files (the `**` path component is ignored). After fix, only files under `src/` are searched.

#### K.7 — `list_tasks_tool.rs`: Add missing statuses to schema; add `rejection_reason` and `retry_count` to output

**File:** `src-tauri/src/agent/builtin_tools/list_tasks_tool.rs` — lines 31, 53–65

```rust
// 1. Update schema enum to include new statuses:
"enum": ["ready", "assigned", "in_progress", "qc_review", "done",
         "failed", "build_failed", "cancelled", "rejected"]

// 2. Add rejection_reason and retry_count to each task in the result array:
json!({
    "id":               t.id,
    "title":            t.title,
    "status":           t.status.as_str(),
    "assigned_worker":  t.assigned_worker,
    "rejection_reason": t.rejection_reason,   // NEW
    "retry_count":      t.retry_count,         // NEW
    // ... existing fields
})
```

Also create `src-tauri/src/agent/builtin_tools/get_task_tool.rs` — a new tool with a single `id: i64` param that returns one full task record. Register it in `mod.rs` for both `FastDevelopment` and `Review` modes.

**Impact:** Without this fix, supervisor and workers cannot filter for `build_failed` or `rejected` tasks using the list tool — the LLM self-censors because those values are not in the schema enum. Workers retrying a rejected task have no visibility into why it was rejected.

#### K.8 — `update_task_status_tool.rs`: Add `build_failed` and `rejected` hook dispatch arms

**File:** `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — hook dispatch block (lines 129–134)

Phase A already adds `"rejected"` to the schema enum and adds a kick arm. This task adds the missing **hook dispatch** for `build_failed` and `rejected`:

```rust
let hook_type = match status.as_str() {
    "assigned"     => Some(HookType::OnTaskClaim),
    "done"         => Some(HookType::OnTaskComplete),
    "failed"       => Some(HookType::OnTaskFail),
    "build_failed" => Some(HookType::OnTaskFail),    // reuse OnTaskFail
    "qc_review"    => Some(HookType::OnQcReviewStart),
    "rejected"     => Some(HookType::OnQcReviewStart), // or dedicated OnTaskRejected
    _ => None,
};
```

**Impact:** Telemetry and any future hook consumer (graph engine, analytics) will observe `build_failed` and `rejected` transitions. Currently these transitions fire no hooks — they are invisible to downstream systems.

#### K.9 — `assign_task.rs`: Add `rejected` to dispatchable states; unify `VALID_WORKERS` with shared constant

**File:** `src-tauri/src/agent/builtin_tools/assign_task.rs` — lines 28–34 and 99–108

```rust
// 1. Add "rejected" to dispatchable gate:
let dispatchable = matches!(prev_status.as_str(),
    "ready" | "assigned" | "failed" | "rejected"  // ← add rejected
);

// 2. Replace hardcoded VALID_WORKERS with a reference to a shared constant
//    defined in recipes/defaults.rs:
pub const FLEET_WORKERS: &[&str] = &[
    "worker-backend",
    "worker-frontend",
    "worker-platform",
    "worker-mcp",
    "worker-cleanup",
];

// In assign_task.rs:
use crate::recipes::defaults::FLEET_WORKERS;
const VALID_WORKERS: &[&str] = FLEET_WORKERS;
```

**Impact:** Without this fix, the supervisor following Phase A recipe logic calls `assign_task` on a `rejected` task and gets the error "Task #N is in status='rejected' and cannot be (re)assigned." — the entire retry path breaks at the tool level. The `FLEET_WORKERS` unification prevents the two lists from drifting out of sync when new workers are added.

#### K.10 — `spawn_subagent.rs` / `spawn_parallel_subagents.rs`: Inject AutoDev MCP server

**Files:** `spawn_subagent.rs` line 109, `spawn_parallel_subagents.rs` line 121

Both tools hardcode `mcp_servers: Vec::new()`. Subagents therefore have zero tool access unless the parent passes explicit `tools` names — which LLMs frequently omit.

```rust
use crate::mcp_server::server::MCP_SERVER_PORT;

let autodev_mcp = SubagentMcpServer {
    url:     format!("http://127.0.0.1:{}/mcp", MCP_SERVER_PORT),
    label:   "autodev".to_string(),
    api_key: None,
};

let config = SubagentConfig {
    // ...
    mcp_servers: vec![autodev_mcp],
    // ...
};
```

**Impact:** QC subagents currently cannot call `read_file`, `grep`, or `bash`. They produce empty or hallucinated reviews. After this fix they have full access to all AutoDev tools via the local MCP server.

#### K.11 — `spawn_subagent.rs` / `spawn_parallel_subagents.rs`: Add `depth` field to `ToolContext`; enforce `MAX_SUBAGENT_DEPTH`

**Files:** `spawn_subagent.rs` line 111, `spawn_parallel_subagents.rs` line 123, `src-tauri/src/agent/context.rs` (`ToolContext` struct)

`depth: 0` is hardcoded regardless of nesting level. Add `depth: u32` to `ToolContext`, increment it when spawning, and guard against infinite recursion:

```rust
// In ToolContext struct:
pub depth: u32,   // 0 = top-level, n+1 = subagent level n

// In both spawn tools, at start of execute():
const MAX_SUBAGENT_DEPTH: u32 = 3;
if ctx.depth >= MAX_SUBAGENT_DEPTH {
    return ToolResult::error(format!(
        "Subagent depth limit ({MAX_SUBAGENT_DEPTH}) reached — complete this work directly."
    ));
}

// When building SubagentConfig:
depth: ctx.depth + 1,
```

**Impact:** Without this, a QC agent can spawn sub-reviewers that each spawn sub-reviewers indefinitely, creating unbounded LLM cost. `MAX_SUBAGENT_DEPTH = 3` allows one level of structured delegation (worker → QC-subagent → file-reviewer) without runaway recursion.

#### K.12 — `spawn_subagent.rs`: Add `task_context` schema field for grounded subagent prompts

**File:** `src-tauri/src/agent/builtin_tools/spawn_subagent.rs` — `input_schema()` and `execute()`

The default system prompt contains no information about the project, working directory, or task. Add an optional `task_context` field:

```rust
// In input_schema():
"task_context": {
    "type": "string",
    "description": "Optional: task title + acceptance criteria to orient the subagent."
},

// In execute(), build richer prompt when provided:
let system_prompt = match input.get("task_context").and_then(|v| v.as_str()) {
    Some(ctx_str) => format!(
        "You are an AutoDev subagent. Working directory: {}.\n\
         Task context: {}\n\
         Perform your assigned subtask and produce a structured summary.",
        ctx.working_dir, ctx_str
    ),
    None => DEFAULT_SUBAGENT_SYSTEM_PROMPT.to_string(),
};
```

**Impact:** QC subagents spawned to verify acceptance criteria currently have no knowledge of what those criteria are. With `task_context`, the parent passes the task title + criteria and the subagent produces grounded, targeted feedback.

---

### Phase L — MCP Server Fixes

**Status:** ⬜ Pending  
**Goal:** Fix the four critical defects in `src-tauri/src/mcp_server/` identified in Section 13. M1 is the highest severity — it means all fleet file writes currently land in the wrong directory.

#### L.1 — `tool_adapter.rs`: Accept `__working_dir` per-call instead of using hardcoded `~/.autodev`

**File:** `src-tauri/src/mcp_server/tool_adapter.rs` — `call_tool()` ToolContext construction (lines 124–127)

```rust
// Extract working_dir from the call arguments (reserved key convention):
let working_dir = args.get("__working_dir")
    .and_then(|v| v.as_str())
    .unwrap_or(&self.working_dir)  // fall back to server default only if not provided
    .to_string();

// Remove __working_dir from args before passing to the tool:
let mut tool_args = args.clone();
tool_args.remove("__working_dir");
tool_args.remove("__session_id");  // also strip L.3 field

let ctx = ToolContext {
    session_id:  /* see L.3 */,
    working_dir,
    call_id:     uuid::Uuid::new_v4().to_string(),
    depth:       0,
};
```

Update `sidecar/client.rs` where workers are created — inject `__working_dir` pointing to the actual project path in the agent's MCP configuration or as a default arg prefix convention.

**Impact:** This is the highest-severity fix in this entire document. Currently every `write_file` call from a fleet worker via MCP writes to `~/.autodev/src/main.py` instead of the project directory. The Phase 3.4 MCP integration is functionally broken for file writes without this fix.

#### L.2 — `mcp_server/server.rs`: Add API key auth middleware

**File:** `src-tauri/src/mcp_server/server.rs` — router setup (lines 33–58)

```rust
// On startup, load or create a machine-local API key:
let api_key = load_or_create_mcp_key();
// Writes to ~/.autodev/mcp.key (readable only by current user)

// Add auth middleware to the axum router:
let router = axum::Router::new()
    .route_service("/mcp", service)
    .layer(axum::middleware::from_fn(move |req, next| {
        let key = api_key.clone();
        async move { validate_bearer_token(req, next, &key).await }
    }));
```

Workers read the key from `~/.autodev/mcp.key` and send it as `Authorization: Bearer <key>`. The key is generated once at app start and stable for the lifetime of the installation.

**Impact:** Without auth, any local process (malicious npm package, VS Code extension, browser extension) can POST to port 8767 and call `write_file` or `bash` with the privileges of the Tauri process.

#### L.3 — `tool_adapter.rs`: Accept `__session_id` to group worker checkpoints

**File:** `src-tauri/src/mcp_server/tool_adapter.rs` — `call_tool()` (lines 124–127)

```rust
let session_id = args.get("__session_id")
    .and_then(|v| v.as_str())
    .map(String::from)
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
```

Workers include a stable `__session_id` (e.g., `worker-backend-task-42`) in every MCP call. All file checkpoints for that task then group under one session directory, making it possible to list and restore all changes made during a specific task.

**Impact:** Currently every `write_file` via MCP gets a different `session_id` → checkpoints are scattered in separate directories. There is no way to roll back "everything worker-backend wrote during task #42".

#### L.4 — `mod.rs`: Add `register_for_mcp_worker()` — restrict tool set exposed via MCP

**File:** `src-tauri/src/agent/builtin_tools/mod.rs`

```rust
pub fn register_for_mcp_worker(registry: &mut ToolRegistry) {
    // File tools — workers need these
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(edit_file::EditFileTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    // Task board — read + own-status updates only
    registry.register(Arc::new(list_tasks_tool::ListTasksTool));
    registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
    // NOT exposed: assign_task, create_task, planner, spawn_subagent, spawn_parallel_subagents
}
```

Use `register_for_mcp_worker` when initialising `AutodevMcpServer` in `mcp_server/server.rs`.

**Impact:** Workers can currently call `assign_task` to steal other workers' tasks or call `spawn_subagent` to fan out unlimited sub-agents via MCP. After this fix, the MCP surface is restricted to the tools workers legitimately need.

---

### Summary Table

| Phase | File(s) | Impact | Risk |
|-------|---------|--------|------|
| A — `rejected` status | `db/tasks.rs`, `update_task_status_tool.rs`, `AutoDevPanel.tsx` | Eliminates QC↔Assigned loop | Low — additive |
| B — bash path fix | `builtin_tools/bash.rs` | Fixes all build failures on Windows paths with spaces | Low — pure string transform |
| C — dashboard live data | `dashboardStore.ts`, `Dashboard.tsx`, `runner.rs` | Dashboard shows real-time cost/task counts | Low — UI only + 1 emit call |
| D — toast granularity | `events.rs`, `runner.rs`, `workerStore.ts` | Users see QC/agent progress during long ticks | Low — additive events |
| E — QC build-failed gate + retry | `db/tasks.rs`, `runner.rs`, `recipes/defaults.rs` | Stops LLM waste on unrunnable tasks; auto-recovers build failures | Medium — DB schema change requires migration |
| F — worker scaffold generation | `recipes/defaults.rs` | Workers generate complete runnable projects | Low — prompt engineering |
| G — startup grace window | `sidecar/client.rs` | Eliminates 59 noise failures at boot | Low — logging change |
| H — theme CSS visibility | Global theme CSS + `Graph.css` | Text legible on dark backgrounds | Low — CSS only |
| I — idle pre-flight | `runner.rs`, `db/tasks.rs` | Eliminates up to 80% of idle LLM invocations | Low — additive DB query |
| J — sidecar log noise | `sidecar/mod.rs` | Cleans up ClosedResourceError noise; genuine errors visible | Low — log routing change |
| K.1 — bash stdout/stderr labels | `builtin_tools/bash.rs` | Agents distinguish build warnings from errors | Low — string formatting |
| K.2 — bash working_dir pre-check | `builtin_tools/bash.rs` | Self-diagnosing working_dir errors | Low — additive guard |
| K.3 — edit_file PostFileWrite | `builtin_tools/edit_file.rs` | Graph engine + hooks see all agent file edits | Low — hook dispatch |
| K.4 — grep case_insensitive fix | `builtin_tools/grep.rs` | Fixes silently broken flag; agents get correct results | Low — 3-line fix |
| K.5 — grep context_lines | `builtin_tools/grep.rs` | Eliminates 2-4 follow-up read_file calls per grep hit | Low — loop expansion |
| K.6 — glob full-path matching | `builtin_tools/grep.rs` + `glob.rs` | `src/**/*.tsx` patterns work correctly | Low — path handling |
| K.7 — list_tasks schema + output | `builtin_tools/list_tasks_tool.rs` + new `get_task_tool.rs` | Agents can filter build_failed/rejected; see rejection_reason | Low — additive |
| K.8 — update_task hook arms | `builtin_tools/update_task_status_tool.rs` | build_failed + rejected transitions fire hooks | Low — match arm addition |
| K.9 — assign_task rejected gate | `builtin_tools/assign_task.rs`, `recipes/defaults.rs` | Supervisor can re-dispatch rejected tasks; FLEET_WORKERS unified | Low — additive match arm |
| K.10 — subagent MCP injection | `spawn_subagent.rs`, `spawn_parallel_subagents.rs` | Subagents have tool access; no more hallucinated reviews | Low — config change |
| K.11 — subagent depth guard | `spawn_subagent.rs`, `spawn_parallel_subagents.rs`, `context.rs` | Prevents unbounded recursive spawning and runaway cost | Low — additive guard |
| K.12 — subagent task_context | `spawn_subagent.rs` | Subagents are grounded in project and task context | Low — schema + prompt |
| L.1 — MCP working_dir per-call | `mcp_server/tool_adapter.rs`, `sidecar/client.rs` | Fixes all fleet file writes landing in wrong directory | **Critical — unblocks MCP** |
| L.2 — MCP auth | `mcp_server/server.rs` | Prevents unauthorized local access to write_file + bash | Medium — adds auth layer |
| L.3 — MCP session_id per-call | `mcp_server/tool_adapter.rs` | Worker checkpoints groupable by task | Low — additive |
| L.4 — MCP worker tool scope | `builtin_tools/mod.rs`, `mcp_server/server.rs` | Workers cannot call assign_task or spawn_subagent via MCP | Low — registration change |

---

## 13. Deep Dive — `builtin_tools/` & `mcp_server/` Analysis

This section documents findings from a full read of every file in `src-tauri/src/agent/builtin_tools/` and `src-tauri/src/mcp_server/`. The issues are grouped by subsystem. Many are silent behavioural defects that cause agents to waste turns, produce wrong output, or silently operate on the wrong directory — none produce Rust compiler errors.

---

### Group T — Shell & File I/O Tools

---

#### T1 · `bash.rs` — `rewrite_for_windows` has no `working_dir` parameter (cannot strip `cd` prefixes)

**File:** `src-tauri/src/agent/builtin_tools/bash.rs`

**Finding:** `execute()` calls `rewrite_for_windows(&command)` at line 55. The function signature is `fn rewrite_for_windows(cmd: &str)` — it receives only the command string, not `working_dir`. This means it **cannot** strip the redundant `cd "<working_dir>" && ` prefix that LLMs frequently prepend to commands. The rewriter is architecturally incapable of performing the fix described in Phase B of Section 12 without first changing this signature.

**Current code (lines 54-58, 143):**
```rust
let command = if cfg!(target_os = "windows") {
    let rewritten = rewrite_for_windows(&command);   // ← no working_dir passed
    ...
fn rewrite_for_windows(cmd: &str) -> String {        // ← can't strip cd prefix
```

**Required change — pass working_dir to the rewriter:**
```rust
// In execute():
let rewritten = rewrite_for_windows(&command, &ctx.working_dir);

// New signature:
fn rewrite_for_windows(cmd: &str, working_dir: &str) -> String {
    let mut out = cmd.to_string();
    // Strip redundant cd prefix in both quoted and unquoted form
    let quoted   = format!("cd \"{}\" && ", working_dir);
    let unquoted = format!("cd {} && ", working_dir);
    if out.starts_with(&quoted)   { out = out[quoted.len()..].to_string(); }
    else if out.starts_with(&unquoted) { out = out[unquoted.len()..].to_string(); }
    // Existing Unix→PowerShell rewrites follow...
    out
}
```

**Impact:** Every time an LLM generates `cd C:\AIRS Foundry\autodev\testing && npm install`, the command is passed to CMD verbatim. CMD tries to `cd` into the already-current dir, sometimes fails if the path has spaces and is unquoted. Fixing the signature unblocks Phase B entirely.

---

#### T2 · `bash.rs` — stdout and stderr are unlabelled in the returned output

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` lines 85-100

**Finding:** `stdout` and `stderr` are concatenated with a bare newline between them. When a build tool writes diagnostic info to stderr and results to stdout (common in `cargo build`, `npm run build`, `pytest`), the agent cannot distinguish which lines are errors and which are normal output. The structured error suggestion on line 106 only fires for `exit_code == 255 && "is not recognized"` — it misses the common case of a build command that exits 1 with a mixed stdout/stderr stream.

**Required change:**
```rust
// Replace the current concat logic with labelled sections:
if !stdout.is_empty() {
    output.push_str("=== STDOUT ===\n");
    output.push_str(&stdout);
}
if !stderr.is_empty() {
    if !output.is_empty() { output.push('\n'); }
    output.push_str("=== STDERR ===\n");
    output.push_str(&stderr);
}
```

**Impact:** Agents parsing build output will be able to identify whether a failure message came from stderr vs stdout, reducing false positives where a warning line in stderr causes the agent to treat a successful build as failed.

---

#### T3 · `bash.rs` — no `working_dir` existence pre-check before spawning

**File:** `src-tauri/src/agent/builtin_tools/bash.rs` `run_command()` line 127

**Finding:** `cmd.current_dir(&normalized_path)` is called without first checking whether the path exists. If `ctx.working_dir` is empty or a stale path from a previous project session, `cmd.output().await` returns `Err(Os { code: 2, kind: NotFound, ... })` which surfaces as the generic "Failed to spawn command: ..." error. The agent has no way to tell whether the command itself failed or the working directory didn't exist.

**Required change:**
```rust
// Add before cmd.current_dir():
if !normalized_path.exists() {
    return Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("working_dir does not exist: {}", normalized_path.display()),
    ));
}
```

**Impact:** Turns an opaque spawn error into a self-diagnosing one. The agent can then call `list_directory` to verify the project root and re-attempt with a corrected path.

---

#### T4 · `edit_file.rs` — `PostFileWrite` hook is NOT fired

**File:** `src-tauri/src/agent/builtin_tools/edit_file.rs` lines 122-140

**Finding:** `write_file.rs` fires `HookType::PostFileWrite` after every successful write (lines 66-87). `edit_file.rs` performs the same logical operation — writing new file content — but **never fires this hook**. Any downstream automation that listens for `PostFileWrite` (graph indexing, file watchers, event bus triggers) will not see changes made via `edit_file`. This also means the graph engine's planned incremental updater (Phase 6 stubs) will be blind to all agent edits performed through the `edit_file` tool.

**Required change (add after `fs::write` at line 128):**
```rust
// After the successful write, fire PostFileWrite — same as write_file.rs does.
let hook_ctx = HookContext {
    hook_type:  HookType::PostFileWrite.as_str().to_string(),
    timestamp:  chrono::Utc::now(),
    session_id: Some(ctx.session_id.clone()),
    agent_mode: None,
    tool_call:  Some(ToolCallContext {
        id:       ctx.call_id.clone(),
        name:     "edit_file".to_string(),
        input:    input.clone(),
        result:   Some(serde_json::json!({"bytes_written": new_content.len()})),
        is_error: false,
    }),
    file_path:  Some(file_path.to_string_lossy().to_string()),
    task_id:    None,
    worker_id:  None,
    extra: serde_json::json!({"bytes_written": new_content.len()}),
};
tokio::spawn(async move {
    HookRegistry::fire(&HookType::PostFileWrite, &hook_ctx).await;
});
```

**Impact:** Graph indexing, telemetry, and any future hook consumer will correctly observe edits made through `edit_file` — currently they see only `write_file` changes.

---

### Group S — Search Tools (grep / glob)

---

#### T5 · `grep.rs` — `case_insensitive` implementation is broken

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` lines 183-196

**Finding:** When `case_insensitive = true`, the code lowercases the line (`line.to_lowercase()`) at line 184 but continues to apply the original regex (which was compiled from the original, potentially mixed-case pattern). If the agent passes `pattern: "User"` with `case_insensitive: true`, the regex tries to match `"User"` against a lowercased line containing `"user"` — and finds nothing. The flag is silently broken for all mixed-case patterns.

**Current broken logic:**
```rust
let search_line = if params.case_insensitive {
    line.to_lowercase()   // ← lowercases the line
} else {
    line.to_string()
};
if let Some(mat) = regex.find(&search_line) { // ← but regex still expects "User"
```

**Required change — compile with `(?i)` flag instead:**
```rust
// In the regex compilation block (around line 125), apply (?i) prefix when flag is set:
let pattern_str = if params.case_insensitive {
    format!("(?i){}", params.pattern)
} else {
    params.pattern.clone()
};
let regex = match regex::Regex::new(&pattern_str) {
    Ok(r) => r,
    Err(e) => return ToolResult::error(format!("Invalid regex pattern: {e}")),
};

// Then remove the to_lowercase() call entirely — match against original line:
if let Some(mat) = regex.find(line) {
    let match_positions = vec![(mat.start(), mat.end())];
    file_matches.push(GrepMatch { line_number, line_content: line.to_string(), match_positions });
```

**Impact:** Every agent that passes `case_insensitive: true` is getting zero results for any pattern with uppercase letters. This forces duplicate tool calls with manually lowercased patterns, wasting turns.

---

#### T6 · `grep.rs` — `context_lines` parameter is parsed but never used

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` lines 43-44, 181-204

**Finding:** The `GrepInput` struct deserialises `context_lines: u32` (default 2) and the JSON schema advertises it. However, the match loop at lines 181-204 **never references `params.context_lines`**. The returned `GrepMatch` objects contain only the matched line — no surrounding context. Agents requiring context must issue a separate `read_file` call for every match location, consuming 2-4 extra turns per grep call.

**Required change — implement context window in the match loop:**
```rust
// After collecting all line matches for a file, expand each with context:
let lines: Vec<&str> = content.lines().collect();
for (line_num, line) in lines.iter().enumerate() {
    if regex.is_match(line) {
        let ctx_start = line_num.saturating_sub(params.context_lines as usize);
        let ctx_end   = (line_num + params.context_lines as usize + 1).min(lines.len());
        let context_before: Vec<String> = lines[ctx_start..line_num]
            .iter().map(|s| s.to_string()).collect();
        let context_after: Vec<String>  = lines[line_num + 1..ctx_end]
            .iter().map(|s| s.to_string()).collect();
        // Include context_before / context_after in GrepMatch struct
    }
}
```

Also add `context_before: Vec<String>` and `context_after: Vec<String>` to `GrepMatch`, and add `truncated: bool` to `GrepOutput` (set when `total_matches >= max_results`).

**Impact:** Agents currently need 3-5 tool turns to locate a function, read its context, and verify correctness. With context lines, this collapses to 1 grep call in most cases.

---

#### T7 · `grep.rs` / `glob.rs` — glob filter in grep matches filename only, not full relative path

**File:** `src-tauri/src/agent/builtin_tools/grep.rs` lines 156-165

**Finding:** The glob filter in `grep` applies `Pattern::matches()` against `entry.file_name()` only — the bare filename without its directory components. A filter like `glob: "src/**/*.py"` will never match because the pattern requires path separators but the input is just `"calendar.py"`. Only flat patterns like `"*.py"` work. The same root issue exists in `glob.rs` where `pattern.matches(&relative_path) || pattern.matches(entry.file_name())` gives a false positive for `**/*.rs` against any `.rs` filename regardless of its directory.

**Required change — match against full relative path:**
```rust
// In grep.rs, replace the glob check:
if let Some(glob_pattern) = &params.glob {
    let pattern = glob::Pattern::new(glob_pattern)
        .unwrap_or_else(|_| glob::Pattern::new("*").unwrap());
    let relative_path = entry.path()
        .strip_prefix(&search_path).unwrap_or(entry.path())
        .to_string_lossy().replace('\\', "/");
    // Match against BOTH the full relative path AND the filename for compatibility
    if !pattern.matches(&relative_path) && 
       !pattern.matches(entry.file_name().to_string_lossy().as_ref()) {
        continue;
    }
}
```

For glob.rs, switch to the `globset` crate which handles `**` correctly across path components, or use `glob::glob()` with the absolute path anchored at `search_path`.

**Impact:** Agents using `grep(pattern: "import", glob: "src/**/*.tsx")` to search frontend files currently get results from ALL `.tsx` files (the `**` is ignored). With the fix they get only files under `src/`.

---

### Group B — Task Board Tools

---

#### T8 · `list_tasks_tool.rs` — schema enum missing `build_failed` and `rejected`; output missing key fields

**File:** `src-tauri/src/agent/builtin_tools/list_tasks_tool.rs` lines 31, 53-65

**Finding (schema):** The `status` enum in `input_schema()` at line 31 lists:
```
["ready", "assigned", "in_progress", "qc_review", "done", "failed", "cancelled"]
```
`"build_failed"` and `"rejected"` (the new status added in Phase A) are absent. LLMs treat JSON schema enums as the complete valid set. An agent writing `list_tasks(status: "build_failed")` will self-censor because the schema says it's invalid — it will never filter for build-failed or rejected tasks, making the supervisor's Phase A recovery logic unreachable.

**Finding (output):** The task serialisation at lines 53-65 omits `rejection_reason` and `retry_count`. A worker agent picking up a previously-rejected task has no way to see WHY it was rejected or how many times it has been retried. This forces the agent to repeat the same mistake that caused rejection.

**Finding (missing tool):** There is no `get_task(id)` tool. Agents needing the full details of a single specific task must call `list_tasks()` (returning all tasks) and scan the array. On a board with 50+ tasks this wastes significant tokens per tick.

**Required changes:**

```rust
// 1. Add missing statuses to the enum:
"enum": ["ready", "assigned", "in_progress", "qc_review", "done",
         "failed", "build_failed", "cancelled", "rejected"]

// 2. Add rejection_reason and retry_count to the serialisation:
let result = tasks.iter().map(|t| json!({
    "id":                  t.id,
    "title":               t.title,
    "description":         t.description,
    "priority":            t.priority.as_str(),
    "task_type":           t.task_type.as_str(),
    "status":              t.status.as_str(),
    "acceptance_criteria": t.acceptance_criteria,
    "assigned_worker":     t.assigned_worker,
    "rejection_reason":    t.rejection_reason,   // NEW
    "retry_count":         t.retry_count,         // NEW
    "created_by":          t.created_by,
    "created_at":          t.created_at,
    "updated_at":          t.updated_at,
})).collect::<Vec<_>>();
```

For the missing `get_task` tool, create a new file `src-tauri/src/agent/builtin_tools/get_task_tool.rs` with a single `id: i64` parameter and register it in `mod.rs`.

---

#### T9 · `update_task_status_tool.rs` — `rejected` status missing from schema, description, hooks, and kick logic

**File:** `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` lines 22, 35, 110-118, 129-134

**Finding 1 — Schema enum at line 35 still uses old status list:**
```rust
"enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "build_failed", "cancelled"],
```
`"rejected"` is absent. QC agents calling `update_task_status(id, "rejected", ...)` will NOT generate this call if the JSON schema says it's invalid — the LLM will fall back to `"assigned"` instead, re-creating the exact QC↔Assigned loop the recipe change was meant to fix.

**Finding 2 — Description at line 22 is stale:** Does not mention `"rejected"` or explain when to use it vs `"failed"`.

**Finding 3 — Kick logic at lines 110-118 has no `"rejected"` arm:** When QC sets a task to `"rejected"`, the assigned worker is not woken up. The worker will remain idle until its next cron tick (up to 60 seconds), wasting a full minute before they even see the rejection.

**Finding 4 — Hook dispatch at lines 129-134 has no arms for `build_failed` or `rejected`:** No hook fires on build failure or rejection, so telemetry and any downstream automation is blind to these important lifecycle events.

**Required changes:**

```rust
// 1. Add "rejected" to the enum in input_schema():
"enum": ["ready", "assigned", "in_progress", "qc_review", "done",
         "failed", "build_failed", "cancelled", "rejected"]

// 2. Update description():
"Valid statuses: ready, assigned, in_progress, qc_review, done, failed, \
 build_failed, cancelled, rejected. Use 'rejected' when QC finds issues — \
 provide rejection_reason. Use 'failed' only for unrecoverable errors that \
 need human intervention. Do NOT use 'assigned' to send work back — use 'rejected'."

// 3. Add "rejected" arm to kick logic:
match status.as_str() {
    "qc_review" => crate::recipes::scheduler::kick("qc-agent"),
    "assigned"  => { if let Some(ref w) = assigned_worker { crate::recipes::scheduler::kick(w); } }
    "rejected"  => {
        // Wake the worker immediately so they see the rejection reason
        if let Some(ref w) = assigned_worker {
            crate::recipes::scheduler::kick(w);
        }
    }
    _ => {}
}

// 4. Add hook arms for build_failed and rejected:
let hook_type = match status.as_str() {
    "assigned"    => Some(HookType::OnTaskClaim),
    "done"        => Some(HookType::OnTaskComplete),
    "failed"      => Some(HookType::OnTaskFail),
    "build_failed"=> Some(HookType::OnTaskFail),   // reuse OnTaskFail
    "qc_review"   => Some(HookType::OnQcReviewStart),
    "rejected"    => Some(HookType::OnQcReviewStart), // or add OnTaskRejected hook type
    _ => None,
};
```

---

#### T10 · `assign_task.rs` — `rejected` tasks cannot be re-dispatched; `VALID_WORKERS` is out of sync

**File:** `src-tauri/src/agent/builtin_tools/assign_task.rs` lines 99-108, 28-34

**Finding 1 — `dispatchable` gate at lines 99-108 excludes `"rejected"`:**
```rust
let dispatchable = matches!(prev_status.as_str(), "ready" | "assigned" | "failed");
```
When the supervisor (following Phase A recipe logic) tries to re-dispatch a rejected task to its original worker using `assign_task(id, worker_slug)`, the tool returns:
```
"Task #N is in status='rejected' and cannot be (re)assigned."
```
This completely breaks the supervisor's rejected-task recovery flow.

**Finding 2 — `VALID_WORKERS` at lines 28-34 is hardcoded and not derived from `recipes/defaults.rs`:** Adding a new worker recipe (e.g., `worker-ml`) requires updating both files. There is no compile-time assertion or shared constant ensuring they stay in sync.

**Required changes:**

```rust
// 1. Add "rejected" to dispatchable states:
let dispatchable = matches!(prev_status.as_str(), "ready" | "assigned" | "failed" | "rejected");

// 2. Move VALID_WORKERS to a shared constant in recipes/defaults.rs or db/tasks.rs:
// In recipes/defaults.rs:
pub const FLEET_WORKERS: &[&str] = &[
    "worker-backend",
    "worker-frontend",
    "worker-platform",
    "worker-mcp",
    "worker-cleanup",
];

// In assign_task.rs:
use crate::recipes::defaults::FLEET_WORKERS;
const VALID_WORKERS: &[&str] = FLEET_WORKERS;
```

---

### Group A — Subagent Tools

---

#### T11 · `spawn_subagent.rs` / `spawn_parallel_subagents.rs` — subagents cannot reach `autodev-mcp-server`

**Files:** `spawn_subagent.rs` line 109, `spawn_parallel_subagents.rs` line 121

**Finding:** Both tools hardcode `mcp_servers: Vec::new()`. The `autodev-mcp-server` runs at `http://127.0.0.1:8767/mcp` and exposes all builtin_tools. Subagents launched with an empty `mcp_servers` list have **zero access to any AutoDev tools** unless the parent explicitly passes tool names in the `tools` array — which the LLM often omits. In practice, QC subagents spawned to review 4+ files cannot call `read_file`, `grep`, or `bash` because they have no tool registry. They produce empty or hallucinated reviews.

**Required change:**
```rust
// In both tools, inject the autodev MCP server automatically:
use crate::mcp_server::server::MCP_SERVER_PORT;

let autodev_mcp = crate::subagent::SubagentMcpServer {
    url:    format!("http://127.0.0.1:{}/mcp", MCP_SERVER_PORT),
    label:  "autodev".to_string(),
    api_key: None,
};

let config = SubagentConfig {
    ...
    mcp_servers: vec![autodev_mcp],  // always inject the local server
    ...
};
```

If the caller also passes explicit `tools`, those become the allowed subset. If `tools` is empty, the subagent has access to all tools via MCP.

---

#### T12 · `spawn_subagent.rs` — `depth` always 0; subagents can spawn subagents without depth limit

**Files:** `spawn_subagent.rs` line 111, `spawn_parallel_subagents.rs` line 123

**Finding:** `depth: 0` is hardcoded regardless of how deeply nested the spawn chain is. The `SubagentPool`'s `MAX_DEPTH` guard (wherever implemented) checks the `depth` field from `SubagentConfig`. Since depth is always 0, a subagent can always spawn more subagents — creating an unbounded recursion tree. A QC agent spawning sub-reviewers that each spawn more sub-reviewers is a realistic infinite-cost scenario.

**Required change:**
```rust
// Pass ctx.depth + 1 from parent context to child:
// ToolContext needs a depth field:
pub struct ToolContext {
    pub session_id:  String,
    pub working_dir: String,
    pub call_id:     String,
    pub depth:       u32,   // NEW — 0 for top-level agents, n+1 for subagents
}

// In both spawn tools:
let config = SubagentConfig {
    ...
    depth: ctx.depth + 1,  // increment depth
    ...
};
```

Add a guard at the start of `execute()`:
```rust
const MAX_SUBAGENT_DEPTH: u32 = 3;
if ctx.depth >= MAX_SUBAGENT_DEPTH {
    return ToolResult::error(format!(
        "Subagent depth limit ({MAX_SUBAGENT_DEPTH}) reached — cannot spawn further subagents. \
         Complete the work directly."
    ));
}
```

---

#### T13 · `spawn_subagent.rs` — `DEFAULT_SUBAGENT_SYSTEM_PROMPT` has no project or task context

**Files:** `spawn_subagent.rs` lines 148-153, `spawn_parallel_subagents.rs` lines 202-207

**Finding:** The default system prompt is:
> *"You are an AutoDev subagent — a focused worker spawned by a parent agent to handle a single, well-defined subtask. You have no memory of the parent's conversation."*

It contains no information about: the project's working directory, the task being worked on, the tech stack, or the acceptance criteria being verified. A QC subagent launched with the default prompt to verify `calendar.py` against acceptance criteria has no grounding and will produce generic output.

**Required change:** Accept a `working_dir` and optional `task_context` string in the tool schema and inject them into the system prompt:

```rust
// Add to input_schema():
"task_context": {
    "type": "string",
    "description": "Optional: task title + acceptance criteria to give the subagent grounding."
},

// Build a richer default prompt when task_context is provided:
let system_prompt = if let Some(ctx_str) = input.get("task_context").and_then(|v| v.as_str()) {
    format!(
        "You are an AutoDev subagent. Working directory: {}.\n\
         Task context: {}\n\
         Perform your assigned subtask and produce a structured summary.",
        ctx.working_dir, ctx_str
    )
} else {
    DEFAULT_SUBAGENT_SYSTEM_PROMPT.to_string()
};
```

---

### Group M — MCP Server

---

#### M1 · `tool_adapter.rs` — `working_dir` hardcoded to `~/.autodev` (all MCP file operations use the wrong path)

**File:** `src-tauri/src/mcp_server/tool_adapter.rs` lines 32-36

**Finding:** `AutodevMcpServer::new()` resolves `working_dir` as:
```rust
std::env::var("HOME")
    .or_else(|_| std::env::var("USERPROFILE"))
    .map(|p| PathBuf::from(p).join(".autodev").to_string_lossy().to_string())
    .unwrap_or_else(|_| ".".to_string())
```
This sets `working_dir` to `C:\Users\<user>\.autodev` on Windows. Every fleet worker that connects and calls `write_file(path: "src/main.py", content: "...")` will write to `C:\Users\<user>\.autodev\src\main.py` — not to the project being built. All file operations from MCP-connected workers use the wrong base directory unless the worker passes absolute paths in every single call.

**Required change:** The MCP server must accept a `working_dir` query parameter on the `/mcp` endpoint, or each `tools/call` request must include a `working_dir` in the arguments that `ToolContext` is built from:

```rust
// Option A: Accept working_dir in the MCP call arguments as a reserved key:
let working_dir = args.get("__working_dir")
    .and_then(|v| v.as_str())
    .unwrap_or(&self.working_dir)
    .to_string();

let ctx = ToolContext {
    session_id:  uuid::Uuid::new_v4().to_string(),
    working_dir,  // project root from caller, not ~/.autodev
    call_id:     uuid::Uuid::new_v4().to_string(),
    depth:       0,
};
```

Option B: Workers embed `working_dir` in the HTTP request headers or connection handshake (requires MCP spec extension).

**Impact:** This is the **single highest-severity finding** in this analysis. Every file written by a fleet worker via MCP is currently landing in `~/.autodev/` instead of the target project. The entire Phase 3.4 MCP server integration is functionally broken for any file write.

---

#### M2 · `tool_adapter.rs` — No authentication on port 8767

**File:** `src-tauri/src/mcp_server/server.rs` lines 33-58, `tool_adapter.rs` lines 106-131

**Finding:** The server binds to `127.0.0.1:8767` (localhost only). However, there is no token, API key, or session validation in `call_tool()`. Any process running on the same machine (another terminal, a browser extension, a VS Code extension, a malicious package) can POST to `http://127.0.0.1:8767/mcp` and call `write_file`, `bash`, or `update_task_status` with the privileges of the Tauri process.

**Required change:** Add a fixed API key (generated at app startup, stored in `~/.autodev/mcp.key`) and validate it in every request:

```rust
// In server.rs, add an auth layer before the MCP router:
let api_key = load_or_create_mcp_key();  // reads ~/.autodev/mcp.key or creates one

let router = axum::Router::new()
    .route_service("/mcp", service)
    .layer(axum::middleware::from_fn(move |req, next| {
        validate_mcp_auth(req, next, api_key.clone())
    }));
```

Workers that connect via `sidecar/client.rs` would read the key from the same file and pass it as a Bearer token or `X-API-Key` header.

---

#### M3 · `tool_adapter.rs` — Every MCP call gets a fresh `session_id`; checkpoints cannot be grouped by worker

**File:** `src-tauri/src/mcp_server/tool_adapter.rs` lines 124-127

**Finding:**
```rust
let ctx = ToolContext {
    session_id: uuid::Uuid::new_v4().to_string(),  // new UUID every call
    call_id:    uuid::Uuid::new_v4().to_string(),
    ...
};
```
`auto_checkpoint_before_write(session_id, file_path)` stores snapshots keyed by `session_id`. When worker-backend calls `write_file` 5 times in one task via MCP, each write gets a different `session_id` → each snapshot goes to a different directory under `~/.autodev/checkpoints/`. There is no way to list "all file changes made by worker-backend during task #42" because every write appears to come from a different session.

**Required change:** The MCP connection itself should carry a stable `session_id` that persists across all tool calls within a single worker tick. This can be implemented as a per-HTTP-session context in the `LocalSessionManager`, or passed as a reserved field `__session_id` alongside `__working_dir`:

```rust
let session_id = args.get("__session_id")
    .and_then(|v| v.as_str())
    .map(String::from)
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
```

---

#### M4 · `tool_adapter.rs` — All tools including task board management exposed to any MCP caller

**File:** `src-tauri/src/mcp_server/tool_adapter.rs` `list_tools()` lines 66-103

**Finding:** `list_tools()` returns `self.registry.all()` — which includes `assign_task`, `update_task_status`, `create_task`, `planner`, and `spawn_subagent`. A worker connecting via MCP can bypass the supervisor entirely and call `assign_task` to steal other workers' tasks, call `update_task_status` to mark tasks `done` without QC review, or call `spawn_subagent` to fan out unlimited subagents.

The `register_all()` path in `mod.rs` calls `register_for_mode(registry, PAMode::FastDevelopment)` which is full-access mode. The MCP server has no concept of per-connection tool scopes.

**Required change — introduce a `WorkerMode` tool set:**

```rust
// In mod.rs, add a new registration function for MCP/worker context:
pub fn register_for_mcp_worker(registry: &mut ToolRegistry) {
    // File and execution tools — workers need these
    registry.register(Arc::new(read_file::ReadFileTool));
    registry.register(Arc::new(write_file::WriteFileTool));
    registry.register(Arc::new(edit_file::EditFileTool));
    registry.register(Arc::new(bash::BashTool));
    registry.register(Arc::new(list_directory::ListDirectoryTool));
    registry.register(Arc::new(grep::GrepTool));
    registry.register(Arc::new(glob::GlobTool));
    // Task read is OK — workers need to see their assigned tasks
    registry.register(Arc::new(list_tasks_tool::ListTasksTool));
    // update_task_status is OK — workers update their OWN task's status
    registry.register(Arc::new(update_task_status_tool::UpdateTaskStatusTool));
    // NOT registered for MCP workers:
    // assign_task, create_task, planner, spawn_subagent, spawn_parallel_subagents
    // These are PA/Supervisor-only capabilities
}
```

Use `register_for_mcp_worker` when initialising the `AutodevMcpServer` registry instead of `register_all`.

---

### Summary Table — builtin_tools & mcp_server Findings

| ID | File | Category | Severity | Description |
|----|------|----------|----------|-------------|
| T1 | `bash.rs` | Shell | High | `rewrite_for_windows` missing `working_dir` param — cannot strip `cd` prefix |
| T2 | `bash.rs` | Shell | Medium | stdout/stderr unlabelled — agents can't distinguish build output from errors |
| T3 | `bash.rs` | Shell | Low | No `working_dir` existence check before spawn — opaque error |
| T4 | `edit_file.rs` | File I/O | High | `PostFileWrite` hook never fired — graph engine + event bus blind to edits |
| T5 | `grep.rs` | Search | High | `case_insensitive` broken — lowercases line but regex still expects original case |
| T6 | `grep.rs` | Search | Medium | `context_lines` param deserialized but never used — no context in results |
| T7 | `grep.rs` / `glob.rs` | Search | Medium | Glob filter matches filename only — `src/**/*.py` patterns don't work |
| T8 | `list_tasks_tool.rs` | Task Board | Critical | `build_failed`+`rejected` missing from schema; `rejection_reason`+`retry_count` absent from output |
| T9 | `update_task_status_tool.rs` | Task Board | Critical | `rejected` missing from schema enum — QC agents fall back to `assigned`, re-creating the loop |
| T10 | `update_task_status_tool.rs` | Task Board | High | No `rejected` kick, no `build_failed`/`rejected` hooks |
| T11 | `assign_task.rs` | Task Board | High | `rejected` not dispatchable — supervisor cannot re-dispatch rejected tasks |
| T12 | `spawn_subagent.rs` | Subagents | High | `mcp_servers: Vec::new()` — subagents have zero tool access |
| T13 | `spawn_subagent.rs` | Subagents | Medium | `depth` always 0 — unbounded recursive spawning possible |
| T14 | `spawn_subagent.rs` | Subagents | Low | Default system prompt has no project/task context |
| M1 | `tool_adapter.rs` | MCP Server | **Critical** | `working_dir` = `~/.autodev` for all workers — every write lands in wrong directory |
| M2 | `mcp_server/server.rs` | MCP Server | High | No auth on port 8767 — any local process can call write_file + bash |
| M3 | `tool_adapter.rs` | MCP Server | Medium | Fresh `session_id` per call — worker checkpoints ungroupable |
| M4 | `tool_adapter.rs` | MCP Server | Medium | All tools exposed via MCP — workers can call assign_task, spawn_subagent |

*Critical = silent data corruption or broken core loop. High = significant token waste or broken feature. Medium = quality/UX degradation.*
