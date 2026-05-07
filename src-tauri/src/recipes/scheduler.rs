// RecipeScheduler — manages cron-based recipe execution for the Worker Fleet.
// Each worker with a bound recipe can be started/stopped independently.
// Uses tokio tasks + the `cron` crate for accurate next-fire-time calculation.
// See: DEVELOPMENT_PLAN.md Phase 2 — Cron Scheduler

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use chrono::Utc;
use cron::Schedule;
use tauri::async_runtime;
use async_runtime::JoinHandle;
use tracing::{error, info, warn};
// Phase 7.7 — used for metadata JSON in record_system_event calls
use serde_json;

use crate::db::workers::WorkersDb;
use crate::errors::{AutoDevError, Result};
use crate::recipes::parser::load_recipe_by_id;
use super::runner::RecipeRunner;

// ─── RecipeScheduler ──────────────────────────────────────────────────────────

/// Manages background cron tasks for all active workers.
/// Stored as Arc<RecipeScheduler> in Tauri managed state — lives for app lifetime.
pub struct RecipeScheduler {
    runner:  Arc<RecipeRunner>,
    handles: Mutex<HashMap<String, JoinHandle<()>>>,  // worker_slug → running task (tauri async runtime)
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
        let worker = db.get_by_slug(worker_slug)?
            .ok_or_else(|| AutoDevError::Agent(format!("Worker '{worker_slug}' not found")))?;

        let recipe_id = worker.recipe_id.as_deref()
            .ok_or_else(|| AutoDevError::Agent(format!(
                "Worker '{worker_slug}' has no recipe bound — bind a recipe first"
            )))?.to_string();

        let recipe = load_recipe_by_id(&recipe_id)?
            .ok_or_else(|| AutoDevError::Agent(format!("Recipe '{recipe_id}' not found on disk")))?;

        // Parse cron schedule (5-field POSIX → 6-field with leading seconds=0)
        let raw_schedule = recipe.schedule.clone()
            .unwrap_or_else(|| "*/2 * * * *".to_string());
        let cron_expr = normalize_to_6field(&raw_schedule);

        let schedule = Schedule::from_str(&cron_expr)
            .map_err(|e| AutoDevError::Agent(
                format!("Invalid cron expression '{cron_expr}' (recipe '{recipe_id}'): {e}")
            ))?;

        info!("[RecipeScheduler] Starting worker '{}' on schedule '{}' (recipe: '{}')",
            worker_slug, cron_expr, recipe_id);

        // Set worker to 'idle' (active, ready to fire)
        db.update_status(worker.id, "idle")?;
        // Phase 3.0 R2 — push status to UI so the Agent Roster turns this
        // row "alive" the moment the user hits Start.
        crate::events::emit_worker_status_global(crate::events::WorkerStatusPayload {
            slug:               worker_slug.to_string(),
            status:             "idle".to_string(),
            previous_status:    None,
            reason:             None,
            cooldown_seconds:   None,
            tick_duration_ms:   None,
            last_outcome:       None,
            timestamp:          chrono::Utc::now().to_rfc3339(),
        });

        let runner     = self.runner.clone();
        let slug_owned = worker_slug.to_string();

