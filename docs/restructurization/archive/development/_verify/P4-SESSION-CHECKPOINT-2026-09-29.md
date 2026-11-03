# P4 Session Checkpoint — 2026-09-29

The owner paused the P4 goal at 2026-09-29 16:37 +07:00. Resume from this
checkpoint; do not start the owner-only Calendar acceptance until all engineering
work and gates are ready. The active goal record is already `paused`.

## Current repository state

- Checkout: `C:\arcana`, branch `main`, HEAD `fff31ec`, tracking `origin/main`.
- The P4/P8 implementation remains dirty and uncommitted. No commit or push was
  made in this continuation.
- Do not run or write anything in `C:\Playground\arcana-calendar`; P4-49 is
  reserved for the owner after the engineering gates close.
- The live Gemini key was read from repository `.env` using `GOOGLE_API_KEY`,
  written to the Google provider vault slot, read back and compared in memory;
  neither value was printed, and both buffers were zeroized. The temporary
  helper was removed. The live test source reads `.env` directly and has no
  ambient shell-environment fallback.

## Work completed in this continuation

- Fixed the plans timestamp migration test to checkpoint and copy its legacy
  fixture into a separate upgrade directory before replaying the destructive
  migration. This prevents test backup collisions without changing production
  backup behavior.
- Fixed the corresponding worker timestamp migration test the same way.
- `cargo test -p arcana-core --lib timestamp --locked -- --test-threads=1` —
  **25 passed**.
- Focused plans, workers, tasks, and app-settings test modules were run serially;
  the command sequence exited successfully.
- `cargo test -p arcana-core --features live-smoke --test live_provider_smoke p4_google_live_key_from_dotenv_is_accepted_by_models_endpoint --locked -- --exact --nocapture`
  — **1 passed** using the key read directly from `.env`.
- An isolated production `project_command` test was interrupted by the owner's
  pause request after running for over 60 seconds. It is **incomplete**, not a
  pass or failure. Its test-created fixture directory remains at
  `C:\Users\razia\AppData\Local\Temp\arcana-project-command-build-5a76b6ff-ed89-4834-ae61-9e3f3e574efd`;
  preserve it unless the owner authorizes cleanup.
- After interruption, no matching Cargo, Rust compiler, or fixture executable
  remained. No listening port belonged to that test. The test did not start a
  server.

## Known test and data state

- The earlier full `arcana-core --lib` run was interrupted after a long,
  contention-heavy run and is not a green gate. Do not cite it as passing.
- The timestamp filter first exposed a worker fixture backup collision. Both
  plans and worker tests are now isolated and the complete timestamp filter
  passes.
- Prior parallel test runs left migration snapshots under
  `C:\Users\razia\.arcana\backups` (10 task-v9 snapshots were previously
  observed). Preserve them; do not open, delete, or overwrite them as routine
  cleanup.
- The streamed Gemini tool-signature replay has previously received bounded
  Google `503 Overloaded` responses. Authentication is verified by the
  models-endpoint smoke; streaming remains provider-availability-blocked and
  must not be described as key failure.

## Remaining work before owner Calendar acceptance

1. Re-run the canceled production `project_command` QC-evidence test in
   isolation. Then run the checked-in Rust/Node/Python ecosystem corpus test
   separately and investigate any sandbox failures. Avoid the parallel full
   suite until its shared-resource tests are isolated.
2. Continue TG4-19/P4-57 write/read inventory. The focused timestamp tests pass,
   but complete production-pair coverage, documentation evidence, and the gate
   remain open.
3. Continue P4-50 observability proof: production producers, persisted readers,
   Logs UI readback, PA/fleet correlation, token/result reconciliation, full
   MCP and memory/SQLite operation coverage, failure isolation, privacy, volume,
   and measured cost.
4. Complete remaining P4 tasks/gates, including P4-51 through P4-60 and TG4-16,
   TG4-19 through TG4-21, and TG4-24 through TG4-30. Read the current MDP and
   readiness matrix for exact statuses; P4 is not complete.
5. Re-run the real CLI Fleet scenario with Gemini 3.8 Flash and the synchronized
   vault credential. A previous attempt stopped on provider overload before
   successful decomposition; later run/readback proof remains required.
