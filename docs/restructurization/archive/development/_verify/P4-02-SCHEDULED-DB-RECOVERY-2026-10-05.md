# P4-02 Scheduled Database Recovery Regression — 2026-10-05

## Scope

Add a test-only TG4-02 regression in `arcana-core/src/recipes/scheduler.rs`: an actual `RecipeScheduler::start_worker` scheduled loop must survive a transient `WorkersDb` read failure, then run the persisted recipe on a later tick after the fixture database becomes readable again.

## Progress

- [x] Created this report before investigation.
- [x] Read MDP §0 and TG4-02.
- [x] Drafted observable assertions from the spec before inspecting implementation bodies.
- [x] Inspected the public scheduler/test seams and the relevant test precedent.
- [x] Added the focused test using only the disposable worker schema.
- [x] Main recorded execution, fixture corrections, and limits.

## Evidence and Findings

### Assertion draft

The test must call production `RecipeScheduler::start_worker`, then cause a transient `WorkersDb` read failure during an actual scheduled tick. It must prove the schedule remains alive for that same worker after the failure, make the fixture database readable again, advance to a later scheduled tick, and observe the persisted worker recipe execute through the production runner (for example, by consuming a persisted inbox message with the existing scripted provider). Assertions must distinguish the later scheduled run from the initial startup tick and from helper-only retry/disposition tests. The failure must be confined to a disposable profile and restoration must happen even if an assertion fails.

These are the TG4-02 contract assertions. The feasibility and exact safe schema seam remain to be established from source.

### Source findings

- `RecipeScheduler::start_worker` resolves the persisted worker/recipe synchronously, sets the profile idle, then spawns an immediate startup tick followed by the fixed five-minute scheduled loop (`arcana-core/src/recipes/scheduler.rs`). This permits creating a disposable profile first, then taking only its `workers` table unavailable after `start_worker` returns.
- The scheduled loop now maps `WorkersDb::open().and_then(get_by_slug)` errors to `StatusReadDisposition::Retry` and continues its existing cron loop. The current `transient_status_read_error_keeps_schedule_for_retry` test exercises only the disposition helper, while `production_startup_idle_workers_skip_provider_and_record_gate` establishes the paused-current-thread-runtime and repeated-yield pattern. TG4-02 explicitly calls for the real loop and a later recipe run.
- `WorkersDb::open` targets `~/.arcana/workers.db`; the fixture can seed the real profile in a tempfile-backed home, drop its handles, temporarily rename its `workers` table, then restore that same table. The local `ensure_columns` helper treats an absent table as no-op, after which the worker migration/seed path fails against the temporarily absent table; this should produce a real `WorkersDb::open` error without touching a user profile.
- `qc-agent` has a persisted `qc-dispatcher` recipe and existing `QcInboxProvider` can script persisted inbox read/ack, queue read, and final turn. A message inserted after the no-work startup tick makes the later recovered scheduled tick observably run the saved recipe. The scheduler exposes its owned handle map inside this test module, and `tokio::task::JoinHandle::id()` is available in the locked Tokio 1.52.2 source to compare handle identity across the failed tick.
- `RecipeScheduler::shutdown().await` aborts and joins scheduled and kicked tasks, providing fixture cleanup. The focused test can use a current-thread runtime with time paused to advance the immediate first cron interval, restore the disposable table, then advance to a second scheduled interval.

The production path appears testable without a production injection seam. No ambiguity requiring owner direction is identified so far.

## Main-session takeover

The GPT-6 Luna High agent hit its usage limit after adding the test; no completed agent execution/review is claimed. Main owns the source from this point. Main added capture of the actual scheduled failure warning, same-handle identity checks and a wall-bounded completion wait instead of assuming 64 yields finish asynchronous filesystem work. Windows validation is running. No production injection seam or runtime change is introduced.

The first Windows execution reached the 30-second fixture completion deadline because the second paused-clock advance was only 310 seconds. The production cron iterator computes subsequent boundaries from wall-clock UTC (unchanged by Tokio pause), so its second sleep can be up to 600 seconds. The fixture now advances 610 seconds for that second boundary and uses RAII restoration of its disposable worker table even on assertion panic. This corrects simulation/cleanup, not production cadence; rerun required.

## Final production-path proof

Exact filter: `scheduled_loop_retries_transient_workers_db_read_and_runs_saved_qc_recipe`.

- Windows: 1/1 pass, 1.32s (`target/p4-scheduled-db-recovery-windows.txt`).
- Ubuntu WSL: 1/1 pass, 1.85s (`target/p4-scheduled-db-recovery-linux.txt`).
- Startup is observed idle with zero provider requests before adding inbox work. A real fixture WorkersDb open/read failure is observed at the actual scheduled tick. The captured retry warning and unchanged JoinHandle ID/non-finished state prove the existing loop survives rather than being re-created.
- Restoring the same table preserves the QC row ID. A later scheduled boundary dispatches the saved qc-dispatcher recipe through the existing scripted provider seam; its four real engine turns read, acknowledge, inspect the queue and finish. The TasksDb reader verifies the message is acknowledged and the original scheduler handle remains alive. Explicit shutdown joins/removes schedules.
- TG4-02 is checked. No production fault-injection seam, scheduling policy or database migration was added. This is controlled-provider production-loop proof; broader interrupted-task continuity and live CLI Fleet remain separate.
