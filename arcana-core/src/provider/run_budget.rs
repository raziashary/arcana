//! Provider decorator enforcing one shared run-token cap before every physical
//! generation attempt.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use futures::Stream;

use super::traits::{Provider, StreamResult};
use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
use crate::budget::run_tokens::{RunTokenLedger, RunTokenReservation};
use crate::errors::Result;
use crate::provider::usage::{StopReason, Usage};

pub struct RunTokenBudgetProvider {
    inner: Arc<dyn Provider>,
    ledger: Arc<RunTokenLedger>,
}

impl RunTokenBudgetProvider {
    pub fn wrap(inner: Arc<dyn Provider>, ledger: Arc<RunTokenLedger>) -> Arc<dyn Provider> {
        Arc::new(Self { inner, ledger })
    }

    async fn reserve(&self, request: &CompletionRequest) -> Result<Option<RunTokenReservation>> {
        let snapshot = self.ledger.snapshot();
        if snapshot.limit.is_none() {
            return Ok(None);
        }
        let started = std::time::Instant::now();
        let input = self.inner.input_token_upper_bound(request).await?;
        let amount = input.saturating_add(u64::from(request.max_tokens));
        match self.ledger.reserve(amount) {
            Ok(reservation) => {
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "run_token_reservation",
                    operation = "provider_generation",
                    input_tokens = input,
                    max_output_tokens = request.max_tokens,
                    reserved_tokens = amount,
                    duration_ns = started.elapsed().as_nanos() as u64,
                    outcome = "granted",
                    "Run-token budget reserved before provider dispatch"
                );
                Ok(Some(reservation))
            }
            Err(error) => {
                tracing::warn!(
                    target: "arcana::telemetry",
                    event_type = "run_token_reservation",
                    operation = "provider_generation",
                    input_tokens = input,
                    max_output_tokens = request.max_tokens,
                    reserved_tokens = amount,
                    duration_ns = started.elapsed().as_nanos() as u64,
                    outcome = "denied",
                    "Run-token budget denied provider dispatch"
                );
                Err(error)
            }
        }
    }
}

struct ReservedStream {
    inner: StreamResult,
    reservation: Option<RunTokenReservation>,
    provider_id: String,
    usage: Usage,
    terminal_stop_reason_seen: bool,
}

impl ReservedStream {
    fn settle_observed_usage(&mut self) {
        if let Some(reservation) = self.reservation.take() {
            let actual = (self.usage.total() > 0).then(|| u64::from(self.usage.total()));
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "run_token_reservation_settled",
                operation = "provider_stream",
                actual_tokens = actual.unwrap_or_default(),
                outcome = if actual.is_some() {
                    "usage_reported"
                } else {
                    "usage_unknown"
                },
                "Run-token reservation reconciled at provider stream completion"
            );
            reservation.settle(actual);
        }
    }

    fn settle_unknown_usage(&mut self) {
        if let Some(reservation) = self.reservation.take() {
            reservation.settle(None);
        }
    }
}

impl Stream for ReservedStream {
    type Item = Result<StreamChunk>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(StreamChunk::MessageDelta { stop_reason, usage }))) => {
                // Usage may arrive in several frames. Anthropic sends input
                // first and output later; OpenAI may send usage after its
                // finish-reason frame. Keep the full reservation until Done,
                // or until EOF follows a recognized terminal stop reason.
                // Bare EOF, cancellation, error, or drop keeps the conservative
                // maximum because output usage may still be unknown.
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
                self.settle_observed_usage();
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Ok(chunk))) => Poll::Ready(Some(Ok(chunk))),
            Poll::Ready(Some(Err(error))) => {
                self.settle_unknown_usage();
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                // Google represents completion by EOF after its finish-reason
                // usage frame rather than by an explicit Done chunk. A bare
                // EOF without a recognized terminal reason (for example,
                // Anthropic message_start followed by a truncated body) is
                // still unknown usage and retains the full conservative bound.
                if self.terminal_stop_reason_seen {
                    self.settle_observed_usage();
                } else {
                    self.settle_unknown_usage();
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }
}

