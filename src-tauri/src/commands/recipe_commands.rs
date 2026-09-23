// Recipe commands — Tauri commands for recipe CRUD + versioning + AI generation.
// Phase 2.1: read-only (list + get).
// Phase 2.2: full create / update / delete + backup versioning.
// Phase 2.3: generate_recipe_yaml — AI-assisted YAML generation from description.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — Recipe Auto-Generation

use crate::errors::{ArcanaError, Result};
use crate::recipes::parser::{parse_recipe, save_recipe};
use crate::recipes::{self, recipes_dir, Recipe};
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;
use arcana_core::state::lock::LockTolerant;

// ─── Backup helpers ────────────────────────────────────────────────────────────

fn backups_dir() -> Result<PathBuf> {
    let dir = recipes_dir()?.join("backups");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Copy current on-disk recipe to the backups dir before overwriting.
fn backup_recipe_file(recipe_id: &str) -> Result<()> {
    let src = recipes_dir()?.join(format!("{recipe_id}.yaml"));
    if !src.exists() {
        return Ok(());
    }

    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S");
    let dst = backups_dir()?.join(format!("{recipe_id}-{ts}.yaml"));
    std::fs::copy(&src, &dst)?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeBackup {
    pub recipe_id: String,
    pub timestamp: String,
    pub filename: String,
}

// ─── Read Commands ─────────────────────────────────────────────────────────────

/// List all recipes in ~/.arcana/recipes/
#[tauri::command]
pub fn list_recipes() -> Result<Vec<Recipe>> {
    recipes::list()
}

/// Get a specific recipe by id (filename stem).
#[tauri::command]
pub fn get_recipe(id: String) -> Result<Option<Recipe>> {
    let all = recipes::list()?;
    Ok(all.into_iter().find(|r| r.id == id))
}

/// List backups for a specific recipe, newest first (max 10).
#[tauri::command]
pub fn list_recipe_backups(recipe_id: String) -> Result<Vec<RecipeBackup>> {
    let dir = backups_dir()?;
    let prefix = format!("{}-", recipe_id);

    let mut backups: Vec<(std::time::SystemTime, RecipeBackup)> = std::fs::read_dir(&dir)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".yaml") {
                // filename: <id>-<YYYYMMDDTHHMMSS>.yaml
                let stem = name.trim_end_matches(".yaml");
                let ts_part = stem.trim_start_matches(prefix.as_str()).to_string();
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((
                    modified,
                    RecipeBackup {
                        recipe_id: recipe_id.clone(),
                        timestamp: ts_part,
                        filename: name,
                    },
                ))
            } else {
                None
            }
        })
        .collect();

    backups.sort_by(|(a, _), (b, _)| b.cmp(a));
    backups.truncate(10);
    Ok(backups.into_iter().map(|(_, b)| b).collect())
}

// ─── Write Commands ────────────────────────────────────────────────────────────

/// Create a new recipe. Fails if a recipe with the same id already exists.
#[tauri::command]
pub fn create_recipe(recipe: Recipe) -> Result<()> {
    let dir = recipes_dir()?;
    let path = dir.join(format!("{}.yaml", recipe.id));
    if path.exists() {
        return Err(ArcanaError::Agent(format!(
            "Recipe '{}' already exists. Use update_recipe to modify it.",
            recipe.id
        )));
    }
    save_recipe(&dir, &recipe)
}

/// Update an existing recipe.
/// - Backs up the current version first.
/// - System recipes (is_system=true) can be updated only if the caller passes `force=true`.
#[tauri::command]
pub fn update_recipe(recipe: Recipe, force: Option<bool>) -> Result<()> {
    let dir = recipes_dir()?;
    let path = dir.join(format!("{}.yaml", recipe.id));

    // Check if it's a system recipe and force is not set
    if path.exists() {
        if let Ok(yaml) = std::fs::read_to_string(&path) {
            if let Ok(existing) = parse_recipe(&yaml) {
                if existing.is_system && !force.unwrap_or(false) {
                    return Err(ArcanaError::Agent(format!(
                        "Recipe '{}' is a system recipe. Pass force=true to override.",
                        recipe.id
                    )));
                }
            }
        }
    }

    backup_recipe_file(&recipe.id)?;
    save_recipe(&dir, &recipe)
}

/// Delete a recipe by id.
/// System recipes cannot be deleted regardless of force flag.
#[tauri::command]
pub fn delete_recipe(recipe_id: String) -> Result<()> {
    let dir = recipes_dir()?;
    let path = dir.join(format!("{recipe_id}.yaml"));
    if !path.exists() {
        return Err(ArcanaError::Agent(format!(
            "Recipe '{recipe_id}' not found"
        )));
    }

    // System recipe guard — always blocked
    if let Ok(yaml) = std::fs::read_to_string(&path) {
        if let Ok(recipe) = parse_recipe(&yaml) {
            if recipe.is_system {
                return Err(ArcanaError::Agent(format!(
                    "System recipe '{}' cannot be deleted",
                    recipe_id
                )));
            }
        }
    }

    backup_recipe_file(&recipe_id)?;
    std::fs::remove_file(&path)?;
    Ok(())
}

