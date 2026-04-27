# Fleet Flow Analysis — completerun.log (2026-04-24)

**Log file:** `autodev/logs/completerun.log`  
**Size:** 38,924 lines (~15 min capture window: 03:47:37 → 04:02:02 UTC)  
**Run mode:** Mode 2 (LivingAgent / core-orchestrator)  
**Sidecars:** memory-mcp-server + core-orchestrator both running

---

## 1. Executive Summary

The fleet booted successfully into Mode 2. All 7 workers (supervisor, qc-agent, worker-backend, worker-frontend, worker-platform, worker-mcp, worker-cleanup) are alive and ticking on schedule. However, **three critical pathologies** are blocking task flow completion:

| # | Issue | Impact | Severity |
|---|-------|--------|----------|
| 1 | **QC Agent Blindness** — qc-agent's `list_tasks` query returns empty for `status='qc_review'` despite 4+ tasks visibly in QC REVIEW column | Tasks stuck in QC REVIEW forever; never reach `done` | 🔴 Critical |
| 2 | **Build Failed Tasks** — 2 frontend tasks failed validation and sit in DONE TODAY with `build_failed` label | Worker output is incomplete/broken | 🔴 Critical |
| 3 | **Memory Bloat / Recipe Pollution** — Full recipe instructions stored as episodic memories on every tick, ballooning context windows | Slow recalls (2.8–4.5s), high token usage, degraded agent performance | 🟡 High |

---

## 2. Fleet Health at a Glance

### 2.1 Worker Tick Summary (sampled across log)

| Worker | Recipe | Schedule | Tick Outcome | Notes |
|--------|--------|----------|--------------|-------|
| supervisor | supervisor-poll | `*/1 * * * *` | ✅ Assigns tasks | Consistently fires every 60s |
| qc-agent | qc-agent-review | `*/1 * * * *` | ⚠️ **"No tasks in qc_review — idle."** | **Blind to actual QC REVIEW tasks** |
| worker-backend | worker-backend-poll | `*/1 * * * *` | ✅ Executes tasks | Active file writes, bash calls |
| worker-frontend | worker-frontend-poll | `*/1 * * * *` | ✅ Executes tasks | Active edit_file, validation |
| worker-platform | worker-platform-poll | `*/5 * * * *` | ✅ Idle (no tasks) | Expected — no platform tasks |
| worker-mcp | worker-mcp-poll | `*/1 * * * *` | ✅ Idle (no tasks) | Expected — no MCP tasks |
| worker-cleanup | worker-cleanup-poll | `*/2 * * * *` | ✅ Idle (no tasks) | Expected — no cleanup tasks |

### 2.2 Startup Sequence (FSM Transitions)

All 13 startup phases completed successfully:

```
init(0/13) → skills(1/13) → memory_starting(2/13) → vector_db(3/13)
→ memory_ready(4/13) → orch_starting(5/13) → routers(6/13)
→ agents(7/13) → living_agents(8/13) → orch_ready(9/13)
→ workers(10/13) → ready(11/13)
```

**Key milestones:**
- `memory-mcp-server` spawned at 03:47:42, health check on :8000
- `core-orchestrator` spawned at 03:47:51, health check on :4000
- All 5 system agents registered by 03:47:58 (supervisor, qc-agent, worker-backend, worker-frontend, worker-mcp)
- MCP Bridge initialized for all agents with 11 tools each

### 2.3 Sidecar Connectivity

| Sidecar | Port | Health | Notes |
|---------|------|--------|-------|
| memory-mcp-server | :8000 | ✅ Healthy | Qdrant vector store active |
| core-orchestrator | :4000 | ✅ Healthy | All agents + MCP bridges online |
| Redis/Memurai | :6379 | ✅ Reachable | Continuity packets stored |

---

## 3. Critical Issue #1: QC Agent Blindness

### 3.1 Symptom

The qc-agent **executes on schedule every minute** but consistently reports:

```
[RecipeRunner] LivingAgent tick complete for 'qc-agent':
  No tasks in qc_review — idle.No tasks in qc_review — idle.
```

This pattern repeats **every single qc-agent tick** throughout the 15-minute log window.

### 3.2 Evidence from Screenshot

The UI Task Board at capture time shows:

| Column | Count | State |
|--------|-------|-------|
| READY | 0 | — |
| IN PROGRESS | 1 | Task: "Add basic styling to match Google Calendar aesthetic" (worker-frontend) |
| **QC REVIEW** | **4** | Tasks stuck: event deletion, event form backend, event creation modal, project structure |
| DONE TODAY | 3 | 2 with `build_failed` label, 1 regular |

### 3.3 Verified Root Cause Analysis

After reading `tasks.rs`, `list_tasks_tool.rs`, `update_task_status_tool.rs`, `runner.rs`, `scheduler.rs`, and `defaults.rs`, all three original hypotheses are **ruled out**. The actual root causes are architectural, not data-layer.

#### Hypothesis Verdicts

| Hypothesis | Verdict | Evidence |
|---|---|---|
| **A — Status Mismatch** | ❌ **FALSE** | `TaskStatus::from_str("qc_review")` → `QcReview` (`tasks.rs:82`); `as_str()` → `"qc_review"` (`tasks.rs:73`); `db.list(Some("qc_review"))` does `WHERE status = ?1` with exact param binding (`tasks.rs:199`). No case drift, no whitespace trimming issues. |
| **B — Database Scope** | ❌ **FALSE** | Both UI (`task_commands.rs`) and tools (`list_tasks_tool.rs:43`) call `TasksDb::open()` → `~/.autodev/tasks.db`. There is no secondary task database in core-orchestrator. |
| **C — Hidden Filters** | ❌ **FALSE** | `list_tasks_tool.rs` has no `limit`, no `assigned_worker` filter, no default exclusions. The `input_schema` enum includes `"qc_review"` and the `execute` body passes `status_filter` directly to `db.list()` with zero transformation. |

