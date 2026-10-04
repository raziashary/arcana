// RecipeScheduler — manages cron-based recipe execution for the Worker Fleet.
// Each worker with a bound recipe can be started/stopped independently.
// Uses tokio tasks + the `cron` crate for accurate next-fire-time calculation.
// See: DEVELOPMENT_PLAN.md Phase 2 — Cron Scheduler

use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::Utc;
use cron::Schedule;
use futures::FutureExt;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
// Phase 7.7 — used for metadata JSON in record_system_event calls
use serde_json;

use super::runner::RecipeRunner;
use crate::db::workers::{Worker, WorkerStatus, WorkersDb};
use crate::errors::{ArcanaError, Result};
use crate::recipes::parser::load_recipe_by_id;
use crate::state::lock::LockTolerant;

// ─── RecipeScheduler ──────────────────────────────────────────────────────────

/// Manages background cron tasks for all active workers.
/// Stored as Arc<RecipeScheduler> in Tauri managed state — lives for app lifetime.
pub struct RecipeScheduler {
    runner: Arc<RecipeRunner>,
    handles: Mutex<HashMap<String, JoinHandle<()>>>, // worker_slug → running task (tauri async runtime)
    kick_handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
}

impl RecipeScheduler {
    pub fn new(runner: Arc<RecipeRunner>) -> Self {
        Self {
            runner,
            handles: Mutex::new(HashMap::new()),
            kick_handles: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start cron scheduling for a worker.
    /// Idempotent — restarts if already running.
    /// Loads worker + bound recipe from DB, then spawns a tokio task that fires on schedule.
    pub fn start_worker(&self, worker_slug: &str) -> Result<()> {
        // Abort any existing task for this worker (idempotent restart)
        self.abort_worker_task(worker_slug);

        // Load the DB-owned worker profile and recipe-owned prompt through the
        // same resolver covered by P4-30's file-backed production-consumer test.
        let (worker, recipe_id, recipe) = worker_recipe_for_start(worker_slug)?;

        // P4-04 / §14.5: recipe assignment events are the dispatch mechanism.
        // This loop is only the fixed five-minute recovery sweep; per-recipe
        // cron cadence must not cause idle model runs or replace event dispatch.
        let cron_expr = normalize_to_6field(RECONCILIATION_CRON);

        let schedule = Schedule::from_str(&cron_expr).map_err(|e| {
            ArcanaError::Agent(format!(
                "Invalid cron expression '{cron_expr}' (recipe '{recipe_id}'): {e}"
            ))
        })?;

        info!(
            "[RecipeScheduler] Starting worker '{}' on schedule '{}' (recipe: '{}')",
            worker_slug, cron_expr, recipe_id
        );

        // Set worker to 'idle' (active, ready to fire)
        WorkersDb::open()?.update_status(worker.id, WorkerStatus::Idle)?;
        // Phase 3.0 R2 — push status to UI so the Agent Roster turns this
        // row "alive" the moment the user hits Start.
        crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
            slug: worker_slug.to_string(),
            status: "idle".to_string(),
            previous_status: None,
            reason: None,
            cooldown_seconds: None,
            tick_duration_ms: None,
            last_outcome: None,
            timestamp: crate::db::timestamps::now_rfc3339(),
        });

        let runner = self.runner.clone();
        let slug_owned = worker_slug.to_string();

        let handle = tokio::spawn(async move {
            info!(
                "[RecipeScheduler] Schedule loop started for worker '{}'",
                slug_owned
            );

            // ⚠ A 120-second start-up wait stood here. It polled every 2s for
            // up to two minutes so the first tick could reach a second runtime
            // that no longer runs — so every worker sat idle for two minutes at
            // boot and then did exactly what it would have done immediately.
            // Removed while fixing the P0 post-phase audit's F-11.

            // ── IMMEDIATE STARTUP TICK (Fleet Initialization) ─────────────────────
            // Fire one tick immediately after orchestrator is ready, regardless of
            // the cron schedule. This ensures all 7 fleet agents create their YAMLs
            // within seconds of boot, even if their normal schedule is 5 or 10 minutes.
            // Without this, worker-platform and worker-cleanup would wait 5-10 min
            // before first registration, breaking first-time user experience.
            info!(
                "[RecipeScheduler] Firing STARTUP tick for '{}' on the in-process engine",
                slug_owned
            );

            // Ruflo Feature 2 — run orphan detection before the Supervisor tick
            // so it sees freshly-released tasks on this poll.
            if slug_owned == "supervisor" {
                reconcile_orphans("startup");
            }

            match run_if_recoverable_work(
                &slug_owned,
                || worker_has_recoverable_work(&slug_owned),
                run_worker_recipe(&runner, &recipe, &slug_owned),
            )
            .await
            {
                Ok(true) => {}
                Ok(false) => info!(
                    "[RecipeScheduler] Startup reconciliation found no work or another run for '{}'",
                    slug_owned
                ),
                Err(e) => error!(
                    kind = crate::observability::error_kinds::KIND_INVOKE_FAILED,
                    worker_slug = slug_owned.as_str(),
                    "[RecipeScheduler] Startup tick failed for '{}': {e}",
                    slug_owned
                ),
            }

            // ── A REMOVED CATCH-UP TICK ─────────────────────────────
            // ⚠ A "catch-up tick" stood here and was removed in `P0`. It
            // waited up to 90s and then ran the same recipe a SECOND time, so a
            // worker whose first tick had taken the fallback path would still
            // get set up by the other one. With only one path left, the second
            // call was simply a duplicate: another paid LLM invocation of the
            // same recipe, minutes after the first, for a reason that could no
            // longer occur. Found by the P0 post-phase audit (F-11).

            // ── CRON SCHEDULE LOOP ───────────────────────────────────────────────
            // After startup tick, fall back to the worker's configured schedule.
            //
            // Phase 7.2 — minimum tick floor.
            // When a long-running recipe (a 24-second tick, say) causes the
            // next cron time to already be in the past, (next_fire - now) is
            // negative → .to_std() returns Err → .unwrap_or_default() returned
            // Duration::ZERO (0 ns), creating a tight spin loop.  Evidence: 32
            // "next fire in 0ns" occurrences in the 2026-04-29 run; worst burst:
            // 6 fires in 99 ms for qc-agent (log lines 47862–47914).
            // Fix: enforce a minimum wait of MIN_TICK_SECS regardless of cron slip.
            const MIN_TICK_SECS: u64 = 10;

            for next_fire in schedule.upcoming(Utc) {
                let now = Utc::now();
                let raw_wait = (next_fire - now).to_std().unwrap_or(Duration::ZERO);
                let wait_dur = raw_wait.max(Duration::from_secs(MIN_TICK_SECS));

                if raw_wait < Duration::from_secs(MIN_TICK_SECS) {
                    // Doc 08 P1.3 — demote sub-5s cron slips to DEBUG. Slips
                    // below 5s are routine "the previous tick finished right
                    // at the next cron boundary" events (L364, L405, L582,
                    // L728, L756, L763, L789 in the 14 May log) — the floor
                    // guard absorbs them silently. WARN-level is reserved
                    // for slips ≥ 5s where the worker is genuinely behind
                    // and the user might want to investigate.
                    if raw_wait >= Duration::from_secs(5) {
                        warn!(
                            "[RecipeScheduler] Worker '{}' — cron slipped ({:?}), enforcing {}s floor",
                            slug_owned, raw_wait, MIN_TICK_SECS
                        );
                    } else {
                        debug!(
                            "[RecipeScheduler] Worker '{}' — cron slipped ({:?}), enforcing {}s floor",
                            slug_owned, raw_wait, MIN_TICK_SECS
                        );
                    }
                } else {
                    info!(
                        "[RecipeScheduler] Worker '{}' — next fire in {:?}",
                        slug_owned, wait_dur
                    );
                }
                tokio::time::sleep(wait_dur).await;

                // The Supervisor's recovery sweep also releases stale claims.
                // The resulting ready tasks are then visible to its work check.
                if slug_owned == "supervisor" {
                    reconcile_orphans("periodic");
                }

                // Check that worker is still active before firing
                let status_read = WorkersDb::open()
                    .and_then(|db| db.get_by_slug(&slug_owned))
                    .map(|worker| worker.map(|worker| worker.status));

                match disposition_for_status_read(status_read) {
                    StatusReadDisposition::Stop => {
                        info!(
                            "[RecipeScheduler] Worker '{}' no longer exists — stopping schedule",
                            slug_owned
                        );
                        break;
                    }
                    StatusReadDisposition::Retry => {
                        // A transient storage error must not be interpreted as a
                        // deleted worker. Keep the schedule alive and retry at the
                        // next cron boundary.
                        warn!(
                            "[RecipeScheduler] Worker '{}' status read failed; retaining schedule for next tick",
                            slug_owned
                        );
                        continue;
                    }
                    StatusReadDisposition::Run(current_status) => match current_status {
                        // 🔴 `idle_no_provider` was NOT in this arm, and it is a
                        // status the runner writes. It fell to the `_` arm, which
                        // warns "unexpected status" and fires nothing — so a worker
                        // that ticked once before a provider key was configured was
                        // **never scheduled again, even after the user added the
                        // key**. Nothing resets the status but a tick, and no tick
                        // could happen; the only escape was to stop and start the
                        // worker, which is not a thing anyone would know to try.
                        //
                        // Firing it is safe: with no provider the runner writes the
                        // same status again and returns, and the cron interval
                        // bounds the retry.
                        WorkerStatus::Idle
                        | WorkerStatus::Active
                        | WorkerStatus::Error
                        | WorkerStatus::IdleNoProvider => {
                            match worker_has_recoverable_work(&slug_owned) {
                                Ok(true) => {}
                                Ok(false) => continue,
                                Err(e) => {
                                    warn!(
                                        "[RecipeScheduler] Recovery work check failed for '{}'; retaining schedule: {e}",
                                        slug_owned
                                    );
                                    continue;
                                }
                            }

                            // Fire the recipe
                            info!(
                                "[RecipeScheduler] Firing recipe '{}' for worker '{}'",
                                recipe_id, slug_owned
                            );
                            let Some(_run_guard) = try_begin_worker_run(&slug_owned) else {
                                debug!(
                                    "[RecipeScheduler] Cron tick for '{}' skipped (another run is in flight)",
                                    slug_owned
                                );
                                continue;
                            };
                            if let Err(e) = run_worker_recipe(&runner, &recipe, &slug_owned).await {
                                error!(
                                    "[RecipeScheduler] Recipe run failed for '{}': {e}",
                                    slug_owned
                                );
                                // Phase 7.7 — record the failure in the system_events table so
                                // the System Logs panel in the Dashboard shows live error history.
                                let _ = crate::analytics::global().collector().record_system_event(
                                    "scheduler".to_string(),
                                    "recipe_run_failed".to_string(),
                                    format!("Worker '{}' recipe run failed: {e}", slug_owned),
                                    "error".to_string(),
                                    Some(
                                        serde_json::json!({
                                            "worker": slug_owned,
                                            "recipe": recipe_id,
                                        })
                                        .to_string(),
                                    ),
                                );
                                // Phase 7.2 — error backoff: pause 60 s before the next cron
                                // tick so a fast-failing recipe (e.g. Anthropic 404) cannot
                                // tight-loop at ~2 invocations/second, flooding the log and
                                // consuming CPU/network bandwidth.
                                warn!(
                                    "[RecipeScheduler] Worker '{}' backing off 60s after run error",
                                    slug_owned
                                );
                                tokio::time::sleep(Duration::from_secs(60)).await;
                            }
                        }
                        WorkerStatus::Working => {
                            if kick_inflight().lock_tolerant().contains(&slug_owned) {
                                warn!(
                                    "[RecipeScheduler] Worker '{}' still has a guarded run in flight — skipping tick",
                                    slug_owned
                                );
                            } else if let Err(e) =
                                recover_interrupted_worker_run(&slug_owned, false)
                            {
                                warn!(
                                    "[RecipeScheduler] Worker '{}' has stale working state; recovery will retry on the next pass: {e}",
                                    slug_owned
                                );
                            } else {
                                info!(
                                    "[RecipeScheduler] Recovered stale working state for '{}'",
                                    slug_owned
                                );
                            }
                        }
                        WorkerStatus::Paused | WorkerStatus::Deleted => {
                            info!(
                            "[RecipeScheduler] Worker '{}' is paused/deleted — stopping schedule",
                            slug_owned
                        );
                            break;
                        }
                        WorkerStatus::Created => {
                            // A seeded worker with no start transition is not runnable.
                            warn!(
                                "[RecipeScheduler] Worker '{}' is not started yet — skipping tick",
                                slug_owned
                            );
                        }
                    },
                }
            }

            info!(
                "[RecipeScheduler] Schedule loop ended for worker '{}'",
                slug_owned
            );
        });

        self.handles
            .lock()
            .unwrap()
            .insert(worker_slug.to_string(), handle);
        Ok(())
    }

