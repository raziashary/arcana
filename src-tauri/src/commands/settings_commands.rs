// Settings commands — provider config, active model, connection testing.

use crate::agent::context::ContextBuilder;
use crate::errors::{AutoDevError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::provider::types::{ModelInfo, ProviderStatus};
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub status: ProviderStatus,
    pub models: Vec<ModelInfo>,
    pub has_key: bool,
}

#[tauri::command]
pub async fn list_providers(_state: State<'_, Arc<Mutex<AppState>>>) -> Result<Vec<ProviderInfo>> {
    let providers = vec![
        build_provider_info("anthropic", "Anthropic")?,
        build_provider_info("openai", "OpenAI")?,
        build_provider_info("google", "Google")?,
        build_provider_info("ollama", "Ollama (Local)")?,
    ];
    Ok(providers)
}

fn build_provider_info(id: &str, name: &str) -> Result<ProviderInfo> {
    let has_key = VaultManager::get(&provider_key(id))?.is_some();
    let models = match id {
        "anthropic" => AnthropicProvider::new(String::new()).models(),
        "openai" => OpenAIProvider::new(String::new()).models(),
        "google" => GoogleProvider::new(String::new()).models(),
        "ollama" => OllamaProvider::default_from_env().models(),
        _ => vec![],
    };
    Ok(ProviderInfo {
        id: id.to_string(),
        name: name.to_string(),
        status: ProviderStatus::Disconnected,
        models,
        has_key,
    })
}

#[tauri::command]
pub async fn test_provider_connection(provider: String) -> Result<ProviderStatus> {
    // Ollama does not require an API key — connect directly.
    if provider == "ollama" {
        let p = OllamaProvider::default_from_env();
        return p.test_connection().await;
    }

    let key = VaultManager::get(&provider_key(&provider))?
        .ok_or_else(|| AutoDevError::Provider("No API key stored for provider".into()))?;

    match provider.as_str() {
        "anthropic" => {
            let p = AnthropicProvider::new(key);
            p.test_connection().await
        }
        "openai" => {
            let p = OpenAIProvider::new(key);
            p.test_connection().await
        }
        "google" => {
            let p = GoogleProvider::new(key);
            p.test_connection().await
        }
        _ => Err(AutoDevError::Provider(format!(
            "Provider '{}' not yet implemented",
            provider
        ))),
    }
}

#[tauri::command]
pub fn set_active_provider(
    provider: String,
    model: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<()> {
    let mut s = state.lock().unwrap();
    s.active_provider = Some(provider);
    s.active_model = Some(model);
    Ok(())
}

#[tauri::command]
pub fn get_active_provider(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<(String, String)>> {
    let s = state.lock().unwrap();
    match (&s.active_provider, &s.active_model) {
        (Some(p), Some(m)) => Ok(Some((p.clone(), m.clone()))),
        _ => Ok(None),
    }
}

#[tauri::command]
pub fn get_cost_today(state: State<'_, Arc<Mutex<AppState>>>) -> Result<(f64, u64, u64)> {
    let s = state.lock().unwrap();
    Ok((s.cost_today_usd, s.tokens_in_today, s.tokens_out_today))
}

#[tauri::command]
pub fn get_system_prompt(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Option<String>> {
    let s = state.lock().unwrap();
    Ok(s.system_prompt_override.clone())
}

#[tauri::command]
pub fn set_system_prompt(
    prompt: Option<String>,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<()> {
    let mut s = state.lock().unwrap();
    s.system_prompt_override = prompt
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    Ok(())
}

#[tauri::command]
pub fn get_default_system_prompt() -> Result<String> {
    Ok(ContextBuilder::default_system_prompt().to_string())
}
