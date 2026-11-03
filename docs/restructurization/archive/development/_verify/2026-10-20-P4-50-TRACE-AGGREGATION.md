# TG4-24 / P4-50 — trace durations aggregate in the view — 2026-10-20

Closes the residue named in `_verify/TG4-24-EVENT-FAMILIES-2026-10-18.md` and the gate's own
status: **"trace durations aggregated in the view."**

Frontend-only change. No producer, reader, schema or export change: the aggregation is derived from
the persisted records the Traces section already reads, so exports keep the raw record set.

## What was actually missing

The Traces section rendered each span node with its own timed events, but nothing summed a node's
subtree. A reader could see `provider_call 2.00 ms` and `tool_call 6.00 ms` in two separate rows and
had no way to see that the trace took 1 s wall-clock or which single operation was slowest. The
gate asks for "correlated parent/child traces with timed sub-operations"; the correlation and the
per-operation timing existed, the aggregate did not.

## Decision (ADR-LOG-RESTRUCT-0405)

A node's summary now shows, when it holds more than one recorded operation:

`<name> · N ops · X ms window · slowest Y ms`

- **window** is wall-clock: `max(timestamp) − min(timestamp)` across the subtree. It is deliberately
  **not** a sum of durations: a parent span's duration already covers its children, so summing
  parent + child double-counts nested work. The falsification below proves a sum would have been a
  different, wrong number.
- **slowest** is the maximum single measured duration. Zero-duration samples stay excluded, matching
  the metrics section's "below timer resolution" convention.
- A node with exactly one operation shows no aggregate — the row's own duration already says
  everything.

`buildTraceTree` gives repeated same-named spans distinct nodes unless the producer supplies a span
id (`spans[].span_id`), so sibling calls from one task stay separate; with span ids present, the
subtree aggregate is the real parent/child roll-up. Both shapes are covered by the test fixture.

## Falsification

| Probe | Result |
|---|---|
| `collectTraceRecords` returns only `node.events` (no subtree) | Test failed on `expect(rootDetails).toHaveTextContent('3 ops')` — the aggregate disappeared. Probe reverted. |
| `windowMs` computed as the sum of durations instead of timestamp distance | Test failed on `1000.00 ms window` — the sum cannot stand in for the window. Probe reverted. |

`git diff` after revert contains no probe remnants.

## Test runs (re-run in the main session)

- `npx vitest run src/tabs/Logs.test.tsx` → **23/23** (one new case).
- `npm test` (full Vitest) → **14 files / 62 tests passed**, exit 0.
- `npm run typecheck` → exit 0; `npx eslint src/tabs/Logs.tsx src/tabs/Logs.test.tsx` → exit 0.

## Deliberately not claimed

- The aggregation lives in the view. `read_structured_events` / the observability bundle still return
  the complete raw records; no aggregate is persisted or exported, so a consumer that wants a
  different roll-up (for example per-provider p95) is not forced through this one.
- No span-id generation was added on the Rust side. Where the producer does not supply
  `span_id`, sibling spans remain separate nodes by design, and the aggregate at those nodes covers
  one operation only.

## Gate mapping

- **TG4-24** — the "trace durations aggregated in the view" residue is closed; the gate stays open
  for the remaining named items (Fleet-run failure injection, cost-measurement groups, offline CLI
  memory/lifecycle readback, owner-authorized live provider).
- **P4-50** — stays `PARTIAL`; evidence block updated.