#### Actual Root Cause #1 — Memory Bias (Primary)

The qc-agent runs inside **core-orchestrator Mode 2** (`runner.rs:157`). On every tick, the full recipe instructions are prepended as a user message (`runner.rs:168-174`). The core-orchestrator's mem0 memory system stores this as an **episodic memory** every tick. After 20+ ticks, the agent's "recent experiences" are dominated by identical copies of the recipe saying "No tasks in qc_review — idle."

**Log evidence** (`logs/completerun.log`, Section 11.1):
```
[core-orch] [TOOL-LOOP] Iteration 1/25
[core-orch] [TOOL-LOOP] No tool calls in response, breaking loop.
```

The LLM **never emits a `list_tasks` tool call**. It skips step 1 of its own recipe because its episodic memory confidently predicts "no tasks." The `list_tasks` tool is never invoked — not because it would return empty, but because the agent never calls it.

#### Actual Root Cause #2 — Kick Race (Secondary)

`update_task_status_tool.rs:111` kicks `qc-agent` immediately when a task hits `qc_review`. But `scheduler.rs:222` gates kicks:
```rust
if w.status == "working" {
    info!("[RecipeScheduler] kick_worker('{}'): skipped (already working)", worker_slug);
    return;
}
```

If a scheduled qc-agent tick is already in-flight (status="working") when the kick arrives, the kick is **silently dropped**. The in-flight tick started before the worker finished, so it finds no tasks. The next scheduled tick is up to 60 seconds away.

**Impact**: Even when `list_tasks` would return results, the qc-agent can miss them for up to one full cron cycle due to this race.

#### Actual Root Cause #3 — Unconditional API Burn (Tertiary)

`runner.rs:100` applies `has_runnable_task()` only to slugs starting with `"worker-"`. The `qc-agent` slug does not match, so it **always** proceeds to `ensure_agent()` + `invoke_worker()` regardless of board state. This costs ~1 LLM API call per minute ($0.01–0.03) even when the board has been empty for hours.

### 3.4 Impact

- Tasks in QC REVIEW **never get reviewed**
- They **never transition to `done`**
- The board accumulates orphaned tasks (4 confirmed in the 15-minute window)
- Workers may re-poll and pick up `assigned` tasks, but QC backlog grows
- Each blind tick burns API tokens and embedding calls for zero progress

### 3.5 Recommended Fix

1. **Recipe imperative rewrite** (`defaults.rs`): Make `list_tasks` an absolute, unconditional first step — remove any early "if no tasks" branch that the LLM can shortcut via memory inference.
2. **Pre-flight DB gate** (`runner.rs`): Before burning an API call, check `TasksDb::list(Some("qc_review"))`. If empty, skip the invoke entirely. If non-empty, prepend task titles to the trigger message so the LLM sees them even if memory claims otherwise.
3. **Kick race mitigation** (`scheduler.rs` or `runner.rs`): The DB gate in #2 also closes the race window — when tasks exist, the gate lets the tick through with task data in-context, so even a delayed scheduled tick sees them.
4. **Observability** (`task_commands.rs`): Add `get_qc_queue()` Tauri command so the UI can surface QC backlog age and alert when tasks sit >5 min.

---

## 4. Critical Issue #2: Build Failed Tasks (Ecosystem-Wide)

### 4.1 Symptom

From the screenshot, **2 tasks in DONE TODAY** show the `build_failed` label:

1. **Task A**: "Fetch and display events on calendar" (worker-frontend)
2. **Task B**: "Build calendar grid UI component (React)" (worker-frontend)

Both are `worker-frontend` tasks hitting a Node.js workspace at `C:\AIRS Foundry\autodev\testing\frontend\`.

### 4.2 Ecosystem Coverage Analysis

The `Ecosystem` enum in `src-tauri/src/hooks/ecosystem.rs` already supports 5 ecosystems:

| Ecosystem | Detected By | Install Command | Validate Command | Sentinel Path |
|---|---|---|---|---|
| **Node** | `package.json` | `npm install` | `npm run build` | `node_modules/.package-lock.json` ❌ |
| **Python (pip)** | `requirements.txt` or `pyproject.toml` (no lock) | `pip install -r requirements.txt` | `python -m pytest` | `.venv/pyvenv.cfg` ⚠️ |
| **Python (poetry)** | `pyproject.toml` + `poetry.lock` | `poetry install` | `poetry run pytest` | `.venv/pyvenv.cfg` ⚠️ |
| **Rust** | `Cargo.toml` | `cargo build` | `cargo test` | `target/.rustc_info.json` ✅ |
| **Go** | `go.mod` | `go mod tidy` | `go test ./...` | `go.sum` ✅ |

**Critical Gap**: `uv` (the modern Python package manager / virtual-environment tool) is **not represented**. Projects using `uv` (`pyproject.toml` + `uv.lock`) are detected as `PythonPip`, but `pip install -r requirements.txt` will fail or install into the wrong environment.

### 4.3 Root Cause — Node Sentinel Path (Primary)

`ecosystem.rs:71` defines the Node sentinel as:
```rust
Ecosystem::Node => PathBuf::from("node_modules/.package-lock.json"),
```

**Problem**: npm v7+ generates `package-lock.json` at the **project root**, not inside `node_modules/`. The hidden file `node_modules/.package-lock.json` is **not guaranteed** on all npm versions or package managers (pnpm, yarn). Even after a successful `npm install`, the sentinel may be missing.

**Impact on `guard_and_validate`** (`runner.rs:448-477`): `guard_and_validate` calls `Ecosystem::detect()` → `Node`, then checks `working_dir.join(sentinel).exists()`. If the sentinel is absent (even after install), the task is incorrectly marked `build_failed`.

### 4.4 Root Cause — Python Sentinel Assumes venv (Secondary)

The Python sentinels (`.venv/pyvenv.cfg`) assume a virtual environment was created. But:
- `pip install -r requirements.txt` may install into the **global** Python environment
- `uv pip install` installs into a `.venv` by default, but standard `pip` does not
- A Python task may run in a container or system Python where `.venv` never exists

**Impact**: `guard_and_validate` will falsely flag Python tasks as `build_failed` even when dependencies are installed globally.

### 4.5 Root Cause — Recipe Build Validation Is Optional (Tertiary)

All worker recipes (`defaults.rs`) include a "Build Validation (MANDATORY)" section, but it appears **after** the "Stop Contract" and "Focus Protocol" sections. The LLM reads the recipe top-to-bottom and can decide it has satisfied the Stop Contract before ever reaching Build Validation.

**Evidence from `worker_frontend_recipe`** (`defaults.rs:233-237`):
```
## Stop Contract (MANDATORY)
Once you have either (a) marked a task `qc_review`/`failed`, or (b) confirmed no runnable tasks...

