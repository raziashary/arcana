// RecipeRunner — executes a single recipe as a background agent session.
// Called by RecipeScheduler on each cron tick for autonomous worker agents.
//
// Dispatch priority (DEVELOPMENT_PLAN.md §4.1 — Dual-Mode Architecture):
//   Mode 2: core-orchestrator LivingAgent (21 FSMs, mem0, A2A) — PRIMARY
//   Mode 1: Rust AgentEngine (direct LLM)                      — LAST-RESORT ONLY
//
// Mode 2 is THE main mode for fleet workers. Mode 1 is only entered when
// core-orchestrator is confirmed genuinely unreachable (is_healthy() → false).
//
// Transient Mode 2 failures (MCP server blip, ensure_agent error, invoke error)
// do NOT fall back to Mode 1. They abort the current tick and let the next
// cron tick retry Mode 2. Reasons:
//   1. (Historical: this reasoning concerned the in-process MCP server on
//      :8767, deleted in P0. Mode 2 is inert now — ADR-LOG-RESTRUCT-0008.)
//   2. ensure_agent failures are typically auth-refresh races — next tick recovers.
//   3. invoke_worker failures mean the LivingAgent may have already started the
//      task; running Mode 1 on top risks double-execution and state corruption.
//
// The only legitimate Mode 1 path is `orch_healthy == false` at tick entry,
// which means the orchestrator process genuinely did not respond to the
// /api/system/health check after the grace-window retry policy in is_healthy().

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::agent::engine::AgentEngine;
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::agent::PAMode;
use crate::db::workers::WorkersDb;
use crate::errors::{AutoDevError, Result};
use crate::events::{
    emit_worker_log_line, emit_worker_status, emit_worker_tick_end, emit_worker_tick_start,
    WorkerLogLinePayload, WorkerStatusPayload, WorkerTickEndPayload, WorkerTickStartPayload,
};
use crate::hooks::ecosystem::Ecosystem;
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::recipes::types::Recipe;
use crate::session::manager::SessionManager;
use crate::sidecar::client::OrchestratorClient;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

/// Initial trigger message sent to the worker agent at each cron execution.
const RECIPE_TRIGGER: &str =
    "Begin your scheduled execution. Check the task board and proceed with your responsibilities.";

// ─── RecipeRunner ─────────────────────────────────────────────────────────────

/// Executes a recipe as an autonomous agent session.
/// Each call creates a fresh ephemeral session (not stored in session DB).
pub struct RecipeRunner {
    app: AppHandle,
    state: Arc<Mutex<AppState>>,
    _session_mgr: Arc<SessionManager>, // retained for future SSE bridging (not attached to recipe sessions)
    pending: PendingPermissions,
    orch_client: Arc<OrchestratorClient>, // Mode 2: core-orchestrator LivingAgent
}

impl RecipeRunner {
    pub fn new(
        app: AppHandle,
        state: Arc<Mutex<AppState>>,
        session_mgr: Arc<SessionManager>,
        pending: PendingPermissions,
        orch_client: Arc<OrchestratorClient>,
    ) -> Self {
        Self {
            app,
            state,
            _session_mgr: session_mgr,
            pending,
            orch_client,
        }
    }

    /// Returns true when the core-orchestrator is reachable and ready.
    /// Used by RecipeScheduler to gate the first tick (T5.1 — E-04 fix).
    pub async fn is_orchestrator_ready(&self) -> bool {
        self.orch_client.is_healthy().await
    }

