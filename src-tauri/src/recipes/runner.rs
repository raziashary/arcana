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
use crate::agent::permission::{PendingPermissions, PermissionMode};
use crate::agent::PAMode;
use crate::db::workers::WorkersDb;
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::google::GoogleProvider;
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

        // Working directory for recipe agents: ~/.autodev/agents/<slug>/
        // This MUST NOT be the workspace — recipe agents write ephemeral helper files
        // (Python scripts, logs, markdown) and those must never pollute the project root.
        // Each worker gets its own sandbox directory inside ~/.autodev/agents/.
        let working_dir = crate::db::autodev_dir()
            .map(|p| {
                let agent_dir = p.join("agents").join(worker_slug);
                let _ = std::fs::create_dir_all(&agent_dir);
                agent_dir.to_string_lossy().into_owned()
            })
            .unwrap_or_else(|_| {
                std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_else(|_| ".".to_string())
            });

        // ── Mode 2: core-orchestrator LivingAgent (primary path) ───────────────
        // Check health before every tick — fast (3 s timeout, localhost).
        // If reachable, dispatch through LivingAgent for FSMs + mem0 + A2A.
        if self.orch_client.is_healthy().await {
            info!("[RecipeRunner] core-orchestrator reachable — dispatching '{}' via LivingAgent", worker_slug);
            self.set_worker_status(worker_slug, "working");

            match self.orch_client.ensure_agent(worker_slug, &model).await {
                Ok(agent_id) => {
                    match self.orch_client
                        .invoke_worker(&agent_id, &recipe.instructions, RECIPE_TRIGGER)
                        .await
                    {
                        Ok(response) => {
                            info!(
                                "[RecipeRunner] LivingAgent tick complete for '{}': {:.200}",
                                worker_slug,
                                response.trim()
                            );
                            self.set_worker_status(worker_slug, "idle");
                            return Ok(());
                        }
                        Err(e) => {
                            warn!(
                                "[RecipeRunner] LivingAgent invoke failed for '{}': {e}; \
                                 falling back to Rust AgentEngine",
                                worker_slug
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
                 using Rust AgentEngine for '{}'",
                worker_slug
            );
        }

        // ── Mode 1: Rust AgentEngine (fallback / standalone) ─────────────────
        // Provider + API key are resolved here (only when Mode 2 is unavailable)
        // so that Mode 2-only setups do not require a Rust provider to be configured.
        let provider_id = {
            let s = self.state.lock().unwrap();
            s.active_provider.clone()
                .ok_or_else(|| AutoDevError::Agent(
                    "No active provider set and core-orchestrator is unreachable. \
                     Configure a provider in Settings or start core-orchestrator.".into()
                ))?
        };

        let api_key = VaultManager::get(&provider_key(&provider_id))?
            .ok_or_else(|| AutoDevError::Agent(
                format!("No API key stored for provider '{provider_id}' — set it in Settings")
            ))?;

        let provider: Arc<dyn Provider> = match provider_id.as_str() {
            "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
            "openai"    => Arc::new(OpenAIProvider::new(api_key)),
            "google"    => Arc::new(GoogleProvider::new(api_key)),
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
            }
            Err(e) => {
                error!("[RecipeRunner] Rust AgentEngine failed for worker '{}': {e}", worker_slug);
                self.set_worker_status(worker_slug, "error");
            }
        }

        result.map(|_| ())
    }

    fn set_worker_status(&self, slug: &str, status: &str) {
        if let Ok(db) = WorkersDb::open() {
            if let Ok(Some(worker)) = db.get_by_slug(slug) {
                let _ = db.update_status(worker.id, status);
            }
        }
    }
}