6. Close all engineering gates, then leave P4-49 as the sole owner action. The
   owner will run the desktop PA calendar scenario and return logs for analysis.

## Safe resume order

Read `GOAL.md`, the P4 sections of `MASTER_DEVELOPMENT_PLAN.md` and
`TESTING_GATES.md`, `READINESS_MATRIX.md`, `PROJECT_MEMORY.md`, and this
checkpoint. Confirm Git status before editing. Continue with isolated launcher
tests; do not repeat the live Gemini models-endpoint test unless the `.env` key
changes.

## 2026-10-02 continuation

- Confirmed host Python is `3.14.7` at `C:\Python314\python.exe`; the
  standard-library `compileall` command passed against the checked-in Python
  fixture with cache redirected to a temporary directory. Its configured
  pytest intent did not run because this installation has no `pytest` module.
  No package was installed and the Python runtime itself starts correctly.
- `python_build_is_standard_library_compilation_and_keeps_tests_separate`
  passes at resolver level. Production sandbox execution of Python remains
  unproven; Windows runtime allowlisting needs to include only the selected
  interpreter and its standard library, without silently enabling shared
  dependency installation.
- `failed_compiler_output_becomes_source_free_structured_diagnostics`,
  `diagnostic_paths_outside_the_project_are_omitted`, and
  `executor_context_preserves_only_bounded_source_free_diagnostic_fields`
  passed in a focused filter. The third-failure production test initially
  exposed stale wording after raw compiler output was removed; the next step
  now explicitly identifies three observed nonzero build exits and directs
  diagnosis to source-free codes/relative locations. The isolated rerun passed
  in 45.26 seconds.
- The Windows `selected_path_ancestors_receive_only_non_inheriting_traversal_grants`
  test and `selected_config_ancestor_grants_do_not_allow_sibling_reads_or_loopback_network`
  passed; the latter took 22.17 seconds. The real production Clippy test still
  fails after the config bytes are readable because canonicalizing the config
  path returns `Access is denied`. Keep TG4-18/TG4-31 open; do not weaken the
  external-read boundary to force Clippy through.
- An experiment granting traversal rights above the selected workspace was
  interrupted because the broader ACL operation appeared high-cost. It was
  reverted; no broad ancestor grants should be retained. The exact non-inheriting
  selected-path grant remains. The root ACL was inspected after interruption;
  no newly-added AppContainer SID from that attempt appeared there. Do not alter
  unrelated pre-existing SIDs or ACLs.
- `cargo check -p arcana-core --tests --locked` passed before the latest small
  guidance/test assertion edits. Rerun formatter, diff checks, and focused
  diagnostics before any commit. No commit or push has been made. No command
  read, wrote, or ran anything in `C:\Playground` or the owner calendar path.

### 2026-10-02 continuation — Gemini replay regression and CLI overload

- Gemini thought-signature handling now retains an opaque token already seen
  for a streamed function-call part when a later update for that same part
  omits it. ADR-LOG-RESTRUCT-0140 records the implementation rule.
- `cargo test -p arcana-core --lib
  repeated_gemini_stream_part_does_not_erase_its_thought_signature --locked`
  passed. The regression parses two Gemini SSE updates, runs them through the
  production `AgentEngine` assembler, and asserts the Google request builder
  puts the preserved token on the replayed function-call part.
- Rebuilt `arcanactl` and retried one isolated production Fleet run with
  Gemini 3.8 Flash, 100,000 Supervisor tokens, $2, and a 10-turn cap. Google
  returned 503 `UNAVAILABLE` before any tool or child task ran. The run exited
  as `provider_overloaded`; read-only SQLite inspection confirms the epic is
  still `ready`, no child tasks exist, and the Fleet tick is persisted as
  failed. This proves the CLI overload failure and readback path, not a
  successful Fleet run or live confirmation of signature replay.
- `--verbose` emitted a 3.4 MB log in this short failed run, dominated by
  per-statement SQLite debug records. Use normal verbosity for later live
  Fleet trials and retain the measured debug volume as a P4-50 filtering and
  overhead follow-up.
- The host Python executable remains healthy; the configured pytest module is
  absent, and the selected production sandbox path has not reached Python
  because Clippy canonicalization still fails. No package was installed.
