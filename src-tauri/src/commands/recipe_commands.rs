// Recipe commands — Tauri commands for recipe CRUD + versioning + AI generation.
// Phase 2.1: read-only (list + get).
// Phase 2.2: full create / update / delete + backup versioning.
// Phase 2.3: generate_recipe_yaml — AI-assisted YAML generation from description.
// See: DEVELOPMENT_PLAN.md Phase 2.3 — Recipe Auto-Generation

use crate::errors::{ArcanaError, Result};
use crate::provider::traits::Provider;
use crate::recipes::parser::{parse_recipe, save_recipe};
use crate::recipes::{self, recipes_dir, Recipe};
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use arcana_core::egress_consent::{
    provider_destination, ConsentProvider, WorkspaceEgressConsentDb,
};
use arcana_core::state::lock::LockTolerant;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;

// ─── Backup helpers ────────────────────────────────────────────────────────────

fn backups_dir() -> Result<PathBuf> {
    let dir = recipes_dir()?.join("backups");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

async fn complete_recipe_generation(
    provider: Arc<dyn crate::provider::traits::Provider>,
    workspace: PathBuf,
    request: crate::provider::types::CompletionRequest,
) -> Result<crate::provider::types::CompletionResponse> {
    let destination = provider_destination(provider.id())?;
    let allowed = WorkspaceEgressConsentDb::open()?.authorize_and_audit(
        &workspace,
        &destination,
        "tauri_recipe_generation",
    )?;
    if !allowed {
        return Err(ArcanaError::EgressDenied { destination });
    }

    // The provider wrapper repeats the check immediately before egress, so a
    // revocation after this audited admission still blocks the request.
    ConsentProvider::wrap(provider, workspace)
        .complete(request)
        .await
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

    // Validate the backed-up artifact before replacing a working recipe.
    // Backups are persisted user content and may predate portability checks.
    let yaml = std::fs::read_to_string(&backup_path)?;
    let recipe = parse_recipe(&yaml)?;
    if recipe.id != recipe_id {
        return Err(ArcanaError::Agent(format!(
            "Backup recipe id '{}' does not match requested recipe '{recipe_id}'",
            recipe.id
        )));
    }

    // Before restoring, back up current version. save_recipe performs the same
    // validation at the final write boundary.
    backup_recipe_file(&recipe_id)?;
    let dir = recipes_dir()?;
    save_recipe(&dir, &recipe)
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
    use crate::provider::types::{CompletionRequest, MessageContent, RequestMessage};

    let workspace =
        arcana_core::platform::workspace::resolve_workspace_dir(None).map_err(|error| {
            ArcanaError::Agent(format!(
                "workspace unavailable for recipe generation: {error}"
            ))
        })?;

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
    let api_key = VaultManager::get(&provider_key(&provider_id))?
        .ok_or_else(|| ArcanaError::Agent(format!("No API key for provider '{provider_id}'")))?;

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
    let provider =
        arcana_core::provider::capped::CappedProvider::wrap(inner, state.inner().clone());

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

    let response = complete_recipe_generation(provider, workspace, request).await?;

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

#[cfg(test)]
mod consent_tests {
    use super::*;
    use crate::provider::traits::{Provider, StreamResult};
    use crate::provider::types::{
        CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus,
    };
    use arcana_core::errors::ArcanaError;
    use arcana_core::provider::types::MessageContent;
    use arcana_core::provider::usage::Usage;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ScriptedProvider(AtomicUsize);

    #[async_trait]
    impl Provider for ScriptedProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "scripted"
        }

        fn models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> crate::errors::Result<CompletionResponse> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                message_id: "scripted".into(),
                content: vec![MessageContent::Text {
                    text: "version: \"1.0\"".into(),
                }],
                stop_reason: "STOP".into(),
                usage: Usage::default(),
                model: "scripted".into(),
            })
        }

        async fn stream(&self, _request: CompletionRequest) -> crate::errors::Result<StreamResult> {
            Err(ArcanaError::Agent(
                "stream not used in recipe consent test".into(),
            ))
        }

        async fn test_connection(&self) -> crate::errors::Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    fn request() -> CompletionRequest {
        CompletionRequest {
            model: "scripted".into(),
            messages: vec![crate::provider::types::RequestMessage {
                role: "user".into(),
                content: vec![MessageContent::Text {
                    text: "private recipe description".into(),
                }],
            }],
            system: None,
            tools: Vec::new(),
            max_tokens: 32,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: Default::default(),
            clear_tool_uses: None,
        }
    }

    #[tokio::test]
    async fn recipe_generation_requires_matching_revocable_workspace_consent_and_audits_denials() {
        let home = tempfile::tempdir().expect("create isolated Arcana home");
        let workspace = home.path().join("workspace");
        let other_workspace = home.path().join("other-workspace");
        std::fs::create_dir_all(&workspace).expect("create selected workspace");
        std::fs::create_dir_all(&other_workspace).expect("create unrelated workspace");
        let workspace = workspace.canonicalize().expect("canonicalize workspace");
        let other_workspace = other_workspace
            .canonicalize()
            .expect("canonicalize other workspace");
        let _home = crate::test_support::isolate_home(home.path());
        let provider = Arc::new(ScriptedProvider(AtomicUsize::new(0)));
        let provider_trait: Arc<dyn Provider> = provider.clone();

        let absent =
            complete_recipe_generation(Arc::clone(&provider_trait), workspace.clone(), request())
                .await;
        assert!(matches!(absent, Err(ArcanaError::EgressDenied { .. })));
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);

        WorkspaceEgressConsentDb::open()
            .expect("open isolated consent store")
            .grant(&other_workspace, "provider:google")
            .expect("grant unrelated workspace");
        let mismatched_workspace =
            complete_recipe_generation(Arc::clone(&provider_trait), workspace.clone(), request())
                .await;
        assert!(matches!(
            mismatched_workspace,
            Err(ArcanaError::EgressDenied { .. })
        ));
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);

        let consent = WorkspaceEgressConsentDb::open().expect("open isolated consent store");
        consent
            .grant(&workspace, "provider:openai")
            .expect("grant mismatched destination");
        let mismatched_destination =
            complete_recipe_generation(Arc::clone(&provider_trait), workspace.clone(), request())
                .await;
        assert!(matches!(
            mismatched_destination,
            Err(ArcanaError::EgressDenied { .. })
        ));
        assert_eq!(provider.0.load(Ordering::SeqCst), 0);

        consent
            .grant(&workspace, "provider:google")
            .expect("grant matching destination");
        let response =
            complete_recipe_generation(Arc::clone(&provider_trait), workspace.clone(), request())
                .await
                .expect("matching grant reaches scripted provider");
        assert_eq!(provider.0.load(Ordering::SeqCst), 1);
        assert!(matches!(
            response.content.first(),
            Some(MessageContent::Text { .. })
        ));

        consent
            .revoke(&workspace, "provider:google")
            .expect("revoke matching destination");
        let revoked =
            complete_recipe_generation(provider_trait, workspace.clone(), request()).await;
        assert!(matches!(revoked, Err(ArcanaError::EgressDenied { .. })));
        assert_eq!(provider.0.load(Ordering::SeqCst), 1);

        let audit = arcana_core::db::config_db::open().expect("read isolated consent audit");
        let rows: Vec<(String, String, String, String)> = audit
            .prepare(
                "SELECT workspace_path, destination, operation, outcome
                 FROM workspace_egress_audit ORDER BY id",
            )
            .expect("prepare audit read")
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .expect("query audit rows")
            .collect::<std::result::Result<_, _>>()
            .expect("collect audit rows");
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].0, workspace.to_string_lossy());
        assert_eq!(rows[0].1, "provider:google");
        assert_eq!(rows[0].2, "tauri_recipe_generation");
        assert_eq!(rows[0].3, "denied");
        assert_eq!(rows[1].0, workspace.to_string_lossy());
        assert_eq!(rows[1].3, "denied");
        assert_eq!(rows[2].1, "provider:google");
        assert_eq!(rows[2].3, "denied");
        assert_eq!(rows[3].3, "allowed");
        assert_eq!(rows[4].2, "provider_complete");
        assert_eq!(rows[4].3, "allowed");
        assert_eq!(rows[5].3, "denied");
        let audit_json = serde_json::to_string(&rows).expect("serialize content-free audit");
        assert!(!audit_json.contains("private recipe description"));
    }
}