    /// Stop the scheduled task for a worker and set its status to 'paused'.
    pub fn stop_worker(&self, worker_slug: &str) -> Result<()> {
        self.abort_worker_task(worker_slug);

        // Update status in DB
        if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(worker_slug) {
                let _ = db.update_status(worker.id, WorkerStatus::Paused);
            }
        }
        // Phase 3.0 R2 — push status so the Agent Roster dims this row
        // immediately without waiting for the next `list_workers` poll.
        crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
            slug: worker_slug.to_string(),
            status: "paused".to_string(),
            previous_status: None,
            reason: None,
            cooldown_seconds: None,
            tick_duration_ms: None,
            last_outcome: None,
            timestamp: crate::db::timestamps::now_rfc3339(),
        });

        info!("[RecipeScheduler] Stopped worker '{}'", worker_slug);
        Ok(())
    }

    /// Stop all running worker schedules (called on app shutdown).
    pub fn stop_all(&self) {
        let mut handles = self.handles.lock_tolerant();
        for (slug, handle) in handles.drain() {
            handle.abort();
            info!("[RecipeScheduler] Aborted schedule for worker '{}'", slug);
        }
    }

    /// Abort and join both persistent schedule loops and one-off event kicks.
    /// Headless hosts use this before dropping their Tokio runtime so no
    /// scheduler work is silently detached from the command's shutdown path.
    pub async fn shutdown(&self) {
        let scheduled = {
            let mut handles = self.handles.lock_tolerant();
            handles
                .drain()
                .map(|(_, handle)| handle)
                .collect::<Vec<_>>()
        };
        for handle in &scheduled {
            handle.abort();
        }
        for handle in scheduled {
            let _ = handle.await;
        }

        let kicks = {
            let mut handles = self.kick_handles.lock_tolerant();
            handles
                .drain()
                .map(|(_, handle)| handle)
                .collect::<Vec<_>>()
        };
        for handle in &kicks {
            handle.abort();
        }
        for handle in kicks {
            let _ = handle.await;
        }
    }

    /// Returns the slugs of currently-scheduled workers.
    pub fn active_workers(&self) -> Vec<String> {
        self.handles.lock_tolerant().keys().cloned().collect()
    }

    /// Check if a specific worker has an active schedule.
    pub fn is_running(&self, worker_slug: &str) -> bool {
        self.handles.lock_tolerant().contains_key(worker_slug)
    }

    // ── Push-based kick (Phase 3.0 R1) ────────────────────────────────────────

    /// Fire an immediate one-off recipe tick for `worker_slug`, bypassing the
    /// cron cadence. Called by the task-board tools when a state transition
    /// means the worker has work to do *right now* (task created →
    /// supervisor; task assigned → worker; qc_review → qc-agent).
    ///
    /// Semantics:
    ///   - No-op if the worker isn't registered, has no bound recipe, or is
    ///     currently `working` (a tick is already in flight). This prevents
    ///     duplicate parallel runs from piling onto a single LLM session.
    ///   - Parallel to — not a replacement for — the cron loop. The cron loop
    ///     stays exactly as it was for idle/recovery polling.
    ///   - Never blocks the caller: the tick runs on the Tauri async runtime.
    pub fn kick_worker(&self, worker_slug: &str) {
        match worker_has_recoverable_work(worker_slug) {
            Ok(true) => {}
            Ok(false) => {
                debug!(
                    "[RecipeScheduler] kick_worker('{}'): skipped (no task or unacknowledged message)",
                    worker_slug
                );
                return;
            }
            Err(e) => {
                warn!(
                    "[RecipeScheduler] kick_worker('{}'): work check failed closed: {e}",
                    worker_slug
                );
                return;
            }
        }

        // Bug F — close the TOCTOU window between the "already working" check
        // and the spawn. A separate process-wide `kick_inflight` HashSet acts
        // as the actual ordering primitive: whichever kick inserts the slug
        // first wins, every other kick observes the slug already present and
        // bails immediately. The spawned task removes itself from the set
        // when it completes, freeing the slot for the next kick.
        //
        // The DB-level "status == working" check is still useful as a
        // belt-and-braces signal for the case where a long-running cron tick
        // is in flight, but it is no longer the ordering primitive.
        let slug_for_inflight = worker_slug.to_string();
        {
            let mut inflight = kick_inflight().lock_tolerant();
            if !inflight.insert(slug_for_inflight.clone()) {
                info!(
                    "[RecipeScheduler] kick_worker('{}'): skipped (another \
                     kick is already in flight)",
                    worker_slug
                );
                return;
            }
        }

        // From here on we MUST remove ourselves from `kick_inflight` on every
        // exit path — otherwise the worker is permanently locked out. Use a
        // small RAII guard for the synchronous early returns; the success
        // path hands ownership to the spawned task which clears the slot on
        // completion.
        let mut guard = KickSlotGuard::new(slug_for_inflight.clone());

        // DB gate: skip if worker is already in flight per the DB status.
        let (worker_opt, recipe_id_opt) = match WorkersDb::open() {
            Ok(db) => match db.get_by_slug(worker_slug) {
                Ok(Some(w)) => {
                    let worker = if w.status == WorkerStatus::Working {
                        if let Err(e) = recover_interrupted_worker_run(worker_slug, false) {
                            warn!(
                                "[RecipeScheduler] kick_worker('{}'): stale run recovery failed: {e}",
                                worker_slug
                            );
                            return;
                        }
                        match db.get_by_slug(worker_slug) {
                            Ok(Some(worker)) => worker,
                            Ok(None) => {
                                warn!("[RecipeScheduler] kick_worker('{}'): worker disappeared during recovery", worker_slug);
                                return;
                            }
                            Err(e) => {
                                warn!("[RecipeScheduler] kick_worker('{}'): re-read after recovery failed: {e}", worker_slug);
                                return;
                            }
                        }
                    } else {
                        w
                    };
                    let rid = worker.recipe_id.clone();
                    (Some(worker), rid)
                }
                Ok(None) => {
                    warn!(
                        "[RecipeScheduler] kick_worker('{}'): worker not found",
                        worker_slug
                    );
                    return;
                }
                Err(e) => {
                    warn!(
                        "[RecipeScheduler] kick_worker('{}'): DB error: {e}",
                        worker_slug
                    );
                    return;
                }
            },
            Err(e) => {
                warn!(
                    "[RecipeScheduler] kick_worker('{}'): open workers.db failed: {e}",
                    worker_slug
                );
                return;
            }
        };
        let _ = worker_opt;

        let recipe_id = match recipe_id_opt {
            Some(id) => id,
            None => {
                warn!(
                    "[RecipeScheduler] kick_worker('{}'): no bound recipe",
                    worker_slug
                );
                return;
            }
        };

        let recipe = match load_recipe_by_id(&recipe_id) {
            Ok(Some(r)) => r,
            Ok(None) => {
                warn!(
                    "[RecipeScheduler] kick_worker('{}'): recipe '{}' not on disk",
                    worker_slug, recipe_id
                );
                return;
            }
            Err(e) => {
                warn!(
                    "[RecipeScheduler] kick_worker('{}'): load recipe failed: {e}",
                    worker_slug
                );
                return;
            }
        };

        let runner = self.runner.clone();
        let slug = worker_slug.to_string();
        let handle = tokio::spawn(async move {
            // This guard belongs to the spawned future, so Rust drops it on
            // normal return, panic, abort, or timeout cancellation.
            let mut run_guard = KickSlotGuard::for_worker_run(slug.clone());
            info!(
                "[RecipeScheduler] kick_worker: firing '{}' immediately (push-based dispatch)",
                slug
            );
            match tokio::time::timeout(PUSH_RUN_TIMEOUT, runner.run_recipe(&recipe, &slug)).await {
                Ok(Ok(())) => run_guard.disarm(),
                Ok(Err(e)) => {
                    error!(
                        "[RecipeScheduler] kick_worker: recipe run failed for '{}': {e}",
                        slug
                    );
                }
                Err(_) => {
                    error!(
                        "[RecipeScheduler] kick_worker: '{}' exceeded the two-hour hard timeout; cancelling run and quarantining its claim for reconciliation",
                        slug
                    );
                }
            }
        });
        self.kick_handles
            .lock_tolerant()
            .insert(worker_slug.to_string(), handle);
        // The spawned task now owns the responsibility for clearing the
        // kick_inflight entry; mark the guard committed so its Drop doesn't
        // double-remove.
        guard.disarm();
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn abort_worker_task(&self, worker_slug: &str) {
        if let Some(handle) = self.handles.lock_tolerant().remove(worker_slug) {
            handle.abort();
            info!(
                "[RecipeScheduler] Aborted previous task for worker '{}'",
                worker_slug
            );
        }
    }
}

fn reconcile_orphans(phase: &'static str) {
    let mut operation =
        crate::observability::timing::OperationTimer::start("orphan_reconciliation")
            .with_worker("supervisor");
    match crate::orphan::reconcile() {
        Ok(released) => {
            operation = operation.with_result_count(released.len());
            operation.succeed();
            if !released.is_empty() {
                info!(
                    operation = "orphan_reconciliation",
                    phase,
                    released_count = released.len(),
                    "[OrphanDetector] Reconciliation released orphaned task claims"
                );
                crate::events::emit_orphans_released(released);
            }
        }
        Err(_) => {
            operation.set_outcome("error");
            error!(
                kind = crate::observability::error_kinds::KIND_INVOKE_FAILED,
                operation = "orphan_reconciliation",
                phase,
                error_class = "task_store_or_reconciliation",
                "[OrphanDetector] Reconciliation failed; no empty-scan result was inferred"
            );
        }
    }
}

fn worker_recipe_for_start(
    worker_slug: &str,
) -> Result<(Worker, String, crate::recipes::types::Recipe)> {
    let db = WorkersDb::open()?;
    let worker = db
        .get_by_slug(worker_slug)?
        .ok_or_else(|| ArcanaError::Agent(format!("Worker '{worker_slug}' not found")))?;
    let recipe_id = worker
        .recipe_id
        .as_deref()
        .ok_or_else(|| {
            ArcanaError::Agent(format!(
                "Worker '{worker_slug}' has no recipe bound — bind a recipe first"
            ))
        })?
        .to_string();
    let recipe = load_recipe_by_id(&recipe_id)?
        .ok_or_else(|| ArcanaError::Agent(format!("Recipe '{recipe_id}' not found on disk")))?;
    Ok((worker, recipe_id, recipe))
}

// ─── Orchestrator readiness gate ────────────────────────────────────────────

// ─── Global scheduler handle (Phase 3.0 R1) ──────────────────────────────────
//
// The task-board tools (`create_task`, `assign_task`, `update_task_status`)
// run inside the agent's tool-execution loop and do not have access to Tauri
// `State<>`. This OnceLock gives them a cheap, lock-free handle to the
// scheduler so they can `kick()` the relevant worker the instant the board
// state changes, collapsing the previous scheduler-only dead window.
//
// `install_global` is called exactly once from main.rs right after the
// RecipeScheduler is constructed. `kick(slug)` is a safe no-op if called
// before installation (tests, early boot).

static GLOBAL_SCHEDULER: OnceLock<Arc<RecipeScheduler>> = OnceLock::new();

// Bug F — process-wide kick-inflight set. Lives as long as the process; the
// set is cheap (one entry per active kick, removed on completion).
fn kick_inflight() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(|| Mutex::new(HashSet::new()))
}

const PUSH_RUN_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const RECONCILIATION_CRON: &str = "*/5 * * * *";

fn try_begin_worker_run(worker_slug: &str) -> Option<KickSlotGuard> {
    let slug = worker_slug.to_string();
    if !kick_inflight().lock_tolerant().insert(slug.clone()) {
        return None;
    }
    Some(KickSlotGuard::for_worker_run(slug))
}

pub(crate) async fn run_if_recoverable_work<F>(
    worker_slug: &str,
    has_work: impl FnOnce() -> Result<bool>,
    run: F,
) -> Result<bool>
where
    F: std::future::Future<Output = Result<()>>,
{
    run_if_recoverable_work_with_timeout(worker_slug, PUSH_RUN_TIMEOUT, has_work, run).await
}

async fn run_if_recoverable_work_with_timeout<F>(
    worker_slug: &str,
    timeout: Duration,
    has_work: impl FnOnce() -> Result<bool>,
    run: F,
) -> Result<bool>
where
    F: std::future::Future<Output = Result<()>>,
{
    let operation =
        crate::observability::timing::OperationTimer::start("scheduler_worker_dispatch_gate")
            .with_worker(worker_slug);
    if !has_work()? {
        operation.with_result_count(0).set_outcome("idle");
        return Ok(false);
    }
    let Some(_run_guard) = try_begin_worker_run(worker_slug) else {
        operation
            .with_result_count(0)
            .set_outcome("already_running");
        return Ok(false);
    };
    run_worker_future(worker_slug, timeout, run).await?;
    operation.with_result_count(1).set_outcome("dispatched");
    Ok(true)
}

async fn run_worker_recipe(
    runner: &RecipeRunner,
    recipe: &crate::recipes::types::Recipe,
    worker_slug: &str,
) -> Result<()> {
    run_worker_future(
        worker_slug,
        PUSH_RUN_TIMEOUT,
        runner.run_recipe(recipe, worker_slug),
    )
    .await
}

