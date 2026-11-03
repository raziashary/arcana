# P4 session checkpoint — 2026-10-02

## Current state

P4 is still active and not ready for the owner's Calendar acceptance. The active closeout objective is to finish every unchecked P4 task and TG4 gate, prove the production workflow, and leave only P4-49 for the owner. Do not access `C:\Playground\arcana-calendar`.

The checkout is `C:\arcana` on `main...origin/main` with a very large pre-existing dirty worktree (152 changed tracked files plus untracked files when resumed). No commit or push has been made in this continuation. Preserve all unrelated changes and stage by logical unit only after the relevant work passes its gates.

## Isolated production CLI Fleet attempt

A disposable profile/workspace was created at:

- `C:\Users\razia\AppData\Local\Temp\arcana-p4-fleet-smoke-20261002\home`
- `C:\Users\razia\AppData\Local\Temp\arcana-p4-fleet-smoke-20261002\workspace`

The workspace contains a minimal Rust library fixture and its own empty Git repository. No owner Calendar files or normal `~/.arcana` databases were used. The real `target\debug\arcanactl.exe fleet run` path was invoked with provider `google`, model `gemini-3.8-flash`, explicit workspace consent, and bounded 900-second / 20,000-token / $0.50 / 8-turn Supervisor limits. All seven recipe-bound workers were initialized and their configured model read back as `gemini-3.8-flash`.

The first Supervisor provider request failed with Google `429 RESOURCE_EXHAUSTED`: the response identified the Gemini free-tier per-day request quota with limit 20 and a retry delay. Arcana persisted epic #1 in `ready`, zero child tasks, and `provider_quota_exhausted` on the Supervisor run. The CLI returned nonzero and shut down the scheduler. Do not retry until quota is available; this result proves production startup and failure handling, not a successful task or build.

The isolated CLI read back epic #1, all seven workers, the log-file list, and structured log tail. Workspace consent was revoked and a follow-up status read returned `granted: false`. No Arcana process or listener remained. Preserve the temp folder for log inspection; do not treat its unfinished fixture as a code result.

The structured event file reached 26,633 JSONL records / 6,977,969 bytes during the short Fleet and diagnostic session; 26,614 were `database_statement` DEBUG events; 26,174 reported `duration_ns: 0` and only 10 were at least 10 ms. This is a measured observability-volume and timing-resolution concern: evaluate the Logs tab's 2,000-event tail, three-second refresh, full export, and retention behavior under this volume before declaring P4-50 green. Logs CLI listing/view itself successfully consumed the generated file.

## Recent code and verification evidence

The 2026-10-02 query-plan work now has production-reader proofs for the dashboard worker summary, diagnostics, latest task build evidence, QC transition guard, and epic/phase progress reads described in the MDP and `_verify/P4-58-QUERY-INVENTORY.md`. Migration 14 adds measured epic and phase-order indexes based on a populated 5,000-task baseline/candidate plan regression. Focused Rust tests for those changes passed. `cargo fmt --all` was run after source edits; rerun `cargo fmt --all -- --check` and `git diff --check` before any phase commit.

Python project intent is owner-confirmed as standard-library `python -m compileall -q` for Build plus a separate test run for additional confidence. The checked-in fixture passes compileall and pytest. Production Python launch still fails before spawn because AppContainer cannot grant access to the installed `C:\Python314` runtime; staging a runtime under the workspace versus selecting a separate user-owned Python installation remains an unresolved owner choice. Keep P4-11/TG4-18 and P4-61/TG4-31 open.

The provider key was read from `.env`, validated through Google's models endpoint, and stored in the authorized Google vault entry without displaying or logging the secret. CLI used the vault entry. The live CLI request quota is currently exhausted; no additional provider attempt is warranted.

## Open owner decision and remaining work

~~A clarification is still pending on P2 daily spend enforcement: preferred atomic conservative per-call reservations versus serialized calls with possible one-request overshoot versus the existing best-effort check. Do not implement a cap policy until the owner answers.~~ **Superseded 2026-10-02:** the owner selected atomic conservative per-call reservations with fail-closed unknown cost/config state and reconciliation against provider-reported usage. See MDP P2-21, TG2-17, and ADR-LOG-RESTRUCT-0165. P2/TG2-17 remains open until production provider-boundary and failure evidence passes.

Continue remaining P4 work from `MASTER_DEVELOPMENT_PLAN.md`, `TESTING_GATES.md`, `READINESS_MATRIX.md`, and `GOAL.md`. P4-58/TG4-20 closed on 2026-10-02 after final source/evidence review; P4-50/TG4-24 still needs producer-to-Logs-UI, metrics, traces, and volume evidence; sandbox/runtime staging, long-command end-to-end recovery, cross-platform gates, and cross-phase audit issues remain open. Do not mark P4 complete until each applicable task and gate has direct evidence.

The only intentionally owner-reserved acceptance is the Google Calendar-like desktop Fleet task in `C:\Playground\arcana-calendar`. Once engineering gates clear, tell the owner when they can launch `npm run tauri dev`; analyze their returned logs and task results, then refine any concrete defects.

## 2026-10-02 continuation — official OS sandbox options reviewed

The owner authorized evaluating an alternate Windows sandbox while preserving
external-user-file and network denial. Official platform documentation gives
these constraints for the next implementation decision:

- Linux Landlock can deny ungranted filesystem read and write actions. Filesystem
  read controls are represented in the kernel's Landlock ruleset; TCP port
  connect/bind restriction requires ABI 4, while the current Arcana Linux layer
  handles write rights only and requires ABI 3. Runtime ABI detection and
  fail-closed read/network rules are required before claiming P4-61 there.
  Reference: https://docs.kernel.org/userspace-api/landlock.html
- Microsoft documents an `Experimental_CreateProcessInSandbox` API in
  `processmodel.dll` with AppContainer, explicit read-only/read-write filesystem
  roots, and network policy. The name and documented API are explicitly
  experimental; API availability, path canonicalization, runtime compatibility,
  and exact toolchain closure have not been proven on this host. It therefore
  is a candidate for a prototype, not an adopted replacement.
  Reference: https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox
- Apple's supported App Sandbox is entitlement-based. Apple documents that
  directly launched helpers inherit the app's sandbox and that distinct
  capabilities require an XPC service, login item, or helper app. The current
  `sandbox-exec` profile only restricts writes, and no supported per-command
  read/network sandbox has been proven in this repository.
  References: https://developer.apple.com/documentation/security/discovering-and-diagnosing-app-sandbox-violations
  and https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app

These sources do not establish one production-ready replacement across all three
platforms. In particular, the Microsoft API still creates an AppContainer, so
whether it avoids the current DOS-volume canonicalization failure is unknown and
must be tested before selection. Keep P4-61/TG4-31 open. The next architectural
choice is whether to (a) preserve fail-closed AppContainer behavior on Windows
until a supported compatible toolchain path exists, (b) prototype the
experimental Microsoft API and accept its version-support constraint only if a
real Rust/Node/Python corpus passes, or (c) design a stronger VM/helper boundary
for unsupported Windows environments. Do not weaken external-read/network
denial to make builds pass.

## Continuation after this checkpoint

On 2026-10-02 the owner reconfirmed the Python command contract: standard-library `compileall` is Build evidence, and a separate Python test run adds confidence. The exact fixture passes both; this does not resolve the production AppContainer runtime-access failure. ~~The choice between staging an Arcana-managed runtime and selecting a user-installed runtime is still pending.~~ **Superseded 2026-10-02:** stage the exact runtime first, then fall back to a user-owned Python install if staging fails; preserve the external-user-file/network denial boundary. Implementation and production proof remain open under P4-61/TG4-31.

The owner also approved exact, temporary, non-inheriting `FILE_TRAVERSE | FILE_READ_ATTRIBUTES` grants on the selected Clippy config path's directory ancestors, including those above the workspace and excluding the volume root. This supersedes the earlier workspace-only rollback in ADR-LOG-RESTRUCT-0143 and is recorded as ADR-LOG-RESTRUCT-0160. The ACL-shape test later passed 1/1. A production Clippy rerun still fails with canonicalization denial; see the later continuation below for the 78.29-second result and the confirmed AppContainer namespace limitation.

A fresh read-only P4-58 audit found three omitted P2 fleet/dashboard readers: `TasksDb::list_qc_claims`, `TasksDb::get_daily_budget_summary`, and `WorkersDb::list` (the roster used by routing/assignment). Their production SQL needs populated plan and result coverage, with measured indexes only where justified. The P4-58 inventory, MDP, and TG4-20 notes now name these omissions. **Correction:** the adjacent daily-cap policy note was stale when written; the owner selected atomic conservative reservations later on 2026-10-02. See P2-21, TG2-17, and ADR-LOG-RESTRUCT-0165.

## 2026-10-02 continuation — query audit and Clippy rerun

A second fresh P4-58 reader audit identified task budget status, task dependency unblocking/blocker status and epic-completion reads, daily-cost point lookup, and worker lookup by slug. Their SQL is now shared with production methods and the populated regressions. Passing focused evidence: `cargo test -p arcana-core task_budget_and_unblock_readers_use_existing_indexes_with_populated_tasks --lib` (1/1), `cargo test -p arcana-core worker_roster_uses_measured_order_index_and_returns_full_non_deleted_roster --lib` (1/1), and `cargo test -p arcana-core dashboard_daily_cost_range_uses_primary_key_index_with_populated_rows --lib` (1/1). The task test covers the real legal assignment/build-evidence/QC/done lifecycle, dependent-task release, primary-key readers, epic index, and the existing status index. SQLite continued to prefer `idx_tasks_status` over the proposed partial unblock index, so the migration was removed. The dashboard now reads yesterday's daily-cost row once for both the cost and token totals. Inventory, TG4-20 notes, MDP history, readiness history, and ADR-0162 were updated. P4-58/TG4-20 stay open for a full source audit and enforceable registration.

The exact ancestor ACL-shape test had already passed. After moving the Clippy probe manifest to the selected workspace root, the production test still failed in 78.29 seconds. The Rust probe reports `canonicalize()` AccessDenied for both the readable config directory and file; metadata and config-byte reads succeed. Cargo additionally cannot create the workspace `target` directory. The test fixture cleanup and AppContainer lease use RAII, but no direct post-drop ACL inspection exists yet. ADR-0160's owner-approved permissions remain unchanged. Do not broaden them; isolate workspace-write and path-canonicalization behavior before another full Clippy run. The outer-ancestor path was exercised in the approved scope; no Playground or Calendar data was accessed.

P4-58 2026-10-02 follow-up: the exact-query registry now fingerprints all 79 statically resolved production SELECT/WITH queries across six scoped modules; the integration test passes 2/2. Populated plan/result tests pass for `RunsDb::latest_worker_run_owner/latest_worker_ticks`, token totals/groups/top sessions, `TasksDb::get_blocked_tasks` behavior, `PlansDb::claim_next_item/check_dependencies`, and `WorkersDb::get_by_id`. The blocked-task plan currently scans the task table, so a measured candidate-index comparison is in progress; its gate status remains open. Source callers were audited: planner presentation/readback helpers and unused `RunsDb::get/recent` are bounded exclusions; worker slug uniqueness is admin-only. ADR-0168/0169 record these refinements. Remaining P4-58 open rows are worker-stat result reads, ACP message/request/success-rate reads, transition embedded reads, and any final in-scope source gap. `cargo fmt --all -- --check`, `git diff --check`, TypeScript `npx tsc --noEmit`, and the Logs panel suite (11/11) pass; full P4-58 and the larger P4 gates remain open.

## 2026-10-02 continuation — Windows AppContainer canonicalization boundary

The failed Clippy probe now matches a documented Windows AppContainer limitation: Rust `std::fs::canonicalize` uses `CreateFile` plus `GetFinalPathNameByHandle`, and the DOS-volume form requires access to `\GLOBAL??`/the Mount Manager, which AppContainer denies independent of filesystem ACL grants. Microsoft’s STL issue reproduces the same `ERROR_ACCESS_DENIED` and NT-volume fallback behavior: https://github.com/microsoft/STL/issues/6286. Rust’s public docs also describe the Windows implementation in terms of these APIs: https://doc.rust-lang.org/stable/std/fs/fn.canonicalize.html. Do not add broader ancestor ACLs; they cannot grant access the kernel denies.