- Reran `cargo fmt --all`; the focused Gemini regression passed and the CLI
  binary built successfully. Successful CLI task completion, production
  Windows Clippy/Node/Python corpus, Linux/macOS sandbox coverage, remaining
  P4 gates, and owner-only desktop acceptance are outstanding. No commit or
  push was made. No path under `C:\Playground` was accessed.
- A separate exact-ancestor ACL experiment was stopped after its Windows test
  ran for over 60 seconds at high CPU while adding traversal ACEs above the
  workspace. Because interrupting the test bypassed RAII cleanup, I removed
  only its unique AppContainer SID from the exact workspace/Temp/Local/AppData/
  profile directories and deleted the unique temporary AppContainer profile;
  the mapping and ACEs were verified absent. Source was reverted to the
  workspace-only traversal behavior. ADR-LOG-RESTRUCT-0142/0143 records the
  retraction. Do not rerun the outer-ancestor ACL experiment; leave Clippy's
  canonicalization gate open.

### 2026-10-02 continuation — Current-tree acceptance reruns

- Owner clarified that the Python confidence path includes both the
  standard-library `compileall` build intent and the separate Python test
  intent. The resolver test
  `python_build_is_standard_library_compilation_and_keeps_tests_separate`
  passes. The local Python runtime is available; `pytest` is absent, and no
  dependency was installed because installation remains explicit intent.
- Re-ran current-tree P8 host commit-safety checks: TG8-13 registration gate,
  all ten `git_commit` unit cases, the Bash mutation guard, and the isolated
  real-Git S8-06 smoke pass. Frontend typecheck completes without diagnostics,
  Vitest passes 38 tests across 11 files, ESLint exits successfully with 43
  pre-existing warnings and zero errors, and the three log-retention unit
  cases pass. `cargo fmt --all -- --check` and `git diff --check` also pass;
  Git reported only existing LF-to-CRLF warnings. No application server or
  listener was started.
- A fresh independent Luna High P8 review confirmed the specified branch,
  explicit-path, staging, staged-blob scan, shared-index serialization,
  commit-message-shape, Bash guard, and production worker wiring controls.
  It identified a possible additional credential-in-commit-message gap. The
  current P8 contract names sensitive diffs; owner direction is pending before
  changing that shipped contract. The reviewer did not run commands or access
  credentials.
- These reruns do not close the open P4 task/gate set, the Windows Clippy
  canonicalization failure, a successful live CLI Fleet run, or owner-only
  P4-49 acceptance. The checkout remains dirty and uncommitted; no push was
  made.

### 2026-10-02 continuation — focused observability defect fixes

- Shared task-correlation serialization now maps absent task IDs to an empty
  value rather than synthetic `0`; the Logs correlation matcher regression
  proves such events match neither task `0` nor a real task ID.
- Metrics aggregation now classifies `outcome="failure"` as a failure. CLI
  log discovery includes both readable `.log` and structured `.jsonl` streams,
  and `--quiet` suppresses telemetry on console/readable output while retaining
  the structured DEBUG capture path.
- Focused test cases pass for all four corrections, including the taskless
  timer producer, failure metrics, CLI stream list, and quiet filter. P4-50
  remains open; no real producer-to-UI workflow or successful CLI Fleet run is
  proven by these unit tests.
- Owner decision: Python `build` uses `python -m compileall -q` for the selected
  project, and Python tests remain a separate confidence signal. The resolver
  test passes; local `pytest` is absent, and no package was installed.
- No application listener was started, no Playground path was accessed, and
  no commit or push was made.

- The real CLI smoke `target/debug/arcanactl.exe logs list` exited 0 and listed
  both readable and JSONL files from `%USERPROFILE%\.arcana\logs`. A later
  fresh `cargo build -p arcanactl --locked` followed by the same smoke showed
  a 5.0 KB JSONL file and a zero-byte readable log. I did not inspect or
  attribute the JSONL contents, so this verifies discovery only; it does not
  prove a current production Fleet event reached the Logs UI. TG4-24 records
  that limit.

- P4-62 now has deterministic duration evidence: `production_worker_deadline_allows_work_past_ten_minutes` passed under paused Tokio time through the production worker timeout wrapper. It simulated 10m+1s and completed within the two-hour cap. The per-call process-tree cancellation/reconciliation and matched stage-timing measurement clauses remain open.

