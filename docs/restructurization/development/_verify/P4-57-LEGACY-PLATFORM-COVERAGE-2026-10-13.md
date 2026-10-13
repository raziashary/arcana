# P4-57 legacy and platform coverage (2026-10-13)

Status: source and CI edits are complete; Rust validation is coordinated with the root-owned Windows build. No annual test reruns are planned.

## Added legacy-reader proof

`arcana-core/src/db/test_clock/legacy_pairs.rs` adds three compact, test-only fixtures. Each seeds old SQLite timestamp spellings in a disposable file-backed database, closes it, reopens through the owning database API, then asserts that the intended public reader returns values accepted by `parse_db_timestamp`:

- `task_auxiliary_timestamp_readers_preserve_legacy_rows_after_reopen`: `get_worker_request_by_id` (`created_at`, `resolved_at`), `get_worker_message` (`created_at`, `read_at`), `get_routing_history` (`assigned_at`, `outcome_at`), `latest_build_evidence` (`created_at`), and `get_task_budget_resolution_history` (`resolved_at`). Actual APIs create the parent task, request/message/routing/build rows, and budget-resolution audit before timestamps are moved to legacy spellings in the isolated fixture.
- `run_health_and_journal_timestamp_readers_preserve_legacy_rows_after_reopen`: `latest_worker_tick` (`heartbeat_at`, `last_progress_at`), `recent_worker_progress` (`event_at`), and `operation_journal_page` (intent and outcome times). The fleet tick is opened through its production writer; legacy event/journal rows are fixture-seeded because their persisted history is append-only.
- `worker_plan_hook_config_timestamp_readers_preserve_legacy_rows_after_reopen`: `WorkersDb::get_by_slug` (`updated_at`), `PlansDb::get_plan` (`updated_at`) and `get_plan_item` (`created_at`, `completed_at`), and `HooksDb::get` (`created_at`, `updated_at`). Actual APIs create each source row, then the isolated DB fixture restores legacy timestamp forms before reopening.

Timestamp triggers or immutability guards are adjusted only inside these temporary test databases to represent already-persisted legacy values that production trigger paths would otherwise canonicalize or reject. No production source or schema is changed.

## TG4-19 platform selectors

The existing `analytics_timestamp_pairs_` selector remains. A finite Bash loop in `.github/workflows/ci.yml` adds these exact filters to the existing three-platform TG4-19 job:

1. `generated_task_and_run_writers_round_trip_a_leap_year_and_dst_after_reopen`
2. `generated_worker_and_plan_writers_round_trip_a_leap_year_and_dst_after_reopen`
3. `generated_task_auxiliary_writers_round_trip_a_leap_year_and_dst`
4. `generated_run_journal_writers_round_trip_a_leap_year_and_dst`
5. `generated_memory_writers_round_trip_a_leap_year_and_dst_after_reopen`
6. `generated_checkpoint_writes_round_trip_a_leap_year_and_dst`
7. `generated_graph_writes_round_trip_a_leap_year_and_dst`
8. `generated_hook_config_writes_round_trip_a_leap_year_and_dst`

## Validation and limits

- YAML parse: passed with the repository-installed `js-yaml` parser (`node -e "require('js-yaml').load(...)"`). PyYAML was unavailable, so no dependency was installed.
- Rust: the pre-fix compiled test binary ran `timestamp_readers_preserve_legacy_rows_after_reopen`: 2 passed and the Tasks fixture failed because its routing row used the default manual classification, which the public automatic-routing reader intentionally excludes. The fixture was corrected to call `assign_with_routing_history`, so it produces the row the reader consumes. The first post-fix Cargo invocation used a fully reconstructed process environment and failed during linking because that environment omitted expected MSVC setup. Retried once with the established PowerShell toolchain environment and live-provider/database credentials unset: `cargo test --offline -p arcana-core --lib preserve_legacy_rows_after_reopen --locked -- --test-threads=1` passed all 3 tests (0 failed; 1131 filtered out). No broad suite or annual test was run.
- This fixture closes the listed legacy timestamp reader gaps only. It is not a claim that every P4-57/TG4-19 governance clause or deployment platform is closed; the broader closure decision remains with the root owner.

## 2026-10-13 root verification addendum

Every legacy field assertion was strengthened from parseability to equality with an independently seeded instant. The creation value is `2024-02-29 12:34:56` UTC; the resolution value is `2024-11-03T01:30:00-04:00`, which must resolve to the same instant after parsing. The freshly compiled Windows test binary passed all three public-reader fixtures after this strengthening (3/3, 0.54s). The CI job now invokes the nonempty `timestamp_readers_preserve_legacy_rows_after_reopen` selector on all three operating systems. Platform CI execution remains to be observed; this is not TG4-19 closure.