The separate `workspace\\target` creation error is not explained by this canonicalization limitation and needs an isolated workspace-write probe. A request is pending for whether P4 should evaluate a different Windows sandbox while retaining external-file and network denial, or preserve AppContainer and fail closed for incompatible toolchains. Until that decision, leave the sandbox architecture unchanged and keep Windows Rust corpus proof open.

The existing `appcontainer_allows_workspace_writes_and_blocks_sibling_writes_from_child_tree` regression now also creates `workspace/target` and writes a nested file. `cargo test -p arcana-core appcontainer_allows_workspace_writes_and_blocks_sibling_writes_from_child_tree --lib -- --nocapture` passes 1/1 in 0.40 seconds. This proves ordinary AppContainer nested-directory creation in its isolated Temp fixture; it does not explain the different `workspace/target` denial from the production Cargo fixture under `C:\arcana\target\p4-acl-tests`, which still needs isolation.

## 2026-10-02 continuation — monthly coverage, daily reservations, and result events

The owner-selected worker-stat contract is now implemented in the core writer and readers: same-day ticks accumulate; rows older than 90 days roll into monthly summaries in the same immediate transaction that prunes daily rows; the combined reader includes recent daily rows and monthly summaries with actual coverage bounds. `get_worker_analytics` now exposes coverage and granularity, and `arcanactl analytics workers` displays them; JSON export includes the same metadata. `cargo test -p arcana-core worker_stats_accumulate_roll_up_atomically_and_read_with_coverage --lib --locked` passes 1/1 and `cargo test -p arcana-shared worker_stats --lib --locked` passes 2/2. P4-59/TG4-21 remain open for migration preservation/failure injection, lifecycle and active-writer proof, full export readback, exact cutoff verification, and aligned retention of the other append-only tables.

The owner-selected daily-cap policy is implemented as an immediate-transaction reservation ledger using conservative model/context pricing before provider dispatch, with reported-usage settlement and full holds for ambiguous usage. Focused ledger tests previously passed 8/8; a newly added disabled-cap → uncapped spend → re-enabled regression proves the reservation transaction reconciles the larger legacy cumulative total without double-counting. Independent review found that worker-pause errors after a denial were swallowed; the path now emits a structured `daily_budget_pause_failed` event and returns a sanitized budget stop rather than claiming that zero workers were paused. `provider::capped::tests` pass 4/4. P2-21/TG2-17 remain open for provider unary/stream success/error/drop integration, restart proof, price upper-bound evidence and kill-switch/broadcast behavior.

The owner-selected operation-level result events now distinguish logical DB operation outcomes from rusqlite's duration-only statement callback. `OperationTimer` carries a fixed logical database label; worker-stat rollup/range and graph build/node/edge/FTS/flush operations emit outcome, duration and result counts. Memory recall now marks unavailable and lexical-fallback results, and Logs counts `fallback` among degraded outcomes. This is partial coverage, not a full production DB/Memory/graph audit or Logs UI consumer proof; P4-50/TG4-24 remain open. The AST query registry now scans the six scoped production files and lists discovered owners with their current proof status in `_verify/P4-58-QUERY-INVENTORY.md`; TG4-20 remains open because several discovered readers still need populated plan evidence and explicit scope classification.

The next independent P4-58 review found three inventory claims that overstated paired production-result proof: latest worker run owner/tick reads had plans but no method readback; token totals/group/top-session plans had no production aggregate readback; and the blocked-task board's owner SQL differed from the SQL tested by the unblock path. The owning modules now have production result assertions, and their exact populated tests pass. The initial owner-only registry was strengthened to exact normalized SQL fingerprints across the six scoped modules; `cargo test -p arcana-core --test p4_query_registry --locked` passes 2/2 with 79 registered query fingerprints. The inventory records the added evidence while leaving unproven plan/result rows open. ADR-0168 records the stronger registration contract.

Focused tests pass for the changed Rust logic. `cargo check -p arcana --lib --locked` and `cargo check -p arcanactl --locked` pass after an initial invalid feature name was corrected; warnings remain in the pre-existing workspace. The exact focused re-run, formatting check, frontend typecheck, and diff check remain to be executed after this continuation's doc and registry updates. No commit or push has been made in this continuation. Calendar acceptance remains owner-run and untouched.


## 2026-10-02 continuation — worker-stat and ACP reader proofs

The owner clarified the historical precision requirement as **expose monthly coverage**. The production worker-stat readers now have populated-plan/result proof in `worker_stats_readers_use_populated_indexes_and_bound_daily_monthly_results`: point reads use the unique worker/date index; range reads explain daily and monthly SQL, verify worker/date bounds and daily/monthly composition, and return actual monthly coverage bounds. Exact command: `cargo test -p arcana-core --lib worker_stats_readers_use_populated_indexes_and_bound_daily_monthly_results --locked` (1 passed). P4-59 remains open for migration preservation/failure injection, lifecycle cadence and active-writer safety, export readback, exact cutoff, and aligned append-only table retention.

ACP query proof now includes `p4_open_task_readers_use_populated_plans_and_production_results` (`cargo test -p arcana-core --lib p4_open_task_readers_use_populated_plans_and_production_results --locked`, 1 passed). It uses one isolated migrated DB to explain both worker-message/request point reads and the success-rate aggregate, calls the production readers, checks exact persisted fields, missing IDs, aggregate groups/ratios, and excludes pending/manual routing rows. The message reader's send/ack consumers and the request readback consumer were source-audited. No Rust production caller was found for `get_worker_success_rates`; therefore its routing scope is an explicit open gap even though its query plan and result semantics are tested. The existing QC lifecycle evidence test already covers its production transition.

Inventory, TG4-20, MDP history, and readiness now reflect these results. P4-58/TG4-20 remain open for the embedded shared lifecycle transition reads, disposition or production wiring for success-rate aggregation, and any final source-discovered in-scope readers. The blocked-task table scan is now addressed by migration 17: on 5,000 tasks with 128 eligible rows, the production plan uses `idx_tasks_blocked_tasks_plan` and the warm-cache median fell from 3.012 ms to 2.034 ms (about 32%) with identical results. `cargo test -p arcana-core --lib task_budget_and_unblock_readers_use_existing_indexes_with_populated_tasks --locked` passed; measurement is reported without a timing assertion. Do not call the SQL registry complete proof: it enforces fingerprints, while each reader still needs paired populated-plan/result evidence or a bounded exclusion.


## 2026-10-02 continuation — measured blocked-task index

The `get_blocked_tasks` board reader was the remaining observed full task-table scan. Its isolated 5,000-task fixture contains 128 eligible nonempty blocker rows. Across five warmups and 31 measured production-reader calls, median latency was 3,011,600 ns without the candidate and 2,034,100 ns with it; the plan changed from `SCAN tasks` to `SCAN tasks USING INDEX idx_tasks_blocked_tasks_plan`, and returned task IDs/status/blocker lists were identical. This is fixture-specific evidence and the test asserts plans/results, not timing. Migration 17 installs the partial index and the regression verifies the migration-created index, drops it only in the isolated fixture for baseline comparison, then re-creates the candidate index. Formatting was applied after test-agent edits.


## 2026-10-02 continuation — Logs failure and zero-resolution metrics

An independent P4-50 audit found that the Logs `Failed outcomes` headline summed a fixed frontend outcome allowlist, while production tool events carry `is_error=true` without an `outcome`; the backend's per-event failure aggregate already counted these correctly. MCP errors also use stable values such as `connection_refused` and `invalid_response` that were not classified. The metric now sums backend per-event failures, the backend classifies MCP failure codes, and tests use production-shaped tool events with no outcome plus an MCP connection failure. The Logs UI suite passes 12/12 and `cargo test -p arcana --lib daily_metrics_aggregate_all_valid_events_and_skip_malformed_lines --locked` passes 1/1.

The same audit found that zero-nanosecond SQLite/profile samples were excluded from duration counts and rendered as generic unavailable data. Metrics now retain a separate `duration_zero_count` for event/provider aggregates, and the UI reports those samples as below timer resolution while continuing to show true missing samples as unavailable. The existing zero-duration aggregation regression checks the separate count. This clarifies resolution limits; it does not improve the underlying timer resolution. P4-50/TG4-24 remain open for full real Fleet producer-to-Logs readback, broader production DB/Memory operation coverage, and meaningful statement timing evidence.


## 2026-10-02 continuation — expanded reader registry and macOS XPC approval

After the owner approved expanded TG4-20 scope, the AST source registry was reconciled to seven production modules, including shared CLI analytics, and embedded SELECTs inside mutation statements. The exact query inventory now covers finite `token_by_group` variants, diagnostic count constants, task/ACP branches, and shared CLI cost/token/daily/monthly readers. `cargo test -p arcana-core --test p4_query_registry --locked` passes 2/2 and `cargo test -p arcana-shared --lib cli_analytics_readers_use_populated_indexes_and_return_production_results --locked` passes 1/1. This proves registration and the specific CLI reader family, not every in-scope plan/result. Embedded dispatch/lifecycle SQL and the worker-progress maintenance read remain open for measured proof or an explicit bounded disposition.

The owner approved a separately signed App Sandbox XPC runner for macOS project commands and subsequently approved sandbox-owned workspace staging/copy-back if direct descendant access fails (ADR-0170/0171). A Luna architecture review found that bookmark-derived access is not inherited by arbitrary compiler descendants. The staging route must validate workspace input and symlinks and return only validated changes beneath the selected workspace. Do not claim TG4-31 complete on service startup alone. This host is Windows, so macOS signing/package and descendant negative tests remain unexecuted. The Windows alternative review likewise found no ready production replacement for AppContainer; Windows Sandbox CLI lacks process I/O and session-independent support, while the experimental API retains AppContainer limitations. Linux, Windows, and macOS strict sandbox proof therefore remains a P4 blocker.

## 2026-10-02 continuation — owner decision and embedded query proof

The owner approved sandbox-owned workspace staging/copy-back for the macOS XPC path if direct workspace descendant access fails. ADR-0171 limits the return boundary to validated changes beneath the selected workspace; XPC packaging and all macOS evidence are still open.

For TG4-20, the exact registry and shared CLI analytics tests pass. A 700-row assignment test now proves the worker-active guard selects `idx_tasks_assigned`, idle assignment succeeds, and a second task is refused while the worker is active. The first worker-progress retention test failed by exposing a global scan in its nested query. A baseline/candidate plan regression justified runs DB migration 3 (`worker_run_events(run_id, id DESC)`); the re-run proves the run-scoped index and keeps exactly the newest 500 rows without altering a second run. Focused tests pass. Remaining claim/status/lifecycle embedded plans and final source audit continue to block TG4-20. No code has been committed or pushed in this continuation.

The owner approved sandbox-owned staging/copy-back for the macOS XPC runner if direct descendant workspace access fails. This authorizes the architecture but does not close P4-61: the implementation must validate the staged input tree and symlinks, deny child network/external reads, and copy back only validated changes beneath the selected workspace.

The first full `cargo test -p arcana-core --lib --locked` run completed with 794 passed and 9 failed. Stale worker migration/index-plan expectations, a broad shell-character assertion that rejected a literal Python regex, and shared analytics test-provider aggregation were corrected; targeted tests now pass for the worker migration fixture, fleet recovery query plan, provider retry reader, command resolver, and production build-evidence/QC flow. The project-command corpus was moved under the repository-local ACL fixture root to avoid mutating protected `C:\Users` ACLs. Windows Cargo output now targets the selected workspace `.arcana-cache/cargo-target`, incremental compilation is disabled for sandboxed commands, and the workspace-only inherited capability includes file delete for Cargo cleanup. The production Rust corpus now completes Build and Test and reaches Lint; Clippy then fails because it cannot canonicalize its readable workspace config under AppContainer. This proves the target-creation and temporary-file cleanup fixes only. A probe to add the volume-root traversal ACE was denied by Windows and was reverted; no root grant is retained. P4-61 remains open for a production-safe Windows boundary, exact Python runtime staging/fallback, dependency staging, and Linux/macOS signed sandbox implementations and evidence. No provider smoke or Calendar test was run.

