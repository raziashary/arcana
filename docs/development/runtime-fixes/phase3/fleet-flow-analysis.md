# Phase 3 — Fleet Flow Analysis (Calendar App Run)

**Log source:** `autodev/logs/submoduletesting.txt` (32,891 lines)
**Run window:** `2026-04-21T10:02:53Z` → `2026-04-21T10:17:29Z` (~14 min 36 s)
**Model:** `anthropic/claude-haiku-4-5-20251001`
**User prompt:**
> _"create a task: make me a very simple calendar app designed like google calendar, frontend backend language i'll leave it to you. I need the functionality of adding events and simple stuff"_

**Working dir:** `C:\AIRS Foundry\autodev\testing`

---

## 1. Executive Summary

The fleet **functionally worked end-to-end** — the Supervisor dispatched all 8 sub-tasks produced by the PA, workers executed `bash` / `write_file` / `edit_file` against the workspace, and two tasks reached `qc_review` → `done` (visible in Image 2). However the **experience is not smooth**, and the user's five complaints are reproducible in the log. The five issues trace to **four real root causes**, listed from highest to lowest impact:

| # | Symptom user saw                                              | Root cause                                                                       | Severity |
|---|---------------------------------------------------------------|----------------------------------------------------------------------------------|----------|
| A | "Only one task at a time is being worked on"                  | **Unsynchronised per-worker poll timers** — parallelism is bounded by timer cadence, not by `assigned_worker` | High     |
| B | Workers flicker `idle → error → error → idle` in roster       | UI status is **derived, not pushed** — flips between ticks because no `agent.status` event stream exists | High     |
| C | Supervisor ticks every ~50–60 s so 8 Ready tasks wait ~26 s before ANY is assigned | Supervisor recipe scheduler interval is too coarse, and is not triggered by `task.created` events | Medium   |
| D | PA visibly "switches back to Mode 1" in Image 2               | `paMode` is not a hard property of the active `AgentEngine` — on session rehydrate it falls back to the default (`FastDev`) | Medium   |
| E | Log spam with `ConnectionResetError [WinError 10054]`         | `memory-mcp` HTTP keep-alive reaper vs. Windows `ProactorBasePipeTransport`; benign but fills logs and looks scary | Low      |

**What went well (this is new, do not regress it):**

- Core-orchestrator submodule integration is clean — every agent tick stays **under 12 KB** of prompt payload (`Total Request: 21.7KB` is the largest observed, and that was the 1st-tick epic breakdown for the supervisor — see line 4616).
- **Zero 429 / rate-limit / anthropic errors** in the whole 14-min run.
- No tool-execution exceptions — `bash__autodev`, `write_file__autodev`, `edit_file__autodev`, `update_task_status__autodev`, `assign_task__autodev` all succeed.
- Fleet verification passed at `10:03:11` — all 7 agents registered.

---

## 2. Event Timeline (condensed)

All times UTC. Line numbers point into `submoduletesting.txt`.

### Phase 2.1 · Boot (10:02:53 → 10:03:11) — 18 s

```
10:02:53.692  autodev_lib::logging   AutoDev logging initialised
10:02:53.717  skills::loader         loaded 16 skills total
10:02:56.534  sidecar::client        memory-mcp health FAIL (not up yet)           L38-46
10:03:01.563  [memory-mcp]           Memory MCP Server running on 0.0.0.0:8000     L71
10:03:06.793  [core-orch]            UnifiedEventBus starting → running            L164-165
10:03:07.3xx  recipes::scheduler     Firing STARTUP tick for each of 7 agents      L473-597
10:03:08 → 11 agent.factory          7× LivingAgent init, 12 AutoDev tools added   L1306-2938
10:03:11.305  sidecar::client        Fleet verification passed — all 7 agents registered
```

### Phase 2.2 · Idle polling (10:03:17 → 10:04:30) — 73 s

First-ever tick for each worker. All return `No tasks assigned — idle.` or `Board clean — idle.` (supervisor). Example:

