// Provider construction, in one place (P2-23, P2-53).
//
// This module was written because the `match provider_id { "anthropic" => ... }`
// dispatch had been copied to three call sites, and `provider_for_model` had
// already drifted into three disagreeing versions, one of which filed `o4-mini`
// under `"unknown"`.
//
// The copies reappeared anyway. By 2026-10-18 four call sites outside this
// module each built their own provider, and two of them — the recipe/fleet
// worker and the native subagent runner — had drifted: they refused `groq`,
// which the catalogue, Settings and the vault all offer, so a Groq user could
// not run fleet workers or subagents at all. `build_inner` is now the single
// construction site, and `provider_dispatch_has_exactly_one_construction_site`
// enforces it mechanically rather than by review.
//
// It also gives key validation (`P2-23`) a provider to validate WITH, without
// a further copy appearing in the vault command.

use std::sync::Arc;

use super::anthropic::AnthropicProvider;
use super::google::GoogleProvider;
use super::openai::OpenAIProvider;
use super::traits::Provider;
use crate::errors::{ArcanaError, Result};

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
    Ok(super::capped::CappedProvider::wrap_global(build_inner(
        provider_id,
        api_key,
    )?))
}

/// Build the provider for `provider_id` **without** the daily-spend ceiling.
///
/// This is the single construction site for every provider. Callers that must
/// bind the ceiling to a specific app `state` (the recipe worker and the
/// subagent runner, whose tests drive an explicit state) wrap the result
/// themselves with `capped::CappedProvider::wrap`; everything else uses
/// `build`, which wraps it globally.
///
/// It exists because the same `match provider_id` had reappeared at two call
/// sites after `build` was introduced, and both copies had already drifted:
/// neither knew about `groq`, so the fleet and subagents refused a provider the
/// user can select.
pub fn build_inner(provider_id: &str, api_key: String) -> Result<Arc<dyn Provider>> {
    let inner: Arc<dyn Provider> = match provider_id {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
        "openai" => Arc::new(OpenAIProvider::new(api_key)),
        "google" => Arc::new(GoogleProvider::new(api_key)),
        "groq" => Arc::new(super::groq::GroqProvider::new(api_key)),
        other => {
            return Err(ArcanaError::Provider(format!(
                "Unknown provider '{other}'. Arcana supports anthropic, openai, google and groq."
            )));
        }
    };
    Ok(inner)
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
    use std::path::Path;

    #[test]
    fn every_supported_id_builds() {
        for id in supported_ids() {
            assert!(build(id, "test-key".into()).is_ok(), "{id} does not build");
            assert!(
                build_inner(id, "test-key".into()).is_ok(),
                "{id} does not build uncapped"
            );
        }
    }

    /// The dispatch was copied out of this module at least four times, and the
    /// copies drifted: the worker and subagent copies silently omitted `groq`,
    /// so the fleet refused a provider the user can select. Review did not
    /// catch it twice, so the single construction site is enforced
    /// mechanically, the same way `observability::telemetry_labels` enforces
    /// its registry.
    ///
    /// Flagged shape: a provider-id arm that constructs a concrete provider
    /// on the same line, outside this module. Per-adapter wire tests construct
    /// their own adapter directly and are not dispatch copies.
    #[test]
    fn provider_dispatch_has_exactly_one_construction_site() {
        const PROVIDER_IDS: [&str; 4] = ["\"anthropic\"", "\"openai\"", "\"google\"", "\"groq\""];
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut offenders = Vec::new();
        let mut files = Vec::new();
        for directory in [
            "arcana-core/src",
            "src-tauri/src",
            "src-cli/src",
            "src-shared/src",
        ] {
            collect_rust_files(&root.join(directory), &mut files);
        }
        assert!(
            files.len() > 100,
            "source scan must find real files, found {}",
            files.len()
        );
        for file in files {
            if file.file_name().and_then(|name| name.to_str()) == Some("factory.rs") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&file) else {
                continue;
            };
            for (index, line) in source.lines().enumerate() {
                if line.contains("Provider::new(")
                    && PROVIDER_IDS.iter().any(|id| line.contains(id))
                {
                    offenders.push(format!("{}:{}", file.display(), index + 1));
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "provider construction must go through factory::build_inner; copies found at {offenders:?}"
        );
    }

    fn collect_rust_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, files);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                files.push(path);
            }
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
    fn the_supported_set_is_exactly_the_shipped_providers() {
        // Groq joined 2026-10-17 (razi-ruled): `qwen/qwen3.8-27b`, the model
        // Groq documents as the successor to the deprecated qwen3.6-27b.
        assert_eq!(
            supported_ids(),
            vec!["anthropic", "google", "groq", "openai"]
        );
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
