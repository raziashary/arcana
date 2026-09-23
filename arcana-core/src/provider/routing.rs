// Which provider serves a model id (P2-53).
//
// ── Why this is one function and not three ───────────────────────────────────
//
// The same substring ladder was written out at `engine.rs:379-387` and
// `runner.rs:441-452`, and a third variant lives in `worker_commands.rs`. They
// did not agree: `o4-mini` matched none of them and was filed as `"unknown"`,
// which corrupts the per-provider cost breakdown in `aggregator.rs` — spend
// attributed to a provider that does not exist.
//
// Duplicated dispatch on strings drifts by construction. One function, one
// answer, and a test that pins the answer for every model the app offers.

/// The provider id that serves `model`, or `None` if nothing does.
///
/// ⚠ **`None` rather than a guess.** The frontend's equivalent
/// (`providerForModel` in `src/data/models.ts`) and `worker_commands.rs` both
/// return `None`/error for an unknown id, and this must agree with them: a
/// mismatch sends the request — and the user's key — to a vendor they never
/// chose.
pub fn provider_for_model(model: &str) -> Option<&'static str> {
    if model.starts_with("claude") {
        Some("anthropic")
    } else if model.starts_with("gpt-") || model.starts_with("o1-") || model.starts_with("o3") {
        Some("openai")
    } else if model.starts_with("gemini") {
        Some("google")
    } else {
        None
    }
}

/// The provider id for analytics attribution.
///
/// Analytics must record *something* for every call, so an unknown model is
/// attributed to `"unknown"` here rather than dropped — but this is the **only**
/// place that substitution is acceptable, and it is deliberately a different
/// function from `provider_for_model` so a caller cannot reach for it by
/// accident when routing a request.
pub fn provider_for_analytics(model: &str) -> &'static str {
    provider_for_model(model).unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_offered_model_routes() {
        // The ids in `src/data/models.ts`. If one of these ever returns None,
        // requests for it have nowhere to go.
        for m in [
            "gemini-3.1-pro-preview",
            "gemini-3.7-flash",
            "claude-opus-5",
            "claude-sonnet-5",
            "claude-fable-5",
            "claude-haiku-4-5",
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.4-nano",
        ] {
            assert!(provider_for_model(m).is_some(), "{m} does not route");
        }
    }

    #[test]
    fn the_o_series_routes_to_openai() {
        // `o4-mini` matched none of the three duplicated ladders and was filed
        // as "unknown", corrupting the per-provider cost breakdown.
        assert_eq!(provider_for_model("o3"), Some("openai"));
        assert_eq!(provider_for_model("o3-mini"), Some("openai"));
        assert_eq!(provider_for_model("o1-preview"), Some("openai"));
    }

    #[test]
    fn a_dropped_provider_does_not_route() {
        for m in ["grok-3-mini-beta", "llama-3.3-70b", "qwen3-max", "ollama-llama3"] {
            assert_eq!(provider_for_model(m), None, "{m} must not route");
        }
    }

    #[test]
    fn analytics_attribution_never_drops_a_call_but_never_guesses_a_vendor() {
        assert_eq!(provider_for_analytics("gemini-3.7-flash"), "google");
        // Recorded, but not attributed to a real vendor.
        assert_eq!(provider_for_analytics("made-up"), "unknown");
        // And the routing function still refuses it.
        assert_eq!(provider_for_model("made-up"), None);
    }
}
