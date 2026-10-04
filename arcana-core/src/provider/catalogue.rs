// The model catalogue — one table, consumed by every adapter (P2-03, P2-31).
//
// ── Why this file exists ─────────────────────────────────────────────────────
//
// Each adapter carried its own `models_static()` list, and by 2026-09-02 all
// three had drifted a full generation behind the app. The backend listed:
//
//   claude-opus-4-5 · claude-sonnet-4-5 · gpt-4.1 · gpt-4.1-mini · gpt-4o ·
//   o4-mini · gemini-2.0-flash-thinking-exp-01-21 · gemini-2.5-pro ·
//   gemini-2.5-flash
//
// while `src/data/models.ts` — the list the user actually picks from — offered
// twelve entirely different ids. The intersection of the two was a single
// model, `claude-haiku-4-5`. Four of the backend's entries (`gpt-4o`,
// `gpt-4.1`, `gpt-4.1-mini`, `o4-mini`) were **retired in February 2026**.
//
// That is not a cosmetic mismatch. Anything that looks a model up by id got
// nothing back for eleven of the twelve models on offer:
//
//   * `P2-57` reads `context_window` from `models()` so compaction stops
//     hardcoding 200k. Against the stale list it found no row for
//     `gemini-3.1-pro` and fell through to the 200k default — so the fix was
//     inert for every model we ship, and Gemini still compacted at 20% of its
//     real window. **A fix that lands in the right place and is never reached
//     is not a fix**, and nothing failed loudly enough to say so.
//   * `capabilities()` (`P2-03`) would have been inert in exactly the same way
//     had it been built against that list.
//
// So the catalogue is one table here, and the adapters read it. `TG-P2-05`
// holds it against `models.ts` so the two lists cannot drift apart again.
//
// ── What is NOT here ─────────────────────────────────────────────────────────
//
// Prices. They live in `cost/pricing.rs`, which owns tiering and per-price
// verification dates. Duplicating them here would create the second list this
// file exists to abolish.

use super::types::ModelInfo;
use super::usage::{CacheSupport, ModelCapabilities};

/// One catalogue row: everything about a model except its price.
pub struct CatalogueEntry {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub context_window: u32,
    /// Maximum tokens the model will emit in one response. Used to clamp
    /// `max_tokens` (`P2-08`) rather than let the provider reject the request.
    pub max_output: u32,
    pub vision: bool,
    pub reasoning: bool,
    pub parallel_tools: bool,
    pub cache: CacheSupport,
}

