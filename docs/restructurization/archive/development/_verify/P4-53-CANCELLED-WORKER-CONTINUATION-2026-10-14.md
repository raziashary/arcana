# P4-53 cancelled worker continuation

Date: 2026-10-14. Host: Windows. Scope: scheduler cancellation after
multiple real worker mutations and lost acknowledgement.

The existing shipped `worker-backend` recipe test fixture now injects three
explicit interruption modes: provider error, provider panic and scheduler
cancellation. In each run the actual AgentEngine claims a persisted task,
executes two registered `write_file` calls, and sees a failed append of the
second operation-journal outcome after the file mutation. The cancellation
mode waits until that unknown result has reached the provider, then aborts and
joins the scheduler-owned kick. Recovery records the interrupted run, clears
only the exact claim and leaves the task assigned in `needs_info`.

The common continuation checks reopen task/run/checkpoint/journal stores,
read both changed files, and require the registered Supervisor tools to
host-verify the uncertain effect before release. An audited override assigns
only the remaining work to another runnable worker. That worker reads the
saved task and both artifacts, writes `continuation.txt` once, runs a
Rust-owned Cargo build and enters QC. The original two writes are never
replayed.

Focused commands, each 1/1 passed:

- `cargo test -p arcana-core --lib shipped_worker_cancel_after_lost_ack_requires_reconciliation_before_handoff -- --nocapture` (18.06 seconds after compilation)
- `cargo test -p arcana-core --lib shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement -- --nocapture` (17.57 seconds)
- `cargo test -p arcana-core --lib shipped_worker_panic_after_lost_ack_requires_reconciliation_before_handoff -- --nocapture` (17.33 seconds; expected injected panic appears in output)

The fixture uses isolated workspace/profile storage, offline Cargo lockfile
generation and a scripted provider; it makes no live provider request. This
adds the named cancellation case but does not prove whole-app process-loss
restart, other effect classes, conflict refusal or task-scoped restore, so
P4-53/TG4-27 remain open.
