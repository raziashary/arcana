# TG4-24 / P4-50 — offline CLI memory and supervisor/worker lifecycle readback — 2026-10-20

Closes the last offline-Fleet residue item of P4-50: the run's **readback** now covers memory
operations and the supervisor/worker lifecycle, not only tasks, build output and worker stats.

## What was missing

The offline success scenario completed a board child through a sandboxed Cargo build, then the CLI
export was asserted to contain the completion event, the persisted epic/child and worker stats. Its
scripted worker never called a memory tool, so no memory operation — write, retrieval, statement
timing, database identity — could appear in any readback, and nothing checked the durable
supervisor/worker run records.

## What changed

**Script (feature-gated, `arcana-core/src/runtime.rs`).** Between `claim_task` and `project_command`
the scripted worker now:

1. `propose_memory` — one real, checkable fact (`OFFLINE_FLEET_MEMORY_FACT`), scope `project`,
   confidence 0.5, attributed to `fleet-fixture/Cargo.toml:1`;
2. `search_memory` — a query for that fact (limit 5).

**Readback (`src-cli/tests/fleet_offline.rs`, success scenario).** The exported logs must contain
`memory_proposal` and `memory_search_all`; at least one `memory_proposal` line must carry a
`"database":"memory"` `"operation":"insert"` child statement; the proposed fact's exact text must be
**absent** from the export; and the run ledger (`RunsDb::open_at`) must hold a `supervisor` tick with
an outcome and a `worker-backend` tick whose `task_id` is the consumed child and which records an
outcome.

Observed shape of the exported detail (abridged, from a real run):

```
{"fields":{"database":"memory","operation":"insert","outcome":"duration_observed", ...},
 "span":{"name":"propose","operation":"memory_proposal"},
 "spans":[{"name":"run_recipe","fleet_correlation_id":"30eae101-…"}, …]}
```

That is the gate's correlation chain — recipe span → `memory_proposal` span → statement record — with
database identity and duration, and no SQL text, parameters or memory contents.

## Embedding safety

`embeddings` is off by default and the CLI does not enable it, so `search_memory` uses the lexical
path. This matters because `memory::embed::download_policy()` defaults to **Allowed**; the reason no
133 MB model download can happen in this test is the feature gate, not a setting. The run therefore
exercises the degraded/fallback retrieval the gate names, not the vector path.

## Falsification (each mechanism removed, then reverted)

| Probe | Result |
|---|---|
| Both memory steps disabled (`if false && …`) | Failed: "CLI log export must read the memory gateway and retrieval the worker performed". |
| Expected worker `task_id` set to `children[0].id + 1` | Failed: `left: Some(2) right: Some(3)` — the ledger really carries the consumed child. |
| Supervisor outcome assertion negated | Failed: "a finished supervisor tick must record an outcome, not only a heartbeat". |

The privacy assertion is an absence check, so it was made non-vacuous from the other side: the same
export must show the `memory_proposal` **insert**. Content-free exists because content was written,
not because nothing happened.

## Test runs (re-run in the main session)

- `cargo test -p arcanactl --features offline-fleet-test --test fleet_offline --locked --
  --test-threads=1` → **7/7 passed** (41.4 s).
- `rustfmt --check` on `runtime.rs` and `fleet_offline.rs` → clean; `cargo check -p arcanactl`
  (without the feature) → clean.

## Deliberately not claimed

- The vector/embedding path is not exercised; "embedding" coverage remains unit-level.
- Memory contents are proven absent from telemetry, not encrypted at rest.
- Still an offline scenario: no provider request, so nothing here closes P4-49 or the live-provider
  milestone.
- The scripted worker is harness code, not a claim that a model would choose these tools.

## Gate mapping

- **TG4-24** — memory write/retrieval, memory-database statement timing with identity, correlated
  parent/child detail, content-free telemetry and supervisor/worker lifecycle readback are now
  demonstrated from one real CLI run. The gate remains open only for the owner-authorized
  live-provider milestone.
- **P4-50** — stays `PARTIAL`: production observability is proven offline end to end, but the
  live-provider clause cannot be closed by the assistant.
