# P4-53 executor fixture refresh

## Scope

Update only `cfg(test)` fixtures in `arcana-core/src/agent/tool_executor.rs` that fail the new task-backed `RecoverySnapshot::capture` admission. Keep the existing assertions and avoid claiming this proves production positive reconciliation; the test seam remains ledger-only.

## Progress

- [x] Read MDP §0, including §0.2e evidence requirements, §0.2i incremental report requirement, §0.3 no-stale rule, §0.8 live-data discipline, and §0.12 re-read requirement.
- [x] Read `_verify/P4-53-HOST-SNAPSHOTS-2026-10-13.md`.
- [x] Find all affected executor test fixtures and identify the minimal task/workspace setup.
- [x] Update fixtures and assertions to use generated task IDs and a selected workspace outside Arcana home.
- [x] Run the offline focused executor module with live credentials removed.
- [x] Record exact outcomes and limitations.

## Findings

Before the change, `unresolved_prior_task_effect_blocks_executor_handler_until_reconciled` used task `42` without creating a task or selecting a separate workspace, so snapshot capture could not reach the intended unresolved-effect admission assertion. The cancellation fixture also sets task `42`, but cancels during lease admission before journaling and snapshot capture.

The initial requested filter `agent::tool_executor::tests` compiles the crate but matches zero tests: this file's test module is named `p8_execution_registry_tests` at line 1474. The focused run therefore used that source-verified module name so the changed and adjacent cancellation fixtures actually executed.


## Fixture change

`unresolved_prior_task_effect_blocks_executor_handler_until_reconciled` now creates a task in the isolated TasksDb, assigns it to `worker-backend`, claims it, and uses its generated ID consistently for the prior ledger row, executor, and resumed-row assertion. Its selected workspace is a separate temporary directory, outside the Arcana home. Assertions about blocking, zero handler calls, no appended intent, explicit RetrySafe reconciliation, successful retry, and task attribution are unchanged.

The cancellation-before-journal fixture still uses ID 42 intentionally: it cancels while waiting for write admission, before `begin_operation_journal` or snapshot capture; its focused execution passed.


## Validation and limitations

- The dispatch-provided filter `cargo test --offline -p arcana-core --lib agent::tool_executor::tests --locked -- --test-threads=1` compiled successfully but matched **0 tests** (`1122 filtered out`). Source identifies this file's test module as `p8_execution_registry_tests`, so the test-bearing focused run used that corrected filter.
- `cargo test --offline -p arcana-core --lib agent::tool_executor::p8_execution_registry_tests --locked -- --test-threads=1`: **20 passed, 0 failed, 0 ignored, 1102 filtered out**, finished in 3.00s. Both the refreshed unresolved-effect fixture and the unchanged cancellation-before-journal fixture passed. Cargo emitted existing unused/dead-code warnings.
- Live credential environment variables were removed from the PowerShell process before each Cargo invocation. The queued corrected invocation was canceled before it acquired the build lock; the completed corrected run also ran with those variables removed.
- This verifies executor journal admission and ledger reconciliation assertions only. It does not prove production positive reconciliation through host snapshot verification.
