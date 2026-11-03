# P4-54 / TG4-28 — startup retry count is persisted and Supervisor-visible — 2026-10-20

Closes the "retry counters" item in P4-54's open list and the gate clause that asks the Supervisor to
read retry state and distinguish a startup retry from a mid-run interruption.

## What was missing

Startup retries already moved the durable phase to `retrying` (`RecipeRunner`'s on-retry callback), but
nothing counted them. A Supervisor polling between attempts saw a phase with no magnitude, and after
exhaustion saw only `failed` / `failure_class="io"` — the number of attempts actually spent was not
persisted anywhere. The count also could not be derived reliably after the fact: the retry events are
telemetry, not a durable ledger.

## Implementation

**Schema (ladder v11, `runs.db`).**

```sql
ALTER TABLE worker_run_state ADD COLUMN startup_retry_count INTEGER NOT NULL DEFAULT 0
    CHECK(startup_retry_count >= 0);
```

Additive. The two tests that simulate older databases (`runs_v7_upgrade_...` and
`operation_journal_v7_upgrade_...`) now also drop the column before upgrading, so they keep exercising
a real v7/v6 → current ladder rather than a database that already has the column.

**One writer, one statement.** `RunsDb::record_worker_startup_retry(run_id, retry_count)`:

```sql
UPDATE worker_run_state
SET phase = 'retrying', startup_retry_count = ?1, heartbeat_at = ?2
WHERE run_id = ?3
```

Phase and count move together. Two separate writes would admit a read of `retrying` with a stale
count, or a count with a phase already back at `running`, which defeats the purpose of the field.
`RecipeRunner`'s on-retry callback calls this with the attempt number (the retries scheduled so far);
the previous `update_worker_tick_phase(..., "retrying", ...)` call is gone.

**Semantics.** The value counts **retries after the initial attempt**. With `STARTUP_MAX_ATTEMPTS = 4`
exhausted, the persisted value is 3. It is named `startup_retry_count`, not `retry_count`, because
mid-run retries are a separate class: a worker that started successfully and failed mid-run reads 0.

**Reader and Supervisor visibility.** `WorkerRunState.startup_retry_count` is selected by both
`latest_worker_tick` and `latest_worker_ticks`; `list_worker_health` exposes it as
`run.startup_retry_count` next to phase, failure class, journal, checkpoints and token usage.

**Legacy rows.** A v10 database upgrades and its existing rows read 0. That is the truthful value: the
run predates the counter, so no retry was ever recorded — deriving one from elapsed time would invent
data.

**Failure stays non-fatal.** A failed write remains a `worker_startup_retry_state_failed` telemetry
event rather than a failed tick. Telemetry must not be able to block application work; the cost is a
stale count, which is visible as such.

## Evidence

`dual_worker_failure_health_tests::dual_worker_failures_persist_distinct_classes_visible_to_supervisor_without_duplicate_dispatch`
drives two real workers through the production runner with a scripted provider: one fails mid-run
after a journaled write, one exhausts startup retries before any provider dispatch. It now asserts:

- startup worker: Supervisor readback `startup_retry_count == STARTUP_MAX_ATTEMPTS - 1` (3) **and**
  the persisted `WorkerRunState.startup_retry_count` equals 3;
- mid-run worker: `startup_retry_count == 0`, so the two failure classes cannot be confused.

New `db::runs::tests::worker_run_state_upgrade_defaults_startup_retry_count_for_existing_rows` proves a
populated v10 database upgrades, the pre-existing tick keeps its id and phase and reads 0, and the new
writer works afterwards (count 2, phase `retrying`).

Test runs (Windows, `--test-threads=1`, current tree):

| Command | Result |
|---|---|
| `cargo test -p arcana-core --lib -- db::runs::tests` | **31/31** (128.7 s) |
| `cargo test -p arcana-core --lib -- list_worker_health` | **9/9** (3.1 s) |
| `cargo test -p arcana-core --lib -- dual_worker_failures_persist_distinct_classes` | **1/1** |
| `cargo check -p arcana-core --locked` | clean (pre-existing warnings only) |
| `rustfmt --check` on the three touched files | clean |

## Falsification (each removed, then reverted)

| Probe | Result |
|---|---|
| On-retry callback writes phase only (`update_worker_tick_phase`) | Failed: `left: Number(0) right: Number(3)`. |
| Writer called with `attempt + 1` | Failed: `left: Number(4) right: Number(3)` — the off-by-one is detected. |
| `startup_retry_count` removed from the tool JSON | Failed: `left: Null right: Number(3)`; the persisted assertion before it still passed, so the tool layer is proven separately. |

## Deliberately not claimed

- Mid-run retries are not counted; only startup retries are, and the field name says so.
- `max_attempts` is a compile-time constant, not persisted per row; a future change to the constant
  would not rewrite historical rows (their counts stay factual, their limit is not recorded).
- Worker parent/child links and bounded temporary-worker creation remain open (the other two items the
  TG4-28 note names).
- No live provider, no user database, no network.
