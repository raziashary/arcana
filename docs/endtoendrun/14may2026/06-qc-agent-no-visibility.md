# 06 — QC Agent has no visibility (always idle in the Live Monitor)

## Symptom
The QC Agent's Live Monitor card stays grey/idle for the entire session
even though the log shows it actually ran a 5-minute LivingAgent tick.

## Recipe — qc-agent does fire
[`defaults.rs:1112`](../../../src-tauri/src/recipes/defaults.rs) defines the
`qc-agent-review` recipe. It is scheduled `0 */1 * * * *` (every minute,
[scheduler log line 35](../../../logs/autodev-.2026-05-14..log)).

The pre-flight gate at
[`runner.rs:218-230`](../../../src-tauri/src/recipes/runner.rs) — added to
avoid burning tokens when there's nothing to review — early-returns when
`list_tasks(status='qc_review')` is empty. **This is the source of the
"No qc_review tasks in DB — skipping qc-agent tick" log line.**

Critically, that early-return path **does** emit:
- `emit_worker_tick_end` with `outcome='idle'` ([line 223](../../../src-tauri/src/recipes/runner.rs))
- `set_worker_status(slug, 'idle')` ([line 228](../../../src-tauri/src/recipes/runner.rs))

But it skips the actual tick body. There is no separate "doing nothing
intentionally" surface — the card just looks dead.

## Supervisor IS moving tasks to qc_review (eventually)
The supervisor's recipe instructions only set `qc_review` directly on
`request_review` worker requests
([`defaults.rs:222`](../../../src-tauri/src/recipes/defaults.rs)). Otherwise,
when a worker stops mid-`in_progress` without explicit completion, the
Reconciler in `runner.rs` does the transition. Log evidence:

```
11:19:53  Reconciled stranded in_progress task #8  → qc_review (worker-backend)
11:21:59  Reconciled stranded in_progress task #3  → qc_review (worker-backend)
11:23:51  Reconciled stranded in_progress task #9  → qc_review (worker-frontend)
11:26:27  Reconciled stranded in_progress task #1  → qc_review (worker-backend)
```
([lines 491 / 581 / 659 / 762](../../../logs/autodev-.2026-05-14..log))

So between 11:14 and 11:19 there really were zero qc_review tasks — the
"skipping" log is correct behaviour, not a bug. The supervisor's path to
qc_review is mostly via reconciliation, not via direct supervisor LLM
action.

## QC Agent IS working — invisibly
At 11:19:54, immediately after the first reconciliation, the scheduler
pushed a kick-fire to qc-agent
([line 492](../../../logs/autodev-.2026-05-14..log)):

```
11:19:54.139  kick_worker: firing 'qc-agent' immediately (push-based dispatch)
11:19:54.145  tick dispatch — worker='qc-agent' core-orchestrator=reachable → Mode 2
```

That tick then ran for **304 644 ms = 5 min 5 s** and completed at:

```
11:24:58.797  LivingAgent tick complete for 'qc-agent' in 304644ms: ## QC Agent - Execution Start
```
([line 692](../../../logs/autodev-.2026-05-14..log))

During those 5 minutes the scheduler repeatedly logged
`Worker 'qc-agent' still working — skipping tick` (lines 504, 552, 587,
638, 660) — proof the engine knew it was busy. So Rust correctly emitted
`worker:tick-start` and `worker:status='working'`. The Live Monitor card
should have been green for 5 minutes.

## Why the card was still idle
Same root cause as Doc 1
([`01-live-monitor-stale.md`](./01-live-monitor-stale.md)):

[`agentLiveStore.ts:200-203`](../../../src/stores/agentLiveStore.ts):

```ts
handleStatusChange: ({ agent_id, status }) => {
  set(state => {
    const agent = state.agents[agent_id]
    if (!agent) return state          //  ← silently drops 'qc-agent' if not initialised
```

`initAgents(['qc-agent', …])` is called from
[`AutoDevPanel.tsx:107`](../../../src/tabs/AutoDevPanel.tsx) only after
`workerStore.loadWorkers()` returns AND only on `workers.length` change.

Two QC-specific failure modes on top of the generic Doc 1 race:

