# P4-54 / TG4-28 — worker parent/child dispatch lineage — 2026-10-28

Closes the "worker parent/child links" item in P4-54's open list and the gate clause that asks the
Supervisor to read parent/child state. ADR-LOG-RESTRUCT-0412 records the decision.

## What was missing

`runs.parent_run_id` has existed since the first ledger migration and `Run` read it, but
`begin_worker_tick` never wrote a value. On a real ledger every worker tick carried `NULL`, so the
column asserted a relationship no code recorded. The Supervisor could read a worker's phase, failure
class, journal and checkpoints, but not **which run woke it** or **which workers a supervisor tick
woke** — the two facts needed to tell "my child is still running" from "this is an orphan".

## Implementation

**Write path (origin travels with the kick).**

| Layer | Change |
|---|---|
| `db::runs` | `begin_worker_tick_with_parent(slug, session, Option<&str>)`; `begin_worker_tick` delegates with `None` |
| `recipes::runner` | `run_recipe_dispatched_by(recipe, worker, dispatch_origin)`; `run_recipe` delegates with `None` |
| `recipes::scheduler` | `kick_worker_dispatched_by`, global `kick_dispatched_by`; `kick_worker` / `kick` delegate with `None` |
| tools | `assign_task`, `assign_tasks_batch`, `send_worker_message`, `update_task_status`, `request_supervisor_action`, `create_task` pass `ctx.run_id.clone()` |

Cron and launch-time kicks pass `None`. The origin is **not** resolved from `GLOBAL_SCHEDULER` or any
process-global: that would depend on which process installed the scheduler first and would be
unobservable in a test. The recorded origin is the run whose tool call performed the kick.

**Schema (ladder v12, `runs.db`).**

```sql
CREATE INDEX idx_runs_parent_dispatch_order
    ON runs(parent_run_id, julianday(started_at) DESC, id DESC);
```

Measured before adding: the children read planned as a full `runs` scan followed by
`USE TEMP B-TREE FOR ORDER BY`. The two simulated-legacy fixtures (v6/v7) and the v10 fixture drop the
index before upgrading, and five schema-version assertions moved 11 → 12.

**Read path (both directions in one health read).**

- `LATEST_WORKER_TICK_BY_SLUG_SQL` and `LATEST_WORKER_TICKS_SQL` select `r.parent_run_id` through the
  existing `runs` join (no plan change: `r.id = s.run_id`).
- `RunsDb::children_of(run_id, limit)` returns the ticks a run dispatched, newest first, bounded; the
  caller can tell the bound was hit.
- `list_worker_health` adds per worker:
  - `run.parent`: `known` (run id, kind, actor, started_at, outcome, open), `none` for an origin-less
    tick, `missing` for a dangling id, `unknown` with a safe error class on read failure;
  - `run.dispatched_workers`: `state`, `count`, `complete`, and per child run id / worker slug / phase /
    task / outcome / started_at, bounded by `journal_limit`.
- `RunsDb::get` (the parent resolver) is now a production reader; its SQL was lifted to
  `RUN_BY_ID_SQL` so it has a plan assertion instead of the old "no production caller" exclusion.

**Refusal of invented lineage.** The writer probes `SELECT 1 FROM runs WHERE id = ?1` in the same
transaction before inserting and returns "the dispatching run … is not in the run ledger".

> Fact corrected in this unit: the earlier note said `runs.db` does not enable SQLite foreign keys, so
> the probe was the only guard. That is **wrong** — the bundled SQLite build enforces declared foreign
> keys by default, and with the probe disabled the write is still refused, as
> `Agent error: opening worker run: FOREIGN KEY constraint failed`. The probe is kept because the
> guarantee must not depend on a compile-time default no code here sets, and because the caller should
> read a refusal naming the run rather than a storage-layer constraint violation.

The test therefore asserts the refusal **phrase**, not merely `is_err()`, and pins
`PRAGMA foreign_keys = 1` so a build change is visible instead of silent. The forged-write fixture uses
`worker-platform`, a worker with no active run, so the one-active-run trigger cannot be what refuses the
write and mask a missing lineage check.

## Evidence

Test runs (Windows, current tree):

| Command | Result |
|---|---|
| `cargo test -p arcana-core --lib -- --test-threads=1 db::runs::` | **33/33** (144.1 s) |
| `cargo test -p arcana-core --lib -- --test-threads=1 supervisor_reads_dispatch_lineage_in_both_directions` | **1/1** (0.5 s) |
| `cargo test -p arcana-core --lib -- --test-threads=1 dispatch` | 20/21 → green after the plan-text fix; covers `dispatch_lineage_tests` and `dispatch_origin_tests` |
| `cargo test -p arcana --lib -- tg8_12_each_native_run_entry_path_holds_a_runtime_lease` | **1/1** |
| `cargo test -p arcana-core --test p4_query_registry -- --test-threads=1` | **9/9** |
| `cargo check --workspace --all-targets` | clean (pre-existing warnings only) |
| `cargo test -p arcanactl --features offline-fleet-test --test fleet_offline -- --test-threads=1` | **7/7** (46.9 s) |
| `rustfmt --check` on the three touched Rust files | clean |

