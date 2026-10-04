# P4 Session Checkpoint — 2026-10-04

## Current goal

Continue the expanded P4 scope from the user's canonical goal attachment. Keep P4 open until its implementation, testing gates, audits, observability/CLI Fleet proof, and production evidence are complete. The owner will run the final Calendar scenario in `C:\Playground\arcana-calendar` only after Arcana is ready. Current live-document counts remain **32/59 P4 tasks** and **10/33 TG4 gates**. This continuation changed no task or gate checkbox.

## Work completed in this continuation

- Corrected the staged-Python isolation test's deadlock. The test previously awaited stdout and stderr EOF before `WindowsSandboxChild::wait()`, while `wait()` finalizes the bridge output streams. The test now drains stdout/stderr and awaits wait concurrently. Exact test passes 1/1 in 38.46s.
- Focused Windows process-sandbox group passes 16/16 in 96.37s, including real bridge output capture, workspace writes, sibling read/write denial, loopback denial, descendant cleanup, external read-root behavior, staged runtime isolation, and `.cmd` invocation.
- Fixed two failures exposed by the first full run. The worker-journal fixture now records an unknown operation before asking the Supervisor to reconcile it; the focused test passes. Dashboard worker-created-today now compares canonical UTC timestamps against the UTC date; its production-reader fixture passes.
- Re-ran the complete core library suite: `cargo test -p arcana-core --lib --locked -- --test-threads=1`. **962 tests: 959 passed, 2 failed, 1 ignored in 645.51s.** The two failures are `production_clippy_environment_is_visible_and_readable_in_workspace_sandbox` and `checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher`. Both fail when the real Cargo Clippy path canonicalizes its config/workspace under Windows SBOX (OS error 5). The child can read and stat those exact files.
- Tested staging the validated Clippy config alone in an explicit read-only directory under Arcana's profile and granting that exact root to SBOX. The child could read/stat the config but still could not canonicalize the directory or file. Removed this ineffective code experiment; no sandbox policy was relaxed.
- Followed the Windows volume-root path-resolution hypothesis with a canonical read-only drive-root SBOX grant. Schema tests passed 2/2, but production Cargo Clippy still failed with OS error 5; the helper and grant were removed. **Security correction, ADR-LOG-RESTRUCT-0245:** the non-recursive exception is for `fs_read_write`; `fs_read_only` is recursive. The one failing run therefore used a broader read boundary than intended while executing only the generated dependency-free Clippy probe, with network denied. There is no evidence unrelated user-file contents were read or disclosed. The current source again grants only the selected workspace and exact runtime roots.
- Corrected documentation that called the earlier Python test a sandbox-launch stall; the observed cause was the test harness pipe/wait ordering. Updated ADR-LOG-RESTRUCT-0242 and added ADR-LOG-RESTRUCT-0243 for this run and the false-positive fixtures.

## What this means

The staged Python/runtime and focused Windows SBOX tests now have stronger evidence, but the full core gate remains red. Clippy path canonicalization is an OS-policy blocker that must be solved while preserving no external reads and no child network. Do not mark guarded workspace staging/copy-back, P4-11/P4-61, or TG4-18/TG4-31 complete from direct readability tests. P4/TG4 counts remain 32/59 and 10/33.

Other established open P4 work remains: secure Windows staging/copy-back and dependency staging; packaged Tauri sandbox smoke and supported Windows build matrix; Linux Landlock and signed macOS XPC/staging; cross-platform ProjectCommand corpus; successful budgeted CLI Fleet coding run; complete Logs producer-to-desktop-reader proof and telemetry measurements; durable mutation reconciliation/continuation/no-duplicate recovery; remaining full expanded-goal audits; and owner-run Calendar acceptance.

## Next actions

1. Inspect Microsoft's supported Windows sandbox/path-resolution options against the exact Rust canonicalization failure. Do not weaken read/network limits or count `metadata`/read as canonicalization proof.
2. Keep Clippy/corpus gates open until the real production launch passes. Preserve a minimal failing regression while comparing backend options.
3. Continue remaining P4 implementation and verification in task order. Re-run focused checks after each change, then rerun full gates at a stable boundary.
4. Refresh the active goal, readiness, gates, and checkpoint from actual evidence. Commit only completed, verified logical stages; no commit or push occurred in this checkpoint.
5. Do not run the owner's Calendar desktop scenario; it remains the final acceptance test after all automated and CLI gates are ready.

## Git and environment

Branch is `main`, upstream is `origin/main`, HEAD observed at start of this continuation was `fff31ec3458f09b0874c581f18fd40dcaf097f8d`. The expanded P4 tree remains broadly modified and uncommitted; this continuation added source corrections and dated evidence. No live Gemini request was made. No commit or push occurred. Historical generated temp Python test data remains for owner cleanup; no active Cargo/test process or listener remains after the completed test run.

## Validation after this checkpoint entry

