// Provider construction, in one place (P2-23, P2-53).
//
// The `match provider_id { "anthropic" => ..., "openai" => ..., ... }` was
// written out at `agent_commands.rs:137`, `diff_commands.rs:37` and
// `native_runner.rs:246`. Three copies of a dispatch on a string is how
// `provider_for_model` drifted into three disagreeing versions, one of which
// filed `o4-mini` under `"unknown"` — so this is the same fix applied before
// the same divergence happens.
//
// It also gives key validation (`P2-23`) a provider to validate WITH, without
// a fourth copy appearing in the vault command.

use std::sync::Arc;

use super::anthropic::AnthropicProvider;
use super::google::GoogleProvider;
use super::openai::OpenAIProvider;
use super::traits::Provider;
use crate::errors::{AutoDevError, Result};

/// Build a provider for `provider_id` with `api_key`, **under the daily
/// spend ceiling**.
///
/// Returns an error for an unknown id rather than defaulting to one. Guessing
/// here would send the user's key to a vendor they never chose.
///
/// ── Why the cap is applied here and not left to callers ──────────────────────
///
/// `TG-P2-04` requires every provider construction to be wrapped, on the
/// principle that the ceiling belongs at the one place every call passes. This
/// function became that place the moment it existed — and the gate said so on
/// its first run, before any caller had been migrated.
///
/// `CappedProvider` deliberately does not cap `test_connection`, so key
/// validation (`P2-23`) still works when the day's ceiling has already been
/// reached. A user who is capped must still be able to fix a wrong key.
pub fn build(provider_id: &str, api_key: String) -> Result<Arc<dyn Provider>> {
    let inner: Arc<dyn Provider> = match provider_id {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        other => {
            return Err(AutoDevError::Provider(format!(
                "Unknown provider '{other}'. AutoDev supports anthropic, openai and google."
            )))
        }
    };
    Ok(super::capped::CappedProvider::wrap_global(inner))
}

/// Every provider id the application supports.
///
/// Derived from the catalogue so it cannot disagree with the model list: a
/// provider with no models is not a provider anyone can use.
pub fn supported_ids() -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = super::catalogue::CATALOGUE
        .iter()
        .map(|e| e.provider)
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_id_builds() {
        for id in supported_ids() {
            assert!(build(id, "test-key".into()).is_ok(), "{id} does not build");
        }
    }

    #[test]
    fn an_unknown_provider_is_refused_rather_than_defaulted() {
        // Defaulting here would send the user's key to a vendor they never
        // chose — the same reason `provider_for_model` returns None.
        let Err(err) = build("xai", "k".into()) else {
            panic!("an unknown provider must not build");
        };
        assert!(format!("{err}").contains("Unknown provider"));
        assert!(build("", "k".into()).is_err());
        assert!(build("ollama", "k".into()).is_err());
    }

    #[test]
    fn the_supported_set_is_exactly_the_three_shipped_providers() {
        assert_eq!(supported_ids(), vec!["anthropic", "google", "openai"]);
    }

    #[test]
    fn a_built_provider_reports_the_id_it_was_asked_for() {
        // Guards against a copy-paste in the match arms sending anthropic keys
        // to the openai adapter — which would look fine until the first call.
        for id in supported_ids() {
            let p = build(id, "k".into()).expect("build");
            assert_eq!(p.id(), id, "{id} built a provider that reports {}", p.id());
        }
    }
}
