// Token usage, cache policy, error classification and per-model capabilities.
//
// P2-01 · spec `02 §2`. These are the types the provider trait v2 is built from,
// landed before the adapters so no adapter is written twice.
//
// ── Why six usage fields and not two ─────────────────────────────────────────
//
// The shipped code carried `tokens_in` and `tokens_out`. Cache reads are priced
// at a fraction of input, cache writes at a premium over it, and reasoning
// tokens are billed but are not output. A two-field split cannot produce a
// correct invoice for any provider that caches — it can only produce a number
// that looks like one.
//
// ── The normalisation invariant, which is the load-bearing part ──────────────
//
// The vendors disagree about what "input" counts, and they disagree silently:
//
//     Anthropic         input EXCLUDES cached tokens
//     OpenAI, Google    cached tokens are reported INSIDE the prompt total
//
// Applying one vendor's arithmetic to another double-counts cached tokens,
// over-reports cost, and — worse — breaks the cache-hit alarm, because the
// alarm's whole job is to notice when `cached_read` is zero.
//
// So this module fixes one meaning and makes the **adapter** responsible for
// reaching it (spec `02 §4.2`: *"the adapter — not the runtime — owns the
// normalisation"*):
//
//     ▸ `input` is ALWAYS non-cached input tokens.
//     ▸ therefore `total_input() == input + cached_read + cache_write`, on every
//       provider, with no per-provider branch anywhere above this line.
//
// An adapter whose vendor includes cached tokens in its prompt total MUST
// subtract them before constructing a `Usage`. `Usage::from_inclusive_input`
// exists so that subtraction is written once and named, rather than open-coded
// in three adapters and mis-copied into the fourth.

use serde::{Deserialize, Serialize};

/// Provider-reported token counts for one request. Never estimated (INV-7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// **Non-cached** input tokens, after adapter normalisation. See the module
    /// note: this is the field the vendors disagree about.
    pub input: u32,
    /// Tokens served from cache. Priced at a fraction of `input`.
    ///
    /// This is the cache-hit alarm's instrument (spec `02 §4.2`). Across
    /// repeated requests with an identical prefix it must be non-zero from the
    /// second request onward; zero means a silent invalidator.
    pub cached_read: u32,
    /// Tokens written into the cache. Priced at a premium over `input`.
    pub cache_write: u32,
    pub output: u32,
    /// Billed, but not part of `output`. Zero on providers without reasoning.
    pub reasoning: u32,
    /// Tokens attributed to tool definitions and results, where the provider
    /// reports them separately.
    pub tool: u32,
}

impl Usage {
    /// Every input token the request was billed for, cached or not.
    ///
    /// Correct on all three providers **because** `input` excludes cached
    /// tokens by this module's invariant. If an adapter forgets to normalise,
    /// this over-counts — which is why `from_inclusive_input` exists.
    pub fn total_input(&self) -> u32 {
        self.input
            .saturating_add(self.cached_read)
            .saturating_add(self.cache_write)
    }

    /// Every token billed, in either direction.
    pub fn total(&self) -> u32 {
        self.total_input()
            .saturating_add(self.output)
            .saturating_add(self.reasoning)
    }

    /// Build from a vendor that reports a prompt total **including** cached
    /// tokens — OpenAI and Google.
    ///
    /// Saturating, not wrapping: if a vendor ever reports more cached tokens
    /// than prompt tokens, the honest answer is zero non-cached input, not four
    /// billion. A panic here would take down a turn over an accounting quirk,
    /// and a wrap would produce a cost figure someone would believe.
    pub fn from_inclusive_input(
        prompt_total: u32,
        cached_read: u32,
        cache_write: u32,
        output: u32,
    ) -> Self {
        Self {
            input: prompt_total.saturating_sub(cached_read),
            cached_read,
            cache_write,
            output,
            reasoning: 0,
            tool: 0,
        }
    }

