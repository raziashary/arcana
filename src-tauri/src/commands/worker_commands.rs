// Worker management commands — CRUD for ~/.autodev/workers.db
// Phase 2.2: CRUD + creation/deletion.
// Phase 2:   start/stop now wired to the real RecipeScheduler (cron engine).
// See: DEVELOPMENT_PLAN.md Phase 2 — Recipe Execution Engine

use std::sync::Arc;
use tauri::State;
use crate::errors::{AutoDevError, Result};
use crate::db::workers::{Worker, WorkersDb, slugify};
use crate::recipes::scheduler::RecipeScheduler;
use crate::sidecar::client::OrchestratorClient;
use serde::{Deserialize, Serialize};

// ─── Input / Output types ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateWorkerInput {
    pub name:             String,
    pub worker_type:      String,
    pub model:            String,
    pub primary_repos:    Vec<String>,
    pub polling_interval: Option<i64>,
    pub description:      Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateWorkerOutput {
    pub id:   i64,
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
pub fn list_workers() -> Result<Vec<Worker>> {
    WorkersDb::open()?.list()
}

#[tauri::command]
pub fn get_worker_by_slug(slug: String) -> Result<Option<Worker>> {
    WorkersDb::open()?.get_by_slug(&slug)
}

#[tauri::command]
pub fn create_worker(input: CreateWorkerInput) -> Result<CreateWorkerOutput> {
    let db   = WorkersDb::open()?;
    let slug = slugify(&input.name);

    if db.slug_exists(&slug)? {
        return Err(crate::errors::AutoDevError::Agent(
            format!("Worker slug '{}' already exists", slug)
        ));
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
    let worker = db.get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    db.update_status(worker_id, &status)?;
    // Phase 3.0 R2 — mirror UI-driven status edits through the push channel
    // so any other subscriber (another open window, a future mobile client)
    // sees the change instantly.
    crate::events::emit_worker_status_global(crate::events::WorkerStatusPayload {
        slug:      worker.slug,
        status,
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
    let worker = db.get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    
    db.update_model(worker_id, &model)?;
    
    // Step 2: Try to sync to core-orchestrator immediately
    let sync_result = if orch_client.is_healthy().await {
        match orch_client.update_agent_model_by_slug(&worker.slug, &model).await {
            Ok(true) => ("synced", format!("Model updated to {} — change active immediately", model)),
            Ok(false) => ("pending", format!("Model updated to {} — will take effect on next agent run", model)),
            Err(e) => {
                tracing::warn!("[update_worker_model_sync] orchestrator sync failed: {e}");
                ("failed", format!("Model saved, but sync failed: {e}. Will retry on next tick."))
            }
        }
    } else {
        ("pending", format!("Model updated to {} — will sync when core-orchestrator is online", model))
    };
    
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
    let db     = WorkersDb::open()?;
    let worker = db.get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    scheduler.start_worker(&worker.slug)
}

/// Stop the cron schedule for a worker.
#[tauri::command]
pub fn stop_worker_recipe(
    worker_id: i64,
    scheduler: State<'_, Arc<RecipeScheduler>>,
) -> Result<()> {
    let db     = WorkersDb::open()?;
    let worker = db.get_by_id(worker_id)?
        .ok_or_else(|| AutoDevError::Agent(format!("Worker id={worker_id} not found")))?;
    scheduler.stop_worker(&worker.slug)
}
