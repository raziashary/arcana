// The typed retry policy consuming `ErrorClass` was added for P3-09 / GAP-12.
//
// ── Why there was none ───────────────────────────────────────────────────────
//
// `ErrorClass` distinguishes `Retryable` from `Fatal` and `QuotaExhausted`.
// This module consumes the classification directly rather than recovering it
// from provider error text.
//
// ── The rule that makes this dangerous to get wrong ──────────────────────────
//
// ⚠ **`QuotaExhausted` is explicitly non-retryable.** A spend-cap 429 carries no
// `retry-after` and never succeeds on retry, so treating it as `Retryable` turns
// a hard stop into a **spend loop against a ceiling that already fired** — the
// system billing itself repeatedly for calls it has already been told it cannot
// afford. `ErrorClass::is_retryable()` already encodes this; the policy's job is
// to respect it rather than re-derive it.

use std::time::Duration;

use crate::errors::ArcanaError;
use crate::provider::usage::ErrorClass;

/// How many attempts a call gets in total, including the first.
///
/// Three is the whole budget: one original and two retries. A provider that
/// fails three times over ~4.5 seconds is having an outage, not a blip, and
/// further attempts spend the user's turn latency on a wall.
pub const MAX_ATTEMPTS: u32 = 3;

/// Delay before the first retry. Doubles thereafter.
pub const BASE_DELAY: Duration = Duration::from_millis(500);

/// The ceiling on any single backoff step.
///
/// Bounded because the caller is a **user waiting for a turn**, not a batch
/// job. An unbounded exponential is correct for a queue consumer and wrong for
/// something a person is watching.
pub const MAX_DELAY: Duration = Duration::from_secs(4);

/// What to do about a failed attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    /// Wait this long, then try again.
    RetryAfter(Duration),
    /// Do not retry. Carries why, so the caller can say so.
    Stop(StopReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// The error will not improve on retry.
    NotRetryable,
    /// It might have, but the attempt budget is spent.
    AttemptsExhausted,
    /// Retrying would spend against a ceiling that has already fired.
    QuotaExhausted,
}

/// Decide whether to retry attempt `attempt` (1-based) after `class`.
///
/// Deliberately takes the **classification**, not the error string. The whole
/// defect being fixed is that the classification existed and was thrown away.
pub fn decide(class: ErrorClass, attempt: u32) -> RetryDecision {
    // Checked before the attempt budget so the reason reported is the useful
    // one: "you are out of quota" rather than "we tried three times".
    if matches!(class, ErrorClass::QuotaExhausted) {
        return RetryDecision::Stop(StopReason::QuotaExhausted);
    }

    if !class.is_retryable() {
        return RetryDecision::Stop(StopReason::NotRetryable);
    }

    if attempt >= MAX_ATTEMPTS {
        return RetryDecision::Stop(StopReason::AttemptsExhausted);
    }

    // Exponential, capped. `attempt` is 1-based, so the first retry waits
    // exactly `BASE_DELAY`.
    let factor = 1u32 << (attempt - 1);
    let delay = BASE_DELAY
        .checked_mul(factor)
        .unwrap_or(MAX_DELAY)
        .min(MAX_DELAY);

    RetryDecision::RetryAfter(delay)
}