#[async_trait]
impl Provider for RunTokenBudgetProvider {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.inner.models()
    }

    fn capabilities(&self, model: &str) -> crate::provider::usage::ModelCapabilities {
        self.inner.capabilities(model)
    }

    async fn input_token_upper_bound(&self, request: &CompletionRequest) -> Result<u64> {
        self.inner.input_token_upper_bound(request).await
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let Some(reservation) = self.reserve(&request).await? else {
            return self.inner.complete(request).await;
        };
        match self.inner.complete(request).await {
            Ok(response) => {
                reservation.settle(Some(u64::from(response.usage.total())));
                Ok(response)
            }
            Err(error) => {
                reservation.settle(None);
                Err(error)
            }
        }
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let Some(reservation) = self.reserve(&request).await? else {
            return self.inner.stream(request).await;
        };
        match self.inner.stream(request).await {
            Ok(inner) => Ok(Box::pin(ReservedStream {
                inner,
                reservation: Some(reservation),
                provider_id: self.inner.id().to_owned(),
                usage: Usage::default(),
                terminal_stop_reason_seen: false,
            })),
            Err(error) => {
                // Once generation dispatch begins, an absent usage result is
                // unknown and the maximum reservation remains charged.
                reservation.settle(None);
                Err(error)
            }
        }
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        self.inner.test_connection().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ArcanaError;
    use crate::provider::types::RequestMessage;
    use crate::provider::usage::Usage;
    use futures::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll};

    #[derive(Clone, Copy)]
    enum UsageStreamScenario {
        PartialThenPending,
        InputThenEof,
        UnknownStopThenEof,
        SplitAndComplete,
        SplitAndEof,
    }

    struct UsageStreamProvider {
        scenario: UsageStreamScenario,
        entered: Arc<tokio::sync::Notify>,
        dropped: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Provider for UsageStreamProvider {
        fn id(&self) -> &str {
            "usage-stream"
        }
        fn name(&self) -> &str {
            "Usage boundary fixture"
        }
        fn models(&self) -> Vec<ModelInfo> {
            vec![ModelInfo {
                id: "fake-model".into(),
                name: "Fake".into(),
                context_window: 100,
                supports_vision: false,
                supports_thinking: false,
                cost_in_per_1m: 0.0,
                cost_out_per_1m: 0.0,
            }]
        }
        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            Ok(40)
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
        scenario: UsageStreamScenario,
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
                (UsageStreamScenario::PartialThenPending, _) => {
                    self.entered.notify_one();
                    return Poll::Pending;
                }
                (UsageStreamScenario::InputThenEof, _) => return Poll::Ready(None),
                (UsageStreamScenario::UnknownStopThenEof, 1) => Some(StreamChunk::MessageDelta {
                    stop_reason: "future_provider_reason".into(),
                    usage: Usage::default(),
                }),
                (UsageStreamScenario::UnknownStopThenEof, _) => return Poll::Ready(None),
                (UsageStreamScenario::SplitAndComplete | UsageStreamScenario::SplitAndEof, 1) => {
                    Some(StreamChunk::MessageDelta {
                        stop_reason: "end_turn".into(),
                        usage: Usage {
                            output: 4,
                            ..Usage::default()
                        },
                    })
                }
                (UsageStreamScenario::SplitAndComplete, 2) => Some(StreamChunk::Done),
                (UsageStreamScenario::SplitAndComplete | UsageStreamScenario::SplitAndEof, _) => {
                    return Poll::Ready(None)
                }
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

    struct FakeProvider {
        preflight: std::result::Result<u64, &'static str>,
        usage: Usage,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl Provider for FakeProvider {
        fn id(&self) -> &str {
            "fake"
        }
        fn name(&self) -> &str {
            "fake"
        }
        fn models(&self) -> Vec<ModelInfo> {
            vec![ModelInfo {
                id: "fake-model".into(),
                name: "Fake".into(),
                context_window: 100,
                supports_vision: false,
                supports_thinking: false,
                cost_in_per_1m: 0.0,
                cost_out_per_1m: 0.0,
            }]
        }
        async fn input_token_upper_bound(&self, _request: &CompletionRequest) -> Result<u64> {
            self.preflight
                .map_err(|error| ArcanaError::Provider(error.into()))
        }
        async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(CompletionResponse {
                message_id: "fake-response".into(),
                content: vec![],
                stop_reason: "stop".into(),
                usage: self.usage,
                model: request.model,
            })
        }
        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResult> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let usage = self.usage;
            Ok(Box::pin(futures::stream::iter([
                Ok(StreamChunk::MessageDelta {
                    stop_reason: "stop".into(),
                    usage: Usage::default(),
                }),
                Ok(StreamChunk::MessageDelta {
                    stop_reason: "stop".into(),
                    usage,
                }),
                Ok(StreamChunk::Done),
            ])))
        }
        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    fn request(max_tokens: u32) -> CompletionRequest {
        CompletionRequest {
            model: "fake-model".into(),
            messages: vec![RequestMessage {
                role: "user".into(),
                content: vec![],
            }],
            system: None,
            tools: vec![],
            max_tokens,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: crate::provider::usage::CachePolicy::Off,
            clear_tool_uses: None,
        }
    }

    #[tokio::test]
    async fn denial_and_preflight_failure_prevent_generation_dispatch() {
        let provider = Arc::new(FakeProvider {
            preflight: Ok(80),
            usage: Usage::default(),
            calls: AtomicUsize::new(0),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider.clone(), ledger);
        assert!(wrapped.complete(request(21)).await.is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);

        let provider = Arc::new(FakeProvider {
            preflight: Err("countTokens unavailable"),
            usage: Usage::default(),
            calls: AtomicUsize::new(0),
        });
        let wrapped =
            RunTokenBudgetProvider::wrap(provider.clone(), RunTokenLedger::new(Some(100)));
        assert!(wrapped.stream(request(10)).await.is_err());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn unary_and_stream_usage_settle_the_same_shared_run_ledger() {
        let provider = Arc::new(FakeProvider {
            preflight: Ok(40),
            usage: Usage {
                input: 20,
                output: 5,
                ..Usage::default()
            },
            calls: AtomicUsize::new(0),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider.clone(), Arc::clone(&ledger));
        wrapped.complete(request(30)).await.expect("unary call");
        assert_eq!(ledger.snapshot().used, 25);
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");
        use futures::StreamExt;
        while stream.next().await.is_some() {}
        assert_eq!(ledger.snapshot().used, 50);
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn empty_finish_usage_keeps_the_reservation_until_later_usage_frame() {
        let provider = Arc::new(FakeProvider {
            preflight: Ok(40),
            usage: Usage {
                input: 17,
                output: 4,
                ..Usage::default()
            },
            calls: AtomicUsize::new(0),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");
        use futures::StreamExt;
        stream
            .next()
            .await
            .expect("finish-reason frame")
            .expect("finish-reason frame is valid");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        stream
            .next()
            .await
            .expect("usage frame")
            .expect("usage frame is valid");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        stream
            .next()
            .await
            .expect("stream completion")
            .expect("stream completion is valid");
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 21);
    }

    #[tokio::test]
    async fn input_only_usage_then_cancel_keeps_the_full_run_reservation() {
        let provider = Arc::new(UsageStreamProvider {
            scenario: UsageStreamScenario::PartialThenPending,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider.clone(), Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");
        let input = stream
            .next()
            .await
            .expect("input usage frame")
            .expect("valid frame");
        assert!(matches!(input, StreamChunk::MessageDelta { usage, .. } if usage.input == 17));
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);

        let entered = Arc::clone(&provider.entered);
        let pending_read = tokio::spawn(async move { stream.next().await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .expect("body waits after the input-only usage frame");
        pending_read.abort();
        let _ = pending_read.await;

        assert_eq!(provider.dropped.load(Ordering::SeqCst), 1);
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 70);
    }

    #[tokio::test]
    async fn split_usage_reconciles_only_when_the_stream_completes() {
        let provider = Arc::new(UsageStreamProvider {
            scenario: UsageStreamScenario::SplitAndComplete,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");

        stream
            .next()
            .await
            .expect("input usage")
            .expect("valid input usage");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        stream
            .next()
            .await
            .expect("output usage")
            .expect("valid output usage");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        assert!(matches!(
            stream
                .next()
                .await
                .expect("completion frame")
                .expect("valid completion"),
            StreamChunk::Done
        ));
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 21);
    }

    #[tokio::test]
    async fn split_usage_reconciles_at_successful_eof() {
        let provider = Arc::new(UsageStreamProvider {
            scenario: UsageStreamScenario::SplitAndEof,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");

        stream
            .next()
            .await
            .expect("input usage")
            .expect("valid input usage");
        stream
            .next()
            .await
            .expect("output usage")
            .expect("valid output usage");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        assert!(
            stream.next().await.is_none(),
            "successful EOF completes stream"
        );
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 21);
    }

    #[tokio::test]
    async fn input_only_then_eof_keeps_the_full_run_reservation() {
        let provider = Arc::new(UsageStreamProvider {
            scenario: UsageStreamScenario::InputThenEof,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");

        let input = stream
            .next()
            .await
            .expect("input usage")
            .expect("valid usage");
        assert!(matches!(input, StreamChunk::MessageDelta { usage, .. } if usage.input == 17));
        assert_eq!(ledger.snapshot().reserved, 70);
        assert_eq!(ledger.snapshot().used, 0);
        assert!(
            stream.next().await.is_none(),
            "fixture ends without terminal stop"
        );
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 70);
    }

    #[tokio::test]
    async fn unknown_stop_reason_then_eof_keeps_the_full_run_reservation() {
        let provider = Arc::new(UsageStreamProvider {
            scenario: UsageStreamScenario::UnknownStopThenEof,
            entered: Arc::new(tokio::sync::Notify::new()),
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let mut stream = wrapped.stream(request(30)).await.expect("stream call");

        stream
            .next()
            .await
            .expect("input usage")
            .expect("valid usage");
        stream
            .next()
            .await
            .expect("unknown reason frame")
            .expect("valid frame");
        assert_eq!(ledger.snapshot().reserved, 70);
        assert!(
            stream.next().await.is_none(),
            "fixture ends after unknown reason"
        );
        assert_eq!(ledger.snapshot().reserved, 0);
        assert_eq!(ledger.snapshot().used, 70);
    }

    #[tokio::test]
    async fn dropped_stream_keeps_its_full_reservation_charged() {
        let provider = Arc::new(FakeProvider {
            preflight: Ok(40),
            usage: Usage::default(),
            calls: AtomicUsize::new(0),
        });
        let ledger = RunTokenLedger::new(Some(100));
        let wrapped = RunTokenBudgetProvider::wrap(provider, Arc::clone(&ledger));
        let stream = wrapped.stream(request(30)).await.expect("stream call");
        drop(stream);
        assert_eq!(ledger.snapshot().used, 70);
        assert_eq!(ledger.snapshot().reserved, 0);
    }
}
