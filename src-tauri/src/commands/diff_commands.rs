// Diff analysis commands — Feature 3 (Ruflo).
// Exposed to the frontend and callable by the QC Agent via MCP tool.

use std::sync::{Arc, Mutex};
use tauri::State;

use crate::diff_analysis::{DiffAnalysisResult, DiffAnalyzer};
use crate::errors::{ArcanaError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use arcana_core::state::lock::LockTolerant;

fn build_analyzer(state: &Arc<Mutex<AppState>>) -> Result<DiffAnalyzer> {
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
        other => return Err(ArcanaError::Agent(format!("Unknown provider: {other}"))),
    };
    // Every call passes the daily ceiling (P2-21).
    let provider = arcana_core::provider::capped::CappedProvider::wrap(inner, state.clone());

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