## Build Validation (MANDATORY)
After all file writes are complete, run the ecosystem-specific install and validate commands...
```

An LLM that marks `qc_review` in step 4e may treat the Stop Contract as satisfied and **skip** the Build Validation section entirely. The recipe structure itself enables skipping.

### 4.6 Root Cause — `update_task_status` Tool Schema Rejects `build_failed` (Quaternary)

`update_task_status_tool.rs:35` defines the allowed enum as:
```json
"enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "cancelled"]
```

The `TaskStatus` enum in `tasks.rs` and the UI store both include `build_failed`, but the tool schema **excludes it**. A worker that detects a build failure cannot use the tool to report `build_failed`; it must either:
- Use `failed` (losing the build-specific distinction)
- Skip the tool and rely on `guard_and_validate` to set the status server-side

### 4.7 Impact

- Worker output files exist but are **unvalidated** (recipe structure lets LLM skip validation)
- `guard_and_validate` **falsely marks** Node tasks `build_failed` due to wrong sentinel
- Python tasks may be falsely flagged if no `.venv` exists
- `build_failed` tasks bypass QC review (they appear in DONE TODAY, not QC REVIEW)
- The supervisor may consider them "done" and not reassign
- Workers cannot self-report `build_failed` because the tool schema rejects it

### 4.8 Recommended Fix (Ecosystem-Wide)

1. **Fix Node sentinel** (`ecosystem.rs`): Change to `node_modules/` directory check (npm always creates it)
2. **Add `uv` ecosystem** (`ecosystem.rs`): Detect `uv.lock` or `[tool.uv]` in `pyproject.toml`; install `uv sync`; validate `uv run pytest`
3. **Fix Python sentinel** (`ecosystem.rs`): Add fallback — if `.venv/pyvenv.cfg` missing, check if `import <top_package>` succeeds via a quick `python -c` probe
4. **Harden ALL worker recipes** (`defaults.rs`): Move Build Validation **before** the Stop Contract; use numbered mandatory steps; include ecosystem-specific commands explicitly
5. **Fix `build_failed` enum** (`update_task_status_tool.rs`): Add `"build_failed"` to the JSON schema so workers can self-report
6. **Add pre-flight install** to each worker recipe: Before file writes, check sentinel; if missing, run install command; only then proceed to validation

---

## 5. High-Priority Issue #3: Memory Bloat / Recipe Pollution

### 5.1 Symptom

Every worker tick stores the **entire recipe instructions** (1,500–3,300 bytes each) as an episodic memory. Over 15 minutes, this accumulates rapidly.

**Evidence from log:**

```
[memory-mcp] ... annotated_first_msg_length=3348 memory_types=['memory:episodic']
[memory-mcp] ... annotated_first_msg_length=3314 memory_types=['memory:episodic']
[memory-mcp] ... annotated_first_msg_length=2537 memory_types=['memory:procedural']
```

The mem0 adapter stores these as **episodic** memories (transient, per-session) but they still:
- Get embedded (OpenAI API call per memory)
- Get inserted into Qdrant vector store
- Get retrieved on subsequent recalls, bloating context

### 5.2 Impact

- **Slow recalls**: 2,841ms – 4,610ms per recall (threshold: 1,000ms)
- **High token usage**: Request sizes grow from 15.6KB → 23.2KB messages + 8.7KB tools
- **Memory scoring churn**: 18–26 memories scored per recall, 10–14 selected
- **Cost**: Each OpenAI embedding API call costs tokens

### 5.3 Root Cause

The **AUTODEV RECIPE INSTRUCTIONS** are sent as a `user` message on every tick. The memory system treats this as a new episodic experience and stores it. Over time, the agent's "recent experiences" list is dominated by identical recipe copies.

### 5.4 Note — Out of Scope for Main Repo

These fixes require changes to the **core-orchestrator** and **memory-mcp-server** submodules, which are being refined in a separate workstream. The analysis above is kept as knowledge for the submodule refinement team.

---

## 6. Task Lifecycle Analysis

### 6.1 Core-Orchestrator Internal FSM vs SQLite Board

The core-orchestrator maintains **two parallel task tracking systems**:

| System | Purpose | States |
|--------|---------|--------|
| **SQLite Task Board** | Source of truth for UI, worker assignment | `ready → assigned → in_progress → qc_review → done / failed` |
| **Core-Orch TaskExecutionFSM** | Tracks agent execution loop for a single tick | `created → planning → executing → reviewing → completed` |

**Observed FSM transitions in log:**

```
task_01KPYSM6JJ8A5AAWZNP2P7KW2Y: executing → reviewing → completed
task_01KPYSM6K97PZ3QQ7N4QENC79W: executing → reviewing
```

The core-orch FSM `completed` state means the **agent tick finished**, not that the SQLite task is `done`. This is a naming collision that causes confusion.

### 6.2 Task Assignment Flow (Successful Path)

```
[03:52:00] supervisor fires → list_tasks → finds 'ready' tasks
[03:52:01] supervisor → update_task_status(id, 'assigned', 'worker-frontend')
[03:52:15] worker-frontend fires → list_tasks → finds 'assigned' task
[03:52:16] worker-frontend → update_task_status(id, 'in_progress', 'worker-frontend')
[03:52:30] worker-frontend → edit_file / write_file / bash
[03:52:45] worker-frontend → update_task_status(id, 'qc_review', 'worker-frontend')
[03:52:46] qc-agent fires → list_tasks with status='qc_review' → RETURNS EMPTY ❌
[03:53:00] qc-agent → "No tasks in qc_review — idle."
```

**Breakdown at step 6** — the qc-agent cannot see the task that worker-frontend just moved to `qc_review`.

### 6.3 Stuck Task Analysis

From the screenshot, the 4 stuck QC REVIEW tasks are:

| # | Task | Worker | Likely Root Cause |
|---|------|--------|-------------------|
| 1 | Add event deletion functionality | worker-frontend | Worker set to `qc_review`, QC agent blind |
| 2 | Connect event form to backend API | worker-frontend | Worker set to `qc_review`, QC agent blind |
| 3 | Build event creation modal | worker-frontend | Worker set to `qc_review`, QC agent blind |
| 4 | Set up project structure (React + Express + SQLite) | worker-backend | Worker set to `qc_review`, QC agent blind |

All 4 were successfully executed by workers but **orphaned at QC stage**.

---

## 7. Worker Execution Details

### 7.1 worker-frontend

**Active tasks executed:**
- File edits in `testing/frontend/src/App.js`
- Validation attempted via `npx tsc --noEmit` (implied by recipe)
- Status updated to `qc_review`

**Tool calls observed:**
- `edit_file_autodev-mcp-server` on `App.js`
- `bash_autodev-mcp-server` (validation)
- `list_tasks_autodev-mcp-server` (discovery)
- `update_task_status_autodev-mcp-server` (state transition)

### 7.2 worker-backend

**Active tasks executed:**
- Project structure setup
- Backend API implementations

**Tool calls observed:**
- `write_file_autodev-mcp-server`
- `bash_autodev-mcp-server`
- `list_tasks_autodev-mcp-server`

### 7.3 worker-mcp, worker-cleanup, worker-platform

All three consistently report:
```
No tasks assigned — idle.
```

This is **expected behavior** — no tasks were created with `assigned_worker='worker-mcp'` etc. The supervisor correctly assigns only to worker-frontend and worker-backend for this workload.

---

## 8. Memory System Deep Dive

### 8.1 Memory Types Stored

| Type | Count (approx) | Purpose | Issue |
|------|---------------|---------|-------|
| **episodic** | ~40+ per agent per 15min | Per-tick experiences | Dominated by recipe copies |
| **procedural** | ~5–10 per agent | Tool success/failure patterns | Useful, low volume |
| **semantic** | ~5 per agent | Facts about workspace | Useful, low volume |

### 8.2 Recall Performance

```
[MEMORY:RECALL] Completed in 2863.4ms: 6 experiences, 11 facts, 4 learnings
[MEMORY:RECALL] Completed in 4610.5ms: 15 experiences, 11 facts, 6 learnings
```

**Threshold**: 1,000ms  
**Status**: ⚠️ **Consistently 2.8×–4.6× over threshold**

### 8.3 Scoring Overhead

```
[MEMORY:SCORING] SCORING_COMPLETED | duration=2789.7ms total=19 critical=0 high=1 medium=8 low=10 avg_score=0.31
[MEMORY:SCORING] SCORING_COMPLETED | duration=4254.6ms total=26 critical=0 high=2 medium=10 low=14 avg_score=0.32
```

Most scored memories are **low relevance** (score ~0.3), meaning the majority of recalled content is noise.

### 8.4 Context Selection

```
[SELECTION-COMPLETE] Selected 14 chunks, rejected 5, tokens=1951, facet_coverage=100.0%
```

Even with 100% facet coverage, the **token count is high** (1,951 tokens just for memory context). This leaves less room for actual task content in the LLM context window.

---

## 9. API Cost Analysis

### 9.1 LLM API Calls

| Model | Calls (est.) | Avg Request Size | Notes |
|-------|-------------|------------------|-------|
| anthropic/claude-haiku-4-5-20251001 | ~100+ | 15–32 KB | All worker ticks |

**Tool loop iterations**: Up to 14 iterations per tick (observed: `Iteration 14/25` for worker-frontend)

### 9.2 Embedding API Calls

```
httpx: HTTP Request: POST https://api.openai.com/v1/embeddings "HTTP/1.1 200 OK"
```

**Frequency**: Every memory store operation triggers 1 embedding call.  
**Estimated**: 50–100 embedding calls per 15-minute window across all agents.

### 9.3 Chat Completions (Memory Reflection)

```
httpx: HTTP Request: POST https://api.openai.com/v1/chat/completions "HTTP/1.1 200 OK"
```

These are Mem0's internal reflection/summarization calls, not directly related to task execution.

---

## 10. Atomic Fix Task List — Main Repo Only (12 Tasks)

All items below target files **inside `autodev/src-tauri/src/` and `autodev/src/`** only. The core-orchestrator submodule is excluded per workstream boundary.

---

### 🔴 P0 — QC Agent Blindness

#### Task 1: `src-tauri/src/recipes/defaults.rs` (qc-agent recipe) — Force `list_tasks` as first tool call

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\recipes\defaults.rs:369-394`