    /// Execute a recipe for the given worker slug.
    /// Updates worker status in DB: idle → working → idle/error.
    pub async fn run_recipe(&self, recipe: &Recipe, worker_slug: &str) -> Result<()> {
        // ── Fleet-ready latch ───────────────────────────────────────────────────
        // Hold the very first tick until core-orchestrator has been
        // confirmed-healthy at least once. Without this, every worker fires
        // its startup tick during the grace window (when `is_healthy()` is
        // optimistically true) and burns 7 × `ensure_agent failed:
        // service-login` warnings in the first 10s of every cold start.
        // Once the latch trips, all subsequent ticks proceed with the
        // normal grace-window health logic — so a single transient orch
        // blip mid-session never blocks dispatch.
        if !self.orch_client.has_ever_been_healthy() {
            debug!(
                "[RecipeRunner] holding tick for '{}' — orch not yet confirmed healthy (fleet-ready latch closed)",
                worker_slug
            );
            self.set_worker_status(worker_slug, "idle");
            return Ok(());
        }

        // ── Mode 2 health check (needed for dispatch routing decision) ──────────
        // We check orchestrator health early because it determines whether we
        // apply the "no tasks" short-circuit below.
        // Mode 2 is INERT from P0.
        //
        // A Mode 2 worker is a core-orchestrator LivingAgent that reaches
        // AutoDev's tools by calling back into the in-process MCP server. That
        // server was deleted in P0 (unauthenticated local RCE exposing bash,
        // write_file and spawn_subagent — ADR-03-02), so a Mode 2 worker would
        // now run with ZERO tools and burn tokens achieving nothing. The
        // pre-flight below already treated that state as "abort the tick".
        //
        // Every worker therefore runs on the in-process Rust engine (Mode 1),
        // which needs no sidecar and no Python. The Mode 2 branch is left in
        // place, unreachable, and is deleted with the rest of the sidecar in
        // P8; deleting it here would mean unpicking ~350 lines of nested
        // control flow in the same commit that removes the MCP server.
        //
        // ADR-LOG-RESTRUCT-0008.
        let _orch_reachable = self.orch_client.is_healthy().await;
        let orch_healthy = false;
        info!(
            "[RecipeRunner] tick dispatch — worker='{}' core-orchestrator={}",
            worker_slug,
            if orch_healthy {
                "reachable → Mode 2 (LivingAgent)"
            } else {
                "unreachable → Mode 1 (Rust Engine)"
            },
        );

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
        let worker_model: Option<String> = WorkersDb::open()
            .ok()
            .and_then(|db| db.get_by_slug(worker_slug).ok().flatten())
            .map(|w| w.model)
            .filter(|m| !m.is_empty());

        let model = {
            let s = self.state.lock().unwrap();
            worker_model
                .or_else(|| recipe.model.clone())
                .unwrap_or_else(|| {
                    s.active_model
                        .clone()
                        .unwrap_or_else(|| "claude-sonnet-4-5".into())
                })
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
            info!(
                "[RecipeRunner] core-orchestrator reachable — dispatching '{}' via LivingAgent",
                worker_slug
            );
            self.set_worker_status(worker_slug, "working");

            // D.1 — Record tick start and notify the frontend. Both the start
            // timestamp and the Instant are captured here so we can compute
            // tick_duration_ms for the WorkerStatusPayload on completion.
            let tick_start = Instant::now();
            emit_worker_tick_start(
                &self.app,
                WorkerTickStartPayload {
                    slug: worker_slug.to_string(),
                    mode: "mode2".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );

            match self.orch_client.ensure_agent(worker_slug, &model).await {
                Ok(agent_id) => {
                    // BUG-002b — pre-flight: if the MCP server is unreachable, the
                    // LivingAgent will have zero tools and burn tokens doing nothing.
                    // Force an immediate Mode 1 fallback rather than discovering this
                    // inside the expensive invoke_worker call.
                    let mcp_up = false; // see ADR-LOG-RESTRUCT-0008 — the MCP server is gone
                                        // ── qc-agent gate (AFTER ensure_agent so YAML is always created) ─────────
                                        // ensure_agent() above already created/registered the YAML config.
                                        // Now skip the LLM invocation if there are no reviewable tasks.
                    let mut qc_trigger_override: Option<String> = None;
                    if worker_slug == "qc-agent" {
                        if let Ok(db) = crate::db::tasks::TasksDb::open() {
                            if let Ok(tasks) = db.list(Some("qc_review")) {
                                if tasks.is_empty() {
                                    info!("[RecipeRunner] No qc_review tasks in DB — skipping qc-agent tick");
                                    // Doc 06 Phase 3 — leave a visible breadcrumb on the QC card so
                                    // "intentionally idle" doesn't look like "stuck idle".
                                    emit_worker_log_line(
                                        &self.app,
                                        WorkerLogLinePayload {
                                            slug: worker_slug.to_string(),
                                            text: "idle — no qc_review tasks".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    emit_worker_tick_end(
                                        &self.app,
                                        WorkerTickEndPayload {
                                            slug: worker_slug.to_string(),
                                            outcome: "idle".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    self.set_worker_status(worker_slug, "idle");
                                    return Ok(());
                                }
                                // E.3 — Filter out build_failed tasks that leaked into qc_review.
                                let reviewable: Vec<_> = tasks
                                    .iter()
                                    .filter(|t| {
                                        !matches!(
                                            t.status,
                                            crate::db::tasks::TaskStatus::BuildFailed
                                        )
                                    })
                                    .collect();
                                if reviewable.is_empty() {
                                    info!(
                                        "[RecipeRunner] Only build_failed tasks in qc_review — \
                                         skipping qc-agent LLM invocation"
                                    );
                                    // Doc 06 Phase 3 — distinct breadcrumb from the
                                    // "no qc_review tasks" case so the user can tell
                                    // why the QC agent is parked.
                                    emit_worker_log_line(
                                        &self.app,
                                        WorkerLogLinePayload {
                                            slug: worker_slug.to_string(),
                                            text: "idle — only build_failed tasks awaiting QC"
                                                .to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    emit_worker_tick_end(
                                        &self.app,
                                        WorkerTickEndPayload {
                                            slug: worker_slug.to_string(),
                                            outcome: "idle".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    self.set_worker_status(worker_slug, "idle");
                                    return Ok(());
                                }
                                let titles: Vec<String> = reviewable
                                    .iter()
                                    .map(|t| format!("#{}: {}", t.id, t.title))
                                    .collect();
                                qc_trigger_override = Some(format!(
                                    "{RECIPE_TRIGGER}\n\n[BOARD STATE] {} task(s) awaiting QC review:\n{}",
                                    reviewable.len(),
                                    titles.join("\n")
                                ));
                            }
                        }
                    }

                    // I.1 — Pre-flight task count check for worker-* agents (Mode 2).
                    // ensure_agent() above already created the YAML config (the main goal).
                    // Now skip the expensive LLM invocation if there are no actionable tasks.
                    // This must happen AFTER ensure_agent so agent YAMLs are always created.
                    if worker_slug.starts_with("worker-") {
                        let has_work = crate::db::tasks::TasksDb::open().ok().map_or(true, |db| {
                            let assigned = db
                                .list_tasks_by_status_and_worker("assigned", worker_slug)
                                .unwrap_or_default();
                            let in_prog = db
                                .list_tasks_by_status_and_worker("in_progress", worker_slug)
                                .unwrap_or_default();
                            let rejected = db
                                .list_tasks_by_status_and_worker("rejected", worker_slug)
                                .unwrap_or_default();
                            !assigned.is_empty() || !in_prog.is_empty() || !rejected.is_empty()
                        });
                        if !has_work {
                            info!(
                                "[RecipeRunner] {} has no actionable tasks — skipping LLM invocation (Mode 2)",
                                worker_slug
                            );
                            emit_worker_tick_end(
                                &self.app,
                                WorkerTickEndPayload {
                                    slug: worker_slug.to_string(),
                                    outcome: "idle".to_string(),
                                    timestamp: chrono::Utc::now().to_rfc3339(),
                                },
                            );
                            self.set_worker_status(worker_slug, "idle");
                            return Ok(());
                        }
                    }

                    if !mcp_up {
                        // The AutoDev MCP server is an in-process Tokio task in the same
                        // Tauri binary. If the port isn't responding, the server has
                        // either crashed or hasn't finished binding yet. Either way, Mode 1
                        // would run without ANY tools and is strictly worse. Abort this tick
                        // and wait for the next cron cycle — the in-process server will
                        // almost certainly recover on its own.
                        let mcp_port = 0;
                        warn!(
                            "[RecipeRunner] MCP server unreachable for '{}' (port :{} not responding). \
                             Aborting tick — will retry next scheduled interval. \
                             NOT falling back to Mode 1 (tool-less Mode 1 is worse than skipping).",
                            worker_slug, mcp_port
                        );
                        emit_worker_tick_end(
                            &self.app,
                            WorkerTickEndPayload {
                                slug: worker_slug.to_string(),
                                outcome: "mcp_unavailable".to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                            },
                        );
                        self.set_worker_status(worker_slug, "idle");
                        return Ok(());
                    }

                    info!(
                        "[RecipeRunner] MCP server healthy for '{}' — \
                         LivingAgent will have tools via :{}, proceeding Mode 2",
                        worker_slug, 0
                    );
                    {
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
                        // Doc 01 Phase 3+ / Doc 06 Phase 3 follow-up: use the
                        // streaming variant so per-tool SSE frames from the
                        // LivingAgent re-emit on subagent:event keyed by the
                        // worker slug. The Live Monitor card's body now
                        // shows the actual tools being called instead of
                        // staying empty during the multi-minute tick.
                        match self
                            .orch_client
                            .invoke_worker_streaming(
                                &agent_id,
                                &instructions_with_ws,
                                trigger,
                                worker_slug,
                            )
                            .await
                        {
                            Ok(result) => {
                                let elapsed_ms = tick_start.elapsed().as_millis() as u64;
                                let response = result.content.clone();
                                info!(
                                "[RecipeRunner] LivingAgent tick complete for '{}' in {}ms: {:.200}",
                                worker_slug,
                                elapsed_ms,
                                response.trim()
                            );

                                // Phase 8.2 — Mode 2 cost tracking.
                                // core-orchestrator returns approximate tokens in
                                // response.metadata.tokens_used; the client falls
                                // back to a char/4 estimate when that field is
                                // unpopulated. Either way we now have non-zero
                                // numbers to bill against the worker's model.
                                let tokens_in = result.tokens_in;
                                let tokens_out = result.tokens_out;
                                let cost = crate::cost::tracker::CostTracker::calculate(
                                    &model, tokens_in, tokens_out,
                                );
                                // Roll into the in-process AppState so the
                                // StatusBar / Sidebar / TabBar cost meter (which
                                // reads `cost_today_usd`) reflects Mode 2 spend
                                // alongside Mode 1 PA spend.
                                {
                                    let mut s = self.state.lock().unwrap();
                                    s.add_cost(cost.cost_usd, tokens_in, tokens_out);
                                }
                                // Persist into the analytics DB so the Dashboard's
                                // get_dashboard_summary picks it up across restarts.
                                let analytics_provider = if model.contains("claude") {
                                    "anthropic"
                                } else if model.contains("gpt")
                                    || model.starts_with("o1")
                                    || model.starts_with("o3")
                                {
                                    "openai"
                                } else if model.contains("gemini") {
                                    "google"
                                } else {
                                    "unknown"
                                };
                                // Phase 1 of docs/endtoendrun/14may2026/02 — fail
                                // loudly. The previous `let _ = ...` silently
                                // swallowed every analytics write for the entire
                                // session and was the reason the Dashboard
                                // showed zeros for days.
                                if let Err(e) =
                                    crate::analytics::global().collector().record_token_usage(
                                        format!(
                                            "mode2-{}-{}",
                                            worker_slug,
                                            chrono::Utc::now().timestamp_millis()
                                        ),
                                        "mode2_worker".to_string(),
                                        analytics_provider.to_string(),
                                        model.clone(),
                                        tokens_in as i32,
                                        tokens_out as i32,
                                        None,
                                        cost.cost_usd,
                                        format!("worker_tick:{}", worker_slug),
                                    )
                                {
                                    warn!("[Mode2] record_token_usage failed for '{worker_slug}': {e}");
                                }
                                // Live-broadcast a cost:update so the Dashboard
                                // and any other cost listeners refresh immediately
                                // rather than waiting for the 60s polling fallback.
                                let total_today = self.state.lock().unwrap().cost_today_usd;
                                crate::events::emit_cost_update(
                                    &self.app,
                                    crate::events::CostUpdatePayload {
                                        session_id: format!("mode2:{}", worker_slug),
                                        tokens_in,
                                        tokens_out,
                                        cost_usd: cost.cost_usd,
                                        total_today,
                                    },
                                );
                                // D.1 — emit tick-end before transitioning to idle
                                emit_worker_tick_end(
                                    &self.app,
                                    WorkerTickEndPayload {
                                        slug: worker_slug.to_string(),
                                        outcome: "success".to_string(),
                                        timestamp: chrono::Utc::now().to_rfc3339(),
                                    },
                                );
                                // D.2 — include timing in the idle status event
                                self.set_worker_status_ex(
                                    worker_slug,
                                    "idle",
                                    Some(elapsed_ms),
                                    Some("success".to_string()),
                                );
                                // Phase 7.5 — Record successful Mode 2 tick in analytics DB.
                                //
                                // Doc 02 Phase 2 task 2.2: populate with real values so the
                                // worker leaderboard widget shows actual per-worker spend
                                // and timing instead of zeros. `cost.cost_usd` and
                                // `elapsed_ms` are already computed above; we just had
                                // to stop hard-coding 0.0 / 0 here.
                                //
                                // `time_spent_minutes` uses ceiling division by 60_000 so a
                                // sub-minute tick records 1, not 0 (zero-min ticks would
                                // make the leaderboard's "avg duration" stat misleading).
                                let time_spent_min = ((elapsed_ms + 59_999) / 60_000) as i32;
                                if let Err(e) =
                                    crate::analytics::global().collector().record_worker_stats(
                                        worker_slug.to_string(),
                                        1,              // tasks_completed (1 successful tick)
                                        0,              // tasks_failed
                                        time_spent_min, // time_spent_minutes (real)
                                        cost.cost_usd,  // cost (real, from CostTracker above)
                                        time_spent_min, // avg_task_duration_minutes
                                    )
                                {
                                    warn!("[Mode2] record_worker_stats failed for '{worker_slug}': {e}");
                                }
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
                                Self::guard_and_validate(
                                    worker_slug,
                                    std::path::Path::new(&working_dir),
                                );
                                // C.4 — Notify the frontend dashboard that a Mode 2 tick
                                // completed so it can refresh metrics without waiting for
                                // the 60-second polling fallback.
                                if let Some(app) = crate::events::app_handle() {
                                    let _ = app.emit(
                                        "task:changed",
                                        serde_json::json!({
                                            "task_id":        null,
                                            "kind":           "worker_tick_complete",
                                            "status":         null,
                                            "assigned_worker": worker_slug,
                                        }),
                                    );
                                }
                                return Ok(());
                            }
                            Err(e) => {
                                // invoke_worker failed — the LivingAgent may have partially
                                // executed this task (it could be in_progress). Do NOT fall
                                // back to Mode 1: that risks double-execution and leaves the
                                // task in an inconsistent state (Mode 1 and Mode 2 wrote
                                // different things to the same files). Instead:
                                //   1. Reconcile any stranded in_progress task → qc_review
                                //   2. Mark worker error so the UI signals the failure
                                //   3. Return Err so the scheduler logs it; next cron tick
                                //      will retry via Mode 2.
                                warn!(
                                "[RecipeRunner] LivingAgent invoke failed for '{}': {e}. \
                                 Aborting tick (NOT falling back to Mode 1 — risk of double-execution). \
                                 Next scheduled tick will retry Mode 2.",
                                worker_slug
                            );
                                emit_worker_tick_end(
                                    &self.app,
                                    WorkerTickEndPayload {
                                        slug: worker_slug.to_string(),
                                        outcome: "invoke_error".to_string(),
                                        timestamp: chrono::Utc::now().to_rfc3339(),
                                    },
                                );
                                // Reconcile any task the LivingAgent left stranded in_progress.
                                if worker_slug.starts_with("worker-") {
                                    Self::reconcile_stranded_task(worker_slug);
                                }
                                // Phase 7.7 — record the invoke error as a system event.
                                let _ = crate::analytics::global().collector().record_system_event(
                                    "runner".to_string(),
                                    "mode2_invoke_error".to_string(),
                                    format!(
                                        "Worker '{}' Mode 2 invoke failed (tick aborted): {e}",
                                        worker_slug
                                    ),
                                    "warning".to_string(),
                                    Some(serde_json::json!({ "worker": worker_slug }).to_string()),
                                );
                                self.set_worker_status(worker_slug, "error");
                                return Err(AutoDevError::Agent(format!(
                                    "LivingAgent invoke failed for '{worker_slug}': {e}"
                                )));
                            }
                        } // closes invoke_worker match body
                    } // closes MCP-healthy block
                } // closes Ok(agent_id) arm
                Err(e) => {
                    // ensure_agent failed — typically a transient condition:
                    // auth token refresh race, orchestrator still warming up,
                    // or a brief network hiccup. The orchestrator passed is_healthy()
                    // earlier in this function, so it IS reachable — this is not
                    // the "genuinely unreachable" case that justifies Mode 1.
                    // Abort the tick; the next cron cycle will retry Mode 2.
                    warn!(
                        "[RecipeRunner] ensure_agent failed for '{}': {e}. \
                         Aborting tick (NOT falling back to Mode 1 — orchestrator was healthy). \
                         Next scheduled tick will retry Mode 2.",
                        worker_slug
                    );
                    emit_worker_tick_end(
                        &self.app,
                        WorkerTickEndPayload {
                            slug: worker_slug.to_string(),
                            outcome: "ensure_agent_error".to_string(),
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        },
                    );
                    self.set_worker_status(worker_slug, "idle");
                    return Ok(());
                }
            }
        } else {
            info!(
                "[RecipeRunner] core-orchestrator not reachable — \
                 attempting Mode 1 (Rust AgentEngine) fallback for '{}'",
                worker_slug
            );
        }

        // Doc 05 Phase 3 — honour the recipe's fallback policy. Mode2Only
        // recipes refuse to fall through to Mode 1 even if a provider is
        // configured; Mode1Capable recipes proceed to the AgentEngine path
        // below. This is enforced AFTER the Mode 2 attempt so a transient
        // orch hiccup doesn't downgrade a Mode2Only recipe to a buggy
        // Mode 1 run.
        if matches!(
            recipe.fallback_policy,
            crate::recipes::types::FallbackPolicy::Mode2Only
        ) {
            info!(
                "[RecipeRunner] Worker '{}' Mode 1 fallback skipped per policy \
                 (recipe.fallback_policy = mode2_only) — staying idle until \
                 core-orchestrator is reachable.",
                worker_slug
            );
            emit_worker_tick_end(
                &self.app,
                WorkerTickEndPayload {
                    slug: worker_slug.to_string(),
                    outcome: "fallback_skipped_policy".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                },
            );
            // Doc 05 Phase 2 — distinct status "fallback_idle" so the Live
            // Monitor can render a different visual state for "fallback
            // engaged but skipped per policy" vs ordinary idle.
            self.set_worker_status(worker_slug, "fallback_idle");
            return Ok(());
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
                // No local LLM provider configured — Mode 1 is not possible.
                // Silently stay idle; Mode 2 (core-orchestrator) will handle
                // this recipe once the orchestrator finishes starting.
                // Do NOT set status to "error" here — this is a transient
                // startup condition, not a permanent failure.
                //
                // Doc 05 Phase 2 — distinct log + status for "Mode 1 idle (no
                // provider)" vs ordinary idle so log scans + Live Monitor can
                // both tell which workers are actually capable of fallback work.
                info!(
                    mode = "mode1",
                    mode1_capable = false,
                    "[RecipeRunner] Worker '{}' Mode 1 idle: no LLM provider configured. \
                     Waiting for core-orchestrator (Mode 2).",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "mode1_idle_no_provider");
                return Ok(());
            }
        };
        // Doc 05 Phase 2 — log the positive case too so a `grep mode1_capable`
        // across the run shows which workers actually ran on the Rust engine.
        info!(
            mode = "mode1",
            mode1_capable = true,
            provider = provider_id.as_str(),
            "[RecipeRunner] Worker '{}' Mode 1 ACTIVE (provider='{}')",
            worker_slug,
            provider_id,
        );

        let api_key = if provider_id != "ollama" {
            VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
                AutoDevError::Agent(format!(
                    "No API key stored for provider '{provider_id}' — set it in Settings"
                ))
            })?
        } else {
            String::new()
        };

        let provider: Arc<dyn Provider> = match provider_id.as_str() {
            "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
            "openai" => Arc::new(OpenAIProvider::new(api_key)),
            "google" => Arc::new(GoogleProvider::new(api_key)),
            "ollama" => Arc::new(OllamaProvider::default_from_env()),
            other => {
                return Err(AutoDevError::Agent(format!(
                    "Unsupported provider: {other}"
                )))
            }
        };

        self.set_worker_status(worker_slug, "working");
        info!(
            "[RecipeRunner] Starting Rust AgentEngine for '{}' (session: {})",
            worker_slug, session_id
        );

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
            // The worker's own slug is its identity. This is what the three
            // worker-messaging tools, claim_task's orphan detection and the
            // qc_review gate all read — previously it was None on this path.
            crate::agent::tool_trait::AgentIdentity::worker(worker_slug),
        );

        // Run agent with recipe instructions as system prompt override
        let result = engine
            .run(
                &session_id,
                &working_dir,
                &model,
                vec![],
                RECIPE_TRIGGER,
                Some(&recipe.instructions),
            )
            .await;

        // Update worker status based on outcome
        match &result {
            Ok(_) => {
                info!(
                    "[RecipeRunner] Rust AgentEngine completed for worker '{}'",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "idle");
                // Phase 7.5 — Record successful Mode 1 tick in analytics DB.
                // Token usage is already recorded inside engine.rs per-session;
                // record the worker-level completion so dashboards can show task counts.
                let _ = crate::analytics::global().collector().record_worker_stats(
                    worker_slug.to_string(),
                    1,   // tasks_completed
                    0,   // tasks_failed
                    0,   // time_spent_minutes — not yet timed
                    0.0, // cost — recorded per-token inside engine.rs
                    0,   // avg_task_duration_minutes
                );
            }
            Err(e) => {
                error!(
                    "[RecipeRunner] Rust AgentEngine failed for worker '{}': {e}",
                    worker_slug
                );
                self.set_worker_status(worker_slug, "error");
                // Phase 7.5 — Record failed Mode 1 tick in analytics DB.
                let _ = crate::analytics::global().collector().record_worker_stats(
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
                && matches!(
                    t.status,
                    crate::db::tasks::TaskStatus::Assigned
                        | crate::db::tasks::TaskStatus::InProgress
                )
        })
    }

    fn set_worker_status(&self, slug: &str, status: &str) {
        self.set_worker_status_ex(slug, status, None, None);
    }

    fn set_worker_status_ex(
        &self,
        slug: &str,
        status: &str,
        tick_duration_ms: Option<u64>,
        last_outcome: Option<String>,
    ) {
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
        let cooldown_seconds: Option<u32> = if status == "error" { Some(30) } else { None };
        emit_worker_status(
            &self.app,
            WorkerStatusPayload {
                slug: slug.to_string(),
                status: status.to_string(),
                previous_status: previous,
                reason,
                cooldown_seconds,
                timestamp: chrono::Utc::now().to_rfc3339(),
                tick_duration_ms,
                last_outcome,
            },
        );
    }

    /// Fix 8c — Sentinel-file guard.
    /// After a worker tick succeeds, verify the generated project has its
    /// dependencies installed by checking the ecosystem sentinel file.
    /// If sentinel is missing, update the active in_progress task to
    /// `build_failed` so QC can inspect it.
    fn guard_and_validate(worker_slug: &str, working_dir: &std::path::Path) {
        use crate::db::tasks::{TaskStatus, TasksDb};
        use std::fs;

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
        let Some(ecosystem) = Ecosystem::detect(working_dir) else {
            return;
        };
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
