# P4-57 / TG4-19 Final Closure Dossier — 2026-10-13

> **Superseded audit snapshot:** The three suggested legacy fixtures and CI
> selections now exist, and the last two missing fields (`tasks.completed_at`
> and `runs.ended_at`) passed through reopened public readers on Windows.
> ADR0388 resolves the helper/representation wording, and ADR0391 transfers
> Linux/macOS parity to P9-16/TG9-06. Current closure evidence is
> `P4-57-WINDOWS-CLOSEOUT-2026-10-13.md`; the gap analysis below is retained
> to show how the inventory was reconciled.

Scope: read-only reconciliation of MDP P4-57, TESTING_GATES TG4-19, the 2026-10-12 production-pair inventory, and the current annual/legacy receipts. No tests were run, and this report changes no source, plan, gate, or status. It does not declare closure.

## Normative clauses and evidence state

P4-57 requires: a complete production timestamp writer/reader inventory; one compatible UTC representation/helper; full-year instants including leap day, local midnight and both hemispheres' DST; old persisted rows remain readable. TG4-19 states every written timestamp must be read by its reader, with a year/DST property proof at every write/read pair, and “one format/helper across every database.”

Against the inventory's actual production writer-to-reader pairs, every identified annual pair now has a Windows production-path test receipt. There are no further annual writer/reader tests to author for that inventory. The 2026-10-13 receipts supersede the older gaps for TasksDb, RunsDb, WorkersDb, PlanDb, Memory, checkpoint, GraphStore and hook configuration. The newer P4-57 annual test names and field scope are:

| Exact test | Current receipt | Coverage |
|---|---|---|
| `generated_task_and_run_writers_round_trip_a_leap_year_and_dst_after_reopen` | Windows 1/1, `P4-57-ISOLATED-CLOCK-2026-10-12.md` | Tasks created/updated; Runs started/ended |
| `generated_worker_and_plan_writers_round_trip_a_leap_year_and_dst_after_reopen` | Windows 1/1, same receipt | Worker created/updated/last_seen; plan created/updated; plan-item created/completed |
| `generated_task_auxiliary_writers_round_trip_a_leap_year_and_dst` | Windows 1/1, `P4-57-TASK-AUXILIARY-ANNUAL-2026-10-13.md`; pushed receipt identifies commit `cd30ff4` | Nine fields: task completed; worker-request created/resolved; worker-message created/read; routing assigned/outcome; build-evidence created; budget-resolution resolved |
| `generated_run_journal_writers_round_trip_a_leap_year_and_dst` | Windows 1/1, `P4-57-RUN-JOURNAL-ANNUAL-2026-10-13.md` | Worker heartbeat and last progress; worker event; journal intent and outcome timestamps. Heartbeat writer variants are checked at write time and terminal state after reopen |
| `generated_memory_writers_round_trip_a_leap_year_and_dst_after_reopen` | Windows 1/1, `P4-57-MEMORY-ANNUAL-2026-10-13.md` | Episodic created; semantic created/updated; procedural created/updated; unified memory created via gateway and tier sync |
| `generated_checkpoint_writes_round_trip_a_leap_year_and_dst` | Windows 1/1, `P4-57-CHECKPOINT-ANNUAL-2026-10-13.md` | Checkpoint `created_ms` through create/list, including bounded 50-row retention |
| `generated_graph_writes_round_trip_a_leap_year_and_dst` | Windows 1/1, `P4-57-GRAPH-HOOK-ANNUAL-2026-10-13.md` | Single/batch node `updated_at` through `get_stats.last_updated` |
| `generated_hook_config_writes_round_trip_a_leap_year_and_dst` | Windows 1/1, same graph/hook receipt | Hook created/updated through create, `set_enabled`, and reopened get |

The established `analytics_timestamp_pairs_` selector covers four annual tests: SessionStorage session/message pairs; Analytics token/worker-history and failure/event pairs; and hook-log writer/readers. The annual Session/Analytics/hook-log group is recorded Windows/Linux 4/4. The worker-history extension is recorded Windows/Linux 1/1 and covers 4,400 rows through monthly rollup/readback plus the legacy row. Hook execution time uses the actual writer and now preserves the executor instant under ADR-0320. The relevant receipts are `P4-57-ANALYTICS-PAIR-PROOF-2026-10-06.md`, `P4-57-ANNUAL-WORKER-ROLLUP-2026-10-09.md`, `P4-57-SESSION-HOOK-PAIRS-2026-10-06.md`, and `P4-57-HOOK-EXECUTION-INSTANT-2026-10-06.md`.

Across the new Windows proofs, 34 physical generated-now fields are covered (Tasks 11, Runs 6, Workers 3, Plans 4, Memory 6, checkpoint 1, Graph 1, hook configuration 2); Runs journal intent/outcome are separate writer/read projections of one physical `created_at`. Older annual session, analytics and hook-log proofs are additional. This count does not count schema-only/stored-only timestamps that have no production timestamp reader under the inventory's stated scope.

## Remaining platform executions and test-list gap

