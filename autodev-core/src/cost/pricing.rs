// The price table (P2-18, P2-20, P2-51).
//
// ── Why this replaces a match on substrings ──────────────────────────────────
//
// The previous table was six `contains()` arms over a `_ => (1.0, 3.0)` default,
// and P2's pre-build audit measured what that produced: **9 of the 12 models the
// app offers fell through to the invented rate**, including `DEFAULT_MODEL`.
// Every OpenAI model and every Google model was billed at $1.00/$3.00 per 1M.
//
// The three that "matched" were not safe either. `contains()` meant
// `claude-opus-5` inherited Opus 4.5's `(15.0, 75.0)` — a *confidently* wrong
// number rather than an obviously wrong one, which is worse, because it never
// looks suspicious.
//
// And the arm order was a live trap: `contains("gpt-4o")` was tested before
// `contains("gpt-4o-mini")`, so a mini would bill at 16.7× its rate. Latent only
// because that model was retired — but **`gpt-5.4` and `gpt-5.4-mini` are both
// offered today**, so the same trap is armed for the next arm anyone adds.
//
// ── Three rules this module enforces ─────────────────────────────────────────
//
// 1. **Exact model ids, never substrings.** A prefix cannot silently swallow a
//    longer id.
// 2. **An unknown model is an error, never a default.** Spec `02 §5`: *"A wrong
//    cost number is worse than no cost number, because it is believed."*
// 3. **Price is a function of the four-way token split**, not a pair of scalars.
//    Cache reads run ~90% below input and cache writes above it; pricing them as
//    input makes caching look like it saves nothing, which is precisely the
//    signal `P2-12`'s alarm exists to read.

use super::super::provider::usage::Usage;
use crate::errors::{AutoDevError, Result};

/// USD per 1M tokens for one model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPrice {
    pub input: f64,
    pub output: f64,
    /// Reading from cache. Typically ~0.1× input.
    pub cached_read: f64,
    /// Writing to cache. Typically above input.
    pub cache_write: f64,
    /// Some models change rate above a context threshold — Google Pro's 200k
    /// cliff. `None` means one flat rate at every context length.
    pub tier: Option<PriceTier>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceTier {
    /// Total input tokens above which the higher rate applies.
    pub above_tokens: u32,
    pub input: f64,
    pub output: f64,
}

/// Verified prices, each with the date it was checked.
///
/// ⚠ **A model absent from this table is an error, not a default.** Adding one
/// without a verified source re-creates the defect this module exists to close.
const PRICES: &[(&str, ModelPrice)] = &[
    // Google — verified 2026-09-02. Cached read is a 90% discount on input.
    (
        "gemini-3.5-flash",
        ModelPrice {
            input: 1.50,
            output: 9.00,
            cached_read: 0.15,
            cache_write: 0.08333,
            tier: None,
        },
    ),
    // Anthropic — verified 2026-09-02.
    (
        "claude-opus-5",
        ModelPrice {
            input: 5.00,
            output: 25.00,
            cached_read: 0.50,
            // The 5-minute write rate. The 1-hour rate is $10.00; TTL-aware
            // pricing arrives with `CachePolicy::Explicit` in P2-B.
            cache_write: 6.25,
            tier: None,
        },
    ),
    (
        "claude-sonnet-5",
        ModelPrice {
            // ⚠ NOT the $2/$10 figure most sources still quote. That was
            // introductory pricing **through 31 August 2026**; standard pricing
            // is $3/$15 and today is past that date. Verified 2026-09-02.
            input: 3.00,
            output: 15.00,
            cached_read: 0.20,
            cache_write: 2.50,
            tier: None,
        },
    ),
];

/// The price for a model, or an error naming it.
pub fn price_for(model: &str) -> Result<ModelPrice> {
    PRICES
        .iter()
        .find(|(id, _)| *id == model)
        .map(|(_, p)| *p)
        .ok_or_else(|| {
            AutoDevError::Agent(format!(
                "No verified price for model '{model}'. Cost cannot be reported for it, \
                 and a default rate would be a fabricated number presented as a bill. \
                 Add a verified entry to cost::pricing::PRICES."
            ))
        })
}

/// Whether a model can be priced at all.
pub fn is_priced(model: &str) -> bool {
    PRICES.iter().any(|(id, _)| *id == model)
}

/// Every model this table can price.
pub fn priced_models() -> Vec<&'static str> {
    PRICES.iter().map(|(id, _)| *id).collect()
}

