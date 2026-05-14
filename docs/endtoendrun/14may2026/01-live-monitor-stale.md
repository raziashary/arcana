# 01 — Live Monitor STALE/IDLE while workers are actually working

## Symptom
The Live Monitor badge stays on `STALE` or `IDLE` even when the Agent Roster
correctly shows workers in the `working` state mid-tick. The recent fix to
[`agentLiveStore.subscribeToEvents()`](../../../src/stores/agentLiveStore.ts:235)
was supposed to bridge Mode 2 `worker:tick-*` events into the same store, but
the cards still never light up.

## The wire we expected
1. Rust emits `worker:tick-start` on tick begin
   ([`runner.rs:201`](../../../src-tauri/src/recipes/runner.rs)).
2. `subscribeToEvents()` listens and calls `handleStatusChange({ agent_id: slug, status: 'active' })`
   ([`agentLiveStore.ts:273-278`](../../../src/stores/agentLiveStore.ts)).
3. `handleStatusChange` flips that agent's `status` to `active` and the card
   turns green.

## The bug — guarded mutation drops the event silently
[`agentLiveStore.ts:201-202`](../../../src/stores/agentLiveStore.ts):

```ts
const agent = state.agents[agent_id]
if (!agent) return state    //  ← silent no-op when agent isn't preinitialised
```

So if `state.agents['worker-backend']` is `undefined` when the event arrives,
the update is dropped. There is no auto-create-on-first-event path.

## Who is supposed to call `initAgents(['worker-backend', …])`?
Exactly one place:
[`AutoDevPanel.tsx:105-108`](../../../src/tabs/AutoDevPanel.tsx) — inside
`AgentRoster`:

```ts
useEffect(() => {
  const ids = workerStore.workers.map(w => w.slug)
  if (ids.length > 0) initAgents(ids)
}, [workerStore.workers.length])
```

Two problems with that:

1. **Race condition.** `subscribeToEvents()` is started by
   [`LiveMonitorSection.tsx:26-29`](../../../src/components/autodev/LiveMonitorSection.tsx)
   on its mount, which happens *before* `workerStore.loadWorkers()` finishes
   the initial Tauri round-trip. If a `worker:tick-start` arrives in that
   window (very plausible at startup — the scheduler fires a STARTUP tick
   within 100 ms, see log lines 35/48), `state.agents` is `{}` and the event
   is dropped. The Live Monitor then sits there forever waiting for an event
   that already passed.

2. **Dependency is `workers.length` only.** If a worker is *renamed* or the
   list reshuffles without changing length, `initAgents` never re-runs, so
   late-added slugs (e.g. a new `worker-cleanup`) never get registered. Log
   line 829 shows a brand-new `qc-agent` schedule starting mid-session at
   11:27:13; if the worker list length didn't change, its card was never
   created.

## Log evidence — events ARE firing
The Tauri `event.emit` calls don't appear in `tracing` log output (they go
through Tauri's IPC bus, not `info!`), so the log file shows zero
`worker:tick-start` strings. But the *causes* are all there:

- 11:14:13.548 — first Mode 2 dispatch for `qc-agent` (line 53). Rust
  `emit_worker_tick_start` fires at this point per `runner.rs:201`.
- 11:14:13.385 — supervisor STARTUP tick (line 39), same path.
- 11:19:54.139 — kick-fire for `qc-agent` (line 492), then a 5-minute tick
  that ended at 11:24:58.797 (line 692). During that window the card
  should have been continuously `active`.

The user reported the Live Monitor staying `IDLE` during this whole 5-minute
QC tick — consistent with the events arriving *before* the slug was
registered in the store, then being silently dropped.

## Suggested fix (no code change in this audit)
Two complementary fixes; do both:

1. **Auto-register unknown agents inside the handlers.** When
   `handleStatusChange` / `handleToolCallStart` etc. see an `agent_id` that
   isn't in `state.agents`, lazily insert a `makeInitialAgent(agent_id)` and
   continue — instead of `return state`. This makes the store robust to
   event-before-init regardless of mount order.