/// Restore a recipe from a specific backup file.
#[tauri::command]
pub fn restore_recipe_backup(recipe_id: String, backup_filename: String) -> Result<()> {
    let backup_path = backups_dir()?.join(&backup_filename);
    if !backup_path.exists() {
        return Err(ArcanaError::Agent(format!(
            "Backup not found: {backup_filename}"
        )));
    }

    // Before restoring, backup current version
    backup_recipe_file(&recipe_id)?;

    let dir = recipes_dir()?;
    let dst = dir.join(format!("{recipe_id}.yaml"));
    std::fs::copy(&backup_path, &dst)?;
    Ok(())
}

// ─── AI-Assisted Recipe Generation ────────────────────────────────────────────

/// Generate a recipe YAML skeleton using the active LLM provider.
/// Accepts a free-text description and returns a complete YAML string.
/// The user can then review, edit, and save via the RecipeEditor modal.
#[tauri::command]
pub async fn generate_recipe_yaml(
    worker_type: String,
    name: String,
    description: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<String> {
    use crate::provider::anthropic::AnthropicProvider;
    use crate::provider::google::GoogleProvider;
    use crate::provider::openai::OpenAIProvider;
    use crate::provider::traits::Provider;
    use crate::provider::types::{CompletionRequest, MessageContent, RequestMessage};

    let (provider_id, model) = {
        let s = state.lock_tolerant();
        let p = s
            .active_provider
            .clone()
            .ok_or_else(|| ArcanaError::Agent("No active provider set".into()))?;
        let m = s
            .active_model
            .clone()
            .ok_or_else(|| ArcanaError::Agent("No active model set".into()))?;
        (p, m)
    };

    // Every supported provider takes a key; the keyless branch went with
    // Ollama (P2-11).
    let api_key = VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
        ArcanaError::Agent(format!("No API key for provider '{provider_id}'"))
    })?;

    let inner: Arc<dyn Provider> = match provider_id.as_str() {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        _ => {
            return Err(ArcanaError::Agent(format!(
                "Unsupported provider: {provider_id}"
            )))
        }
    };
    // Every call passes the daily ceiling (P2-21).
    let provider = arcana_core::provider::capped::CappedProvider::wrap(inner, state.inner().clone());

    let prompt = format!(
        "Generate a recipe YAML for an Arcana worker agent with the following details:\n\
         - Worker type: {worker_type}\n\
         - Name: {name}\n\
         - Description: {description}\n\n\
         Output ONLY a valid YAML document in this exact schema (no markdown fences):\n\
         version: \"1.0\"\n\
         id: <kebab-case-id>\n\
         title: <title>\n\
         description: <description>\n\
         schedule: \"*/5 * * * *\"  # cron expression\n\
         permission_mode: autonomous  # or supervised\n\
         agent_role: <role>\n\
         model: claude-sonnet-4-5\n\
         instructions: |\n\
           <detailed agent instructions here>\n\
         \n\
         Fill in all fields appropriately based on the worker type and description.\n\
         The instructions section should describe exactly what the agent should do in its work loop."
    );

    let request = CompletionRequest {
        model:                  model.clone(),
        messages:               vec![RequestMessage {
            role:    "user".to_string(),
            content: vec![MessageContent::Text { text: prompt }],
        }],
        system:                 Some("You are an expert at writing Arcana worker agent recipes in YAML format. Output only valid YAML with no explanation or markdown.".to_string()),
        tools:                  vec![],
        max_tokens:             2048,
        temperature:            Some(0.4),
        top_p:                  None,
        reasoning_effort:       None,
        thinking_budget_tokens: None,
        // A recipe step's prompt is built per invocation; there is no shared
        // prefix across runs to cache.
        cache: arcana_core::provider::usage::CachePolicy::Off,
        // P3-25: a one-shot recipe generation with no tool history —
        // nothing to clear.
        clear_tool_uses:        None,
    };

    let response = provider.complete(request).await?;

    let yaml = response
        .content
        .into_iter()
        .find_map(|block| {
            if let MessageContent::Text { text } = block {
                Some(text)
            } else {
                None
            }
        })
        .ok_or_else(|| ArcanaError::Agent("Provider returned no text content".into()))?;

    // Strip any accidental markdown fences
    let yaml = yaml.trim();
    let yaml = yaml.strip_prefix("```yaml").unwrap_or(yaml);
    let yaml = yaml.strip_prefix("```").unwrap_or(yaml);
    let yaml = yaml.strip_suffix("```").unwrap_or(yaml);

    Ok(yaml.trim().to_string())
}