`cargo check --workspace --locked`, `cargo fmt --all -- --check`, `npx tsc --noEmit`, `npm test -- --run` (12 files, 52 tests), and `git diff --check` pass. No background test listener remains. The only full core failures are the two real Clippy path-canonicalization cases described above; counts remain P4 32/59 and TG4 10/33.

## Continuation after checkpoint — Windows ancestor traversal experiment

- Ran a disposable Rust child through the production experimental SBOX bridge from `C:\arcana\target\p4-acl-tests`, after applying the approved exact, non-inheriting `FILE_TRAVERSE | FILE_READ_ATTRIBUTES` grants to every accessible ancestor. The child still returned `workspace_resolves=false`; the sibling file read remained denied. The test failed after 90.07 seconds, so this ACL shape does not unblock Rust path canonicalization even on a user-owned workspace path.
- Removed the temporary test seam and test after the result. No production SBOX policy, ACL, or identity-reuse behavior changed. The existing test that inspects the non-inheriting ACE shape remains separate and is not path-resolution proof.
- `cargo fmt --all -- --check`, `cargo check -p arcana-core --locked`, and `git diff --check` pass after cleanup. No P4 task or gate changed; the Windows Clippy/full-core failures remain open and no Calendar app test was run.
- Re-ran the existing Windows sandbox group after removing the probe seam: 16/16 pass in 97.75s. `cargo check -p arcanactl --locked`, `cargo check -p arcana --bin arcana --locked`, and the built `arcanactl --help`, `fleet --help`, and `fleet run --help` smoke checks pass; the CLI exposes the explicit per-run Supervisor cost/token/turn ceilings. These help checks do not count as a Fleet execution or provider-path proof.
- Ran an isolated CLI Fleet preflight with `google`/`gemini-3.8-flash`, a $0.01 hard cap, a fresh temporary `USERPROFILE`, and a temporary workspace. The CLI found the provider credential but refused before epic/task DB creation because that workspace had no persisted egress consent; `tasks.db` was absent and the error gave the explicit `arcanactl consent grant` action. No provider request occurred. The disposable profile remains at `C:\Users\razia\AppData\Local\Temp\arcana-cli-preflight-8374323e03fe445c9931e5e5a43d9253`; a cleanup command was rejected by execution policy, so preserve it for later manual removal rather than broadening the cleanup attempt.
- The exact ancestor traversal probe was extended to `C:\`. `SetNamedSecurityInfoW` returned Windows OS error 5 before modifying the volume-root DACL, confirming the process cannot add even the narrow non-inheriting traversal/read-attributes ACE there. The temporary profile was dropped; no root ACL change occurred. Together with the prior 90.07-second SBOX test, exact accessible-ancestor grants do not solve canonicalization and the volume-root exception cannot be granted from this process.
- The entire `arcanactl` test suite passes: 11 unit tests, the isolated consent subprocess test, and 3 core-runtime integration tests (15 total). The CLI test suite makes no live provider request. This remains component/entrypoint evidence; a successful Fleet run is still open.
- Added and ran the live-smoke guard `p4_cli_google_credential_matches_the_live_smoke_dotenv_key`: the credential `arcanactl` reads from the Google vault exactly matches `GOOGLE_API_KEY` in the repository `.env` (1/1). The test zeroizes both in-memory copies and prints neither value; it made no network call and the key did not need updating. This protects future CLI smoke runs from silently using an older vault credential.
- A canceled external helper build left generated, ignored scratch source under `C:\arcana\target\p4-vault-check`; it contains no credential values. The preflight profile listed above also remains because the execution policy rejected its cleanup command. No active Cargo/Rust/CLI processes remain after these checks.

## Continuation — bounded SBOX bridge output proof

- The owner approved Arcana's in-sandbox bridge for the experimental Windows SBOX runner. The bridge implementation was already present in the working tree; this continuation added direct proof for its capture bound.
- `bridge_stream_capture_keeps_a_bounded_tail_and_total_byte_count` feeds a stream larger than the 256 KiB capture limit and proves that only the newest 256 KiB are retained while the full observed byte count is preserved.
- `cargo test -p arcana-core --lib bridge_ --locked -- --test-threads=1` passes 5/5, including the bounded-tail test and real SBOX bridge output/workspace-write/sibling-read-denial/external-runtime-read/OS-temp-workspace probes. No provider request was made and no listener remains.
- `cargo fmt --all -- --check` and `git diff --check` pass. An initial exact-filter invocation selected zero tests; the corrected `bridge_` filter above ran all five tests.
- This proves bridge capture only. The full core suite remains 959 passed, 2 failed, 1 ignored; both failures are production Cargo Clippy path canonicalization under Windows SBOX. P4/TG4 remain 32/59 and 10/33. No task/gate closes and no commit or push occurred.

## Continuation — concurrent read cancellation

- A fresh P1/P3 cross-phase Luna High review identified that parallel read-only tool handlers did not observe session cancellation while the executor awaited their `JoinHandle`s.
- `ToolExecutor` now selects each read handler against the existing session cancel signal. Cancellation drops an in-flight read future and follows the normal structured result/event/registry-cleanup path; write operations retain their separate unknown-outcome journal behavior.
- `cancelling_session_drops_inflight_concurrent_read_and_returns_correlated_failure` verifies prompt completion, dropped handler, `operation_cancelled` category/code, call correlation, and active registry cleanup. The complete `p8_execution_registry_tests` group passes 17/17.
- No P4 task, gate, or readiness score closes from this slice. P4/TG4 remain 32/59 and 10/33; full core still has the two Windows Clippy path-canonicalization failures. The provider-capped CLI Fleet run and owner Calendar test remain separate, unresolved acceptance work.

## Continuation — observability privacy and retention fixes

- The readable log writer now buffers complete records up to 256 KiB, redacts after reassembly across writes/flushes, and emits only a fixed omission marker for oversized records. Focused writer tests pass 3/3.
- The retention sorter now preserves today's active file ahead of future-dated recognized names. The retention unit suite passes 6/6, including append/readback through an open active log after pruning.
- Tauri and CLI startup now share `log_retention::prune_default()` after logger initialization. The real isolated `arcanactl consent status` subprocess regression prunes both streams to five files and retains today's active paths (1/1).
- ADR-LOG-RESTRUCT-0249/0250/0251 and `_verify/2026-10-04-CROSS-PHASE-OBSERVABILITY-AUDIT.md` record the findings and scope. No task/gate closes. P4/TG4 remain 32/59 and 10/33; total MCP wire-body bounds, DB table retention, Supervisor health completeness, live Fleet-to-Logs UI, full telemetry privacy/source coverage, CLI Fleet completion, the full-core Clippy failures, and owner Calendar acceptance remain open.

## Continuation — result-aware SQLite setup spans

- Added fixed-label parent operation spans for generic tuned SQLite open (`database_connection_open`), complete workers DB repair/migration/seed (`workers_database_initialize`), and daily-cap save (`daily_cap_save`). Success is recorded only after the operation succeeds, including transaction commit for the saved cap; early errors remain `error`.
- Exact focused tests pass 1/1 each for tuned-open success/failure and path redaction, workers initializer success/failure and path redaction, and daily-cap save success/failure, production readback, and value redaction.
- `cargo fmt --all -- --check`, `cargo check --workspace --locked`, `npx tsc --noEmit`, `npm test -- --run` (12 files/52 tests), and `git diff --check` pass after this source slice. The workspace check reports existing unused/dead-code warnings but exits successfully. The focused Rust tests above each pass 1/1.
- No P4 task or TG4 gate closes. Counts remain 32/59 and 10/33. Database retention periods for `token_usage`, `system_events`, `api_failures`, and `hook_logs` are awaiting owner direction because implementation would irreversibly prune historical rows. MCP pre-parse wire response bounds and the existing Windows SBOX Clippy canonicalization failures remain open.

## Continuation — Supervisor task recovery fields

- `list_worker_health` now returns the configured model and a bounded task summary read through the existing `TasksDb::get_by_id` production API: task status, retry count, assigned worker, epic/phase/dependency IDs, and budget-exceeded state. It deliberately omits title, description, criteria, and milestone prose; database failures are reported as `unknown` with a fixed safe class.
- `cargo test -p arcana-core --lib agent::builtin_tools::list_worker_health::tests --locked -- --test-threads=1` passes 8/8. The fixture creates an epic/child task, records two retry increments, invokes the same helper used by the Supervisor tool, verifies persisted values and configured model, and plants text markers to prove they are absent. A separate failure test confirms unavailable task storage is reported as unknown without raw SQLite details.
- Updated P4-50 and P4-54 notes in the MDP, TG4-24/TG4-24 and P4-54 evidence in Testing Gates, and the corresponding readiness rows. No task/gate closes. P4/TG4 counts remain 32/59 and 10/33.
- P4-54 remains open for provider token totals, worker parent/child links, recovery continuation, reassignment, and no-duplicate side-effect proof. Retention periods for the four additional analytics/hook tables still await owner direction; MCP pre-parse wire bounds, two Windows SBOX canonicalization failures, full CLI Fleet completion, producer-to-Logs UI proof, and owner Calendar acceptance remain open.

## Continuation — 2026-10-04 owner bridge confirmation and Supervisor token health

- The owner confirmed the Arcana-owned in-sandbox Windows process bridge. The bridge implementation already existed in this working tree; the focused bridge tests pass 5/5, including the bounded 256 KiB capture-tail/total-byte contract and real SBOX output/sandbox cases. This does not fix production Clippy canonicalization or prove guarded stage/copy-back.
- `list_worker_health` now aggregates request count, prompt/completion/thinking/total tokens, and cost by the durable worker-run session using `idx_token_usage_session`. The result excludes session IDs and request bodies. The 502-session populated plan/result test passes 1/1; the production Supervisor health tests pass 9/9 with exact session isolation and privacy coverage.
- Current-tree execution of `production_readers_are_registered_in_the_p4_query_inventory` reports 22 unregistered fingerprints across RunsDb and TasksDb and one stale TasksDb fingerprint after the newly added session-token fingerprint was registered. P4-58 and TG4-20 were reopened and must not be reported complete until every current fingerprint has a populated-plan/result proof or an approved bounded rationale, the stale row is reconciled, and the registry suite passes.
- At this point in the session, a direct checkbox scan temporarily reported P4 **31/59** and TG4 **9/33** while P4-58/TG4-20 were being reconciled. This was superseded by the subsequent registry reconciliation below. The owner Calendar test remains unrun. No commit or push had occurred at that point.


## Continuation — P4-58/TG4-20 restored

- Reconciled the 2026-10-04 AST query inventory after its current-source check found 23 missing (owner, fingerprint) pairs and one stale QC pair. Added populated plan/result or trigger behavior proof references for every current pair and removed the obsolete QC fingerprint. The source checker passes; v7 is DDL-only and does not add a reader fingerprint.
- Added RunsDb migration 7 with idx_operation_journal_task_cursor, a partial descending (task_id, id) index over task-bound intents. A simulated v6 database with a persisted task intent upgrades, reads the row back and selects the new index.
- cargo test -p arcana-core --test p4_query_registry --locked -- --test-threads=1 passes 8/8. Focused populated RunsDb operation-journal, v7-upgrade, TasksDb build-batch plan, interrupted-QC, Supervisor retry-approval and QC evidence tests pass.
- P4-58 and TG4-20 are checked again. The note captured a 31/58 scan and a 10/33 TG4 scan while the P4 denominator was still being reconciled. The current exact scan on 2026-10-04 is **32/59 P4 tasks** and **10/33 TG4 gates**; the temporary 31/58 count is historical, not current. This restores the earlier 32/59 tracker count after a temporary reopen, so it is not a net checked-task increase over that checkpoint.
- No commit or push. Calendar remains owner-only and was not run. The full core suite remains red on the two Windows Clippy canonicalization cases; CLI Fleet success and remaining P4 work remain open.

## Continuation — P4-57 RunsDb legacy timestamp readback

- `RunsDb::active_worker_operation` previously parsed operation-journal timestamps with strict RFC3339 even though its shared timestamp contract accepts legacy offset-less SQLite values. It now uses `parse_db_timestamp`.
- `cargo fmt --all -- --check` and `cargo test -p arcana-core --lib db::runs::tests::active_worker_operation_reads_canonical_and_legacy_timestamps --locked -- --exact --test-threads=1` pass. The test exercises canonical timestamp readback from the production intent writer and legacy timestamp readback from an append-only journal fixture through the production active-operation reader.
- Updated the P4-57/TG4-19 evidence without closing either item. Mixed-format analytics/session ordering and all remaining production timestamp write/read pairs are still open.

## Continuation — P4-57 analytics and session timestamp ordering

- Corrected lexical timestamp ordering in CLI token history and SessionStorage messages. Both now order by SQLite `julianday` while preserving the stored display string; additive composite expression indexes support the filtered production readers.
- `cargo test -p arcana-shared cli_analytics_readers_use_populated_indexes_and_return_production_results --locked -- --test-threads=1` passes 1/1 with mixed legacy/canonical rows, actual-instant order, and global/filtered EXPLAIN index assertions. `cargo test -p arcana-core --lib session::storage::tests::session_and_message_round_trip_canonical_timestamps_and_legacy_rows --locked -- --exact --test-threads=1` passes 1/1 with chronological readback and its EXPLAIN index assertion.
- The first query-registry scan correctly failed after the SQL fingerprint changed. Updated both current fingerprints and removed both stale forms; the focused registry test passes 1/1 against current source.
- ADR-LOG-RESTRUCT-0256 records the existing `julianday` convention and additive index decision. P4-57/TG4-19 remain open for all remaining production timestamp pairs.

## Continuation — P4-57 hook execution timestamp readback

- Extended the production HooksDb operation test to verify the `config.db` trigger-created `hook_logs.executed_at` value is fixed-millisecond UTC and can be parsed by the shared timestamp reader after `HooksDb::get_logs` reads it back.
- `cargo fmt --all -- --check` and `cargo test -p arcana-core --lib hooks::db::observability_tests::hook_configuration_and_execution_log_operations_emit_private_result_spans --locked -- --exact --test-threads=1` pass; the focused test passes 1/1.
- This adds one production write/read pair to P4-57 evidence. All other database pairs remain in the inventory gap; no task or gate closes.

## Continuation — P4-14 multi-project cancellation after a passing child

- Expanded the production cancellation regression to select two independent Cargo projects in sorted order. The first completes successfully; the second starts a build-script descendant before the caller is cancelled.
- `cargo test -p arcana-core --lib agent::builtin_tools::project_command::tests::cancelling_production_project_command_stops_descendant_and_closes_batch --locked -- --exact --test-threads=1` passes 1/1 in 33.96s. After TasksDb reopen, the batch is incomplete with expected=2, observed=1, passing=1. The descendant stops, and both workspace-bound TasksDb QC transition and direct SQL reject `qc_review`.
- This closes only multi-project partial-cancellation proof. Spawn-failure and cross-platform production command coverage remain open; P4-14/TG4-07 and overall readiness counts do not change. Full core suite still has the two Windows Clippy path-canonicalization failures. Calendar was not run.

## Continuation — P4-14 multi-project spawn failure after a passing child

- Added a production ProjectCommand test with a build-script barrier: the first Cargo child pauses, the second detected project is moved away, and the first child then exits successfully. This makes the second process fail at launch without fabricating an exit.
- `cargo test -p arcana-core --lib agent::builtin_tools::project_command::tests::production_multi_project_spawn_failure_cannot_be_hidden_by_passing_sibling --locked -- --exact --test-threads=1` passes 1/1 in 17.17s. The result reports `command_spawn_failed`; reopened TasksDb records expected=2, observed=1, passing=1, retry count 0; the TasksDb API and direct SQL both refuse QC.
- Updated P4-14/TG4-07 evidence only. Cross-platform command coverage remains open and overall counts do not change. The prior attempted synchronization initially watched the wrong Cargo target path; corrected it to Arcana's workspace-local `.arcana-cache/cargo-target`. The interrupted test process was stopped, and no Cargo/rustc process remained before rerunning successfully.

## Continuation — P4-59 CLI export of retained worker history

- Added an isolated production CLI regression. It seeds one real daily worker-stat row, rolls and prunes it through `AnalyticsDb`, starts `arcanactl analytics export` in a child process, and parses the actual export file.
- `cargo test -p arcanactl --test analytics_export --locked -- --exact --test-threads=1` passes 1/1. The export retains monthly granularity, coverage start/end, completed/failed counts, minutes, cost, and heartbeat.
- This proves the CLI export consumer reads the retention rollup. P4-59/TG4-21 remain open for migration preservation, active-writer safety, current-date export evidence, and policies/lifecycle proofs for the other append-only tables. No user data or live credentials were used.
- Extended the export fixture with a row dated to the current UTC day. The same CLI export now verifies the current daily row remains alongside the historical monthly rollup; rerun passes 1/1.
- A first pre-RAII failed CLI-export fixture left its own temp profile at `C:\Users\razia\AppData\Local\Temp\arcana-analytics-export-7d79fcec-eadb-40b7-8073-b220657c891d`; it contains only test-generated analytics data. It was preserved rather than removed. The passing fixture now uses an RAII profile guard.

## Continuation — P4-50 GraphStore timing assertion and timestamp suite

- The timestamp-filtered core suite found a stale GraphStore timing assertion. `production_graph_readers_emit_private_result_spans` captured 10 timing events because nested statement timings are also emitted, but it expected only five named public graph operations. The test now selects those five expected operation labels while preserving result/privacy assertions.
- The focused regression passes 1/1, and `cargo test -p arcana-core --lib timestamp --locked -- --test-threads=1` passes 30/30. No P4/TG4 checkbox changes.

## Continuation — P4-52 claimed-task startup retry proof

- Extended `model_produced_commit_runs_through_recipe_runner_and_host_guard` with one injected setup failure before the first provider request and a persisted task advisory budget. The actual shipped backend recipe then reaches AgentEngine once, claims the assigned task, applies its budget, and commits the scripted change once through the production host guard.
- `cargo test -p arcana-core --lib model_produced_commit_runs_through_recipe_runner_and_host_guard --locked -- --test-threads=1` passes 1/1; the task remains assigned to the same worker, has a nonzero bounded turn count, and is not marked budget-exceeded. An initial overly strict `--exact` filter matched zero tests; rerunning with the unique test-name filter selected and passed the intended test.
- Updated P4-52/TG4-26/Readiness evidence. This is a pre-provider retry followed by a single task/budget workflow; daily-budget reservation continuity, retry-event Logs readback, post-side-effect fault recovery, and successful CLI Fleet proof remain open. No checklist checkbox changed.


## Continuation — 2026-10-04 append-only retention decision and implementation

- Owner selected 90-day raw retention, monthly summaries where useful. ADR-LOG-RESTRUCT-0257 records the table-specific policy: preserve token-cost history by refreshing existing daily summaries before deleting expired token details; retain worker monthly rollups; avoid rollups for recent-only API failure/system event/hook detail.
- `AnalyticsDb::prune_append_only_before` applies the UTC cutoff transactionally across token usage, system events and API failures. It backfills daily costs/tokens from all expired token rows, including after long offline periods; exactly-at-cutoff rows remain. Invalid timestamps abort the whole transaction. `HooksDb::prune_logs_before` applies the same boundary and failure behavior. The shared hourly maintenance job invokes both plus the existing worker-stat rollup, covering desktop and CLI runtime owners.
- Focused tests pass: analytics retention 2/2; hook retention 2/2; shared production manager lifecycle 1/1. `cargo fmt --all -- --check` and `git diff --check` pass. The first manager-test invocation used an over-specific `--exact` filter and ran zero tests; the corrected unique-name invocation passed 1/1.
- P4-59/TG4-21 remain open for cross-platform writer-contention coverage, complete export of retained/pruned history, visible cleanup-failure log readback and target-platform proof. No live user DB was pruned.

## Continuation — 2026-10-04 CLI Fleet observability result

- The real `arcanactl fleet run` executed under a temporary profile/workspace using the verified current Gemini key, Google `gemini-3.8-flash`, explicit workspace consent, and a $0.25/50,000-token/10-turn Supervisor cap. It created an epic and seven recipe workers and made one successful provider call. A later retryable provider failure was retried; the next conservative pre-dispatch reservation was denied, so the CLI exited 1 and the epic stayed incomplete. Raw provider text is intentionally excluded from this record.
- Real `arcanactl logs export` and `arcanactl analytics export` read back two log streams, one token-usage row and one worker-stat row. This is useful failure-path telemetry and export evidence, but not the successful CLI Fleet proof needed by TG4-24. Artifacts are preserved in `%TEMP%\arcana-p4-cli-fleet-7adb0d516d854f66a4696b824b28a015`. No P4 count changed.


## Continuation — 2026-10-04 retention contention and export consumer proof

- Analytics and hook cleanup each pass exact-cutoff, malformed-time, and concurrent active-writer tests (3/3 per suite). Cleanup waits for a committing SQLite writer transaction, then prunes the now-visible expired row. The test currently runs on Windows; cross-platform contention remains open.
- The scheduled production maintenance test now reads back current system events, API failures, and hook logs through the real APIs, and confirms expired token rows survive as daily cost/token summaries.
- Extended the real CLI export subprocess fixture: after production retention prunes a 2025 token call, its raw row is absent from `token_usage`, but its daily cost and token totals remain in exported `cost_summary`. Existing worker monthly coverage and current-day daily row remain included. `cargo test -p arcanactl --test analytics_export --locked -- --exact --test-threads=1` passes 1/1.
- TG4-21 remains open for platform coverage, full cleanup-failure log export/readback, and remaining phase-level checks.

## Continuation — 2026-10-04 CLI Fleet budget and quota rerun

- The feature-gated credential identity test passes 1/1: the Google key in the production vault equals `.env` `GOOGLE_API_KEY`; the test zeroizes both copies and makes no provider request.
- A fresh-profile real CLI run using Gemini 3.8 Flash, explicit workspace consent, `$0.25`, 75,000 tokens, and 10 turns persisted one successful usage row (62,822 tokens, `$0.04865`) but stopped before any child task was created when conservative reservation accounting reached 87,269 tokens. The task database has no child tasks for this epic.
- A third fresh-profile attempt after seven minutes, again at 150,000 tokens, was rejected before child creation with `provider_quota_exhausted`. Its log and analytics exports succeeded and contain zero token-usage rows plus one worker-stat row.
- Real CLI `logs export` and `analytics export` succeeded for all three isolated profiles. The first two exports contain one token-usage row and one worker-stat row apiece; their logs are 921,864 and 696,843 bytes. The log export is line-oriented key/value text, and only a small number of typed Fleet/token events appeared among ordinary logs.
- Artifacts are preserved at `%TEMP%\arcana-p4-cli-fleet-ea593ab3bdd94feda5977f128d38f3a2`, `%TEMP%\arcana-p4-cli-fleet-68eb7e0bad1a40f89d072d5f56f54222`, and `%TEMP%\arcana-p4-cli-fleet-a4c542436c584f65af1f802587280c04`. No CLI, worker, or Node process remained after export. This is failure-path and CLI-reader evidence only; successful Fleet completion, producer-to-Logs UI readback, and TG4-24 remain open. P4 remains 32/59 and TG4 remains 10/33. Calendar is untouched and stays owner-run.

## Continuation — 2026-10-04 Gemini provider error privacy

- Source tracing confirmed a provider response was placed directly in `ArcanaError::Provider`, then formatted into RecipeRunner stderr, worker tick outcomes, and the scheduler system-event path. Provider analytics already used typed failure classes; it was not the leaking consumer.
- Google generation, stream, and countTokens error handling now classifies from a bounded body and returns HTTP status, typed `ErrorClass`, and the existing validated safe summary only. Oversized or unreadable error bodies produce a bounded unavailable-details summary and fail closed as `Fatal`; quota and retry classes remain distinguishable when a complete bounded body is available. The two generation paths now honor the injected API base for deterministic loopback tests.
- `cargo test -p arcana-core --lib provider::google::google_tests --locked -- --test-threads=1` passes 19/19; the production AgentEngine no-retry/analytics regression passes 1/1; `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass.
- ADR-LOG-RESTRUCT-0258 records the boundary. This does not close P4-51 or TG4-24: other providers, full runner-to-system-event/Logs readback, and the real successful CLI Fleet workflow remain unproven. Overall counts remain P4 32/59 and TG4 10/33.

