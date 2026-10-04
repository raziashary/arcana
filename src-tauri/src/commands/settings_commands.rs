// Settings commands — provider config, active model, connection testing.

use crate::agent::context::ContextBuilder;
use crate::errors::{ArcanaError, Result};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::traits::Provider;
use crate::provider::types::{ModelInfo, ProviderStatus};
use crate::state::AppState;
use crate::vault::keychain::{provider_key, VaultManager};
use arcana_core::state::lock::LockTolerant;
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
    supported_provider_labels()?
        .into_iter()
        .map(|(id, name)| build_provider_info(id, name))
        .collect()
}

fn supported_provider_labels() -> Result<Vec<(&'static str, &'static str)>> {
    arcana_core::provider::factory::supported_ids()
        .into_iter()
        .map(|id| {
            let name = match id {
                "anthropic" => "Anthropic",
                "openai" => "OpenAI",
                "google" => "Google",
                other => {
                    return Err(ArcanaError::Provider(format!(
                        "Settings has no display label for supported provider '{other}'"
                    )))
                }
            };
            Ok((id, name))
        })
        .collect()
}

fn build_provider_info(id: &str, name: &str) -> Result<ProviderInfo> {
    // P2-70. `VaultManager::get(..).is_some()` pulled the plaintext key out of
    // the keychain to compute a boolean and dropped it unzeroized — on every
    // Settings render, once per provider. `exists` overwrites it instead.
    let has_key = VaultManager::exists(&provider_key(id))?;

    // P2-53/P2-23. The fourth copy of this provider match. An empty key is
    // fine here: nothing is called, the adapter is constructed only to ask it
    // which models it serves, and that answer comes from the shared catalogue.
    let models = arcana_core::provider::factory::build(id, String::new())
        .map(|p| p.models())
        .unwrap_or_default();
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
    let key = VaultManager::get(&provider_key(&provider))?
        .ok_or_else(|| ArcanaError::Provider("No API key stored for provider".into()))?;

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
        _ => Err(ArcanaError::Provider(format!(
            "Provider '{}' not yet implemented",
            provider
        ))),
    }
}

