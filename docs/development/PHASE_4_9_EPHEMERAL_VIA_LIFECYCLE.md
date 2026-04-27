# Phase 4.9 — Architectural Deviation: Ephemeral Subagents via Lifecycle

**Status**: Approved
**Date**: 2026-04-27
**Phase**: 4.9 (Subagent System)
**Affects**: `src-tauri/src/subagent/*`, `src-tauri/src/sidecar/client.rs`, frontend subagent UI

---

## Summary

Phase 4.9 in `DEVELOPMENT_PLAN.md` originally specified four endpoints on
core-orchestrator that **do not exist**:

| Spec'd endpoint            | Reality                                       |
|----------------------------|-----------------------------------------------|
| `POST /api/agents/create`  | `POST /api/agents` (no `/create`)             |
| `POST /api/agents/{id}/process` | `POST /api/agents/{id}/invoke[/stream]`  |
| `POST /api/agents/{id}/terminate` | **Does not exist** — only `DELETE /api/agents/{id}` (wipes YAML) |
| `GET /api/agents/{id}/events` | **Does not exist** — SSE is on `/invoke/stream` |

Additionally, core-orchestrator's `/api/agents` is designed for **persistent
personas** (PA, Supervisor, Worker, QC), not ephemeral subagents:

- Creating an agent writes a YAML file to `config/agents/{slug}.yaml`
- Agent IDs are slugified persona names — duplicate slugs are rejected
- `DELETE /api/agents/{id}` permanently removes the YAML config + registry entry

The original Phase 4.9 vision (lightweight, transient, in-memory subagents) is
incompatible with this contract.

---

## Decision

**Use the existing `Create → Invoke(stream) → Delete` lifecycle as the
ephemerality boundary**, with no changes to core-orchestrator.

```
┌───────────────────────────────────────────────────────────────────┐
│  Spawn subagent                                                   │
│    1. Generate unique slug: subagent-{parent}-{millis}-{seq}      │
│    2. POST /api/agents          → creates YAML + LivingAgent      │
│    3. POST /api/agents/{id}/invoke/stream → SSE event stream      │
│    4. Consume events, accumulate result                           │
│    5. DELETE /api/agents/{id}   → wipes YAML + registry entry     │
│  Subagent is gone                                                 │
└───────────────────────────────────────────────────────────────────┘
```

Each subagent is a **real, fresh `LivingAgent`** with full memory consciousness
(MemoryCore, FSMs, A2A protocol) — exactly what the Phase 4.9 vision wanted —
but its existence is bounded by the subtask's lifetime.

---

## Why This Works

| Original Phase 4.9 goal           | Achieved by lifecycle approach              |
|-----------------------------------|---------------------------------------------|
| Isolated context per subagent     | ✅ Fresh LivingAgent = fresh MemoryCore + session |
| Custom system prompt + tool subset | ✅ `AgentCreate` body controls both        |
| Parent blocks for summary          | ✅ SSE stream terminates with final message |
| Parallel execution                 | ✅ Each spawn = unique slug (no contention) |
| No core-orchestrator changes       | ✅ Uses existing 3 endpoints                |
| Real LivingAgent capabilities      | ✅ Inherited "for free" via the `AgentFactory` |

---

## Slug Convention

```
subagent-{parent_slug}-{epoch_millis}-{seq}
```

**Example**: `subagent-worker-backend-1745740800123-1`

- `parent_slug` — traceability in logs/UI/lineage
- `epoch_millis` — uniqueness across process restarts
- `seq` — atomic counter for same-millisecond parallel spawns

This pattern is reserved by convention. The `subagent-*` prefix lets cleanup
logic distinguish ephemeral subagents from persistent fleet agents.

---

## Trade-offs

### Costs

1. **YAML file I/O per spawn** — every subagent writes/deletes a config file
   (~10-100ms). Negligible against typical LLM inference time (5-60s).

2. **Orphan risk on parent crash** — if the Rust process panics between
   `create` and `delete`, the YAML config + LivingAgent persist.
   **Mitigation**: SidecarManager scans `config/agents/subagent-*.yaml` on
   startup and deletes orphans before booting core-orchestrator.

3. **Memory MCP records may persist** — episodic/semantic memories written
   during the subtask remain in mem0/Redis after deletion. This is currently
   considered a **feature** (parent can query subagent memories post-hoc) but
   may be revisited if it causes clutter. Tagged with the subagent's
   ephemeral session_id for future filtering.

### Benefits

1. **Zero core-orchestrator changes** — fully unblocks Phase 4.9
2. **No special-casing** — uses the same code paths fleet workers already use
3. **Battle-tested** — `OrchestratorClient.find_or_create_agent` proves the
   contract works in production (used by the worker fleet)
4. **RAII safety** — `SubagentHandle` implements `Drop` for best-effort
   cleanup on early returns / panics

---

## Resilience: Three Layers of Cleanup

```
Layer 1: Happy path
  └── Bridge::run() calls DELETE after stream completes
      ├── Success → YAML + LivingAgent removed
      └── Failure → Layer 2 kicks in

Layer 2: RAII Drop
  └── SubagentHandle::Drop spawns fire-and-forget DELETE
      ├── Catches panics, early returns, ?-operator bailouts
      └── If process is alive → cleanup happens
      └── If process crashes → Layer 3 kicks in

Layer 3: Startup orphan sweep
  └── SidecarManager startup deletes all subagent-*.yaml files
      ├── Runs BEFORE core-orchestrator boots
      └── Catches everything missed by layers 1 + 2
```

---

## API Surface (AutoDev Side)

`OrchestratorClient` extensions (in `src-tauri/src/sidecar/client.rs`):

```rust
/// DELETE /api/agents/{id} — terminate ephemeral subagent
pub async fn delete_agent(&self, agent_id: &str) -> Result<()>;

/// POST /api/agents/{id}/invoke/stream — SSE stream of agent events
pub async fn invoke_agent_stream(
    &self,
    agent_id: &str,
    prompt: &str,
) -> Result<mpsc::Receiver<SubagentEvent>>;
```

`SubagentBridge` (in `src-tauri/src/subagent/bridge.rs`):

```rust
/// Full lifecycle: create + invoke-stream + delete
pub async fn run(
    &self,
    config: SubagentConfig,
    prompt: String,
) -> Result<SubagentResult>;
```

---

## What Stays the Same

- All AutoDev-side data structures (`SubagentConfig`, `SubagentHandle`,
  `SubagentStatus`, `SubagentResult`) — these are unchanged from the spec
- Tauri command surface (`spawn_subagent`, `execute_subtask`, etc.)
- Built-in tools (`spawn_subagent`, `spawn_parallel_subagents`)
- Frontend UI (`SubagentMonitor`, `SubagentEventStream`, `SubagentResultCard`)
- Concurrency limits, nesting depth caps, timeouts

The bridge layer is the only place that diverges from the spec, and it
diverges in a way the spec writer would have chosen if they'd known the
real API surface.

---

## Future Migration Path

If core-orchestrator's rewrite later adds **truly transient** in-memory
agents (no YAML write), `SubagentBridge` can be swapped to use them with
**zero changes** to the AutoDev-side `subagent/manager.rs`, tool layer,
or frontend. The bridge is the boundary; everything above it is stable.
