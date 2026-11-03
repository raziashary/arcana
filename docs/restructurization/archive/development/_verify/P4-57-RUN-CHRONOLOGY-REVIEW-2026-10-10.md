# P4-57 worker run chronology review — 2026-10-10

## Scope and progress

- Independent read-only review of `worker_run_phase_heartbeat_progress_and_terminal_outcome_round_trip` in `arcana-core/src/db/runs.rs`.
- Check: whether assertions prove persisted creation/progress/terminal timestamps through production APIs and reopen; assess clock/order assumptions.
- Coordinator reports exact Windows/Linux test filters passed; I will not run tests/builds or mutate Git.
- Findings will be appended incrementally after inspecting the scoped test and called production methods only.

## Findings

- **Creation timestamp is not round-tripped across reopen.** `latest_worker_tick` selects `r.started_at` into `WorkerRunState.started_at` (`runs.rs:204-211`, reader at `:1257-1273`), and `get` returns `Run.started_at` (`:1344-1363`). The test does read `terminal_run.started_at` before dropping the connection and checks it parses and is no later than progress (`:2928`), so a production read does occur. But after reopening it checks only `reopened_run.ended_at`, the state heartbeat/progress timestamps, and event time (`:2930-2963`); it never asserts `reopened_run.started_at` or `reopened_state.started_at`. Also `started_instant` at `:2888` is derived from the initial heartbeat, not the run creation field. To prove persisted creation through reopen, capture `starting.started_at` (or `terminal_run.started_at`) and assert the reopened run and joined worker-state `started_at` parse to that same instant. The test currently proves creation readability/order before reopen, but not its reopen round-trip or exact tie to the initial state.

- **Wall-clock order assertions allow same-millisecond writes correctly, but assume the system clock does not move backward.** Production writers call `now_rfc3339()` independently at begin, phase/heartbeat, progress, and finish (`runs.rs:615,665-686,715,1180`); the test compares `progress >= starting heartbeat`, `ended >= progress`, and start `<= progress` (`:2896,2927-2928`). Because values have millisecond precision, `>=` appropriately permits equal instants. A backward wall-clock adjustment during the test could violate these inequalities even when writes are correctly persisted; the assertions are evidence of normal observed ordering, not monotonic-clock guarantees. No false strict-order (`>`) assumption found.

- Positive evidence: creation, progress, and finish are driven through production APIs (`begin_worker_tick`, `update_worker_tick_phase`, `heartbeat_worker_tick`, `record_worker_progress`, `finish_worker_tick`); the progress state and event share the exact timestamp, terminal state and run share the exact finish timestamp, and both progress and terminal timestamps are compared after reopen (`runs.rs:2855-2963`). The coordinator's Windows/Linux pass results were accepted as supplied; this review did not rerun them.

## Limits

- Read-only review of this one test and its called queries/writers. No schema-wide or adjacent test audit; no builds/tests/runtime execution/Git operation.

## Coordinator follow-up

- Coordinator reports the implementation was updated after this review finding: the worker chronology test now asserts the creation instant equals the initial worker-state `started_at` and verifies both reopened projections. This is follow-up information from the coordinator; the original observation above is retained as the state I reviewed.