1. **STARTUP tick fires at 11:14:13** ([line 48](../../../logs/autodev-.2026-05-14..log))
   — that is ~1 ms after `RecipeScheduler` boots and well before the
   frontend has finished loading workers. `worker:tick-start` for
   `qc-agent` arrives, finds `state.agents['qc-agent'] === undefined`,
   returns state unchanged.

2. **Restart at 11:27:13** ([line 829](../../../logs/autodev-.2026-05-14..log)):
   qc-agent's scheduler was re-created mid-session ("Starting worker
   'qc-agent' on schedule"). This issued a fresh STARTUP tick at
   11:27:13.898 but `workers.length` didn't change, so `initAgents` was
   not re-run, and even if it had been, the agent was already in the store
   — so its `status` was never explicitly reset to `'active'` for this
   second-life tick if the first life had already moved it to `idle`. The
   `worker:tick-start` arrives, **does** update `status` to `'active'`,
   but the user looking at the panel sees a single static state — without
   a current_tool string (LivingAgent ticks don't emit
   `agent:tool-call-start` events because that's the Mode 1 channel,
   [comment block at agentLiveStore.ts:264-272](../../../src/stores/agentLiveStore.ts)),
   so the card has no log lines, no tool name, no file name — just a
   colour. Easy to miss.

## Report
**Is qc-agent actually idle?** No — when there is `qc_review` work, it
runs (304 s tick proven in logs). When there isn't, it correctly
short-circuits with an explicit `idle` status emit.

**Is it working invisibly?** Yes, during the 11:19:54 → 11:24:58 window.
The events are emitted; the frontend store drops them because the slug
wasn't preinitialised (Doc 1 root cause). Even after the slug is added,
the QC card is intentionally bare because Mode 2 doesn't emit the
`agent:tool-call-*` events that populate the card body.

## Suggested fix (no code change in this audit)
1. Apply Doc 1's auto-register-on-unknown-slug fix; that alone makes the
   `worker:tick-start` events stick.
2. For Mode 2 LivingAgent ticks, bridge core-orchestrator's SSE tool
   events (already pumped by `SubagentManager`,
   [`events.rs:85` `EVT_SUBAGENT_EVENT`](../../../src-tauri/src/events.rs))
   into `agent:tool-call-start` / `agent:tool-call-result` so the card
   body actually shows what QC is doing instead of just a green dot.