The Windows alternate-runner evaluation found this host is Windows Home Single Language, and Windows Sandbox is not supported on Home per Microsoft documentation. The newer `wsb` CLI also documents no stdout/stderr retrieval and an active-session requirement for `ExistingLogin`; mapped-folder writes persist to the host. It is not a usable alternate on this machine, so Windows AppContainer stays fail-closed where its config canonicalization/runtime policy cannot be proven. ADR-0174 records the evaluation and host evidence.

## 2026-10-02 continuation — owner-approved macOS staging and P8 hardening

The owner approved sandbox-owned workspace staging/copy-back for the signed
macOS XPC runner when direct descendant access fails. ADR-0171 now defines the
return boundary: canonical selected workspace, reject symlinks/special files,
traversal, identity changes, and retargeting; derive a bounded changed-path
write set; check destination components without following links; use atomic
replacement; and never write outside the original workspace. The existing
default-allow/write-only `sandbox-exec` wrapper could not satisfy P4-61, so the
production macOS launch path now fails closed with `Unsupported` until the XPC
runner is packaged. A macOS-only regression was added but cannot run on this
Windows host; target compilation and macOS denial tests remain open. ADR-0175
records the security fix. The MDP history, TG4-31 and readiness now state that
staging is owner-approved but not implemented.

Independent P8 review found two bypasses and one missing staging guarantee:
the Bash Git guard's five-token look-ahead missed commit after multiple global
options; commit-message validation accepted an all-whitespace scope; and an
explicit ignored build output had no refusal proof. The guard now scans all
remaining tokens, scope validation rejects blank values, and `git_commit`
rejects paths Git reports ignored before changing the index. P8-74/TG8-13/S8-06
were reopened while their current production-path evidence is rerun. Focused
proof passed: `cargo test -p arcana-core --lib git_mutation_guard_tests
--locked` (1/1), `cargo test -p arcana-core --lib
commit_message_requires_conventional_subject_and_body --locked` (1/1),
`cargo test -p arcana-core --lib
commit_refuses_an_explicitly_named_ignored_build_output --locked -- --nocapture`
(1/1), and `cargo test -p arcana-core --test s8_git_safety --locked
-- --nocapture` (1/1), including the new Git-global-options refusal through
the real Bash tool. The Tauri TG8-13 consumer check passed
(`cargo test -p arcana --lib
ci_gate::tests::tg8_13_host_commit_safety_is_registered_and_used_by_workers
--locked -- --nocapture`, 1/1). P8-74, TG8-13, and S8-06 were re-closed in
the plan, gate register, and readiness matrix based on these current-tree
checks. The full 801/2 core suite limitation below remains an open P4 Windows
sandbox issue and does not invalidate the focused P8 checks.

Fresh current-source validation: `cargo test -p arcana-core --lib --locked`
finished 801 passed / 2 failed in 263.41 seconds. The only failures are the
known Windows production Clippy-config/corpus failures: AppContainer permits
metadata and config-byte reads but Rust canonicalization returns `Access is
denied`; the dependency-free Rust corpus passes Build and Test then fails
Clippy at the same config boundary. Seven stale schema/assertion failures from
the earlier 794/9 run are fixed. `npm run typecheck` passes. This full suite ran
before the newest P8 and macOS fail-closed edits; focused P8 and follow-up
observability/query checks cover those incremental changes. Current formatting
and `git diff --check` pass; Git reports only its expected LF-to-CRLF working
copy warnings.

Independent read-only TG4-20 review confirms registration and populated plans
are separate. Follow-up plan tests now prove the embedded active-task guard in
`claim_task_with_pid` uses the existing status/worker index, and that both epic
completion trigger predicates use the measured epic index on a 5,000-task
fixture. The QC trigger uses the populated latest-build-evidence query proof.
The registry now fingerprints task migration SQL containing lifecycle reads;
the exact-registration test passes after adding the two lifecycle migration
fingerprints. SQLite hides trigger subprogram plans from `EXPLAIN QUERY PLAN
UPDATE`, so the epic predicates are explained directly. Remaining embedded
variants, stale fingerprints, and the complete scoped source audit remain
open. P4-58/TG4-20 remain open. Independent P4-50
review also found uncovered shared CLI
analytics statement profiling, no real producer-to-Logs UI readback,
task-status/MCP-call event gaps,
coarse timer resolution and high event volume, and partial logical DB result
events. P4-50/TG4-24 remain open; reconcile those gaps before claiming
observability ready.

The independent cross-phase P1/P2/P3/P4/P8 review found one new high-severity
production defect and one stale status label in `GOAL.md` (corrected above).
After an earlier passing build, a later `project_command` timeout records only
telemetry and skips build evidence persistence; QC could then reuse the stale
pass after workspace changes. The owner approved workspace-fingerprint
binding. Additive task migration 18 stores canonical workspace path and the
pre-build SHA-256 fingerprint on observed build exits. Both the application QC
reader and SQLite trigger recompute the fingerprint and reject changed,
unreadable, or missing workspace state. Timeout/spawn errors remain distinct
from observed nonzero exits, create no synthetic evidence, and do not change the
build-failure streak. The production pass → edit → timeout → API/direct-SQL QC
refusal → exact restoration → QC acceptance test passes, as do the focused
fingerprint, legacy-schema migration, and task-status API tests. P4-14 stays open for complete
cross-platform process-exit corpus proof. The fingerprint excludes generated
trees `.git`, `.arcana-cache`, `.arcana-tmp`, `.mypy_cache`, `.next`,
`.pytest_cache`, `.ruff_cache`, `.turbo`, `.venv`, `__pycache__`, `coverage`,
`dist`, `node_modules`, `target`, and `venv`, while hashing remaining files
and lockfiles; any traversal/read/change error fails the QC guard closed.

The corrected full `cargo test -p arcana-core --lib --locked` run completed
804 passed and 2 failed in 240.18 seconds. Both failures are the known Windows
AppContainer Clippy configuration canonicalization denial in the production
Rust corpus and its focused Clippy test: metadata and config-byte reads succeed,
but `canonicalize()` returns Access denied. The previous synthetic evidence
helper failure is fixed; `cargo test -p arcana-core --lib
agent::builtin_tools::build_evidence::tests --locked` passes 9/9. The
production project-command diagnostic guidance test also passes. No full-suite
green claim is made until the Windows sandbox boundary is resolved.

After that full run, `cargo check -p arcana-core --locked`,
`cargo test -p arcana-core --test p4_query_registry --locked` (2/2),
`npm run typecheck`, `npm test -- --run src/tabs/Logs.test.tsx` (12/12),
`cargo fmt --all -- --check`, and `git diff --check` passed. The last two
format/diff checks report only expected Git LF-to-CRLF working-copy warnings.

A relative `CLIPPY_CONF_DIR` was also tested as a narrow workaround for the
AppContainer path-handle denial. Its path-shape unit passed, but the production
Clippy sandbox test still returned `Access is denied` from `canonicalize()`
for both the directory and config file after 74.53 seconds. The experiment was
reverted and recorded as ADR-0177; the absolute, host-canonicalized config path
remains in production. Windows Sandbox is unavailable on this Windows 10 Home
host (ADR-0174), so a supported alternate production sandbox is still an open
P4-11/P4-61 dependency.

The independent reviewer confirms the documented P4-50, P4-61, CLI Fleet, and
P8 boundaries otherwise match their current evidence; no successful Fleet
completion is implied.

The provider/model aggregate now retains and displays the producer's
`result_blocks` count. The focused Rust aggregation test passes (1/1), the
Logs UI suite passes (12/12), and `npm run typecheck` passes. This is one
reconciled metric field; the remaining producer, database profiling, and
volume findings stay open.

No provider smoke was retried; Gemini free-tier quota remains exhausted. No
Calendar workspace was accessed. No commit or push has been made.

### Python sandbox continuation — 2026-10-02

ADR-0178 implements exact Windows Python runtime staging. The selected PATH
CPython 3.14.7 standard library, DLLs, and pytest 9.1.1 import closure are
copied into `%USERPROFILE%\\.arcana\\python-runtimes`, isolated with a generated
`python314._pth`, content-verified, and granted read/execute access only at the
staged runtime tree. `PYTHONHOME`, `PYTHONPATH`, `PYTHONUSERBASE`, and
`PYTHONSTARTUP` are removed from the child environment; pytest plugin autoload
is disabled. Workspace bytecode/cache remains inside the selected project.

Production proof now passes:

- `platform::python_runtime::tests::staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling` proves staged pytest import, runtime-write refusal, denial of a synthetic sibling secret, and denial of a controlled loopback TCP connection through the Windows AppContainer launcher.
- `agent::builtin_tools::project_command::tests::production_python_build_and_tests_use_the_staged_runtime` runs the checked-in Python corpus through the production resolver/launcher; `compileall` Build and pytest Test pass, with one test passed.
- The measured cold production test took 42.61s; a verified on-disk cache reuse test took 9.14s. Process events now expose `runtime_prepare_ms` separately from sandbox preparation, command wait, and output drain.

One cache-integrity defect was caught and fixed: host pytest probing created
bytecode after the first content digest. The host probe now disables bytecode
writes, and verification rejects symlinks before excluding disposable `.pyc`
files and `__pycache__` directories from the immutable-code fingerprint.

P4-11/TG4-18 and P4-61/TG4-31 remain open. No explicit user-owned Python
fallback selector exists; current `%USERPROFILE%\\AppData\\Local\\Programs\\Python\\Python314`
does not contain `python.exe`, and the selected `C:\\Python314` prefix is not a
user-owned ACL fallback. Python lint is also unavailable (`ruff` is not
installed). The broader unrelated-user-file boundary, full Rust/Node corpus,
Windows Clippy canonicalization issue, Linux/macOS sandboxes and XPC runner,
dependency staging, and trusted denial consumption remain unproven. Windows
Sandbox remains unavailable on this Home edition. The owner’s separate Windows
alternate-runner choice is still pending. No provider smoke or Calendar run was
performed; no commit or push has been made.

### Python runtime trust and integrity follow-up — 2026-10-02

The owner answered the Python trust question: accept a signed standard Python
installation, and add Ruff to the explicit per-install approval/staging flow.
ADR-0179 records the first part. Before any host-side Python inspection,
`find_python_executable` now verifies the candidate with Windows
`WinVerifyTrustEx` using cache-only certificate retrieval and no UI. The
reported prefix must contain its runtime DLL tree; `Lib` must resolve inside
that prefix or to the exact versioned per-user Python `Lib` path observed on
this PC. A synthetic regression rejects a different external target. Python
3.14.7 at `C:\Python314\python.exe` reports a valid PSF Authenticode signature
on this host.

The staged-runtime source key now hashes file contents instead of size/mtime,
omits `site-packages` from the standard-library copy, and uses layout version
5. The copier prunes `__pycache__` before traversal and skips `.pyc`; staged
integrity hashes every copied file. Arcana rechecks the cache tree on process
cache hits, and staged probes/commands pass `-B` so Python cannot mutate the
runtime after verification. No `.pyc` files were found in the runtime caches
after the latest tests. Older test-generated cache directories remain on disk;
they were not removed.

Latest focused evidence after the trust and content checks:

- `cargo test -p arcana-core --lib platform::python_runtime::tests --locked -- --nocapture`: 2 passed, including the staged-runtime external-read/write/network denial and exact `Lib` target regression (38.78s before the Authenticode addition).
- `cargo test -p arcana-core --lib platform::python_runtime::tests::staged_runtime_is_readable_but_not_writable_and_cannot_read_its_sibling --locked -- --nocapture`: passed after Authenticode and process-cache revalidation (58.50s cold stage/sandbox run).
- `cargo test -p arcana-core --lib agent::builtin_tools::project_command::tests::production_python_build_and_tests_use_the_staged_runtime --locked -- --nocapture`: passed after the trust changes; production `compileall` Build and pytest Test both pass (1 fixture test, 15.31s warm run).
- `cargo check -p arcana-core --locked` and `cargo fmt --all` pass. A fresh `git diff --check` exited 0; Git emitted only line-ending conversion warnings.

The host has no Ruff module. The owner-selected Ruff installation/staging work
has not started; project-command installation remains fail-closed until its
host-controlled staging implementation exists. Do not claim Python Lint or
TG4-18 complete. The independent Luna High review is otherwise addressed for
interpreter signing, source-cache identity, the observed junction, and staged
bytecode drift; Ruff closure remains its outstanding finding.

