// Worker management commands — CRUD for ~/.autodev/workers.db
// Phase 2.2: CRUD + creation/deletion.
// Phase 2:   start/stop now wired to the real RecipeScheduler (cron engine).
// See: DEVELOPMENT_PLAN.md Phase 2 — Recipe Execution Engine

use crate::db::workers::{slugify, Worker, WorkersDb};
use crate::errors::{AutoDevError, Result};
use crate::recipes::scheduler::RecipeScheduler;
use crate::sidecar::client::OrchestratorClient;
use crate::sidecar::resolve_orchestrator_path;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tauri::State;

// ─── Input / Output types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateWorkerInput {
    pub name: String,
    pub worker_type: String,
    pub model: String,
    pub primary_repos: Vec<String>,
    pub polling_interval: Option<i64>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateWorkerOutput {
    pub id: i64,
    pub slug: String,
}

/// Result of updating a worker's model with immediate sync to core-orchestrator.
#[derive(Debug, Serialize)]
pub struct UpdateModelSyncResult {
    /// Database update succeeded
    pub db_updated: bool,
    /// Orchestrator sync status: "synced", "pending", or "failed"
    pub sync_status: String,
    /// Human-readable message for toast
    pub message: String,
    /// The model that was requested
    pub model: String,
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_workers(orch_client: State<'_, Arc<OrchestratorClient>>) -> Result<Vec<Worker>> {
    let mut workers = WorkersDb::open()?.list()?;

    // Source 1 (preferred): query the live core-orchestrator over HTTP. This is
    // the most reliable source — it reflects any PUT /config writes immediately
    // and bypasses all file-path resolution.
    if orch_client.is_healthy().await {
        let api_models = orch_client.fetch_agent_models().await;
        if !api_models.is_empty() {
            for worker in &mut workers {
                if let Some(model) = api_models.get(&worker.slug) {
                    worker.model = model.clone();
                }
            }
            return Ok(workers);
        }
    }

    // Source 2 (fallback): read llm.model directly from the agent YAML files.
    // Used when the orchestrator is offline (e.g. Demo Mode, or first launch
    // before the sidecar is up).
    if let Ok(orch_dir) = resolve_orchestrator_path(None) {
        let agents_dir = orch_dir.join("config").join("agents");
        for worker in &mut workers {
            if let Some(model) = read_yaml_model(&agents_dir, &worker.slug) {
                worker.model = model;
            }
        }
    }
    Ok(workers)
}

#[tauri::command]
pub fn get_worker_by_slug(slug: String) -> Result<Option<Worker>> {
    WorkersDb::open()?.get_by_slug(&slug)
}

#[tauri::command]
pub fn create_worker(input: CreateWorkerInput) -> Result<CreateWorkerOutput> {
    let db = WorkersDb::open()?;
    let slug = slugify(&input.name);

    if db.slug_exists(&slug)? {
        return Err(crate::errors::AutoDevError::Agent(format!(
            "Worker slug '{}' already exists",
            slug
        )));
    }

    let interval = input.polling_interval.unwrap_or(120);
    let id = db.create(
        &input.name,
        &slug,
        &input.worker_type,
        &input.model,
        &input.primary_repos,
        interval,
        input.description.as_deref(),
    )?;

    Ok(CreateWorkerOutput { id, slug })
}

#[tauri::command]
pub fn update_worker_status(worker_id: i64, status: String) -> Result<()> {
    let db = WorkersDb::open()?;
    // Resolve slug for the status push event before mutating so a missing
    // worker row still surfaces as a clean error without a phantom emit.
    let worker = db
        .get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    db.update_status(worker_id, &status)?;
    // Phase 3.0 R2 — mirror UI-driven status edits through the push channel
    // so any other subscriber (another open window, a future mobile client)
    // sees the change instantly.
    crate::events::emit_worker_status(crate::events::WorkerStatusPayload {
        slug: worker.slug,
        status,
        previous_status: None,
        reason: None,
        cooldown_seconds: None,
        tick_duration_ms: None,
        last_outcome: None,
        timestamp: chrono::Utc::now().to_rfc3339(),
    });
    Ok(())
}

#[tauri::command]
pub fn update_worker_model(worker_id: i64, model: String) -> Result<()> {
    WorkersDb::open()?.update_model(worker_id, &model)
}

/// Update worker model and immediately sync to core-orchestrator.
///
/// Unlike `update_worker_model` which only updates the DB, this command:
/// 1. Updates workers.db
/// 2. Calls core-orchestrator API to rewrite the agent YAML immediately
/// 3. Returns a result the UI can use for toast + loading feedback
#[tauri::command]
pub async fn update_worker_model_sync(
    worker_id: i64,
    model: String,
    orch_client: State<'_, Arc<OrchestratorClient>>,
) -> Result<UpdateModelSyncResult> {
    // Step 1: Update database
    let db = WorkersDb::open()?;
    let worker = db
        .get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;

    db.update_model(worker_id, &model)?;

    // Step 2: Try to sync to core-orchestrator immediately
    let sync_result = if orch_client.is_healthy().await {
        match orch_client
            .update_agent_model_by_slug(&worker.slug, &model)
            .await
        {
            Ok(true) => (
                "synced",
                format!("Model updated to {} — change active immediately", model),
            ),
            Ok(false) => (
                "pending",
                format!(
                    "Model updated to {} — will take effect on next agent run",
                    model
                ),
            ),
            Err(e) => {
                tracing::warn!("[update_worker_model_sync] orchestrator sync failed: {e}");
                (
                    "failed",
                    format!("Model saved, but sync failed: {e}. Will retry on next tick."),
                )
            }
        }
    } else {
        (
            "pending",
            format!(
                "Model updated to {} — will sync when core-orchestrator is online",
                model
            ),
        )
    };

    // Step 3: Write directly to YAML so the file stays in sync regardless of
    // whether core-orchestrator is running. This is the authoritative source the
    // agent reads on boot — without this step a "pending" orchestrator sync would
    // leave the YAML stale until the next tick.
    if let Ok(orch_dir) = resolve_orchestrator_path(None) {
        let agents_dir = orch_dir.join("config").join("agents");
        if let Err(e) = write_yaml_model(&agents_dir, &worker.slug, &model) {
            tracing::warn!(
                "[update_worker_model_sync] YAML write failed for '{}': {e}",
                worker.slug
            );
        } else {
            tracing::info!(
                "[update_worker_model_sync] YAML updated for '{}' → '{}'",
                worker.slug,
                model
            );
        }
    }

    Ok(UpdateModelSyncResult {
        db_updated: true,
        sync_status: sync_result.0.to_string(),
        message: sync_result.1,
        model,
    })
}

#[tauri::command]
pub fn update_worker_repos(worker_id: i64, primary_repos: Vec<String>) -> Result<()> {
    WorkersDb::open()?.update_repos(worker_id, &primary_repos)
}

#[tauri::command]
pub fn bind_worker_recipe(worker_id: i64, recipe_id: String) -> Result<()> {
    WorkersDb::open()?.bind_recipe(worker_id, &recipe_id)
}

#[tauri::command]
pub fn delete_worker(worker_id: i64) -> Result<()> {
    WorkersDb::open()?.delete(worker_id)
}

/// Start the cron schedule for a worker's bound recipe.
/// Looks up the worker slug then delegates to the RecipeScheduler.
#[tauri::command]
pub fn start_worker_recipe(
    worker_id: i64,
    scheduler: State<'_, Arc<RecipeScheduler>>,
) -> Result<()> {
    let db = WorkersDb::open()?;
    let worker = db
        .get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    scheduler.start_worker(&worker.slug)
}

/// Stop the cron schedule for a worker.
#[tauri::command]
pub fn stop_worker_recipe(
    worker_id: i64,
    scheduler: State<'_, Arc<RecipeScheduler>>,
) -> Result<()> {
    let db = WorkersDb::open()?;
    let worker = db
        .get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    scheduler.stop_worker(&worker.slug)
}

// ─── YAML agent config helpers ────────────────────────────────────────────────

/// Read `llm.model` from `{agents_dir}/{slug}.yaml`. Returns None if the file
/// doesn't exist or the field is missing — caller falls back to the DB value.
fn read_yaml_model(agents_dir: &Path, slug: &str) -> Option<String> {
    let yaml_path = agents_dir.join(format!("{}.yaml", slug));
    let content = std::fs::read_to_string(&yaml_path).ok()?;
    let value: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    value
        .get("llm")?
        .get("model")?
        .as_str()
        .map(|s| s.to_string())
}

/// Surgically rewrite `llm.model` and `llm.provider` in `{agents_dir}/{slug}.yaml`
/// without disturbing any other fields, comments, or metadata.
///
/// Line-by-line replacement: when inside the `llm:` block (detected by the top-level
/// key) replace the `model:` and `provider:` lines.  All other lines pass through
/// unchanged, preserving comments, ordering, and the `metadata:` section.
fn write_yaml_model(agents_dir: &Path, slug: &str, model: &str) -> Result<()> {
    let yaml_path = agents_dir.join(format!("{}.yaml", slug));
    let content = std::fs::read_to_string(&yaml_path)
        .map_err(|e| AutoDevError::Agent(format!("read YAML for '{slug}': {e}")))?;

    let provider = model_to_provider(model).ok_or_else(|| {
        AutoDevError::Agent(format!(
            "Unknown model '{model}' — no supported provider serves it. \
             AutoDev supports Anthropic, OpenAI and Google; see src/data/models.ts \
             for the models on offer."
        ))
    })?;
    let mut result: Vec<String> = Vec::with_capacity(content.lines().count() + 2);
    let mut in_llm_block = false;
    let mut model_written = false;
    let mut provider_written = false;

    for line in content.lines() {
        let trimmed = line.trim_start();

        // Detect top-level keys (no leading whitespace, ends with ':')
        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_llm_block = trimmed.starts_with("llm:");
            // If we're leaving the llm block and fields were not present, inject them now.
            if !in_llm_block {
                // No-op: if the previous block was llm and fields were missing we'd need
                // more complex handling — for fleet YAMLs they always exist.
            }
        }

        if in_llm_block {
            if trimmed.starts_with("model:") && !model_written {
                // Preserve the indentation prefix of the original line.
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                result.push(format!("{}model: {}", indent, model));
                model_written = true;
                continue;
            }
            if trimmed.starts_with("provider:") && !provider_written {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                result.push(format!("{}provider: {}", indent, provider));
                provider_written = true;
                continue;
            }
        }
        result.push(line.to_string());
    }

