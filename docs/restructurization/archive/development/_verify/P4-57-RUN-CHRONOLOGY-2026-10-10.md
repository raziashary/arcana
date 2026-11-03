# P4-57 worker-run timestamp consumer proof — 2026-10-10

## Verified behavior

The existing `worker_run_phase_heartbeat_progress_and_terminal_outcome_round_trip`
test drives the actual `RunsDb` begin, phase, heartbeat, progress and finish APIs.
It checks canonical millisecond UTC shape and shared-parser readability, exact
creation equality between the initial run and worker state, exact progress
equality between the worker state and recent progress event, and exact terminal
equality between run completion and worker heartbeat. After dropping and reopening
the database, both creation projections, the progress state/event and the terminal
state/run retain the same instants.

Independent GPT-6 Luna High review identified a missing creation-time reopen
assertion in the first version. The final test includes it. Non-strict chronology
assertions permit equal millisecond values; they describe normal observed wall-clock
ordering and do not establish immunity to a backward system-clock adjustment.

## Execution receipts

`cargo test --offline -p arcana-core --lib worker_run_phase_heartbeat_progress_and_terminal_outcome_round_trip --locked -- --test-threads=1`

- Windows: 1 passed, 0 failed; 0.09 seconds test execution.
- Ubuntu WSL: 1 passed, 0 failed; 0.13 seconds test execution.
- Credential variables were removed per command. Only disposable test storage was
  used; no provider calls, user databases, servers or listeners were involved.
- Logs: ignored `target/p4-ci-checks/p4-run-chronology-{windows,linux}-2026-10-10.log`.

## Scope limits

This changes one existing test, not production schema, clocks, writer or reader
contracts. It resolves the concrete missing progress-event/readback assertions
identified in the TasksDb/RunsDb inventory. It is not a full-year corpus at every
database pair, and it does not close P4-57 or TG4-19. Inventory:
`P4-57-TASK-RUN-PAIR-INVENTORY-2026-10-09.md`; review:
`P4-57-RUN-CHRONOLOGY-REVIEW-2026-10-10.md`; decision: ADR-0356.