## Continuation — 2026-10-04 retention failure Logs readback

- Added a production-path regression in the Tauri Logs command module: an isolated production `AnalyticsDb` refuses a malformed timestamp during retention, the operation error is written to a JSONL sink, and the production reader/metrics aggregator consumes it as a failed `operation_timing`.
- The test confirms the event identifies only the fixed operation and logical database, and does not expose planted database row/component content. `cargo test -p arcana retention_cleanup_failure_is_visible_through_structured_logs_reader --locked -- --nocapture --test-threads=1` passes 1/1.
- This closes the analytics-pruner-to-Logs-reader proof slice only. Hook-pruner error readback, full retained-detail export, cross-platform writer-contention coverage, TG4-21, and P4-59 remain open. P4/TG4 counts remain 32/59 and 10/33.

## Continuation — 2026-10-04 hook retention failure Logs readback

- Added `HooksDb::open_at`, reusing the existing config schema owner's path-based initializer so integration tests can open isolated production config databases.
- The Tauri Logs test injects a SQLite delete-trigger failure into `hook_logs`, calls the production `HooksDb::prune_logs_before`, persists the resulting operation event, and proves the production reader/metrics aggregator reports one failed operation without exposing the trigger or hook fixture values. `cargo test -p arcana hook_retention_failure_is_visible_through_structured_logs_reader --locked -- --nocapture --test-threads=1` passes 1/1.
- Together with the analytics cleanup test above, both production pruning paths now have error-to-structured-Logs reader proof. Full retained-detail export, cross-platform writer-contention coverage, TG4-21 and P4-59 remain open; counts remain P4 32/59 and TG4 10/33.

