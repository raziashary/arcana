// Diff analysis commands — Feature 3 (Ruflo).
// Exposed to the frontend and callable by the QC Agent via MCP tool.

use std::sync::{Arc, Mutex};
use tauri::State;

use crate::diff_analysis::{DiffAnalysisResult, DiffAnalyzer};
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};

fn build_analyzer(state: &Arc<Mutex<AppState>>) -> Result<DiffAnalyzer> {
    let (provider_id, model) = {
        let s = state.lock().unwrap();
        let p = s
            .active_provider
            .clone()
            .ok_or_else(|| AutoDevError::Agent("No active provider set".into()))?;
        let m = s
            .active_model
            .clone()
            .ok_or_else(|| AutoDevError::Agent("No active model set".into()))?;
        (p, m)
    };

    let api_key = if provider_id != "ollama" {
        VaultManager::get(&provider_key(&provider_id))?.ok_or_else(|| {
            AutoDevError::Agent(format!("No API key for provider '{provider_id}'"))
        })?
    } else {
        String::new()
    };

    let provider: Arc<dyn Provider> = match provider_id.as_str() {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        "ollama" => Arc::new(OllamaProvider::default_from_env()),
        other => return Err(AutoDevError::Agent(format!("Unknown provider: {other}"))),
    };

    Ok(DiffAnalyzer::new(provider, model))
}

/// Analyze a raw unified diff string.
#[tauri::command]
pub async fn analyze_diff(
    diff_text: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<DiffAnalysisResult> {
    let analyzer = build_analyzer(&state)?;
    analyzer.analyze(&diff_text).await
}

/// Generate and analyze the diff between two branches in a git repository.
#[tauri::command]
pub async fn analyze_pr_diff(
    repo_path: String,
    base_branch: String,
    head_branch: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<DiffAnalysisResult> {
    let patch = crate::git::diff::branch_diff(&repo_path, &base_branch, &head_branch)?;
    let analyzer = build_analyzer(&state)?;
    analyzer.analyze(&patch).await
}
