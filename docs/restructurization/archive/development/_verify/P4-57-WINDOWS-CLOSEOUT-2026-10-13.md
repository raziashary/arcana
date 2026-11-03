# P4-57 / TG4-19 Windows closeout — 2026-10-13

ADR0391 assigns equivalent Linux/macOS timestamp-pair execution to
P9-16/TG9-06; P4-57/TG4-19 retains every Windows production write/read pair.
ADR0388 preserves canonical UTC text for textual timestamps, equivalent
SQLite defaults/triggers and integer milliseconds for checkpoints.

The source inventory and eight generated-now test groups are reconciled in
`P4-57-FINAL-CLOSURE-DOSSIER-2026-10-13.md`. Their recorded Windows passes
exercise 34 physical generated-now fields through actual writers and intended
readers across a 4,400-instant leap-year/DST corpus. The existing annual
Session, Analytics, worker-history and hook-log receipts cover their own
writers/readers and execution-time preservation. The CI workflow selects the
generated and legacy test filters; this receipt does not claim remote CI
execution on other hosts.

The three disposable file-backed legacy fixtures use public database readers
after reopen. The earlier receipt
`P4-57-LEGACY-PLATFORM-COVERAGE-2026-10-13.md` covers task auxiliary,
run/journal and worker/plan/hook configuration fields. The final two missing
fields now have explicit production-reader assertions:

- `tasks.completed_at`: complete a claimed task through `TasksDb::update_status`,
  seed an offset-form legacy completion value, reopen, then read it with
  `TasksDb::list(Some("done"))` and assert the represented instant.
- `runs.ended_at`: finish through `RunsDb::finish`, seed the legacy end value,
  reopen, then read with `RunsDb::get` and assert the represented instant.

The exact Windows command
`cargo test --offline -p arcana-core --lib preserve_legacy_rows_after_reopen --locked -- --test-threads=1`
passed **3/3**, with 1,131 other tests filtered out. The test-only source
diff is in `arcana-core/src/db/test_clock/legacy_pairs.rs`. No live profile,
provider, Calendar run, server or user database was used. The annual receipts
and focused legacy test jointly satisfy the named Windows pair contract;
Linux/macOS parity remains a blocking P9 gate.
