# TG4-24 / P4-50 — producer cost measured, and queue loss made visible — 2026-10-20

Closes the gate's **remaining cost-measurement group** ("measure producer, database
instrumentation, refresh and export cost on reproducible fixtures") and the loss-visibility gap the
fixture exposed.

## The measurement

`observability::logging::tests::measure_structured_producer_cost_on_a_reproducible_fixture` runs
20,000 structured events through the **shipped** stack — `RedactingJsonWriter` → `ObservedWriter` →
the default `tracing_appender::non_blocking` queue → buffered file — and compares it with the same
JSON formatting into a discarded sink. Every hundredth event carries a planted credential so the
redaction work is inside the measured path.

This host, debug profile (unoptimized), Windows:

| Quantity | Value |
|---|---|
| producer total (caller thread) | 535,135,800 ns |
| producer per event | **26,756 ns** (26.8 µs) |
| sink baseline total (formatting only) | 497,597,200 ns |
| sink baseline per event | **24,879 ns** (24.9 µs) |
| worker drain total | 5,526,379,500 ns |
| persisted bytes | 6,268,894 |
| dropped fragments | **0** |
| persisted records | **20,000 / 20,000** |

Reading: on the **producer** thread the shipped stack costs ~1.9 µs/event more than raw JSON
formatting into a sink — the queue send and the redaction buffer, not file I/O. Parsing, redaction
and writing happen on the worker thread, which is what the 5.5 s drain shows. Debug-profile numbers
are not a production latency target; the only assertion is a 250 µs/event ceiling (>9× headroom)
that exists to catch a synchronous-flush regression.

The same fixture therefore also answers the gate's large-volume question: a 20k-event burst loses
nothing and fits the shipped queue.

## The retraction

The first version of this fixture read the file immediately after `WorkerGuard::drop` and saw 5,226
of 20,000 lines. That is **not** telemetry loss: `WorkerGuard::drop` waits only 100 ms to enqueue
shutdown and 1 s for the worker, so the read raced the writer. Polling for the drain, and reading
the queue's own `dropped_lines()` counter, showed 20,000/20,000 with zero drops. The fixture now
waits and asserts both, and its doc comment records the trap.

## The real gap: drops were counted but never reported

`tracing_appender`'s lossy queue discards a fragment at capacity, returns `Ok(len)`, and increments
an `ErrorCounter` that Arcana never read. So a genuine overflow produced exactly the ambiguity the
missing-sink fix removed one layer in: a healthy-looking host with missing records.

`LoggingHealth` now adopts the two queues' counters at installation (`install_drop_counters`) and
reports `readable_dropped_lines` / `structured_dropped_lines` in `LoggingHealthSnapshot`. They ride
`ObservabilityMetrics` (process lifetime, like the failure counters) and the Logs header shows
**"N telemetry lines dropped by the writer queue — this view is missing records."** when nonzero.
Drops stay distinct from write failures, because a dropped fragment never reaches the writer.

The queue stays **lossy**: the logging worker must not be able to block application work, which is
the `LoggingHealth` contract. No capacity, backpressure or retention change.

## Falsification (each claim fails when its own mechanism is removed)

| Probe | Result |
|---|---|
| `install_drop_counters` stops adopting the counters | `dropped_telemetry_is_reported_through_the_health_record` failed: snapshot reported **0** against **63** real drops. Reverted. |
| `apply_writer_health` writes a literal `0` for `structured_dropped_lines` | `metrics_payload_carries_the_missing_file_sink_state` failed (left 0, right 7). Reverted. |
| Header warning made unconditional (`droppedLines >= 0`) | 3 Vitest failures — including the "quiet host" case asserting no warning. Reverted. |
| Header warning removed (`false &&`) | The new "warns that records are missing" Vitest case failed. Reverted. |

`git diff` contains no probe remnants.

## Test runs (re-run in the main session)

- `cargo test -p arcana-core --lib --locked -- --test-threads=1 observability::` → **34/34**.
- `cargo test -p arcana --lib --locked -- --test-threads=1 commands::log_commands::` → **42/42**.
- `npx vitest run src/tabs/Logs.test.tsx` → **24/24**; `npm test` → **14 files / 63 tests**.
- `npm run typecheck` → exit 0; `npx eslint` on both touched TS files → exit 0;
  `rustfmt --check` on both touched Rust files → clean.

## Deliberately not claimed

- `init_with_filter`'s single call to `install_drop_counters` is not exercised end to end: a
  process can install a global subscriber once, so a test cannot re-init it. The adoption mechanism
  itself is proven against a deliberately overfilled queue; the call site is one visible line.
- No production-profile numbers, no percentile distribution, no cross-host baseline.
- Cost of the **console** layer and of the readable stream under load is not separately measured.
- TG4-24 stays open for Fleet-run failure injection, offline CLI memory/supervisor-lifecycle
  readback, and the owner-authorized live-provider milestone.

## Gate mapping

- **TG4-24** — the "producer" cost group is now measured on a reproducible fixture against a
  recorded baseline, and telemetry loss is visible; the gate remains open for the three named items
  above.
- **P4-50** — stays `PARTIAL`; evidence block updated.
