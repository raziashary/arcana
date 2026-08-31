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
    let patch = generate_branch_diff(&repo_path, &base_branch, &head_branch)?;
    let analyzer = build_analyzer(&state)?;
    analyzer.analyze(&patch).await
}

/// Generate a unified diff string between two branches using git2.
/// Public so `analyze_diff_tool` can call it directly without going through Tauri state.
pub fn generate_branch_diff_pub(
    repo_path: &str,
    base_branch: &str,
    head_branch: &str,
) -> Result<String> {
    generate_branch_diff(repo_path, base_branch, head_branch)
}

fn generate_branch_diff(repo_path: &str, base_branch: &str, head_branch: &str) -> Result<String> {
    let repo = git2::Repository::open(repo_path)
        .map_err(|e| AutoDevError::Git(format!("open repo: {e}")))?;

    let base_obj = repo
        .revparse_single(base_branch)
        .map_err(|e| AutoDevError::Git(format!("revparse '{base_branch}': {e}")))?;
    let head_obj = repo
        .revparse_single(head_branch)
        .map_err(|e| AutoDevError::Git(format!("revparse '{head_branch}': {e}")))?;

    let base_commit = base_obj
        .peel_to_commit()
        .map_err(|e| AutoDevError::Git(format!("peel to commit '{base_branch}': {e}")))?;
    let head_commit = head_obj
        .peel_to_commit()
        .map_err(|e| AutoDevError::Git(format!("peel to commit '{head_branch}': {e}")))?;

    let base_tree = base_commit
        .tree()
        .map_err(|e| AutoDevError::Git(format!("base tree: {e}")))?;
    let head_tree = head_commit
        .tree()
        .map_err(|e| AutoDevError::Git(format!("head tree: {e}")))?;

    let mut opts = git2::DiffOptions::new();
    opts.context_lines(3).interhunk_lines(1);

    let diff = repo
        .diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut opts))
        .map_err(|e| AutoDevError::Git(format!("diff_tree_to_tree: {e}")))?;

    let mut patch = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        match line.origin() {
            '+' | '-' | ' ' => patch.push(line.origin()),
            _ => {}
        }
        if let Ok(s) = std::str::from_utf8(line.content()) {
            patch.push_str(s);
        }
        true
    })
    .map_err(|e| AutoDevError::Git(format!("print patch: {e}")))?;

    Ok(patch)
}
