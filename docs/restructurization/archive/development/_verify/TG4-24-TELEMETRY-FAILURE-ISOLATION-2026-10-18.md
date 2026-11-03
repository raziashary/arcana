# TG4-24 — telemetry persistence and export failure isolation

**Date:** 2026-10-18
**Commit:** `1e03e08` (tests), on top of `a2e7aa8`
**Gate clause closed:** "Inject telemetry persistence and export failures and prove core task
execution remains safe and its actual result is preserved." (TG4-24)
**Status:** clause proven at producer level. TG4-24 remains **open** for its other clauses.

## What was proven

Three tests, all inside existing `#[cfg(test)]` modules. No production behaviour was changed.

| Test | Location | Claim |
|---|---|---|
| `process_health_reports_injected_structured_write_failure_and_task_result_survives` | `arcana-core/src/observability/logging.rs` | A structured-telemetry write failure is counted and surfaced, while a real `TasksDb::create` returns its true success result and the row is durable across a reopen. |
| `unpreparable_log_directory_degrades_to_console_only_without_blocking_core_work` | `arcana-core/src/observability/logging.rs` | With no usable log directory there is no sink at all; core DB work still succeeds. Pins current behaviour, does not endorse it. |
| `observability_bundle_export_failure_is_reported_and_keeps_persisted_telemetry_readable` | `src-tauri/src/commands/log_commands.rs` | An unwritable export destination yields one bounded error naming the destination with no payload leak; every previously persisted record survives and still parses. |

## Production branches exercised

- Persistence: `logging.rs:113` (`PROCESS_HEALTH` handle), `:129-137` (shipped stack
  `RedactingJsonWriter -> ObservedWriter -> rolling appender`), `:228-248`
  (`ObservedWriter::write` increments `structured_write_failures` and returns `Err`).
- Reader the failure counter feeds: `src-tauri/src/commands/log_commands.rs:999-1001`.
- No-sink degradation: `logging.rs:102-111` (console-only early return), `:169-178`
  (`resolve_log_dir` returning `None`).
- Export: `log_commands.rs:1143-1199`, failure branch `:1191-1192`.

## Why the failures cannot harm core work (root cause reading)

Telemetry is fire-and-forget: `OperationTimer::drop` ends in a `tracing::info!` and discards
the writer result; no producer maps a sink failure to an error. The only loss signal is the
process-health counter. The tests therefore assert the *real* contract: the operation's own
result is unchanged and durable, and the loss is visible in the counter a reader consumes.

## Validation (re-run by the main session, not taken on a subagent report)

```
cargo test -p arcana-core --lib --locked -- --test-threads=1 observability::logging::tests::
  -> 12 passed; 0 failed; exit 0

cargo test -p arcana --lib --locked -- --test-threads=1 commands::log_commands::tests
  -> 41 passed; 0 failed; exit 0
```

Both files' diffs are pure additions inside their existing test modules (`git diff -U0`
shows one hunk each, starting inside `mod tests`).

## Received finding — escalated, not fixed

When the log directory cannot be prepared, `init` degrades to console-only: no writer is
installed, so no failure counter exists. The Logs reader reports `0` failures while structured
telemetry is completely absent — "no sink" and "no activity" are indistinguishable in the UI.
The only signal is an `eprintln!` (`logging.rs:172-176`). Fixing this needs a production
change (a counter independent of the writer, surfaced in health), which is outside a
test-repair scope. Recorded here and in ADR-LOG-RESTRUCT-0397.

## Not covered, stated rather than faked

- Injection through a live Fleet/CLI run — the gate text itself still lists this as open, and
  it needs the owner-authorized live-provider milestone.
- A mid-write failure (`ENOSPC`) that could leave a partial export file: the healthy-parent path
  fails before any byte is written, and a mid-write failure could not be injected in-process.
  Recorded as an unmeasured hypothesis, not a defect claim.
