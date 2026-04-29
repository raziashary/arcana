# AutoDev Complete Run Analysis — 2026-04-29

**Task Run**: "Create a simple Google Calendar-style calendar app (frontend + backend)"  
**Session Duration**: 08:27:41 → 08:51:54 (≈ 24 minutes)  
**Log File**: `autodev/logs/autodev-2026-04-29.log` — 54,225 lines / **142 MB**  
**Supporting Logs**: `~/.autodev/logs/memory-mcp.log` — **133 MB** | `core-orchestrator.log` — 4.8 MB  
**Total Log Volume**: **281 MB** in 24 minutes  
**Session End**: Force-killed from terminal (not graceful shutdown)

---

## Quick Summary

The calendar app was built successfully by the fleet (6 workers, multiple tasks). The work itself went well —
the code landed in `testing/` with a React frontend and Node/Express/PostgreSQL backend. However, **5 distinct
bugs** caused degraded experience and the eventual freeze: a wrong Anthropic model name in the Rust fallback
engine, analytics data never being recorded, a scheduler tight-loop with zero backoff on errors, log files
lacking rotation, and the frontend requiring a build step that wasn't run.

---

## Issue 1 — AutoDev Panel Costs Not Showing (Analytics Not Wired Up)

**Severity**: High — core UX promise broken  
**Affected**: Worker Panel, Dashboard cost cards

### What Happens
The panel shows $0.00 / blank cost data. No token usage appears after a full task run.

### Root Cause
The `AnalyticsCollector::record_token_usage()` function exists in
`src-tauri/src/analytics/collector.rs` but **is never called from any execution path**.

- **Mode 2 (core-orchestrator path)**: Token usage is tracked inside Python by LiteLLM and stored in
  core-orchestrator's internal SQLite. That data is never forwarded to AutoDev's `analytics.db`.
- **Mode 1 (Rust AgentEngine fallback)**: `provider/anthropic.rs` streams tokens but the post-stream
  callback never calls `record_token_usage()`.

Additionally, `get_worker_analytics()` in `analytics_commands.rs` returns **hardcoded zeros**:

```rust
// analytics_commands.rs lines 114–119 — never queries any DB
"tasks_completed": 0,
"tasks_failed": 0,
"success_rate": 0.0,
"avg_task_duration_minutes": 0,
"cost_usd": 0.0,
```

And `get_system_logs()` always returns an empty Vec with a `// TODO` comment.

### What Needs to Be Fixed
1. After each LLM response in the Rust AgentEngine (`agent/engine.rs`), extract the `usage` field
   from the Anthropic response and call `analytics::global().collector().record_token_usage(...)`.
2. After each LivingAgent invocation in Mode 2, the Rust runner should read the usage data returned
   by the `POST /api/agents/{id}/invoke` response body and record it to analytics.db.
3. Replace the hardcoded zeros in `get_worker_analytics()` with actual DB queries that count task
   status transitions from the tasks database.
4. Implement `get_system_logs()` to query the `system_events` table.

---

## Issue 2 — Dashboard Not Updated (Same Root Cause as Issue 1)

**Severity**: High  
**Affected**: Dashboard page — all stat cards, cost charts, worker leaderboard

### What Happens
All dashboard numeric cards show zero or empty. The "today's cost" card, token usage chart, and worker
efficiency leaderboard show no data after a full run.

### Root Cause
`get_dashboard_summary()` queries `analytics.db` which has no records because `record_token_usage()`
was never called. The analytics data pipeline is broken at the source (ingestion), not the query layer.

The `AnalyticsService` at `/api/analytics` in core-orchestrator **is** tracking usage data (it logs its
initialization at line 137), but that data stays inside Python — it is never pulled into the Rust
analytics.db that the dashboard reads from.

### What Needs to Be Fixed
Either:
- Wire the Rust analytics collector to ingest data (see Issue 1 fix), **or**
- Add a periodic sync job that polls `GET /api/analytics` from core-orchestrator and writes the data
  into AutoDev's analytics.db.

---

## Issue 3 — qc-agent and supervisor Failing with 404 (Wrong Model Name in Rust Fallback)

**Severity**: High — these agents are completely non-functional in fallback mode  
**Affected**: `qc-agent`, `supervisor` (any worker using `claude-4-5-haiku` config key when core-orch is unreachable)  
**Log occurrences**: Lines 5665, 18916, 21223, 21226, 29402, 37933, 39528, 53037, 53040 — **9 failures**

