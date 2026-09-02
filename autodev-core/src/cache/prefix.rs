// The one prefix builder (P2-13, P2-14, P2-15).
//
// ── The invariant ────────────────────────────────────────────────────────────
//
// Anthropic caches on an **exact byte match** of everything up to the
// breakpoint, in the order `tools → system → messages`
// (`G6-agents-sdk-research.md:352`). One changed byte anywhere before the
// breakpoint invalidates everything after it, and the provider reports this the
// same way it reports success: it does not.
//
// So there is exactly one function that assembles that region, and it is this
// one. `context.rs` already carried a second builder — `build_system_prompt`,
// zero callers, reading AUTODEV.md from the filesystem while the live path read
// it from SQLite and injected memory. It produced a *different* prefix from the
// one actually sent, and it was deleted in `P2-62` precisely because whoever
// wired caching might have picked it and got a cache key that could never match
// the request.
//
// ── Stable and volatile are separated, not merged ────────────────────────────
//
// The live system prompt ends with a memory block built from
// `get_recent_summaries` and `recall_facts` — content that changes whenever the
// agent learns anything. Appending it to the system prompt is fine; including
// it *before the breakpoint* is not, because then every new memory invalidates
// the multi-KB base prompt in front of it.
//
// `CachePrefix` therefore carries the two regions apart, and the caller places
// the breakpoint between them. This is `P2-14`: the volatile content is not
// removed from the request, it is moved out of the cached region.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use tracing::warn;

use crate::provider::usage::ModelCapabilities;

/// A request's prompt, split at the cache breakpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePrefix {
    /// The bytes offered for caching. Must be identical across calls or
    /// nothing caches.
    pub stable: String,
    /// Content that changes between calls and therefore sits **after** the
    /// breakpoint. Still sent — just not cached.
    pub volatile: String,
    /// Hash of `stable`. The key the monitor tracks reuse by.
    pub hash: u64,
    /// Estimated token count of `stable`.
    ///
    /// ⚠ **An estimate, not a measurement.** `P2-65` replaces this with
    /// `count_tokens` per model. It is deliberately biased LOW (see
    /// `estimate_tokens`) so that a prefix near the boundary is reported as
    /// below the minimum rather than above it: a false "below" produces a
    /// warning, while a false "above" makes the monitor expect a cache hit
    /// that was never possible and raise a false alarm.
    pub estimated_tokens: u32,
}

/// Assemble the cacheable prefix.
///
/// `tools` is serialised first because that is the wire order Anthropic hashes
/// in; a tool list that reorders between calls invalidates the prefix on its
/// own, so it is sorted by name here rather than trusted to arrive stable.
pub fn build(
    tool_names_and_schemas: &[(String, String)],
    system_base: &str,
    instructions: &[String],
    volatile_blocks: &[String],
) -> CachePrefix {
    let mut stable = String::new();

    // ── tools ────────────────────────────────────────────────────────────────
    //
    // Sorted by name. The tool registry is a HashMap in several places, and
    // HashMap iteration order is not stable across processes — so an unsorted
    // list would produce a different prefix on every restart while looking
    // identical in a diff. This is the `BTreeMap`/`preserve_order` requirement
    // in P2-13, applied at the point it actually matters.
    let mut tools: Vec<&(String, String)> = tool_names_and_schemas.iter().collect();
    tools.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, schema) in tools {
        stable.push_str(name);
        stable.push('\u{1}');
        stable.push_str(schema);
        stable.push('\u{2}');
    }

    // ── system ───────────────────────────────────────────────────────────────
    stable.push_str(system_base);
    for block in instructions {
        stable.push_str("\n\n---\n\n");
        stable.push_str(block);
    }

    let volatile = volatile_blocks.join("\n\n---\n\n");

    let mut hasher = DefaultHasher::new();
    stable.hash(&mut hasher);

    CachePrefix {
        hash: hasher.finish(),
        estimated_tokens: estimate_tokens(&stable),
        stable,
        volatile,
    }
}

/// Rough token count for a string.
///
/// Four characters per token is the usual approximation for English and code.
/// It is used **only** to decide whether a prefix clears a model's minimum, and
/// it rounds DOWN: `P2-65` measures this properly with `count_tokens`, and
/// until then an under-estimate is the error that fails safe. Over-estimating
/// would tell the monitor to expect a cache hit on a prefix the provider
/// silently declined, producing an alarm with no fault behind it.
pub fn estimate_tokens(s: &str) -> u32 {
    (s.len() / 4) as u32
}