## Continuation — 2026-10-04 all-provider HTTP error privacy

- Source audit found OpenAI and Anthropic unary/stream adapter failures still interpolated unbounded raw response bodies into `ArcanaError`, unlike the recently fixed Google adapter.
- The shared provider error reader now caps response inspection at 16 KiB and classifies a quota/retry outcome only from a fully read bounded body. All three adapters omit raw HTTP error text; Google keeps its allowlisted status summary and OpenAI/Anthropic return static safe summaries. Incomplete or oversized bodies fail closed.
- Local OpenAI/Anthropic loopback tests pass 2/2 across unary and stream calls; Google tests pass 19/19, including the oversized response case; the production AgentEngine no-retry/analytics regression passes 1/1. `cargo check -p arcana-core --locked`, formatting, and diff checks pass.
- ADR-LOG-RESTRUCT-0259 records the shared boundary. Provider transport, malformed-success, in-stream frame errors, timeout/cancellation, and complete builtin failure matrix remain open; P4-51/TG4-25 and all overall counts remain unchanged.

## Continuation — 2026-10-04 provider transport and frame privacy

- Closed additional provider diagnostic reflection paths: Google/OpenAI/Anthropic request and stream transport failures now return bounded provider/stage codes with the existing retry classification; malformed successful JSON returns a fixed fatal code; malformed SSE frames remain errors but no longer echo provider-controlled frame data. Connection-test logs omit raw reqwest errors.
- Added production-adapter fixtures for malformed successful JSON and unavailable endpoints for all three providers, plus planted private-content parser checks across all three SSE implementations.
- `cargo test -p arcana-core --lib provider:: --locked -- --test-threads=1` passes 126/126. `cargo check -p arcana-core --locked`, `cargo fmt --all`, and `git diff --check` pass. ADR-LOG-RESTRUCT-0260 records the boundary.
- P4-51/TG4-25 remain open for complete builtin failure matrix, timeout/cancellation, and full telemetry readback; P4/TG4 counts remain 32/59 and 10/33. No live provider request or Calendar run was made.