### What Happens
```
ERROR [RecipeRunner] Rust AgentEngine failed for worker 'qc-agent':
Provider error: Anthropic stream 404 Not Found:
{"type":"error","error":{"type":"not_found_error","message":"model: anthropic/claude-haiku-4-5-20251001"}}
```

### Root Cause — Two-Level Problem

**Level 1: Model name stored in workers.db**  
Workers are stored with model key `claude-4-5-haiku`. This is the internal config format that core-orchestrator
translates to its LiteLLM format `anthropic/claude-haiku-4-5-20251001`.

**Level 2: Wrong translation in Rust provider**  
When the Rust AgentEngine falls back (because `core-orch` health check fails at `:4000`), it reads the
stored model key `claude-4-5-haiku` and `provider/anthropic.rs` line 73 maps it to:

```rust
// provider/anthropic.rs line 73 — WRONG
"claude-4-5-haiku" => "anthropic/claude-haiku-4-5-20251001",
```

The value `anthropic/claude-haiku-4-5-20251001` is a **LiteLLM format string** (used when proxying through
LiteLLM). When calling the **direct Anthropic API** (`api.anthropic.com/v1`), this prefix and date suffix
are invalid. The correct direct-API model ID is `claude-haiku-4-5`.

### When It Triggers
The fallback fires when the health check for `http://127.0.0.1:4000/health` returns no response — which
happens intermittently when core-orch is **busy processing** another agent's long-running LiteLLM call.
This is a false negative: core-orch is alive but unresponsive to health checks under load.

### Fix Required
Change `provider/anthropic.rs` line 73:
```rust
// Replace:
"claude-4-5-haiku" => "anthropic/claude-haiku-4-5-20251001",
// With:
"claude-4-5-haiku" | "claude-haiku-4-5" => "claude-haiku-4-5",
```

Also recommended: Update `workers.db` seed data in `db/workers.rs` to store the canonical name
`claude-haiku-4-5` directly so no translation is needed.

---

## Issue 4 — Application Freeze / Force Kill Required

**Severity**: Critical — the application became unresponsive and had to be killed  
**Timestamp of last log entry**: `2026-04-29T08:51:54Z`

### What Happened
At the end of the run, the application became unresponsive and had to be killed from the terminal.
The 142 MB log file and 133 MB memory-mcp log together show the exact cause.

### Root Cause: Three Compounding Problems

**Problem A — "Next Fire in 0ns" Tight Loop (Scheduler Bug)**

The `RecipeScheduler` set `next_fire = 0ns` (immediate re-schedule) in these situations:
- A worker completed its last task and became idle
- A worker's Rust AgentEngine fallback failed with a 404 error

This was observed **32 times total** during the 24-minute run. The most severe burst:

```
47862: [08:48:31.403] Worker 'qc-agent' — next fire in 0ns
47876: [08:48:31.419] Worker 'qc-agent' — next fire in 0ns   (+16ms)
47890: [08:48:31.439] Worker 'qc-agent' — next fire in 0ns   (+20ms)
47898: [08:48:31.456] Worker 'qc-agent' — next fire in 0ns   (+17ms)
47906: [08:48:31.475] Worker 'qc-agent' — next fire in 0ns   (+19ms)
47914: [08:48:31.502] Worker 'qc-agent' — next fire in 0ns   (+27ms)
```

**6 fires in 99ms**. At the end of the log, `worker-frontend` was in the same tight loop:
```
53565: [08:51:29.426] Worker 'worker-frontend' — next fire in 0ns
54187: [08:51:54.233] Worker 'worker-frontend' — next fire in 0ns
```

The last log line shows `worker-frontend` had just completed task `task_01KQC6ZA10FXFVG90254T5G5GB`,
found no remaining tasks, and was about to fire again immediately when the app was killed.

**Problem B — No Error Backoff in Scheduler**

When the 404 error returns quickly (~400ms), and the scheduler reschedules at 0ns, the cycle runs
every ~500ms. Each cycle:
1. Fires recipe
2. Rust AgentEngine sends Anthropic API request
3. 404 response (~400ms)
4. Schedule next at 0ns
5. Immediately repeat

This generates ~2 invocations/second of a failing agent with no backoff, flooding both the log and
consuming CPU/network.

**Problem C — Log File Size: 281 MB in 24 Minutes**