/// Run `op`, retrying per [`decide`].
///
/// `classify` extracts the class from the error. It is a parameter rather than
/// a trait impl on `ArcanaError` because only the adapter that produced the
/// error knows which provider's taxonomy it came from.
pub async fn with_retry<T, F, Fut, C>(mut op: F, classify: C) -> Result<T, ArcanaError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ArcanaError>>,
    C: Fn(&ArcanaError) -> ErrorClass,
{
    let mut attempt = 1u32;
    loop {
        let attempt_started = std::time::Instant::now();
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let class = classify(&e);
                match decide(class.clone(), attempt) {
                    RetryDecision::RetryAfter(delay) => {
                        tracing::info!(
                            target: "arcana::telemetry",
                            event_type = "provider_retry",
                            attempt,
                            duration_ns = attempt_started.elapsed().as_nanos() as u64,
                            outcome = "retrying",
                            "Provider attempt will be retried"
                        );
                        tracing::warn!(
                            "attempt {attempt}/{MAX_ATTEMPTS} failed ({class:?}); retrying in \
                             {}ms: {e}",
                            delay.as_millis()
                        );
                        tokio::time::sleep(delay).await;
                        attempt += 1;
                    }
                    RetryDecision::Stop(why) => {
                        tracing::info!(
                            target: "arcana::telemetry",
                            event_type = "provider_retry",
                            attempt,
                            duration_ns = attempt_started.elapsed().as_nanos() as u64,
                            outcome = "stopped",
                            "Provider retry policy stopped"
                        );
                        if matches!(why, StopReason::QuotaExhausted) {
                            tracing::error!(
                                "not retrying: the provider reports quota or credit \
                                 exhausted. Retrying would spend against a ceiling that has \
                                 already fired."
                            );
                        }
                        return Err(e);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spend_cap_429_is_never_retried() {
        // The rule this policy exists to respect. A quota 429 carries no
        // retry-after and never succeeds on retry, so retrying it is a spend
        // loop against a ceiling that already fired.
        for attempt in 1..=MAX_ATTEMPTS {
            assert_eq!(
                decide(ErrorClass::QuotaExhausted, attempt),
                RetryDecision::Stop(StopReason::QuotaExhausted)
            );
        }
    }

    #[test]
    fn a_rate_limit_429_is_retried_with_growing_backoff() {
        // Same status code as the above, different class — the distinction
        // `ErrorClass::classify` reads out of the body.
        assert_eq!(
            decide(ErrorClass::Retryable, 1),
            RetryDecision::RetryAfter(BASE_DELAY)
        );
        assert_eq!(
            decide(ErrorClass::Retryable, 2),
            RetryDecision::RetryAfter(BASE_DELAY * 2)
        );
    }

    #[test]
    fn the_attempt_budget_is_bounded() {
        assert_eq!(
            decide(ErrorClass::Retryable, MAX_ATTEMPTS),
            RetryDecision::Stop(StopReason::AttemptsExhausted)
        );
        assert_eq!(
            decide(ErrorClass::Overloaded, MAX_ATTEMPTS + 5),
            RetryDecision::Stop(StopReason::AttemptsExhausted)
        );
    }

    #[test]
    fn backoff_is_capped_because_a_person_is_waiting() {
        // An unbounded exponential is right for a queue consumer and wrong for
        // a user watching a turn.
        for attempt in 1..MAX_ATTEMPTS {
            if let RetryDecision::RetryAfter(d) = decide(ErrorClass::Overloaded, attempt) {
                assert!(d <= MAX_DELAY, "attempt {attempt} would wait {d:?}");
            }
        }
    }

    #[test]
    fn a_fatal_error_stops_immediately() {
        assert_eq!(
            decide(ErrorClass::Fatal, 1),
            RetryDecision::Stop(StopReason::NotRetryable)
        );
    }

    #[test]
    fn anthropics_529_overload_is_retried() {
        // 529 is Anthropic's overload code, not 503 — the status most likely to
        // be hand-rolled wrongly, and one that genuinely does recover.
        assert!(matches!(
            decide(ErrorClass::Overloaded, 1),
            RetryDecision::RetryAfter(_)
        ));
    }

    #[tokio::test]
    async fn a_transient_failure_succeeds_on_the_second_attempt() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let calls = AtomicU32::new(0);

        let out = with_retry(
            || {
                let n = calls.fetch_add(1, Ordering::SeqCst);
                async move {
                    if n == 0 {
                        Err(ArcanaError::ProviderClassified {
                            class: ErrorClass::Overloaded,
                            message: "openai 503".into(),
                        })
                    } else {
                        Ok(42)
                    }
                }
            },
            |_| ErrorClass::Overloaded,
        )
        .await;

        assert_eq!(out.unwrap(), 42);
        assert_eq!(calls.load(Ordering::SeqCst), 2, "one retry, not more");
    }

    #[tokio::test]
    async fn a_quota_error_is_attempted_exactly_once() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let calls = AtomicU32::new(0);

        let out: Result<(), _> = with_retry(
            || {
                calls.fetch_add(1, Ordering::SeqCst);
                async {
                    Err(ArcanaError::ProviderClassified {
                        class: ErrorClass::QuotaExhausted,
                        message: "429 insufficient_quota".into(),
                    })
                }
            },
            |_| ErrorClass::QuotaExhausted,
        )
        .await;

        assert!(out.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a quota failure must not be retried even once — that is a spend loop"
        );
    }
}