New/changed production tests:

- `db::runs::tests::worker_ticks_record_their_dispatch_run_and_the_dispatcher_reads_them_back` — the
  dispatcher records `parent_run_id`; `children_of` returns exactly the dispatched ticks and honours the
  bound; an origin-less tick reads `None`; an invented dispatcher is refused with the domain phrase and
  leaves no row.
- `db::runs::tests::dispatched_ticks_read_through_the_dispatch_index_without_a_sort` — 64 children and
  512 unrelated runs; baseline without the index scans and sorts; the migration-installed index is
  sought, no `runs` scan, no temp B-tree; `RUN_BY_ID_SQL` plans as
  `SEARCH runs USING INDEX sqlite_autoindex_runs_1 (id=?)`; the production getter returns the stored
  dispatcher and `None` for the dispatching run itself.
- `recipes::runner::dispatch_lineage_tests::a_dispatched_worker_tick_records_the_run_that_woke_it`.
- `recipes::scheduler::dispatch_origin_tests::{a_kick_carries_its_dispatch_run_into_the_spawned_worker_tick,
  a_kick_without_a_dispatch_origin_records_no_parent}`.
- `agent::builtin_tools::list_worker_health::tests::supervisor_reads_dispatch_lineage_in_both_directions`
  — one supervisor tick, two dispatched children, one unrelated origin-less tick, driven through the
  real tool: `run.parent == none` for the dispatcher, `dispatched_workers.count == 2` with phase and
  slug, `parent.state == known` / `kind == fleet-tick` / `actor == supervisor` / `open == true` for a
  child, `none` for the launch kick, and no session id or home path in the payload.
- `src-cli/tests/fleet_offline.rs::offline_fleet_cli_completes_board_child_and_exports_observability`
  — full chain in a separate process: the CLI launch kick has no parent, the worker tick's
  `parent_run_id` is the supervisor tick's run id, and `children_of(supervisor)` returns that worker
  tick.

## Falsification (each mutation applied, run, then reverted)

| Probe | Result |
|---|---|
| Child tick inserted with `parent_run_id = NULL` | `db::runs` lineage test fails on "a dispatched tick must name the run that woke it"; the health tool fails on `dispatched_workers.count` (0 vs 2); the offline CLI test fails on "the worker tick must record the supervisor tick that dispatched it" (`left: None`). |
| v12 creates an equally-named but ineffective index (`ON runs(actor)`) | Plan test fails: `dispatched children must seek their parent's index; plan: ["SCAN s", "SEARCH r USING INDEX sqlite_autoindex_runs_1 (id=?)", "USE TEMP B-TREE FOR ORDER BY"]` — the index is load-bearing for the plan, not just present. |
| Explicit parent-existence probe disabled | Test fails: "the refusal must come from the lineage check, not a raw SQLite guard: Agent error: opening worker run: FOREIGN KEY constraint failed". This is the assertion that also documents the corrected FK fact. |

## Deliberately not claimed

- **Bounded temporary-worker creation** remains open (TG4-28 / P4-54): no temporary worker is created
  through validated registration yet.
- **A populated model/token-usage value proven end to end** remains open: the field is exposed and the
  reader has populated-row proof, but no scripted-provider run proves a non-zero value through the
  full loop.
- **Two-worker interruption/resume without duplicate effects** remains open; the 2026-10-18 test proves
  distinct failure classes and duplicate-dispatch refusal, not resume.
- **Cross-process stress testing** and **production heartbeat fault injection** remain open.
- This unit records lineage for worker ticks only. Non-worker runs (interactive turns, CLI invocations)
  still begin through `RunsDb::begin_with`, which has no dispatch origin; nothing here changes that.
- No live provider, no user database, no network; the offline Fleet fixture is dependency-free and
  offline.

## Observed while verifying (not caused by this unit, not fixed here)

`fleet_offline`'s success test fails intermittently when the whole 7-test target runs in parallel: the
board and lineage assertions pass, then "CLI log export must read the Fleet completion event" fails
because `fleet_cli_run_completed` is absent from the exported structured log. Reproduced **2/3 runs with
the pre-change test file restored** (`git stash push -- src-cli/tests/fleet_offline.rs`, three parallel
runs: fail, pass, fail), so it predates this work; serial runs were 7/7 on repeated attempts. The likely
mechanism is the asynchronous writer's bounded shutdown drain (the logging code already documents that
`WorkerGuard::drop` waits only a bounded time), so the last emitted event can be lost under load. A
completion event that exists in the process but not in the log is a telemetry-completeness defect and
needs its own unit; until then, this target's parallel result is not dependable evidence.
