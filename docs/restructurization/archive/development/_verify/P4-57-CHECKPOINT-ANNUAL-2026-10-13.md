# P4-57 Checkpoint Annual Pair Proof — 2026-10-13

Status: investigation started. This report is updated as source and execution evidence are established. It does not close P4-57 or TG4-19.

## Scope and constraints

- Add a focused `#[cfg(test)]` proof in `arcana-core/src/git/checkpoint.rs` for the existing checkpoint writer and production readers.
- Drive 4,400 approved test-clock instants from the existing leap-year/DST corpus through actual checkpoint creation and readback, including a database reopen.
- Preserve the stored integer Unix-millisecond contract. Use an isolated temporary Arcana home and a separate workspace. Do not access user databases, providers, Vault, or Calendar.
- Existing retention is 50 rows. Assert the intended production reader at each step and after reopen; do not claim that all 4,400 rows remain retained at the end.
- No production/API/schema/dependency changes. Exact authorized test command is `cargo test --offline -p arcana-core --lib generated_checkpoint_writes_round_trip_a_leap_year_and_dst --locked -- --test-threads=1`, after clearing live credentials per-command and waiting for root build coordination.

## Source baseline

- MDP P4-57 requires timestamp writer/reader inventory coverage and old persisted-row readability.
- TG4-19 requires annual/DST coverage at every production write/read pair.
- `_verify/P4-57-PRODUCTION-PAIR-INVENTORY-2026-10-12.md` identifies `config.db: checkpoints.created_ms` as an integer epoch-millisecond production pair: `create` writes it; `list` and latest-per-file paths order/read it; no annual test was present.
- ADR0375/0376 authorize the isolated test-only clocks and implementation path. This test will reuse the existing `ClockScope` and `checkpoint::now_ms` behavior without adding runtime consumers or APIs.

## Progress

- [x] Create this report first.
- [x] Inspect checkpoint writer/readers, existing fixture, corpus, clock scope, and retention behavior.
- [x] Add focused test-only production-path proof.
- [x] Run the single authorized offline test after root build coordination; record exact command, test count, and outcome.
- [x] Review assigned-file diff and whitespace; report remaining limits without changing gate state.

## Source findings and test shape

- `checkpoint::now_ms` uses `db::test_clock::current()` under `cfg(test)` and otherwise retains `SystemTime` behavior. The test uses the existing `at()` scope and corpus generator.
- The test calls `create_within_workspace`, which validates the target under a separately-created workspace and reaches the actual `create` writer. The Arcana home, and therefore `config.db` plus snapshot directory, is a different temporary root.
- `create` opens the schema-owned config database for the write and pruning; `list` opens it independently and reads `created_ms` as `i64`, ordered descending. The test uses `list` after every write and again after all writer clock scopes end.
- The corpus is sorted chronologically before creation because its DST-edge entries are appended after the annual points. Equal-millisecond values are assigned to separate positive checkpoint `task_id` ownership scopes, since the production reader orders only by `created_ms` without an ID tie-break. The fixture IDs exercise checkpoint association/filtering only; this test does not create actual TasksDb lifecycle records or prove restore behavior. This makes each newly-created ID readable before the existing 50-row retention policy can evict it. Snapshot filenames also include a UUID, so equal timestamps do not collide on disk.
- Each scope keeps one file and exercises the existing retention limit; assertions compare each production list with the expected most-recent rows. The final reads assert only retained rows, not that every corpus row survives.

## Execution evidence

The first coordinated, credential-cleared invocation of `cargo test --offline -p arcana-core --lib generated_checkpoint_writes_round_trip_a_leap_year_and_dst --locked -- --test-threads=1` failed during compilation before test execution (0 tests ran). The compile errors were in the separately-owned `arcana-core/src/db/test_clock.rs`; that owner corrected the moved `episodic_id`/`semantic_id` references before the retry. This checkpoint task did not modify that file.

After the shared source fix and root coordination, the same exact command passed: **1 passed, 0 failed, 0 ignored, 0 measured, 1,125 filtered out**, with the test itself finishing in **112.33 seconds** and the build finishing in **61 seconds**. The compile reported 45 warnings across the crate. The test executed all 4,400 actual `create_within_workspace`/`create` writes, performed production `list` readback after every write (including the new ID before retention could evict it), and read retained results again after all clock scopes ended. No gate/task state was changed.

`git diff --check -- arcana-core/src/git/checkpoint.rs` passes. The diff is limited to the new test in the assigned source file plus this report. The existing integer-millisecond representation is covered through the actual writer's `i64` return and production reader. This test does not add a separate old-row migration/legacy fixture, restore behavior, cross-platform evidence, or complete P4-57/TG4-19 coverage.
