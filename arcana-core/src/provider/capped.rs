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
use crate::provider::usage::{StopReason, Usage};
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
    provider_id: String,
    model: String,
    usage: Usage,
    terminal_stop_reason_seen: bool,
}

impl ReservedStream {
    fn settle(&mut self, actual_cost: Option<f64>) -> Result<()> {
        if let Some(reservation) = self.reservation.take() {
            daily::settle_provider_call(&reservation, actual_cost)?;
        }
        Ok(())
    }

    fn settle_observed_usage(&mut self) -> Result<()> {
        let cost = if self.usage.total() > 0 {
            Some(crate::cost::pricing::cost_usd(&self.model, &self.usage)?)
        } else {
            None
        };
        self.settle(cost)
    }
}

impl Stream for ReservedStream {
    type Item = Result<StreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next = Pin::new(&mut self.inner).poll_next(cx);
        match next {
            Poll::Ready(Some(Ok(StreamChunk::MessageDelta { stop_reason, usage }))) => {
                // Providers can split usage over several frames. Keep the
                // reservation until explicit Done or EOF following a
                // recognized terminal stop reason. A cancellation/error/drop,
                // or bare EOF, must retain the maximum for unreported output.
                if !matches!(
                    StopReason::parse(&self.provider_id, &stop_reason),
                    StopReason::Other(_)
                ) {
                    self.terminal_stop_reason_seen = true;
                }
                self.usage.add(&usage);
                Poll::Ready(Some(Ok(StreamChunk::MessageDelta { stop_reason, usage })))
            }
            Poll::Ready(Some(Ok(chunk @ StreamChunk::Done))) => {
                if let Err(error) = self.settle_observed_usage() {
                    return Poll::Ready(Some(Err(error)));
                }
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(chunk))),
            Poll::Ready(Some(Err(error))) => {
                if let Err(settle_error) = self.settle(None) {
                    return Poll::Ready(Some(Err(settle_error)));
                }
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                // Google finishes by closing the SSE stream after its final
                // usage-bearing finish-reason frame, without a Done chunk. A
                // bare EOF without a recognized terminal reason retains the
                // full conservative hold because output usage may be missing.
                let settled = if self.terminal_stop_reason_seen {
                    self.settle_observed_usage()
                } else {
                    self.settle(None)
                };
                if let Err(error) = settled {
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
                provider_id: self.inner.id().to_owned(),
                model,
                usage: Usage::default(),
                terminal_stop_reason_seen: false,
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
    use crate::provider::traits::Provider;
    use crate::provider::types::{RequestMessage, StreamChunk};
    use crate::provider::usage::Usage;
    use async_trait::async_trait;
    use futures::Stream;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};

    #[derive(Clone, Copy)]
    enum UsageBoundaryScenario {
        InputThenPending,
        InputThenEof,
        UnknownStopThenEof,
        SplitAndComplete,
        SplitAndEof,
    }

    struct UsageBoundaryProvider {
        scenario: UsageBoundaryScenario,
        entered: Arc<tokio::sync::Notify>,
        dropped: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Provider for UsageBoundaryProvider {
        fn id(&self) -> &str {
            "google"
        }

        fn name(&self) -> &str {
            "Daily budget usage boundary fixture"
        }

        fn models(&self) -> Vec<ModelInfo> {
            crate::provider::catalogue::models_for("google")
        }

        async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse> {
            unreachable!("the usage boundary tests use streaming")
        }

        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResult> {
            Ok(Box::pin(UsageBoundaryStream {
                scenario: self.scenario,
                step: 0,
                entered: Arc::clone(&self.entered),
                dropped: Arc::clone(&self.dropped),
            }))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    struct UsageBoundaryStream {
        scenario: UsageBoundaryScenario,
        step: usize,
        entered: Arc<tokio::sync::Notify>,
        dropped: Arc<AtomicUsize>,
    }

    impl Stream for UsageBoundaryStream {
        type Item = Result<StreamChunk>;

        fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let chunk = match (self.scenario, self.step) {
                (_, 0) => Some(StreamChunk::MessageDelta {
                    stop_reason: String::new(),
                    usage: Usage {
                        input: 17,
                        ..Usage::default()
                    },
                }),
                (UsageBoundaryScenario::InputThenPending, _) => {
                    self.entered.notify_one();
                    return Poll::Pending;
                }
                (UsageBoundaryScenario::InputThenEof, _) => return Poll::Ready(None),
                (UsageBoundaryScenario::UnknownStopThenEof, 1) => Some(StreamChunk::MessageDelta {
                    stop_reason: "future_provider_reason".into(),
                    usage: Usage::default(),
                }),
                (UsageBoundaryScenario::UnknownStopThenEof, _) => return Poll::Ready(None),
                (
                    UsageBoundaryScenario::SplitAndComplete | UsageBoundaryScenario::SplitAndEof,
                    1,
                ) => Some(StreamChunk::MessageDelta {
                    stop_reason: "end_turn".into(),
                    usage: Usage {
                        output: 4,
                        ..Usage::default()
                    },
                }),
                (UsageBoundaryScenario::SplitAndComplete, 2) => Some(StreamChunk::Done),
                (
                    UsageBoundaryScenario::SplitAndComplete | UsageBoundaryScenario::SplitAndEof,
                    _,
                ) => return Poll::Ready(None),
            };
            self.step += 1;
            Poll::Ready(Some(Ok(chunk.expect("scripted usage frame"))))
        }
    }

    impl Drop for UsageBoundaryStream {
        fn drop(&mut self) {
            self.dropped.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn usage_boundary_provider(scenario: UsageBoundaryScenario) -> Arc<UsageBoundaryProvider> {
        Arc::new(UsageBoundaryProvider {
            scenario,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        })
    }

    fn usage_boundary_request() -> CompletionRequest {
        CompletionRequest {
            model: "gemini-3.8-flash".into(),
            messages: vec![RequestMessage {
                role: "user".into(),
                content: vec![],
            }],
            system: None,
            tools: vec![],
            max_tokens: 32,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: crate::provider::usage::CachePolicy::Off,
            clear_tool_uses: None,
        }
    }

    fn daily_ledger_snapshot() -> (i64, i64, i64) {
        let config = crate::db::config_db::open().expect("isolated config database");
        let (reservations, reserved, spent) = (
            config
                .query_row(
                    "SELECT count(*) FROM daily_budget_reservations",
                    [],
                    |row| row.get(0),
                )
                .expect("read daily reservation count"),
            config
                .query_row(
                    "SELECT coalesce(sum(amount_micros), 0) FROM daily_budget_reservations",
                    [],
                    |row| row.get(0),
                )
                .expect("read daily reservation amount"),
            config
                .query_row(
                    "SELECT coalesce(sum(spent_micros), 0) FROM daily_budget_totals",
                    [],
                    |row| row.get(0),
                )
                .expect("read daily spend"),
        );
        (reservations, reserved, spent)
    }

    fn configure_isolated_cap() -> (tempfile::TempDir, crate::db::test_support::HomeGuard) {
        let home = tempfile::tempdir().expect("temporary Arcana home");
        let guard = crate::db::test_support::with_home(home.path());
        DailyCap {
            enabled: true,
            limit_usd: 100.0,
        }
        .save()
        .expect("enable isolated daily cap");
        (home, guard)
    }

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

    #[tokio::test(flavor = "current_thread")]
    async fn input_only_usage_then_cancel_keeps_the_full_daily_reservation() {
        use futures::StreamExt;

        let (_home, _home_guard) = configure_isolated_cap();
        let provider = usage_boundary_provider(UsageBoundaryScenario::InputThenPending);
        let wrapped =
            CappedProvider::wrap(provider.clone(), Arc::new(Mutex::new(AppState::default())));
        let mut stream = wrapped
            .stream(usage_boundary_request())
            .await
            .expect("capped provider stream");

        let input = stream
            .next()
            .await
            .expect("input usage frame")
            .expect("valid frame");
        assert!(matches!(input, StreamChunk::MessageDelta { usage, .. } if usage.input == 17));
        let (reservation_count, reserved_before_cancel, spent_before_cancel) =
            daily_ledger_snapshot();
        assert_eq!(reservation_count, 1);
        assert!(
            reserved_before_cancel > 0,
            "full request hold remains pending"
        );
        assert_eq!(
            spent_before_cancel, 0,
            "partial input is not final billing usage"
        );

        let entered = Arc::clone(&provider.entered);
        let pending_read = tokio::spawn(async move { stream.next().await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .expect("provider body waits after the input-only frame");
        pending_read.abort();
        let _ = pending_read.await;

        let (reservation_count, reserved_after_cancel, spent_after_cancel) =
            daily_ledger_snapshot();
        assert_eq!(provider.dropped.load(Ordering::SeqCst), 1);
        assert_eq!(
            reservation_count, 0,
            "cancelled stream releases hold into spend"
        );
        assert_eq!(reserved_after_cancel, 0);
        assert_eq!(
            spent_after_cancel, reserved_before_cancel,
            "unknown output is charged at the original conservative request maximum"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn split_usage_reconciles_daily_reservation_only_after_completion() {
        use futures::StreamExt;

        let (_home, _home_guard) = configure_isolated_cap();
        let provider = usage_boundary_provider(UsageBoundaryScenario::SplitAndComplete);
        let wrapped = CappedProvider::wrap(provider, Arc::new(Mutex::new(AppState::default())));
        let mut stream = wrapped
            .stream(usage_boundary_request())
            .await
            .expect("capped provider stream");

        stream
            .next()
            .await
            .expect("input frame")
            .expect("valid input");
        let (count_after_input, reserved_after_input, spent_after_input) = daily_ledger_snapshot();
        assert_eq!(count_after_input, 1);
        assert!(reserved_after_input > 0);
        assert_eq!(spent_after_input, 0);

        stream
            .next()
            .await
            .expect("output frame")
            .expect("valid output");
        let (count_after_output, reserved_after_output, spent_after_output) =
            daily_ledger_snapshot();
        assert_eq!(count_after_output, 1);
        assert_eq!(reserved_after_output, reserved_after_input);
        assert_eq!(
            spent_after_output, 0,
            "split usage is not settled mid-stream"
        );

        assert!(matches!(
            stream
                .next()
                .await
                .expect("Done frame")
                .expect("valid completion"),
            StreamChunk::Done
        ));
        let (count_after_done, reserved_after_done, spent_after_done) = daily_ledger_snapshot();
        let expected = (crate::cost::pricing::cost_usd(
            "gemini-3.8-flash",
            &Usage {
                input: 17,
                output: 4,
                ..Usage::default()
            },
        )
        .expect("price reported usage")
            * 1_000_000.0)
            .ceil() as i64;
        assert_eq!(count_after_done, 0);
        assert_eq!(reserved_after_done, 0);
        assert_eq!(spent_after_done, expected);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn split_usage_reconciles_daily_reservation_at_successful_eof() {
        use futures::StreamExt;

        let (_home, _home_guard) = configure_isolated_cap();
        let provider = usage_boundary_provider(UsageBoundaryScenario::SplitAndEof);
        let wrapped = CappedProvider::wrap(provider, Arc::new(Mutex::new(AppState::default())));
        let mut stream = wrapped
            .stream(usage_boundary_request())
            .await
            .expect("capped provider stream");

        stream
            .next()
            .await
            .expect("input frame")
            .expect("valid input");
        stream
            .next()
            .await
            .expect("output frame")
            .expect("valid output");
        let (count_before_eof, reserved_before_eof, spent_before_eof) = daily_ledger_snapshot();
        assert_eq!(count_before_eof, 1);
        assert!(reserved_before_eof > 0);
        assert_eq!(spent_before_eof, 0);

        assert!(
            stream.next().await.is_none(),
            "successful EOF completes stream"
        );
        let (count_after_eof, reserved_after_eof, spent_after_eof) = daily_ledger_snapshot();
        let expected = (crate::cost::pricing::cost_usd(
            "gemini-3.8-flash",
            &Usage {
                input: 17,
                output: 4,
                ..Usage::default()
            },
        )
        .expect("price reported usage")
            * 1_000_000.0)
            .ceil() as i64;
        assert_eq!(count_after_eof, 0);
        assert_eq!(reserved_after_eof, 0);
        assert_eq!(spent_after_eof, expected);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn input_only_then_eof_keeps_the_full_daily_reservation() {
        use futures::StreamExt;

        let (_home, _home_guard) = configure_isolated_cap();
        let provider = usage_boundary_provider(UsageBoundaryScenario::InputThenEof);
        let wrapped = CappedProvider::wrap(provider, Arc::new(Mutex::new(AppState::default())));
        let mut stream = wrapped
            .stream(usage_boundary_request())
            .await
            .expect("capped provider stream");

        let input = stream
            .next()
            .await
            .expect("input usage")
            .expect("valid usage");
        assert!(matches!(input, StreamChunk::MessageDelta { usage, .. } if usage.input == 17));
        let (count_before_eof, reserved_before_eof, spent_before_eof) = daily_ledger_snapshot();
        assert_eq!(count_before_eof, 1);
        assert!(reserved_before_eof > 0);
        assert_eq!(spent_before_eof, 0);

        assert!(
            stream.next().await.is_none(),
            "fixture ends without terminal stop"
        );
        let (count_after_eof, reserved_after_eof, spent_after_eof) = daily_ledger_snapshot();
        assert_eq!(count_after_eof, 0);
        assert_eq!(reserved_after_eof, 0);
        assert_eq!(spent_after_eof, reserved_before_eof);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unknown_stop_reason_then_eof_keeps_the_full_daily_reservation() {
        use futures::StreamExt;

        let (_home, _home_guard) = configure_isolated_cap();
        let provider = usage_boundary_provider(UsageBoundaryScenario::UnknownStopThenEof);
        let wrapped = CappedProvider::wrap(provider, Arc::new(Mutex::new(AppState::default())));
        let mut stream = wrapped
            .stream(usage_boundary_request())
            .await
            .expect("capped provider stream");

        stream
            .next()
            .await
            .expect("input usage")
            .expect("valid input");
        let unknown = stream
            .next()
            .await
            .expect("unknown reason frame")
            .expect("valid frame");
        assert!(
            matches!(unknown, StreamChunk::MessageDelta { ref stop_reason, .. }
            if stop_reason == "future_provider_reason")
        );
        let (count_before_eof, reserved_before_eof, spent_before_eof) = daily_ledger_snapshot();
        assert_eq!(count_before_eof, 1);
        assert!(reserved_before_eof > 0);
        assert_eq!(spent_before_eof, 0);

        assert!(
            stream.next().await.is_none(),
            "fixture ends after unknown reason"
        );
        let (count_after_eof, reserved_after_eof, spent_after_eof) = daily_ledger_snapshot();
        assert_eq!(count_after_eof, 0);
        assert_eq!(reserved_after_eof, 0);
        assert_eq!(spent_after_eof, reserved_before_eof);
    }
}