**Current problem:**
- Recipe step 1 says "Call list_tasks to find tasks with status='qc_review'"
- Step 3 says "If no tasks in qc_review: STOP IMMEDIATELY"
- The LLM (running inside core-orchestrator Mode 2) skips step 1 and jumps to step 3 because its episodic memory from the previous 20+ ticks all say "No tasks in qc_review — idle." (Root cause verified: `tasks.rs`, `list_tasks_tool.rs`, and `db.list()` are 100% consistent — the tool is never even invoked.)
- The conditional "If no tasks" branch early in the prompt gives the LLM a shortcut to bypass tool calls entirely.
- Result: `Tool calls: 0` on every qc-agent tick; the `list_tasks` tool is never invoked.

**Fix:** Rewrite the instructions block so the first step is an **absolute imperative** that cannot be bypassed by memory inference:

```rust
instructions: r#"You are the AIRS QC Agent. You review completed worker output.

## ABSOLUTE FIRST STEP — NEVER SKIP
You MUST call `list_tasks` with status='qc_review' as your VERY FIRST action in EVERY tick.
Do NOT rely on memory, previous tick results, or context to decide whether tasks exist.
The task board changes between ticks. ALWAYS call the tool first.

## Second Step — Review
If list_tasks returns tasks:
  a. Read the acceptance criteria carefully
  b. Use grep/glob/read_file to inspect the implementation
  c. Run tests if applicable: bash 'cargo check' / 'cargo test' / 'pytest' / 'npx tsc --noEmit'
  d. If ALL criteria met AND checks pass:
     → update_task_status(id, 'done', assigned_worker)
  e. If any criterion not met OR tests fail:
     → update_task_status(id, 'assigned', assigned_worker) with rejection notes in the task

## Stop Contract
After every task decision (done/assigned-with-notes) or after confirming no qc_review tasks VIA THE TOOL, reply with text-only and emit NO further tool_use blocks."#.into(),
```