    /// Attach reasoning tokens, which are billed but are not output.
    ///
    /// A builder rather than a field in `from_inclusive_input`: only some
    /// providers report them, and threading an `Option` through the common
    /// constructor would make every call site carry a `None` it does not mean.
    pub fn with_reasoning(mut self, reasoning: u32) -> Self {
        self.reasoning = reasoning;
        self
    }

    /// Accumulate across the turns of a session.
    pub fn add(&mut self, other: &Usage) {
        self.input = self.input.saturating_add(other.input);
        self.cached_read = self.cached_read.saturating_add(other.cached_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
        self.output = self.output.saturating_add(other.output);
        self.reasoning = self.reasoning.saturating_add(other.reasoning);
        self.tool = self.tool.saturating_add(other.tool);
    }

    /// Whether this request read anything from cache — the alarm's predicate.
    pub fn is_cache_hit(&self) -> bool {
        self.cached_read > 0
    }
}

/// How a provider is asked to cache, per spec `02 §2`.
///
/// ⚠ **Four variants because the vendors genuinely differ, not for generality.**
/// These are **policy shapes**, and providers share them — removing a provider
/// removes a *user* of a variant, not the variant. Collapsing this to a boolean
/// loses the feature on at least two providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePolicy {
    /// No caching requested.
    Off,
    /// The vendor caches on its own; we only steer affinity so repeated
    /// requests land on the same prefix. OpenAI works this way.
    Automatic { affinity_key: String },
    /// We mark the breakpoints ourselves. Anthropic: at most four, with
    /// per-model minimums below which nothing caches and no error is returned.
    Explicit {
        breakpoints: Vec<usize>,
        ttl: CacheTtl,
    },
    /// Server-side state referenced by handle. Adopted only where it actually
    /// saves — OpenAI bills prior tokens regardless, so it does not there.
    Handle { id: String },
}

impl Default for CachePolicy {
    fn default() -> Self {
        CachePolicy::Off
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheTtl {
    /// 1.25× write cost.
    FiveMinutes,
    /// 2.0× write cost.
    OneHour,
}

/// How an error should be treated, per spec `02 §3`.
///
/// ⚠ `QuotaExhausted` is **explicitly non-retryable**. A spend-cap 429 carries
/// no `retry-after` and never succeeds on retry; treating it as `Retryable`
/// turns a hard stop into a spend loop against a cap that already fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClass {
    Retryable,
    /// Anthropic returns **529** for this, not 503.
    Overloaded,
    QuotaExhausted,
    Deferred,
    Fatal,
}

impl ErrorClass {
    pub fn is_retryable(&self) -> bool {
        matches!(self, ErrorClass::Retryable | ErrorClass::Overloaded)
    }
}

/// Why the model stopped, normalised across vendors (P2-10).
///
/// ── Why this type exists ─────────────────────────────────────────────────────
///
/// `CompletionResponse::stop_reason` is a raw `String` carrying whatever the
/// vendor put on the wire, and the three vendors do not agree on any of it:
///
/// | meaning | Anthropic | OpenAI | Google |
/// |---|---|---|---|
/// | ran out of output budget | `max_tokens` | `length` | `MAX_TOKENS` |
/// | finished normally | `end_turn` | `stop` | `STOP` |
/// | wants to call a tool | `tool_use` | `tool_calls` | *(absent)* |
/// | cut by a safety filter | *(absent)* | `content_filter` | `SAFETY`, `RECITATION`, … |
///
/// The docstring on that field claimed the vocabulary was
/// `"end_turn" | "tool_use" | "max_tokens"` — Anthropic's three values,
/// presented as if they were universal. Any caller that believed it read an
/// OpenAI or Google truncation as a normal completion.
///
/// **Truncation is the case that matters.** A `max_tokens` stop means the
/// assistant message is cut mid-sentence — and possibly mid-`tool_use`, with
/// arguments that will not parse. Continuing the loop on a truncated turn feeds
/// a malformed message back to the model. The raw string is kept alongside this
/// enum, because `Other` has to stay diagnosable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    /// The model finished on its own.
    EndTurn,
    /// Output budget exhausted — **the content is truncated**.
    MaxTokens,
    /// The model is asking for one or more tools.
    ToolUse,
    /// A caller-supplied stop sequence matched.
    StopSequence,
    /// A safety or policy filter cut the response.
    ContentFilter,
    /// Anything else, kept verbatim so it stays diagnosable.
    Other(String),
}

