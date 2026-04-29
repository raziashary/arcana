// RecipeRunner — executes a single recipe as a background agent session.
// Called by RecipeScheduler on each cron tick for autonomous worker agents.
//
// Dispatch priority (DEVELOPMENT_PLAN.md §4.1 — Dual-Mode Architecture):
//   Mode 2: core-orchestrator LivingAgent (21 FSMs, mem0, A2A) — primary
//   Mode 1: Rust AgentEngine (direct LLM)                      — fallback
//
// Mode 2 is used when core-orchestrator responds to /api/system/health.
// Any failure (unreachable, invoke error) transparently falls back to Mode 1.

use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::agent::engine::AgentEngine;
// Phase 3.0 R2 — status push. The `working`/`idle`/`error` transitions emitted
// from `set_worker_status` already encode the full tick lifecycle (start when
// the runner sets `working`, end when it sets `idle` or `error`), so we don't
// need separate tick-start/tick-end events.
use crate::events::{emit_worker_status, WorkerStatusPayload};
use crate::hooks::ecosystem::Ecosystem;
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::agent::PAMode;
use crate::db::workers::WorkersDb;
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::traits::Provider;
use crate::recipes::types::Recipe;
use crate::session::manager::SessionManager;
use crate::sidecar::client::OrchestratorClient;
use crate::state::AppState;
use crate::vault::keychain::{VaultManager, provider_key};

/// Initial trigger message sent to the worker agent at each cron execution.
const RECIPE_TRIGGER: &str =
    "Begin your scheduled execution. Check the task board and proceed with your responsibilities.";

// ─── RecipeRunner ─────────────────────────────────────────────────────────────

/// Executes a recipe as an autonomous agent session.
/// Each call creates a fresh ephemeral session (not stored in session DB).
pub struct RecipeRunner {
    app:          AppHandle,
    state:        Arc<Mutex<AppState>>,
    _session_mgr: Arc<SessionManager>,  // retained for future SSE bridging (not attached to recipe sessions)
    pending:      PendingPermissions,
    orch_client:  Arc<OrchestratorClient>,  // Mode 2: core-orchestrator LivingAgent
}

impl RecipeRunner {
    pub fn new(
        app:         AppHandle,
        state:       Arc<Mutex<AppState>>,
        session_mgr: Arc<SessionManager>,
        pending:     PendingPermissions,
        orch_client: Arc<OrchestratorClient>,
    ) -> Self {
        Self { app, state, _session_mgr: session_mgr, pending, orch_client }
    }

    /// Returns true when the core-orchestrator is reachable and ready.
    /// Used by RecipeScheduler to gate the first tick (T5.1 — E-04 fix).
    pub async fn is_orchestrator_ready(&self) -> bool {
        self.orch_client.is_healthy().await
    }

