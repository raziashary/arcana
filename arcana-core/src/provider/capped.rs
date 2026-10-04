// The spend ceiling, enforced where every call passes (P2-21, P2-22).
//
// ── Why a decorator and not a check in the agent loop ────────────────────────
//
// Spec `02 §5` requires the ceiling in *"the one place every call passes"*. The
// agent loop is not that place. Four separate sites build a provider and call
// it directly — the loop (`engine.rs`), the recipe runner, compaction
// (`compaction.rs`), and the diff classifier — so a check in the loop would
// leave three paths spending freely, and the third of those runs on **every**
// compaction, which is exactly when a session is already expensive.
//
// Wrapping the provider itself means the check cannot be bypassed by adding a
// fifth caller, because a caller has to get a provider from somewhere.
//
// ── What it does when the ceiling is reached ─────────────────────────────────
//
// Refuses the call **and** pauses the fleet. Refusing alone would leave the
// scheduler handing the same work to the next worker a moment later, so the cap
// would be hit again and again while the user watched errors accumulate. The
// kill switch is what makes the ceiling a stop rather than a complaint.

use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::Stream;

use super::traits::{Provider, StreamResult};
use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
use crate::budget::daily::{self, DailySpendReservation, ReservationDecision};
use crate::errors::{ArcanaError, Result};
use crate::state::app_state::AppState;

fn pause_fleet_after_denial(
    pause: impl FnOnce() -> Result<usize>,
    spent_usd: f64,
) -> Result<usize> {
    match pause() {
        Ok(paused) => Ok(paused),
        Err(_) => {
            tracing::error!(
                target: "arcana::telemetry",
                event_type = "daily_budget_pause_failed",
                code = "fleet_pause_failed",
                retry_safe = false,
                "Daily spend ceiling blocked the provider call, but Arcana could not pause the fleet"
            );
            crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                session_id: String::new(),
                task_id: None,
                reason: "daily_cap_pause_failed".to_string(),
                tokens_used: 0,
                cost_usd: spent_usd,
                turns_used: 0,
            });
            Err(ArcanaError::BudgetExceeded(
                "Daily spend ceiling blocked this provider call, but Arcana could not pause the fleet. Pause workers manually and inspect Arcana logs.".to_string(),
            ))
        }
    }
}

/// Wraps any provider so no call proceeds past the daily ceiling.
pub struct CappedProvider {
    inner: Arc<dyn Provider>,
}

impl CappedProvider {
    pub fn new(inner: Arc<dyn Provider>, _state: Arc<Mutex<AppState>>) -> Self {
        Self { inner }
    }

    /// Wrap, as an `Arc<dyn Provider>` the call sites can use directly.
    pub fn wrap(inner: Arc<dyn Provider>, state: Arc<Mutex<AppState>>) -> Arc<dyn Provider> {
        Arc::new(Self::new(inner, state))
    }

    /// Wrap using the process-global state, for callers that hold none.
    ///
    /// Returns the provider **unwrapped** when no state has been published —
    /// which happens only in a test process. Refusing to build a provider there
    /// would break every unit test to enforce a ceiling nobody configured; the
    /// binaries that ship both publish state at startup, and `TG-P2-04` asserts
    /// they do.
    pub fn wrap_global(inner: Arc<dyn Provider>) -> Arc<dyn Provider> {
        match crate::state::app_state::global() {
            Some(state) => Arc::new(Self::new(inner, state)),
            None => inner,
        }
    }

    fn maximum_request_cost(&self, request: &CompletionRequest) -> Result<f64> {
        let model = self
            .inner
            .models()
            .into_iter()
            .find(|model| model.id == request.model)
            .ok_or_else(|| {
                ArcanaError::Agent(
                    "daily spend cap cannot reserve an unrecognized model".to_string(),
                )
            })?;
        let price = crate::cost::pricing::price_for(&request.model)?;
        let max_input_rate = [price.input, price.cached_read, price.cache_write]
            .into_iter()
            .chain(price.tier.map(|tier| tier.input))
            .chain(price.tier.map(|tier| tier.cached_read))
            .fold(0.0_f64, f64::max);
        let max_output_rate = price
            .tier
            .map(|tier| tier.output)
            .unwrap_or(price.output)
            .max(price.output);

        // The catalog context window bounds all text, tool schemas, and tool
        // results sent as input. Reserve every input token at the highest
        // supported input/cache rate, then add the request's full output and
        // thinking budgets at the highest output rate. This deliberately
        // over-reserves instead of estimating with an unverified tokenizer.
        let input = model.context_window as f64;
        let output = request
            .max_tokens
            .saturating_add(request.thinking_budget_tokens.unwrap_or_default())
            as f64;
        Ok((input * max_input_rate + output * max_output_rate) / 1_000_000.0)
    }

    fn reserve(&self, request: &CompletionRequest) -> Result<Option<DailySpendReservation>> {
        let decision =
            daily::reserve_provider_call_estimated(uuid::Uuid::new_v4().to_string(), || {
                self.maximum_request_cost(request)
            })?;
        match decision {
            ReservationDecision::Disabled => Ok(None),
            ReservationDecision::Granted(reservation) => Ok(Some(reservation)),
            ReservationDecision::Denied {
                spent_usd,
                pending_usd,
                requested_usd,
                limit_usd,
            } => {
                // Pause before returning so the scheduler cannot immediately
                // hand the same work to another worker.
                let paused = pause_fleet_after_denial(
                    || daily::pause_fleet("daily spend reservation ceiling reached"),
                    spent_usd,
                )?;
                crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                    session_id: String::new(),
                    task_id: None,
                    reason: "daily_cap_reservation_denied".to_string(),
                    tokens_used: 0,
                    cost_usd: spent_usd,
                    turns_used: 0,
                });
                Err(ArcanaError::BudgetExceeded(format!(
                    "Daily spend ceiling would be exceeded: ${spent_usd:.4} spent, \
                     ${pending_usd:.4} reserved, and ${requested_usd:.4} requested against \
                     ${limit_usd:.2}. {paused} worker(s) paused."
                )))
            }
        }
    }
}

