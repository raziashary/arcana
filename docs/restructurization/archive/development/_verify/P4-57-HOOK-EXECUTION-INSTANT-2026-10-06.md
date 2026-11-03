# P4-57 Hook Execution Instant — 2026-10-06

Status: scoped implementation and main-session Windows/Linux validation complete. TG4-19 remains open for the other production pairs and complete platform proof.

## Evidence log

- Report created before source investigation, per task instructions.

## Source and decision findings

- MDP P4-57 and TG4-19 require the represented written instant to survive its production reader over the annual/leap-day/local-midnight/DST corpus, while retaining legacy readability. TG4-19 remains open; this task does not update gate status.
- ADR-LOG-RESTRUCT-0320 records the owner approval: `HooksDb::insert_log` must validate and canonically persist `HookExecutionLog.executed_at`, reject malformed values before insert, use the shared parser/formatter, preserve schema and legacy readers, and extend actual writer/reader tests. This task implements that approved contract in the assigned source file.
- `HookExecutor::execute` supplies `now_rfc3339()`. `parse_db_timestamp` accepts offset-bearing RFC3339 and legacy SQLite timestamp strings, returning `DateTime<Utc>`; `format_utc_timestamp` writes fixed-millisecond RFC3339 UTC with `Z`. Before this change, `insert_log` omitted the field and the schema default supplied insertion time. `get_logs` and `get_logs_since` return stored raw timestamp strings; the latter also refuses invalid rows.
- Existing timestamp tests resided in `analytics_timestamp_pair_tests`, `observability_tests`, and `retention_tests`; there was no existing `hooks::db::tests` module. New focused tests are under `mod tests` so the coordinator-prescribed filter selects them. The existing observability writer test now supplies and checks an explicit valid instant.

## Implementation and proof added

- `HooksDb::insert_log` now parses `HookExecutionLog.executed_at` with `parse_db_timestamp`, formats the represented instant with `format_utc_timestamp`, and includes the canonical value in the insert. Parse failure returns before SQLite is called. No schema or API change was made.
- The annual `analytics_timestamp_pairs_hook_writer_and_readers_cover_full_leap_year_and_dst` test now sends all 1,106 leap-year/midnight/DST instants through the actual `insert_log` writer and `get_logs_since` reader, checks each persisted value is canonical and instant-equal, and retains an offset-less legacy row readback.
- `hook_execution_instant_survives_delayed_persistence_exactly` waits before production insertion and checks a historical leap-day instant survives through `get_logs`. `hook_execution_log_rejects_malformed_timestamp_without_inserting` proves malformed input returns an error with no row added.
- The existing observability writer test now supplies a valid canonical timestamp and asserts the reader returns that exact supplied instant, while retaining its injected database-write failure coverage.

## Validation status

- `cargo fmt --all -- --check` passed.
- `cargo test --offline -p arcana-core --lib hooks::db::tests --locked -- --test-threads=1` passed 3/3 (annual writer/reader, delayed exact instant, malformed no-row), 1.05s test time. All three use tempfile-backed production config databases. Compilation emitted 43 existing warnings; no failures.
- The edited `observability_tests::hook_configuration_and_execution_log_operations_emit_private_result_spans` was not selected by the coordinator-prescribed test filter and was not run separately. It now supplies a valid canonical timestamp and checks exact reader equality; root may include it in broader integration validation.
- No schema/API changes, tests outside the authorized filter, gate closure, ADR update, or Git operation was performed.

## Main-session integration

The public method signature and schema remain unchanged; its owner-approved behavior now preserves execution time. The shared parser's error text can echo malformed input, so the insertion boundary returns a fixed error instead. A planted invalid timestamp must be absent from the error and create no row. This is a privacy assertion, not only validation presence.

`cargo test --offline -p arcana-core --lib hooks::db:: --locked -- --test-threads=1` passes **11/11 on Windows (7.13s) and Ubuntu WSL (17.29s)**. This includes the edited observability test, portability and retention tests, and all new timestamp cases. `cargo check --offline -p arcana-core --locked` passes. The real CLI subprocess smoke `cargo test --offline -p arcanactl --test analytics_export --locked -- --test-threads=1` passes **1/1 on Windows (1.77s)** with the actual hook writer and retained export reader.

The existing three-host CI matrix now selects the four `analytics_timestamp_pairs_` cases before the independently failing command corpus. No macOS result for this new writer change is claimed yet. Safe raw logs: `target/p4-ci-checks/hook-instant-{windows,linux}-2026-10-06.log`, `hook-instant-check-2026-10-06.log`, `hook-instant-cli-export-2026-10-06.log`. No live provider, owner Calendar, or user database was used. P4 remains 40/59; TG4 19/33.