async fn run_worker_future<F>(worker_slug: &str, timeout: Duration, run: F) -> Result<()>
where
    F: std::future::Future<Output = Result<()>>,
{
    match AssertUnwindSafe(tokio::time::timeout(timeout, run))
        .catch_unwind()
        .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(ArcanaError::Agent(format!(
            "worker '{worker_slug}' exceeded the two-hour run timeout; the run was cancelled and its task claim will be held for side-effect reconciliation"
        ))),
        Err(_) => Err(ArcanaError::Agent(format!(
            "worker '{worker_slug}' panicked; its interrupted task claim will be held for side-effect reconciliation"
        ))),
    }
}

struct KickSlotGuard {
    slug: String,
    recovery: Option<Box<dyn FnOnce(&str) -> Result<Vec<i64>> + Send>>,
    active: bool,
}

impl KickSlotGuard {
    fn new(slug: String) -> Self {
        Self {
            slug,
            recovery: None,
            active: true,
        }
    }

    fn for_worker_run(slug: String) -> Self {
        Self::with_recovery(slug, |worker_slug| {
            recover_interrupted_worker_run(worker_slug, true)
        })
    }

    fn with_recovery(
        slug: String,
        recovery: impl FnOnce(&str) -> Result<Vec<i64>> + Send + 'static,
    ) -> Self {
        Self {
            slug,
            recovery: Some(Box::new(recovery)),
            active: true,
        }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for KickSlotGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Some(recovery) = self.recovery.take() {
            match recovery(&self.slug) {
                Ok(_) => {}
                Err(e) => {
                    warn!(
                        "[RecipeScheduler] Could not release interrupted task claims for '{}': {e}",
                        self.slug
                    );
                }
            }
        }
        kick_inflight().lock_tolerant().remove(&self.slug);
    }
}

fn recover_interrupted_worker_run(
    worker_slug: &str,
    interrupted_by_current_run_guard: bool,
) -> Result<Vec<i64>> {
    let runs = crate::db::runs::RunsDb::open()?;
    runs.reap_abandoned()?;
    let owner = runs.latest_worker_run_owner(worker_slug)?.ok_or_else(|| {
        ArcanaError::Agent(format!(
            "worker '{worker_slug}' has no durable run owner; refusing to release task claims"
        ))
    })?;
    let current_host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    let current_pid = std::process::id() as i64;
    if owner.host != current_host {
        return Err(ArcanaError::Agent(format!(
            "worker '{worker_slug}' run belongs to host '{}'; this host cannot prove it is stopped",
            owner.host
        )));
    }
    if interrupted_by_current_run_guard {
        if owner.pid != current_pid
            || owner.phase == "completed"
            || owner.phase == "blocked"
            || owner.outcome.as_deref() == Some(crate::db::runs::outcome::OK)
        {
            return Err(ArcanaError::Agent(format!(
                "worker '{worker_slug}' recovery guard does not own the latest run"
            )));
        }
    } else {
        let pid = u32::try_from(owner.pid)
            .map_err(|_| ArcanaError::Agent("worker run has an invalid process id".into()))?;
        if crate::orphan::pid_checker::is_pid_alive(pid) {
            return Err(ArcanaError::Agent(format!(
                "worker '{worker_slug}' still has a live process ({pid}); refusing duplicate recovery"
            )));
        }
        if owner.outcome.is_none() {
            return Err(ArcanaError::Agent(format!(
                "worker '{worker_slug}' run has no completed process-death reconciliation"
            )));
        }
    }
    // Keep the persisted worker in `working` until its task claims are actually
    // moved to needs_info. The cron reconciliation pass retries stale
    // `working` workers; resetting status first would hide an unreconciled claim
    // after a transient tasks.db failure. Quarantine prevents automatic replay
    // until the Supervisor/PA reconciles effects from the interrupted run.
    let tasks = crate::db::tasks::TasksDb::open()?;
    let held = tasks.hold_interrupted_claims_for_worker_run(
        worker_slug,
        owner.pid,
        &owner.host,
        "interrupted run requires side-effect reconciliation before retry",
    )?;
    runs.interrupt_worker_ticks(worker_slug, "scheduler_reconciled_interruption")?;
    reset_interrupted_worker_status(worker_slug)?;
    for task_id in &held {
        crate::events::emit_task_changed(crate::events::TaskChangedPayload {
            task_id: Some(*task_id),
            kind: "recovery_required".into(),
            status: Some("needs_info".into()),
            assigned_worker: Some(worker_slug.to_string()),
        });
    }
    if !held.is_empty() && worker_slug != "supervisor" {
        kick("supervisor");
    }
    Ok(held)
}

fn reset_interrupted_worker_status(worker_slug: &str) -> Result<()> {
    let db = WorkersDb::open()?;
    let Some(worker) = db.get_by_slug(worker_slug)? else {
        return Ok(());
    };
    if worker.status != WorkerStatus::Working {
        return Ok(());
    }
    db.update_status(worker.id, WorkerStatus::Idle)?;
    crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
        slug: worker_slug.to_string(),
        status: "idle".into(),
        previous_status: Some("working".into()),
        reason: Some("interrupted run recovered".into()),
        cooldown_seconds: None,
        tick_duration_ms: None,
        last_outcome: Some("recovered".into()),
        timestamp: crate::db::timestamps::now_rfc3339(),
    });
    Ok(())
}

/// Install the process-wide RecipeScheduler handle. Idempotent; subsequent
/// calls are silently ignored (OnceLock semantics).
pub fn install_global(scheduler: Arc<RecipeScheduler>) {
    let _ = GLOBAL_SCHEDULER.set(scheduler);
}

/// Fire an immediate tick for `worker_slug`. No-ops if the scheduler has not
/// been installed yet. This is the public API called from the task-board tools
/// — they never hold an `Arc<RecipeScheduler>` directly.
pub fn kick(worker_slug: &str) {
    if let Some(s) = GLOBAL_SCHEDULER.get() {
        s.kick_worker(worker_slug);
    } else {
        // Early-boot or test environment — tick will be handled by the next
        // cron fire once the scheduler is installed.
        warn!("[RecipeScheduler::kick] global scheduler not installed — '{}' will wait for next cron tick", worker_slug);
    }
}

/// Kick the Supervisor plus every domain worker so freshly-unblocked or
/// freshly-routed tasks dispatch on the next event-loop tick instead of
/// waiting up to one cron cycle. Used by the blocker-resolution hook
/// (Ruflo Feature 5) — when a task completes and unblocks dependent work,
/// we don't know in advance which worker will end up owning each unblocked
/// task, so we kick the whole fleet and let the Supervisor route.
pub fn kick_all_workers() {
    // Keep this in sync with the worker recipe ids in defaults.rs. Adding
    // a new worker only requires extending this list.
    for slug in &[
        "supervisor",
        "worker-backend",
        "worker-frontend",
        "worker-platform",
        "worker-mcp",
        "worker-cleanup",
        "qc-agent",
    ] {
        kick(slug);
    }
}

// ─── Cron expression helpers ──────────────────────────────────────────────────

/// Convert standard 5-field POSIX cron to 6-field (prepend seconds = 0).
/// "*/2 * * * *"   → "0 */2 * * * *"   (every 2 minutes)
/// "0 * * * *"     → "0 0 * * * *"     (every hour)
/// "0 */2 * * * *" → unchanged (already 6-field)
fn normalize_to_6field(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    match fields.len() {
        5 => format!("0 {}", expr), // prepend seconds = 0
        _ => expr.to_string(),      // 6 or 7 field — use as-is
    }
}

#[derive(Debug, PartialEq, Eq)]
enum StatusReadDisposition {
    Run(WorkerStatus),
    Stop,
    Retry,
}

fn disposition_for_status_read(result: Result<Option<WorkerStatus>>) -> StatusReadDisposition {
    match result {
        Ok(Some(status)) => StatusReadDisposition::Run(status),
        Ok(None) => StatusReadDisposition::Stop,
        Err(_) => StatusReadDisposition::Retry,
    }
}

/// The five-minute schedule is only a recovery path. Normal task events call
/// `kick()` immediately; this read keeps idle workers from making provider calls.
pub(crate) fn worker_has_recoverable_work(worker_slug: &str) -> Result<bool> {
    let mut operation =
        crate::observability::timing::OperationTimer::start("scheduler_recoverable_work_check")
            .with_database("tasks")
            .with_worker(worker_slug);
    let db = crate::db::tasks::TasksDb::open()?;
    let has_work = worker_has_recoverable_work_in(&db, worker_slug)?;
    operation = operation.with_result_count(usize::from(has_work));
    operation.set_outcome(if has_work { "work" } else { "idle" });
    Ok(has_work)
}