2. **Move `initAgents` out of `AgentRoster`** and call it once in
   `AutoDevTab`'s top-level effect, immediately after `loadWorkers()`
   resolves, with `await`. Also change the dep array from
   `[workers.length]` to something that detects slug-set changes
   (e.g. `workers.map(w=>w.slug).join(',')`).

---

## Atomic implementation tasks

### Phase 1: Lazy slug registration in `agentLiveStore`

- [ ] Replace silent `if (!agent) return state` with lazy-create — `src/stores/agentLiveStore.ts:200-210` (`handleStatusChange`) — when `state.agents[agent_id]` is undefined, call `makeInitialAgent(agent_id)` and merge into `state.agents`, then apply the status update on the freshly created record instead of returning `state` unchanged.
- [ ] Apply the same lazy-create pattern to `handleToolCallStart` — `src/stores/agentLiveStore.ts` (tool-call-start handler) — auto-insert the agent record on first event so a `worker:tool-call-start` arriving before `initAgents()` is not dropped.
- [ ] Apply the same lazy-create pattern to `handleToolCallResult` — `src/stores/agentLiveStore.ts` (tool-call-result handler) — same auto-insert behaviour to keep paired start/result events consistent.
- [ ] Move `initAgents()` invocation out of `AgentRoster` — `src/tabs/AutoDevPanel.tsx:105-108` — remove the local `useEffect` and call `initAgents()` once at the top-level mount (either in `App.tsx` or in `AutoDevPanel`'s root effect) immediately after `workerStore.loadWorkers()` resolves.
- [ ] Update the dep array to detect slug-set changes — `src/tabs/AutoDevPanel.tsx` (the new top-level effect) — use `workers.map(w => w.slug).sort().join(',')` instead of `workers.length` so late-added or renamed workers re-register.

### Phase 2: Bridge Mode 2 tool-call events

> See also [02-dashboard-not-live.md](02-dashboard-not-live.md) — same upstream issue (Mode 2 LivingAgent ticks do not emit `agent:tool-call-*` events to the frontend).

- [ ] Add a `worker:tool-call-start` Tauri event listener — `src/stores/agentLiveStore.ts` (`subscribeToEvents` near line 235) — translate Mode 2 SSE tool events (already pumped by `SubagentManager` via `events.rs:85 EVT_SUBAGENT_EVENT`) into the same `handleToolCallStart` path used by Mode 1.
- [ ] Add a paired `worker:tool-call-result` listener — `src/stores/agentLiveStore.ts` (`subscribeToEvents`) — mirror the start listener so result/error frames update `current_tool` and append `log_lines`.
- [ ] Verify the Rust-side bridge exists — `src-tauri/src/events.rs` and `src-tauri/src/sidecar/subagent_manager.rs` — confirm `EVT_SUBAGENT_EVENT` is re-emitted as `worker:tool-call-start` / `worker:tool-call-result` with the slug as `agent_id`; if not, add the re-emit (read-only inspection first, then a thin adapter file in the events module).

### Phase 3: UI polish

- [ ] Show the worker model (haiku / sonnet) on each card — `src/components/autodev/AgentRosterCard.tsx` (or wherever the card body renders) — read `worker.model` from `workerStore.workers` and render a small badge next to the slug.
- [ ] Show last 3 tool names instead of "no activity yet" — `src/components/autodev/AgentRosterCard.tsx` — derive a rolling `recentTools: string[]` from `agent.log_lines` (or add a dedicated `recent_tools` field to `AgentLiveState`) and render the tail when no current tool is in flight.
- [ ] Distinguish "intentionally idle" from "never received an event" — `src/components/autodev/AgentRosterCard.tsx` — gray dot vs amber dot depending on whether the agent has ever transitioned past `unknown`.
