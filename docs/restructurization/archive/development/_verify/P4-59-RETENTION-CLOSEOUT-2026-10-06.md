# P4-59 / TG4-21 retention closeout — 2026-10-06

**Result: GATE-PROVEN.** The approved five-table retention contract has production lifecycle, migration, boundary, concurrency, reader/export and failure evidence on Windows, Linux and macOS. This closes P4-59/TG4-21 only. It does not close P4 or its command-sandbox, observability/Fleet, timestamp or Calendar gates.

## Policy and real consumers

| Store | Selected policy | Production producer / cleanup / reader |
| --- | --- | --- |
| `token_usage` | 90 days raw; preserve daily cost aggregates transactionally before pruning | AnalyticsDb token writer; `prune_append_only_before`; retained shared reader and actual `arcanactl analytics export` |
| `system_events` | 90 days raw | AnalyticsDb event writer; same transactional pruner; retained shared reader and CLI export |
| `api_failures` | 90 days raw | AnalyticsDb failure writer; same transactional pruner; retained shared reader and CLI export |
| `worker_stats` | 90 days daily detail; older rows become monthly summaries with actual coverage bounds | Daily tick accumulator; atomic monthly rollup/prune; combined daily/monthly history and CLI export |
| `hook_logs` | 90 days raw | HooksDb execution-log writer; transactional `prune_logs_before`; retained hook reader and CLI export |

AnalyticsManager owns the immediate/hourly job in the shared desktop runtime. CLI Fleet explicitly enables it and owns stop/join cleanup on return. P4-55 separately proves readable/structured file retention and full daily export. No credential presence enables paid work.

## Clause-to-proof map

| Required clause | Executed production behavior |
| --- | --- |
| Migration preserves existing populated tables | `populated_pre_retention_analytics_upgrade_preserves_all_existing_rows`; `populated_pre_retention_config_upgrade_preserves_hook_logs_for_the_reader` open actual prior ladders, compare preserved values and consume them through readers. Disposable migration backups only. |
| Exact age cutoff; malformed dates fail closed | Analytics and hook strict-before cutoff tests keep the exact boundary/current rows. Invalid timestamp tests refuse cleanup without deleting valid rows. |
| Cost history across advancing partial-day cutoffs | Both `advancing_retention_*` tests plus coverage/session/sealing cases prove complete pre-existing summaries survive, later batches accumulate exactly once, session unions are deduplicated, and sealed dates reject insert/update. Session IDs expire when their raw date is sealed. |
| Repeated worker ticks; atomic/idempotent monthly rollup | `worker_stats_accumulate_roll_up_atomically_and_read_with_coverage` accumulates same-day ticks and composes monthly/daily readers; `worker_stats_rollup_failure_is_atomic_and_preserves_cutoff_day` injects a later-month failure, proves transaction rollback, retries and keeps the exact cutoff date. |
| Owned scheduled lifecycle and current-date readback | `analytics_manager_runs_append_only_and_worker_retention_on_startup` starts the real immediate job and reads all raw stores plus cost/worker history, then stops/joins it. CLI Fleet uses the same maintenance owner and a lifecycle guard. |
| Active writers | Analytics and HooksDb contention tests hold a real second SQLite writer, verify cleanup serialization and read both the committed writer row and retained boundary afterward. |
| Full export without truncation | Shared analytics reader tests include 15,001 token rows, retained event/failure rows and malformed-date refusal; hook export is covered separately. The actual CLI subprocess exports every retained store, preserved costs, monthly coverage and today's daily worker row. |
| Visible cleanup failure and privacy | Both desktop retention-failure tests invoke real pruners, write JSONL, read it through Logs and aggregate exactly one failed operation. Private row/component/trigger markers are absent. A separate fixed capture probe and explicitly drained writer cannot substitute for the operation assertion. |

## Platform evidence

Exact six-group selection is source-controlled in the existing CI build-smoke matrix: core `retention_`, core `worker_stats_`, shared `db::analytics::`, CLI `analytics_export`, and the two desktop cleanup-failure readers. Selected counts: **23, 5, 4, 1, 1, 1**.

- **Windows:** all six selected groups passed locally on 2026-10-06. After the guarded capture change, the retention desktop selection passes 3/3 and the complete desktop library 167/167 with four threads (24.26s). Real producer-to-persisted-reader assertions remain intact. The remaining retention production bodies did not change between those selected runs and `1cdb7a1`; `140002d` adds timestamp tests only.
- **Linux:** current CI run [37444400127](https://github.com/raziashary/arcana/actions/runs/37444400127), whole-workspace job **112205610578**, passes at `1cdb7a1`, including both cleanup-failure readers, populated migrations, cost/worker retention, active-writer tests, shared export and actual CLI export. Earlier focused WSL retention 23/23 and worker/reader/export proofs corroborate the platform. This current CI job supplies GTK-backed desktop reader execution unavailable in local WSL.
- **macOS:** the same current run, job **112207740059**, passes the complete six-group retention step: 23/23, 5/5, 4/4, 1/1, 1/1, 1/1. It subsequently fails the separate command corpus because the signed App Sandbox XPC runner is not packaged. That failure remains under P4-11/P4-61/TG4-18/TG4-31; the complete pipeline is not green.

Raw safe evidence remains ignored under `target/p4-ci-checks/112205610578-workspace-success.log`, `112207740059-macos.log`, and the Windows retention/capture logs. The earlier `294086c` Ubuntu capture failure is retained in ADR-0315; the latest guarded-capture run passes, but the former loss mechanism is not claimed as diagnosed. No skip, retry loop or removed assertion establishes this closure.

Independent pre-closeout review: `_verify/P4-59-LUNA-CLOSEOUT-AUDIT-2026-10-05.md`. Its cost-loss and CLI-lifecycle findings received owner direction and were fixed in ADR-0306–0308, with the regressions above. Migration and macOS evidence gaps are now covered. Narrow capture triage is retained separately. No live provider request, owner Calendar access or user-data mutation occurred in this closeout. Local verification processes finished; owned application/driver process trees and ports were cleaned up.

Recorded tally immediately after this closure: **P4 39/59; TG4 18/33**. The subsequent independently approved P4-52 closure brings the current tally to **40/59; 19/33**. Successful CLI Fleet completion and the owner-only Calendar acceptance remain unproven.

Later current-run confirmation: Linux build-smoke job **112207740120** also completes successfully, including the complete retention step. Windows build smoke remains in progress at this check; macOS's separate XPC command-corpus failure remains open.