    /// Execute a recipe for the given worker slug.
    /// Updates worker status in DB: idle → working → idle/error.
    pub async fn run_recipe(&self, recipe: &Recipe, worker_slug: &str) -> Result<()> {
        // ── Mode 2 health check (needed for dispatch routing decision) ──────────
        // We check orchestrator health early because it determines whether we
        // apply the "no tasks" short-circuit below.
        let orch_healthy = self.orch_client.is_healthy().await;

        // ── Deterministic no-op short-circuit ──────────────────────────────────
        //
        // For "worker-*" slugs (backend / frontend / platform / mcp / cleanup),
        // the recipe is meaningful ONLY when there is an actual task on the
        // board assigned to this worker in a runnable state
        // (`assigned` or `in_progress`). Historically the LLM was spun up on
        // every cron tick regardless and — finding an empty board — would
        // hallucinate work, drift through random files, and churn until the
        // hard `MAX_TOOL_TURNS` cap (20). That produced the "18 loops for a
        // simple markdown" behaviour seen in `logs/demo.txt`.
        //
        // CRITICAL FIX: Only apply this short-circuit when running in Mode 1
        // (Rust AgentEngine). When Mode 2 (orchestrator) is healthy, we ALWAYS
        // dispatch through the LivingAgent so that:
        //   1. ensure_agent() is called, creating the YAML config if missing
        //   2. The agent's idle handling ("Board clean — idle.") runs
        //   3. All fleet workers have consistent Mode 2 behavior
        //
        // Without this exception, workers with no tasks never reach
        // ensure_agent(), so their YAMLs are never created and they appear
        // "missing" from the fleet even though they're scheduled.
        if !orch_healthy && worker_slug.starts_with("worker-") {
            if !Self::has_runnable_task(worker_slug) {
                info!(
                    "[RecipeRunner] worker '{}' has no assigned/in_progress tasks — skipping tick (Mode 1)",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "idle");
                return Ok(());
            }
        }

        // ── Model resolution (shared by both modes) ───────────────────────────
        // Priority: worker.model (DB) > recipe.model (YAML) > active_model > default.
        let worker_model: Option<String> = WorkersDb::open().ok()
            .and_then(|db| db.get_by_slug(worker_slug).ok().flatten())
            .map(|w| w.model)
            .filter(|m| !m.is_empty());

        let model = {
            let s = self.state.lock().unwrap();
            worker_model
                .or_else(|| recipe.model.clone())
                .unwrap_or_else(|| s.active_model.clone().unwrap_or_else(|| "claude-sonnet-4-5".into()))
        };

        // Ephemeral session id — used by Mode 1 if reached
        let session_id = format!("recipe-{}-{}", worker_slug, Uuid::new_v4());

        // Working directory for recipe agents.
        //
        // Priority:
        //   1. Configured workspace dir (set via Settings → Workspace Folder).
        //      This is the SAME root the Personal Assistant uses, so the worker
        //      fleet and the PA share a consistent view of the project.
        //   2. Fallback sandbox ~/.autodev/agents/<slug>/ when no workspace is set,
        //      so background ticks can still run on a fresh install without
        //      polluting a random cwd.
        let workspace_dir = crate::commands::workspace_commands::load_workspace_dir();
        let working_dir = if !workspace_dir.trim().is_empty() {
            workspace_dir
        } else {
            crate::db::autodev_dir()
                .map(|p| {
                    let agent_dir = p.join("agents").join(worker_slug);
                    let _ = std::fs::create_dir_all(&agent_dir);
                    agent_dir.to_string_lossy().into_owned()
                })
                .unwrap_or_else(|_| {
                    std::env::var("USERPROFILE")
                        .or_else(|_| std::env::var("HOME"))
                        .unwrap_or_else(|_| ".".to_string())
                })
        };

        // ── Mode 2: core-orchestrator LivingAgent (primary path) ───────────────
        // Use the pre-checked health result from earlier in the function.
        // If reachable, dispatch through LivingAgent for FSMs + mem0 + A2A.
        if orch_healthy {
            info!("[RecipeRunner] core-orchestrator reachable — dispatching '{}' via LivingAgent", worker_slug);
            self.set_worker_status(worker_slug, "working");

            // ── qc-agent pre-flight gate ───────────────────────────────────────────
            // If no qc_review tasks exist in the local DB, skip the expensive
            // core-orchestrator invocation entirely and set status to idle.
            // When tasks DO exist, prepend their titles to the trigger so the
            // agent has them in-context even if its memory claims otherwise.
            let mut qc_trigger_override: Option<String> = None;
            if worker_slug == "qc-agent" {
                if let Ok(db) = crate::db::tasks::TasksDb::open() {
                    if let Ok(tasks) = db.list(Some("qc_review")) {
                        if tasks.is_empty() {
                            info!("[RecipeRunner] No qc_review tasks in DB — skipping qc-agent tick");
                            self.set_worker_status(worker_slug, "idle");
                            return Ok(());
                        } else {
                            let titles: Vec<String> = tasks.iter()
                                .map(|t| format!("#{}: {}", t.id, t.title))
                                .collect();
                            qc_trigger_override = Some(format!(
                                "{RECIPE_TRIGGER}\n\n[BOARD STATE] {} task(s) awaiting QC review:\n{}",
                                tasks.len(),
                                titles.join("\n")
                            ));
                        }
                    }
                }
            }

            match self.orch_client.ensure_agent(worker_slug, &model).await {
                Ok(agent_id) => {
                    // Prepend the active workspace dir to the instructions so the
                    // LivingAgent's file tools resolve paths against the same root
                    // the Personal Assistant uses. Without this, Mode 2 workers
                    // operate on whatever cwd core-orchestrator was launched in
                    // and ignore the user's configured workspace.
                    let instructions_with_ws = format!(
                        "[AUTODEV WORKSPACE]\nworking_dir: {working_dir}\n\
                         All file operations (read_file, write_file, bash, glob, grep) \
                         MUST be resolved relative to this absolute path unless the user \
                         provides another absolute path.\n\n{}",
                        recipe.instructions
                    );
                    info!(
                        "[RecipeRunner] worker '{}' workspace={}",
                        worker_slug, working_dir
                    );
                    let trigger = qc_trigger_override.as_deref().unwrap_or(RECIPE_TRIGGER);
                    match self.orch_client
                        .invoke_worker(&agent_id, &instructions_with_ws, trigger)
                        .await
                    {
                        Ok(response) => {
                            info!(
                                "[RecipeRunner] LivingAgent tick complete for '{}': {:.200}",
                                worker_slug,
                                response.trim()
                            );
                            self.set_worker_status(worker_slug, "idle");
                            // Phase 7.5 — Record successful Mode 2 tick in analytics DB.
                            // Cost/tokens are tracked by core-orchestrator internally;
                            // we record the completion event so the dashboard shows activity.
                            let _ = crate::analytics::global()
                                .collector()
                                .record_worker_stats(
                                    worker_slug.to_string(),
                                    1, // tasks_completed (1 successful tick)
                                    0, // tasks_failed
                                    0, // time_spent_minutes — not tracked in Mode 2
                                    0.0, // cost — tracked by core-orchestrator
                                    0,   // avg_task_duration_minutes
                                );
                            // Same stranded-task safety net as the Mode 1 path —
                            // see the long comment at the reconcile call below.
                            // Without this, a Mode 2 LivingAgent that finishes
                            // its tick without moving the task past `in_progress`
                            // would leave the task stranded forever.
                            // Run reconciliation BEFORE guard_and_validate so a
                            // stranded task is promoted to qc_review instead of
                            // being incorrectly marked build_failed by the sentinel.
                            if worker_slug.starts_with("worker-") {
                                Self::reconcile_stranded_task(worker_slug);
                            }
                            // Fix 8c: sentinel-file guard after successful tick.
                            Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
                            return Ok(());
                        }
                        Err(e) => {
                            warn!(
                                "[RecipeRunner] LivingAgent invoke failed for '{}': {e}; \
                                 falling back to Rust AgentEngine",
                                worker_slug
                            );
                            // Phase 7.7 — record Mode 2 → Mode 1 fallback as a system event.
                            let _ = crate::analytics::global()
                                .collector()
                                .record_system_event(
                                    "runner".to_string(),
                                    "mode2_fallback".to_string(),
                                    format!("Worker '{}' falling back to Mode 1 (invoke error): {e}", worker_slug),
                                    "warning".to_string(),
                                    Some(serde_json::json!({ "worker": worker_slug }).to_string()),
                                );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "[RecipeRunner] ensure_agent failed for '{}': {e}; \
                         falling back to Rust AgentEngine",
                        worker_slug
                    );
                }
            }
        } else {
            info!(
                "[RecipeRunner] core-orchestrator not reachable — \
                 attempting Mode 1 (Rust AgentEngine) fallback for '{}'",
                worker_slug
            );
        }

        // ── Mode 1: Rust AgentEngine (fallback / standalone) ─────────────────
        // Provider + API key are resolved here (only when Mode 2 is unavailable)
        // so that Mode 2-only setups do not require a Rust provider to be configured.
        let provider_id = {
            let s = self.state.lock().unwrap();
            s.active_provider.clone()
        };

        let provider_id = match provider_id {
            Some(id) => id,
            None => {
                error!(
                    "[RecipeRunner] Cannot run recipe for '{}': \
                     core-orchestrator is unreachable AND no LLM provider is configured. \
                     Go to Settings → Provider to add an API key, or wait for the orchestrator to finish starting.",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "error");
                return Err(AutoDevError::Agent(
                    "No LLM provider configured and core-orchestrator is not ready yet. \
                     Go to Settings → Provider to add an API key.".into()
                ));
            }
        };

        let api_key = if provider_id != "ollama" {
            VaultManager::get(&provider_key(&provider_id))?
                .ok_or_else(|| AutoDevError::Agent(
                    format!("No API key stored for provider '{provider_id}' — set it in Settings")
                ))?
        } else {
            String::new()
        };

        let provider: Arc<dyn Provider> = match provider_id.as_str() {
            "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
            "openai"    => Arc::new(OpenAIProvider::new(api_key)),
            "google"    => Arc::new(GoogleProvider::new(api_key)),
            "ollama"    => Arc::new(OllamaProvider::default_from_env()),
            other       => return Err(AutoDevError::Agent(format!("Unsupported provider: {other}"))),
        };

        self.set_worker_status(worker_slug, "working");
        info!("[RecipeRunner] Starting Rust AgentEngine for '{}' (session: {})", worker_slug, session_id);

        // Build autonomous engine — workers never wait for user approval.
        // NOTE: session_mgr is intentionally NOT attached. Recipe sessions are ephemeral
        // (session_id never inserted into sessions table), so append_message would fail
        // with FOREIGN KEY constraint. In-memory context is sufficient for recipe runs.
        let engine = AgentEngine::new(
            provider,
            PermissionMode::Autonomous,
            PAMode::FastDevelopment,
            self.app.clone(),
            self.state.clone(),
            self.pending.clone(),
        );

        // Run agent with recipe instructions as system prompt override
        let result = engine.run(
            &session_id,
            &working_dir,
            &model,
            vec![],
            RECIPE_TRIGGER,
            Some(&recipe.instructions),
        ).await;

        // Update worker status based on outcome
        match &result {
            Ok(_)  => {
                info!("[RecipeRunner] Rust AgentEngine completed for worker '{}'", worker_slug);
                self.set_worker_status(worker_slug, "idle");
                // Phase 7.5 — Record successful Mode 1 tick in analytics DB.
                // Token usage is already recorded inside engine.rs per-session;
                // record the worker-level completion so dashboards can show task counts.
                let _ = crate::analytics::global()
                    .collector()
                    .record_worker_stats(
                        worker_slug.to_string(),
                        1, // tasks_completed
                        0, // tasks_failed
                        0, // time_spent_minutes — not yet timed
                        0.0, // cost — recorded per-token inside engine.rs
                        0,   // avg_task_duration_minutes
                    );
            }
            Err(e) => {
                error!("[RecipeRunner] Rust AgentEngine failed for worker '{}': {e}", worker_slug);
                self.set_worker_status(worker_slug, "error");
                // Phase 7.5 — Record failed Mode 1 tick in analytics DB.
                let _ = crate::analytics::global()
                    .collector()
                    .record_worker_stats(
                        worker_slug.to_string(),
                        0, // tasks_completed
                        1, // tasks_failed
                        0,
                        0.0,
                        0,
                    );
            }
        }

        // ── Post-session reconciliation ────────────────────────────────────────
        //
        // If a session ends (naturally, cancelled, or at MAX_TOOL_TURNS) with a
        // task still in `in_progress` for this worker, that means the worker
        // did NOT reach an explicit terminal state (qc_review / failed / done).
        // Left alone, subsequent ticks re-enter the same in_progress task with
        // a fresh session and the LLM redoes work it already completed —
        // observed at 2026-04-20T13:03:41 where worker-backend wrote the file
        // at turn 5 but dawdled on bash verifications until hitting max_turns.
        //
        // Auto-promote stranded in_progress tasks to `qc_review`. QC will then
        // verify whether the work product exists. If it does → mark done.
        // If not → bounce back to `assigned` for a retry. Either way the task
        // escapes the in_progress limbo and the board keeps moving.
        //
        // Run reconciliation BEFORE the sentinel guard so a stranded task that
        // completed but forgot to update status is promoted to qc_review
        // instead of being incorrectly marked build_failed.
        if worker_slug.starts_with("worker-") {
            Self::reconcile_stranded_task(worker_slug);
        }

        // Fix 8c: sentinel-file guard after successful tick.
        // Only apply when the engine actually completed (not on error).
        if result.is_ok() {
            Self::guard_and_validate(worker_slug, std::path::Path::new(&working_dir));
        }

        result.map(|_| ())
    }

    /// If a task remains `in_progress` for this worker after the session
    /// ends, promote it to `qc_review` so QC can verify work state. See
    /// comment at the call site for motivation.
    fn reconcile_stranded_task(worker_slug: &str) {
        let db = match crate::db::tasks::TasksDb::open() {
            Ok(db) => db,
            Err(_) => return,
        };
        let tasks = match db.list(None) {
            Ok(v) => v,
            Err(_) => return,
        };
        for t in tasks {
            let is_mine = t.assigned_worker.as_deref() == Some(worker_slug);
            let is_in_progress = matches!(t.status, crate::db::tasks::TaskStatus::InProgress);
            if is_mine && is_in_progress {
                match db.update_status(t.id, "qc_review", Some(worker_slug)) {
                    Ok(()) => {
                        info!(
                            "[RecipeRunner] Reconciled stranded in_progress task #{} → qc_review \
                             (worker '{}' session ended without explicit completion)",
                            t.id, worker_slug
                        );
                    }
                    Err(e) => {
                        warn!(
                            "[RecipeRunner] Failed to reconcile stranded task #{}: {e}",
                            t.id
                        );
                    }
                }
            }
        }
    }

    /// Returns true when the local task board has at least one task assigned to
    /// `worker_slug` in a runnable state (`assigned` or `in_progress`).
    ///
    /// Used by `run_recipe` to skip LLM invocation on empty ticks so workers
    /// can't fabricate work when there is literally nothing queued. Reads
    /// `~/.autodev/tasks.db` via `TasksDb`; any DB failure is treated as
    /// "no work" (fail-safe: never spin up the LLM when we can't confirm work
    /// exists).
    fn has_runnable_task(worker_slug: &str) -> bool {
        let db = match crate::db::tasks::TasksDb::open() {
            Ok(db) => db,
            Err(e) => {
                warn!("[RecipeRunner] tasks.db open failed ({e}); treating as no-work");
                return false;
            }
        };
        let all = match db.list(None) {
            Ok(v) => v,
            Err(e) => {
                warn!("[RecipeRunner] tasks.db list failed ({e}); treating as no-work");
                return false;
            }
        };
        all.iter().any(|t| {
            t.assigned_worker.as_deref() == Some(worker_slug)
                && matches!(t.status, crate::db::tasks::TaskStatus::Assigned
                                    | crate::db::tasks::TaskStatus::InProgress)
        })
    }

    fn set_worker_status(&self, slug: &str, status: &str) {
        let previous = if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(slug) {
                let prev = worker.status.clone();
                let _ = db.update_status(worker.id, status);
                Some(prev)
            } else {
                None
            }
        } else {
            None
        };
        // Phase 3.0 R2 — push the single source-of-truth transition to the UI.
        // The Agent Roster subscribes via `workerStore.initEventListeners()`
        // and mutates local state in place, eliminating the derived-status
        // flicker where rows went `idle → error → idle` between ticks.
        // Fix 9a: expanded payload with previous_status, reason, cooldown_seconds.
        let reason: Option<String> = if status == "error" {
            Some("Invoke failed — see logs for details".into())
        } else {
            None
        };
        let cooldown_seconds: Option<u32> = if status == "error" {
            Some(30)
        } else {
            None
        };
        emit_worker_status(&self.app, WorkerStatusPayload {
            slug:               slug.to_string(),
            status:             status.to_string(),
            previous_status:    previous,
            reason,
            cooldown_seconds,
            timestamp:          chrono::Utc::now().to_rfc3339(),
        });
    }