pub(crate) fn worker_has_recoverable_work_in(
    db: &crate::db::tasks::TasksDb,
    worker_slug: &str,
) -> Result<bool> {
    let has_requests =
        worker_slug == "supervisor" && !db.list_worker_requests(Some("pending"))?.is_empty();
    let has_tasks = match worker_slug {
        "supervisor" => {
            db.list(Some("ready"))?
                .iter()
                .any(|task| task.assigned_worker.is_none())
                || db.list(Some("needs_info"))?.iter().any(|task| {
                    task.rejection_reason
                        .as_deref()
                        .is_some_and(|reason| reason.starts_with("[worker recovery]"))
                })
        }
        "qc-agent" => !db.list(Some("qc_review"))?.is_empty(),
        _ => {
            !db.list_tasks_by_status_and_worker("assigned", worker_slug)?
                .is_empty()
                || !db
                    .list_tasks_by_status_and_worker("in_progress", worker_slug)?
                    .is_empty()
                || !db
                    .list_tasks_by_status_and_worker("rejected", worker_slug)?
                    .is_empty()
        }
    };
    let has_messages = db.count_unacknowledged_for(worker_slug)? > 0;
    Ok(has_tasks || has_messages || has_requests)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tasks::{CreateTaskInput, TasksDb};
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{
        CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus,
        StreamChunk,
    };
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::stream;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct QcInboxProvider {
        responses: Mutex<VecDeque<Vec<StreamChunk>>>,
        requests: Mutex<Vec<CompletionRequest>>,
        pause_after_script: Option<Arc<tokio::sync::Notify>>,
    }

    impl QcInboxProvider {
        fn new(message_ids: &[i64]) -> Self {
            let call = |id: &str, name: &str, input: serde_json::Value| {
                vec![
                    StreamChunk::ToolUseStart {
                        id: id.into(),
                        name: name.into(),
                        block_index: 0,
                        opaque: None,
                    },
                    StreamChunk::ToolInputDelta {
                        block_index: 0,
                        partial_json: input.to_string(),
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "tool_use".into(),
                        usage: Usage::default(),
                    },
                    StreamChunk::Done,
                ]
            };
            let mut responses =
                VecDeque::from([call("qc-read", "list_worker_messages", json!({}))]);
            for (index, message_id) in message_ids.iter().enumerate() {
                responses.push_back(call(
                    &format!("qc-ack-{index}"),
                    "acknowledge_worker_message",
                    json!({"id": message_id}),
                ));
            }
            responses.push_back(call(
                "qc-queue",
                "list_tasks",
                json!({"status": "qc_review"}),
            ));
            responses.push_back(vec![
                StreamChunk::TextDelta {
                    text: "Inbox and queue empty — idle.".into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]);
            Self {
                responses: Mutex::new(responses),
                requests: Mutex::new(Vec::new()),
                pause_after_script: None,
            }
        }

        fn request_count(&self) -> usize {
            self.requests.lock().expect("provider requests lock").len()
        }
    }

    #[async_trait]
    impl Provider for QcInboxProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "QC inbox fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            Ok(10)
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            unreachable!("RecipeRunner uses the provider streaming path")
        }

        async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
            self.requests
                .lock()
                .expect("provider requests lock")
                .push(request);
            let response = self
                .responses
                .lock()
                .expect("provider script lock")
                .pop_front();
            if response.is_none() {
                if let Some(paused) = &self.pause_after_script {
                    paused.notify_one();
                    return std::future::pending().await;
                }
            }
            let response = response.expect("one scripted response per AgentEngine turn");
            Ok(Box::pin(stream::iter(response.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    struct DependencyCompletionProvider {
        dependent_task_id: i64,
        supervisor_turns: AtomicUsize,
        worker_turns: AtomicUsize,
        requests: Mutex<Vec<CompletionRequest>>,
    }

    impl DependencyCompletionProvider {
        fn new(dependent_task_id: i64) -> Self {
            Self {
                dependent_task_id,
                supervisor_turns: AtomicUsize::new(0),
                worker_turns: AtomicUsize::new(0),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn tool_call(id: &str, name: &str, input: serde_json::Value) -> Vec<StreamChunk> {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }

        fn final_answer(text: &str) -> Vec<StreamChunk> {
            vec![
                StreamChunk::TextDelta {
                    text: text.into(),
                    block_index: 0,
                },
                StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage::default(),
                },
                StreamChunk::Done,
            ]
        }
    }

    #[async_trait]
    impl Provider for DependencyCompletionProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "P4-39 dependency completion fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            Ok(10)
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            unreachable!("RecipeRunner uses the provider streaming path")
        }

        async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
            self.requests
                .lock()
                .expect("dependency fixture request lock")
                .push(request.clone());

            let prompt = request
                .system
                .as_deref()
                .expect("saved recipe system prompt");
            let is_supervisor = prompt.contains("You are the Arcana Supervisor Agent.");
            assert!(
                is_supervisor || prompt.contains("You are the Arcana Backend Worker Agent."),
                "only the two selected saved recipes may dispatch"
            );
            let response = if is_supervisor {
                assert!(request.tools.iter().any(|tool| tool.name == "list_tasks"));
                match self.supervisor_turns.fetch_add(1, Ordering::SeqCst) {
                    0 => Self::tool_call(
                        "p4-39-list-ready",
                        "list_tasks",
                        serde_json::json!({"status": "ready", "compact": false}),
                    ),
                    1 => Self::tool_call(
                        "p4-39-route-dependent",
                        "assign_task",
                        serde_json::json!({"id": self.dependent_task_id}),
                    ),
                    2 => Self::final_answer("Routed the newly unblocked dependency."),
                    turn => panic!("unexpected Supervisor provider turn {turn}"),
                }
            } else {
                assert!(request.tools.iter().any(|tool| tool.name == "claim_task"));
                assert!(request
                    .tools
                    .iter()
                    .any(|tool| tool.name == "update_task_status"));
                match self.worker_turns.fetch_add(1, Ordering::SeqCst) {
                    0 => Self::tool_call(
                        "p4-39-claim-dependent",
                        "claim_task",
                        serde_json::json!({"task_id": self.dependent_task_id}),
                    ),
                    1 => Self::tool_call(
                        "p4-39-complete-dependent",
                        "update_task_status",
                        serde_json::json!({
                            "id": self.dependent_task_id,
                            "status": "done"
                        }),
                    ),
                    2 => Self::final_answer("Completed the claimed dependency."),
                    turn => panic!("unexpected worker provider turn {turn}"),
                }
            };

            Ok(Box::pin(stream::iter(response.into_iter().map(Ok))))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    #[tokio::test]
    async fn dependency_completion_wakes_persisted_supervisor_and_completes_saved_worker_recipe() {
        const CHILD_MARKER: &str = "ARCANA_P4_39_DEPENDENCY_COMPLETION_CHILD";

        if std::env::var_os(CHILD_MARKER).is_none() {
            let test_name = std::thread::current()
                .name()
                .expect("Rust test harness names the current test")
                .to_owned();
            let output = std::process::Command::new(
                std::env::current_exe().expect("resolve current Rust test executable"),
            )
            .arg("--exact")
            .arg(test_name)
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .output()
            .expect("run scheduler fixture in an isolated test process");
            assert!(
                output.status.success(),
                "isolated P4-39 test process failed\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        use crate::agent::builtin_tools::{
            claim_task_tool::ClaimTaskTool, update_task_status_tool::UpdateTaskStatusTool,
        };
        use crate::agent::permission::PermissionMode;
        use crate::agent::tool_trait::{Tool, ToolContext};
        use crate::db::tasks::{CreatePhasedTaskInput, CreateTaskInput, TasksDb};
        use serde_json::Value;

        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("initialize persisted worker recipes");

        let tasks = TasksDb::open().expect("open isolated task database");
        let blocker = tasks
            .create(CreateTaskInput {
                title: "P4-39 prerequisite completed through status tool".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "complete through the production worker path".into(),
                created_by: Some("p4-39-scheduler-test".into()),
            })
            .expect("create prerequisite")
            .id;
        let epic = tasks
            .create_epic("P4-39 autonomous dependency completion", None, "test")
            .expect("create dependency epic");
        let dependent = tasks
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "ordinary dependency".into(),
                phase_order: 1,
                title: "ordinary child auto-routed and completed".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "the saved worker recipe claims and completes this task"
                    .into(),
                blocked_by: vec![blocker.to_string()],
                milestone: None,
                created_by: Some("p4-39-scheduler-test".into()),
            })
            .expect("create ordinary dependent")
            .id;
        let budget_held = tasks
            .create_phased_task(CreatePhasedTaskInput {
                epic_id: epic.id.to_string(),
                phase: "budget-held dependency".into(),
                phase_order: 2,
                title: "budget hold requires explicit resolution".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "automatic dependency completion must leave this held".into(),
                blocked_by: vec![blocker.to_string()],
                milestone: None,
                created_by: Some("p4-39-scheduler-test".into()),
            })
            .expect("create budget-held dependent")
            .id;
        tasks
            .mark_task_budget_exceeded(budget_held, "saved task budget was reached")
            .expect("persist explicit budget hold");
        tasks
            .update_status(blocker, "assigned", Some("worker-backend"))
            .expect("assign prerequisite to backend worker");
        tasks
            .claim_task_with_pid(blocker, "worker-backend", std::process::id())
            .expect("claim prerequisite before its worker reports completion");

        let workers = WorkersDb::open().expect("open persisted worker roster");
        let supervisor = workers
            .get_by_slug("supervisor")
            .expect("read persisted Supervisor")
            .expect("default Supervisor is installed");
        assert!(supervisor.recipe_id.is_some());
        workers
            .update_status(supervisor.id, WorkerStatus::Idle)
            .expect("make Supervisor runnable");
        let backend = workers
            .get_by_slug("worker-backend")
            .expect("read backend worker")
            .expect("default backend worker is installed");
        workers
            .update_status(backend.id, WorkerStatus::Active)
            .expect("make routed worker runnable");
        assert!(backend.recipe_id.is_some());

        let workspace = tempfile::tempdir().expect("isolated worker workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated consent store")
            .grant(workspace.path(), "provider:google")
            .expect("grant only the scripted workspace destination");
        let provider = Arc::new(DependencyCompletionProvider::new(dependent));
        let runner = RecipeRunner::new_with_workspace_override(
            Arc::new(Mutex::new(crate::state::AppState {
                active_provider: Some("google".into()),
                active_model: Some("gemini-3.8-flash".into()),
                ..crate::state::AppState::default()
            })),
            Arc::new(
                crate::session::manager::SessionManager::new()
                    .expect("open isolated session manager"),
            ),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone());
        let scheduler = Arc::new(RecipeScheduler::new(Arc::new(runner)));
        install_global(scheduler.clone());

        let context = |task_id, agent_role: &str, call_id: &str| ToolContext {
            session_id: "p4-39-dependency-completion".into(),
            working_dir: workspace.path().to_string_lossy().into_owned(),
            call_id: call_id.into(),
            run_id: None,
            task_id,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: agent_role.into(),
            run_token_budget: None,
        };
        let completed = UpdateTaskStatusTool
            .execute(
                serde_json::json!({"id": blocker, "status": "done"}),
                &context(
                    Some(blocker),
                    "worker-backend",
                    "p4-39-complete-prerequisite",
                ),
            )
            .await;
        assert!(
            !completed.is_error,
            "prerequisite status tool failed: {}",
            completed.output
        );

        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                let dependent_row = tasks
                    .get_by_id(dependent)
                    .expect("read dependent while scheduler runs")
                    .expect("dependent remains persisted");
                let supervisor_status = WorkersDb::open()
                    .expect("reopen worker roster while scheduler runs")
                    .get_by_slug("supervisor")
                    .expect("read Supervisor state")
                    .expect("persisted Supervisor remains present")
                    .status;
                let backend_status = WorkersDb::open()
                    .expect("reopen worker roster while scheduler runs")
                    .get_by_slug("worker-backend")
                    .expect("read backend state")
                    .expect("persisted backend remains present")
                    .status;
                if dependent_row.status.as_str() == "done"
                    && supervisor_status == WorkerStatus::Idle
                    && backend_status == WorkerStatus::Idle
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Supervisor and saved worker recipe autonomously complete the dependent");

        let dependent_row = tasks
            .get_by_id(dependent)
            .expect("read completed dependent")
            .expect("completed dependent remains persisted");
        assert_eq!(dependent_row.status.as_str(), "done");
        assert!(
            dependent_row.assigned_worker.is_none(),
            "completed status call clears the active assignment"
        );
        let route = tasks
            .get_routing_history(20)
            .expect("production routing-history reader")
            .into_iter()
            .find(|entry| entry.task_id == dependent.to_string())
            .expect("automatic dispatch persists its worker attribution");
        assert_eq!(route.worker_slug, "worker-backend");
        assert_eq!(route.outcome.as_deref(), Some("success"));

        let held_before_claim = tasks
            .get_by_id(budget_held)
            .expect("read budget-held dependent")
            .expect("budget-held dependent remains persisted");
        assert_eq!(held_before_claim.status.as_str(), "needs_info");
        assert!(held_before_claim.budget_exceeded);
        assert_eq!(
            held_before_claim.budget_exceeded_reason.as_deref(),
            Some("saved task budget was reached")
        );
        let held_claim = ClaimTaskTool
            .execute(
                serde_json::json!({"task_id": budget_held}),
                &context(
                    Some(budget_held),
                    "worker-platform",
                    "p4-39-claim-budget-held",
                ),
            )
            .await;
        assert!(
            held_claim.is_error,
            "budget-held task was claimable without explicit resolution: {}",
            held_claim.output
        );
        assert_eq!(
            tasks
                .get_by_id(budget_held)
                .expect("verify held task after refused claim")
                .expect("held task remains persisted")
                .status
                .as_str(),
            "needs_info"
        );

        let requests = provider
            .requests
            .lock()
            .expect("read captured scripted provider requests")
            .clone();
        let ready_list = requests
            .iter()
            .flat_map(|request| &request.messages)
            .flat_map(|message| &message.content)
            .find_map(|content| match content {
                MessageContent::ToolResult {
                    name,
                    content,
                    is_error,
                    ..
                } if name == "list_tasks" && !is_error => Some(content),
                _ => None,
            })
            .expect("persisted Supervisor recipe must consume list_tasks output");
        let ready_list: Value =
            serde_json::from_str(ready_list).expect("parse Supervisor task list");
        let ready_ids = ready_list["tasks"]
            .as_array()
            .expect("Supervisor task list includes tasks")
            .iter()
            .filter_map(|task| task["id"].as_i64())
            .collect::<Vec<_>>();
        assert!(ready_ids.contains(&dependent));
        assert!(!ready_ids.contains(&budget_held));

        assert!(
            requests
                .iter()
                .any(|request| request.tools.iter().any(|tool| tool.name == "claim_task")),
            "the saved worker recipe must run through AgentEngine's claim tool"
        );
        assert!(
            requests
                .iter()
                .flat_map(|request| &request.messages)
                .flat_map(|message| &message.content)
                .any(|content| matches!(
                    content,
                    MessageContent::ToolResult {
                        name,
                        is_error: false,
                        ..
                    } if name == "claim_task"
                )),
            "the worker's atomic claim must succeed before completion"
        );
        scheduler.shutdown().await;
    }

    async fn ensure_isolated_test_analytics() {
        static INIT: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
        INIT.get_or_init(|| async {
            let analytics_home = Box::leak(Box::new(
                tempfile::tempdir().expect("create isolated analytics home"),
            ));
            let _home = crate::db::test_support::with_home(analytics_home.path());
            let manager = Arc::new(
                crate::analytics::AnalyticsManager::new()
                    .await
                    .expect("initialize isolated analytics DB"),
            );
            crate::analytics::install_global(manager);
        })
        .await;
    }

    fn test_db(name: &str) -> TasksDb {
        let root =
            std::env::temp_dir().join(format!("arcana-scheduler-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("create unique isolated fixture directory");
        TasksDb::open_at(&root.join("tasks.db")).expect("open isolated task database")
    }

    fn unique_test_db_path(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("arcana-scheduler-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("create isolated fixture directory");
        root.join("tasks.db")
    }

    #[tokio::test]
    async fn scheduler_kick_runs_persistent_qc_inbox_through_recipe_runner_and_acknowledges() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("initialize shipped QC recipe and worker");
        let tasks = TasksDb::open().expect("open isolated task DB");
        let workers = WorkersDb::open().expect("open isolated worker DB");
        let qc_worker = workers
            .get_by_slug("qc-agent")
            .expect("read persisted QC worker")
            .expect("QC worker is seeded");
        assert_eq!(qc_worker.recipe_id.as_deref(), Some("qc-dispatcher"));
        workers
            .update_status(qc_worker.id, WorkerStatus::Idle)
            .expect("make the QC worker runnable");

        let mut sender = crate::agent::tool_executor::ToolExecutor::new(
            crate::agent::permission::PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "backend-inbox-sender".into(),
            home.path().to_string_lossy().into_owned(),
            crate::agent::tool_trait::AgentIdentity::worker("worker-backend"),
        );
        let sent = sender.execute_batch(vec![(
            Arc::new(crate::agent::builtin_tools::send_worker_message::SendWorkerMessageTool),
            json!({"to": "qc-agent", "body": "Please review the backend result.", "kind": "fyi"}),
            "backend-to-qc".into(),
        )]).await.expect("native worker sends to persistent QC recipient");
        assert!(!sent[0].1.is_error, "{}", sent[0].1.output);
        let sent_payload: serde_json::Value = serde_json::from_str(&sent[0].1.output).unwrap();
        let stale_message = tasks
            .get_worker_message(sent_payload["persisted_message_ids"][0].as_i64().unwrap())
            .unwrap()
            .expect("native sender's row persists");
        assert_eq!(stale_message.from_slug, "worker-backend");
        assert_eq!(stale_message.to_slug, "qc-agent");
        drop(sender);
        let tasks_path = home.path().join(".arcana").join("tasks.db");
        rusqlite::Connection::open(&tasks_path)
            .expect("open isolated task DB for old-timestamp fixture")
            .execute(
                "UPDATE worker_messages
                 SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-10 minutes')
                 WHERE id = ?1",
                [stale_message.id],
            )
            .expect("age the fixture message past the Supervisor threshold");

        let mut supervisor = crate::agent::tool_executor::ToolExecutor::new(
            crate::agent::permission::PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            "stale-message-supervisor".into(),
            home.path().to_string_lossy().into_owned(),
            crate::agent::tool_trait::AgentIdentity::supervisor(),
        );
        let stale_scan = supervisor
            .execute_batch(vec![(
                Arc::new(
                    crate::agent::builtin_tools::list_stale_worker_messages::ListStaleWorkerMessagesTool,
                ),
                json!({"older_than_minutes": 5}),
                "stale-scan".into(),
            )])
            .await
            .expect("read stale message through the production Supervisor tool");
        assert!(
            !stale_scan[0].1.is_error,
            "stale scan: {}",
            stale_scan[0].1.output
        );
        let stale_payload: serde_json::Value =
            serde_json::from_str(&stale_scan[0].1.output).expect("stale list JSON");
        assert_eq!(stale_payload["messages"][0]["id"], stale_message.id);
        let stale_nudge = supervisor
            .execute_batch(vec![(
                Arc::new(crate::agent::builtin_tools::send_worker_message::SendWorkerMessageTool),
                json!({
                    "to": "qc-agent",
                    "body": format!("Please handle stale message #{}.", stale_message.id),
                    "kind": "nudge"
                }),
                "send-stale-nudge".into(),
            )])
            .await
            .expect("persist Supervisor recovery nudge");
        assert!(
            !stale_nudge[0].1.is_error,
            "stale nudge: {}",
            stale_nudge[0].1.output
        );
        let nudge_payload: serde_json::Value =
            serde_json::from_str(&stale_nudge[0].1.output).expect("nudge result JSON");
        let nudge_id = nudge_payload["persisted_message_ids"][0]
            .as_i64()
            .expect("persisted QC nudge ID");
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated egress consent DB")
            .grant(workspace.path(), "provider:google")
            .expect("grant test provider egress for the isolated workspace");
        let state = Arc::new(Mutex::new(crate::state::AppState {
            active_provider: Some("google".into()),
            active_model: Some("gemini-3.8-flash".into()),
            ..crate::state::AppState::default()
        }));
        let provider = Arc::new(QcInboxProvider::new(&[stale_message.id, nudge_id]));
        let runner = RecipeRunner::new_with_workspace_override(
            state,
            Arc::new(
                crate::session::manager::SessionManager::new()
                    .expect("open isolated session manager"),
            ),
            crate::agent::permission::new_pending_permissions(),
            Some(workspace.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone())
        .with_startup_failures_for_test(1);
        let scheduler = RecipeScheduler::new(Arc::new(runner));

        scheduler.kick_worker("qc-agent");
        let first_tick = scheduler
            .kick_handles
            .lock()
            .expect("scheduler kick handles lock")
            .remove("qc-agent")
            .expect("eligible QC inbox work creates one scheduler tick");
        first_tick.await.expect("QC worker tick task completes");

        let acknowledged = tasks
            .list_worker_messages_for("qc-agent", false)
            .expect("read durable QC messages");
        assert!(
            acknowledged
                .iter()
                .any(|message| { message.id == stale_message.id && message.status == "acknowledged" })
                && acknowledged
                    .iter()
                    .any(|message| { message.id == nudge_id && message.status == "acknowledged" }),
            "the real QC tool path must acknowledge the stale item and its Supervisor nudge; rows={acknowledged:?}; requests={}; worker={:?}; run={:?}",
            provider.request_count(),
            workers.get_by_slug("qc-agent").unwrap(),
            crate::db::runs::RunsDb::open()
                .unwrap()
                .latest_worker_tick("qc-agent")
                .unwrap()
        );
        assert_eq!(
            provider.request_count(),
            5,
            "the transient preparation retry must enter AgentEngine exactly once; the shipped QC recipe reads, acks both message IDs, inspects its queue, and stops"
        );
        let qc_runs = crate::db::runs::RunsDb::open()
            .expect("open durable worker run ledger")
            .latest_worker_ticks(10)
            .expect("read production worker run rows")
            .into_iter()
            .filter(|run| run.worker_slug == "qc-agent")
            .collect::<Vec<_>>();
        assert_eq!(
            qc_runs.len(),
            1,
            "retryable setup failure must keep the original durable run instead of opening a duplicate"
        );
        assert_eq!(qc_runs[0].phase, "completed");

        scheduler.kick_worker("qc-agent");
        assert!(
            !scheduler
                .kick_handles
                .lock()
                .expect("scheduler kick handles lock")
                .contains_key("qc-agent"),
            "an acknowledged inbox with no QC tasks must not dispatch another model run"
        );
        assert_eq!(provider.request_count(), 5);
        assert_eq!(tasks.count_unacknowledged_for("qc-agent").unwrap(), 0);
    }

    #[tokio::test]
    async fn scheduler_replays_inbox_after_completed_effect_without_duplicate_mutation() {
        use crate::provider::types::MessageContent;

        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("persist shipped Supervisor recipe and worker");
        let tasks = TasksDb::open().unwrap();
        let task_id = new_ready_task(&tasks, "cancel obsolete inbox task");
        let message = tasks
            .insert_worker_message(
                "worker-backend",
                "supervisor",
                "Cancel this obsolete task and acknowledge after handling it.",
                "fyi",
                Some(task_id),
            )
            .unwrap();
        let audit = rusqlite::Connection::open(home.path().join(".arcana/tasks.db")).unwrap();
        audit
            .execute_batch(
                "CREATE TABLE test_status_effects(task_id INTEGER NOT NULL);
             CREATE TRIGGER test_count_status_effect AFTER UPDATE OF status ON tasks
             WHEN OLD.status != NEW.status BEGIN
                 INSERT INTO test_status_effects(task_id) VALUES(NEW.id);
             END;",
            )
            .unwrap();
        let workers = WorkersDb::open().unwrap();
        let worker = workers.get_by_slug("supervisor").unwrap().unwrap();
        workers
            .update_status(worker.id, WorkerStatus::Idle)
            .unwrap();
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .unwrap()
            .grant(workspace.path(), "provider:google")
            .unwrap();
        let call = |id: &str, name: &str, input: serde_json::Value| {
            vec![
                StreamChunk::ToolUseStart {
                    id: id.into(),
                    name: name.into(),
                    block_index: 0,
                    opaque: None,
                },
                StreamChunk::ToolInputDelta {
                    block_index: 0,
                    partial_json: input.to_string(),
                },
                StreamChunk::MessageDelta {
                    stop_reason: "tool_use".into(),
                    usage: Usage {
                        input: 10,
                        output: 1,
                        ..Usage::default()
                    },
                },
                StreamChunk::Done,
            ]
        };
        let paused = Arc::new(tokio::sync::Notify::new());
        let first_provider = Arc::new(QcInboxProvider {
            responses: Mutex::new(VecDeque::from([
                call("first-inbox", "list_worker_messages", json!({})),
                call(
                    "completed-effect",
                    "update_task_status",
                    json!({
                    "id": task_id, "status": "cancelled",
                    }),
                ),
            ])),
            requests: Mutex::new(Vec::new()),
            pause_after_script: Some(paused.clone()),
        });
        let make_scheduler = |provider: Arc<QcInboxProvider>| {
            let state = Arc::new(Mutex::new(crate::state::AppState {
                active_provider: Some("google".into()),
                active_model: Some("gemini-3.8-flash".into()),
                ..crate::state::AppState::default()
            }));
            RecipeScheduler::new(Arc::new(
                RecipeRunner::new_with_workspace_override(
                    state,
                    Arc::new(crate::session::manager::SessionManager::new().unwrap()),
                    crate::agent::permission::new_pending_permissions(),
                    Some(workspace.path().to_string_lossy().into_owned()),
                )
                .with_provider_for_test(provider),
            ))
        };
        let first_scheduler = make_scheduler(first_provider.clone());
        first_scheduler.kick_worker("supervisor");
        let first_tick = first_scheduler
            .kick_handles
            .lock()
            .unwrap()
            .remove("supervisor")
            .expect("first inbox work is dispatched");
        tokio::time::timeout(Duration::from_secs(10), paused.notified())
            .await
            .expect("provider reached post-effect/pre-ack interruption point");
        assert_eq!(
            tasks.get_by_id(task_id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::Cancelled
        );
        assert_eq!(
            tasks
                .get_worker_message(message.id)
                .unwrap()
                .unwrap()
                .status,
            "read"
        );
        first_tick.abort();
        assert!(first_tick.await.unwrap_err().is_cancelled());
        assert!(!kick_inflight().lock().unwrap().contains("supervisor"));
        assert_eq!(
            workers.get_by_slug("supervisor").unwrap().unwrap().status,
            WorkerStatus::Idle
        );
        assert!(worker_has_recoverable_work("supervisor").unwrap());
        let runs_db = crate::db::runs::RunsDb::open().unwrap();
        let interrupted = runs_db.latest_worker_tick("supervisor").unwrap().unwrap();
        assert_eq!(interrupted.phase, "interrupted");
        drop(first_scheduler);

        let replay_provider = Arc::new(QcInboxProvider {
            responses: Mutex::new(VecDeque::from([
                call("replay-inbox", "list_worker_messages", json!({})),
                call(
                    "reconcile-state",
                    "list_tasks",
                    json!({"status": "cancelled", "compact": false}),
                ),
                call(
                    "ack-reconciled",
                    "acknowledge_worker_message",
                    json!({"id": message.id}),
                ),
                vec![
                    StreamChunk::TextDelta {
                        text: "Cancellation already persisted; inbox acknowledged.".into(),
                        block_index: 0,
                    },
                    StreamChunk::MessageDelta {
                        stop_reason: "end_turn".into(),
                        usage: Usage {
                            input: 10,
                            output: 1,
                            ..Usage::default()
                        },
                    },
                    StreamChunk::Done,
                ],
            ])),
            requests: Mutex::new(Vec::new()),
            pause_after_script: None,
        });
        let second_scheduler = make_scheduler(replay_provider.clone());
        second_scheduler.kick_worker("supervisor");
        let replay_tick = second_scheduler
            .kick_handles
            .lock()
            .unwrap()
            .remove("supervisor")
            .expect("persisted unacked message is redispatched");
        tokio::time::timeout(Duration::from_secs(10), replay_tick)
            .await
            .unwrap()
            .unwrap();
        let requests = replay_provider.requests.lock().unwrap();
        assert!(
            requests[0]
                .system
                .as_deref()
                .unwrap()
                .contains("check current domain state and confirm that effect is still needed"),
            "the saved Supervisor replay policy must reach the real engine request"
        );
        let result_for = |call_id: &str| {
            requests
                .iter()
                .flat_map(|request| &request.messages)
                .flat_map(|message| &message.content)
                .find_map(|content| match content {
                    MessageContent::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                        ..
                    } if tool_use_id == call_id => {
                        assert!(!is_error);
                        Some(serde_json::from_str::<serde_json::Value>(content).unwrap())
                    }
                    _ => None,
                })
                .expect("production tool result reaches the replay consumer")
        };
        let inbox = result_for("replay-inbox");
        assert_eq!(inbox["messages"][0]["id"], message.id);
        assert_eq!(inbox["messages"][0]["status"], "read");
        let state = result_for("reconcile-state");
        assert_eq!(state["tasks"][0]["id"], task_id);
        assert_eq!(state["tasks"][0]["status"], "cancelled");
        drop(requests);
        let effects: i64 = audit
            .query_row(
                "SELECT count(*) FROM test_status_effects WHERE task_id=?1",
                [task_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            effects, 1,
            "replay must not repeat the already-persisted domain effect"
        );
        assert_eq!(
            tasks
                .get_worker_message(message.id)
                .unwrap()
                .unwrap()
                .status,
            "acknowledged"
        );
        assert_eq!(tasks.count_unacknowledged_for("supervisor").unwrap(), 0);
        let completed = runs_db.latest_worker_tick("supervisor").unwrap().unwrap();
        assert_ne!(completed.run_id, interrupted.run_id);
        assert_eq!(completed.phase, "completed");
        assert!(runs_db
            .get(&interrupted.run_id)
            .unwrap()
            .unwrap()
            .ended_at
            .is_some());
        second_scheduler.kick_worker("supervisor");
        assert!(!second_scheduler
            .kick_handles
            .lock()
            .unwrap()
            .contains_key("supervisor"));
        assert_eq!(replay_provider.request_count(), 4);
        assert_eq!(first_provider.request_count(), 3);
    }

    fn new_ready_task(db: &TasksDb, title: &str) -> i64 {
        db.create(CreateTaskInput {
            title: title.into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "test".into(),
            created_by: Some("scheduler-test".into()),
        })
        .expect("create task")
        .id
    }

    #[test]
    fn startup_preserves_custom_recipe_and_scheduler_reads_it_through_file_backed_worker_binding() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("initialize shipped recipe files");

        let workers = WorkersDb::open().expect("open file-backed worker database");
        let worker = workers
            .get_by_slug("worker-backend")
            .expect("read worker row")
            .expect("shipped backend worker");
        workers
            .update_status(worker.id, WorkerStatus::Idle)
            .expect("persist worker state");
        workers
            .update_model(worker.id, "gemini-3.7-flash")
            .expect("persist worker model");

        let recipes_dir =
            crate::recipes::recipes_dir().expect("resolve isolated recipes directory");
        let mut recipe = crate::recipes::parser::load_recipe_by_id("worker-backend-poll")
            .expect("load shipped recipe")
            .expect("shipped backend recipe");
        recipe.instructions = "Customized backend workflow survives startup.".into();
        crate::recipes::parser::save_recipe(&recipes_dir, &recipe).expect("save user prompt edit");
        drop(workers);

        // Exercise the startup initializer again, then the resolver called by
        // `start_worker`; the worker's on-disk DB row selects the recipe file.
        crate::recipes::init().expect("reinitialize without overwriting prompt edits");
        let (persisted_worker, recipe_id, resolved_recipe) =
            worker_recipe_for_start("worker-backend").expect("resolve scheduled worker profile");

        assert!(home.path().join(".arcana/workers.db").is_file());
        assert_eq!(persisted_worker.status, WorkerStatus::Idle);
        assert_eq!(persisted_worker.model, "gemini-3.7-flash");
        assert_eq!(
            persisted_worker.recipe_id.as_deref(),
            Some("worker-backend-poll")
        );
        assert_eq!(recipe_id, "worker-backend-poll");
        assert_eq!(
            resolved_recipe.instructions,
            "Customized backend workflow survives startup."
        );
    }

    #[test]
    fn interrupted_worker_run_recovers_claim_and_persisted_worker_status() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let slug = "worker-recovery-test";
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(
                "Recovery worker",
                slug,
                "custom",
                "gemini-2.5-flash",
                &[],
                120,
                None,
            )
            .expect("create test worker");
        workers
            .update_status(worker_id, WorkerStatus::Working)
            .expect("persist interrupted state");
        let runs = crate::db::runs::RunsDb::open().expect("open runs ledger");
        let run_id = runs
            .begin_worker_tick(slug, "recovery-test-session")
            .expect("record interrupted run");
        runs.update_worker_tick_phase(&run_id, "running", None, None)
            .expect("mark run active");

        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task_id = new_ready_task(&tasks, "recover after interrupted run");
        tasks
            .update_status(task_id, "assigned", Some(slug))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task_id, slug, std::process::id())
            .expect("persist in-progress claim");

        assert_eq!(
            recover_interrupted_worker_run(slug, true).unwrap(),
            vec![task_id]
        );
        let recovered_task = tasks.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(
            recovered_task.status,
            crate::db::tasks::TaskStatus::NeedsInfo
        );
        assert_eq!(recovered_task.assigned_worker.as_deref(), Some(slug));
        assert!(recovered_task
            .rejection_reason
            .as_deref()
            .is_some_and(|reason| reason.starts_with("[worker recovery]")));
        assert!(recovered_task.claimed_by_pid.is_none());
        assert!(worker_has_recoverable_work_in(&tasks, "supervisor").unwrap());
        assert_eq!(
            workers.get_by_slug(slug).unwrap().unwrap().status,
            WorkerStatus::Idle
        );
        let run = runs
            .latest_worker_tick(slug)
            .expect("read reconciled run")
            .expect("run record exists");
        assert_eq!(run.phase, "interrupted");
        assert_eq!(
            run.outcome.as_deref(),
            Some(crate::db::runs::outcome::ABANDONED)
        );
        assert_eq!(
            run.failure_code.as_deref(),
            Some("scheduler_reconciled_interruption")
        );
    }

    #[tokio::test]
    async fn production_timeout_guard_quarantines_claim_for_supervisor_reconciliation() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let slug = "worker-timeout-reconcile";
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(
                "Timeout recovery worker",
                slug,
                "custom",
                "gemini-3.8-flash",
                &[],
                120,
                None,
            )
            .expect("create worker");
        workers
            .update_status(worker_id, WorkerStatus::Working)
            .expect("mark worker active");

        let runs = crate::db::runs::RunsDb::open().expect("open runs ledger");
        let run_id = runs
            .begin_worker_tick(slug, "timeout-recovery-session")
            .expect("persist worker run owner");
        runs.update_worker_tick_phase(&run_id, "running", None, None)
            .expect("mark worker run active");

        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task_id = new_ready_task(&tasks, "reconcile timed-out project work");
        tasks
            .update_status(task_id, "assigned", Some(slug))
            .expect("assign task to worker");
        tasks
            .claim_task_with_pid(task_id, slug, std::process::id())
            .expect("persist claimed task");

        let guard = try_begin_worker_run(slug).expect("acquire production run guard");
        let timed_out = run_worker_future(
            slug,
            Duration::from_millis(1),
            std::future::pending::<Result<()>>(),
        )
        .await;
        assert!(timed_out.is_err(), "the bounded worker run must time out");
        drop(guard);

        let task = tasks.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(task.status, crate::db::tasks::TaskStatus::NeedsInfo);
        assert_eq!(task.assigned_worker.as_deref(), Some(slug));
        assert!(task.claimed_by_pid.is_none());
        assert!(task
            .rejection_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("side-effect reconciliation")));
        assert!(worker_has_recoverable_work_in(&tasks, "supervisor").unwrap());
        assert!(!worker_has_recoverable_work_in(&tasks, slug).unwrap());
        assert!(!kick_inflight().lock_tolerant().contains(slug));

        let run = runs
            .latest_worker_tick(slug)
            .expect("read recovered run")
            .expect("run is durable");
        assert_eq!(run.phase, "interrupted");
        assert_eq!(
            run.failure_code.as_deref(),
            Some("scheduler_reconciled_interruption")
        );
        assert_eq!(
            workers.get_by_slug(slug).unwrap().unwrap().status,
            WorkerStatus::Idle
        );
    }

    #[test]
    fn stale_scan_refuses_to_recover_a_worker_with_a_live_run_owner() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let slug = "worker-live-owner";
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(
                "Live owner",
                slug,
                "custom",
                "gemini-3.8-flash",
                &[],
                120,
                None,
            )
            .expect("create worker");
        workers
            .update_status(worker_id, WorkerStatus::Working)
            .expect("mark worker active");
        let runs = crate::db::runs::RunsDb::open().expect("open runs ledger");
        runs.begin_worker_tick(slug, "live-owner-session")
            .expect("record live process owner");

        assert!(
            recover_interrupted_worker_run(slug, false).is_err(),
            "a stale scan must not release a live process's worker claim"
        );
        assert_eq!(
            workers.get_by_slug(slug).unwrap().unwrap().status,
            WorkerStatus::Working
        );
    }

    #[test]
    fn task_db_recovery_failure_keeps_worker_working_for_the_next_recovery_pass() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let arcana_dir = home.path().join(".arcana");
        let runs = crate::db::runs::RunsDb::open().expect("open isolated runs DB");
        let run_id = runs
            .begin_worker_tick("worker-recovery-db-failure", "task-db-failure")
            .expect("create run owner for recovery");
        runs.update_worker_tick_phase(&run_id, "running", None, None)
            .expect("mark run active");
        std::fs::create_dir_all(arcana_dir.join("tasks.db"))
            .expect("make tasks DB path fail closed as a directory");
        let slug = "worker-recovery-db-failure";
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(slug, slug, "custom", "gemini-2.5-flash", &[], 120, None)
            .expect("create worker");
        workers
            .update_status(worker_id, WorkerStatus::Working)
            .expect("persist interrupted state");

        assert!(
            recover_interrupted_worker_run(slug, true).is_err(),
            "failed task recovery must remain visible as an error"
        );
        assert_eq!(
            workers.get_by_slug(slug).unwrap().unwrap().status,
            WorkerStatus::Working,
            "the scheduled recovery pass must still see the interrupted worker"
        );
    }

    #[tokio::test]
    async fn panic_and_timeout_inside_guarded_runs_recover_state_and_keep_dispatch_eligible() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let tasks = TasksDb::open().expect("open isolated tasks DB");

        for (slug, interrupt) in [
            ("worker-panic-recovery", "panic"),
            ("worker-timeout-recovery", "timeout"),
        ] {
            let worker_id = workers
                .create(slug, slug, "custom", "gemini-2.5-flash", &[], 120, None)
                .expect("create worker");
            workers
                .update_status(worker_id, WorkerStatus::Working)
                .expect("mark run active");
            let task_id = new_ready_task(&tasks, &format!("{interrupt} recovery"));
            tasks
                .update_status(task_id, "assigned", Some(slug))
                .expect("assign task");
            tasks
                .claim_task_with_pid(task_id, slug, std::process::id())
                .expect("claim task");
            crate::db::runs::RunsDb::open()
                .expect("open run ledger")
                .begin_worker_tick(slug, &format!("{interrupt}-recovery-session"))
                .expect("persist the interrupted run owner");

            let result = if interrupt == "panic" {
                run_if_recoverable_work_with_timeout(
                    slug,
                    Duration::from_secs(1),
                    || Ok(true),
                    async {
                        panic!("injected recipe panic");
                        #[allow(unreachable_code)]
                        Ok(())
                    },
                )
                .await
            } else {
                run_if_recoverable_work_with_timeout(
                    slug,
                    Duration::from_millis(1),
                    || Ok(true),
                    std::future::pending::<Result<()>>(),
                )
                .await
            };
            assert!(result.is_err(), "{interrupt} should fail the recipe run");
            assert!(!kick_inflight().lock_tolerant().contains(slug));
            let recovered_task = tasks.get_by_id(task_id).unwrap().unwrap();
            assert_eq!(
                recovered_task.status,
                crate::db::tasks::TaskStatus::NeedsInfo
            );
            assert_eq!(recovered_task.assigned_worker.as_deref(), Some(slug));
            assert!(recovered_task.claimed_by_pid.is_none());
            assert_eq!(
                workers.get_by_slug(slug).unwrap().unwrap().status,
                WorkerStatus::Idle
            );
            assert!(worker_has_recoverable_work_in(&tasks, "supervisor").unwrap());
        }
    }

    #[tokio::test]
    async fn recovered_supervisor_state_reaches_a_real_recipe_runner_on_the_next_tick() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::db::app_settings::set(
            crate::db::app_settings::WORKSPACE_KEY,
            &home.path().to_string_lossy(),
        )
        .expect("configure the isolated workspace for the real runner");
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let supervisor = workers
            .get_by_slug("supervisor")
            .expect("read seeded Supervisor")
            .expect("seeded Supervisor exists");
        workers
            .update_status(supervisor.id, WorkerStatus::Working)
            .expect("mark interrupted Supervisor run");
        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task_id = new_ready_task(&tasks, "redispatch after interrupted Supervisor tick");
        tasks
            .update_status(task_id, "assigned", Some("supervisor"))
            .expect("assign recovery task to Supervisor");
        tasks
            .claim_task_with_pid(task_id, "supervisor", std::process::id())
            .expect("persist interrupted claim");
        crate::db::runs::RunsDb::open()
            .expect("open run ledger")
            .begin_worker_tick("supervisor", "interrupted-supervisor-session")
            .expect("persist the interrupted Supervisor run owner");

        let interrupted = run_if_recoverable_work_with_timeout(
            "supervisor",
            Duration::from_millis(1),
            || Ok(true),
            std::future::pending::<Result<()>>(),
        )
        .await;
        assert!(interrupted.is_err(), "the injected run must time out");
        assert_eq!(
            tasks.get_by_id(task_id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::NeedsInfo,
            "the interrupted claim must be held for side-effect reconciliation"
        );
        assert_eq!(
            workers.get_by_slug("supervisor").unwrap().unwrap().status,
            WorkerStatus::Idle,
            "recovery must make the Supervisor runnable again"
        );

        // Exercise the same next-tick helper and RecipeRunner used by startup
        // reconciliation. No provider is configured, so a real recovered tick
        // reaches the runner and stops before provider construction.
        let runner = RecipeRunner::new(
            Arc::new(Mutex::new(crate::state::AppState::default())),
            Arc::new(crate::session::manager::SessionManager::new().unwrap()),
            crate::agent::permission::new_pending_permissions(),
        );
        let recipe = crate::recipes::types::Recipe {
            version: "1.0.0".into(),
            id: "supervisor-poll".into(),
            title: "Supervisor recovery test".into(),
            description: None,
            schedule: Some("*/5 * * * *".into()),
            permission_mode: crate::recipes::types::RecipePermissionMode::Autonomous,
            instructions: "inspect the task board".into(),
            agent_role: None,
            model: None,
            is_system: true,
        };
        let ran = run_if_recoverable_work(
            "supervisor",
            || worker_has_recoverable_work("supervisor"),
            run_worker_recipe(&runner, &recipe, "supervisor"),
        )
        .await
        .expect("the recovered production runner tick should complete");
        assert!(ran, "the next tick must be admitted after recovery");
        assert_eq!(
            workers.get_by_slug("supervisor").unwrap().unwrap().status,
            WorkerStatus::IdleNoProvider,
            "the real RecipeRunner was reached without restarting Arcana"
        );
        assert!(!kick_inflight().lock_tolerant().contains("supervisor"));
    }

    #[tokio::test]
    async fn cancelling_a_guarded_recipe_recovers_the_task_and_worker() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let slug = "worker-cancel-recovery";
        let workers = WorkersDb::open().expect("open isolated workers DB");
        let worker_id = workers
            .create(slug, slug, "custom", "gemini-2.5-flash", &[], 120, None)
            .expect("create worker");
        workers
            .update_status(worker_id, WorkerStatus::Working)
            .expect("mark run active");
        let tasks = TasksDb::open().expect("open isolated tasks DB");
        let task_id = new_ready_task(&tasks, "cancelled in-process run");
        tasks
            .update_status(task_id, "assigned", Some(slug))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task_id, slug, std::process::id())
            .expect("claim task");
        crate::db::runs::RunsDb::open()
            .expect("open run ledger")
            .begin_worker_tick(slug, "cancelled-run-session")
            .expect("persist the interrupted run owner");

        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let run = tokio::spawn(async move {
            run_if_recoverable_work_with_timeout(
                slug,
                Duration::from_secs(30),
                || Ok(true),
                async move {
                    let _ = started_tx.send(());
                    std::future::pending::<Result<()>>().await
                },
            )
            .await
        });
        started_rx.await.expect("run entered its in-process future");
        run.abort();
        let _ = run.await;

        let recovered_task = tasks.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(
            recovered_task.status,
            crate::db::tasks::TaskStatus::NeedsInfo
        );
        assert_eq!(recovered_task.assigned_worker.as_deref(), Some(slug));
        assert_eq!(
            workers.get_by_slug(slug).unwrap().unwrap().status,
            WorkerStatus::Idle
        );
        assert!(!kick_inflight().lock_tolerant().contains(slug));
    }

    #[test]
    fn transient_status_read_error_keeps_schedule_for_retry() {
        let result: Result<Option<WorkerStatus>> =
            Err(ArcanaError::Agent("temporary database failure".into()));

        assert_eq!(
            disposition_for_status_read(result),
            StatusReadDisposition::Retry
        );
    }

    #[test]
    fn missing_worker_stops_schedule_but_present_worker_runs() {
        assert_eq!(
            disposition_for_status_read(Ok(None)),
            StatusReadDisposition::Stop
        );
        assert_eq!(
            disposition_for_status_read(Ok(Some(WorkerStatus::Idle))),
            StatusReadDisposition::Run(WorkerStatus::Idle)
        );
    }

    #[test]
    fn recovery_work_check_recognizes_supervisor_and_assigned_worker_tasks() {
        let db = test_db("recovery-work");
        assert!(!worker_has_recoverable_work_in(&db, "supervisor").unwrap());
        assert!(!worker_has_recoverable_work_in(&db, "worker-backend").unwrap());

        db.create_epic(
            "un-decomposed epic",
            Some("The Supervisor must decompose this request."),
            "scheduler-test",
        )
        .unwrap();
        assert!(
            worker_has_recoverable_work_in(&db, "supervisor").unwrap(),
            "an unassigned ready epic is work for the Supervisor to decompose"
        );

        let ready = new_ready_task(&db, "needs supervisor");
        assert!(worker_has_recoverable_work_in(&db, "supervisor").unwrap());
        db.update_status(ready, "assigned", Some("worker-backend"))
            .unwrap();
        assert!(worker_has_recoverable_work_in(&db, "worker-backend").unwrap());
        assert!(
            worker_has_recoverable_work_in(&db, "supervisor").unwrap(),
            "the un-decomposed epic remains Supervisor work after a child is assigned"
        );

        let interrupted = new_ready_task(&db, "hold interrupted effects");
        let interrupted_owner = "worker-held-only";
        db.update_status(interrupted, "assigned", Some(interrupted_owner))
            .unwrap();
        db.claim_task_with_pid(interrupted, interrupted_owner, 813)
            .unwrap();
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".to_string());
        assert_eq!(
            db.hold_interrupted_claims_for_worker_run(
                interrupted_owner,
                813,
                &host,
                "test interruption"
            )
            .unwrap(),
            vec![interrupted]
        );
        assert!(worker_has_recoverable_work_in(&db, "supervisor").unwrap());
        assert!(
            !worker_has_recoverable_work_in(&db, interrupted_owner).unwrap(),
            "held work must not return to the worker's automatic queue"
        );
    }

    #[tokio::test]
    async fn a_pending_escalation_keeps_supervisor_dispatch_available() {
        use crate::agent::permission::PermissionMode;
        use crate::agent::tool_trait::{Tool, ToolContext};
        use serde_json::json;

        let sandbox = tempfile::tempdir().expect("create isolated Arcana home");
        let _home = crate::db::test_support::with_home(sandbox.path());
        let db = TasksDb::open().expect("open isolated tasks DB");
        let workers = crate::db::workers::WorkersDb::open().expect("seed worker roster");
        let frontend = workers
            .get_by_slug("worker-frontend")
            .unwrap()
            .expect("frontend worker exists");
        workers
            .update_status(frontend.id, crate::db::workers::WorkerStatus::Idle)
            .expect("make frontend fixture runnable");
        let blocker = db
            .create(CreateTaskInput {
                title: "unresolved escalation blocker".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "remain unresolved".into(),
                created_by: Some("scheduler-test".into()),
            })
            .unwrap();
        db.update_status(blocker.id, "assigned", Some("worker-backend"))
            .unwrap();
        db.create_worker_request(&crate::db::tasks::CreateWorkerRequestInput {
            worker_slug: "worker-backend".into(),
            task_id: Some(blocker.id),
            request_type: "needs_info".into(),
            payload: "{\"question\":\"unresolved\"}".into(),
        })
        .unwrap();

        assert!(worker_has_recoverable_work_in(&db, "supervisor").unwrap());
        assert_eq!(
            db.list_worker_requests(Some("pending")).unwrap().len(),
            1,
            "the request remains unresolved while the supervisor is scheduled"
        );
        let unrelated = new_ready_task(&db, "unrelated task continues to dispatch");

        let context = ToolContext {
            session_id: "p4-escalation-fairness".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "p4-escalation-dispatch".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let result = crate::agent::builtin_tools::assign_tasks_batch::AssignTasksBatchTool
            .execute(
                json!({"assignments": [{"task_id": unrelated, "worker_slug": "worker-frontend", "override_reason": "fixture exercises partial dispatch while backend is occupied"}]}),
                &context,
            )
            .await;
        assert!(!result.is_error, "dispatch remains usable: {result:?}");
        assert_eq!(
            db.get_by_id(unrelated).unwrap().unwrap().status.as_str(),
            "assigned"
        );
        assert_eq!(
            db.list_worker_requests(Some("pending")).unwrap().len(),
            1,
            "dispatch must not require silently resolving the blocker"
        );
    }

    #[test]
    fn recovery_work_check_keeps_read_messages_until_acknowledged() {
        let db = test_db("message-work");
        let message = db
            .insert_worker_message("supervisor", "worker-backend", "resume", "task", None)
            .unwrap();
        assert!(worker_has_recoverable_work_in(&db, "worker-backend").unwrap());
        let read = db.list_worker_messages_for("worker-backend", true).unwrap();
        assert_eq!(read[0].id, message.id);
        assert_eq!(read[0].status, "pending");
        assert!(worker_has_recoverable_work_in(&db, "worker-backend").unwrap());
        let replay = db.list_worker_messages_for("worker-backend", true).unwrap();
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].id, message.id, "re-dispatch keeps the stable ID");
        assert_eq!(replay[0].status, "read");
        assert!(worker_has_recoverable_work_in(&db, "worker-backend").unwrap());
        db.acknowledge_worker_message(message.id).unwrap();
        assert!(!worker_has_recoverable_work_in(&db, "worker-backend").unwrap());
    }

    #[test]
    fn rejected_task_remains_recoverable_for_the_worker() {
        let db = test_db("rejected-task-work");
        let task_id = new_ready_task(&db, "rejected work is re-dispatched");
        db.update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task to its implementation worker");
        db.claim_task_with_pid(task_id, "worker-backend", 42)
            .expect("claim task before QC rejection");
        db.update_status_with_reason(
            task_id,
            "rejected",
            Some("worker-backend"),
            Some("The persisted build result needs a correction."),
        )
        .expect("persist QC rejection for rework");

        let rejected = db.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(rejected.status, crate::db::tasks::TaskStatus::Rejected);
        assert_eq!(rejected.assigned_worker.as_deref(), Some("worker-backend"));
        assert!(
            worker_has_recoverable_work_in(&db, "worker-backend").unwrap(),
            "a rejected task remains eligible for the worker/dispatch recovery path"
        );
    }

    #[tokio::test]
    async fn empty_startup_and_qc_recovery_checks_do_not_poll_the_provider() {
        let db = test_db("empty-provider-budget");
        let provider_calls = std::sync::atomic::AtomicUsize::new(0);

        for slug in ["supervisor", "qc-agent"] {
            let called = &provider_calls;
            let ran = run_if_recoverable_work(
                slug,
                || worker_has_recoverable_work_in(&db, slug),
                async move {
                    called.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                },
            )
            .await
            .expect("empty queue is a successful idle check");
            assert!(!ran, "{slug} must remain idle with empty work queues");
        }

        assert_eq!(
            provider_calls.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "empty startup/recovery checks must not invoke the provider"
        );
    }

    #[tokio::test]
    async fn production_idle_worker_kick_skips_provider_and_records_idle_gate() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("initialize shipped recipes and default workers");
        let workers = WorkersDb::open().expect("open isolated worker roster");
        let qc_worker = workers
            .get_by_slug("qc-agent")
            .expect("read QC worker")
            .expect("default QC worker is seeded");
        workers
            .update_status(qc_worker.id, WorkerStatus::Idle)
            .expect("make QC worker runnable");

        let provider = Arc::new(QcInboxProvider::new(&[]));
        let runner = RecipeRunner::new_with_workspace_override(
            Arc::new(Mutex::new(crate::state::AppState {
                active_provider: Some("google".into()),
                active_model: Some("gemini-3.8-flash".into()),
                ..crate::state::AppState::default()
            })),
            Arc::new(
                crate::session::manager::SessionManager::new()
                    .expect("open isolated session manager"),
            ),
            crate::agent::permission::new_pending_permissions(),
            Some(home.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone());
        let scheduler = RecipeScheduler::new(Arc::new(runner));

        let (_, events) = crate::observability::timing::capture_json_events(|| {
            scheduler.kick_worker("qc-agent");
        });

        assert_eq!(
            provider.request_count(),
            0,
            "an empty production push-kick must stop before provider resolution"
        );
        assert!(
            scheduler
                .kick_handles
                .lock()
                .expect("scheduler kick handles lock")
                .is_empty(),
            "an idle work gate must not create a worker-run future"
        );
        let gate = events
            .iter()
            .find(|event| event["fields"]["operation"] == "scheduler_recoverable_work_check")
            .expect("the production work gate records an observation");
        assert_eq!(gate["fields"]["worker"], "qc-agent");
        assert_eq!(gate["fields"]["outcome"], "idle");
        assert_eq!(gate["fields"]["database"], "tasks");
        assert!(gate["fields"]["duration_ns"].as_u64().is_some());
    }

    #[test]
    fn production_startup_idle_workers_skip_provider_and_record_gate() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create current-thread test runtime");
        runtime.block_on(ensure_isolated_test_analytics());
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().expect("initialize shipped recipes and default workers");

        let provider = Arc::new(QcInboxProvider::new(&[]));
        let runner = RecipeRunner::new_with_workspace_override(
            Arc::new(Mutex::new(crate::state::AppState {
                active_provider: Some("google".into()),
                active_model: Some("gemini-3.8-flash".into()),
                ..crate::state::AppState::default()
            })),
            Arc::new(
                crate::session::manager::SessionManager::new()
                    .expect("open isolated session manager"),
            ),
            crate::agent::permission::new_pending_permissions(),
            Some(home.path().to_string_lossy().into_owned()),
        )
        .with_provider_for_test(provider.clone());
        let scheduler = RecipeScheduler::new(Arc::new(runner));

        let startup_workers = ["supervisor", "qc-agent", "worker-backend"];
        let (_, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(async {
                tokio::time::pause();
                for worker in startup_workers {
                    scheduler.start_worker(worker).unwrap_or_else(|error| {
                        panic!("start production schedule for {worker}: {error}")
                    });
                }

                // Startup ticks are spawned onto this current-thread runtime.
                // Yield until their work-check events have run, then stop all
                // schedules before the test can leave live tasks behind.
                for _ in 0..32 {
                    tokio::task::yield_now().await;
                }

                // Drive the actual fixed five-minute cron recovery loop,
                // rather than invoking only its work-check helper.
                tokio::time::advance(Duration::from_secs(310)).await;
                for _ in 0..32 {
                    tokio::task::yield_now().await;
                }

                for worker in startup_workers {
                    scheduler.stop_worker(worker).unwrap_or_else(|error| {
                        panic!("stop production schedule for {worker}: {error}")
                    });
                }
            })
        });

        assert_eq!(
            provider.request_count(),
            0,
            "production Supervisor/QC/backend startup ticks must stop before provider resolution when no work exists"
        );
        assert!(
            scheduler
                .handles
                .lock()
                .expect("scheduler handles lock")
                .is_empty(),
            "the schedule handle must be removed after stop"
        );

        for worker in startup_workers {
            let check = events
                .iter()
                .find(|event| {
                    event["fields"]["operation"] == "scheduler_recoverable_work_check"
                        && event["fields"]["worker"] == worker
                })
                .unwrap_or_else(|| panic!("the {worker} startup tick records its work check"));
            assert_eq!(check["fields"]["outcome"], "idle", "worker={worker}");
            assert_eq!(check["fields"]["database"], "tasks", "worker={worker}");
            assert!(check["fields"]["duration_ns"].as_u64().is_some());
            assert!(
                events
                    .iter()
                    .filter(|event| {
                        event["fields"]["operation"] == "scheduler_recoverable_work_check"
                            && event["fields"]["worker"] == worker
                            && event["fields"]["outcome"] == "idle"
                    })
                    .count()
                    >= 2,
                "{worker} must execute both startup and the real cron recovery work-check"
            );

            let gate = events
                .iter()
                .find(|event| {
                    event["fields"]["operation"] == "scheduler_worker_dispatch_gate"
                        && event["fields"]["worker"] == worker
                })
                .unwrap_or_else(|| panic!("the {worker} startup dispatch gate is recorded"));
            assert_eq!(gate["fields"]["outcome"], "idle", "worker={worker}");
            assert_eq!(gate["fields"]["result_count"], 0, "worker={worker}");
            assert!(gate["fields"]["duration_ns"].as_u64().unwrap_or_default() > 0);
        }
    }

    #[tokio::test]
    async fn production_supervisor_and_qc_provider_turns_stop_at_run_caps() {
        ensure_isolated_test_analytics().await;
        let home = tempfile::tempdir().unwrap();
        let _home = crate::db::test_support::with_home(home.path());
        crate::recipes::init().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        crate::egress_consent::WorkspaceEgressConsentDb::open()
            .unwrap()
            .grant(workspace.path(), "provider:google")
            .unwrap();
        let tasks = TasksDb::open().unwrap();

        for (slug, limit) in [("supervisor", 10), ("qc-agent", 20)] {
            let message = tasks
                .insert_worker_message(
                    "worker-backend",
                    slug,
                    "Bounded intentional work fixture",
                    "fyi",
                    None,
                )
                .unwrap();
            let responses = (0..limit + 1)
                .map(|turn| {
                    vec![
                        StreamChunk::ToolUseStart {
                            id: format!("bounded-{slug}-{turn}"),
                            name: "list_worker_messages".into(),
                            block_index: 0,
                            opaque: None,
                        },
                        StreamChunk::ToolInputDelta {
                            block_index: 0,
                            partial_json: "{}".into(),
                        },
                        StreamChunk::MessageDelta {
                            stop_reason: "tool_use".into(),
                            usage: Usage {
                                input: 10,
                                output: 1,
                                ..Usage::default()
                            },
                        },
                        StreamChunk::Done,
                    ]
                })
                .collect();
            let provider = Arc::new(QcInboxProvider {
                responses: Mutex::new(responses),
                requests: Mutex::new(Vec::new()),
                pause_after_script: None,
            });
            let runner = RecipeRunner::new_with_workspace_override(
                Arc::new(Mutex::new(crate::state::AppState {
                    active_provider: Some("google".into()),
                    active_model: Some("gemini-3.8-flash".into()),
                    ..crate::state::AppState::default()
                })),
                Arc::new(crate::session::manager::SessionManager::new().unwrap()),
                crate::agent::permission::new_pending_permissions(),
                Some(workspace.path().to_string_lossy().into_owned()),
            )
            .with_provider_for_test(provider.clone());
            let worker = WorkersDb::open()
                .unwrap()
                .get_by_slug(slug)
                .unwrap()
                .unwrap();
            let recipe = load_recipe_by_id(worker.recipe_id.as_deref().unwrap())
                .unwrap()
                .expect("persisted worker recipe exists");
            let result = runner.run_recipe(&recipe, slug).await;
            assert!(
                matches!(&result, Err(ArcanaError::BudgetExceeded(reason)) if reason == "BUDGET_EXCEEDED_TURNS"),
                "{slug}: {result:?}"
            );
            // The existing pre-dispatch breaker counts the pending turn and
            // refuses it when turns_used reaches the configured cap.
            assert_eq!(
                provider.request_count(),
                limit - 1,
                "the real {slug} engine must refuse turn {limit}"
            );
            assert_eq!(
                provider.responses.lock().unwrap().len(),
                2,
                "the denied turn must not enter the provider"
            );
            assert_ne!(tasks.get_worker_message(message.id).unwrap().unwrap().status, "acknowledged", "the fixture must retain genuine recoverable work, rather than stop because its inbox emptied");
        }
    }

    #[test]
    fn startup_and_event_dispatch_gate_records_idle_and_completed_runs() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create test runtime");
        let dispatched = std::sync::atomic::AtomicUsize::new(0);
        let (results, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(async {
                let idle =
                    run_if_recoverable_work("p4-43-idle-worker", || Ok(false), async { Ok(()) })
                        .await
                        .expect("idle gate succeeds");
                let count = &dispatched;
                let ran =
                    run_if_recoverable_work("p4-43-dispatched-worker", || Ok(true), async move {
                        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    })
                    .await
                    .expect("work dispatch succeeds");
                (idle, ran)
            })
        });
        assert_eq!(results, (false, true));
        assert_eq!(dispatched.load(std::sync::atomic::Ordering::SeqCst), 1);
        for (worker, outcome, result_count) in [
            ("p4-43-idle-worker", "idle", 0),
            ("p4-43-dispatched-worker", "dispatched", 1),
        ] {
            let event = events
                .iter()
                .find(|event| {
                    event["fields"]["operation"] == "scheduler_worker_dispatch_gate"
                        && event["fields"]["worker"] == worker
                })
                .expect("dispatch gate timing is emitted");
            assert_eq!(event["fields"]["outcome"], outcome);
            assert_eq!(event["fields"]["result_count"], result_count);
            assert!(event["fields"]["duration_ns"].as_u64().unwrap_or_default() > 0);
        }
    }

    #[test]
    fn production_work_gate_emits_safe_idle_and_work_timing() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let db = crate::db::tasks::TasksDb::open().expect("seed task database");
        let (idle, idle_events) = crate::observability::timing::capture_json_events(|| {
            worker_has_recoverable_work("worker-backend")
        });
        assert!(!idle.expect("empty work check succeeds"));

        db.create(crate::db::tasks::CreateTaskInput {
            title: "scheduler work gate timing".into(),
            description: None,
            priority: "p2".into(),
            task_type: "feature".into(),
            acceptance_criteria: "the work gate reports only a bounded presence signal".into(),
            created_by: Some("p4-43-test".into()),
        })
        .expect("create ready task");
        let (work, work_events) = crate::observability::timing::capture_json_events(|| {
            worker_has_recoverable_work("supervisor")
        });
        assert!(work.expect("Supervisor work check succeeds"));

        for (events, expected) in [(&idle_events, "idle"), (&work_events, "work")] {
            let event = events
                .iter()
                .find(|event| event["fields"]["operation"] == "scheduler_recoverable_work_check")
                .expect("production work gate emits timing");
            assert_eq!(event["fields"]["outcome"], expected);
            assert_eq!(event["fields"]["database"], "tasks");
            assert!(event["fields"]["duration_ns"].as_u64().unwrap_or_default() > 0);
        }
        let serialized = serde_json::to_string(&idle_events).expect("serialize safe telemetry");
        assert!(!serialized.contains("scheduler work gate timing"));
        assert!(!serialized.contains("acceptance_criteria"));
    }

    fn reserve_test_kick(slug: &str) {
        assert!(kick_inflight().lock_tolerant().insert(slug.to_string()));
    }

    #[test]
    fn kick_slot_guard_releases_on_normal_return() {
        let slug = "p4-guard-normal-test";
        reserve_test_kick(slug);
        drop(KickSlotGuard::new(slug.to_string()));
        assert!(!kick_inflight().lock_tolerant().contains(slug));
    }

    #[tokio::test]
    async fn kick_slot_guard_releases_after_panicked_run() {
        let slug = "p4-guard-panic-test";
        reserve_test_kick(slug);
        let task_slug = slug.to_string();
        let task = tokio::spawn(async move {
            let _guard = KickSlotGuard::new(task_slug);
            panic!("injected worker panic");
        });
        assert!(task.await.is_err());
        assert!(!kick_inflight().lock_tolerant().contains(slug));
    }

    #[tokio::test]
    async fn kick_slot_guard_releases_after_cancelled_run() {
        let slug = "p4-guard-cancel-test";
        reserve_test_kick(slug);
        let task_slug = slug.to_string();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let _guard = KickSlotGuard::new(task_slug);
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        });
        started_rx.await.expect("guard-owning task started");
        task.abort();
        let _ = task.await;
        assert!(!kick_inflight().lock_tolerant().contains(slug));
    }

    #[tokio::test]
    async fn hard_timeout_cancels_run_and_drops_kick_guard() {
        let slug = "p4-guard-timeout-test";
        reserve_test_kick(slug);
        let task_slug = slug.to_string();
        let outcome = tokio::time::timeout(Duration::from_millis(1), async move {
            let _guard = KickSlotGuard::new(task_slug);
            std::future::pending::<()>().await;
        })
        .await;
        assert!(outcome.is_err());
        assert!(!kick_inflight().lock_tolerant().contains(slug));
        assert_eq!(PUSH_RUN_TIMEOUT, Duration::from_secs(7_200));
    }

    #[tokio::test(start_paused = true)]
    async fn production_worker_deadline_allows_work_past_ten_minutes() {
        let started = tokio::time::Instant::now();
        let result = run_worker_future("p4-long-running-test", PUSH_RUN_TIMEOUT, async {
            tokio::time::sleep(Duration::from_secs(10 * 60 + 1)).await;
            Ok(())
        })
        .await;

        assert!(result.is_ok(), "work must complete before the two-hour cap");
        assert!(
            started.elapsed() > Duration::from_secs(10 * 60),
            "the paused clock must prove more than ten minutes elapsed"
        );
        assert_eq!(PUSH_RUN_TIMEOUT, Duration::from_secs(2 * 60 * 60));
    }

    #[tokio::test]
    async fn timeout_guard_releases_a_claimed_task_through_its_recovery_handler() {
        let slug = "supervisor";
        kick_inflight().lock_tolerant().remove(slug);
        reserve_test_kick(slug);
        let db_path = unique_test_db_path("guard-claim-recovery");
        let db = TasksDb::open_at(&db_path).expect("open test db");
        let task_id = new_ready_task(&db, "claimed before timeout");
        db.update_status(task_id, "assigned", Some(slug)).unwrap();
        db.claim_task_with_pid(task_id, slug, 903).unwrap();
        drop(db);

        let recovery_path = db_path.clone();
        let outcome = async move {
            let _guard = KickSlotGuard::with_recovery(slug.to_string(), move |worker_slug| {
                let db = TasksDb::open_at(&recovery_path)?;
                db.release_in_progress_claims_for_worker(worker_slug, "test timeout recovery")
            });
            run_worker_future(
                slug,
                Duration::from_millis(1),
                std::future::pending::<Result<()>>(),
            )
            .await
        }
        .await;
        assert!(outcome.is_err(), "timeout should surface as a run failure");
        assert!(!kick_inflight().lock_tolerant().contains(slug));

        let db = TasksDb::open_at(&db_path).expect("reopen test db");
        let task = db.get_by_id(task_id).unwrap().unwrap();
        assert_eq!(task.status, crate::db::tasks::TaskStatus::Ready);
        assert!(task.assigned_worker.is_none());
        assert!(task.claimed_by_pid.is_none());
    }
}