    let new_content = result.join("\n") + "\n";
    std::fs::write(&yaml_path, new_content)
        .map_err(|e| AutoDevError::Agent(format!("write YAML for '{slug}': {e}")))?;

    Ok(())
}

/// Which provider serves this model.
///
/// Mirrors `providerForModel` in `src/data/models.ts`. The two must agree: the
/// UI offers a model, this decides where the request and the user's key go, and
/// a disagreement sends a credential to a vendor the user never chose.
///
/// ⚠ **Returns `None` for an unknown model rather than guessing.** This used to
/// end in `_ => "openai"`, so a typo, a retired id, or a model from a provider
/// we dropped was silently routed to OpenAI — which then failed with an
/// authentication or unknown-model error that pointed nowhere near the cause.
///
/// The old prefix table also routed `llama`/`gpt-oss` to Groq, `qwen` to
/// Dashscope and anything `-nvidia` to NVIDIA. None of those are supported
/// providers: `is_supported_provider` accepts only these three, so no key can
/// be stored for them and the route could never have completed.
/// Which provider serves a model.
///
/// ⚠ P2-53 collapsed this substring ladder into `routing::provider_for_model`
/// because three copies had drifted apart — `o4-mini` matched none of them and
/// was filed under `"unknown"`, corrupting the per-provider cost breakdown.
///
/// This was a FIFTH copy, missed in that pass and found by `TG-P2-09` on
/// 2026-09-02: its tests still asserted routing for `gpt-5.3-codex` and
/// `gemini-3.5-pro`, models the app no longer offers — a copy quietly
/// disagreeing with the registry, which is exactly the failure mode.
fn model_to_provider(model: &str) -> Option<&'static str> {
    autodev_core::provider::routing::provider_for_model(model)
}