## Continuation — 2026-10-04 retained database export

- Expanded the real `arcanactl analytics export` consumer: it no longer truncates token details at 10,000 rows, and now includes all retained 90-day API failures, system events, and hook logs along with daily costs and combined daily/monthly worker history.
- Added retained-history readers in `src-shared` and the schema-owned hook DB. Malformed timestamps cause explicit export refusal instead of being silently filtered out.
- `cargo test -p arcana-shared --lib analytics_reader_plan_tests --locked -- --test-threads=1` passes 2/2, including 15,001 token rows, current/expired cutoff behavior, and invalid timestamps. `cargo test -p arcana-core --lib hook_log_export_reads_complete_retained_rows_and_refuses_invalid_dates --locked -- --test-threads=1` passes 1/1. `cargo test -p arcanactl --test analytics_export --locked -- --test-threads=1` passes 1/1 against the production CLI subprocess and all raw stores. `cargo check -p arcanactl --locked`, formatting, and diff checks pass.
- ADR-LOG-RESTRUCT-0261 records the contract. P4-59/TG4-21 remain open for cross-platform active-writer contention and platform proof; P4/TG4 counts remain 32/59 and 10/33. No owner Calendar run was made.

## Continuation — 2026-10-04 Memory Search observability readback

- Added a Tauri integration test that runs the production `SearchMemoryTool` against a migrated isolated Memory database, persists its correlated fallback timing to JSONL, and reads it through the production correlation-filtered `read_structured_events` command and metrics aggregator. It checks operation/database, call/run/task correlation, fallback outcome, result count and duration, and verifies neither the query nor stored-memory marker appears in telemetry.
- `cargo test -p arcana production_memory_search_event_persists_and_reads_through_tauri_logs --locked -- --nocapture --test-threads=1` passes 1/1. `npm test -- --run src/tabs/Logs.test.tsx` passes 20/20 and renders the same event contract in Structured Events and Traces; the frontend test still mocks Tauri IPC.
- Refreshed P4-50/TG4-24 readiness with this narrow producer-to-reader proof and refreshed P4-59 readiness to reflect the already-proven hook-cleanup failure readback and complete retained export. TG4-24 live Fleet/desktop UI proof and complete SQLite/Memory coverage remain open; P4-59 cross-platform writer contention and non-Windows evidence remain open. Counts remain P4 32/59 and TG4 10/33.

