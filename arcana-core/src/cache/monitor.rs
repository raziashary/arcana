// The cache-hit alarm (P2-12).
//
// ── Why this is built BEFORE caching, not after ──────────────────────────────
//
// Prompt caching is the one optimisation that fails **completely silently**.
// Every provider we ship declines to cache below a per-model minimum and
// returns no error for it (`G6-agents-sdk-research.md:341`). A single changed
// byte anywhere in the prefix produces the same nothing. In both cases the call
// succeeds, the content is correct, the latency is unremarkable — and the only
// symptom is the bill.
//
// So an implementation cannot be checked by reading it. `cache_control` can sit
// in the request, look right in review, and cache nothing on every call. The
// only fact that distinguishes a working cache from a broken one is
// `usage.cached_read > 0` on the second request that carries the same prefix.
//
// This module is that instrument, and it exists first so that `P2-16` has
// something that can fail. Building it afterwards would mean writing the
// verification against the behaviour we ended up with rather than the invariant
// we meant to hold — L-C9 in this project's learnings register, seven instances
// and counting.
//
// ── What it will NOT do ──────────────────────────────────────────────────────
//
// It does not alarm on a prefix that is *legitimately* uncacheable: below the
// model's minimum (`P2-15` warns there instead), on a model with no cache
// support, or on the first sighting of a prefix. An alarm that fires when
// nothing is wrong gets muted, and a muted alarm is worse than none.

use std::collections::HashMap;
use std::sync::Mutex;

use tracing::{error, warn};

use crate::provider::usage::{CacheSupport, ModelCapabilities, Usage};

/// What the monitor concluded about one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheVerdict {
    /// This model cannot cache; nothing to check.
    NotSupported,
    /// First time this prefix has been seen. A cache write is expected here,
    /// and a read is not.
    FirstSighting,
    /// The prefix is shorter than the model's minimum, so the provider will
    /// silently decline. Not an alarm — a design fact, reported by `P2-15`.
    BelowMinimum { prefix_tokens: u32, minimum: u32 },
    /// A repeat of a known prefix that read from cache. The working case.
    Hit { cached_read: u32 },
    /// ⚠ A repeat of a known prefix that read **nothing** from cache.
    ///
    /// Something invalidated the prefix between the two calls, and no provider
    /// will say what. This is the alarm.
    SilentMiss { seen_before: u32 },
}

impl CacheVerdict {
    /// Whether this verdict means caching is broken.
    ///
    /// Deliberately narrow: only `SilentMiss`. The other variants are all
    /// states in which zero cached reads is the correct outcome.
    pub fn is_alarm(&self) -> bool {
        matches!(self, CacheVerdict::SilentMiss { .. })
    }
}

/// Tracks prefixes across requests and reports whether the cache is working.
///
/// Keyed by a hash of the exact bytes sent as the cacheable prefix — not by
/// model, not by session. Two calls share a cache entry only if those bytes are
/// identical, so that is what the key has to be.
#[derive(Default)]
pub struct CacheMonitor {
    /// prefix hash → how many times we have sent it.
    seen: Mutex<HashMap<u64, u32>>,
    /// Alarms raised, for the smoke test and the readiness matrix to read.
    alarms: Mutex<u32>,
}

