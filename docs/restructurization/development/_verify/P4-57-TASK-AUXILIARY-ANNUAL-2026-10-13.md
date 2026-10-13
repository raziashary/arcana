# P4-57 TasksDb Auxiliary Annual Pair Proof — 2026-10-13

Status: the focused annual test passed. This report is updated from current source and execution evidence; it does not close P4-57 or TG4-19.

## Scope and constraints

- Add one `#[cfg(test)]` annual producer/reader test in `arcana-core/src/db/test_clock.rs` for still-uncovered generated-now auxiliary fields owned by TasksDb.
- Use the existing isolated 4,400-instant leap-year/DST corpus and test-only clock. Exercise real writer APIs, reopen the TasksDb, and assert through public production readers.
- Do not edit production database/recovery sources, schema, or unrelated files. Do not invent direct-SQL reader proof. No credentials, live DBs, providers, Vault, network, Calendar, or Git mutations.
- Run only the exact authorized test filter after implementation, with credentials stripped. No broad suite or task/gate checkbox changes.

## Progress

- [x] Create this report first.
- [x] Ground exact remaining Task timestamp pairs against current inventories, source writers, and public readers.
- [x] Add one focused test-only annual auxiliary-pair proof.
- [x] Run the exact authorized test and scoped formatting/diff checks; record exact counts and limits.

## Source findings and pair selection

The current `_verify/P4-57-PRODUCTION-PAIR-INVENTORY-2026-10-12.md` and `_verify/P4-57-TASK-RUN-PAIR-INVENTORY-2026-10-09.md` identify these still-uncovered TasksDb generated-now producer/public-reader pairs:

- `tasks.completed_at`: status completion writes via `update_status_with_routing_outcome`; `list`/`get_by_id` return it through the public task DTO.
- `worker_requests.created_at` and `resolved_at`: `create_worker_request` and `update_worker_request_status`; `list_worker_requests`/`get_worker_request_by_id` read both.
- `worker_messages.created_at` and `read_at`: `insert_worker_message`, then the production inbox reader `list_worker_messages_for` advances `read_at`; `get_worker_message`/`list_recent_worker_messages` return both.
- `routing_history.assigned_at` and `outcome_at`: `TaskRouter::assignment_entry`, `assign_with_routing_history`, and the status/outcome transition; `get_routing_history` reads both.
- `build_evidence.created_at`: production `record_build_attempt_for_worker` after an assigned task is claimed; `latest_build_evidence` returns the persisted timestamp. This fixture proves timestamp storage/readback only; it does not assert that a real command ran or manufacture QC proof.
- `task_budget_resolutions.resolved_at`: the Supervisor-authorized `resolve_exceeded_task_budget` path; `get_task_budget_resolution_history` returns it. The reader is capped at 20 per task, so the test will use one disposable audit task per instant and read each task's one-row history after reopen.

The test uses 4,400 disposable task records, each with a separate disposable budget-audit task, so public reopened readers can recover every annual row through existing bounded APIs. `project_build_batches.created_at` has no production projection; `completed_at` is predicate/state-only. `build_retry_approvals.created_at` and `consumed_at` have no application reader and are state/trigger-only. Those fields remain inventory-only and will not receive a fabricated timestamp reader.

## Implemented test shape

`generated_task_auxiliary_writers_round_trip_a_leap_year_and_dst` uses a temporary Arcana home and explicit temporary `tasks.db`. For each of the 4,400 approved instants it creates and assigns a task, claims it with the real worker API, records build evidence through `record_build_attempt_for_worker`, creates and resolves one normal worker request, inserts and reads one inbox message, and completes the task through the status/routing outcome path. It creates a separate task per budget-resolution fixture because the public history reader is intentionally capped at 20 rows per task. After closing and reopening TasksDb, the test checks each stored timestamp through the public task list, request list, recent-message list, routing-history list, latest-build-evidence reader, or per-task budget-resolution history reader. The per-step inbox read assertion confirms the production reader-generated `read_at` before the final reopened-list check.

The build-evidence fixture calls the persistence API with a recorded exit result; it does not execute a shell command, prove process provenance, or claim QC passed. The task completion fixture proves timestamp storage/readback only and is not task-restoration or lifecycle proof.

## Execution evidence

With matching API-key, token, secret, credential, password, and `DATABASE_URL` environment variables removed for the command, ran:

```text
cargo test -p arcana-core --lib generated_task_auxiliary_writers_round_trip_a_leap_year_and_dst -- --test-threads=1
```

The initial run result was `1 passed; 0 failed; 0 ignored; 0 measured; 1129 filtered out`, with the test itself finishing in `160.13s`. Each of the 4,400 corpus instants produced one distinct task-completion timestamp, request created/resolved pair, message created/read pair, routing assigned/outcome pair, build-evidence creation timestamp, and separate budget-resolution timestamp. Every value was read back through its named production reader after reopening TasksDb; the inbox reader also verified its generated read timestamp at every iteration before reopen. ~~Timestamp comparisons parsed the stored epoch strings and compared them to the controlled UTC instants, preserving the production integer epoch representation without replacing actual writer/reader behavior with SQL assertions.~~ Correction: these nine persisted timestamp fields are TEXT instants, not integer epochs. The test now parses each raw value with the shared timestamp parser, asserts that formatting the parsed instant with the shared formatter reproduces the exact stored text, and then compares the instant with its controlled UTC value. This checks canonical representation without replacing actual writer/reader behavior with SQL assertions.

After adding the canonical-text assertion, reran the same exact credential-stripped command. Result: `1 passed; 0 failed; 0 ignored; 0 measured; 1129 filtered out`; test runtime was `172.09s`. All nine raw timestamp fields passed the byte-for-byte shared-format check and controlled-instant equality. Scoped `rustfmt --edition 2021 --check arcana-core/src/db/test_clock.rs` and `git diff --check` on the assigned source/report passed after this change. Compilation again emitted warnings in unrelated modules, without errors.

Scoped checks passed: `rustfmt --edition 2021 --check arcana-core/src/db/test_clock.rs` and `git diff --check` on the assigned source and report. `cargo fmt -p arcana-core -- --check` initially reported style differences in the new test; the assigned file was formatted with Rust 2021 rustfmt, after which the scoped rustfmt check passed. The test build emitted existing warnings in other modules; it compiled successfully. No live resources or provider credentials were used. This proves generated timestamp persistence/readback for these fields, not real command execution, QC approval, restore lifecycle, or the remaining fields without public readers.