## Continuation — 2026-10-04 alternate Windows sandbox evaluation

- Verified the current host is Windows 11 Home Single Language, build 26200. Microsoft documents Windows Sandbox as unsupported on Home, so it cannot serve as this PC's VM-backed runner. The documented `.wsb` controls can disable networking and map folders/run startup commands, but writable mapped-folder modifications persist on the host; guarded staging/outbox/copy-back would still be required on a supported edition.
- Kept the current experimental process sandbox fail-closed when Rust cannot canonicalize a selected workspace/config path. No broader filesystem grant or live-workspace fallback was introduced. ADR-LOG-RESTRUCT-0262 records the disposition and official Microsoft documentation.
- Re-ran `cargo test -p arcana-core --lib analytics_retention_serializes_with_an_active_database_writer --locked -- --nocapture --test-threads=1` and `cargo test -p arcana-core --lib hook_log_retention_serializes_with_an_active_database_writer --locked -- --nocapture --test-threads=1`; each passes 1/1 on Windows. These tests already existed and remain platform-neutral, but this host run is not non-Windows execution evidence.
- P4-11/P4-61 and TG4-18/TG4-31 remain open for guarded staging/copy-back, locked dependency staging, Windows production Cargo path resolution, Linux production isolation, and signed macOS XPC/staging. P4/TG4 totals remain 32/59 and 10/33. The owner Calendar workspace was not accessed.

