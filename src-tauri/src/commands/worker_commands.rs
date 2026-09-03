// Worker management commands — CRUD for ~/.autodev/workers.db
// Phase 2.2: CRUD + creation/deletion.
// Phase 2:   start/stop now wired to the real RecipeScheduler (cron engine).
// See: DEVELOPMENT_PLAN.md Phase 2 — Recipe Execution Engine

use crate::db::workers::{slugify, Worker, WorkersDb};
use crate::errors::{AutoDevError, Result};
use crate::recipes::scheduler::RecipeScheduler;

use serde::{Deserialize, Serialize};
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

/// Result of updating a worker's model.
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
pub async fn list_workers() -> Result<Vec<Worker>> {
    // P8-09 — `workers.db` is the source of truth for a worker's model.
    //
    // 🔴 This used to prefer a second process over HTTP and then fall back
    // to reading `config/agents/*.yaml` out of that process's own
    // directory. Both sources are deleted with it, and neither was ever the
    // authority: `P4-30` states the shape this now has — **the DB owns
    // identity and state, the recipe owns the prompt**. The two extra sources
    // were a way for a Python process's opinion to overwrite the row a user
    // had just edited.
    Ok(WorkersDb::open()?.list()?)
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

/// Update a worker's model and report the result to the UI.
///
/// ⚠ The name says `_sync` and the `UpdateModelSyncResult` shape carries a
/// `sync_status`, because this used to push the model into the sidecar as well
/// as the database. `P8-09` deleted that; the database write **is** the change
/// now. Both are kept because `WorkerTable.tsx:75` invokes this command by name
/// and renders its result — renaming them is a UI change, not a cleanup.
///
/// Unlike `update_worker_model`, which writes and returns nothing, this
/// validates the worker exists first and returns a message the UI can toast.
#[tauri::command]
pub async fn update_worker_model_sync(
    worker_id: i64,
    model: String,
) -> Result<UpdateModelSyncResult> {
    // Step 1: Update database
    let db = WorkersDb::open()?;

    // Existence guard, not a read. `update_model` on a missing id is a silent
    // no-op in SQLite, so without this the command would report "synced" for a
    // worker that does not exist.
    db.get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;

    db.update_model(worker_id, &model)?;

    // P8-09 — steps 2 and 3 are DELETED. Both wrote into the sidecar.
    //
    // 🔴 Step 2 pushed the model to a second process over HTTP; step 3
    // wrote it into `config/agents/*.yaml` inside that process's own
    // directory, described there as *"the authoritative source the agent
    // reads on boot"*. Neither destination exists. The database write above
    // IS the change now, and it takes effect on the next tick because the
    // engine reads the row rather than a file a Python process owned.
    //
    // ⚠ The command, its name and its return SHAPE are unchanged —
    // `WorkerTable.tsx:75` invokes it and renders the toast. What changes is
    // that `sync_status` can no longer be `"pending"` or `"failed"`: there
    // is nothing left to be pending on. Reporting "pending" forever would
    // be a status with no producer, which is the defect `W-7` names.
    let sync_result = (
        "synced",
        format!("Model updated to {model} — active on the next tick"),
    );

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

// ─── YAML agent config helpers — DELETED (P8-09 knock-on) ─────────────────────
//
// 🔴 `read_yaml_model` and `write_yaml_model` read and surgically rewrote
// `{agents_dir}/{slug}.yaml` — the sidecar's `config/agents/*.yaml`, described
// in their own doc comments as the authoritative source an agent read on boot.
// That directory is gone with the sidecar, so both were writing to a path that
// cannot exist. Neither had a caller after `P8-09` removed the sync steps, and
// both were private, so they warned rather than failing the build.
//
// `model_to_provider` went with them: it was a one-line delegate to
// `provider::routing::provider_for_model` kept only so those two could call it.
// ⚠ Its tests did NOT go with it — they assert real routing behaviour, so they
// now call the router directly. A test that only reaches its subject through a
// deleted wrapper is a test of nothing (`L-C12`).

#[cfg(test)]
mod model_provider_tests {
    use autodev_core::provider::routing::provider_for_model as model_to_provider;

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