**Rationale:** Removing the conditional "If no tasks in qc_review" from the early part of the prompt forces the LLM to always execute a tool call before it can emit the idle text.

---

#### Task 2: `src-tauri/src/recipes/runner.rs` (Mode 2 invoke path) — Pre-flight DB gate for qc-agent

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\recipes\runner.rs:154-200`

**Current problem:**
- `run_recipe` unconditionally calls `orch_client.invoke_worker()` for qc-agent whenever the orchestrator is healthy
- The qc-agent costs ~1 LLM API call per minute even when the board is empty
- Worse: because the qc-agent's memory is polluted with "no tasks" history, it never calls `list_tasks` even when tasks DO exist

**Fix:** Insert a pre-flight SQLite check before the qc-agent invoke:

```rust
// ── qc-agent pre-flight gate ───────────────────────────────────────────
// If no qc_review tasks exist in the local DB, skip the expensive
// core-orchestrator invocation entirely and set status to idle.
// When tasks DO exist, prepend their titles to the trigger so the
// agent has them in-context even if its memory claims otherwise.
let mut qc_trigger_override: Option<String> = None;
if worker_slug == "qc-agent" {
    if let Ok(db) = crate::db::tasks::TasksDb::open() {
        if let Ok(tasks) = db.list(Some("qc_review")) {
            if tasks.is_empty() {
                info!("[RecipeRunner] No qc_review tasks in DB — skipping qc-agent tick");
                self.set_worker_status(worker_slug, "idle");
                return Ok(());
            } else {
                let titles: Vec<String> = tasks.iter()
                    .map(|t| format!("#{}: {}", t.id, t.title))
                    .collect();
                qc_trigger_override = Some(format!(
                    "{RECIPE_TRIGGER}\n\n[BOARD STATE] {} task(s) awaiting QC review:\n{}",
                    tasks.len(),
                    titles.join("\n")
                ));
            }
        }
    }
}
```

Then replace the `invoke_worker` call at line 180:

```rust
let trigger = qc_trigger_override.as_deref().unwrap_or(RECIPE_TRIGGER);
match self.orch_client
    .invoke_worker(&agent_id, &instructions_with_ws, trigger)
    .await
```

**Rationale:** This gives the qc-agent two independent ways to discover tasks: (a) the DB gate prevents wasted API calls when empty, (b) the task list in the trigger message bypasses any memory hallucination when tasks exist, and (c) the gate also mitigates the kick-race — even if `kick_worker` was dropped because `qc-agent` was already mid-tick, the next scheduled tick will see the tasks in the DB and let them through with titles prepended.

---

#### Task 3: `src-tauri/src/recipes/runner.rs` (`guard_and_validate` + `reconcile_stranded_task` ordering) — Protect `qc_review` from sentinel guard

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\recipes\runner.rs:183-200`

**Current problem:**
- `guard_and_validate` runs at line 191 (before `reconcile_stranded_task` at line 198)
- `guard_and_validate` searches for `TaskStatus::InProgress` tasks and marks them `build_failed` if sentinel is missing
- `reconcile_stranded_task` then searches for `InProgress` tasks to promote to `qc_review`
- Because `guard_and_validate` already changed the status to `build_failed`, `reconcile_stranded_task` finds nothing
- Stranded tasks that should go to `qc_review` instead become `build_failed`

**Fix:** Swap the order so stranded tasks are promoted to `qc_review` FIRST, then the sentinel guard runs on any that are still `in_progress`:

```rust
// BEFORE (current):
Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
if worker_slug.starts_with("worker-") {
    Self::reconcile_stranded_task(worker_slug);
}

// AFTER (fixed):
if worker_slug.starts_with("worker-") {
    Self::reconcile_stranded_task(worker_slug);
}
Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
```

