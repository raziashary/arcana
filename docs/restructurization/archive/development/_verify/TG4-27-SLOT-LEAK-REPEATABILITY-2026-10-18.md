# TG4-27 — lost-ack trio repeatability: root cause was a production slot leak

**Date:** 2026-10-18
**Commit:** `a2e7aa8` (`fix(recipes): release the process-wide worker slot after a successful kick`)
**Gate:** TG4-27 (continuity). The trio is this gate's proof vehicle for
"completed mutating side effect with lost acknowledgement"; TG4-27 remains **open**.

## Symptom

Three shipped tests:

- `recipes::runner::production_git_commit_recipe_tests::shipped_worker_preserves_multiple_mutations_and_quarantines_lost_acknowledgement`
- `...::shipped_worker_panic_after_lost_ack_requires_reconciliation_before_handoff`
- `...::shipped_worker_cancel_after_lost_ack_requires_reconciliation_before_handoff`

Each passed 1/1 alone (~17-25 s). Any two in one process failed the later one at
`runner.rs:2204` — `replacement worker settles: Elapsed(())` — i.e. the replacement
worker's run never reached a terminal outcome within the 90 s settle window. The
earlier receipt for this gate recorded a `command_spawn_failed` symptom; today's runs
show a settle timeout. Both are the same consequence.

## Root cause (production defect, not test isolation)

`recipes/scheduler.rs::kick_worker` acquires a process-wide `kick_inflight` slot and hands
ownership to the spawned run. The code's own comment states the contract: "From here on we
MUST remove ourselves from `kick_inflight` on every exit path - otherwise the worker is
permanently locked out. ... the success path hands ownership to the spawned task which clears
the slot on completion."

- Failure / panic / timeout: the run's `KickSlotGuard` drops with `active = true` and removes
  the slug (`scheduler.rs:810-828`).
- Success: the arm called `run_guard.disarm()`, which sets `active = false` and so suppresses
  **both** the interrupted-claim recovery **and** the slot removal.

Both dispatch paths consult the set:

- `kick_worker` bails with "another kick is already in flight" when the insert fails
  (`scheduler.rs:469-477`).
- The periodic tick's `try_begin_worker_run` returns `None` -> "Cron tick skipped (another run
  is in flight)" (`scheduler.rs:696-701`, used at `:267`).

So one successful push-based kick locked that worker out of push **and** periodic dispatch for
the life of the process. In the trio, the first test to hand off to `worker-frontend` leaked its
slot; every later test's replacement-worker kick was a silent no-op, so it waited out the 90 s
timeout. This is exactly the "first test wins, later tests starve" signature.

## Fix

Separate the two responsibilities the single flag had merged:

```rust
/// The run completed successfully, so there is no interrupted claim to
/// recover — but the process-wide slot must still be released when this
/// guard drops. `disarm()` cannot be used here: it suppresses the slot
/// release too, which locks the worker out of every later kick (push or
/// periodic tick) for the life of the process.
fn skip_recovery(mut self) {
    self.recovery = None;
}
```

Success arm: `Ok(Ok(())) => run_guard.skip_recovery(),`. The guard now drops normally on
every exit path, releasing the slot; only the recovery closure is skipped on success.

## Proof

| Check | Command | Result |
|---|---|---|
| New assertion fails without the fix | `scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges` with `!kick_inflight().contains("qc-agent")` | **FAILED** at the new assertion (leak reproduced through the production path) |
| Same test with the fix | same command | 1/1, exit 0 |
| Guard-level both halves | `kick_slot_guard` filter | 4 passed, 0 failed |
| The trio, co-run in one process | `cargo test -p arcana-core --lib --locked -- --test-threads=1 production_git_commit_recipe_tests::shipped_worker` | **3 passed, 0 failed**, 52.98 s, exit 0 |

The existing successful-QC-kick test previously could not distinguish "no work on the second
kick" from "the slot is still held", which is why the leak stayed invisible; it now asserts the
slot directly.

## Why the fix is in production, not in test isolation

Isolating each test in its own process (or relying on `--test-threads`) would have made the
suite green while leaving a worker permanently un-dispatchable after one successful run. The
code's own documented contract says the slot must be released; the fix restores that contract.

## Remaining TG4-27 clauses (unchanged, still open)

- Whole-app process loss across a real restart, with an explicit Supervisor-approved manual
  handoff.
- Broader mutation classes beyond the covered ones.
- Repository-snapshot breadth (staged / unstaged / untracked) and task-scoped restore under the
  full fleet loop.

## Owner-judgment item carried forward

`do_not_retry` reconciliation decisions skip `RecoverySnapshot::verify_state` — safe (the intent
stays unresolved) but conflict wording is enforced only on continuation paths. Recorded in
`_verify/TG4-27-CONFLICT-REFUSAL-2026-10-18.md`; unchanged by this work.