- A fresh Luna High read-only P4-50 review found mismatched CLI/Fleet grouping,
  JSONL-only date discovery, and a failure-outcome headline omission. All three
  consumer defects are fixed with regression coverage: Logs tests pass 9/9,
  `npm run typecheck` passes, and the Tauri JSONL-only date helper test passes.
  The reviewer also confirmed the SQLite profile callback cannot know statement
  success/failure; keep its `duration_observed` status honest. A successful real
  producer-to-UI proof is still missing.

- After enabling Tokio test clock support for arcana-core tests, reran the
  worker deadline simulation plus isolated timeout claim recovery and
  project_command default/clamp tests; all passed. `cargo fmt --all --
  --check`, `git diff --check`, and `cargo check --workspace --locked` pass on
  the current tree. The workspace check reports existing unused-import and
  unused-code warnings. These results do not prove process-tree cancellation
  or stage timing.

- Pending owner direction for P4-50: rusqlite's profile callback cannot report
  SQLite statement success/failure. Options are a result-aware layer across
  existing DB/Memory/graph statement calls or retaining per-statement duration
  only and reporting failure at the containing operation. Do not change this
  boundary until the owner chooses.

### 2026-10-02 continuation — Python confidence and process-tree cancellation

- Owner confirmed Python verification includes both the standard-library
  `compileall` build intent and a separate Python test intent for added
  confidence. The exact command resolver regression passes. A fresh host check
  found Python 3.14.7 runs `compileall` successfully on `fx-repo-python` with
  bytecode redirected under ignored `target/`; `python -m pytest --version`
  reports `No module named pytest`. No package was installed. The Python test
  corpus therefore remains unrun on this host; the separate intent is correctly
  represented by `python -m pytest` and dependency setup remains explicit.
- `dropping_a_timed_out_appcontainer_child_terminates_it` passes on Windows for
  the direct child. Added `dropping_an_appcontainer_command_terminates_its_descendant_processes`,
  which runs a nested `cmd` script inside the production AppContainer job,
  observes its workspace heartbeat, drops the parent job, then proves the
  descendant stops writing and never reaches its later marker. The regression
  passes. An initial test-script form using detached `start` was denied by the
  AppContainer; the verified synchronous nested-command form matches the
  existing nested-process sandbox contract. This proves the job cleanup
  primitive, not yet the complete `project_command` timeout → claim recovery
  integration required to close TG4-32.
- Added `production_command_timeout_stops_nested_process_writes`, which wraps
  `run_resolved_command` in Tokio's configured timeout shape and confirms the
  nested descendant's heartbeat stops after timeout. This verifies cancellation
  through the actual project-command process launcher; task reconciliation
  caused by that timeout remains unproven.
- TG4-31 and TG4-32 now record this partial descendant evidence. P4 remains open
  for the Windows Clippy/runtime corpus issue, full outside-user-file and
  network denial, dependency staging, cross-platform proofs, and P4-62's
  production timeout/reconciliation and comparable stage measurements. No
  application server was started, no `C:\Playground` path was accessed, and no
  commit or push was made.

### 2026-10-02 continuation — P4-51 suggested-action guarantee

The shared executor enrichment now guarantees the P4-51 `suggested_action`
field on tool failures. It preserves a handler-provided suggestion; otherwise
it copies the already-redacted and bounded `next_step`, falling back to the
existing conservative guidance. It does not change `retry_safe`/`retryable`.
`cargo fmt --all -- --check` passes. `cargo test -p arcana-core --lib
executor_context_ --locked` passes 5 focused tests, including empty-action
fallback, generic next-step promotion, preservation of a sandbox suggestion,
legacy default guidance, and bounded source-free diagnostic fields.
ADR-LOG-RESTRUCT-0147, P4-51, and TG4-25 now
record this partial proof. P4-51 remains open for full builtin/error-class
coverage, provider timeout/cancellation classes, and planted-input privacy
coverage. No commit or push was made.

### 2026-10-02 correction — Python test runner repaired

After the owner confirmed separate Python test confidence was desired, installed
pytest 9.1.1 into the user Python 3.14 environment. Both the fixture's exact
compileall intent (workspace-local bytecode cache) and `python -m pytest` now
pass; the test suite reports 1 passed. The earlier note that pytest was missing
records the state before this repair. This is local fixture execution only;
production sandbox/worker execution remains open under TG4-18.

