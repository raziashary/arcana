# P4-50 / TG4-24 — a missing log sink is visible, not silent — 2026-10-20

Closes the **received finding** recorded (not fixed) in
`_verify/TG4-24-TELEMETRY-FAILURE-ISOLATION-2026-10-18.md`:

> when the log directory cannot be prepared, logging degrades to console-only with no writer and
> therefore no failure counter, so "no sink" is indistinguishable from "no activity" in the Logs UI

Owner ruling on mechanism: the missing sink is process health, not a structured event
(`ADR-LOG-RESTRUCT-0404`). Emitting an event about the missing event stream is circular, and the
event could not belong to the day it describes. No new Tauri command, no schema change, no new
dependency.

## The defect had two halves, and both are fixed

1. **Producer.** `init_with_filter`'s console-only early return built `LoggingGuards` with a
   *fresh* `LoggingHealth::default()`, so the process-wide record the Logs reader consumes
   (`process_health_snapshot()`) never learned that the sink was missing. `console_only_health()` is
   now the single place that marks `file_logging_unavailable`, and it is the exact helper the early
   return uses — the reader and the producer share one path rather than two copies.
2. **Transport + consumer.** `ObservabilityMetrics` now carries `file_logging_unavailable` through
   `apply_writer_health`, next to the writer-failure counters it already carried. The frontend had a
   second defect of its own: `loadLog` (the only caller of `summarize_observability_events`) runs
   only once `selectedDate` is set, so the "no log directory" scenario — zero log files — never
   fetched host health at all. `loadDates` now reads host health when the file list is empty.

The UI states are now distinct: **"File logging unavailable in this process — console only. Empty
sections below mean no file sink exists, not that nothing happened."** (with `role="status"`)
versus the previous, now conditional, **"No log files found — restart the app to generate today's
log."** The metrics footer also names the state.

## Falsification (each link fails when its own claim is weakened)

| Probe | Result |
|---|---|
| Remove `mark_file_logging_unavailable()` from `console_only_health()` | `console_only_logging_reports_an_unavailable_file_sink_to_the_logs_reader` failed on the guards assertion (line 707). Probe reverted. |
| Drop `metrics.file_logging_unavailable = health.file_logging_unavailable` from `apply_writer_health` | `metrics_payload_carries_the_missing_file_sink_state` failed with `assertion failed: metrics.file_logging_unavailable`. Probe reverted. |
| Revert the header to the old `dates.length === 0` fallback (the defect state) | Vitest: 1 failed / 21 passed — *"Unable to find role=status"* — while the companion "keeps the restart advice" test still passed, so the new test detects the removal without being a blanket assertion. Probe reverted. |
| Make the banner unconditional (`true ?`) | Vitest: 2 failed / 20 passed — *"Unable to find an element with the text: /No log files found/"* — proving the second test catches over-showing, not merely under-showing. Probe reverted. |

`git diff` after revert contains no probe remnants (`FALSIFICATION` grep is empty).

## Test runs (all re-run in the main session, not taken from a report)

- `npx vitest run src/tabs/Logs.test.tsx` → **22/22** (includes the two new cases).
- `npm test` (full Vitest) → **14 files / 61 tests passed**, exit 0.
- `npm run typecheck` → exit 0.
- `npx eslint src/tabs/Logs.tsx src/tabs/Logs.test.tsx` → exit 0.
- `cargo test -p arcana-core --lib --locked -- observability:: --test-threads=1` → **32/32**.
- `cargo test -p arcana --lib --locked -- commands::log_commands:: --test-threads=1` → **42/42**.
- `rustfmt --check` clean on both touched Rust files (the repo-wide `cargo fmt --check` still
  reports only the owner's WIP `process_sandbox/linux_smoke/staging.rs`).

## Deliberately not claimed

- The state is **process-lifetime**; it is not persisted across restarts, so a reader cannot audit
  a past window in which the sink was missing. That is the same scope as the existing
  writer-failure counters, and a persisted host-health record is not part of TG4-24's text.
- No runtime log-level change, no size-based rotation, no external metrics exporter.
- **The rest of TG4-24 stays open** under P4-50: injection through a real Fleet run, aggregated
  trace durations in the Traces view, remaining cost-measurement groups, offline CLI
  memory/supervisor-lifecycle readback, and the owner-authorized live-provider milestone
  (`GOAL.md`: no second paid provider run is authorized, so that item is not the assistant's to
  close).

## Gate mapping

- **P4-50** — stays `PARTIAL`; the row's evidence now names this defect as fixed and the residue
  above as open.
- **TG4-24** — stays open; this closes the gate's "empty/error states remain visible" and
  "inject telemetry persistence/export failures" clauses for the **no-sink** case, at the layer
  that owns the state.
