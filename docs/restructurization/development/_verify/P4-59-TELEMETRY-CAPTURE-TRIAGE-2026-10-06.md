# P4-59 telemetry capture CI triage — 2026-10-06

## Scope and sequence

Read-only triage of the Ubuntu four-thread test failure `hook_retention_failure_is_visible_through_structured_logs_reader` at `src-tauri/src/commands/log_commands.rs:1548`, where the persisted test log lacks `database_hook_logs_retention_prune`. Allowed write: this report only. No tests/executions, Git, environment or credential inspection, user data, Calendar, or expanded-goal access.

This file is being created before investigation per the task instruction. Append evidence and conclusions here as they are gathered. Distinguish repository/CI evidence from hypotheses; do not change tests/runtime or declare a fix.

## Progress: source trace

- `HooksDb::prune_logs_before` constructs its `OperationTimer` inside the called method, then returns through transaction/validation errors or success (`arcana-core/src/hooks/db.rs`, `prune_logs_before`). The test separately confirms the returned error includes its private trigger marker, so the injected failure path is reached.
- `OperationTimer` defaults to `outcome = "error"`; `Drop` synchronously calls `tracing::info!` with fixed target `arcana::telemetry`, event type `operation_timing`, operation label, and database (`arcana-core/src/observability/timing.rs`, `Drop for OperationTimer`). The timer is method-local and drops before `prune_logs_before` returns into the `with_default` closure.
- The test's filter accepts exactly the telemetry target at INFO, and its subscriber is installed around the synchronous prune call (`src-tauri/src/commands/log_commands.rs`, `hook_retention_failure_is_visible_through_structured_logs_reader`). The analytics retention test immediately above uses the same writer/filter/subscriber pattern and checks the same persisted reader; it passes in the reported CI run.
- `read_full_log` is only `std::fs::read_to_string`; it adds no filtering, metrics cache, or truncation (`src-tauri/src/commands/log_commands.rs`, `read_full_log`). The supplied failed logs report the same missing-operation assertion; the latest fails at the persisted string assertion after the trigger error assertion has passed.

Evidence narrows the fault to trace dispatch/filtering or file-backed capture; the production prune failure path and returned error are exercised. Source evidence so far does not distinguish those capture stages.

## Findings and next check

### Confirmed

- In `src-tauri/src/commands/log_commands.rs:1508-1563`, the failing test is synchronous, and the assertion on the private SQLite trigger marker succeeds before it reads the file. The CI outputs identify only the absent operation label; the prune error was not bypassed.
- In `arcana-core/src/hooks/db.rs:178-206`, the operation is started within `prune_logs_before`; the injected DELETE failure returns with the `OperationTimer` still in scope. `Drop` (`arcana-core/src/observability/timing.rs:106-129`) emits the expected fixed target and operation synchronously. A timer lifetime extending past the subscriber scope is therefore not the cause.
- `read_full_log` (`src-tauri/src/commands/log_commands.rs:597-599`) is a plain file read. The adjacent analytics test at `:1458-1504` uses the identical cloned-File writer, INFO/target filter, and scoped Registry and succeeds in this same CI invocation. Other nearby production-consumer tests use `tracing_appender::non_blocking(file)` and explicitly drop the returned guard before reading (`:1920-1960`).
- Installed `tracing-core 0.1.36` `dispatcher::with_default` installs a scoped default for the closure (`dispatcher.rs:254-260`); installed `tracing-subscriber 0.3.23` writes the formatted event with `write_all` (`fmt/fmt_layer.rs:1050`). This supports scoped synchronous dispatch/write but does not identify why this one label is absent.

### Unresolved hypothesis and minimal falsifiable diagnostic

No material root cause is confirmed by source or the two CI failure excerpts. A file-writer/flush interaction is plausible but weak: this test uses a synchronous `File` sink, and the adjacent analytics test passes with the same pattern. A callsite/filter interaction specific to the hook path is also unproven; both methods share the same `OperationTimer::Drop` macro target and level.

The smallest discriminating test adjustment is to use the established `non_blocking(file)` plus guard pattern and explicitly drop the guard before `read_full_log`, while emitting one uniquely labeled `tracing::info!` sentinel with the same target and level inside the same `with_default` closure. Check sentinel and operation independently. If both are absent, investigate dispatch/filter or subscriber installation; if the sentinel is present but the operation is absent, investigate the timer's specific callsite/drop path; if both appear after the guarded writer change, the sink/drain handling is implicated. This is diagnostic only; do not change production telemetry without contrary evidence.

The guard-based sink matches existing tests in this file and ensures its writer worker has drained before the persisted-read assertion. It is the smallest bounded correction to try if the next CI rerun is intended to test writer handling, but it is not a proven fix because the current sink is synchronous. Retain the privacy checks and aggregate-reader assertions.