**Rationale:** A worker that wrote files but forgot to update status should be promoted to `qc_review` (so QC can verify the work). Only tasks that are STILL `in_progress` after reconciliation deserve the `build_failed` sentinel check.

---

### 🔴 P0 — Build Failed Tasks

#### Task 4: `src-tauri/src/hooks/ecosystem.rs` — Fix Node.js sentinel path for modern npm

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\hooks\ecosystem.rs:69-76`

**Current problem:**
```rust
Ecosystem::Node => PathBuf::from("node_modules/.package-lock.json"),
```
- `node_modules/.package-lock.json` is **not generated by npm v7+** (the lockfile lives at the project root)
- Even after a successful `npm install`, the sentinel is missing
- `guard_and_validate` therefore marks every frontend task `build_failed` incorrectly

**Fix:** Change the sentinel to check for the `node_modules` directory itself, which npm always creates:

```rust
Ecosystem::Node => PathBuf::from("node_modules"),
```

**Rationale:** The directory is a stronger signal than a file whose location changed between npm versions.

---

#### Task 5: `src-tauri/src/agent/builtin_tools/update_task_status_tool.rs` — Add `build_failed` to allowed enum

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\agent\builtin_tools\update_task_status_tool.rs:25-45`

**Current problem:**
```json
"enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "cancelled"]
```
- The `TaskStatus` enum in `tasks.rs` includes `BuildFailed` (`"build_failed"`)
- The UI store (`taskStore.ts`) includes `'build_failed'`
- But the `update_task_status` tool schema rejects it
- A worker that wants to report a build failure via the tool is forced to use `failed` (losing the distinction) or skip the tool call entirely

**Fix:** Add `"build_failed"` to the JSON schema enum:

```json
"enum": ["ready", "assigned", "in_progress", "qc_review", "done", "failed", "build_failed", "cancelled"]
```

Also update the description string on line 22 to include `build_failed`:
```
Valid statuses: ready, assigned, in_progress, qc_review, done, failed, build_failed, cancelled.
```

**Rationale:** The DB, the UI, and the runner (`guard_and_validate`) all support `build_failed`. The tool schema is the only outlier.

---

#### Task 6: `src-tauri/src/recipes/defaults.rs` (worker-frontend recipe) — Ecosystem-aware Build Validation with pre-flight install

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\recipes\defaults.rs:233-237`

**Current problem:**
- The recipe lists Node steps only as bullet points; the LLM sometimes skips `npm install` and runs validation directly
- No mention of Python (Streamlit, PyQt), Rust (Leptos/Yew WASM), or Go (WASM) frontend stacks
- When validation fails, the LLM may not know whether it's because dependencies are missing or because the code is broken
- Build Validation appears AFTER the Stop Contract, so the LLM may decide to stop before ever reaching it

**Fix:** Move Build Validation BEFORE the Stop Contract and make it numbered, mandatory, and ecosystem-aware:

```
## Build Validation (MANDATORY — execute BEFORE marking qc_review)
Detect the project's ecosystem by checking for these manifest files IN THIS ORDER:
1. `package.json` → Node:   run `npm install` if `node_modules/` is missing,
                           then `npx tsc --noEmit` or `npm run build`