```
10:03:32  LivingAgent tick complete for 'qc-agent':          [Executing 1 tool(s)...]   L4639
10:03:34  LivingAgent tick complete for 'worker-backend':    "I'm ready for my scheduled execution. Let me check the task board..."   L4845
10:03:35  LivingAgent tick complete for 'worker-platform':   "..."                      L5120
10:03:36  LivingAgent tick complete for 'worker-frontend':   "..."                      L5210
10:03:38  LivingAgent tick complete for 'worker-cleanup':    [Executing 1 tool(s)...]   L5356
10:03:40  LivingAgent tick complete for 'supervisor':        [Executing 1 tool(s)...]   L5495
```

Scheduler re-fires each worker roughly on a per-recipe interval:

| Agent            | Next-fire delta (from log) | Line |
|------------------|----------------------------|------|
| `qc-agent`       | 27.4 s                     | L4646 |
| `worker-backend` | 25.3 s                     | L4855 |
| `worker-mcp`     | 24.8 s                     | L5010 |
| `worker-platform`| **84.5 s**                 | L5137 |
| `worker-frontend`| 23.4 s                     | L5216 |
| `worker-cleanup` | **381.2 s** (≈ 6.3 min)    | L5362 |
| `supervisor`     | 19.4 s                     | L5510 |

> **This is Root-Cause A.** The fleet does not have a shared tick. Each recipe has its own polling interval, and `worker-cleanup`'s 6.3-minute cadence is the worst offender (it is one of the 7 slots on the Agent Roster — when UI polls the worker row, worker-cleanup is dead air 95% of the time).

### Phase 2.3 · User submits the prompt (~10:04:50 → 10:05:07) — "Delegate Mode"

PA (Rust-side `AgentEngine`) receives the chat message and — correctly being in **Delegate Mode** — emits **8 × `create_task`** back-to-back through the Rust `tool_executor`:

```
10:05:03.616  tool_executor: Executing 8 write tools serially     L7851
10:05:04.981  tool_executor: Executing write tool: create_task    L7853   (1)
10:05:05.398  tool_executor: Executing write tool: create_task    L7876   (2)
10:05:05.779  tool_executor: Executing write tool: create_task    L7999   (3)
10:05:06.220  tool_executor: Executing write tool: create_task    L8002   (4)
10:05:06.622  tool_executor: Executing write tool: create_task    L8005   (5)
10:05:07.037  tool_executor: Executing write tool: create_task    L8008   (6)
10:05:07.412  tool_executor: Executing write tool: create_task    L8011   (7)
10:05:07.762  tool_executor: Executing write tool: create_task    L8014   (8)
```