The full core suite has not been rerun after these latest Python edits. Its
most recent full run remains 804 passed / 2 failed at the Windows AppContainer
Clippy-config canonicalization boundary. Other open work remains as recorded
above and in the prior sections: Windows alternate sandbox decision/solution,
Rust Clippy and Node production corpus, Linux/macOS sandbox/XPC proof, locked
dependency staging and trusted-denial consumption, P4-58 enforceable reader
inventory, end-to-end observability/Logs consumer proof, successful CLI Fleet
completion (the last live Gemini run hit quota), and owner-run Calendar
acceptance. No Gemini smoke was retried; the Calendar workspace was not
accessed.

Session was paused at the owner's request at checkpoint time. Goal status was
`paused` then. Checkout is `C:\arcana`, branch `main` at `fff31ec`, tracking `origin/main`; the worktree is
still broadly dirty (157 changed tracked files plus untracked source/tests and
verification documents; about 30,512 insertions / 3,932 deletions in the
tracked diff). Nothing was committed or pushed. Preserve this state and
continue from the open items above; do not reset or clean the worktree. The
owner previously authorized phased commits and a push to `main` after full
verification; no such closeout has happened yet.

### Continued verification — 2026-10-02

The expanded P4 goal is active. No Calendar workspace was accessed and no
provider request was made during this continuation.

The latest full `cargo test -p arcana-core --lib --locked --quiet` run completed
with **809 passed / 2 failed / 0 ignored** in 353.56 seconds. Both failures are
the existing Windows AppContainer Clippy config canonicalization boundary:
`production_clippy_environment_is_visible_and_readable_in_workspace_sandbox`
and the Rust fixture's Lint intent inside
`checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher`.
AppContainer can read config bytes and metadata, but Clippy's config path
canonicalization returns `ERROR_ACCESS_DENIED`; the command fails closed. The
previously rejected relative `CLIPPY_CONF_DIR` experiment is not being revived.
The profile-scoped Python staging test, deterministic startup-cancellation
test, and nested-process timeout/cancellation test pass in this full run. Ruff's
17 MiB extraction bound fixture and the focused staged Python Build/Test,
Ruff extraction, install approval, and arbitrary-package denial tests also
pass. `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and
`git diff --check` pass; Cargo reports existing unused/dead-code warnings.

An alternate Windows API has now received a disposable policy probe. This host
reports ProductName `Windows 10 Home Single Language`, build `26200`; its
`processmodel.dll` exports `Experimental_CreateProcessInSandbox` and
`Experimental_CreateProcessAsUserInSandbox`. A manually encoded `SBOX` v0.1.0
AppContainer spec launched a child; with one selected workspace read/write
grant, the child wrote inside that workspace, failed to read a synthetic
sibling secret, and could not connect to a local TCP listener. This is
host-specific prototype evidence only: it did not test read-only toolchain
grants, Rust/Python/Node execution, descendants, job cancellation, captured
output, timeouts, or a generated schema binding. Microsoft labels the API
experimental and gives Windows 11 experimental as the minimum client. Do not
integrate it into Arcana's production backend or close gates from this probe.
Windows Sandbox and Hyper-V are unavailable on the reported Home edition.
Detailed observations and limits are recorded in
[`WINDOWS-EXPERIMENTAL-SANDBOX-EVAL-2026-10-02.md`](WINDOWS-EXPERIMENTAL-SANDBOX-EVAL-2026-10-02.md).
References: [Microsoft Create Process In Sandbox API](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox),
[Windows Sandbox prerequisites](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-install),
[Hyper-V edition requirements](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/get-started/install-hyper-v).

No phase can close from this evidence. P4-11/P4-61 and TG4-18/TG4-31 remain
open until Rust Lint works through the production sandbox and the broader
Windows/Linux/macOS capability contract is directly proven. P4-50, P4-58,
P4-59, P4-60, P4-51–56, P4-62, successful CLI Fleet completion, cross-phase
audit disposition, and all other unchecked P4 tasks/gates remain open. P4-49
is still reserved for the owner's final desktop Calendar run. No commit or
push has been made; preserve the dirty worktree.

### Resumed continuation — 2026-10-02

The owner resumed the expanded P4 goal; the live goal tracker is `active`.
The owner-only `C:\Playground\arcana-calendar` remains untouched.

A fresh GPT-6 Luna High read-only review found no additional P8 safety blocker
in its static pass and reconfirmed these open boundaries: Windows/Linux/macOS
production sandbox proof; a successful CLI Fleet task (the recorded Gemini 429
quota refusal created no children, so do not retry while quota is exhausted);
P4-50 producer-to-Logs UI/volume evidence; P4-58 complete plan/result audit;
Python Ruff staging; and the remaining failure-containment work. It also found
that GOAL.md understated completed Windows Python runtime staging; GOAL.md and
READINESS_MATRIX.md now distinguish the production staged Build/Test proof from
the still-open Ruff, fallback, locked-dependency, denial-consumption, and
cross-platform work.

P4-58 source registration now discovers Rust modules under
`arcana-core/src/db`, `arcana-core/src/analytics`, and `src-shared/src/db`
instead of relying on seven literal file paths. Five settings, annotation, and
schema-helper modules remain explicit, test-checked scope exclusions. The
inventory, TG4-20 note, MDP, READINESS_MATRIX, PROJECT_MEMORY, and ADR-0180 were
updated. `cargo test -p arcana-core --test p4_query_registry --locked --
--nocapture` passes 4/4; `cargo fmt --all -- --check` and `git diff --check`
pass. This closes only the omitted-source mechanism; populated lifecycle plans
and the remaining reader audit still keep TG4-20 open.

The expanded core test suite has not been rerun after the latest Python runtime
trust changes. The last known full core suite remains 804 passed / 2 failed at
the Windows AppContainer Clippy-config canonicalization boundary. Python's
focused staged production Build/Test tests pass, but Ruff and explicit user-owned
fallback selection are not implemented. No commit or push has been made.

### Resumed continuation — approved Ruff staging (2026-10-02)

The owner-approved Python lint gap now has a narrowly scoped implementation. Generic `project_command(intent="install")` for project dependencies remains fail-closed. Only `intent=install`, `toolchain=python`, `package=ruff` enters the explicit host-tool path and retains the existing per-call `HostApproval` gate (including Autonomous mode); standing approval remains unavailable and headless CLI rejects the effect before handler entry. The host fetches only the pinned Ruff 0.16.8 Windows x86-64 wheel from PyPI, disables redirects, caps the streamed response at 12 MiB, verifies the published wheel SHA-256, and asks the signature-verified Python standard library to extract only `ruff.exe` from the exact wheel member. Arcana cache manifest and executable integrity are rechecked before use; the process sandbox receives only the executable file as a read-only path. Unsupported host architectures fail closed.

ADR-0181 records this contract. MDP P4-11/P4-61, TG4-31, GOAL, and READINESS_MATRIX now describe implementation vs proof accurately. Focused checks pass: Ruff exact-member/traversal extraction test (1/1), unsupported project-package denial (1/1), existing generic install approval/deny tests (2/2), `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` (line-ending conversion warnings only). Ruff wheel URL/hash were verified against official PyPI 0.16.8 metadata and its official install docs were consulted.

The actual Tauri one-call approval, real wheel download/cache stage through that approval, and sandboxed production lint have not yet run. Python Lint and TG4-31 remain open. No app data was changed by installation and no Calendar workspace was accessed. The independent GPT-6 Luna High review of this bounded patch is pending. No commit or push has been made; the worktree remains broadly dirty.

### Resumed verification and P4-58 mutation proof — 2026-10-02

The attached goal was read from C:\Users\razia\.codex\attachments\1a8b4c86-39b1-49c6-a229-64a4f905f7b4\pasted-text-1.txt; the path supplied in the request omitted the .codex separator. The expanded goal still requires all P4 automation/CLI/audit work to pass before only the owner's desktop Calendar test remains. No Calendar directory or Gemini provider was accessed.

A fresh Luna High review identified inconsistent P4-58 inventory wording for three embedded mutation fingerprints. Source review confirmed existing populated EXPLAIN and production-reader coverage for the entire ready-claim UPDATE, task-scoped routing outcome UPDATE, and worker-claim outcome UPDATE. The latter fixture previously had only one candidate pending row. It now seeds an older pending row for the same task/worker and verifies that production completion closes only the newest row and preserves the older row. The inventory exact-fingerprint rows, active MDP/TG4 status text, and readiness summary now accurately cite that evidence; gate remains open for final scoped audit and any unmatched proofs.

Verification after the change: cargo test -p arcana-core --lib p4_task_health_and_routing_outcome_readers_use_populated_plans_and_results --locked -- --nocapture passes 1/1; cargo test -p arcana-core --test p4_query_registry --locked -- --nocapture passes 4/4; cargo test -p arcanactl --test consent_cli --locked -- --nocapture passes 1/1; cargo fmt --all -- --check passes. The prior full core suite remains 809 passed / 2 failed at the Windows AppContainer Clippy config canonicalization path. No commits or pushes were made. Preserve the dirty checkout.

### Linux validation environment check — 2026-10-02

WSL Ubuntu is present but has no `rustc` or `cargo`. The Windows Rust toolchain has `x86_64-unknown-linux-gnu` installed, but `cargo check -p arcana-core --target x86_64-unknown-linux-gnu --locked` stops in `openssl-sys`: this host has no Linux OpenSSL sysroot or cross `pkg-config` setup. This is not a Rust source diagnostic and does not count as Linux compile or runtime proof. No toolchain or host packages were installed. Linux production sandbox/corpus evidence remains open under TG4-18/TG4-31. No ports or provider requests were started.

### Observability and test continuation — 2026-10-02

The current tree now ties `graph_build` parent duration/outcome to its existing
phase `trace_id`; the operation-timer JSON test confirms the field is serialized.
`refresh_memory_md_index` performs filesystem work, so its erroneous SQLite
database label was removed. `Logs.tsx` queues a refresh when a filter/date
change lands during an in-flight read; a delayed-read regression confirms the
latest run correlation is fetched afterward. CLI `read_tail` uses a streaming
reader and a bounded tail deque; a 50,000-line fixture proves the returned tail.

Evidence after these edits: `npm test -- --run src/tabs/Logs.test.tsx` passes
13/13; `npm run typecheck` passes; `cargo test -p arcana-core --lib
observability::timing::tests --locked` passes 3/3; and `cargo test -p arcanactl
--bin arcanactl log_tail_retains_only_the_requested_last_lines --locked` passes
1/1. The complete current core suite finishes at 809 passed / 2 failed / 0
ignored in 382.25 seconds; both failures remain the known Windows AppContainer
Clippy-config canonicalization denial in the focused Clippy test and production
Rust corpus. `cargo fmt --all` was applied after source edits; re-run
`cargo fmt --all -- --check`, `npm run typecheck`, and `git diff --check` before
any commit. These fixes do not close P4-50/TG4-24. Real Fleet producer-to-Logs
readback, full producer coverage, telemetry storage/export failure isolation,
privacy and volume checks, and refresh/export cost measurements remain open.

The alternate Windows process-sandbox direction is still a separate pending
owner decision; no FlatBuffers dependency or non-default runner was added. The
Calendar workspace and Gemini provider were not accessed. No commits or pushes
were made; preserve all existing dirty worktree contents.

### Graph trace failures and SQLite cost measurement — 2026-10-02

Graph phases (`detect`, `build`, `cluster`) now emit a bounded failure event on
error, with the same trace ID as the parent timer and without serializing the
underlying error text. A Tauri library test captures the event and verifies the
safe failure contract (1/1). The Logs trace renderer now shows the phase label;
its focused suite passes 14/14, including parent-plus-phase grouping by trace
ID. The Memory procedural `MEMORY.md` index timer remains free of a false SQLite
database label. CLI tail streaming and filter-race coverage remain green.

SQLite benchmark: `cargo test -p arcana-core --lib
observability::sqlite::tests::measure_profile_callback_overhead_and_duration_resolution
--locked -- --nocapture` passed using 20,000 prepared `SELECT ?1 + 1` reads from
an in-memory connection. Measured elapsed totals were 10,769,000 ns unprofiled,
14,276,100 ns with only the callback, and 483,356,400 ns with JSON event
formatting writing to `io::sink`; 19,998 callback samples were zero and two
were nonzero. The reviewer caught that the initial benchmark used compact
formatting; this run uses JSON formatting. It still excludes the async file
appender and must not be treated as Fleet latency. The bundled SQLite source
computes profile duration from millisecond clock ticks. The owner chose to keep
that native statement granularity and rely on nanosecond logical DB operation
spans; no higher-resolution callback prototype is planned.

The same independent review found filesystem-only timers in
`get_memory_md`, `save_memory_md`, `list_topics`, and `get_topic` falsely claimed
the SQLite `memory` database. The database labels were removed from those four
timers. `save_topic` and `delete_topic` retain the label because they also update
the SQLite mirror.

Logs reader cost test: `cargo test -p arcana --lib
measure_metrics_refresh_tail_and_full_export_on_reproducible_jsonl --locked --
--nocapture` passes on a 3,462,780-byte fixture. The 25,000-event initial
metrics refresh measured 404,895,300 ns; ingesting 1,000 appended records
measured 19,561,900 ns; reading the newest 2,000 lines measured 3,768,600 ns;
full-file read/export helper measured 1,632,600 ns. Counts and bytes are
asserted. This excludes UI render, IPC, and destination write, and is a single
Windows run, not representative Fleet latency. Repeat with captured Fleet data
once available.

Re-run closeout checks after these latest edits: `cargo fmt --all -- --check`,
`cargo check -p arcana --lib --locked`, `npm run typecheck`, and
`git diff --check`. No commits/pushes yet. The full core result of 809 passed /
2 failed was measured before adding the latest diagnostic test and remains the
known Clippy canonicalization failure pair, not a current all-green claim.

### Memory observability contract follow-up — 2026-10-02

The source/caller audit found an inaccurate `save_memory_md` comment claiming
that raw index writes synchronized SQLite topic timestamps. The Tauri command
only forwards to a filesystem write; actual topic save/delete own the SQLite
mirror and regenerate the index. The comment now documents that behavior.
MEMORY.md read/write timers record only byte counts on success and do not claim
SQLite activity. The structured OperationTimer test now verifies `result_bytes`.
Latest checks: `cargo check -p arcana-core --locked`, `npm test -- --run
src/tabs/Logs.test.tsx` (14/14), `npm run typecheck`, `cargo fmt --all --
--check`, and `git diff --check` pass. The core check reports existing unused
code/import warnings. The full core suite remains 809 passed / 2 failed at the
documented Windows AppContainer Clippy config canonicalization boundary; it has
not been rerun after this small observability correction. No provider calls,
Calendar access, commits, or pushes occurred.

### Independent P4-50 review follow-up — 2026-10-02

An independent GPT-6 Luna High review found two telemetry mismatches. The
procedural search path now puts the same actual result count in both its result
event and timed operation event; `cargo test -p arcana-core --lib
procedural_search_result_and_timing_events_agree_on_result_count --locked`
passes 1/1. Logs Metrics now counts the graph producer's `partial` outcome as a
degraded failure; `cargo test -p arcana --lib
metrics_count_partial_graph_results_as_degraded_failures --locked` passes 1/1.
The reviewer confirmed the corrected raw MEMORY.md write contract. These are
focused fixes only. Full P4-50 producer coverage/readback, a successful CLI Fleet
run, and the broader P4 gates remain open. Re-run formatting, diff check, and
core check after this edit; no provider or Calendar test was run.

Final checks after the review fixes: `cargo fmt --all -- --check` and
`git diff --check` exit 0; `cargo check -p arcana-core --locked` exits 0 with
existing workspace warnings. The Memory event-pair and partial-graph metrics
tests each pass 1/1. `npm run typecheck` and Logs UI tests pass from this same
continuation; no frontend files changed afterward. The full core suite was not
rerun and remains represented only by its earlier 809/2 known-failure result.

### Full core-suite retest — 2026-10-02

The current `cargo test -p arcana-core --lib --locked` run completed 811
passed / 2 failed / 0 ignored in 369.49 seconds. The only failures are the
same Windows AppContainer Clippy-config path canonicalization denials in the
focused Clippy test and real Rust corpus (the file bytes are readable, but
`canonicalize()` returns Access denied). The new procedural-search telemetry
test passes in the full suite. No additional failure appeared. This supersedes
the earlier checkpoint note that the suite had not been rerun. Do not widen
ACLs: the recorded Windows namespace limitation remains the failure boundary.

### Tauri library and cross-phase gate follow-up — 2026-10-02

The first full `cargo test -p arcana --lib --locked` run found 148 passes and
one failed TG-P1-06 source guard. Inspection confirmed both read and write
partitions call the shared `execute_handler`, which redacts returned results;
the guard failed only because `.await` was line-wrapped in one call site. The
guard now counts the call expression independently of formatting. Its focused
test passes 1/1, and the full Tauri suite passes 149/149 in 2.53 seconds. No
production redaction behavior changed. This is a cross-phase test correction;
P4 and the broader P1/P2/P3/P8 audit are still open.

### Memory observability follow-up — 2026-10-02

An independent Memory/SQLite source audit found three result-contract gaps: aggregate `get_stats()` could label component-counter fallbacks as success, prompt recall timing omitted the result count, and procedural topic listing silently omitted directory-entry or unreadable-file failures. `get_stats()` now marks incomplete reads as `partial`; prompt-recall result and timing events carry the same actual count; and topic listing propagates I/O failures so its timer records an error. Embedding inference no longer claims a SQLite database identity. The audit found every inspected production SQLite connection path installs the profile callback; callback events still cannot report statement success/failure, so operation-level result events remain necessary.

`cargo test -p arcana-core --lib memory:: --locked -- --nocapture --test-threads=1` passes 66/66, covering captured partial-stats and recall-count events, invalid topic listing and index-refresh failure, partial-index write prevention, and non-database event identity. `cargo fmt --all -- --check`, `cargo check -p arcana-core --locked`, and `git diff --check` pass (the diff check reports only existing LF-to-CRLF conversion notices). The last full core suite remains 811 passed / 2 failed before this patch, with only the known Windows AppContainer Clippy config canonicalization denial pair. `cargo test -p arcanactl --locked` passes 13/13. P4 remains open; no provider request, Calendar access, commit, or push occurred.

### P4-58/TG4-20 closeout continuation — 2026-10-02

Independent GPT-6 Luna High review found an unresolved positional
`execute_batch` builder bypass and missing plan/result proof for the worker-run
UPDATE trigger. The AST registry now rejects unresolved SQL builders; the only
exception is the exact owner/template pair for `backup_database_in`'s fixed
`VACUUM INTO '{escaped}'` backup command. New tests reject a runtime SQL
fragment, constrain that exception, and verify every exact fingerprint row has
source-linked proof or rationale. The worker-run populated regression explains
both insert/update guard predicates against 300 rows and proves both trigger
paths. The ACP replay regression now covers multiple messages under the static
per-row mark-read update.

Validation: query registry 8/8; worker-run insert/update trigger 1/1; ACP
two-message replay/ack 1/1; `cargo fmt --all -- --check`,
`cargo check -p arcana-core --locked`, and `git diff --check` pass. Independent
review found no remaining unpaired query variant. P4-58 and TG4-20 are closed.
At that checkpoint, the latest full core run was 811 passed / 2 failed on the known Windows
AppContainer Clippy config-canonicalization pair; the full suite was not rerun
for this scoped registry closeout. No provider call or Calendar workspace
access occurred. No commit or push has been made.

Next: continue with the remaining open P4 tasks and gates, prioritizing
P4-50/TG4-24 producer-to-Logs verification, P4-61/TG4-31 sandbox/runtime
coverage, P4-62 timeout/recovery and stage measurements, CLI Fleet success when
Gemini quota allows, and cross-phase audits. P4-49 remains owner-only final
desktop acceptance. Keep the working tree intact and stage only verified
logical units.

## 2026-10-02 continuation — committed-boundary DB result events

`OperationTimer` gained optional `run_id` correlation. Production task claim
and status methods plus worker run begin/phase/finish and heartbeat now emit
safe result events. Task status events are emitted at public transaction
boundaries, so success follows commit; stale/missing ownership failures retain
error events with the known worker identity. A Luna High review caught the
initial inner-helper placement before it was recorded as complete; the
instrumentation was corrected and regression cases added.

Focused captured-method tests pass for task claim success/empty, status success,
missing row, stale owner and stale route, heartbeat success/missing worker, and
worker tick begin/finish success/missing run. The P4-58 query registry passes
8/8, `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and
`git diff --check` pass. The owner chose native millisecond SQLite statement
samples and nanosecond logical operation spans. TG4-24 remains open for other
database families, persisted real Fleet-to-Logs readback, telemetry failure
isolation and realistic cost measurements. No Gemini request or Calendar
access occurred. No commit or push has been made.

