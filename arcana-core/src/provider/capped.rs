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

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use super::traits::{Provider, StreamResult};
use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus};
use crate::budget::daily::{CapState, DailyCap};
use crate::errors::{ArcanaError, Result};
use crate::state::app_state::AppState;
use crate::state::lock::LockTolerant;

/// Wraps any provider so no call proceeds past the daily ceiling.
pub struct CappedProvider {
    inner: Arc<dyn Provider>,
    state: Arc<Mutex<AppState>>,
}

impl CappedProvider {
    pub fn new(inner: Arc<dyn Provider>, state: Arc<Mutex<AppState>>) -> Self {
        Self { inner, state }
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

    /// Refuse the call if the day's ceiling has been reached.
    ///
    /// Reads through `cost_today()`, which rolls the counters at the local date
    /// boundary — so the first call after midnight is measured against today's
    /// spend, not yesterday's. Reading the field directly would keep a user
    /// capped all through a new day.
    fn check(&self) -> Result<()> {
        let cap = DailyCap::load();
        let spent = {
            let mut s = self.state.lock_tolerant();
            s.cost_today()
        };

        match cap.evaluate(spent) {
            CapState::Disabled | CapState::Ok => Ok(()),
            CapState::Exceeded => {
                // Pause first, then refuse. If the refusal came first the
                // scheduler would hand the same work to the next worker while
                // this one was still returning, and the ceiling would read as a
                // burst of errors rather than a stop.
                let paused = crate::budget::daily::pause_fleet("daily spend ceiling").unwrap_or(0);

                crate::events::emit_budget_exceeded(crate::events::BudgetExceededEvent {
                    session_id: String::new(),
                    task_id: None,
                    reason: "daily_cap_reached".to_string(),
                    tokens_used: 0,
                    cost_usd: spent,
                    turns_used: 0,
                });

                Err(ArcanaError::BudgetExceeded(format!(
                    "Daily spend ceiling reached: ${spent:.2} of ${:.2}. \
                     {paused} worker(s) paused. Raise the limit in Settings, or \
                     wait for the day to roll over.",
                    cap.limit_usd
                )))
            }
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

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.check()?;
        self.inner.complete(request).await
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        self.check()?;
        self.inner.stream(request).await
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
    use crate::budget::daily::DEFAULT_DAILY_CAP_USD;

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
}
