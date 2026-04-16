// RecipeScheduler — manages cron-based recipe execution for the Worker Fleet.
// Each worker with a bound recipe can be started/stopped independently.
// Uses tokio tasks + the `cron` crate for accurate next-fire-time calculation.
// See: DEVELOPMENT_PLAN.md Phase 2 — Cron Scheduler

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use cron::Schedule;
use tauri::async_runtime;
use async_runtime::JoinHandle;
use tracing::{error, info, warn};

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

        let runner     = self.runner.clone();
        let slug_owned = worker_slug.to_string();

        let handle = async_runtime::spawn(async move {
            info!("[RecipeScheduler] Schedule loop started for worker '{}'", slug_owned);

            // Gate the first tick until core-orchestrator is reachable (T5.1/T5.2 — E-04 fix).
            // Polls every 2 s for up to 45 s before proceeding regardless.
            if !wait_for_orchestrator_ready(&runner, 45).await {
                warn!(
                    "[RecipeScheduler] Orchestrator not ready after 45s for '{}' — \
                     starting schedule anyway (Mode 1 fallback active)",
                    slug_owned
                );
            }

            for next_fire in schedule.upcoming(Utc) {
                let now      = Utc::now();
                let wait_dur = (next_fire - now).to_std().unwrap_or_default();

                info!("[RecipeScheduler] Worker '{}' — next fire in {:?}", slug_owned, wait_dur);
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
                            // Worker status already set to "error" by runner — keep scheduling
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