3. When the qc-agent gate short-circuits with "No qc_review tasks", emit
   a one-shot info breadcrumb (e.g. a `log_lines` push with text "idle —
   no tasks") instead of leaving the card visually empty. Distinguishes
   "intentionally idle" from "stuck idle".

---

## Atomic implementation tasks

> Two root causes here: (1) generic slug-not-registered drop — same as [01-live-monitor-stale.md](01-live-monitor-stale.md); (2) Mode 2 ticks emit no tool-call events — same as [02-dashboard-not-live.md](02-dashboard-not-live.md). Cross-reference [04-qc-backlog.md](04-qc-backlog.md): if QC becomes ephemeral subagents, this visibility surface changes shape.

### Phase 1: Lazy slug registration — ✅ COMPLETE 14 May 2026

> See [01-live-monitor-stale.md](01-live-monitor-stale.md#phase-1-lazy-slug-registration-in-agentlivestore) — this fix unblocks the QC card too.

- [x] Inherited the lazy-create patch from doc 01 — `src/stores/agentLiveStore.ts` `handleStatusChange` / `handleToolCallStart` / `handleToolCallResult` now auto-create unknown agents via `state.agents[agent_id] ?? makeInitialAgent(agent_id)`. The `qc-agent` STARTUP tick at 11:14:13 now registers correctly even before `initAgents` has run.
- [x] Verified `qc-agent` slug parity — `src-tauri/src/db/workers.rs:178` seeds `qc-agent` into the workers table, and `src-tauri/src/recipes/defaults.rs:1112` sets `agent_role: Some("qc-agent")` on the recipe. Frontend slug exposed via `workerStore.workers` matches the slug used by `emit_worker_tick_start`.

### Phase 2: Bridge Mode 2 events to UI — ✅ COMPLETE 14 May 2026

> See [02-dashboard-not-live.md](02-dashboard-not-live.md) — same bridge work serves both surfaces.

- [x] Subscribed to `subagent:event` in `agentLiveStore.subscribeToEvents` — `src/stores/agentLiveStore.ts`. Translates `kind: "tool_start" | "tool_end" | "status" | "error"` into the existing handlers, using the subagent slug as `agent_id`. QC's 5-minute tick now fills the card body via the lazily-registered slug.
- [x] Confirmed the Rust re-emit path — `src-tauri/src/subagent/manager.rs:277` already pumps every `SubagentEvent` to the `subagent:event` channel via `emit_subagent_event_global`. No new adapter needed — the frontend listener completes the bridge.
- [x] Mode 2 *main-worker* tool-call streaming — ✅ now landed. `OrchestratorClient::invoke_worker_streaming` (`src-tauri/src/sidecar/client.rs`) opens an SSE connection to `/api/agents/{id}/invoke/stream`, reuses `subagent::bridge::{find_frame_end, parse_event_frame}` (both made `pub(crate)` in `src-tauri/src/subagent/bridge.rs`), and re-emits each parsed `SubagentEvent` on the existing `subagent:event` channel using the *worker slug* as `subagent_id`. `recipes::runner` swapped `invoke_worker` → `invoke_worker_streaming` for the Mode 2 path. The Live Monitor card body now fills with main-worker tool calls (read_file, bash, etc.) in real time during the multi-minute LivingAgent tick, not just for subagents.

### Phase 3: QC-specific UI surface — ✅ COMPLETE 14 May 2026

- [x] "Currently reviewing task #N" on the QC card — `src/components/autodev/AgentLiveCard.tsx` + `src/stores/agentLiveStore.ts`. New `parseTaskId(tool_name, args)` helper recognises QC-style tool calls (`get_task`, `update_task_status`, `update_task`, `claim_task`, `request_review`, `set_task_status`) and extracts the integer `id` / `task_id` from the args preview. `handleToolCallStart` writes it to `agent.task_id`, falling back to the previous value when the call doesn't carry an id. The QC card renders a `reviewing #N` row (above current-tool) and clicking it pulses the qc_review column.
- [x] "idle — no qc_review tasks" breadcrumb — `src-tauri/src/events.rs` (new `EVT_WORKER_LOG_LINE` + `WorkerLogLinePayload` + `emit_worker_log_line[_global]`), `src-tauri/src/recipes/runner.rs` (both qc-agent skip branches now fire the breadcrumb with text `"idle — no qc_review tasks"` or `"idle — only build_failed tasks awaiting QC"` immediately before `emit_worker_tick_end`), `src/stores/agentLiveStore.ts` (new `worker:log-line` listener pushes a synthetic LogLine with `tool_name: '·'` and `status: 'thought'` so it renders as an inert breadcrumb in the card's log tail). "Stuck idle" and "intentionally idle" are now visually distinguishable.
- [x] "Tasks in qc_review" count chip on the QC card — `src/components/autodev/AgentLiveCard.tsx`. Card now reads `useTaskStore` and counts qc_review tasks via a memoised filter; renders a small chip in the header (`QC: 0` muted / `QC: N` accent-tinted when N > 0). Updates live because `taskStore.tasks` already fans out on `task:changed`. The chip is QC-card-only — the worker / supervisor / PA cards don't render it.
- [x] Click chip / "reviewing" row → jump to qc_review column — `src/stores/uiStore.ts` (new `taskColumnPulse: { colId, nonce }` + `pulseTaskColumn(colId)` action), `src/tabs/AutoDevPanel.tsx` (new effect watches the pulse counter, looks up `[data-kanban-col="qc_review"]`, scrolls into view, and applies a temporary `kanban-col-pulse` class), `src/components/autodev/LiveMonitor.css` (new `@keyframes kanban-col-pulse-anim` with a 1.5s accent-glow). Since the QC kanban column is already on-screen on the AutoDev tab, this scrolls + flashes the existing column rather than activating a tab filter — same UX intent without inventing a separate Tasks tab.