The receipts establish the new eight generated tests on Windows, not a complete cross-platform matrix. Linux receipts exist for the older analytics/session/hook-log group and worker rollup; this dossier found no current-tree Linux or macOS execution receipt for the eight generated tests above. The hook/session/analytics receipts do not establish macOS results. `.github/workflows/ci.yml` currently selects only `analytics_timestamp_pairs_` under its three-OS build-smoke matrix; it does not explicitly select the eight new generated tests. A general build/test job is not a pair-specific receipt.

Finite remaining execution set:

- Linux and macOS: the eight exact `generated_*` tests in the table, using their current source and fixture.
- macOS: `analytics_timestamp_pairs_` and the worker-history annual selector; no macOS result is recorded in the receipts reviewed here.
- Preserve Windows receipts already recorded; do not rerun the 4,400-instant tests without source change or root coordination.
- Add the eight new filters to the existing platform CI selector, then aggregate current Windows/Linux/macOS results. Do not treat workflow YAML alone as execution evidence.

## Legacy-row receipts and the finite residual

Existing production legacy/mixed-format receipts should be aggregated rather than rerun blindly: Session `analytics_timestamp_pairs_session_and_message_writers_read_full_leap_year_and_dst` plus `session_list_orders_legacy_and_canonical_timestamps_by_instant`; Analytics `analytics_timestamp_pairs_token_and_worker_writes_round_trip_a_leap_year_and_dst` and `analytics_timestamp_pairs_failure_and_event_writes_read_a_year_and_legacy_rows`; hook logs `analytics_timestamp_pairs_hook_writer_and_readers_cover_full_leap_year_and_dst`; Runs `production_run_chronology_reads_mixed_timestamp_formats_by_instant`, `production_worker_run_readers_select_latest_instant_across_legacy_rows`, and `active_worker_operation_reads_canonical_and_legacy_timestamps`; Tasks `production_task_reader_returns_canonical_timestamps_and_legacy_rows_survive_upgrade`; Workers `production_worker_reader_returns_canonical_timestamps_and_keeps_legacy_rows`; Plans `production_plan_reader_returns_canonical_timestamps_and_keeps_legacy_rows`; Graph `file_backed_graph_writer_default_and_mixed_timestamp_reader_are_canonicalized`; and the named episodic, semantic, procedural, fallback and rank mixed-format tests in the Memory inventory. These receipts are not all same-platform executions and must be joined to the host matrix rather than counted as broad proof from test presence.

The new annual tests generate canonical values; they are not legacy-row fixtures. The existing source receipts leave these bounded legacy-reader areas without field-complete evidence: TasksDb auxiliary timestamps (especially worker-request, message and routing chronology); RunsDb worker-state/event and journal outcome projections beyond legacy run-start ordering and legacy active-intent parsing; Workers `updated_at`; Plan `updated_at` and plan-item created/completed; and hook configuration created/updated. Existing legacy tests cover only selected columns in those tables. If P4-57's “old persisted rows remain readable” is required per actual writer/reader pair, author compact production-reader fixtures (not another annual corpus), one per owning DB, for these uncovered columns. Suggested exact test names: `task_auxiliary_timestamp_readers_preserve_legacy_rows_after_reopen`, `run_health_and_journal_timestamp_readers_preserve_legacy_rows_after_reopen`, and `worker_plan_hook_config_timestamp_readers_preserve_legacy_rows_after_reopen`. Do not add a checkpoint legacy-format fixture: `created_ms` is an unchanged integer-millisecond contract and already has annual production list/read proof.

If the project interprets legacy readability at the DB/migration level rather than at every column, some of that fixture work may be unnecessary; the governing sentence does not name the granularity. Record which interpretation is accepted before claiming this clause complete. The legacy receipts already prove the known chronological regressions for Session, Analytics, Runs, Memory, and Graph; they do not substitute for the annual proofs.

## Representation/helper reconciliation

ADR-0128 says Rust writers use the shared formatter and SQLite defaults/triggers emit the same fixed-millisecond UTC `Z` representation. The source inventory confirms SQLite schema writers repeat equivalent `strftime('%Y-%m-%dT%H:%M:%fZ','now')` expressions instead of consuming the shared `SQLITE_UTC_NOW` constant. Annual tests assert the resulting text against the shared formatter, but this proves output equivalence, not one shared SQL helper owner. Separately, ADR-0370/0375 explicitly preserve checkpoint timestamps as integer epoch milliseconds, while TG4-19 literally says “one format/helper across every database.” The current approved test decision does not itself amend that normative gate clause. Root must reconcile the existing accepted checkpoint exception and the meaning of “one helper” for SQLite defaults before claiming this part met; no source change is proposed here.

## Document reconciliation before any closure claim

The newer Task auxiliary and Runs journal receipts supersede stale “missing task auxiliary/run heartbeat/progress/event/journal annual proof” statements in the 2026-10-12 pair inventory follow-up and the 2026-10-13 GOAL continuation. The GOAL's 2026-10-13 note that run journal fields remain the next annual target is now stale. Update those records only through the owning closeout process; this dossier does not alter them.

No new annual pair test authoring is indicated by the current source inventory. The remaining finite work is platform execution/CI selection, any owner-required per-column legacy fixtures, and explicit reconciliation of TG4-19's helper/representation wording with the accepted decisions. Do not mark P4-57/TG4-19 closed from this dossier alone.
