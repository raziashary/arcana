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

### Phase 1 — Replace serial QC with per-task subagents (functional fix)

- [ ] Add a new builtin tool `spawn_qc_review` at `src-tauri/src/agent/builtin_tools/spawn_qc_review.rs` — wraps `SubagentManager::spawn` with a fixed system-prompt template, fixed model (`claude-sonnet-4-5`), and a required `task_id` input.
- [ ] Register the tool in `src-tauri/src/agent/builtin_tools/mod.rs` alongside `spawn_subagent` / `spawn_parallel_subagents`.
- [ ] Extract the per-task body of the existing `qc_agent_recipe` (Phases 0–5 at `src-tauri/src/recipes/defaults.rs:1104`) into a new `qc_reviewer_system_prompt(task_id, repo_path)` builder so the spawned reviewer's prompt is task-scoped.
- [ ] Add `qc_dispatcher_recipe()` at `src-tauri/src/recipes/defaults.rs` — model `claude-haiku-4-5`, `schedule: "*/30 * * * * *"` (30 s), `agent_role: "qc-dispatcher"`, instructions = "list_tasks(qc_review); for each not under review, call spawn_qc_review(task_id); update metadata; return".
- [ ] Register the new recipe ID `QC_DISPATCHER_RECIPE_ID` in the recipes registry at `src-tauri/src/recipes/defaults.rs:43`.
- [ ] In `src-tauri/src/db/workers.rs`, swap the seeded `qc-agent` row's `recipe_id` to `qc-dispatcher` (or insert `qc-dispatcher` and retire `qc-agent`).
- [ ] Update the reconciler / `kick_worker` call sites that currently target `qc-agent` to target `qc-dispatcher` — check `src-tauri/src/commands/task_commands.rs` and `src-tauri/src/db/workers.rs` for hardcoded `"qc-agent"` slugs.

### Phase 2 — Idempotency and queue safety

- [ ] Add an `under_review_by` text column to `tasks.db` (migration in `src-tauri/src/db/migrations/`) — value is the subagent slug, NULL when not under review.
- [ ] In `spawn_qc_review`, set `under_review_by = subagent_id` atomically before the spawn returns; clear it on terminal status events in `SubagentManager::finalise` at `src-tauri/src/subagent/manager.rs:287`.
- [ ] Update the supervisor recipe at `src-tauri/src/recipes/defaults.rs:295` to skip tasks where `under_review_by IS NOT NULL` (covers a dispatcher crash mid-spawn).
- [ ] Add a stranded-reviewer reconciler: on startup and every 5 min, scan for tasks with `under_review_by` referencing a subagent not present in `SubagentManager::list` and clear the flag so the next dispatcher tick re-spawns. Hook this into the existing stranded-task reconciler.

### Phase 3 — Tighten resource ceilings

- [ ] Add a QC-specific concurrency cap (e.g. `MAX_CONCURRENT_QC = 6`) at `src-tauri/src/subagent/pool.rs:26` — a second tokio Semaphore that any `parent_slug == "qc-dispatcher"` acquires before the global pool. Keeps a burst of 30 `qc_review` tasks from starving codegen workers.
- [ ] Wire a per-tick cap into the dispatcher recipe instructions: "spawn at most 6 reviewers per tick; remaining tasks ride the next 30 s pulse".
- [ ] Surface the live QC reviewer count in the Subagent Monitor: filter by `parent_slug == "qc-dispatcher"` — uses `SubagentManager::list` at `manager.rs:172` which already supports filtering.

### Phase 4 (optional) — Observability and cost guardrails

- [ ] Emit a `qc_review_dispatched` analytics event with `task_id`, `subagent_id`, dispatcher tick latency. Hook into `src-tauri/src/events/` alongside the existing `subagent:event` emitter.
- [ ] Aggregate per-task QC cost from the `SubagentEvent::Cost` frames already parsed at `src-tauri/src/subagent/bridge.rs:259` and write to a new `task_qc_cost_usd` column on completion.
- [ ] Add a "QC ROI" panel in the AutoDev UI — for each finished task, show `qc_cost_usd / worker_cost_usd` so the user can spot tasks where the reviewer cost outweighs the implementation cost.
- [ ] Add a hard cost guard: if cumulative QC spend in the last hour exceeds a configurable threshold (default $5), the dispatcher pauses spawning and emits a warning event. Belt-and-braces against a runaway reject/retry loop.

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
