# P4-36 / S4-03: in-process recovery and Supervisor handoff

Date: 2026-10-14. Host: Windows. Scope: P4-36 and S4-03 only.

## Production path

`run_if_recoverable_work_with_timeout` times out an active backend run after
`TasksDb::claim_task_with_pid` and `RunsDb::begin_worker_tick` persist its exact
owners. Scheduler recovery releases only that worker's claim, preserves its
assignment, marks the task `needs_info`, records the interrupted run, and calls
`kick("supervisor")`. The global scheduler resolves the shipped Supervisor
recipe and runs it through `RecipeRunner` and `AgentEngine`. Its registered
`list_worker_health` and `get_task` results reach the next scripted provider
turn. The script is test-only; the scheduler, recipe, engine, tools and DB
readers are production components. The test grants isolated workspace egress
consent and removes ambient Gemini keys, so it makes no paid provider call.

## Checks that can fail

- `cargo test -p arcana-core interrupted_worker_kick_reaches_shipped_supervisor_and_reads_held_task -- --nocapture`: 1/1 passed. Asserts the backend run is interrupted; its task stays assigned and held without a claim PID; the Supervisor run completes; the provider receives the shipped Supervisor prompt and production health/task tool results. The child process isolates the global scheduler registry.
- `cargo test -p arcana-core --lib shipped_worker_panic_after_lost_ack_requires_reconciliation_before_handoff -- --nocapture`: 1/1 passed. A real worker panics after two file mutations and a lost acknowledgement. The registered Supervisor reconciliation and lifecycle tools verify/release the held task before an audited override lets another worker complete only the remaining work and enter QC. The expected injected panic appears in test output.
- Earlier focused scheduler cancellation and timeout tests cover exact-owner release, retryable DB recovery and next-tick admission; see the P4-36 status and the prior P4-53 continuation receipt.

## Limit

This is an in-process recovery smoke. It does not close P4-53/TG4-27's
whole-app restart, other mutation/effect classes, conflict refusal and
task-scoped restore. P4-54/TG4-28 still require complete two-worker health
field and manual-handoff proof. Automatic process-loss reassignment belongs to
P7-19/TG7-06.