/// Read the host-selected user-owned Python fallback used only when staging
/// the preferred signed interpreter fails. The executable path is not secret.
#[tauri::command]
pub fn get_python_user_fallback() -> Result<Option<String>> {
    #[cfg(windows)]
    {
        return arcana_core::platform::python_runtime::get_user_python_fallback()
            .map(|path| path.map(|path| path.to_string_lossy().into_owned()))
            .map_err(|error| ArcanaError::Agent(error.to_string()));
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

/// Persist an explicitly selected, signed Python executable under the current
/// user's `%LOCALAPPDATA%\\Programs\\Python` installation tree.
#[tauri::command]
pub fn set_python_user_fallback(path: Option<String>) -> Result<Option<String>> {
    #[cfg(windows)]
    {
        let path = path.map(std::path::PathBuf::from);
        return arcana_core::platform::python_runtime::set_user_python_fallback(path.as_deref())
            .map(|path| path.map(|path| path.to_string_lossy().into_owned()))
            .map_err(|error| ArcanaError::Agent(error.to_string()));
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(ArcanaError::Agent(
            "User-selected Python fallback is available only on Windows".into(),
        ))
    }
}

#[tauri::command]
pub fn set_active_provider(
    provider: String,
    model: String,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<()> {
    // P2-26. This wrote to in-memory AppState and nowhere else, so the user's
    // provider and model choice died on every restart — they picked a model,
    // used it, quit, and came back to nothing selected.
    //
    // Persisted BEFORE the in-memory update: if the write fails, the process
    // must not go on believing a selection that will not survive it. The
    // opposite order gives the user a working session and a silent surprise
    // next launch.
    arcana_core::db::app_settings::set(ACTIVE_PROVIDER_KEY, &provider)?;
    arcana_core::db::app_settings::set(ACTIVE_MODEL_KEY, &model)?;

    let mut s = state.lock_tolerant();
    s.active_provider = Some(provider);
    s.active_model = Some(model);
    Ok(())
}

/// Settings keys for the active selection (`P2-26`).
pub const ACTIVE_PROVIDER_KEY: &str = "active_provider";
pub const ACTIVE_MODEL_KEY: &str = "active_model";

/// Restore the persisted selection into `AppState` at startup (`P2-26`).
///
/// ⚠ A persisted model that is no longer in the catalogue is DISCARDED rather
/// than restored. Models are retired — four of ours were, in February 2026 —
/// and hydrating a dead id would hand the user a selection that looks valid and
/// 404s on first use, which is harder to diagnose than an empty picker.
pub fn hydrate_active_provider(state: &Arc<Mutex<AppState>>) {
    let provider = arcana_core::db::app_settings::get(ACTIVE_PROVIDER_KEY)
        .ok()
        .flatten();
    let model = arcana_core::db::app_settings::get(ACTIVE_MODEL_KEY)
        .ok()
        .flatten();

    let (provider, model) = match (provider, model) {
        (Some(provider), Some(model)) => (provider, model),
        (None, None) => {
            let mut s = state.lock_tolerant();
            s.active_provider = Some("google".into());
            s.active_model = Some("gemini-3.8-flash".into());
            return;
        }
        _ => return,
    };

    // The owner selected Gemini 3.8 Flash as the PA and fleet default. Move a
    // saved Google selection from the previous default, while preserving any
    // explicit alternative model selection.
    let model = if provider == "google" && model == "gemini-3.7-flash" {
        if let Err(error) = arcana_core::db::app_settings::set(ACTIVE_MODEL_KEY, "gemini-3.8-flash")
        {
            tracing::warn!("Could not update the saved Google model default: {error}");
            model
        } else {
            "gemini-3.8-flash".to_owned()
        }
    } else {
        model
    };

    if arcana_core::provider::catalogue::entry(&model).is_none() {
        tracing::warn!(
            "The saved model '{model}' is no longer offered, so it has not been restored.              Choose a model in Settings."
        );
        return;
    }

    if arcana_core::provider::routing::provider_for_model(&model) != Some(provider.as_str()) {
        tracing::warn!(
            "The saved selection ({provider} / {model}) is inconsistent — that model is              not served by that provider — so it has not been restored."
        );
        return;
    }

    let mut s = state.lock_tolerant();
    s.active_provider = Some(provider);
    s.active_model = Some(model);
}

#[tauri::command]
pub fn get_active_provider(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Option<(String, String)>> {
    let s = state.lock_tolerant();
    match (&s.active_provider, &s.active_model) {
        (Some(p), Some(m)) => Ok(Some((p.clone(), m.clone()))),
        _ => Ok(None),
    }
}

#[tauri::command]
pub fn get_cost_today(state: State<'_, Arc<Mutex<AppState>>>) -> Result<(f64, u64, u64)> {
    let mut s = state.lock_tolerant();
    Ok((s.cost_today(), s.tokens_in_today, s.tokens_out_today))
}

#[tauri::command]
pub fn get_system_prompt(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Option<String>> {
    let s = state.lock_tolerant();
    Ok(s.system_prompt_override.clone())
}

#[tauri::command]
pub fn set_system_prompt(
    prompt: Option<String>,
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<()> {
    let mut s = state.lock_tolerant();
    s.system_prompt_override = prompt
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    Ok(())
}

#[tauri::command]
pub fn get_default_system_prompt() -> Result<String> {
    Ok(ContextBuilder::default_system_prompt().to_string())
}

#[cfg(test)]
mod provider_list_tests {
    use super::supported_provider_labels;

    #[test]
    fn settings_provider_list_covers_the_factory_supported_set() {
        assert_eq!(
            supported_provider_labels().expect("Settings provider labels"),
            vec![
                ("anthropic", "Anthropic"),
                ("google", "Google"),
                ("openai", "OpenAI"),
            ]
        );
    }
}