impl StopReason {
    /// Classify a raw wire value. `provider` is carried into `Other` so an
    /// unrecognised value stays attributable in a log line.
    ///
    /// Matching is case-insensitive: Google SCREAMING_CASEs its values while
    /// the other two use lower_snake.
    pub fn parse(provider: &str, raw: &str) -> Self {
        match raw.to_ascii_lowercase().as_str() {
            // Truncation. Three spellings, one meaning.
            "max_tokens" | "length" | "max_output_tokens" => StopReason::MaxTokens,
            "end_turn" | "stop" => StopReason::EndTurn,
            "tool_use" | "tool_calls" | "function_call" => StopReason::ToolUse,
            "stop_sequence" => StopReason::StopSequence,
            "content_filter" | "safety" | "recitation" | "prohibited_content"
            | "blocklist" | "spii" => StopReason::ContentFilter,
            // Gemini emits this when it tried to call a tool and produced
            // arguments that do not parse. It is a tool intent that FAILED, not
            // a clean end — mapping it to `EndTurn` would end the loop as
            // though the model were satisfied, and mapping it to `ToolUse`
            // would send the loop looking for a call it cannot run.
            "malformed_function_call" => StopReason::Other(format!("{provider}:{raw}")),
            "" => StopReason::Other(format!("{provider}:<empty>")),
            _ => StopReason::Other(format!("{provider}:{raw}")),
        }
    }

    /// Whether the content that arrived with this stop is **incomplete**.
    pub fn is_truncated(&self) -> bool {
        matches!(self, StopReason::MaxTokens)
    }

    /// Whether the loop should continue because the model wants a tool.
    pub fn wants_tools(&self) -> bool {
        matches!(self, StopReason::ToolUse)
    }
}

/// What one **model** can do. Keyed by model, not provider (P2-03): capability
/// varies per model within a vendor, so a provider-level answer is wrong for
/// most of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub tools: bool,
    /// Whether more than one tool call may be returned in a single turn.
    pub parallel_tools: bool,
    pub vision: bool,
    pub reasoning: bool,
    /// The cache shape this model supports, if any.
    pub cache: CacheSupport,
    /// Total context window, used by compaction rather than a hardcoded number.
    pub context_window: u32,
    /// Maximum tokens the model will emit.
    pub max_output: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheSupport {
    None,
    /// The vendor caches on its own — there is no breakpoint for us to place.
    ///
    /// It still has a **minimum**. Gemini's implicit cache and OpenAI's
    /// automatic caching both decline to engage below a prefix length, and
    /// both do it silently, exactly as Anthropic does. `Automatic` originally
    /// carried no minimum on the belief that "the vendor handles it"; that
    /// made a short OpenAI prefix report as fully cacheable when nothing
    /// would ever cache.
    Automatic { min_tokens: u32 },
    /// We mark breakpoints, and **below `min_tokens` nothing caches and no
    /// error is returned** (spec `02 §4.3`). The prefix builder warns when the
    /// assembled prefix falls short, because the provider will not.
    Explicit { min_tokens: u32 },
}