## 2026-10-02 continuation — status telemetry run correlation

`ToolExecutor` now propagates the current worker `run_id` into `ToolContext` for
both parallel read and serial write tools. `UpdateTaskStatusTool` passes that
value into run-aware task transition methods; existing `TasksDb` APIs preserve
compatibility for callers without a worker run. Captured status-event testing
asserts task status telemetry contains the supplied run ID, and the P4-58 query
inventory now follows the run-aware SQL owners. Focused status tests pass;
query registry 8/8; core check, format, and diff checks pass. Owner selected
native millisecond statement samples plus nanosecond logical operation spans.
This is one P4-50 correlation fix, not gate closure. Broad DB/Memory coverage,
real producer-to-Logs readback, telemetry failure isolation, and cost evidence
remain open. No provider call or Calendar access occurred.

## 2026-10-02 continuation — latest regression results

The latest `cargo test -p arcana-core --lib --locked` run completed 823 passed,
2 failed, 0 ignored in 411.80 seconds. The same two Windows AppContainer
Clippy-config canonicalization failures recurred in the Rust production corpus
and dedicated Clippy probe: config bytes are readable, but Clippy's Windows
canonicalization returns access denied. No new failure class appeared. Both new
run-correlation tests passed, as did the 8/8 P4-58 query registry. `npm run
typecheck` passed and the Logs UI suite passed 14/14. These checks do not close
TG4-24; production Fleet Logs readback, complete operation coverage, failure
isolation, and representative costs remain open. No provider request, Calendar
access, commit, or push occurred.

## 2026-10-03 continuation — Plan Board/task lifecycle telemetry and trace readback

PlansDb CRUD plus task assignment, assigned claim, orphan release,
interrupted-run quarantine, and dependency reconciliation now emit
result-aware OperationTimer spans with fixed DB identity, elapsed nanoseconds,
and safe result counts. Task lifecycle events carry task/worker identifiers
where known and are emitted after transaction commit for assignment,
quarantine, and dependency reconciliation. The PlansDb production-method test
passes 1/1 and captures create/list/missing-read/foreign-key failure events
without plan text or session IDs; the full PlansDb module tests pass 4/4. The
task lifecycle capture test passes 1/1 and reads back committed claim and
quarantine state while proving refused claims are errors and empty dependency
reconciliation is a zero-result success.

The audit's SQLite correlation gap is closed for the task lifecycle paths
instrumented here: production JSON stores span fields flattened on span
objects, while both readers had expected a nested `fields` property. The
Tauri correlation filter and Logs trace tree now accept the emitted shape,
carry parent task/worker context onto the statement row, and the production
capture test proves a real task SQL statement groups under its task trace.
Logs panel tests pass 15/15 and the Tauri flattened-span filter test passes
1/1. Other database operations still need their own correlated parent spans.
Transient vector embedding unavailability can still appear as successful empty
recall. P4-50/TG4-24 remains open for that gap, remaining DB/Memory operations,
persisted Fleet-to-Logs readback, telemetry failure isolation, and
representative instrumentation cost evidence. No provider call or Calendar
access occurred; no commit or push was made.

Final checks for this continuation: `cargo check -p arcana-core --locked`,
`cargo fmt --all -- --check`, `git diff --check`, and `npm run typecheck` pass.
Focused results: PlansDb tests 4/4; task lifecycle telemetry capture 1/1;
existing task claim/status event test 1/1; query registry 8/8 from this
continuation's earlier run; Logs panel 15/15; and Tauri flattened-span
correlation filter 1/1. No full core-suite rerun was made after this slice;
the previously recorded 823-pass/2-known-Windows-failure run remains the latest
broad result.