impl CacheMonitor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one request and judge it.
    ///
    /// `prefix_hash` identifies the exact bytes offered for caching;
    /// `prefix_tokens` is their measured length. Both come from
    /// `cache::prefix`, which is the single builder — computing them at the
    /// call site would let the monitor and the request disagree about what was
    /// actually sent, which is precisely the bug class this exists to catch.
    pub fn observe(
        &self,
        prefix_hash: u64,
        prefix_tokens: u32,
        caps: &ModelCapabilities,
        usage: &Usage,
    ) -> CacheVerdict {
        if matches!(caps.cache, CacheSupport::None) {
            return CacheVerdict::NotSupported;
        }

        // Below the minimum the provider declines and says nothing. Checked
        // BEFORE the repeat check, because otherwise every short prefix would
        // alarm on its second use — which is the fastest way to get a real
        // alarm ignored.
        if let Some(minimum) = caps.cache_minimum() {
            if prefix_tokens < minimum {
                return CacheVerdict::BelowMinimum {
                    prefix_tokens,
                    minimum,
                };
            }
        }

        let seen_before = {
            let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
            let count = seen.entry(prefix_hash).or_insert(0);
            let before = *count;
            *count += 1;
            before
        };

        if seen_before == 0 {
            return CacheVerdict::FirstSighting;
        }

        if usage.cached_read > 0 {
            return CacheVerdict::Hit {
                cached_read: usage.cached_read,
            };
        }

        // The alarm. Logged here rather than at the call site so it cannot be
        // wired up and then left unlogged.
        *self.alarms.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        error!(
            "CACHE MISS on a repeated prefix: {prefix_tokens} tokens, hash {prefix_hash:x}, \
             sent {} times before, and this call read 0 cached tokens. The prefix is being \
             invalidated between calls — every request is paying full price plus the cache \
             write surcharge.",
            seen_before
        );
        CacheVerdict::SilentMiss { seen_before }
    }

    /// How many silent misses have been seen. Read by `S2` and the readiness
    /// matrix; a non-zero count after a caching smoke means caching is broken
    /// regardless of what the code looks like.
    pub fn alarm_count(&self) -> u32 {
        *self.alarms.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Distinct prefixes seen. A number close to the request count means the
    /// prefix is effectively single-use — `P2-64`'s failure mode, where a
    /// per-run identifier sits inside the cached region.
    pub fn distinct_prefixes(&self) -> usize {
        self.seen.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Warn when prefixes are single-use across many requests (`P2-64`).
    ///
    /// Naive caching is not merely useless there, it is **more expensive**: a
    /// cache write is billed above the normal input rate, so a prefix that
    /// never gets read back turns every request into full price plus a
    /// surcharge. Six concurrent QC reviewers over an otherwise identical
    /// multi-KB protocol, each stamped with its own `task_id`, is exactly this.
    ///
    /// ⚠ **This is a CROSS-RUN question, and it was being asked per-run**
    /// (F-13, P2 post-phase audit). A single agent run computes its prefix once,
    /// so `distinct` was structurally 1 and the condition could never be
    /// satisfied — an alarm for the P2-64 failure mode that was incapable of
    /// firing on it, because that failure mode IS six separate runs.
    ///
    /// Call it on [`global()`], which accumulates across runs. Called on a
    /// per-run monitor it is a no-op by construction, and says so rather than
    /// staying quiet.
    pub fn report_reuse(&self, requests: u32) {
        let distinct = self.distinct_prefixes() as u32;

        if distinct <= 1 {
            // Nothing to conclude: one prefix means either perfect reuse or a
            // single run. Distinguishing them needs the cross-run monitor.
            return;
        }

        if requests >= 4 && distinct * 2 > requests {
            warn!(
                "Cache prefixes are effectively single-use: {distinct} distinct prefixes \
                 across {requests} requests. A per-run identifier is most likely inside \
                 the cached region, which makes caching COST more than it saves — each \
                 write is billed above the input rate and never read back."
            );
        }
    }
}

/// The process-wide monitor.
///
/// ── Why a global, when the per-run one already exists ────────────────────────
///
/// `P2-64`'s failure mode is **six concurrent QC reviewers**, which is six
/// separate agent runs. A monitor scoped to one run computes one prefix and can
/// never observe that the other five differ — so the alarm for it was
/// structurally incapable of firing (F-13).
///
/// This one accumulates. Every run observes into it as well as into its own,
/// so "distinct prefixes across the process" becomes answerable.
pub fn global() -> &'static CacheMonitor {
    static GLOBAL: std::sync::OnceLock<CacheMonitor> = std::sync::OnceLock::new();
    GLOBAL.get_or_init(CacheMonitor::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps(cache: CacheSupport) -> ModelCapabilities {
        ModelCapabilities {
            streaming: true,
            tools: true,
            parallel_tools: true,
            vision: false,
            reasoning: false,
            cache,
            context_window: 200_000,
            max_output: 8192,
        }
    }

    fn usage_with_cached_read(cached_read: u32) -> Usage {
        Usage {
            input: 100,
            cached_read,
            cache_write: 0,
            output: 50,
            reasoning: 0,
            tool: 0,
        }
    }

    #[test]
    fn a_repeated_prefix_that_reads_nothing_is_an_alarm() {
        // The whole point of the module.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });

        assert_eq!(
            m.observe(0xABC, 4096, &c, &usage_with_cached_read(0)),
            CacheVerdict::FirstSighting,
            "the first call cannot read from a cache that does not exist yet"
        );
        assert_eq!(m.alarm_count(), 0);

        let v = m.observe(0xABC, 4096, &c, &usage_with_cached_read(0));
        assert_eq!(v, CacheVerdict::SilentMiss { seen_before: 1 });
        assert!(v.is_alarm());
        assert_eq!(m.alarm_count(), 1);
    }

    #[test]
    fn a_repeated_prefix_that_reads_from_cache_is_a_hit() {
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });

        m.observe(0xABC, 4096, &c, &usage_with_cached_read(0));
        let v = m.observe(0xABC, 4096, &c, &usage_with_cached_read(3900));
        assert_eq!(v, CacheVerdict::Hit { cached_read: 3900 });
        assert!(!v.is_alarm());
        assert_eq!(m.alarm_count(), 0);
    }

    #[test]
    fn a_short_prefix_does_not_alarm_because_the_provider_is_behaving_correctly() {
        // A prefix below the minimum never caches and never errors. Alarming
        // here would fire on every Delegate-mode call (P2-65: ~461 tokens
        // against a 4,096 minimum) and the alarm would be muted within a day.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 4096 });

        for _ in 0..5 {
            let v = m.observe(0xABC, 461, &c, &usage_with_cached_read(0));
            assert_eq!(
                v,
                CacheVerdict::BelowMinimum {
                    prefix_tokens: 461,
                    minimum: 4096
                }
            );
            assert!(!v.is_alarm());
        }
        assert_eq!(m.alarm_count(), 0, "a correctly-declined cache is not a fault");
    }

    #[test]
    fn a_model_that_cannot_cache_is_never_an_alarm() {
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::None);
        for _ in 0..3 {
            assert_eq!(
                m.observe(0xABC, 100_000, &c, &usage_with_cached_read(0)),
                CacheVerdict::NotSupported
            );
        }
        assert_eq!(m.alarm_count(), 0);
    }

    #[test]
    fn an_automatic_cache_is_held_to_the_same_standard_as_an_explicit_one() {
        // `Automatic` used to report no minimum, which meant this path could
        // never reach `BelowMinimum` — and OpenAI's short prefixes would have
        // alarmed falsely instead.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Automatic { min_tokens: 1024 });

        assert_eq!(
            m.observe(0x1, 500, &c, &usage_with_cached_read(0)),
            CacheVerdict::BelowMinimum {
                prefix_tokens: 500,
                minimum: 1024
            }
        );

        // Above the minimum it alarms exactly like the explicit shape.
        m.observe(0x2, 2000, &c, &usage_with_cached_read(0));
        assert!(m
            .observe(0x2, 2000, &c, &usage_with_cached_read(0))
            .is_alarm());
    }

    #[test]
    fn different_prefixes_do_not_alarm_each_other() {
        // Two distinct prefixes are two first-sightings, not a miss. Keying on
        // anything coarser than the exact bytes — model, session — would make
        // this case alarm falsely.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });

        assert_eq!(
            m.observe(0xAAA, 4096, &c, &usage_with_cached_read(0)),
            CacheVerdict::FirstSighting
        );
        assert_eq!(
            m.observe(0xBBB, 4096, &c, &usage_with_cached_read(0)),
            CacheVerdict::FirstSighting
        );
        assert_eq!(m.alarm_count(), 0);
        assert_eq!(m.distinct_prefixes(), 2);
    }

    #[test]
    fn single_use_prefixes_are_visible_as_such() {
        // P2-64's signature: every request carries its own prefix, so the cache
        // can never be read back and each write is a pure surcharge.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });

        for i in 0..6u64 {
            m.observe(i, 4096, &c, &usage_with_cached_read(0));
        }
        assert_eq!(m.distinct_prefixes(), 6, "six requests, six prefixes — nothing reusable");
        assert_eq!(m.alarm_count(), 0, "no prefix repeated, so no miss to report");
        m.report_reuse(6);
    }
    #[test]
    fn a_single_prefix_draws_no_conclusion_about_reuse() {
        // F-13. A per-run monitor sees exactly one prefix, so `distinct == 1`
        // says nothing — it is equally consistent with perfect reuse and with a
        // single run. Reporting either way would be a guess.
        let m = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });
        for _ in 0..10 {
            m.observe(0xAAA, 4096, &c, &usage_with_cached_read(4000));
        }
        assert_eq!(m.distinct_prefixes(), 1);
        m.report_reuse(10); // no-op, and must not panic
    }

    #[test]
    fn the_process_monitor_accumulates_across_runs() {
        // The cross-run view F-13 needed. Two "runs" observing into the same
        // monitor produce two distinct prefixes, which is the signal
        // `report_reuse` exists to read.
        let shared = CacheMonitor::new();
        let c = caps(CacheSupport::Explicit { min_tokens: 1024 });

        for run in 0..6u64 {
            // Each run has its own prefix — the P2-64 shape.
            shared.observe(run, 4096, &c, &usage_with_cached_read(0));
        }

        assert_eq!(shared.distinct_prefixes(), 6, "six runs, six single-use prefixes");
        assert_eq!(shared.alarm_count(), 0, "no prefix repeated, so no MISS to report");
        shared.report_reuse(6);
    }

    #[test]
    fn the_global_monitor_is_one_instance() {
        let a = global() as *const CacheMonitor;
        let b = global() as *const CacheMonitor;
        assert_eq!(a, b, "the process monitor must be a single accumulating instance");
    }
}