impl ModelCapabilities {
    /// The cache minimum for this model, if it has one.
    pub fn cache_minimum(&self) -> Option<u32> {
        match self.cache {
            // Both shapes have a minimum and both fail silently below it. The
            // difference between them is who places the breakpoint, not
            // whether a short prefix caches.
            CacheSupport::Explicit { min_tokens } | CacheSupport::Automatic { min_tokens } => {
                Some(min_tokens)
            }
            CacheSupport::None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_input_sums_the_three_input_kinds() {
        let u = Usage {
            input: 100,
            cached_read: 900,
            cache_write: 50,
            output: 20,
            reasoning: 5,
            tool: 0,
        };
        assert_eq!(u.total_input(), 1050);
        assert_eq!(u.total(), 1075);
    }

    #[test]
    fn an_inclusive_vendor_is_normalised_to_the_exclusive_meaning() {
        // OpenAI/Google report a prompt total that ALREADY contains the cached
        // tokens. Storing that as `input` and then adding `cached_read` would
        // bill the cached tokens twice, at the uncached rate.
        let u = Usage::from_inclusive_input(1000, 900, 0, 20);
        assert_eq!(u.input, 100, "cached tokens must be removed from input");
        assert_eq!(
            u.total_input(),
            1000,
            "and the total must come back to what the vendor reported"
        );
    }

    #[test]
    fn an_exclusive_vendor_needs_no_adjustment() {
        // Anthropic already excludes cached tokens, so the fields are used as
        // reported and total_input reconstructs the true prompt size.
        let u = Usage {
            input: 100,
            cached_read: 900,
            ..Default::default()
        };
        assert_eq!(u.total_input(), 1000);
    }

    #[test]
    fn normalisation_saturates_rather_than_wrapping() {
        // If a vendor ever reports more cached tokens than prompt tokens, the
        // honest answer is zero non-cached input. Wrapping would produce ~4
        // billion, and a cost figure someone would believe.
        let u = Usage::from_inclusive_input(500, 900, 0, 10);
        assert_eq!(u.input, 0);
    }

    #[test]
    fn the_cache_alarm_predicate_reads_cached_read_only() {
        assert!(!Usage::default().is_cache_hit());
        assert!(Usage {
            cached_read: 1,
            ..Default::default()
        }
        .is_cache_hit());
        // A cache WRITE is not a hit. The first request of a session writes the
        // cache and reads nothing, and calling that a hit would make the alarm
        // green on exactly the turn it cannot yet be.
        assert!(!Usage {
            cache_write: 5000,
            ..Default::default()
        }
        .is_cache_hit());
    }

    #[test]
    fn accumulation_adds_every_field() {
        let mut a = Usage {
            input: 1,
            cached_read: 2,
            cache_write: 3,
            output: 4,
            reasoning: 5,
            tool: 6,
        };
        a.add(&a.clone());
        assert_eq!(
            a,
            Usage {
                input: 2,
                cached_read: 4,
                cache_write: 6,
                output: 8,
                reasoning: 10,
                tool: 12
            }
        );
    }

    #[test]
    fn accumulation_saturates_at_the_ceiling() {
        let mut a = Usage {
            input: u32::MAX,
            ..Default::default()
        };
        a.add(&Usage {
            input: 10,
            ..Default::default()
        });
        assert_eq!(a.input, u32::MAX, "a long session must not wrap to zero");
    }

    #[test]
    fn quota_exhausted_is_never_retryable() {
        // A spend-cap 429 carries no retry-after and never succeeds on retry.
        // Retrying it is a spend loop against a cap that already fired.
        assert!(!ErrorClass::QuotaExhausted.is_retryable());
        assert!(!ErrorClass::Fatal.is_retryable());
        assert!(ErrorClass::Retryable.is_retryable());
        assert!(ErrorClass::Overloaded.is_retryable());
    }

    #[test]
    fn a_cache_minimum_is_reported_for_every_shape_that_has_one() {
        let explicit = ModelCapabilities {
            streaming: true,
            tools: true,
            parallel_tools: true,
            vision: false,
            reasoning: false,
            cache: CacheSupport::Explicit { min_tokens: 2048 },
            context_window: 200_000,
            max_output: 8192,
        };
        assert_eq!(explicit.cache_minimum(), Some(2048));

        // ⚠ This asserted `None` until 2026-09-02, on the belief that a vendor
        // that caches automatically has no threshold to respect. It has one —
        // OpenAI declines below ~1,024 tokens and Gemini below its own figure,
        // both silently. Under the old semantic a 300-token OpenAI prefix
        // reported as having no minimum to fall short of, so the P2-15 warning
        // could never fire for two of the three providers.
        let automatic = ModelCapabilities {
            cache: CacheSupport::Automatic { min_tokens: 1_024 },
            ..explicit.clone()
        };
        assert_eq!(
            automatic.cache_minimum(),
            Some(1_024),
            "an automatic cache still has a floor, and still fails silently below it"
        );

        let none = ModelCapabilities {
            cache: CacheSupport::None,
            ..explicit
        };
        assert_eq!(none.cache_minimum(), None);
    }

    #[test]
    fn cache_policy_defaults_to_off() {
        // A default that caches would send cache_control to a provider the
        // caller never asked to cache on.
        assert_eq!(CachePolicy::default(), CachePolicy::Off);
    }
    // ── P2-10 · truncation is detectable on all three vendors ─────────────

    #[test]
    fn every_vendors_truncation_spelling_maps_to_max_tokens() {
        // The whole point: three wire values, one meaning. If any regresses to
        // `Other`, a truncated turn is fed back to the model as complete.
        assert_eq!(StopReason::parse("anthropic", "max_tokens"), StopReason::MaxTokens);
        assert_eq!(StopReason::parse("openai", "length"), StopReason::MaxTokens);
        assert_eq!(StopReason::parse("google", "MAX_TOKENS"), StopReason::MaxTokens);
        assert!(StopReason::parse("openai", "length").is_truncated());
        assert!(StopReason::parse("google", "MAX_TOKENS").is_truncated());
    }

    #[test]
    fn every_vendors_clean_finish_maps_to_end_turn() {
        assert_eq!(StopReason::parse("anthropic", "end_turn"), StopReason::EndTurn);
        assert_eq!(StopReason::parse("openai", "stop"), StopReason::EndTurn);
        assert_eq!(StopReason::parse("google", "STOP"), StopReason::EndTurn);
        assert!(!StopReason::parse("openai", "stop").is_truncated());
    }

    #[test]
    fn tool_intent_maps_across_the_two_vendors_that_signal_it() {
        assert_eq!(StopReason::parse("anthropic", "tool_use"), StopReason::ToolUse);
        assert_eq!(StopReason::parse("openai", "tool_calls"), StopReason::ToolUse);
        assert!(StopReason::parse("openai", "tool_calls").wants_tools());
        assert!(!StopReason::parse("openai", "stop").wants_tools());
    }

    #[test]
    fn google_safety_stops_are_a_filter_not_a_clean_end() {
        // Google has several of these and OpenAI one. Treating any as `EndTurn`
        // reports a blocked response as a satisfied one.
        for raw in ["SAFETY", "RECITATION", "PROHIBITED_CONTENT", "BLOCKLIST", "SPII"] {
            assert_eq!(
                StopReason::parse("google", raw),
                StopReason::ContentFilter,
                "{raw} must classify as a filter stop"
            );
        }
        assert_eq!(StopReason::parse("openai", "content_filter"), StopReason::ContentFilter);
    }

    #[test]
    fn a_malformed_tool_call_is_neither_a_clean_end_nor_a_runnable_call() {
        let r = StopReason::parse("google", "MALFORMED_FUNCTION_CALL");
        assert!(!matches!(r, StopReason::EndTurn), "must not end the loop cleanly");
        assert!(!r.wants_tools(), "there is no parseable call to run");
    }

    #[test]
    fn an_unknown_value_stays_diagnosable_and_carries_its_provider() {
        let r = StopReason::parse("openai", "some_new_reason_2027");
        assert_eq!(r, StopReason::Other("openai:some_new_reason_2027".into()));
        assert!(!r.is_truncated());
        assert!(!r.wants_tools());
        // An absent finish_reason stays distinguishable from a present unknown.
        assert_eq!(StopReason::parse("google", ""), StopReason::Other("google:<empty>".into()));
    }
}