## Continuation — 2026-10-04 Memory database startup observability

- Added result-aware `memory_database_initialize` and `memory_database_migrate` operation timings around the production Memory database opener and schema ladder. Both use the fixed logical label `database=memory`; no paths or error prose are emitted.
- The isolated production open/migration success test and forced open-failure redaction test pass 2/2 with `cargo test -p arcana-core --lib memory_database_ --locked -- --nocapture --test-threads=1`. ADR-LOG-RESTRUCT-0263 records the contract.
- This is one startup instrumentation slice, not complete Memory/SQLite producer coverage. TG4-24 remains open, no P4/TG4 count changes, and Calendar remains owner-run.

## Continuation — episodic Memory timestamp ordering

- `EpisodicStore::get_session_timeline` and `get_recent_summaries` sorted mixed legacy SQLite timestamps lexically. A legacy space separator sorts before `T`, which can place a later instant before an earlier canonical timestamp.
- Both production readers now order with `julianday(created_at)` and a stable ID tie-breaker. A file-backed isolated test writes through the production episodic methods, seeds mixed legacy/canonical rows, and verifies parsed instants and chronological order through both production readers.
- `cargo test -p arcana-core --lib production_timeline_and_summary_readers_order_mixed_timestamp_formats_by_instant --locked -- --nocapture --test-threads=1` passes 1/1; `cargo fmt --all -- --check` passes. This is one additional P4-57/TG4-19 reader-pair slice; the complete timestamp inventory and gate remain open. No task/gate checkbox or overall count changes.

## Continuation — concurrent provider-consent revocation

- Added a deterministic fake-provider test that pauses after an authorized request reaches the provider, revokes the persisted workspace grant, and then starts a second request through the production `ConsentProvider` wrapper.
- The test proves the already-authorized request remains in flight, while the new request is denied before provider dispatch. `cargo test -p arcana-core --lib concurrent_revocation_blocks_the_next_request_without_rewriting_in_flight_authorization --locked -- --nocapture --test-threads=1` passes 1/1.
- This closes the provider-wrapper revocation-race test gap only. Live desktop IPC, complete cross-consumer allow/revoke readback, and desktop smoke remain open; no P4/TG4 checkbox or count changes.

## Continuation — 2026-10-04 P4-59 cross-platform active-writer gate

- CI's existing Windows/Linux/macOS build-smoke matrix now runs one focused filter covering the production analytics and hook-log retention races against an active immediate SQLite writer transaction.
- `cargo test -p arcana-core --lib serializes_with_an_active_database_writer --locked -- --nocapture --test-threads=1` passes 2/2 locally on Windows. Linux and macOS results are pending CI; P4-59/TG4-21 remain open until those runs pass.
- No P4/TG4 counts changed. The owner Calendar workspace remains untouched.