        let handle = async_runtime::spawn(async move {
            info!("[RecipeScheduler] Schedule loop started for worker '{}'", slug_owned);

            // Gate the first tick until core-orchestrator is reachable (T5.1/T5.2 — E-04 fix).
            // Polls every 2 s for up to 120 s before proceeding regardless.
            // 120s covers the observed ~106s Python venv cold-boot time.
            if !wait_for_orchestrator_ready(&runner, 120).await {
                warn!(
                    "[RecipeScheduler] Orchestrator not ready after 120s for '{}' — \
                     starting schedule anyway (Mode 1 fallback active)",
                    slug_owned
                );
            }

            // ── IMMEDIATE STARTUP TICK (Fleet Initialization) ─────────────────────
            // Fire one tick immediately after orchestrator is ready, regardless of
            // the cron schedule. This ensures all 7 fleet agents create their YAMLs
            // within seconds of boot, even if their normal schedule is 5 or 10 minutes.
            // Without this, worker-platform and worker-cleanup would wait 5-10 min
            // before first registration, breaking first-time user experience.
            let orch_ready = runner.is_orchestrator_ready().await;
            if orch_ready {
                info!("[RecipeScheduler] Orchestrator ready — firing STARTUP tick for '{}'", slug_owned);
            } else {
                info!("[RecipeScheduler] Orchestrator still starting — firing STARTUP tick for '{}' (Mode 1 fallback)", slug_owned);
            }
            if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await {
                error!("[RecipeScheduler] Startup tick failed for '{}': {e}", slug_owned);
                // Non-fatal: continue to cron loop even if startup tick failed
            }

            // ── MODE 2 CATCH-UP TICK ──────────────────────────────────────────────
            // If the startup tick fired in Mode 1 fallback (orch was not yet ready),
            // wait for the orchestrator and fire a Mode 2 tick once it becomes healthy.
            // This handles the race where some workers (e.g. supervisor, worker-frontend)
            // start before the orchestrator's optimistic health-check grace window expires
            // and get a Mode 1 startup tick while others wait and correctly get Mode 2.
            // Without this catch-up, those workers' agent YAMLs would never be created
            // until their next cron tick (up to 1–10 minutes later).
            if !orch_ready {
                if wait_for_orchestrator_ready(&runner, 90).await {
                    info!(
                        "[RecipeScheduler] Orchestrator now ready — firing Mode 2 catch-up tick for '{}'",
                        slug_owned
                    );
                    if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await {
                        error!(
                            "[RecipeScheduler] Mode 2 catch-up tick failed for '{}': {e}",
                            slug_owned
                        );
                    }
                }
            }

            // ── CRON SCHEDULE LOOP ───────────────────────────────────────────────
            // After startup tick, fall back to the worker's configured schedule.
            //
            // Phase 7.2 — minimum tick floor.
            // When a long-running recipe (e.g. 24 s LivingAgent call) causes the
            // next cron time to already be in the past, (next_fire - now) is
            // negative → .to_std() returns Err → .unwrap_or_default() returned
            // Duration::ZERO (0 ns), creating a tight spin loop.  Evidence: 32
            // "next fire in 0ns" occurrences in the 2026-04-29 run; worst burst:
            // 6 fires in 99 ms for qc-agent (log lines 47862–47914).
            // Fix: enforce a minimum wait of MIN_TICK_SECS regardless of cron slip.
            const MIN_TICK_SECS: u64 = 10;

            for next_fire in schedule.upcoming(Utc) {
                let now      = Utc::now();
                let raw_wait = (next_fire - now).to_std().unwrap_or(Duration::ZERO);
                let wait_dur = raw_wait.max(Duration::from_secs(MIN_TICK_SECS));

                if raw_wait < Duration::from_secs(MIN_TICK_SECS) {
                    warn!(
                        "[RecipeScheduler] Worker '{}' — cron slipped ({:?}), enforcing {}s floor",
                        slug_owned, raw_wait, MIN_TICK_SECS
                    );
                } else {
                    info!("[RecipeScheduler] Worker '{}' — next fire in {:?}", slug_owned, wait_dur);
                }
                tokio::time::sleep(wait_dur).await;

                // Check that worker is still active before firing
                let current_status = WorkersDb::open()
                    .and_then(|db| db.get_by_slug(&slug_owned))
                    .ok()
                    .flatten()
                    .map(|w| w.status);

                match current_status.as_deref() {
                    Some("idle") | Some("active") | Some("error") => {
                        // Fire the recipe
                        info!("[RecipeScheduler] Firing recipe '{}' for worker '{}'", recipe_id, slug_owned);
                        if let Err(e) = runner.run_recipe(&recipe, &slug_owned).await {
                            error!("[RecipeScheduler] Recipe run failed for '{}': {e}", slug_owned);
                            // Phase 7.7 — record the failure in the system_events table so
                            // the System Logs panel in the Dashboard shows live error history.
                            let _ = crate::analytics::global()
                                .collector()
                                .record_system_event(
                                    "scheduler".to_string(),
                                    "recipe_run_failed".to_string(),
                                    format!("Worker '{}' recipe run failed: {e}", slug_owned),
                                    "error".to_string(),
                                    Some(serde_json::json!({
                                        "worker": slug_owned,
                                        "recipe": recipe_id,
                                    }).to_string()),
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
                        warn!("[RecipeScheduler] Worker '{}' still working — skipping tick", slug_owned);
                    }
                    Some("paused") | Some("deleted") | None => {
                        info!("[RecipeScheduler] Worker '{}' is paused/deleted — stopping schedule", slug_owned);
                        break;
                    }
                    _ => {
                        // Unknown status — continue
                        warn!("[RecipeScheduler] Worker '{}' has unexpected status, continuing", slug_owned);
                    }
                }
            }

            info!("[RecipeScheduler] Schedule loop ended for worker '{}'", slug_owned);
        });

        self.handles.lock().unwrap().insert(worker_slug.to_string(), handle);
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
        crate::events::emit_worker_status_global(crate::events::WorkerStatusPayload {
            slug:               worker_slug.to_string(),
            status:             "paused".to_string(),
            previous_status:    None,
            reason:             None,
            cooldown_seconds:   None,
            tick_duration_ms:   None,
            last_outcome:       None,
            timestamp:          chrono::Utc::now().to_rfc3339(),
        });

        info!("[RecipeScheduler] Stopped worker '{}'", worker_slug);
        Ok(())
    }

    /// Stop all running worker schedules (called on app shutdown).
    pub fn stop_all(&self) {
        let mut handles = self.handles.lock().unwrap();
        for (slug, handle) in handles.drain() {
            handle.abort();
            info!("[RecipeScheduler] Aborted schedule for worker '{}'", slug);
        }
    }

    /// Returns the slugs of currently-scheduled workers.
    pub fn active_workers(&self) -> Vec<String> {
        self.handles.lock().unwrap().keys().cloned().collect()
    }

    /// Check if a specific worker has an active schedule.
    pub fn is_running(&self, worker_slug: &str) -> bool {
        self.handles.lock().unwrap().contains_key(worker_slug)
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
        // Gate: skip if worker is already in flight.
        let (worker_opt, recipe_id_opt) = match WorkersDb::open() {
            Ok(db) => match db.get_by_slug(worker_slug) {
                Ok(Some(w)) => {
                    if w.status == "working" {
                        info!("[RecipeScheduler] kick_worker('{}'): skipped (already working)", worker_slug);
                        return;
                    }
                    let rid = w.recipe_id.clone();
                    (Some(w), rid)
                }
                Ok(None) => {
                    warn!("[RecipeScheduler] kick_worker('{}'): worker not found", worker_slug);
                    return;
                }
                Err(e) => {
                    warn!("[RecipeScheduler] kick_worker('{}'): DB error: {e}", worker_slug);
                    return;
                }
            },
            Err(e) => {
                warn!("[RecipeScheduler] kick_worker('{}'): open workers.db failed: {e}", worker_slug);
                return;
            }
        };
        let _ = worker_opt;

        let recipe_id = match recipe_id_opt {
            Some(id) => id,
            None => {
                warn!("[RecipeScheduler] kick_worker('{}'): no bound recipe", worker_slug);
                return;
            }
        };

        let recipe = match load_recipe_by_id(&recipe_id) {
            Ok(Some(r)) => r,
            Ok(None) => {
                warn!("[RecipeScheduler] kick_worker('{}'): recipe '{}' not on disk", worker_slug, recipe_id);
                return;
            }
            Err(e) => {
                warn!("[RecipeScheduler] kick_worker('{}'): load recipe failed: {e}", worker_slug);
                return;
            }
        };

        let runner = self.runner.clone();
        let slug   = worker_slug.to_string();
        async_runtime::spawn(async move {
            info!("[RecipeScheduler] kick_worker: firing '{}' immediately (push-based dispatch)", slug);
            if let Err(e) = runner.run_recipe(&recipe, &slug).await {
                error!("[RecipeScheduler] kick_worker: recipe run failed for '{}': {e}", slug);
            }
        });
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn abort_worker_task(&self, worker_slug: &str) {
        if let Some(handle) = self.handles.lock().unwrap().remove(worker_slug) {
            handle.abort();
            info!("[RecipeScheduler] Aborted previous task for worker '{}'", worker_slug);
        }
    }
}

// ─── Orchestrator readiness gate ────────────────────────────────────────────

/// Poll orchestrator health until ready or timeout (T5.1 — E-04 fix).
/// Returns true if healthy within `timeout_secs`, false if timed out.
async fn wait_for_orchestrator_ready(runner: &super::runner::RecipeRunner, timeout_secs: u64) -> bool {
    let deadline = tokio::time::Instant::now()
        + tokio::time::Duration::from_secs(timeout_secs);
    loop {
        if runner.is_orchestrator_ready().await {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }
}

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

// ─── Cron expression helpers ──────────────────────────────────────────────────

/// Convert standard 5-field POSIX cron to 6-field (prepend seconds = 0).
/// "*/2 * * * *"   → "0 */2 * * * *"   (every 2 minutes)
/// "0 * * * *"     → "0 0 * * * *"     (every hour)
/// "0 */2 * * * *" → unchanged (already 6-field)
fn normalize_to_6field(expr: &str) -> String {
    let fields: Vec<&str> = expr.split_whitespace().collect();
    match fields.len() {
        5 => format!("0 {}", expr),   // prepend seconds = 0
        _ => expr.to_string(),         // 6 or 7 field — use as-is
    }
}