#[cfg(test)]
mod model_provider_tests {
    use super::model_to_provider;

    #[test]
    fn the_three_supported_providers_are_routed() {
        assert_eq!(model_to_provider("claude-opus-5"), Some("anthropic"));
        assert_eq!(model_to_provider("claude-haiku-4-5"), Some("anthropic"));
        assert_eq!(model_to_provider("gpt-5.5"), Some("openai"));
        assert_eq!(model_to_provider("gpt-5.4-mini"), Some("openai"));
        assert_eq!(model_to_provider("gemini-3.7-flash"), Some("google"));
        assert_eq!(model_to_provider("gemini-3.1-pro-preview"), Some("google"));
    }

    #[test]
    fn a_dropped_provider_is_refused_not_rerouted() {
        // Each of these used to resolve to a provider no key can be stored
        // for -- or, worse, silently to OpenAI.
        for model in ["grok-3-mini-beta", "llama-3.3-70b", "qwen3-max", "kimi-k2.5-nvidia"] {
            assert_eq!(model_to_provider(model), None, "{model} must not route");
        }
    }

    #[test]
    fn an_unknown_model_is_refused_rather_than_guessed() {
        assert_eq!(model_to_provider("totally-made-up"), None);
        assert_eq!(model_to_provider(""), None);
    }
}
