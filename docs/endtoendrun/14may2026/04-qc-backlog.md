# QC backlog — fix with headless per-task QC agents

## TL;DR

The persistent `qc-agent` worker serializes every QC review on a single sonnet
LivingAgent (304 s per tick in the 14 May run), so three tasks landed in
`qc_review` while it was busy and none could be picked up until 5 minutes
later. The submodule already exposes the create → invoke(stream) → delete
lifecycle that powers AutoDev's existing `spawn_subagent` builtins — we will
reuse the same `SubagentManager` plumbing to fan out one **ephemeral headless
QC subagent per task**, replacing the shared worker with a thin
`qc-dispatcher` that just picks tasks off the queue and spawns reviewers.

## Current behavior

From `logs/autodev-.2026-05-14..log`:

- `11:19:54.139Z` (L492) — single push-kick fires `kick_worker('qc-agent')`
  after task #8 reconciles into `qc_review`.
- `11:24:58.797Z` (L692) — `LivingAgent tick complete for 'qc-agent' in
  304644ms` (**5 min 4 sec** for one task).
- Inside that window: `Worker 'qc-agent' still working — skipping tick`
  fires on every 60 s cron pulse (L504, L552, L587, L638, L660) and every
  reconcile-driven push-kick (L572 for task #3, L653 for task #9) is dropped
  by the `if w.status == "working" { return; }` guard at
  `src-tauri/src/recipes/scheduler.rs:332`.
- Tasks #3, #9, #1 sat in `qc_review` for the full 304 s window. The orch
  crashed before the second tick (L703) reported completion, so the realized
  end-to-end latency was effectively unbounded.

## Why a single shared qc-agent doesn't scale

Three structural reasons, all rooted in the persistent-worker model.

1. **Serial bottleneck by construction.** `kick_worker` at
   `scheduler.rs:327` is gated on `status == "working"` and a worker has
   exactly one row in `workers.db`. There is no queue — concurrent kicks are
   dropped, not enqueued.
2. **Full-context recreation every tick.** Each `LivingAgent` tick replays
   the full QC system prompt (the recipe at `src-tauri/src/recipes/defaults.rs:1104`
   is ~250 lines of phase logic, domain checklists, and rejection format),
   then iterates over every task in `qc_review` in one giant chain of
   `analyze_diff` → `read_file` → `bash` → `update_task_status` calls.
   Even with prompt caching, the per-task work happens sequentially inside
   one model session.
3. **Sonnet latency dominates.** qc-agent runs `claude-sonnet-4-5` (vs
   haiku for workers/supervisor — see L74, L110 of the log). One task
   review is acceptable on sonnet; six tasks chained inside one session
   compounds to multi-minute ticks. The supervisor's haiku ticks averaged
   30–60 s in the same run.

## What the submodule already supports

Per-task fan-out does **not** require any change inside
`submodules/core-orchestrator/`. The lifecycle endpoints we need already
exist and are already wrapped by AutoDev:

- `POST /api/agents` — `OrchestratorClient::create_ephemeral_agent` at
  `src-tauri/src/sidecar/client.rs:1273` posts a full agent body
  (`llm_model`, `system_prompt`, `mcp_servers`, `capabilities`) and returns
  an agent_id. core-orchestrator writes a YAML to
  `config/agents/{slug}.yaml` and registers a `LivingAgent` (lifecycle
  factory under `submodules/core-orchestrator/src/airs/agent/factory.py`).
  The persona is short-lived — we own the deletion.
- `POST /api/agents/{id}/invoke/stream` — `invoke_agent_stream_raw` at
  `client.rs:1369` returns the SSE response. `subagent/bridge.rs:214`
  already parses the frames (`token`, `tool_start`, `tool_end`, `final`,
  `cost`, `error`) into our internal `SubagentEvent` enum.
- `DELETE /api/agents/{id}` — `delete_agent` at `client.rs:1338`.
- Orphan cleanup of `subagent-*` slugs already runs at startup via
  `SubagentManager::cleanup_orphans` (`manager.rs:234`).
- **Concurrency** — `SubagentPool` at `src-tauri/src/subagent/pool.rs:26`
  enforces `MAX_GLOBAL_CONCURRENT = 16` and `MAX_PER_PARENT = 10` via a
  tokio `Semaphore`, with RAII `PoolPermit` drop-guards releasing slots.
- **MCP wiring** — `bridge.rs:64` defaults `mcp_servers` to
  `["autodev-mcp-server"]` and `bridge.rs:80` prepends a `[AUTODEV
  WORKSPACE]` preamble with `working_dir` and `agent_depth` plus the
  mandatory `__depth` propagation instruction. A spawned QC reviewer
  inherits the full MCP tool surface (read_file, write_file, bash,
  update_task_status, analyze_diff, annotate_file) with zero extra config.

In short: AutoDev already has every primitive needed to spawn N
independent QC reviewers, route their SSE events into the existing
`subagent:event` bus, and reap them on completion. We just don't use those
primitives for QC yet.

## Proposed architecture: qc-dispatcher + ephemeral qc-reviewer subagents

Replace the `qc-agent` worker with a lightweight `qc-dispatcher` worker
whose only job is to **scan `qc_review` and fan out**.

**qc-dispatcher** (new persistent worker, haiku, 30 s cron + push-kick
on `qc_review` transitions):

1. Calls `list_tasks(status='qc_review')`.
2. For each task not already under review, invokes a new
   `spawn_qc_review` builtin tool (or `spawn_parallel_subagents` with a QC
   preset) that bottoms out in `SubagentManager::spawn` with:
   - `parent_slug = "qc-dispatcher"`
   - `model = "claude-sonnet-4-5"` (keeps the quality bar; latency is now
     parallel)
   - `system_prompt` = the existing `qc_agent_recipe()` instructions
     **scoped to a single task_id** (templated at spawn time)
   - `tools = []` (MCP-only — same as today)
   - `timeout_secs = 600`, `max_turns = 30`
3. Marks the task with an `under_review_by` metadata flag so a second
   dispatcher tick does not double-spawn.
4. Returns immediately — the dispatcher tick is sub-second.

**qc-reviewer-{slug}** (ephemeral, one per task):

- Lives only for the duration of one review (~30–90 s on sonnet for one
  task vs 304 s for six).
- Streams events through the existing `SubagentManager` → `subagent:event`
  pipeline; the frontend Subagent Monitor already renders these.
- Calls `update_task_status(id, 'done'|'rejected', ...)` on the MCP
  server; on completion, `SubagentManager::finalise` releases the
  `PoolPermit` and `delete_agent` is called via the bridge's drop path.

This is **Approach B** from the spec — chosen over A and C because:

- **A (spawn_parallel_subagents inside a still-persistent qc-agent)**
  preserves the cold-start-per-tick problem and the serial gate at
  `kick_worker`. It also requires the agent to manage its own dispatch
  state across ticks (which task did I just spawn for?), reintroducing the
  same race the dispatcher avoids.
- **C (call `/api/agents/{id}/invoke` directly from a Rust tool,
  bypassing SubagentManager)** loses the pool, the event bus integration,
  the orphan-cleanup safety net, the depth guard, and the Subagent Monitor
  UI. None of those are worth duplicating.

Approach B keeps every existing safety mechanism, costs almost nothing to
implement (the dispatcher recipe is ~40 lines), and naturally inherits the
hard cap from `SubagentPool` (16 concurrent reviewers max, 10 per parent —
plenty of headroom and a real ceiling under burst load).

## Atomic implementation tasks

### Phase 1 — Replace serial QC with per-task subagents (functional fix) — ✅ COMPLETE 14 May 2026

- [x] New builtin tool `spawn_qc_review` at `src-tauri/src/agent/builtin_tools/spawn_qc_review.rs` — atomically claims the task via `TasksDb::try_claim_for_qc_review`, builds the per-task system prompt, spawns the subagent on `claude-sonnet-4-5`, upgrades the claim to the real subagent_id, and detaches a background tokio task that awaits the reviewer's terminal state to attribute cost (`add_qc_cost`), release the claim, and record analytics events.
- [x] Registered the tool in `src-tauri/src/agent/builtin_tools/mod.rs` for both `register_for_mcp_worker` (Mode 2 LivingAgents) and `register_for_mode` (PA / Mode 1). Recipe-level guardrails forbid all non-dispatcher callers from using it.
- [x] Extracted `qc_reviewer_system_prompt(task_id, repo_path)` in `src-tauri/src/recipes/defaults.rs` — Phases 0–5 of the legacy QC protocol, scoped to one task. Removes "list_tasks queue iteration", "No tasks available" branch, and "Subagent Protocol" (subagents cannot nest more QC). Exposed as `pub fn` for the spawn tool to consume.
- [x] Added `qc_dispatcher_recipe()` — model `claude-haiku-4-5`, schedule `*/30 * * * * *`, `agent_role: "qc-agent"` (slug retained for backward compat). Recipe instructions: list queue → filter `under_review_by` / `build_failed` → cap at 6 spawns/tick → call `spawn_qc_review(task_id)` per task → stop.
- [x] Registered `QC_DISPATCHER_RECIPE_ID = "qc-dispatcher"` in the recipes registry next to `QC_AGENT_RECIPE_ID` (both kept; the old one is no longer bound but remains available for manual rebinding).
- [x] Swapped `src-tauri/src/db/workers.rs:178` seed: QC Agent row now binds to recipe `qc-dispatcher`, model `claude-haiku-4-5`, poll interval 60s. The `qc-agent` slug is retained — kick_worker / Live Monitor / hooks all continue working.
- [x] Slug parity verified — keeping the `qc-agent` slug means the existing hardcoded references in `scheduler.rs:462`, `task_commands.rs:36`, `update_task_status_tool.rs:140`, `client.rs:284`, `runner.rs:218-230` all continue to function unchanged. The dispatcher's `parent_slug` for the subagent pool is the separate constant `QC_DISPATCHER_SLUG = "qc-dispatcher"` defined in `pool.rs`.

### Phase 2 — Idempotency and queue safety — ✅ COMPLETE 14 May 2026

- [x] Added `under_review_by TEXT` and `task_qc_cost_usd REAL DEFAULT 0.0` columns to `tasks.db` via idempotent `ALTER TABLE` migrations in `TasksDb::init_schema`. New `idx_tasks_under_review_by` index keeps the dispatcher's filter query fast.
- [x] Atomic claim/release primitives in `src-tauri/src/db/tasks.rs`: `try_claim_for_qc_review(task_id, slug) -> bool` uses `UPDATE … WHERE under_review_by IS NULL` and returns whether the row count was 1 (claim won) or 0 (race lost). `upgrade_qc_claim` swaps the synthetic claim token for the real subagent_id post-spawn. `release_qc_claim` and `list_qc_claims` round out the lifecycle. Wired into `spawn_qc_review` (claim before spawn, upgrade after spawn returns the handle, release in the background completion handler).
- [x] Supervisor recipe `defaults.rs` — clarifying note under "QC-review tasks": acknowledges the new `under_review_by` flag, points at the orphan reconciler for stranded-claim cleanup, and reaffirms the "do NOT touch qc_review" rule. The supervisor already skipped qc_review entirely, so this is documentation-only — no behavior change required.
- [x] Stranded-reviewer reconciler in `src-tauri/src/orphan/mod.rs` — `reconcile_stranded_qc_claims` runs on every existing orphan reconciler tick (no new timer). For each claim it cross-references `SubagentManager::list` (live registry); the real subagent_id is matched exactly, while the pre-upgrade synthetic `qc-rev-task<id>-<ts>` token falls back to a 90-second age heuristic. On process restart, the empty registry sweeps every claim on the first tick.

### Phase 3 — Tighten resource ceilings — ✅ COMPLETE 14 May 2026

- [x] `MAX_CONCURRENT_QC = 6` + dedicated `qc: Semaphore` slot in `src-tauri/src/subagent/pool.rs`. Acquired BEFORE the global semaphore using `try_acquire_owned` (so a burst of QC spawns rejects immediately instead of back-pressuring worker subagents). Released-on-drop via the same `PoolPermit` mechanism as the global cap. New `QC_DISPATCHER_SLUG = "qc-dispatcher"` constant single-sources the slug check across pool / tool / reconciler.
- [x] Per-tick cap baked into the dispatcher recipe instructions: "Cap at 6 spawns per tick. Even if 30 tasks are pending, spawn at most 6 reviewers per tick. The remaining tasks ride the next 30 s pulse." Belt-and-suspenders with the pool cap — recipe-level keeps the LLM from even trying to overflow.
- [x] Live reviewer count via new Tauri command `get_qc_reviewer_stats` in `src-tauri/src/commands/subagent_commands.rs` returning `{ in_flight, max_concurrent }`. Registered in `lib.rs`. Pool exposes `qc_available()` so the count is O(1).

### Phase 4 — Observability and cost guardrails — ✅ COMPLETE 14 May 2026

- [x] Analytics events — `spawn_qc_review` records `qc_review_dispatched` synchronously on spawn, then `qc_review_dispatched_complete` / `_failed` from the background completion handler. All three flow through `analytics::global().collector().record_system_event` with component=`qc-dispatcher`, severity=info/warn, and JSON metadata containing `{ kind, task_id, subagent_id, cost_usd, duration_secs, success }`. Surfaces in `get_system_logs` (admin dashboard) and is queryable via the analytics DB for ad-hoc audits.
- [x] Per-task QC cost rollup — `TasksDb::add_qc_cost(task_id, cost_usd)` increments `task_qc_cost_usd` atomically. Background completion handler in `spawn_qc_review` calls it with `SubagentResult.cost_usd` on success. Task DTO + frontend `Task` interface carry the new field for the UI to consume.
- [x] QC ROI panel — new `src/components/dashboard/QcRoiPanel.tsx`. Top 8 tasks by `task_qc_cost_usd` descending, each row showing `#id`, title, `QC$`, `WORKER$` (= `budget_cost_usd`), and `QC/W` ratio. Ratios > 100% are tinted with `var(--danger)` so suspiciously-expensive reviews jump out. Header chip shows `Reviewers: in_flight / max_concurrent`, polled from `get_qc_reviewer_stats` every 5 s and refreshed instantly on every `subagent:event`. Mounted in `AutoDevPanel.tsx` directly under the Live Monitor section.
- [x] Hard cost guard in `spawn_qc_review.rs::qc_cost_guard` — rolling 1-hour ring of `(timestamp, cost)` entries. `check_and_record_dispatch_intent()` runs BEFORE the atomic claim and rejects when the sum (including a $0.10 provisional reserve per dispatch) would exceed $5/hour. `record_dispatch_complete(realised_cost)` swaps the provisional entry for the realised cost when the reviewer terminates so the window self-corrects. The reject error includes a human-readable explanation so the dispatcher's next tick stops spawning and reports the trip to the user.

## Concurrency / safety considerations

- **Hard cap on concurrent reviewers**: `MAX_GLOBAL_CONCURRENT = 16` and
  the proposed `MAX_CONCURRENT_QC = 6` bound the worst case. If 30 tasks
  land in `qc_review` at once, the dispatcher spawns 6, the rest wait in
  the DB (not in memory), and the next 30 s tick picks up the next 6.
  Throughput cap: ~12 reviews/min sustained, ~30 s p50 per task — vs the
  current ~12 reviews/hour.
- **No depth explosion**: `qc-dispatcher` is a persistent worker (depth
  0). Its spawned reviewers are at depth 1. `MAX_NESTING_DEPTH = 3` at
  `pool.rs:32` blocks any chain of QC-spawning-QC, even if a reviewer
  hallucinates a `spawn_subagent` call.
- **No double review**: the `under_review_by` flag + supervisor skip
  rule ensure a task cannot be picked up twice. A reviewer crash leaves
  the flag stuck — the Phase 2 stranded-reviewer reconciler clears it
  within 5 min.
- **Cost is roughly equivalent, latency much better**: the current
  serial qc-agent already runs sonnet on every task it sees; six parallel
  sonnet invocations cost the same in tokens but complete in ~1/6 the
  wall time. The Phase 4 cost guard caps the worst case if rejections
  loop.
- **No submodule changes**: every endpoint we consume already exists in
  `submodules/core-orchestrator/`. The submodule stays read-only.
- **Cleanup is automatic**: `SubagentManager::cleanup_orphans` at
  `manager.rs:234` deletes any `subagent-*` agents left behind on
  restart. `PoolPermit` Drop releases pool slots on either success or
  panic. The bridge always attempts `delete_agent` regardless of
  outcome (`bridge.rs:130`).
- **Failure isolation**: one reviewer timing out or crashing now affects
  exactly one task instead of blocking the whole queue for 5 minutes. The
  failed task transitions to `rejected` with a "QC reviewer failed:
  <reason>" rejection, surfacing the issue without halting the pipeline.