## 2026-10-03 continuation — annotation database result telemetry

AnnotationsDb create/list/read/update/resolve/delete operations now emit result-aware spans with the fixed `annotations` identity and safe counts. Each operation opens a tracing span so the SQLite statement event carries the annotation ID and operation without file path, selected text, prompt, or response content. The focused production-method test passes 1/1: create/list/missing-read/triggered write failure are captured, a statement is verified under its parent span, and source path/prompt exclusion is asserted. The annotation module passes 2/2; `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass. This extends P4-50 coverage but does not close TG4-24. No provider/Calendar run, commit, or push occurred.

## 2026-10-03 continuation — Memory degraded-recall fix and SQLite resolution decision

When vector inference failed after semantic-backend selection, `VectorBackend::search` previously returned an empty success and reported no degradation. It now invokes the scoped lexical FTS fallback and marks the recall degraded, allowing the Memory consumer to render its keyword-only caveat and record a degraded operation outcome. The shared production fallback helper test proves the matching row is returned and a sibling scope is excluded; `cargo test -p arcana-core --lib memory::backend::tests --locked -- --nocapture --test-threads=1` passes 9/9. The enabled `embeddings` feature was not compiled because that build fetches the optional ONNX runtime; feature-gated compilation remains unverified.

The owner chose native millisecond SQLite callback samples with nanosecond logical operation spans. ADR-LOG-RESTRUCT-0186 records the choice and limits of the earlier synthetic timing fixture. Default-feature `cargo check -p arcana-core --locked`, formatting, and diff check pass. P4-50/TG4-24 remains open for broad database and Memory producer coverage, real CLI Fleet-to-Logs readback, failure isolation, representative cost evidence, and successful CLI Fleet completion. No provider request or Calendar access occurred; no commit or push occurred.

## 2026-10-03 continuation — analytics token-usage result telemetry

The provider token ledger's production writer and local-date reader now emit result-aware OperationTimer spans with fixed `analytics` identity, elapsed duration and safe row counts. The isolated production database capture verifies insert/read success, an injected trigger failure, and that session/provider/model markers are absent from JSON. `cargo test -p arcana-core --lib token_usage_writes_and_reads_emit_safe_result_spans --locked -- --nocapture --test-threads=1` passes 1/1; `cargo check -p arcana-core --locked`, `cargo fmt --all -- --check`, and `git diff --check` pass. This adds two DB paths and does not complete TG4-24. No provider call or Calendar access occurred; no commit or push occurred.

## 2026-10-03 continuation — worker registry result telemetry

The worker-roster query and status update now emit safe `OperationTimer` results with the fixed `workers` database identity and row counts. `worker_roster_and_status_operations_emit_safe_result_events` passes 1/1 against a seeded in-memory WorkersDb and checks that worker names are not included. P4-50/TG4-24 remains open; no provider or Calendar work occurred, and no commit or push occurred.

## 2026-10-03 continuation — Supervisor health reader spans

`RunsDb::latest_worker_tick`, `latest_worker_run_owner`, and `latest_worker_ticks` now emit result-aware spans with `database=runs`, duration/count, and worker correlation where available. The populated real-reader test passes 1/1 and confirms a private session marker is not emitted. The optional embeddings build was attempted with Cargo offline and cannot run because `aligned v0.4.3` is absent from cache; no network access was attempted. TG4-24 remains open; no provider or Calendar run, commit, or push occurred.

## 2026-10-03 continuation — isolated temporary test fixtures

The fixed-path `RunsDb` test helper was unsafe: it recursively removed a shared
tag-named temporary directory before creating its database. It now holds a
unique `tempfile::TempDir` for each test. A source sweep found the same
pre-delete pattern in task, scheduler, orphan, app-settings, budget, backup,
config, Bash timeout, file-tool, redaction-boundary, secret-boundary, and
concurrent-writer fixtures; these now allocate UUID-suffixed directories with
`create_dir` and never delete a pre-existing path. The test-only updater file
fixture also uses a unique filename. Cleanup remains scoped to paths each test
created. No test or command accessed `C:\Playground\arcana-calendar`.

The exact Supervisor health-reader test passes 1/1, and the complete core DB
module filter passes 162/162, including migration backup/refusal fixtures,
task claim/QC guards, and the amended RunsDb tests. Formatting and `git diff
--check` pass. An earlier run had already executed the old fixed-path RunsDb
test; that helper may have removed an existing directory with the same name.
Whether it contained user data is unknown, the directory is no longer present,
and no backup was identified. No recovery was attempted so as not to overwrite
or fabricate state. This remains a recorded possible data-loss incident.

## 2026-10-03 continuation — Windows log-rotation identity and failure metrics

The independent observability review identified duplicated provider-attempt
counts for retries, trace roots split from Fleet correlation, and omitted
`denied` outcomes in the failure headline. Metrics now count physical
`provider_attempt` events once and classify denied outcomes as failures. The
Logs trace tree roots Fleet-correlated activity together and preserves call
nodes. The focused aggregator regression passes; Logs panel tests pass 16/16.

The complete Tauri log-command test group initially exposed a Windows-specific
cache bug: replacing a JSONL file with equal length could retain the old
aggregate when creation timestamps had coarse resolution. Windows file identity
now comes from the open handle's volume serial and file index, and metrics read
from that same handle. This uses the already enabled `windows-sys` dependency
in `arcana-core`; no dependency was added. `cargo test -p arcana --lib
commands::log_commands::tests --locked -- --test-threads=1` passes 24/24,
including the replacement regression. `cargo check -p arcana --locked`,
`cargo fmt --all -- --check`, `git diff --check`, `npm run typecheck`, and
`npm test -- --run src/tabs/Logs.test.tsx` pass (16/16). The core library was
also compiled on Windows with the handle-identity helper. No provider request,
Calendar access, commit, or push occurred. P4-50/TG4-24 remains open for the
production Fleet reader-to-UI path, remaining database/Memory instrumentation,
failure isolation, and representative cost/reconciliation proof.

## 2026-10-03 continuation — explicit Windows Python fallback selection

Added a Settings file picker and persisted `python.exe` fallback path using the
existing config database. Selection is accepted only for a signed executable
whose canonical path is beneath the current user's
`%LOCALAPPDATA%\Programs\Python` directory. The preferred signed Python is
staged first; a selected fallback is consulted only when that stage fails.
Changing the selection invalidates the process-local staged-runtime cache so a
new fallback takes effect on the next project command. No worker/model argument
can choose this host path.

`cargo test -p arcana-core --lib platform::python_runtime::tests --locked --
--test-threads=1` passes 5/5, including path-boundary and cache invalidation
tests plus the production staged-runtime denial probe. `cargo check -p arcana
--locked`, `npm run typecheck`, `npm test -- --run src/tabs/Settings.test.tsx`
(2/2), format, and diff checks pass. The current user-owned Python installation
tree contains libraries but no `python.exe`, so this machine cannot exercise
the fallback execution branch; that remains open. A read-only review also
confirmed that P4-61/P4-62 plan/gate additions are still uncommitted and absent
from `HEAD`, and that successful fallback consumption and matched cold/warm
launcher measurements remain open. No Calendar access, provider request,
commit, or push occurred.

## 2026-10-03 continuation — P4-62 timeout recovery and matched launcher timing

Added an ignored, measurement-only Windows test for one cold and five warm
Python Build launches through the production `project_command` sandbox. All
samples use the same isolated Arcana profile, workspace, fixture, and resolved
command; the copied fixture's existing `tests/__pycache__` is removed before
the cold sample, and generated bytecode is removed between samples. A Luna
High review caught that the first run omitted this initial cleanup; its
measurements are superseded and retained here as a correction trail in the
plan/gate records. The corrected exact test passed:

`cargo test -p arcana-core --lib p4_62_measure_matching_python_build_cold_and_warm --locked -- --ignored --nocapture --test-threads=1`

Corrected cold runtime preparation/sandbox preparation/process execution/output
drain: 36,386ms / 1,030ms / 435ms / <1ms. Warm medians: 3,147ms / 13ms /
394ms / <1ms; warm total range: 3,552–3,603ms. The corrected run took 57.41
seconds total for six commands. This isolates signed Python materialization,
cache verification and Windows AppContainer setup; it is not representative
application throughput or a general performance target. The test emits
timings and has no performance threshold.

`production_timeout_guard_quarantines_claim_for_supervisor_reconciliation`
also passes. It uses the production worker timeout wrapper and `KickSlotGuard`
with a durable worker run and claimed task, then verifies the timed-out run is
persisted interrupted, the exact claim moves to `needs_info`, recovery work is
visible to Supervisor, and the interrupted worker cannot re-claim it.
P4-62/TG4-32 remains partial for matched evidence-persistence timing, a full
provider-backed CLI shutdown smoke, cross-platform timing, and cold-profile
variance.
User confirmed native SQLite millisecond callback timings with finer
nanosecond operation spans. No Calendar access, provider request, commit, or
push occurred.

The CLI Fleet path also had a production deadline bypass: `TaskStatus::Ready`
was handled before the timeout catch-all, while newly created epics begin
Ready. The timeout check now runs for every nonterminal state before the state
match, and polling sleeps are limited to the remaining deadline. The focused
test `cargo test -p arcanactl
ready_epic_expires_at_cli_deadline_without_losing_durable_state --locked --
-- --nocapture --test-threads=1` passes against an isolated real TasksDb and
proves a Ready epic returns a timeout error without losing its persisted row.
This does not exercise a provider-backed `fleet run` or its complete scheduler
shutdown; those remain open. `cargo fmt --all -- --check` and `git diff
--check` pass after the source and documentation changes.

## 2026-10-03 continuation — QC fingerprint hardening and independent finding

Implemented pre/post workspace fingerprint comparison for the full selected build batch. Any changed or unavailable fingerprint strips workspace provenance from all observed exits in that batch and produces a structured `workspace_changed_during_build` failure; real process exits remain intact. Fingerprinting also verifies that the pathname still identifies the opened file before and after hashing (Windows volume/file index, Unix device/inode). The changed/unavailable comparison test passes, and the existing production ProjectCommand/QC regression passes. `cargo fmt --all` ran before these focused tests.

The first attempted production build-script mutation fixture exited 101 under the launcher, so it did not prove a successful build racing a workspace write and is not counted as proof. A GPT-6 Luna High independent review also found a high-severity multi-project gap: a passing project can remain the latest persisted evidence after a sibling timeout/spawn failure, allowing QC if the workspace hash is unchanged. The safe fix requires persisting batch completeness separately from observed process exits. Owner direction is pending; P4-14/TG4-07 stay open. A transient mutation restored before post-hash and directory membership races remain residual limits of pre/post fingerprinting.

No provider request or Calendar access occurred. No commit or push occurred.

## 2026-10-03 resumed validation and source review

Read the expanded P4 goal at the actual attachment path under `C:\Users\razia\.codex\attachments\1a8b4c86-39b1-49c6-a229-64a4f905f7b4\pasted-text-1.txt`; the supplied path omitted the separator before `.codex`. `npm run typecheck` passes and `npm test -- --run` passes 49/49 across 12 files. P8 host-commit unit tests pass 11/11 and the real S8-06 smoke passes 1/1 after the test context included the required `run_id`. CLI suite remains 14/14 including Ready deadline coverage. P4-14 fingerprint comparison passes 1/1 and the production ProjectCommand/QC regression passes 1/1; core check, format check and diff check pass.

Independent GPT-6 Luna High review found the multi-project partial-batch QC bypass. Owner direction on additive batch-completeness persistence is pending. The production sandboxed in-build mutation fixture failed with Cargo exit 101, so it is not proof. P4 remains not ready for the Calendar run. Gemini free-tier quota is still a known CLI smoke blocker; no provider request or Calendar access was made in this continuation. No commit or push occurred.

P8 host commit-safety verification was rerun during this continuation: `cargo test -p arcana --lib agent::builtin_tools::git_commit::tests --locked -- --test-threads=1` passes 11/11; `cargo test -p arcana --test s8_git_safety --locked -- --nocapture` passes 1/1 after the fixture gained `run_id: None`; and `cargo test -p arcana --lib tg8_13_host_commit_safety_is_registered_and_used_by_workers --locked -- --nocapture --test-threads=1` passes 1/1. This verifies host behavior, real-tool smoke and Tauri worker reachability in the current tree, but is not a commit/push.

## 2026-10-03 independent recovery-path audit

A GPT-6 Luna High read-only source audit covered P4-36/42/52/53/54. P4-36 has safe timeout/panic quarantine and Supervisor reconciliation but no provider-backed automatic redispatch proof; retain P4-53's no-blind-replay semantics. P4-42 proves shipped QC recipe/inbox and stale-message paths at AgentEngine level, but not scheduler→RecipeRunner→persistent QC ack. P4-52 implements bounded four-attempt full-jitter preparation retries; no production runner test yet proves same durable run identity and exactly one engine execution. P4-53 lacks persisted completed operation outcomes/cursor, continuation instructions, changed-path records and repository snapshot (HEAD, branch/worktree and staged/unstaged/untracked state). P4-54 health output lacks active-operation elapsed time, retry state, model/token usage and parent/child relationships. Keep all related tasks open; the auditor recommended one focused production proof per seam and durable operation/run metadata before expanding the health reader. No tests or provider calls were performed by the auditor.

## 2026-10-03 P4-42 scheduler/runner continuation

A production-path regression now calls the real `RecipeScheduler::kick_worker`
for the persisted `qc-agent`/`qc-dispatcher` pair, executes the shipped recipe
through `RecipeRunner` and `AgentEngine`, reads and acknowledges a persisted
ACP inbox message using its stable ID, and proves a second empty-work kick
causes no provider request. The test-only scripted provider and egress grant
are isolated; no live key or network request is used. Exact command:

`cargo test -p arcana-core --lib scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges --locked -- --nocapture --test-threads=1`

Result: 1 passed. At this initial proof stage, `P4-42` and new `TG4-33` remained open because the Supervisor's
stale-message recovery nudge has not yet been consumed through RecipeRunner.
The multi-project QC batch-completeness owner decision is still pending. No
Calendar access, provider request, commit, or push occurred.

P4-52 follow-up: the P4-42 scheduler/runner test injects one transient preparation
failure before any provider or tool effect, then verifies that RecipeRunner
finishes the same single durable worker-run row and AgentEngine performs one
scripted workflow. This is not a claimed-task/budget continuity proof and does
not cover telemetry readback or a failure after a side effect. TG4-26 remains
open.

P4-42/TG4-33 closeout: `scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges` now covers the stale lifecycle: an aged row is returned by the production Supervisor stale-message tool, the production sender persists a QC nudge, the persisted QC worker is started via the production scheduler kick, and RecipeRunner/AgentEngine acknowledge both message IDs. The test checks durable acknowledgement, empty follow-up dispatch, and one-run/one-engine behavior after an injected transient setup failure. Result: 1/1 pass. TG4-33 and P4-42 are proven. This is isolated scripted-provider evidence, not live Gemini behavior; other P4 gates remain open.

## 2026-10-03 P4-36/S4-03 contract alignment

ADR-LOG-RESTRUCT-0190 reconciles the older automatic-redispatch wording with
the owner's P4-53 rule: uncertain side effects must be reconciled before the
same task is resumed. The plan now requires exact-owner quarantine, durable
interruption, Supervisor visibility without restarting Arcana, and replay only
after state proves it safe. Safe continuation and provider-backed recovery are
still open; no automatic redispatch is claimed.

## 2026-10-03 resumed observability follow-up

Owner confirmed the native SQLite profile callback's millisecond timing is
sufficient alongside nanosecond logical-operation spans (ADR-LOG-RESTRUCT-0186).
The optional `embeddings` feature now passes
`cargo check -p arcana-core --features embeddings --locked` after fetching the
locked dependencies; 30 compiler warnings remain. Logs now renders repeated
same-named spans as separate operation nodes when span IDs are absent, with a
regression proving their 1 ms and 2 ms timings remain distinct; the Logs suite
passes 17/17; the full frontend suite passes 50/50 and `npm run typecheck`
passes. Independent Luna High observability review keeps TG4-24 open for
real Fleet-to-JSONL-to-Tauri-to-UI readback, successful CLI Fleet completion,
external MCP call outcome instrumentation, degraded telemetry persistence
proof, and representative volume/cost measurements. The Memory suite passes
68/68, annotation DB telemetry passes 1/1, and formatting/diff checks pass. No Calendar access,
provider request, commit, or push occurred.

P4-52 retry-correlation follow-up: the retry helper now receives the run ID
allocated before startup preparation. Scheduled, recovered, and retry-phase
persistence events include that ID. The captured event regression passes 1/1
and verifies attempt 1/retry_scheduled plus attempt 2/recovered. No persistent
Logs reader readback is claimed; task budget continuity and post-side-effect
recovery remain open. All nine startup-retry tests pass, and
`cargo test -p arcanactl --locked` passes 14/14 after the shared runner change;
these tests do not contact a live provider or demonstrate Fleet completion.

## 2026-10-03 continuation — expanded cross-phase audit and Memory tool correction

The GPT-6 Luna High current-source audit for P1/P2/P3/P8 is recorded at
`_verify/2026-10-03-CROSS-PHASE-AUDIT-REFRESH.md`. It leaves P1 graph
registration/runtime provenance, P2 provider replay and remaining daily-cap
stream/restart/estimate/kill-switch evidence, P3 live consent IPC/revocation,
and P8 AgentEngine→scheduler commit proof open. The old P2 check-then-record
finding is superseded for the atomic reservation and result-aware writer paths.
Current P8 host behavior remains proven at its host-tool/real-tool/Tauri
registration boundary; the higher-level model-produced call remains unproven.

The audit's P3 `search_memory` gap is now fixed at the tool contract. A
backwards-compatible detailed recall API preserves the selected backend's
degraded flag/reason; the production tool includes the bounded, redacted state
for empty and nonempty results and records degraded operation timing as
`fallback`. `cargo test -p arcana-core --lib memory::recall:: --locked --
--nocapture --test-threads=1` passes 10/10 and `cargo test -p arcana-core
--lib search_memory --locked -- --nocapture --test-threads=1` passes 3/3. In
addition to the injected fallback cases, the tool test opens a migrated isolated
on-disk MemoryDb and verifies the real `SearchMemoryTool::execute()` returns a
persisted result with the active lexical-only degraded state. This
does not close TG3-17: active-backend absent/downloading behavior remains
unproven. Supporting audit and goal/readiness records are updated.

The owner reaffirmed SQLite callback millisecond samples plus nanosecond logical
operation spans; ADR-LOG-RESTRUCT-0186 records this. No Calendar workspace,
provider request, commit, or push was used. P4 remains active. The earlier
owner questions about multi-project build-batch completeness (P4-14) and the
P4-53 operation-journal storage boundary remain unresolved; dependent semantic
changes stay open pending their answers.

## 2026-10-03 resumed continuation — real Memory tool proof and MCP scope finding

Added `production_tool_reads_persisted_results_and_detailed_status` to the
`search_memory` unit suite. It creates an isolated Arcana home, opens the real
migrated MemoryDb, seeds a scoped memory row, invokes `SearchMemoryTool::execute`,
and verifies persisted content plus the lexical backend's degraded status.
`cargo test -p arcana-core --lib search_memory --locked -- --nocapture
--test-threads=1` passes 3/3. The earlier 10/10 recall tests remain the same;
TG3-17 still lacks production absent/downloading-state proof.

A read-only GPT-6 Luna High MCP audit confirms the repository has no production
external MCP tool-call client or call consumer. `src-tauri/src/mcp/health.rs`
records connector registration, HTTP health probes, and reconnect outcomes;
GitHub configuration commands only read/write settings. TG4-24 call outcomes
cannot be instrumented without introducing invocation capability. The owner
has been asked whether to add that capability in P4 or defer call telemetry to
its introducing phase; no implementation is made ahead of that direction.

The owner-confirmed SQLite precision remains native millisecond callback samples
plus nanosecond operation spans (ADR-LOG-RESTRUCT-0186). `cargo fmt --all --
--check` and `git diff --check` pass (Git reports existing LF→CRLF conversion
warnings). No provider request, Calendar access, commit, or push occurred.

P4-59/TG4-21 now has a second focused proof:
`cargo test -p arcana-core --lib
worker_stats_rollup_failure_is_atomic_and_preserves_cutoff_day --locked --
--nocapture --test-threads=1` passes 1/1. A trigger aborts the second monthly
bucket; the test verifies the first bucket and pruning roll back, removes the
trigger, retries, and verifies two committed month rows plus one daily row on
the exact cutoff. This does not prove deployed schema preservation, automatic
retention cadence, active-writer safety, export, or the other append-only
tables. P4-50 MCP tool-call scope decision remains pending.

A third P4-59/TG4-21 regression now exercises the production scheduled
consumer: `cargo test -p arcana-core --lib
analytics_manager_runs_worker_retention_on_startup --locked -- --nocapture
--test-threads=1` passes 1/1. The isolated `AnalyticsManager` starts its real
background job, derives `Local::now() - 90 days`, rolls the older point into a
monthly bucket, and retains the exact cutoff date as a daily row; the test
stops and joins the job. The test does not close deployed migration, concurrent
writer, full export, or other append-only table retention requirements.

## 2026-10-03 continuation

Owner selected native SQLite statement timings at millisecond resolution,
paired with nanosecond operation spans; see ADR-LOG-RESTRUCT-0186. The test
`production_operation_event_persists_and_reads_through_tauri_log_consumers`
passes 1/1: a real `OperationTimer` event is written to JSONL, then consumed by
the Tauri structured-event and incremental metrics readers with run/trace
correlation, duration, outcome and result count intact. This does not prove
live desktop IPC or Fleet-to-Logs UI readback. The reproducible cost fixture
was expanded to 7,492,780 bytes (25,000 rich DB events plus 1,000 appended)
to approximate the prior 6,977,969-byte Fleet log. Three warm runs measured
median initial metrics refresh 559.4 ms, incremental refresh 25.3 ms, tail-2,000
7.49 ms and full read/export 3.52 ms. These numbers exclude desktop IPC, React,
concurrent disk load and Fleet latency. TG4-24 remains open. The MCP invocation
scope question is also pending: source audit found only connector settings and
health/reconnect probes, with no production MCP call client.

The focused DB/Memory source audit identified direct PA `get_memory` and
`list_memories` reads without result spans, silent row-decoding drops in list
and procedural search, and a best-effort recall access-history write without
its own outcome. These are covered with correlated, content-free operation
events; not-found get records an observed zero; list truncation uses raw-row
lookahead and malformed counts are separate; procedural search distinguishes
malformed tag fields from skipped rows. A production detailed search returns
its recalled result despite an injected access-history failure, which receives
its own degraded event. Focused regressions now cover these cases. ADR-LOG-
RESTRUCT-0193 records the contract. This narrows but does not close TG4-24:
additional DB families and the real CLI Fleet-to-Logs run remain open.

Operation timing now distinguishes measured zero-result/zero-byte outcomes
from unset metrics with additive `result_count_observed` and
`result_bytes_observed` flags; `cargo test -p arcana-core --lib
observability::timing::tests --locked -- --nocapture --test-threads=1` passes
4/4. ADR-LOG-RESTRUCT-0194 records this event contract. It improves expanded
JSON interpretation but does not add missing result counts to uninstrumented
producers.

### 2026-10-03 Memory and config.db telemetry continuation

The Memory follow-up was tightened after independent review: `list_memories`
uses raw-row lookahead and names malformed rows explicitly; procedural search
distinguishes malformed tag sets from rows it actually dropped; `get_memory`
not-found records an observed zero. `search_all_detailed` now exercises the
public production DB-open/timing path with an injected `access_count` trigger
failure, returns its search result, and records both the secondary degraded
event and the expected lexical `fallback` outcome. `cargo test -p arcana-core
--lib memory:: --locked -- --nocapture --test-threads=1` passes 78/78; the
individual new regressions are included in that suite. `cargo fmt --all --
--check` and `git diff --check` pass.

The config.db producer audit added result-aware fixed-label spans for
app-settings, model configs, and Arcana instructions. Their isolated tests pass
6/6, 1/1, and 1/1; the settings group injects a SQL failure and verifies the
key/value and failure text do not appear in telemetry. The public detailed
Memory resilience regression passes 1/1. ADR-LOG-RESTRUCT-0195 records this
scope; these additions do not close remaining DB/Memory producer coverage.

The owner confirmed this continuation that native SQLite statement timing
stays at millisecond resolution, paired with nanosecond logical-operation
spans (ADR-LOG-RESTRUCT-0186). Three material P4 choices have been sent for
owner direction: task-scoped atomic batch-completion evidence for P4-14;
separate operation journal versus extending `worker_run_events` for P4-53;
and lifecycle-only MCP telemetry versus adding a new invocation capability in
P4. Dependent contract work remains open until answered. No Calendar access,
provider request, commit, or push occurred.

### Additional telemetry review fixes

An independent Luna High review found that the detailed Memory search event
dropped task/call/run/role context that its production tool consumer already
has, and that malformed stored timestamps could be copied into logs via the
parser error. The production search now carries this correlation; malformed
timestamps yield a fixed safe warning and mark the result degraded. The
production-path regressions are in `memory::recall::observability_tests`
(6/6).

The app-mediated egress-consent SQLite path was also missing operation spans.
Grant, revoke, read, and authorize/audit now emit fixed-label config-DB timings
with observed counts and allowed/denied outcomes, while excluding workspace
paths and provider destinations. Its production-method isolated test passes
1/1. These incremental P4-50 improvements do not close TG4-24; remaining
producer inventory, Fleet-to-Logs UI readback, CLI completion, telemetry
failure isolation, and representative costs stay open. Focused consent and
Memory tests pass. No Calendar access, provider request, commit, or push
occurred.

Daily-cost analytics insert, point-read and range-read now emit fixed-label
`analytics` operation timings with observed counts. An isolated production DB
test covers absent/present reads, two-row range, successful writes and a
trigger-injected write failure while confirming breakdown values and SQL error
text stay out of telemetry (1/1). ADR-LOG-RESTRUCT-0197 records the contract.
This still leaves other analytics, task DB and graph operations without
logical result spans; TG4-24 remains open.

High-fan-out GraphStore readers now emit result-aware timings for file-node
listing, paged node and edge reads, search, and aggregate stats. The isolated
production reader test passes 1/1 and excludes node names, paths, search input,
and DB path. Daily-cost analytics insert/read/range spans also pass their 1/1
production-method test, including an injected write failure. ADR-LOG-RESTRUCT-
0197 and 0198 record these additions. Logical mutation spans and remaining
TasksDb/control-plane operations are still open, as are TG4-24's cross-layer
proof requirements.

The primary TasksDb Plan Board/dispatch selectors now have fixed-label timing
spans for create, ID lookup, worker-claim lookup, board list, and
worker/status list. A production migrated-DB test covers missing and populated
reads, empty and successful claim lookup, and title privacy (1/1). Combined
with the GraphStore reader test (1/1) and daily-cost analytics test (1/1),
these are focused producer proofs only; TasksDb message/build/budget/epic
families, remaining analytics operations, write spans, full Fleet-to-Logs
readback, failure isolation and representative desktop costs remain open.
ADR-LOG-RESTRUCT-0199 records the task-reader event contract.

The ACP task database now also times message insert/read, recipient inbox,
stale/task/recent listing, unread and unacknowledged counts, and ack. A
production test proves delivery/read-but-unacknowledged/re-ack state, duplicate
ack classification, result counts, and that message bodies are not logged
(1/1). ADR-LOG-RESTRUCT-0200 records this event contract; end-to-end scheduler
replay proof and remaining DB telemetry work stay open.

Supervisor worker-request create/read/list/update also now report fixed-label
task DB outcomes/counts with available typed task/worker correlation, omitting
request payloads and resolution text. The isolated lifecycle/readback/privacy
test passes 1/1; ADR-LOG-RESTRUCT-0201 records it. These are still focused DB
producers, not proof that all Supervisor behavior reaches the Logs UI.

AnalyticsDb's API-failure insert/recent-read and system-event insert/recent-
read paths now emit fixed-label result spans. The isolated production test
checks filtered readback, counts/outcomes, an injected SQL failure, and
exclusion of provider/error/request fields plus event message/metadata (1/1).
ADR-LOG-RESTRUCT-0202 records this boundary. The broader analytics inventory
and TG4-24 end-to-end requirements remain open.

### 2026-10-03 resumed checkpoint — DB producer coverage

The owner reaffirmed native SQLite statement sampling at millisecond
resolution; nanosecond logical-operation spans provide finer-grained timing
(ADR-LOG-RESTRUCT-0186). The core check and workspace format check pass on the
current tree. Build-evidence batch writes/readback, routing-history operations,
and HooksDb configuration/execution-log paths now emit fixed-label result
spans. Their isolated production-method tests pass 1/1 each, including injected
database failures, not-found counts, and privacy assertions. Test fixture
correction: routing-history readers intentionally exclude legacy assignment
rows, so the populated automatic-routing fixture now uses the production
assignment API. These changes do not close TG4-24. Remaining P4-50 gaps include
other database producers, telemetry persistence/export failure isolation,
successful CLI Fleet completion, live desktop/Fleet-to-Logs readback, MCP
invocation scope, and representative end-to-end cost evidence. The owner
decisions on P4-14 atomic batch evidence, P4-53 journal storage, and MCP
invocation capability remain unresolved. No Calendar access or provider call
occurred; no commit or push occurred.

2026-10-03 follow-up: WorkersDb registry operations (create, ID/slug reads,
model/repository/recipe updates, slug existence, deactivate and delete) now
emit safe result-aware spans. RunsDb generic lifecycle and recovery methods
(begin, finish, get, recent/open readers, worker-tick interruption, and dead
run reaping) now emit spans; missing `get` calls do not echo arbitrary input as
a run-correlation ID, while successful lookups retain the stored run ID. The
isolated production-method regressions pass 1/1 each. A test first flagged that
session IDs are intentional correlation fields in worker-tick start events;
the test now checks expected correlation instead of treating that field as
private content. These additions still do not close TG4-24. No provider or
Calendar call occurred, and no commit or push occurred.

2026-10-03 follow-up: AnalyticsDb worker-stat insert/read and live activity, token-range, and token-total methods now emit result-aware fixed-label spans. The focused production-method test passes 1/1, including missing rows, injected storage failure, and privacy assertions; core check, workspace format, and diff checks pass. TG4-24 remains open for complete producer inventory, telemetry failure isolation, successful CLI Fleet completion, desktop/Fleet to Logs readback, and representative end-to-end costs. The goal attachment is C:\Users\razia\.codex\attachments\1a8b4c86-39b1-49c6-a229-64a4f905f7b4\pasted-text-1.txt. No Calendar access or provider call occurred; no commit or push occurred.

2026-10-03 follow-up: SessionStorage now instruments store open, migration, schema version, and session/message CRUD and reads using fixed operation/database labels and safe session correlation. session::storage::tests passes 3/3, including production-method capture, not-found, readback, and injected SQL failure; titles, workspace paths, provider/model IDs, message/tool content and SQL error text remain absent. TG4-24 remains open for remaining producer inventory, persistence failure isolation, CLI Fleet completion, live Fleet-to-Logs readback and representative end-to-end costs. No Calendar access or provider call occurred; no commit or push occurred.
2026-10-03 follow-up: Full GraphStore logical DB coverage now includes startup,
schema version, writes, point/page reads, deletes, community assignment,
snapshots, FTS rebuild, and WAL flush. `graph::store::timestamp_tests` passes
3/3, including the reader suite and a production-method test with an injected
batch failure; graph content, user paths, queries, snapshot JSON, and database
path are excluded from telemetry. GraphStore no longer writes its DB path into
ordinary open logs. This is producer coverage only; TG4-24 remains open for
other DB producers, CLI Fleet completion, persisted Fleet-to-Logs readback,
telemetry failure isolation, and end-to-end cost evidence.
2026-10-03 follow-up: TasksDb QC reviewer claim and cost persistence now emit
fixed-label, task-correlated spans with safe counts and conflict/missing/skipped
outcomes. `qc_claim_and_cost_operations_emit_private_result_spans` passes 1/1
against the production DB methods, including duplicate claim, row readback,
missing task, no-op cost, injected SQL failure, and privacy checks for reviewer
IDs, task title, cost values, DB path, and SQL detail. Remaining task methods
and TG4-24 end-to-end requirements remain open.
2026-10-03 follow-up: TasksDb QC reviewer claim and cost persistence now emit
fixed-label, task-correlated spans with safe counts and conflict/missing/skipped
outcomes. `qc_claim_and_cost_operations_emit_private_result_spans` passes 1/1
against the production DB methods, including duplicate claim, row readback,
missing task, no-op cost, injected SQL failure, and privacy checks for reviewer
IDs, task title, cost values, DB path, and SQL detail. Remaining task methods
and TG4-24 end-to-end requirements remain open.
2026-10-03 follow-up: Epic/phased-task creation, epic task listing, phase
summaries, and ready-phase selectors now emit fixed-label `tasks` spans with
safe counts and integer task correlation only after successful insert readback.
`epic_phase_operations_emit_private_result_spans` passes 1/1, including empty
results and injected insert failure; planning titles/descriptions/criteria,
phase/epic IDs, milestones, creator fields, DB path, and SQL detail remain out
of telemetry. Broader task DB producer inventory and TG4-24 end-to-end evidence
remain open.

2026-10-03 follow-up: TasksDb retry increment/count read, diff-analysis write, task delete, and board clear now emit result-aware spans. `task_retry_diff_and_delete_operations_emit_private_result_spans` and `clearing_the_board_backs_up_first_and_the_backup_holds_the_tasks` pass 1/1 each against isolated databases, including injected write failures and backup-first behavior. The previous clear-board backup log exposed the filesystem location; it now emits a structured event with only fixed operation/reason and byte count, verified against path leakage. TG4-24 and P4 remain open; no Calendar workspace or provider request was touched, and no commit/push occurred.

2026-10-03 follow-up: Token-usage model/provider/agent breakdowns and top sessions now emit distinct fixed-label child spans with measured result counts beneath the range query. The production analytics capture test passes 1/1 and confirms no session, model, or provider values leak. This is query-level attribution, not full TG4-24 proof.

2026-10-03 follow-up: The shared SQLite `run_ladder` path now records total migration duration, allowlisted logical database identity, applied-rung count, and success/error. The isolated capture test passes 1/1 across applied, already-current, empty, and invalid-SQL cases; the operation event excludes database paths, arbitrary DB labels, migration names, and SQL text. Run full `db::migrate::tests` before considering this bounded slice complete. TG4-24 remains open.

Validation update: `cargo test -p arcana-core --lib db::migrate::tests --locked -- --nocapture --test-threads=1` passes 15/15 after the result-count change, including backup-first/refusal and rollback tests on isolated temp databases. `cargo fmt --all -- --check` passes.

2026-10-03 follow-up: Provider spend reservation/settlement now emits decision-level config events (`granted`, `denied`, `disabled`, `settled`, `not_found`, `error`), without IDs or spend amounts; its isolated capture test passes 1/1 with injected insert failure. `TasksDb::get_orphan_candidates` now reports empty/success/error and observed row count, fixing ambiguity when the scheduler's caller suppresses the read error; its isolated test passes 1/1 for empty, assigned/in-progress, and invalid-schema cases. TG4-24 remains open.

2026-10-03 follow-up: Checkpoint creation/automatic pre-write, restore, list, compare and delete now emit operation timings with safe task/session correlation and measured result counts/bytes. `checkpoint_lifecycle_operations_emit_private_result_spans` passes 1/1 through the production API and verifies restored content, empty/not-found semantics, and privacy for filesystem paths, source text, reasons, and checkpoint IDs. TG4-24 remains open.

2026-10-03 follow-up: `arcanactl analytics` cost/token/worker reads and export now emit fixed-label operation events at the CLI boundary, keeping `arcana-shared` decoupled from core. The CLI capture test passes 1/1 for populated, empty and error states and verifies private row/error values are absent. TG4-24 remains open.

2026-10-03 follow-up: IDE workspace-state and GitHub MCP configuration operations now emit safe timed results. `workspace_state_operations_emit_safe_result_spans` and `github_mcp_configuration_operations_emit_private_result_spans` pass 1/1 each, including malformed/empty/error cases and privacy assertions. This does not close TG4-24. No provider request or Calendar workspace access occurred; no commit or push occurred.