/// Every model the application offers, and nothing else.
///
/// ⚠ **This list must equal `MODEL_GROUPS` in `src/data/models.ts` exactly** —
/// `TG-P2-05` fails the build if it does not. A model here but not there cannot
/// be selected; a model there but not here has no capabilities, no context
/// window and no route.
///
/// ── On the cache minimums ────────────────────────────────────────────────────
///
/// Anthropic's are documented and load-bearing: below the minimum **nothing
/// caches and no error is returned** (`G6-agents-sdk-research.md:341`) —
/// Opus 4.6/4.5 → 4,096, Sonnet 5/4.6/4.5 → 1,024, Haiku 4.5 → 4,096.
///
/// Where a current model has no published figure, the value is carried from
/// its immediate predecessor and marked below. **The error is taken upward on
/// purpose**: a minimum set too high raises a warning on a prefix that would
/// in fact have cached — noisy but harmless — while one set too low reports a
/// prefix as cacheable that silently never caches, which is the exact failure
/// this table exists to make visible. `P2-65` replaces every carried value
/// with a measured one via `count_tokens`.
pub const CATALOGUE: &[CatalogueEntry] = &[
    // ── Google · the primary provider for rollout 1 ──────────────────────────
    //
    // Gemini caches implicitly: there is no breakpoint to place, but there IS a
    // minimum below which the implicit cache does not engage — so `Automatic`
    // carries a minimum rather than claiming to have none.
    CatalogueEntry {
        id: "gemini-3.8-flash",
        display_name: "Gemini 3.8 Flash",
        provider: "google",
        context_window: 1_048_576,
        max_output: 65_536,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        cache: CacheSupport::Automatic { min_tokens: 4_096 },
    },
    CatalogueEntry {
        // ⚠ `-preview` is the ONLY id this model ships under. `gemini-3.1-pro`
        // returns 404 from Google — caught by the live smoke on its first run,
        // and by nothing else: a fixture test would have gone on passing
        // against a captured payload forever.
        id: "gemini-3.1-pro-preview",
        display_name: "Gemini 3.1 Pro",
        provider: "google",
        context_window: 1_000_000,
        max_output: 65_536,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        // Carried from Gemini 2.5 Pro pending P2-65.
        cache: CacheSupport::Automatic { min_tokens: 2_048 },
    },
    CatalogueEntry {
        // razi-ruled 2026-09-02: 3.7 Flash replaces 3.5 Flash — newer, and
        // half the price at the current published rate.
        id: "gemini-3.7-flash",
        display_name: "Gemini 3.7 Flash",
        provider: "google",
        context_window: 1_000_000,
        max_output: 65_536,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        // Carried from the Gemini 2.5 Flash line pending P2-65.
        cache: CacheSupport::Automatic { min_tokens: 1_024 },
    },
    // ── Anthropic ────────────────────────────────────────────────────────────
    CatalogueEntry {
        id: "claude-opus-5",
        display_name: "Claude Opus 5",
        provider: "anthropic",
        context_window: 200_000,
        max_output: 64_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        // Opus family minimum. This is the one that makes Delegate mode's
        // ~461-token prefix uncacheable by construction (P2-65).
        cache: CacheSupport::Explicit { min_tokens: 4_096 },
    },
    CatalogueEntry {
        id: "claude-sonnet-5",
        display_name: "Claude Sonnet 5",
        provider: "anthropic",
        context_window: 200_000,
        max_output: 64_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        // Documented for the Sonnet line, and a quarter of the Opus figure —
        // the one shipped model where a modest prefix can actually cache.
        cache: CacheSupport::Explicit { min_tokens: 1_024 },
    },
    CatalogueEntry {
        id: "claude-fable-5",
        display_name: "Claude Fable 5",
        provider: "anthropic",
        context_window: 200_000,
        max_output: 64_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        // No published figure and no predecessor to carry from — this is a new
        // line. Taken at the family ceiling on the rule above: warn early
        // rather than promise a cache that never forms.
        cache: CacheSupport::Explicit { min_tokens: 4_096 },
    },
    CatalogueEntry {
        id: "claude-haiku-4-5",
        display_name: "Claude Haiku 4.5",
        provider: "anthropic",
        context_window: 200_000,
        max_output: 64_000,
        vision: true,
        reasoning: false,
        parallel_tools: true,
        // Documented at 4,096 — the small model has the LARGE minimum, which
        // inverts the intuition and is why this is a table and not a formula.
        cache: CacheSupport::Explicit { min_tokens: 4_096 },
    },
    // ── OpenAI ───────────────────────────────────────────────────────────────
    //
    // OpenAI caches automatically and bills prior tokens regardless, which is
    // why `CachePolicy::Handle` is not adopted here (see `usage.rs`).
    CatalogueEntry {
        id: "gpt-5.5",
        display_name: "GPT-5.5",
        provider: "openai",
        context_window: 400_000,
        max_output: 128_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        cache: CacheSupport::Automatic { min_tokens: 1_024 },
    },
    CatalogueEntry {
        id: "gpt-5.4",
        display_name: "GPT-5.4",
        provider: "openai",
        context_window: 400_000,
        max_output: 128_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        cache: CacheSupport::Automatic { min_tokens: 1_024 },
    },
    CatalogueEntry {
        id: "gpt-5.4-mini",
        display_name: "GPT-5.4 Mini",
        provider: "openai",
        context_window: 400_000,
        max_output: 128_000,
        vision: true,
        reasoning: true,
        parallel_tools: true,
        cache: CacheSupport::Automatic { min_tokens: 1_024 },
    },
    CatalogueEntry {
        id: "gpt-5.4-nano",
        display_name: "GPT-5.4 Nano",
        provider: "openai",
        context_window: 400_000,
        max_output: 128_000,
        vision: true,
        reasoning: false,
        parallel_tools: true,
        cache: CacheSupport::Automatic { min_tokens: 1_024 },
    },
];

/// Models offered without a verified price.
///
/// ⚠ **An unpriced model bills at $0**, which means it is invisible to the
/// daily spend ceiling and effectively runs uncapped. That is a real hole, not
/// a rounding error, and it is listed here rather than left to a sentence in a
/// comment so a test can hold it.
///
/// `cost/pricing.rs` deliberately refuses to invent a rate — a fabricated price
/// is worse than none, because it looks metered.
///
/// **Currently empty, and that is the intended state.** `gpt-5.5-pro` and
/// `gpt-5.3-codex` sat here on 2026-09-02; razi-ruled the same day to stop
/// offering them rather than ship a model whose spend the cap cannot see. The
/// list stays as the mechanism, so the next unpriced model is a declared
/// decision instead of a silent $0.
pub const UNPRICED: &[&str] = &[];

/// The catalogue row for `model`, or `None` if the app does not offer it.
pub fn entry(model: &str) -> Option<&'static CatalogueEntry> {
    CATALOGUE.iter().find(|e| e.id == model)
}