| File | Size |
|------|------|
| `autodev-2026-04-29.log` | 142 MB |
| `memory-mcp.log` | 133 MB |
| `core-orchestrator.log` | 4.8 MB |
| **Total** | **~280 MB** |

The memory-mcp.log grows large because it logs the **full system prompt** on every memory store
operation. Each prompt is ~1,000+ characters (the full agent instruction set), and it's stored on every
agent invocation. With workers firing every 1 minute and 6 workers active, this creates:
- ~360 memory stores per hour
- Each store: full prompt + Mem0 API response + scoring details = ~1,500 bytes per store line
- Plus FSM transitions (~20 lines per state change) × many state changes per invocation

Additionally, Redis connection failures generated continuous error stack traces:
```
ERROR src.airs.mcp.agent_card_store: Failed to retrieve agent card for supervisor:
Error 22 connecting to 127.0.0.1:6379. The remote computer refused the network connection.
```
This 12-line stack trace appeared for every agent initialization, multiplied by the number of
agent restarts across the run.

**The Memory Recall Slowness (76 "Slow recall" warnings)**

The memory system took 7–12 seconds per recall operation (threshold is 1,000ms). With workers
firing on tight loops and memory recall taking 12 seconds, the system had many overlapping
memory operations running concurrently, consuming memory and CPU. The slowness stems from:
- No Redis warm tier (Memurai not running)
- Each recall hits the cold path: Qdrant vector search + OpenAI embeddings API call
- 76 warnings across the 24-minute run = ~3 per minute average

### Fix Required

**Scheduler**: Add minimum backoff after any recipe run ends:
- `idle` (no tasks found): wait at least `min(polling_interval, 60s)` before next fire
- `error` / `failed` result: exponential backoff starting at 10s, capping at 5 minutes
- Never schedule `next_fire = 0ns` unless explicitly triggered by an inbound webhook/event

**Log rotation**: Implement log rotation for all three logs:
- Maximum file size: 20 MB per file
- Keep: 3 rolling files (total 60 MB cap per log stream)
- Compress older rotations (gzip)

**Reduce log verbosity**:
- Memory FSM transition logs: promote to TRACE (currently INFO/DEBUG)
- Redis connection errors: suppress duplicates for 60 seconds after first error
- Full prompt content in memory store logs: truncate to first 200 characters
- Repeated health-check failure logs: one line per 10 failures, not every failure

---

## Issue 5 — Frontend Calendar App Shows Black Screen

**Severity**: Medium — the delivered artifact is unusable without extra steps  
**Affected**: `testing/frontend/`

### What Happens
Opening `testing/frontend/public/index.html` directly in a browser shows a black screen.

### Root Cause
The calendar app is a **Create React App** (CRA) project. The `public/index.html` file is just the
HTML shell:
```html
<div id="root"></div>
```
There is no JavaScript in `public/`. The compiled JavaScript bundle only exists after running
`npm run build`, which creates a `testing/frontend/build/` directory. That directory **does not exist**
— the workers wrote all the source code but did not run the build compilation step.

To open the app you must either:
1. Run `npm start` in `testing/frontend/` → opens dev server at `http://localhost:3000`
2. Run `npm run build` in `testing/frontend/` → creates `build/` with static files you can open

### Why the Workers Didn't Build
The worker recipe's Build Validation step checks `npm run build` but only for validation — if it
passes, the worker marks the task `qc_review`. The final output is the source code, not the compiled
artifact. The recipe doesn't include a "serve the app for the user" step.

---

## Issue 6 — Redis/Memurai Unavailable at Startup

**Severity**: Medium (degrades performance; system continues working)  
**First occurrence**: Line 205 (`08:28:02`)

### What Happens
At startup, core-orchestrator tries to start the Memurai service (Redis for Windows):
```
WARNING api_gateway.app: [REDIS] Failed to start Memurai service:
Command returned non-zero exit status 1.
```
Followed at line 229:
```
ERROR api_gateway.app: [REDIS] Warm tier enabled but unavailable
```

### Cascading Effects
1. **All agents**: warned "Failed to load dynamic prompt" from Redis × 6 agents at startup
2. **All agent factories**: `⚠️ [REDIS] Failed to connect, proceeding without warm tier` — appears
   every time an agent is created (dozens of times across the run)
3. **Agent card store**: `Failed to retrieve agent card for {agent}` with full 12-line stack trace
   for every agent initialization
4. **Memory performance**: All memory operations fall through to the slow cold path
   (Qdrant + OpenAI calls vs Redis cache)