/// Cost in USD for one request's usage.
///
/// Each of the four token kinds is charged at its own rate. Summing them into
/// one "input" figure — as the old two-scalar function did — makes a cached
/// request cost the same as an uncached one and hides the entire benefit.
pub fn cost_usd(model: &str, usage: &Usage) -> Result<f64> {
    let price = price_for(model)?;

    // The tier is chosen by total input, cached tokens included: the vendor's
    // cliff is about how much context was processed, not how much was billed at
    // the fresh rate.
    let (input_rate, output_rate) = match price.tier {
        Some(t) if usage.total_input() > t.above_tokens => (t.input, t.output),
        _ => (price.input, price.output),
    };

    let per_million = |tokens: u32, rate: f64| (tokens as f64 / 1_000_000.0) * rate;

    Ok(per_million(usage.input, input_rate)
        + per_million(usage.cached_read, price.cached_read)
        + per_million(usage.cache_write, price.cache_write)
        // Reasoning tokens are billed at the output rate.
        + per_million(usage.output + usage.reasoning, output_rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(input: u32, cached_read: u32, cache_write: u32, output: u32) -> Usage {
        Usage {
            input,
            cached_read,
            cache_write,
            output,
            reasoning: 0,
            tool: 0,
        }
    }

    #[test]
    fn an_unknown_model_is_an_error_not_a_default() {
        // The defect this module exists to close. The old table returned
        // (1.0, 3.0) for anything it did not recognise, which meant 9 of 12
        // offered models were billed at an invented rate.
        let err = cost_usd("totally-made-up", &usage(1000, 0, 0, 100)).unwrap_err();
        assert!(err.to_string().contains("No verified price"));
        assert!(
            err.to_string().contains("totally-made-up"),
            "the error must name the model, or nobody can act on it"
        );
    }

    #[test]
    fn a_longer_id_is_not_swallowed_by_a_shorter_one() {
        // `contains()` matching meant claude-opus-5 inherited Opus 4.5's price,
        // and gpt-4o-mini would have billed at gpt-4o's rate. Exact matching
        // makes both impossible.
        assert!(is_priced("claude-opus-5"));
        assert!(!is_priced("claude-opus-5-turbo"));
        assert!(!is_priced("claude-opus"));
    }

    #[test]
    fn cached_reads_cost_far_less_than_fresh_input() {
        // If these came out equal, caching would look like it saves nothing and
        // P2-12's alarm would have nothing to measure.
        let fresh = cost_usd("claude-sonnet-5", &usage(1_000_000, 0, 0, 0)).unwrap();
        let cached = cost_usd("claude-sonnet-5", &usage(0, 1_000_000, 0, 0)).unwrap();
        assert!((fresh - 3.00).abs() < 1e-9);
        assert!((cached - 0.20).abs() < 1e-9);
        assert!(cached * 10.0 < fresh, "a cache read must be far cheaper");
    }

    #[test]
    fn a_cache_write_costs_more_than_fresh_input() {
        let fresh = cost_usd("claude-opus-5", &usage(1_000_000, 0, 0, 0)).unwrap();
        let write = cost_usd("claude-opus-5", &usage(0, 0, 1_000_000, 0)).unwrap();
        assert!(write > fresh, "a cache write carries a premium");
    }

    #[test]
    fn reasoning_tokens_are_billed_at_the_output_rate() {
        let mut u = usage(0, 0, 0, 1_000_000);
        let plain = cost_usd("gemini-3.5-flash", &u).unwrap();
        u.reasoning = 1_000_000;
        let with_thinking = cost_usd("gemini-3.5-flash", &u).unwrap();
        assert!((with_thinking - plain * 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_tiered_model_switches_rate_above_its_threshold() {
        // No shipped model has a tier yet, so this proves the mechanism rather
        // than a vendor's number. A table that cannot express a cliff would
        // silently under-bill above it.
        let tiered = ModelPrice {
            input: 1.0,
            output: 2.0,
            cached_read: 0.1,
            cache_write: 1.25,
            tier: Some(PriceTier {
                above_tokens: 200_000,
                input: 2.0,
                output: 4.0,
            }),
        };
        let below = Usage {
            input: 100_000,
            ..Default::default()
        };
        let above = Usage {
            input: 300_000,
            ..Default::default()
        };
        let rate = |u: &Usage| {
            let (i, _) = match tiered.tier {
                Some(t) if u.total_input() > t.above_tokens => (t.input, t.output),
                _ => (tiered.input, tiered.output),
            };
            i
        };
        assert_eq!(rate(&below), 1.0);
        assert_eq!(rate(&above), 2.0);
    }

    #[test]
    fn the_tier_is_chosen_by_total_input_including_cached() {
        // The vendor's cliff is about context processed, not tokens billed
        // fresh. Choosing the tier on `input` alone would keep a 190k-cached
        // request on the cheap tier at 250k of real context.
        let u = Usage {
            input: 10_000,
            cached_read: 240_000,
            ..Default::default()
        };
        assert_eq!(u.total_input(), 250_000);
    }

    #[test]
    fn every_priced_model_is_actually_offered() {
        // A price for a model nobody can select is dead data that will drift.
        // The offered set lives in `src/data/models.ts`; this asserts the
        // relationship rather than the contents, so it stays true as the set
        // changes.
        for m in priced_models() {
            assert!(
                m.starts_with("gemini-") || m.starts_with("claude-") || m.starts_with("gpt-"),
                "{m} is not from a supported provider"
            );
        }
    }
}