/// Every model served by `provider`, as `ModelInfo`.
///
/// Prices are filled from `cost::pricing` so the two tables cannot disagree.
/// A model with no verified price reports `0.0` rather than a guess — the
/// pricing module treats an absent price as an error and says so there; a
/// plausible fabricated number here would bury it.
/// The model to use for `provider` when nothing better is known.
///
/// P3-61. A subagent inherited a hardcoded **Anthropic** id regardless of the
/// active provider, so a Google-primary install sent Anthropic ids to Google on
/// every default subagent. Any fallback that is not provider-aware has the same
/// defect, so the fallback lives here -- beside the table that knows which
/// models each provider actually serves.
///
/// The first catalogue entry for a provider is its default: entries are listed
/// most-capable-first within each provider, and `models_for` preserves that
/// order. Returns `None` for a provider with no entries, which is a
/// configuration error rather than something to paper over with a guess.
pub fn default_model_for(provider: &str) -> Option<&'static str> {
    CATALOGUE
        .iter()
        .find(|e| e.provider == provider)
        .map(|e| e.id)
}

pub fn models_for(provider: &str) -> Vec<ModelInfo> {
    CATALOGUE
        .iter()
        .filter(|e| e.provider == provider)
        .map(|e| {
            let price = crate::cost::pricing::price_for(e.id).ok();
            ModelInfo {
                id: e.id.to_string(),
                name: e.display_name.to_string(),
                context_window: e.context_window,
                supports_vision: e.vision,
                supports_thinking: e.reasoning,
                cost_in_per_1m: price.map(|p| p.input).unwrap_or(0.0),
                cost_out_per_1m: price.map(|p| p.output).unwrap_or(0.0),
            }
        })
        .collect()
}