    /// Fix 8c — Sentinel-file guard.
    /// After a worker tick succeeds, verify the generated project has its
    /// dependencies installed by checking the ecosystem sentinel file.
    /// If sentinel is missing, update the active in_progress task to
    /// `build_failed` so QC can inspect it.
    fn guard_and_validate(worker_slug: &str, working_dir: &std::path::Path) {
        use std::fs;
        use crate::db::tasks::{TasksDb, TaskStatus};

        // Only guard for workers (not supervisor, etc.)
        if !worker_slug.starts_with("worker-") {
            return;
        }

        // Find active task for this worker
        let db = match TasksDb::open() {
            Ok(db) => db,
            Err(_) => return,
        };
        let tasks = match db.list(None) {
            Ok(v) => v,
            Err(_) => return,
        };
        let active_task = tasks.into_iter().find(|t| {
            t.assigned_worker.as_deref() == Some(worker_slug)
                && matches!(t.status, TaskStatus::InProgress)
        });
        let Some(task) = active_task else { return };

        // Detect ecosystem
        let Some(ecosystem) = Ecosystem::detect(working_dir) else { return };
        let sentinel: std::path::PathBuf = working_dir.join(ecosystem.sentinel_path());

        if sentinel.exists() {
            info!(
                "[RecipeRunner] Sentinel found for '{}' ({:?}) — build guard passed",
                worker_slug, sentinel
            );
            return;
        }

        // Sentinel missing → mark build_failed
        warn!(
            "[RecipeRunner] Sentinel missing for '{}' ({:?}) — marking task #{} build_failed",
            worker_slug, sentinel, task.id
        );

        // Write a guard log note in the workspace
        let guard_log = working_dir.join("build_guard.log");
        let note = format!(
            "[AutoDev Build Guard] {:?} dependencies not installed.\nExpected sentinel: {:?}\nRun '{}' and '{}' in this directory, then retry.\n",
            ecosystem,
            sentinel,
            ecosystem.install_cmd(),
            ecosystem.validate_cmd(),
        );
        let _ = fs::write(&guard_log, note);

        let _ = db.update_status(task.id, "build_failed", Some(worker_slug));
    }
}