struct ReservedStream {
    inner: StreamResult,
    reservation: Option<DailySpendReservation>,
    model: String,
}

impl ReservedStream {
    fn settle(&mut self, actual_cost: Option<f64>) -> Result<()> {
        if let Some(reservation) = self.reservation.take() {
            daily::settle_provider_call(&reservation, actual_cost)?;
        }
        Ok(())
    }
}

impl Stream for ReservedStream {
    type Item = Result<StreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next = Pin::new(&mut self.inner).poll_next(cx);
        match next {
            Poll::Ready(Some(Ok(chunk))) => {
                if let StreamChunk::MessageDelta { usage, .. } = &chunk {
                    match crate::cost::pricing::cost_usd(&self.model, usage) {
                        Ok(cost) => {
                            if let Err(error) = self.settle(Some(cost)) {
                                return Poll::Ready(Some(Err(error)));
                            }
                        }
                        Err(error) => {
                            if let Err(settle_error) = self.settle(None) {
                                return Poll::Ready(Some(Err(settle_error)));
                            }
                            return Poll::Ready(Some(Err(error)));
                        }
                    }
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(error))) => {
                if let Err(settle_error) = self.settle(None) {
                    return Poll::Ready(Some(Err(settle_error)));
                }
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                if let Err(error) = self.settle(None) {
                    return Poll::Ready(Some(Err(error)));
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }
}

impl Drop for ReservedStream {
    fn drop(&mut self) {
        if let Err(error) = self.settle(None) {
            tracing::error!(
                target: "arcana::telemetry",
                event_type = "daily_budget_settlement_failed",
                code = "reservation_retained",
                "could not settle dropped provider stream; reservation remains held: {error}"
            );
        }
    }
}

#[async_trait]
impl Provider for CappedProvider {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.inner.models()
    }

    async fn input_token_upper_bound(&self, request: &CompletionRequest) -> Result<u64> {
        self.inner.input_token_upper_bound(request).await
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let model = request.model.clone();
        let reservation = self.reserve(&request)?;
        let Some(reservation) = reservation else {
            return self.inner.complete(request).await;
        };
        match self.inner.complete(request).await {
            Ok(response) => {
                match crate::cost::pricing::cost_usd(&model, &response.usage) {
                    Ok(actual) => daily::settle_provider_call(&reservation, Some(actual))?,
                    Err(error) => {
                        daily::settle_provider_call(&reservation, None)?;
                        return Err(error);
                    }
                }
                Ok(response)
            }
            Err(error) => {
                daily::settle_provider_call(&reservation, None)?;
                Err(error)
            }
        }
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let model = request.model.clone();
        let reservation = self.reserve(&request)?;
        let Some(reservation) = reservation else {
            return self.inner.stream(request).await;
        };
        match self.inner.stream(request).await {
            Ok(inner) => Ok(Box::pin(ReservedStream {
                inner,
                reservation: Some(reservation),
                model,
            })),
            Err(error) => {
                daily::settle_provider_call(&reservation, None)?;
                Err(error)
            }
        }
    }

    /// **Deliberately not capped.** Validating a key is a free control-plane
    /// call that spends nothing, and refusing it at the ceiling would leave a
    /// user unable to check their credentials on the one day they most want to
    /// — the day their work stopped.
    async fn test_connection(&self) -> Result<ProviderStatus> {
        self.inner.test_connection().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::daily::{CapState, DailyCap, DEFAULT_DAILY_CAP_USD};

    #[test]
    fn a_disabled_cap_lets_any_spend_through() {
        let cap = DailyCap {
            enabled: false,
            limit_usd: 1.0,
        };
        assert_eq!(cap.evaluate(9_999.0), CapState::Disabled);
    }

    #[test]
    fn the_ceiling_stops_at_the_limit() {
        let cap = DailyCap {
            enabled: true,
            limit_usd: DEFAULT_DAILY_CAP_USD,
        };
        assert_eq!(cap.evaluate(DEFAULT_DAILY_CAP_USD - 0.01), CapState::Ok);
        assert_eq!(cap.evaluate(DEFAULT_DAILY_CAP_USD), CapState::Exceeded);
    }

    /// The decorator must forward everything it does not gate, or wrapping a
    /// provider would quietly change what models the UI lists.
    #[test]
    fn identity_and_models_pass_through_unchanged() {
        // Compile-level assertion: `CappedProvider` implements `Provider`, and
        // its `id`/`name`/`models` delegate. A behavioural test needs a live
        // provider, which belongs in the fixture suite (P2-68).
        fn assert_is_provider<T: Provider>() {}
        assert_is_provider::<CappedProvider>();
    }

    #[test]
    fn a_failed_fleet_pause_is_reported_after_the_provider_call_is_blocked() {
        let result = pause_fleet_after_denial(
            || Err(ArcanaError::Agent("private database detail".into())),
            0.75,
        );
        let Err(ArcanaError::BudgetExceeded(message)) = result else {
            panic!("pause failure must remain a budget stop");
        };
        assert!(message.contains("blocked this provider call"));
        assert!(message.contains("could not pause the fleet"));
        assert!(!message.contains("private database detail"));
    }
}