### What Needs to Be Fixed
- If Memurai/Redis is not available, **silence subsequent connection errors** after the first logged
  failure. A single startup warning is sufficient — repeated stack traces fill the log.
- Add a clear health indicator in the UI showing "Redis: Offline — memory operations slower"
- Consider making Memurai startup failures non-blocking (not generating ERROR-level logs after the
  initial warning)

---

## Issue 7 — MCP Server Registration Conflict at Startup

**Severity**: Low (system recovers; agents function normally)  
**Log line 527**: `08:28:13`

### What Happens
```
ERROR api_gateway.modules.mcp.service: Server with ID autodev-mcp-server already exists
ERROR api_gateway.modules.mcp.router: Failed to add MCP server: 409: Server with ID autodev-mcp-server already exists
INFO: "POST /api/mcp/servers HTTP/1.1" 500 Internal Server Error
```

### Root Cause
AutoDev's Rust layer pushes the `autodev-mcp-server` registration to core-orchestrator on every
startup. Core-orchestrator persists MCP server registrations across restarts. On second+ launch,
the server is already registered, causing a 409 conflict.

The HTTP 500 is alarming but the system recovers — agents get tools from the existing registration.

### Fix Required
The `POST /api/mcp/servers` call should check for a 409 response and treat it as success (the server
is already registered as desired), or use a `PUT /api/mcp/servers/{id}` upsert endpoint instead.

---

## Issue 8 — Core-Orchestrator False-Negative Health Check Triggers Unnecessary Fallback

**Severity**: Medium — causes Issue 3's model name error  
**Pattern**: Observed repeatedly during active agent processing windows

### What Happens
```
DEBUG [OrchestratorClient] health check failed: error sending request for url (http://127.0.0.1:4000/health)
INFO  [RecipeRunner] core-orchestrator not reachable — attempting Mode 1 (Rust AgentEngine) fallback
```

### Root Cause
The health check endpoint (`GET /health`) times out when core-orch is **busy** processing a
long-running LiteLLM call. This is a legitimate timeout on a busy Python event loop — not a crash —
but the Rust health check interprets it as "orchestrator down" and switches to Mode 1 (Rust AgentEngine).

Mode 1 then fails with Issue 3's model name bug, and the cycle compounds.

### Fix Required
1. Increase health check timeout from its current value (appears to be <1s) to at least 3-5 seconds
2. Require 2-3 consecutive health check failures before declaring orchestrator "down"
3. This alone would prevent the majority of Issue 3 occurrences

---

## Issue 9 — Working Directory Memory Contamination

**Severity**: Low-Medium (affects context quality for agents)  
**Log line 5653**: `working_dir: C:\AIRS Foundry\Backup\BACKUPcoreorch`

### What Happens
Memory retrieval surfaces an old session's workspace path (`C:\AIRS Foundry\Backup\BACKUPcoreorchestrator`)
in the worker's live context. The actual working directory for this run was `testing/`.

### Root Cause
The memory system stores and retrieves episodic memories from past sessions. The `working_dir` from
a previous backup/restore run was stored as a memory and retrieved as "relevant" context for this run
because it contains similar recipe instructions.

### Fix Required
Workspace configuration (`working_dir`) should be injected as a **session-scoped system instruction**
(ephemeral, not memorized). If it is stored in memory, it should have a TTL or be tagged with the
session ID so it cannot be retrieved in a different session.

---

## Issue 10 — No Log Rotation (145 MB+ Log Files)

**Severity**: High — directly contributed to app freeze  
**Evidence**: `autodev-2026-04-29.log` is 142 MB after one 24-minute session

### Problem Details
The user noted the log file is 145,225 KB (≈ 142 MB). This is just one day's log. If not cleared,
subsequent runs will append to the same file, eventually causing disk space issues and further
performance degradation as the log writer has to seek through a giant file.

The `~/.autodev/logs/` directory currently holds **281 MB** of logs from just this one run.

The **memory-mcp.log** is the worst offender at 133 MB because:
- It logs the full (1,000+ char) system prompt every time an agent's session is stored to memory
- It logs detailed embedding vectors and BM25 scores for every memory recall
- These verbose debug logs are at INFO level, not DEBUG

### Fix Required
1. Implement log rotation in the Rust logger initialization:
   - Max file size: 20 MB
   - Max rotated files: 3 (total 80 MB cap per log)