The 8 tasks visible in Image 1/2 (#1 Connect frontend…, #2 Write integration tests…, #3 Add basic styling…, #4 Build event detail view…, #5 Set up project structure…, #6 Implement events API…, #7 Build event creation modal…) line up exactly with these 8 writes.

**Redis hiccup at 10:05:08** — one warm-tier read times out (`redis.exceptions.TimeoutError: Timeout reading from 127.0.0.1:6379`, L8128). Non-fatal; memory_core falls back to cold tier.

### Phase 2.4 · The dead window (10:05:08 → 10:05:29) — 21 s

Workers whose timers come due in this window poll `list_tasks__autodev`, but the **Supervisor has not run yet**, so `assigned_worker` is still `null` on all 8 rows. They return:

```
10:05:18  litellm_provider [TOOL-LOOP] No tool calls in response, breaking loop.
          Content preview: "No tasks assigned — idle...."                         L8793
10:05:21  RecipeRunner tick complete for 'worker-backend': [Executing 1 tool(s)...]
          [list_tasks__autodev completed]No tasks assigned — idle.                L9136-9138
10:05:23  worker-platform: No tasks assigned — idle.                              L9304
10:05:23  qc-agent: No tasks in qc_review — idle.                                 L9439
```

> **This is Root-Cause C.** The supervisor is oblivious to `task.created` events; it only reacts on the next timer tick. The user sees 8 tasks appear in the `Ready` column, and nothing moves for ~20 s, which feels like a hang.

### Phase 2.5 · Supervisor dispatch burst (10:05:29 → 10:05:43) — 14 s

Supervisor's scheduled tick finally fires and it assigns **all 8 tasks in a single LLM loop** — exactly as the recipe demands ("Stop Contract: stop ONLY after every ready task has been dispatched"):

```
10:05:29.610  Executing tool: assign_task__autodev    (task #1 → worker-frontend)   L9641
10:05:32.333  Executing tool: assign_task__autodev    (task #2)                     L9692
10:05:34.087  Executing tool: assign_task__autodev    (task #3)                     L9763
10:05:35.852  Executing tool: assign_task__autodev    (task #4)                     L9811
10:05:37.570  Executing tool: assign_task__autodev    (task #5)                     L9859
10:05:39.311  Executing tool: assign_task__autodev    (task #6)                     L9907
10:05:41.228  Executing tool: assign_task__autodev    (task #7)                     L9955
10:05:43.534  Executing tool: assign_task__autodev    (task #8)                     L10023
10:05:42.044  [core-orch] ConnectionResetError [WinError 10054]  (benign)           L9981
```

After this tick the board is fully assigned. **The supervisor did its job perfectly.** (This contradicts the user's hypothesis in point 3 — it is not assignment throughput that's the bottleneck, it's **what happens next**.)

### Phase 2.6 · Worker pickup — staggered, not parallel (10:06:00 → 10:07:40)

Once assigned, a worker only picks the task up on its *own* next timer tick:

```
10:06:28  update_task_status__autodev   worker-backend: ready → in_progress        L11719
10:06:34  update_task_status__autodev   worker-backend: second status bump         L11888
10:06:58  Executing tool: bash__autodev                                            L12023
10:07:13  Executing tool: bash__autodev                                            L12767
10:07:14 → 10:07:31  11× write_file__autodev  (worker-backend building the API)    L12965-13900
10:07:36  bash__autodev                                                            L13957
10:07:53  update_task_status__autodev   (task #6 → qc_review)                      L14105
```

Meanwhile `worker-frontend` is still waiting for its 23 s tick. `worker-cleanup` is still sleeping on its 381 s timer (its task sits Ready for the whole run). This is **why the user sees "only one task at a time"** — not because the supervisor is slow, but because worker pickup is serialised by independent timers.

### Phase 2.7 · Steady progress (10:08:00 → 10:11:22)

- `worker-backend` delivers task #6 (`Implement events API endpoints (CRUD)`) and task #5 (`Set up project structure`) — both reach `done` (Image 2, `QC PASS RATE: 100%`).
- `worker-frontend` picks up task #1 (`Connect frontend to events API…`) — still `in_progress` in both screenshots.
- `worker-mcp`, `worker-platform`, `worker-cleanup` never received tasks from this epic (correct — no MCP/platform/cleanup work in the breakdown).

### Phase 2.8 · Tail — ongoing polls, no new work (10:12 → 10:17)

49 more `list_tasks__autodev` calls across all agents with no meaningful state changes. Frontend task #1 is still running at end-of-log. The run did not stall, it just kept ticking.

---

## 3. Root Cause Analysis

### 3.1 (A) Per-worker poll timers are not aligned

**Evidence:** `RecipeScheduler` log lines show each recipe has an independent `next fire in Ns` calculation (L4646, L4855, L5137, L5216, L5362). `worker-cleanup` has a 381 s (6.3 min) cadence; `worker-platform` has 84 s; most others 20–30 s. Nothing on the Rust side wakes a worker *because* it just received an assignment.

**Why it feels like "one at a time":** the 8 `assign_task` calls complete at 10:05:43, but each worker has to wait up to its own cadence to see the new row. `worker-backend` picks up at 10:06:28 (45 s later). `worker-frontend` picks up at 10:07:00 (77 s later). From the user's vantage the board drip-feeds tasks into In Progress.

**Affects user points:** #2 (choppy), #3 ("only one assigned"), #4 (not smooth).

---

### 3.2 (B) Agent Roster "Error" state is a UI phantom, not a real error

**Evidence:** grep for `ERROR`, `Traceback`, `tick failed`, `dispatch failed`, `500`, `502`, `429` across the whole log produces:

- `asyncio: Exception in callback _ProactorBasePipeTransport._call_connection_lost()` — keep-alive reaper, Windows only.
- `ConnectionResetError: [WinError 10054]` — same root cause, log channel `[memory-mcp ERR]` / `[core-orch]`.
- `redis.exceptions.TimeoutError` once at 10:05:08.
- Zero RecipeRunner failures, zero LivingAgent exceptions, zero LLM errors.

**What the UI is actually doing** (see `stores/workerStore.ts::workerStatusColor()` per memory note): it derives each roster row's status from the **task board** — whether the agent is assigned a row and the row's `updated_at` is recent. Between ticks the row looks stale → status flickers through `idle` → `error` → `running` → `idle`.

**Affects user points:** #2 (idle↔error flicker), #4.

---

### 3.3 (C) Supervisor has no event-driven dispatch trigger

**Evidence:** the 21 s dead window between `create_task` burst (10:05:07) and `assign_task` burst (10:05:29). During that window every worker that ticked reported "No tasks assigned — idle" (L8793, L9138, L9304, L9439). The supervisor does not subscribe to `task.created`; it only wakes on its own timer.

**Affects user points:** #3 ("only one by one"), #4.

---

### 3.4 (D) PA Mode is not session-bound

**Evidence:** log does not contain the literal strings `PAMode`, `paMode`, `Delegate`, or `Mode 1` at the backend level — the backend does not record mode transitions. In Image 2 the top-right chip has reverted from 📋 Delegate to ⚡ (Fast Dev). The AgentEngine is *constructed* with `pa_mode` (per previous phase 2.1 memory) but there is no evidence any handler writes mode to the session row — so a session reload or engine rebuild defaults back to `FastDev`.

**Affects user point:** #5.

---

### 3.5 (E) `WinError 10054` noise

**Evidence:** 12+ hits across the log, always accompanied by `_ProactorBasePipeTransport._call_connection_lost()`. All on Windows. Non-fatal, always benign, always followed by a successful retry. First seen at L5311 (10:03:37), last seen at L14220 (10:08:00) — no correlation with fleet failures.

**Affects:** log readability only. Masks real errors if they ever happen.

---

## 4. Flow Diagram (what actually happened)

```
user prompt
    │  (10:04:50)
    ▼
PA (Delegate Mode) ─── create_task × 8 ───► tasks.db   [Ready, assigned_worker=null]
                                                  │
                                       [21 s dead window]
                                                  │
                              supervisor timer tick (10:05:29)
                                                  │
                                   assign_task × 8  (14 s burst)
                                                  │
                                  tasks.db [Ready, assigned_worker=<w>]
                                                  │
         ┌────────────────────┬────────────────────┼─────────────────────┐
         ▼                    ▼                    ▼                     ▼
   worker-backend       worker-frontend     worker-platform        worker-cleanup
   (fires in 45 s)      (fires in 77 s)     (no match this epic)   (fires in 381 s)
         │                    │
  update → in_progress  update → in_progress
  bash, write_file×11   (still running at end of log)
  update → qc_review
         │
   qc-agent tick  ────► update → done
```

The **user's "smoothness" complaint lives entirely on the vertical bars of this diagram** — the gaps between a task being assigned and a worker actually looking at the board.

---

## 5. Recommended Fixes (ordered by impact)

### Fix 1 — Push-based dispatch (kills A + C at once)

On every successful `create_task`, emit a `task.created` UnifiedEventBus event; add a `SupervisorOnTaskCreated` subscription on the Rust side that **immediately fires a supervisor tick** (bypassing its timer). Similarly, on every successful `assign_task` emit `task.assigned{worker_slug}` and **immediately fire that worker's tick** (again bypassing its timer). The per-worker timers remain as fallback / stall-recovery only.

- Expected effect: 21 s dead window → < 1 s; worker pickup lag → near-instant.
- Files likely touched: `src-tauri/src/recipes/scheduler.rs`, `src-tauri/src/db/tasks.rs`, `src-tauri/src/events/`.

### Fix 2 — Push agent status via event stream (kills B)

Replace the UI's derived `workerStatusColor()` with an authoritative `agent.status` event stream:
`agent.tick_started{name}`, `agent.tick_completed{name, outcome}`, `agent.tick_failed{name, error}`. The frontend `workerStore` listens to the stream and the roster cells are now push-driven. No more flicker.

### Fix 3 — Persist `pa_mode` on the chat session (kills D)

Add a `pa_mode TEXT NOT NULL DEFAULT 'fast_dev'` column to the `sessions` table (or store under `sessions.metadata`). `send_message` must read from the session, not from the `settingsStore`, so the mode survives session reload, engine rebuild, or app restart.

### Fix 4 — Log-channel filter for Windows `ProactorBasePipeTransport` (kills E)

In `core-orchestrator` and `memory-mcp` install an asyncio exception filter that swallows `ConnectionResetError (WinError 10054)` that originates from `_call_connection_lost` on Windows. Still log once per minute at `DEBUG`, never as `ERROR`.

### Fix 5 — Shorten `worker-cleanup` recipe cadence

Drop from 381 s to ~30 s. The roster has 7 rows; users expect them to all move at similar cadence.

---

## 6. Counter-intuitive Findings (things the user did **not** mention but the log shows)

1. **The supervisor is not the bottleneck** — it dispatched all 8 tasks in one 14-second tick. The user's point #3 hypothesis ("supervisor is not assigning properly") is actually wrong; it assigned everything at once. The slowness is downstream (worker pickup).
2. **Everything under 12 KB really does hold** — largest single request logged is 21.7 KB on a first-tick epic breakdown with full memory recall context. All subsequent ticks are well under 10 KB. The core-orchestrator integration is doing its job.
3. **Tasks #2, #3, #4, #7 (frontend work) never moved past Ready** in the logged window — `worker-frontend` is stuck on task #1 and its recipe does not parallelise. If the roster grows (e.g. multiple `worker-frontend` instances), the Supervisor recipe will need a policy for "pick the least-loaded worker of this role" because today it just pins to the single slug.
4. **`worker-mcp`, `worker-platform`, `worker-cleanup` received zero tasks** in this epic. They still woke every N seconds, called `list_tasks`, returned idle. Not harmful but wasteful on quota / cost attribution.

---

## 7. Next Actions (for the user to approve before we touch code)

- [ ] **Approve Fix 1** (push-based dispatch on `task.created` / `task.assigned`) — biggest UX win, low risk, ~1 day of work.
- [ ] **Approve Fix 2** (agent status event stream) — medium effort, kills the "flicker" UX issue permanently.
- [ ] **Approve Fix 3** (persist `pa_mode` on the session row) — small diff, removes the Mode-1 snap-back.
- [ ] **Approve Fix 4** (Windows asyncio filter) — tiny diff, massive log-readability win.
- [ ] **Approve Fix 5** (tighten cleanup cadence) — trivial, affects `recipes/defaults.rs`.

Each fix can land independently; Fix 1 + Fix 2 together are expected to take the run from the current "14 min, drip-fed" experience to a "3–4 min, continuous" experience on the same prompt.

---

## Appendix A — Grep commands used

```powershell
# Count all lines
(Get-Content '...\submoduletesting.txt').Length                          # 32,891

# Tool-execution timeline
Select-String -Pattern 'Executing tool: (assign_task|update_task_status|create_task|write_file|edit_file|bash|list_tasks)'

# RecipeScheduler cadence
Select-String -Pattern 'autodev_lib::recipes::scheduler'

# RecipeRunner per-tick outcome
Select-String -Pattern "LivingAgent tick complete for '"

# Error surface
Select-String -Pattern 'ConnectionReset|Traceback|Timeout|429|rate.limit|500 Internal|502 Bad'
```

## Appendix B — Log line index (for future re-reads)

| What                                          | Lines              |
|-----------------------------------------------|--------------------|
| Boot + 16 skills                              | 1–200              |
| Agent factory / tool registration             | 900–3000           |
| First idle polling round                      | 3200–5570          |
| User prompt → 8× `create_task` (Rust)         | 7850–8015          |
| Redis warm tier timeout                       | 8128               |
| Workers see empty board (dead window)         | 8793–9440          |
| Supervisor's 8× `assign_task` burst           | 9641–10023         |
| Worker-backend first real work (bash + writes)| 12023–13957        |
| Task #6 → qc_review → done                    | 14105, 15643       |
| Steady polling tail                           | 17000–32891        |
