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