/// Capabilities for `model` (`P2-03`), keyed by **model**, not provider.
///
/// An unknown model returns the most conservative record rather than a
/// permissive default: no caching, no parallel tools, the smallest window we
/// ship. Guessing generously here would have the agent send a request the model
/// cannot serve — and `cache: None` means an unknown model never claims a cache
/// that will not form.
pub fn capabilities_for(model: &str) -> ModelCapabilities {
    match entry(model) {
        Some(e) => ModelCapabilities {
            streaming: true,
            tools: true,
            parallel_tools: e.parallel_tools,
            vision: e.vision,
            reasoning: e.reasoning,
            cache: e.cache.clone(),
            context_window: e.context_window,
            max_output: e.max_output,
        },
        None => ModelCapabilities {
            streaming: true,
            tools: true,
            parallel_tools: false,
            vision: false,
            reasoning: false,
            cache: CacheSupport::None,
            context_window: 200_000,
            max_output: 8_192,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalogued_model_routes_to_its_own_provider() {
        // The catalogue and `routing.rs` are two answers to "who serves this
        // model". They must be the same answer.
        for e in CATALOGUE {
            assert_eq!(
                super::super::routing::provider_for_model(e.id),
                Some(e.provider),
                "{} is catalogued under {} but routes elsewhere",
                e.id,
                e.provider
            );
        }
    }

    #[test]
    fn every_catalogued_model_is_priced_or_declared_unpriced() {
        // A model on offer with no price records spend as **$0** — so it is
        // invisible to the daily cap and runs uncapped. `TG-P2-02` accepts a
        // model whose id merely appears in `pricing.rs` prose; that is too
        // weak a check to rest this on, because a mention in a comment is not
        // a decision. `UNPRICED` below is the decision, in code.
        for e in CATALOGUE {
            let priced = crate::cost::pricing::price_for(e.id).is_ok();
            assert!(
                priced || UNPRICED.contains(&e.id),
                "{} is offered, unpriced, and not declared in UNPRICED — it would \
                 bill at $0 and run uncapped",
                e.id
            );
        }
    }

    #[test]
    fn the_unpriced_list_does_not_outlive_its_reason() {
        // Guards the other direction: once a price lands, the exemption must go,
        // or a real price sits behind a stale waiver.
        for id in UNPRICED {
            assert!(
                crate::cost::pricing::price_for(id).is_err(),
                "{id} is priced now — remove it from UNPRICED"
            );
            assert!(
                entry(id).is_some(),
                "{id} is exempted but not offered — drop the entry"
            );
        }
    }

    #[test]
    fn the_retired_generation_is_gone() {
        // These were retired in February 2026 and were still being offered by
        // the adapters on 2026-09-02. Selecting one is a call that cannot
        // succeed, not a degraded choice.
        for dead in [
            "gpt-4o",
            "gpt-4.1",
            "gpt-4.1-mini",
            "o4-mini",
            "claude-opus-4-5",
            "claude-sonnet-4-5",
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.0-flash-thinking-exp-01-21",
            // Never existed at all. It was in the picker and the price table on
            // 2026-09-02 and Google 404s it — the id is `-preview`.
            "gemini-3.1-pro",
            // Superseded by 3.7 Flash by ruling on 2026-09-02.
            "gemini-3.5-flash",
            // Removed the same day: offered, unpriced, and therefore uncapped.
            "gpt-5.5-pro",
            "gpt-5.3-codex",
        ] {
            assert!(
                entry(dead).is_none(),
                "{dead} is retired and must not be offered"
            );
        }
    }

    #[test]
    fn geminis_full_window_is_reachable() {
        // The bug this table was built for: `P2-57` reads the window from the
        // catalogue, and against the stale list it found nothing for
        // `gemini-3.1-pro` and fell back to 200k — compacting at a fifth of the
        // real window while looking fixed.
        //
        // 🔴 `P3-59` — **my own defect, from P2.** This test asserted on
        // `capabilities_for(..).context_window`, which is
        // `ModelCapabilities::context_window`. Production reads
        // `ModelInfo::context_window`, reached through `provider.models()` →
        // `models_static()` → `models_for()`. **Two different fields.** They
        // agree only because both derive from the same `CatalogueEntry`, so a
        // regression in `models_for` alone would have left this green while
        // Gemini silently compacted at 200k again.
        //
        // That is `L-C6` — a check written from the claim rather than the
        // invariant — inside the very test written to stop `L-18` recurring.
        // It now walks the production path.
        let window_of = |id: &str| -> Option<u32> {
            models_for("google")
                .into_iter()
                .find(|m| m.id == id)
                .map(|m| m.context_window)
        };

        assert_eq!(
            window_of("gemini-3.1-pro-preview"),
            Some(1_000_000),
            "the field PRODUCTION reads must carry the full window"
        );
        assert_eq!(window_of("gemini-3.7-flash"), Some(1_000_000));

        // The two fields must also agree, since both are shipped. A drift
        // between them is a real defect even though production reads only one.
        assert_eq!(
            capabilities_for("gemini-3.1-pro-preview").context_window,
            window_of("gemini-3.1-pro-preview").unwrap(),
            "ModelCapabilities and ModelInfo disagree about the same model's window"
        );

        // And the fallback production uses for an unknown model is still the
        // smallest window we ship, so a lookup that silently breaks compacts
        // early rather than overflowing.
        assert!(
            models_for("google").iter().all(|m| m.id != "not-a-model"),
            "the unknown-model probe must actually be unknown"
        );
        assert_eq!(capabilities_for("not-a-model").context_window, 200_000);
    }

    #[test]
    fn capabilities_vary_within_a_vendor_which_is_why_this_is_keyed_by_model() {
        // If capability were a provider-level answer, these would be equal.
        let sonnet = capabilities_for("claude-sonnet-5").cache_minimum();
        let opus = capabilities_for("claude-opus-5").cache_minimum();
        assert_eq!(sonnet, Some(1_024));
        assert_eq!(opus, Some(4_096));
        assert_ne!(
            sonnet, opus,
            "a provider-level answer would be wrong for one of these"
        );
        // And reasoning differs inside OpenAI too.
        assert!(capabilities_for("gpt-5.4").reasoning);
        assert!(!capabilities_for("gpt-5.4-nano").reasoning);
    }

    #[test]
    fn an_unknown_model_gets_the_conservative_record_never_a_permissive_one() {
        let c = capabilities_for("some-model-shipped-next-year");
        assert_eq!(
            c.cache,
            CacheSupport::None,
            "must not claim a cache that will not form"
        );
        assert!(!c.parallel_tools);
        assert!(!c.vision);
        assert_eq!(c.cache_minimum(), None);
    }

    #[test]
    fn each_provider_serves_the_models_it_is_supposed_to() {
        assert_eq!(models_for("google").len(), 3);
        assert!(models_for("google")
            .iter()
            .any(|model| model.id == "gemini-3.8-flash"));
        assert_eq!(models_for("anthropic").len(), 4);
        // Four, not six: gpt-5.5-pro and gpt-5.3-codex were removed on
        // 2026-09-02 because no verified price existed for them, and an
        // unpriced model runs invisible to the daily spend cap.
        assert_eq!(models_for("openai").len(), 4);
        assert_eq!(models_for("nonexistent").len(), 0);
        // And every ModelInfo carries a real window, not a zero.
        for p in ["google", "anthropic", "openai"] {
            for m in models_for(p) {
                assert!(m.context_window > 0, "{} has no context window", m.id);
                assert!(!m.name.is_empty(), "{} has no display name", m.id);
            }
        }
    }
}