### 2026-10-02 continuation — retention to Logs reader proof

Added `retention_keeps_the_active_date_discoverable_and_fully_exportable` in
`src-tauri/src/commands/log_commands.rs`. It runs the production core log
pruner against a temporary directory with eight dated readable files, then
uses the real Logs date-discovery and full-file reader helpers. The active date
stays first/discoverable, the retained date set respects the five-file cap,
and all 3,005 lines remain readable. `cargo test -p arcana --lib
retention_keeps_the_active_date_discoverable_and_fully_exportable --locked`
passes. The core retention suite passes 3/3. P4-55 remains open for concurrent
active-writer and startup cleanup-failure proof under TG4-29; Unix symlink
coverage cannot run on this Windows host. No real user log directory was
changed.

### 2026-10-02 continuation — active log writer during pruning

Added `active_writer_remains_writable_while_retention_prunes_old_files` to
`arcana-core/src/observability/log_retention.rs`. It holds an append handle open
to the active daily log while the production pruning function removes an older
recognized file, then writes and reads a post-prune marker. The focused
`cargo test -p arcana-core --lib observability::log_retention::tests --locked`
passes 5/5 on Windows. A captured tracing-subscriber regression also proves the
public pruner logs cleanup-failure detail and its summary, then returns
normally. Startup integration and cross-platform symlink execution remain
open under P4-55/TG4-29. No real log directory was touched.

### 2026-10-02 continuation — persisted skill portability

A P4-16/TG4-17 audit found saved-skill loading used the weaker executable
example validator and accepted `/tmp/` in persisted prose. The loader now
applies `validate_persisted_agent_guidance` to both description and template;
the negative regression passes. `cargo test -p arcana --lib portability
--locked` passes 3/3, `cargo test -p arcana-core --lib portability --locked`
passes 8/8, and the shipped recipe and skill corpus tests each pass. Full
saved-artifact corpus and Linux/macOS evidence remain open. No existing skill
or user profile was modified.

### 2026-10-02 continuation — PR-diff consent and test isolation correction

The isolated Tauri PR-diff helper tests now cover denied fallback without
analyzer invocation and matching consent reaching a local analyzer callback;
the callback avoids reading a vault credential or creating a live provider.
`cargo test -p arcana --lib commands::diff_commands::tests --locked` passes
3/3. The first matching-consent test draft incorrectly assumed redirecting
`HOME` isolated the Windows OS keychain. It failed its missing-key assertion
after a matching grant, so a provider request may have been attempted with the
synthetic diff. Whether it reached Google or was billed is unknown; no second
attempt was made. ADR-LOG-RESTRUCT-0148 records the retraction and fix. Keep
P4-60/TG4-16 open for the actual Tauri IPC/UI and provider-backed allow/revoke
workflow.

Correction: an earlier checkpoint statement that desktop log-retention startup
integration was missing was stale. `src-tauri/src/main.rs` initializes logging
before pruning `.arcana/logs`; ADR-LOG-RESTRUCT-0149 records the correction.
P4-55 remains open for isolated startup-order execution and Linux/macOS
symlink-refusal proof.

### 2026-10-02 continuation — SQLite event capture and Python fixture

`cargo test -p arcana-core --lib observability::sqlite::tests --locked` passes
4/4. A captured JSON event from the production profile callback contains the
normalized `tasks` database identity, operation category, duration, and
`duration_observed`; SQL text and bound values are absent. This does not reveal
statement success/failure or prove production reader/UI reconciliation. The
initial test expected `tasks.db`; the emitted identity is intentionally
normalized to `tasks`, and the assertion was corrected.

Python 3.14.7 / pytest 9.1.1 passes the checked-in Python fixture's
`compileall` build intent and separate pytest intent (1/1), with bytecode
redirected under `target/`. This is fixture evidence only; Python production
worker/AppContainer proof remains open. P4-50 and P4-11 remain active.

The focused `cargo test -p arcana-core --lib observability::logging::tests
--locked` suite also passes 4/4. Its default-filter test confirms fast SQLite
DEBUG events remain in structured JSONL while console/readable verbosity stays
at the default level.

### 2026-10-02 continuation — worker-request query plan

