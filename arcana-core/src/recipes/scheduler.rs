// RecipeScheduler — manages cron-based recipe execution for the Worker Fleet.
// Each worker with a bound recipe can be started/stopped independently.
// Uses tokio tasks + the `cron` crate for accurate next-fire-time calculation.
// See: DEVELOPMENT_PLAN.md Phase 2 — Cron Scheduler

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::Utc;
use cron::Schedule;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
// Phase 7.7 — used for metadata JSON in record_system_event calls
use serde_json;

use super::runner::RecipeRunner;
use crate::db::workers::WorkersDb;
use crate::errors::{ArcanaError, Result};
use crate::recipes::parser::load_recipe_by_id;
use crate::state::lock::LockTolerant;

// ─── RecipeScheduler ──────────────────────────────────────────────────────────

/// Manages background cron tasks for all active workers.
/// Stored as Arc<RecipeScheduler> in Tauri managed state — lives for app lifetime.
pub struct RecipeScheduler {
    runner: Arc<RecipeRunner>,
    handles: Mutex<HashMap<String, JoinHandle<()>>>, // worker_slug → running task (tauri async runtime)
}

impl RecipeScheduler {
    pub fn new(runner: Arc<RecipeRunner>) -> Self {
        Self {
            runner,
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Start cron scheduling for a worker.
    /// Idempotent — restarts if already running.
    /// Loads worker + bound recipe from DB, then spawns a tokio task that fires on schedule.
    pub fn start_worker(&self, worker_slug: &str) -> Result<()> {
        // Abort any existing task for this worker (idempotent restart)
        self.abort_worker_task(worker_slug);

        // Load worker + bound recipe
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

        // Parse cron schedule (5-field POSIX → 6-field with leading seconds=0)
        let raw_schedule = recipe
            .schedule
            .clone()
            .unwrap_or_else(|| "*/2 * * * *".to_string());
        let cron_expr = normalize_to_6field(&raw_schedule);

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
        db.update_status(worker.id, "idle")?;
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
            timestamp: chrono::Utc::now().to_rfc3339(),
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
                let released = crate::orphan::reconcile();
                if !released.is_empty() {
                    info!(
                        "[OrphanDetector] Startup: released {} orphaned tasks: {:?}",
                        released.len(),
                        released
                    );
                    crate::events::emit_orphans_released(released);
                }
            }

            if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await {
                // Doc 08 Phase 2 — kind taxonomy. `grep kind=invoke_failed`
                // surfaces every start-up tick failure regardless of slug.
                //
                // ⚠ The reason given for this kind named two causes that can
                // no longer occur. A start-up tick fails today because the
                // provider rejected the call, a tool errored, or the recipe
                // itself is malformed.
                error!(
                    kind = crate::observability::error_kinds::KIND_INVOKE_FAILED,
                    worker_slug = slug_owned.as_str(),
                    "[RecipeScheduler] Startup tick failed for '{}': {e}",
                    slug_owned
                );
                // Non-fatal: continue to cron loop even if startup tick failed
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

                // Check that worker is still active before firing
                let current_status = WorkersDb::open()
                    .and_then(|db| db.get_by_slug(&slug_owned))
                    .ok()
                    .flatten()
                    .map(|w| w.status);

                match current_status.as_deref() {
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
                    Some("idle") | Some("active") | Some("error") | Some("idle_no_provider") => {
                        // Ruflo Feature 2 — run orphan detection before each Supervisor tick
                        if slug_owned == "supervisor" {
                            let released = crate::orphan::reconcile();
                            if !released.is_empty() {
                                info!(
                                    "[OrphanDetector] Released {} orphaned tasks: {:?}",
                                    released.len(),
                                    released
                                );
                                crate::events::emit_orphans_released(released);
                            }
                        }

                        // Fire the recipe
                        info!(
                            "[RecipeScheduler] Firing recipe '{}' for worker '{}'",
                            recipe_id, slug_owned
                        );
                        if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await {
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
                    Some("working") => {
                        // Previous run is still in progress — skip this tick
                        warn!(
                            "[RecipeScheduler] Worker '{}' still working — skipping tick",
                            slug_owned
                        );
                    }
                    Some("paused") | Some("deleted") | None => {
                        info!(
                            "[RecipeScheduler] Worker '{}' is paused/deleted — stopping schedule",
                            slug_owned
                        );
                        break;
                    }
                    _ => {
                        // Unknown status — continue
                        warn!(
                            "[RecipeScheduler] Worker '{}' has unexpected status, continuing",
                            slug_owned
                        );
                    }
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
                let _ = db.update_status(worker.id, "paused");
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
            timestamp: chrono::Utc::now().to_rfc3339(),
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
        struct KickGuard {
            slug: String,
            committed: bool,
        }
        impl Drop for KickGuard {
            fn drop(&mut self) {
                if !self.committed {
                    kick_inflight().lock_tolerant().remove(&self.slug);
                }
            }
        }
        let mut guard = KickGuard {
            slug: slug_for_inflight.clone(),
            committed: false,
        };

        // DB gate: skip if worker is already in flight per the DB status.
        let (worker_opt, recipe_id_opt) = match WorkersDb::open() {
            Ok(db) => match db.get_by_slug(worker_slug) {
                Ok(Some(w)) => {
                    if w.status == "working" {
                        info!(
                            "[RecipeScheduler] kick_worker('{}'): skipped (already working)",
                            worker_slug
                        );
                        return;
                    }
                    let rid = w.recipe_id.clone();
                    (Some(w), rid)
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
        tokio::spawn(async move {
            info!(
                "[RecipeScheduler] kick_worker: firing '{}' immediately (push-based dispatch)",
                slug
            );
            if let Err(e) = runner.run_recipe(&recipe, &slug).await {
                error!(
                    "[RecipeScheduler] kick_worker: recipe run failed for '{}': {e}",
                    slug
                );
            }
            // Free the kick slot so the next push-based kick can fire.
            kick_inflight().lock_tolerant().remove(&slug);
        });
        // The spawned task now owns the responsibility for clearing the
        // kick_inflight entry; mark the guard committed so its Drop doesn't
        // double-remove.
        guard.committed = true;
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