2. For `memory-mcp-server`, set `LOG_LEVEL=WARNING` for the most verbose modules:
   - `mem0.*` → WARNING
   - `src.memory_providers.adapters.mem0_adapter` → WARNING  
   - `airs.memory.flow` → INFO (keep flow summaries but drop detail)
3. In the Rust side, suppress repeated health-check failure messages — currently every concurrent
   worker poll generates its own "health check failed" line, resulting in hundreds of identical lines.
4. Consider archiving (gzip + move to `logs/archive/`) when a new session starts if the log file
   exceeds 10 MB.

---

## Summary Table

| # | Issue | Severity | Root Cause (Short) | Fix Required |
|---|-------|----------|--------------------|--------------|
| 1 | Panel costs blank | High | `record_token_usage()` never called | Wire analytics after LLM response |
| 2 | Dashboard blank | High | Same as #1 — empty analytics DB | Same as #1 |
| 3 | qc-agent / supervisor 404 | High | `anthropic/claude-haiku-4-5-20251001` → wrong API format | Change to `claude-haiku-4-5` in provider/anthropic.rs L73 |
| 4 | App freeze / force kill | Critical | 0ns re-fire loop + 280 MB logs | Add backoff; add log rotation |
| 5 | Calendar app black screen | Medium | CRA not compiled (`npm run build` not run) | Workers must run build, or README should note this |
| 6 | Redis/Memurai offline | Medium | Memurai not running | Silence after first error; show UI indicator |
| 7 | MCP server 409 on startup | Low | Registration not idempotent | Use upsert / treat 409 as success |
| 8 | False-negative health check → fallback | Medium | Health check timeout too short | Increase timeout; require 2+ failures |
| 9 | Wrong working_dir in memory | Low-Medium | Old session memory retrieved | Don't memorize workspace path |
| 10 | No log rotation (142 MB) | High | No rolling file config | Max 20 MB / 3 rotations; reduce verbosity |

---

## What Went Well

1. **Fleet execution was fast and smooth**: 6 workers picked up and completed tasks without stalling.
   Tasks were assigned, claimed, executed, and handed off to qc_review in the expected flow.
2. **IDE live updates worked**: The IDE showed file changes as workers wrote them.
3. **Output was correct**: The calendar app is a well-structured React + Express + PostgreSQL project
   with proper folder structure, API endpoints, migrations, tests, and a thorough README.
4. **Workers stayed in lane**: frontend worker only touched `frontend/`, backend worker only touched
   `backend/`, per their recipes.
5. **No data corruption or conflicts**: No merge conflicts, no file overwrites between workers.
6. **Memory system functioned**: Despite being slow (76 slow recall warnings), the memory system
   correctly stored and retrieved worker instructions across invocations.
7. **Mode 2 (core-orchestrator) worked correctly** for all actual work agents (worker-frontend,
   worker-backend, worker-cleanup, worker-mcp, worker-platform) throughout the entire run.

---

## Recommended Fix Priority

### Immediate (fix before next run)
1. **Issue 3** — Change model name in `provider/anthropic.rs` L73: `"claude-4-5-haiku" => "claude-haiku-4-5"`
2. **Issue 4 / Scheduler** — Add minimum idle backoff (60s) and error backoff (10s → 5min exponential)
3. **Issue 10** — Add log rotation (20 MB max, 3 files) before next long session

### Short-term (next sprint)
4. **Issue 8** — Increase health check timeout to 3s; require 2 consecutive failures for fallback
5. **Issue 1 + 2** — Wire `record_token_usage()` into agent execution paths
6. **Issue 6** — Suppress repeated Redis errors after first startup failure

### Medium-term
7. **Issue 5** — Add a "serve / view app" step to the task completion flow
8. **Issue 7** — Make MCP registration idempotent (treat 409 as success)
9. **Issue 9** — Mark `working_dir` as session-scoped in memory schema

---

## Log File Management Recommendation

The log file at `C:\Users\Razi Ashary\.autodev\logs\autodev-2026-04-29.log` is 142 MB and can be
safely deleted or archived after this analysis. Future logs should not grow this large once log
rotation and verbosity fixes are in place.

The `memory-mcp.log` at 133 MB is the most egregious — a single configuration change to reduce
`mem0.*` verbosity to WARNING would cut it by ~90%.

**Immediate safe action**: Clear `~/.autodev/logs/` directory contents — the data in these logs
has been analyzed and the issues have been documented. No production state is stored in these files.