/// Whether this prefix can cache on this model, warning when it cannot
/// (`P2-15`).
///
/// Returns `true` when caching is worth requesting. **No provider returns an
/// error below its minimum** — the request succeeds and nothing caches — so
/// this warning is the only notice anyone gets.
pub fn will_cache(prefix: &CachePrefix, caps: &ModelCapabilities, model: &str) -> bool {
    let Some(minimum) = caps.cache_minimum() else {
        return false;
    };

    if prefix.estimated_tokens < minimum {
        warn!(
            "Prompt caching will NOT engage for {model}: the cacheable prefix is about \
             {} tokens against a {minimum}-token minimum. The provider returns no error \
             for this — the request will simply cost full price every time.",
            prefix.estimated_tokens
        );
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::usage::CacheSupport;

    fn tools() -> Vec<(String, String)> {
        vec![
            ("write_file".to_string(), r#"{"type":"object"}"#.to_string()),
            ("read_file".to_string(), r#"{"type":"object"}"#.to_string()),
            ("grep".to_string(), r#"{"type":"object"}"#.to_string()),
        ]
    }

    fn caps(min: u32) -> ModelCapabilities {
        ModelCapabilities {
            streaming: true,
            tools: true,
            parallel_tools: true,
            vision: false,
            reasoning: false,
            cache: CacheSupport::Explicit { min_tokens: min },
            context_window: 200_000,
            max_output: 8192,
        }
    }

    #[test]
    fn the_same_inputs_produce_the_same_bytes() {
        let a = build(&tools(), "base", &["inst".into()], &["memory".into()]);
        let b = build(&tools(), "base", &["inst".into()], &["memory".into()]);
        assert_eq!(a.hash, b.hash);
        assert_eq!(a.stable, b.stable);
    }

    #[test]
    fn tool_order_does_not_change_the_prefix() {
        // The registry is a HashMap in places, and HashMap order is not stable
        // ACROSS PROCESSES. Unsorted, the prefix would differ on every restart
        // while looking identical in a diff — a cache that mysteriously never
        // hits after a redeploy.
        let mut reordered = tools();
        reordered.reverse();
        let a = build(&tools(), "base", &[], &[]);
        let b = build(&reordered, "base", &[], &[]);
        assert_eq!(a.hash, b.hash, "tool order must not affect the cached prefix");
    }

    #[test]
    fn volatile_content_is_kept_out_of_the_hashed_region() {
        // P2-14. Two calls that differ only in remembered facts must share a
        // prefix — otherwise learning one thing invalidates the entire base
        // prompt in front of it.
        let a = build(&tools(), "base", &[], &["remembered: X".into()]);
        let b = build(&tools(), "base", &[], &["remembered: X and Y".into()]);
        assert_eq!(a.hash, b.hash, "a new memory must not invalidate the prefix");
        assert_ne!(a.volatile, b.volatile, "but the content must still be SENT");
        assert!(!a.stable.contains("remembered"));
    }

    #[test]
    fn a_changed_instruction_does_change_the_prefix() {
        // The other direction: instructions ARE part of the cached region, so
        // editing AUTODEV.md must produce a different key. A hash that ignored
        // them would serve a stale cached prefix that no longer matches what
        // the user asked for.
        let a = build(&tools(), "base", &["do X".into()], &[]);
        let b = build(&tools(), "base", &["do Y".into()], &[]);
        assert_ne!(a.hash, b.hash);
    }

    #[test]
    fn a_changed_tool_schema_changes_the_prefix() {
        let mut changed = tools();
        changed[0].1 = r#"{"type":"object","required":["path"]}"#.to_string();
        assert_ne!(build(&tools(), "b", &[], &[]).hash, build(&changed, "b", &[], &[]).hash);
    }

    #[test]
    fn tool_fields_cannot_be_confused_across_the_boundary() {
        // Concatenating name+schema without a separator lets ("ab","c") and
        // ("a","bc") hash identically. Contrived, but the failure would be a
        // cache serving the wrong tool schema, which is worth one separator.
        let x = vec![("ab".to_string(), "c".to_string())];
        let y = vec![("a".to_string(), "bc".to_string())];
        assert_ne!(build(&x, "", &[], &[]).hash, build(&y, "", &[], &[]).hash);
    }

    #[test]
    fn a_prefix_below_the_minimum_is_refused_rather_than_attempted() {
        // P2-15 / P2-65. Delegate mode's prompt is ~461 tokens against a 4,096
        // minimum on two of three shipped models. Requesting a cache there is
        // not an edge case that might work — it is a design-time certainty of
        // failure, and worse than not asking, because a cache WRITE is billed
        // above the input rate.
        let short = build(&[], &"x".repeat(1_800), &[], &[]);
        assert!(short.estimated_tokens < 4_096);
        assert!(!will_cache(&short, &caps(4_096), "claude-opus-5"));

        // The same prefix on a model with the lower minimum does cache.
        assert!(will_cache(&short, &caps(1), "claude-sonnet-5"));
    }

    #[test]
    fn a_model_with_no_cache_support_never_requests_one() {
        let p = build(&tools(), &"x".repeat(100_000), &[], &[]);
        let none = ModelCapabilities {
            cache: CacheSupport::None,
            ..caps(1024)
        };
        assert!(!will_cache(&p, &none, "some-model"));
    }

    #[test]
    fn the_token_estimate_rounds_down() {
        // Stated as a property because the direction of the error matters: an
        // over-estimate makes the monitor expect a hit that was never possible
        // and raise an alarm with no fault behind it.
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abc"), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefg"), 1);
    }
}
