// Settings commands — provider config, active model, connection testing.

use crate::agent::context::ContextBuilder;
use crate::errors::{ArcanaError, Result};
#[cfg(not(feature = "e2e-ipc-receipt"))]
use crate::provider::traits::Provider;
use crate::provider::types::{ModelInfo, ProviderStatus};
use crate::state::AppState;
#[cfg(not(feature = "e2e-ipc-receipt"))]
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
    // The receipt build must render Settings without even probing the host
    // credential store. Consent UI uses the GitHub MCP row independently.
    #[cfg(feature = "e2e-ipc-receipt")]
    return Ok(Vec::new());

    #[cfg(not(feature = "e2e-ipc-receipt"))]
    supported_provider_labels()?
        .into_iter()
        .map(|(id, name)| build_provider_info(id, name))
        .collect()
}

#[cfg(not(feature = "e2e-ipc-receipt"))]
fn supported_provider_labels() -> Result<Vec<(&'static str, &'static str)>> {
    arcana_core::provider::factory::supported_ids()
        .into_iter()
        .map(|id| {
            let name = match id {
                "anthropic" => "Anthropic",
                "openai" => "OpenAI",
                "google" => "Google",
                "groq" => "Groq",
                other => {
                    return Err(ArcanaError::Provider(format!(
                        "Settings has no display label for supported provider '{other}'"
                    )));
                }
            };
            Ok((id, name))
        })
        .collect()
}

#[cfg(not(feature = "e2e-ipc-receipt"))]
fn build_provider_info(id: &str, name: &str) -> Result<ProviderInfo> {
    // P2-70. `VaultManager::get(..).is_some()` pulled the plaintext key out of
    // the keychain to compute a boolean and dropped it unzeroized — on every
    // Settings render, once per provider. `exists` overwrites it instead.
    let has_key = VaultManager::exists(&provider_key(id))?;

    // P2-53/P2-23. This used to be a hand-rolled copy of the provider match;
    // it now goes through the factory (see the comment at the top of
    // `provider/factory.rs`). An empty key is fine here: nothing is called,
    // the adapter is constructed only to ask it which models it serves, and
    // that answer comes from the shared catalogue.
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
    #[cfg(feature = "e2e-ipc-receipt")]
    {
        let _ = provider;
        return Err(ArcanaError::Agent(
            "provider access is disabled in the isolated receipt build".into(),
        ));
    }

    #[cfg(not(feature = "e2e-ipc-receipt"))]
    {
        let key = VaultManager::get(&provider_key(&provider))?
            .ok_or_else(|| ArcanaError::Provider("No API key stored for provider".into()))?;

        let candidate = arcana_core::provider::factory::build(&provider, key)?;
        check_provider_metadata_connection(candidate.as_ref(), false).await
    }
}

/// Explicit Settings authentication/model-list checks send no workspace data.
/// ADR-0303 permits these before workspace selection; generation still requires
/// persisted workspace consent. Never reflect credentials or response text here.
#[cfg(not(feature = "e2e-ipc-receipt"))]
pub(super) async fn check_provider_metadata_connection(
    provider: &dyn Provider,
    key_validation: bool,
) -> Result<ProviderStatus> {
    let mut timer =
        arcana_core::observability::timing::OperationTimer::start("settings_provider_connection");
    let result = provider.test_connection().await;
    let outcome = match &result {
        Ok(ProviderStatus::Connected) => "success",
        Ok(_) => "rejected",
        Err(_) => "error",
    };
    timer.set_outcome(outcome);
    // The consent owner also validates the id (it refuses anything that is not
    // a real provider destination), so this call keeps one list instead of a
    // second copy that a newly added provider would not reach.
    let destination = arcana_core::egress_consent::provider_destination(provider.id())
        .unwrap_or_else(|_| "unsupported".to_string());
    tracing::info!(
        target: "arcana::telemetry",
        event_type = "provider_connection_check",
        destination,
        key_validation,
        outcome,
        workspace_payload_sent = false,
        "Explicit Settings metadata-only provider check completed"
    );
    result
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

#[cfg(all(test, not(feature = "e2e-ipc-receipt")))]
mod provider_list_tests {
    use super::supported_provider_labels;

    #[test]
    fn settings_provider_list_covers_the_factory_supported_set() {
        assert_eq!(
            supported_provider_labels().expect("Settings provider labels"),
            vec![
                ("anthropic", "Anthropic"),
                ("google", "Google"),
                ("groq", "Groq"),
                ("openai", "OpenAI"),
            ]
        );
    }
}

#[cfg(all(test, not(feature = "e2e-ipc-receipt")))]
mod connection_audit_tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CapturedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct MetadataProvider {
        outcome: u8,
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl Provider for MetadataProvider {
        fn id(&self) -> &str {
            "google"
        }
        fn name(&self) -> &str {
            "metadata fixture"
        }
        fn models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }
        async fn complete(
            &self,
            _: crate::provider::types::CompletionRequest,
        ) -> Result<crate::provider::types::CompletionResponse> {
            panic!("Settings must not generate workspace completions")
        }
        async fn stream(
            &self,
            _: crate::provider::types::CompletionRequest,
        ) -> Result<crate::provider::traits::StreamResult> {
            panic!("Settings must not stream workspace completions")
        }
        async fn test_connection(&self) -> Result<ProviderStatus> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            match self.outcome {
                0 => Ok(ProviderStatus::Connected),
                1 => Ok(ProviderStatus::Error("planted-private-response".into())),
                _ => Err(ArcanaError::Provider("planted-private-transport".into())),
            }
        }
    }

    #[test]
    fn explicit_settings_metadata_checks_audit_safe_outcomes_for_both_entrypoints() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        for key_validation in [false, true] {
            for (kind, expected) in [(0, "success"), (1, "rejected"), (2, "error")] {
                let provider = MetadataProvider {
                    outcome: kind,
                    calls: AtomicUsize::new(0),
                };
                let bytes = Arc::new(Mutex::new(Vec::new()));
                let output = bytes.clone();
                let subscriber = tracing_subscriber::fmt()
                    .json()
                    .with_writer(move || CapturedWriter(output.clone()))
                    .finish();
                let result = tracing::subscriber::with_default(subscriber, || {
                    runtime.block_on(check_provider_metadata_connection(
                        &provider,
                        key_validation,
                    ))
                });
                assert_eq!(result.is_err(), kind == 2);
                assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
                let text = String::from_utf8(bytes.lock().unwrap().clone()).unwrap();
                assert!(!text.contains("planted-private"));
                let events = text
                    .lines()
                    .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                    .collect::<Vec<_>>();
                let audit = events
                    .iter()
                    .find(|event| event["fields"]["event_type"] == "provider_connection_check")
                    .unwrap();
                assert_eq!(audit["fields"]["destination"], "provider:google");
                assert_eq!(audit["fields"]["key_validation"], key_validation);
                assert_eq!(audit["fields"]["workspace_payload_sent"], false);
                assert_eq!(audit["fields"]["outcome"], expected);
                let timing = events
                    .iter()
                    .find(|event| event["fields"]["operation"] == "settings_provider_connection")
                    .unwrap();
                assert_eq!(timing["fields"]["outcome"], expected);
                assert!(timing["fields"]["duration_ns"].as_u64().unwrap() > 0);
            }
        }
    }
}
