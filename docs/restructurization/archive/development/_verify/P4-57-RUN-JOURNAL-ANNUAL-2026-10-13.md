# P4-57 RunsDb Annual Timestamp Pair Proof — 2026-10-13

## Scope and progress

This report tracks a bounded RunsDb annual writer/reader proof under P4-57 and TG4-19. The test must generate 4,400 deterministic leap-year/DST instants and pass them through actual public production writers, then read them through the corresponding production readers after reopening the explicit test database. No provider, Vault, user database, Calendar, schema, or dependency work is in scope. Existing legacy tests remain intact. This is not a phase-close report.

- [x] Read the named P4-57/TG4-19 scope and the two requested timestamp inventories.
- [x] Inventory the existing approved test clock API and relevant RunsDb writer/reader variants.
- [x] Add the single annual test using only public writer/reader APIs; cover the timestamp fields with exact expected counts.
- [x] Run only the approved exact test filter after coordinator authorization; record the result.

## Source-grounded target

P4-57 requires full-year instants including leap day, local midnight, and DST transitions in both hemispheres at each production write/read pair, while preserving legacy readability. TG4-19 assigns that proof to P4-57. The 2026-10-12 RunsDb inventory identifies the still-uncovered annual fields as worker run state `heartbeat_at` and `last_progress_at`, worker event `event_at`, and operation journal intent/outcome timestamps exposed as `intent_at`/`outcome_at`. `WorkerRunState.started_at` is projected from `runs.started_at`; it is not a physical worker-state column. `runs.started_at` and run terminal timestamps already have an annual proof in the approved clock test; existing run chronology/reopen tests remain as legacy and reopen evidence.

Actual production consumer paths from the inventory include `latest_worker_tick(s)` / worker health for state values, `recent_worker_progress` projected by `list_worker_health` for event time, and `operation_journal_page` / `active_worker_operation` for journal entries. Reconciliation `created_at` is persisted but is not projected as a timestamp by a production reader, so it is not a writer/reader pair in this task. The existing approved test clock in `arcana-core/src/db/test_clock.rs` supplies `at()` scopes and 4,400 leap-year/DST instants. The annual test uses only `RunsDb::open_at` with an explicit temporary database path.

## Implementation and evidence

The new `generated_run_journal_writers_round_trip_a_leap_year_and_dst` test in `arcana-core/src/db/runs.rs` uses `RunsDb::open_at` over the module's temporary database fixture and `db::test_clock::{at, yearly_and_dst_instants}`. It writes only through public RunsDb APIs and verifies fixed-millisecond UTC text plus parsed instant equality.

| Fields / paths | Corpus and public readback | Count |
|---|---|---:|
| `worker_run_state.heartbeat_at`: `begin_worker_tick`, `update_worker_tick_phase`, `heartbeat_worker_tick`, `record_worker_progress`, `finish_worker_tick` | `latest_worker_tick` checks each successive writer immediately; after close/reopen, the last (terminal) heartbeat is checked again | 4,400 samples per writer variant; 4,400 reopened terminal values |
| `worker_run_state.last_progress_at` | `record_worker_progress` → `latest_worker_tick`, then reopened `latest_worker_tick` | 4,400 |
| `worker_run_events.event_at` | `record_worker_progress` → `recent_worker_progress`, then reopened `recent_worker_progress` | 4,400 |
| `operation_journal.created_at` intent / API `intent_at` | `record_operation_intent` → `active_worker_operation`, then reopened `operation_journal_page` | 4,400 |
| `operation_journal.created_at` outcome / API `outcome_at` | `record_operation_outcome` → reopened `operation_journal_page` | 4,400 |

The 4,400 source instants span the 2024 leap year, UTC−12/UTC/UTC+14 wall-clock samples, local-midnight edges, and both Northern and Southern Hemisphere DST transitions. Each API variant uses the same 4,400 source points with millisecond offsets. The overwritten heartbeat column cannot retain every intermediate value, so each heartbeat writer variant is checked through the public state reader at write time, while the final heartbeat survives and is rechecked after reopen. Persisted progress, event, intent, and outcome timestamps are each checked after reopen.

Source/consumer anchors: `RunsDb::begin_worker_tick`, `update_worker_tick_phase`, `heartbeat_worker_tick`, `record_worker_progress`, and `finish_worker_tick`; `latest_worker_tick` / `latest_worker_ticks` and `list_worker_health` for worker state; `recent_worker_progress` projected by `arcana-core/src/agent/builtin_tools/list_worker_health.rs`; `record_operation_intent` / `record_operation_outcome`; `active_worker_operation` (timestamp parser) and `operation_journal_page` (intent/outcome projection). The earlier production pair inventory records reconciliation timestamps as persisted-only, so they are deliberately not included. The existing run create/end annual pair test remains the evidence for `runs.started_at` / `ended_at`; this test does not assert those fields.

Root ran the compiled Windows core test binary with the exact filter `generated_run_journal_writers_round_trip_a_leap_year_and_dst --test-threads=1`, stripping Gemini/Google/database credentials in the command process. Result: **1 passed, 0 failed, 1126 filtered out, 32.74s**. The agent's earlier execution result was lost at its usage limit; this root receipt is the accepted execution evidence. Scoped rustfmt and whitespace checks pass. This adds four physical generated-now fields to the earlier 21 Windows fields (25 total), with intent/outcome as separate public projections of journal.created_at. Other inventory pairs and cross-platform/legacy aggregation remain open. No listener, live database, provider, or Calendar was used.