2. `pyproject.toml` → Python: run `pip install -r requirements.txt` OR `poetry install` OR `uv sync`
                              (pick whichever tool's lockfile exists: requirements.txt, poetry.lock, uv.lock)
                              then `python -m pytest` or `uv run pytest`
3. `Cargo.toml` → Rust:    run `cargo build` then `cargo test`
4. `go.mod` → Go:          run `go mod tidy` then `go test ./...`

After the validate command:
- If it exits non-zero: write stderr to `build.log`, mark `build_failed`
- If it exits zero: mark `qc_review`
```

**Rationale:** Numbered, ecosystem-aware steps with explicit pre-conditions reduce the LLM's tendency to skip dependency installation and cover modern frontend stacks beyond React (e.g., Rust WASM, Streamlit). Placing it before the Stop Contract prevents the LLM from stopping early.

---

#### Task 7: `src-tauri/src/recipes/defaults.rs` (ALL worker recipes) — Harden Build Validation with ecosystem-aware numbered steps

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\recipes\defaults.rs:170-176` (worker-backend), `223-229` (worker-platform), `289-295` (worker-mcp), `337-343` (worker-cleanup)

**Current problem:**
- `worker-backend` has bullet-point ecosystem-aware instructions (Node, Python, Rust, Go) but the LLM can skip them
- `worker-platform`, `worker-mcp`, and `worker-cleanup` say "for the detected manifest" — too vague for an LLM to know which commands to run
- Build Validation appears AFTER the Stop Contract in all recipes, so the LLM may stop before reaching it
- No mention of `uv` (Python) or `pnpm`/`yarn` (Node) lockfile detection

**Fix:** Apply the same numbered, mandatory, ecosystem-aware structure to ALL four worker recipes. Example for `worker-backend` (adapt per worker scope):

```
## Build Validation (MANDATORY — execute BEFORE marking qc_review)
1. Detect the manifest in the working directory IN THIS ORDER:
   - `package.json`       → Node ecosystem
   - `pyproject.toml`     → Python ecosystem (check for `poetry.lock`, `uv.lock`, or bare)
   - `requirements.txt`   → Python (pip) ecosystem
   - `Cargo.toml`         → Rust ecosystem
   - `go.mod`             → Go ecosystem
2. Run the ecosystem-specific INSTALL command if the sentinel is missing:
   - Node:      `npm install` (or `pnpm install` / `yarn` if lockfile indicates)
   - Python:    `pip install -r requirements.txt` OR `poetry install` OR `uv sync`
   - Rust:      `cargo build`
   - Go:        `go mod tidy`
3. Run the ecosystem-specific VALIDATE command:
   - Node:      `npm run build` or `npx tsc --noEmit`
   - Python:    `python -m pytest` or `poetry run pytest` or `uv run pytest`
   - Rust:      `cargo test`
   - Go:        `go test ./...`
4. If step 3 exits non-zero: capture stderr to `build.log`, mark `build_failed`.
5. Only if step 3 exits zero: mark `qc_review`.
```

**Rationale:** Numbered, explicit ecosystem-aware steps prevent the LLM from skipping validation. Listing all four ecosystems (Node, Python, Rust, Go) with their install/validate commands ensures the fleet handles any codebase. Placing it before the Stop Contract guarantees the LLM sees it.

---

### 🟢 P2 — Observability

#### Task 8: `src-tauri/src/commands/task_commands.rs` — Add `get_qc_queue` Tauri command

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\commands\task_commands.rs`

**New function to add (after `clear_all_tasks`):**

```rust
/// Return tasks in qc_review plus age in seconds for observability.
#[tauri::command]
pub fn get_qc_queue() -> Result<Vec<(Task, i64)>> {
    let db = TasksDb::open()?;
    let tasks = db.list(Some("qc_review"))?;
    let now = chrono::Utc::now();
    let result = tasks.into_iter().map(|t| {
        let age = if let Ok(updated) = chrono::DateTime::parse_from_rfc3339(&t.updated_at) {
            now.signed_duration_since(updated).num_seconds()
        } else {
            0
        };
        (t, age)
    }).collect();
    Ok(result)
}
```

**Rationale:** Exposes QC backlog age so the UI and fleet health dashboard can detect stalled reviews.

---

#### Task 9: `src/stores/taskStore.ts` — Wire `get_qc_queue` into the store

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src\stores\taskStore.ts:47-59`

**New state + method:**

```typescript
interface TaskState {
  tasks:        Task[]
  qcQueue:      { task: Task; ageSeconds: number }[]  // NEW
  isLoading:    boolean
  error:        string | null

  loadTasks:          () => Promise<void>
  loadQcQueue:        () => Promise<void>            // NEW
  updateTaskStatus:   (id: number, status: TaskStatus, worker?: string) => Promise<void>
  deleteTask:         (id: number) => Promise<void>
  tasksByStatus:      (status: TaskStatus) => Task[]
  initEventListeners: () => Promise<void>
}
```

Implement `loadQcQueue`:

```typescript
loadQcQueue: async () => {
  try {
    const queue = await invoke<{ task: Task; ageSeconds: number }[]>('get_qc_queue')
    set({ qcQueue: queue })
  } catch (e) {
    console.error('Failed to load QC queue:', e)
  }
},
```

---

#### Task 10: `src/tabs/AutoDevPanel.tsx` — Surface `build_failed` count in MetricsRow

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src\tabs\AutoDevPanel.tsx:330-369`

**Current problem:**
```typescript
const todayDone   = tasks.filter(t => t.status === 'done' && isToday(t.completed_at))
const todayFailed = tasks.filter(t => t.status === 'failed' && isToday(t.updated_at))
```
- `build_failed` tasks are included in `doneTodayTasks` for the kanban but are **NOT counted** in the `todayDone` or `todayFailed` metrics
- QC pass rate calculation ignores them, understating the real failure rate

**Fix:** Add `todayBuildFailed` and include it in the metrics:

```typescript
const todayDone        = tasks.filter(t => t.status === 'done' && isToday(t.completed_at))
const todayFailed      = tasks.filter(t => t.status === 'failed' && isToday(t.updated_at))
const todayBuildFailed = tasks.filter(t => t.status === 'build_failed' && isToday(t.updated_at))
const qcPassRate  = todayDone.length + todayFailed.length + todayBuildFailed.length > 0
  ? Math.round((todayDone.length / (todayDone.length + todayFailed.length + todayBuildFailed.length)) * 100)
  : null
```

Add a new metric card in the `metrics` array:

```typescript
{
  label: 'Build Failed',
  value: String(todayBuildFailed.length),
  color: '#FFCB8B',
},
```

**Rationale:** `build_failed` is a terminal state (like `failed`). It should be visible in the daily summary.

---

#### Task 11: `src-tauri/src/hooks/ecosystem.rs` — Add `PythonUv` ecosystem variant

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\hooks\ecosystem.rs:10-17`

**Current problem:**
```rust
pub enum Ecosystem {
    Node,
    PythonPip,
    PythonPoetry,
    RustCargo,
    GoMod,
}
```
- Projects using `uv` (`pyproject.toml` + `uv.lock`) are detected as `PythonPip`
- `pip install -r requirements.txt` fails or installs into the wrong environment when `uv` is the actual tool

**Fix:** Add `PythonUv` variant and update `detect()`:

```rust
pub enum Ecosystem {
    Node,
    PythonPip,
    PythonPoetry,
    PythonUv,      // NEW
    RustCargo,
    GoMod,
}

pub fn detect(dir: &Path) -> Option<Self> {
    // ... existing checks ...
    if dir.join("pyproject.toml").exists() {
        if dir.join("uv.lock").exists() {
            return Some(Ecosystem::PythonUv);
        }
        if dir.join("poetry.lock").exists() {
            return Some(Ecosystem::PythonPoetry);
        }
        return Some(Ecosystem::PythonPip);
    }
    // ...
}
```

Add `install_cmd`:
```rust
Ecosystem::PythonUv => "uv sync",
```

Add `validate_cmd`:
```rust
Ecosystem::PythonUv => "uv run pytest",
```

Add `sentinel_path`:
```rust
Ecosystem::PythonUv => PathBuf::from(".venv/pyvenv.cfg"),
```

**Rationale:** `uv` is the fastest-growing Python toolchain. Without explicit support, `uv` projects are mis-detected as `PythonPip` and `guard_and_validate` fails or runs the wrong install command.

---

#### Task 12: `src-tauri/src/hooks/ecosystem.rs` — Fix Node sentinel path for modern npm

**Target code:** `@c:\AIRS Foundry\autodev\autodev\src-tauri\src\hooks\ecosystem.rs:69-76`

**Current problem:**
```rust
Ecosystem::Node => PathBuf::from("node_modules/.package-lock.json"),
```
- npm v7+ generates `package-lock.json` at the project root, not inside `node_modules/`
- `node_modules/.package-lock.json` is not guaranteed on pnpm/yarn or some npm configurations
- `guard_and_validate` falsely marks tasks `build_failed` even after a successful `npm install`

**Fix:** Change the Node sentinel to check for the directory itself:

```rust
Ecosystem::Node => PathBuf::from("node_modules"),
```

**Rationale:** The `node_modules` directory is created by **all** Node package managers (npm, pnpm, yarn, bun) on every install. It is a stronger, version-agnostic sentinel than an internal lockfile whose location changed between npm versions.

---

## 11. Appendix: Raw Log Patterns

### 11.1 QC Agent Tick Pattern (Repeats Every Minute)

```
[RecipeScheduler] Firing recipe 'qc-agent-review' for worker 'qc-agent'
[core-orch] --> POST /api/agents/qc-agent/invoke
[core-orch] [INVOKE] Headless invocation — session_id=worker-qc-agent
[core-orch] [TIMELINE] Retrieved N timeline messages
[core-orch] [TOOL-LOOP] === Iteration 1/25 ===
[core-orch] [API-CALL] Model: anthropic/claude-haiku-4-5-20251001
[core-orch] [TOOL-LOOP] Iteration 1 complete. Content length: 25, Tool calls: 0
[core-orch] [TOOL-LOOP] No tool calls in response, breaking loop.
[core-orch] [STREAM V2] Completed streaming for qc-agent (thinking: N chars, content: 25 chars)
[RecipeRunner] LivingAgent tick complete for 'qc-agent':
  No tasks in qc_review — idle.No tasks in qc_review — idle.
```

**Key observation**: `Tool calls: 0` — the qc-agent **never calls `list_tasks`** because the LLM decides (based on memory context) that there are no tasks. This suggests the memory context is telling the qc-agent "no tasks" before it even queries.

### 11.2 Worker Frontend Tick Pattern (Task Execution)

```
[RecipeScheduler] Firing recipe 'worker-frontend-poll' for worker 'worker-frontend'
[core-orch] --> POST /api/agents/worker-frontend/invoke
[core-orch] [TIMELINE] Retrieved 7 timeline messages
[core-orch] [TOOL-LOOP] === Iteration 1/25 ===
... (edit_file, bash, update_task_status tool calls) ...
[core-orch] [STREAM V2] Completed streaming for worker-frontend
[RecipeRunner] LivingAgent tick complete for 'worker-frontend':
  [Executing N tool(s)...]
```

### 11.3 Memory Store Pattern (Per Tick)

```
[memory-mcp] Processing request of type CallToolRequest
[memory-mcp] Executing method with failover: method=add_async
[memory-mcp] Annotated messages with memory type: annotated_first_msg_length=3348
[memory-mcp] Mem0 add result - NORMALIZED: memories_added=1
[memory-mcp] Provider mem0_primary add_async succeeded
```

---

## 12. Conclusion

The fleet architecture is **structurally sound** — sidecars boot, agents register, MCP bridges connect, recipes fire on schedule, workers execute tasks. The three blocking issues are all **fixable with targeted changes** in the main repo:

1. **QC agent blindness** is the most critical — it breaks the entire `qc_review → done` pipeline. The root cause is the qc-agent recipe's weak imperative (the LLM skips `list_tasks` based on stale memory) combined with unconditional Mode-2 invocation. Tasks 1–3 provide a two-layer fix: force the tool call in the recipe, gate the invoke path against the local DB, and protect `qc_review` tasks from the sentinel guard.

2. **Build failures** are secondary but important for output quality. The `node_modules/.package-lock.json` sentinel is wrong for modern npm (Task 12), the `update_task_status` tool schema rejects the `build_failed` value that the DB and UI already support (Task 5), and the worker recipes present validation as optional bullet points that the LLM skips (Tasks 6–7). The ecosystem enum also lacks `uv` support (Task 11), which is critical for modern Python projects.

3. **Memory bloat** is a performance/cost drag that compounds over time. The analysis in Section 5 is kept for the submodule refinement team; the core-orchestrator and memory-mcp-server submodules are being refined separately and should consume that knowledge.

**Execution order (main repo only):**
1. Task 12 (`ecosystem.rs` Node sentinel fix) — zero-risk, instant impact on all future frontend tasks
2. Task 11 (`ecosystem.rs` add `PythonUv`) — zero-risk, expands Python ecosystem coverage
3. Task 5 (`update_task_status_tool.rs` enum fix) — unblocks workers from reporting build failures correctly
4. Task 1 (`defaults.rs` qc-agent recipe) + Task 2 (`runner.rs` DB gate) — fixes the QC pipeline
5. Task 3 (`runner.rs` ordering) — prevents `build_failed` from swallowing stranded `qc_review` tasks
6. Tasks 6–7 (`defaults.rs` worker recipes) — hardens validation ordering for all workers across all ecosystems
7. Tasks 8–10 (`task_commands.rs`, `taskStore.ts`, `AutoDevPanel.tsx`) — observability layer on top
