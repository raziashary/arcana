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
    /// The vendor caches automatically; there is no minimum to respect.
    Automatic,
    /// We mark breakpoints, and **below `min_tokens` nothing caches and no
    /// error is returned** (spec `02 §4.3`). The prefix builder warns when the
    /// assembled prefix falls short, because the provider will not.
    Explicit { min_tokens: u32 },
}

impl ModelCapabilities {
    /// The cache minimum for this model, if it has one.
    pub fn cache_minimum(&self) -> Option<u32> {
        match self.cache {
            CacheSupport::Explicit { min_tokens } => Some(min_tokens),
            _ => None,
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
    fn cache_minimum_exists_only_for_explicit_caching() {
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

        let automatic = ModelCapabilities {
            cache: CacheSupport::Automatic,
            ..explicit.clone()
        };
        assert_eq!(automatic.cache_minimum(), None);

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
}