The first populated 160-row worker-request plan test reproduced a table scan
and temporary sort for the pending-status reader. A temporary measured
expression index on `(status, julianday(created_at) DESC, id DESC)` changed the
production-shaped query plan to an indexed search with no temporary sort. The
same index is now migration 10 as
`idx_worker_requests_status_created_at`; the production list method and plan
regression share `WORKER_REQUESTS_BY_STATUS_SQL`. The focused test passes. ADR-
LOG-RESTRUCT-0152 records the measured decision. P4-58/TG4-20 remain open; the
full production inventory and enforceable new-query registry are outstanding.

`task_board_reads_use_primary_and_filter_indexes_with_populated_rows` now
checks shared production SQL for task point reads, status-filtered board reads,
and worker/status-filtered reads against 200 migrated task rows after
`ANALYZE`. It confirms the rowid primary key, `idx_tasks_status`, and the
applicable status/assignment index. The focused test passes; the unfiltered
board list is explicitly a full-result query and remains an inventory exception.

### 2026-10-02 continuation — workspace consent UI contract

`Settings.test.tsx` exercises the consent card's read/grant/refresh/revoke
calls and proves consent for one workspace is hidden in another through the
mocked Tauri `invoke` boundary; it passes 1/1 and TypeScript typecheck passes.
Rust Tauri command tests separately cover persisted scope.
This is command-contract/UI-state proof only; the live desktop IPC workflow,
provider-wrapper revocation race, and complete consumer audit remain open under
P4-60/TG4-16.

### 2026-10-02 continuation — query-plan evidence

Populated production-SQL plan tests now cover task claim/orphan and atomic
ready-task dispatch, assigned-task claims, exact-owner interrupted recovery,
task-board reads, routing history/affinity, ACP recipient/task/recent/stale
inbox and count reads, worker-request status filtering, dashboard analytics,
latest worker tick, fleet roster/progress, and open-run recovery. Analytics
migration v3 adds measured timestamp expression indexes; task migrations 11/12/13
add measured ready-dispatch, recent-routing, and recent/stale ACP indexes.
Local-day UTC ranges discriminate canonical `T` rows from legacy
space-separated rows. Analytics, ready-dispatch, routing/affinity, ACP, exact
owner recovery, and fleet health focused tests pass. This remains partial
P4-58/TG4-20: diagnostic scope, additional request variants, and an enforceable
query registry remain.

The Python intent is now set to `compileall` for build evidence plus a separate
Python test run for extra confidence. The production Python AppContainer launch
still fails before spawn because its installed `C:\Python314` runtime directory
does not grant the sandbox read/execute access; the compatible ownership/staging
approach is still awaiting the owner's choice. Do not access the owner's
`C:\Playground\arcana-calendar`; the final calendar acceptance run belongs to
the owner.

### 2026-10-02 continuation — worker summary and diagnostic query plans

The cross-database `get_dashboard_summary` worker reads now share SQL constants
with a populated 1,000-row regression. Migration 5 adds
`idx_workers_created_date` after the baseline date query reproduced a roster
scan; the indexed query returns 500 workers added today. Exact total and active
roster counts return 800 and 600 and remain explicit full-count exceptions.
`dashboard_worker_summary_uses_date_index_and_preserves_counts` passes.

The diagnostic inventory test now EXPLAINs all five fixed `COUNT(*)` statements,
creates the migrated analytics schema in a temporary database, and calls the
real `row_counts_for_diagnostic` reader. Worker requests cover filtered and
unfiltered production list variants. The query inventory and MDP/TG/readiness
records reflect these results; ADR-LOG-RESTRUCT-0158 records the measured index.
P4-58/TG4-20 remain open pending a mechanically enforceable new-reader registry
and final in-scope reader audit. Python build intent is `compileall` plus a
separate Python test, as approved by the owner.

### 2026-10-02 continuation — task-health evidence query

Latest task build-evidence SQL is now shared between `latest_build_evidence`
and the common `qc_review` transition guard. The populated 512-record test
confirms both SELECT plans use `idx_build_evidence_task_id`, reads back the
newest passing build, and completes the production status transition after an
actual task claim. `latest_build_evidence_and_qc_guard_use_task_index_with_populated_rows`
passes. P4-58/TG4-20 remain open for the remaining in-scope SQL audit and the
explicit query-registration mechanism.
