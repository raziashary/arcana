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

### Phase 1: Lazy slug registration in `agentLiveStore` — ✅ COMPLETE 14 May 2026

- [x] Replaced silent `if (!agent) return state` with lazy-create in all three handlers — `src/stores/agentLiveStore.ts` `handleStatusChange` / `handleToolCallStart` / `handleToolCallResult`. Each now reads `state.agents[agent_id] ?? makeInitialAgent(agent_id)` and applies its update on the (possibly fresh) record. STARTUP `worker:tick-start` events are no longer dropped.
- [x] Moved `initAgents()` invocation out of `AgentRoster` to top-level `AutoDevTab` effect — `src/tabs/AutoDevPanel.tsx`. The deep-mount race with `subscribeToEvents()` is gone.
- [x] Dep array uses a stable slug-set hash — `src/tabs/AutoDevPanel.tsx` `workerSlugKey = workers.map(w=>w.slug).sort().join(',')` — late-added or renamed workers re-seed.

### Phase 2: Bridge Mode 2 tool-call events — ✅ COMPLETE 14 May 2026

> See also [02-dashboard-not-live.md](02-dashboard-not-live.md) — `worker:tick-end` listener was added there too, which refreshes the Dashboard on every Mode 2 tick.

- [x] Verified the Rust-side bridge — `src-tauri/src/subagent/manager.rs:277` already pumps every `SubagentEvent` to the `subagent:event` channel via `emit_subagent_event_global`. No Rust adapter was added because the same effect is achievable with one frontend listener — keeps the bridge in one place.
- [x] Subscribed to `subagent:event` in `agentLiveStore` — `src/stores/agentLiveStore.ts` `subscribeToEvents`. Translates `kind: "tool_start" | "tool_end" | "status" | "error"` into the existing handlers using the subagent slug as `agent_id`. Tool-call result frames pair to the most recent matching `running` log line by `tool_name`.
- [x] Forward-work note: this covers *explicitly spawned subagents*. Mode 2 main-worker tool streaming would require switching `OrchestratorClient::invoke_worker` to SSE — flagged as Phase 3+ in the index.

### Phase 3: UI polish — ✅ COMPLETE 14 May 2026

- [x] Worker model badge on each card — `src/components/autodev/AgentLiveCard.tsx`. A `shortModelName()` helper compresses registry keys (`claude-haiku-4-5`, `claude-sonnet-4-5-thinking`, `deepseek-v4-pro` …) to a 5–8-char chip (`haiku` / `sonnet` / `sonnet⊕` / `deepseek`). Full model string lives in the `title` attribute for hover. Wiring: `AutoDevTab` calls a new `agentLiveStore.setAgentModel(slug, model)` action for every worker on slug/model change (dep: `workerModelKey`), so re-binding a worker to a new model updates the chip live without a full re-render.
- [x] Recent-tools tail when nothing is in flight — `src/components/autodev/AgentLiveCard.tsx`. New `recentTools` memo walks `agent.log_lines` backwards collecting up to 3 *distinct* tool names. Rendered as compact chips below the current-action row only when `current_tool` is null AND the tail is non-empty — keeps freshly-mounted cards genuinely empty (matched to the "never seen alive" dot below) and post-tick cards informative.
- [x] Gray vs blue dot for unknown vs intentionally-idle — `src/types/live.ts` + `src/stores/agentLiveStore.ts` + `src/components/autodev/AgentLiveCard.tsx`. New `has_ever_been_active: boolean` flag on `AgentLiveState` flips to `true` the first time any handler observes real activity (`handleToolCallStart`, or `handleStatusChange` to `'active' | 'starting'`). Card's dot uses `UNKNOWN_DOT_COLOR = '#555555'` (gray) when `status === 'idle' && !has_ever_been_active`, else the existing palette. Combined with Doc 06 Phase 3's "idle — no qc_review tasks" breadcrumb, "stuck idle" and "intentionally idle" are now visually distinct.
