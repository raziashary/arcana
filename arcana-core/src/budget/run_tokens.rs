//! Atomic, run-scoped token reservations for physical provider attempts.

use std::sync::{Arc, Mutex};

use crate::errors::{ArcanaError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunTokenSnapshot {
    pub limit: Option<u64>,
    pub used: u64,
    pub reserved: u64,
}

#[derive(Debug)]
struct State {
    limit: Option<u64>,
    used: u64,
    reserved: u64,
}

/// Shared by every provider attempt belonging to one root agent run,
/// including compaction and child agents.
#[derive(Debug)]
pub struct RunTokenLedger {
    state: Mutex<State>,
}

impl RunTokenLedger {
    pub fn new(limit: Option<u64>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                limit,
                used: 0,
                reserved: 0,
            }),
        })
    }

    /// Apply a stricter runtime cap without widening a previously set cap.
    /// Pending reservations remain charged against the new limit.
    pub fn tighten_limit(&self, limit: Option<u64>) {
        let mut state = self.lock();
        state.limit = match (state.limit, limit) {
            (Some(current), Some(next)) => Some(current.min(next)),
            (Some(current), None) => Some(current),
            (None, Some(next)) => Some(next),
            (None, None) => None,
        };
    }

    /// Bring saved task usage into the current run's accounting without
    /// double-counting tokens already settled by this in-memory ledger.
    pub fn raise_used_floor(&self, used: u64) {
        let mut state = self.lock();
        state.used = state.used.max(used);
    }

    pub fn snapshot(&self) -> RunTokenSnapshot {
        let state = self.lock();
        RunTokenSnapshot {
            limit: state.limit,
            used: state.used,
            reserved: state.reserved,
        }
    }

    pub fn reserve(self: &Arc<Self>, amount: u64) -> Result<RunTokenReservation> {
        let mut state = self.lock();
        if let Some(limit) = state.limit {
            let requested = state
                .used
                .saturating_add(state.reserved)
                .saturating_add(amount);
            if requested > limit {
                return Err(ArcanaError::BudgetExceeded(format!(
                    "run-token reservation would exceed the configured cap ({}/{})",
                    requested, limit
                )));
            }
        }
        state.reserved = state.reserved.saturating_add(amount);
        Ok(RunTokenReservation {
            ledger: Arc::clone(self),
            amount,
            settled: false,
        })
    }

    fn settle(&self, reserved: u64, actual: Option<u64>) {
        let mut state = self.lock();
        state.reserved = state.reserved.saturating_sub(reserved);
        state.used = state.used.saturating_add(actual.unwrap_or(reserved));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A reservation remains charged at its full conservative amount if the
/// provider was dispatched but returns no reliable usage, or if its stream is
/// dropped before terminal usage arrives.
pub struct RunTokenReservation {
    ledger: Arc<RunTokenLedger>,
    amount: u64,
    settled: bool,
}

impl RunTokenReservation {
    pub fn amount(&self) -> u64 {
        self.amount
    }

    pub fn settle(mut self, actual: Option<u64>) {
        self.ledger.settle(self.amount, actual);
        self.settled = true;
    }
}

impl Drop for RunTokenReservation {
    fn drop(&mut self) {
        if !self.settled {
            self.ledger.settle(self.amount, None);
            self.settled = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_reservations_cannot_overshoot_the_run_limit() {
        let ledger = RunTokenLedger::new(Some(100));
        let first = ledger.reserve(60).expect("first reservation");
        assert!(ledger.reserve(41).is_err());
        let second = ledger.reserve(40).expect("remaining budget");
        assert_eq!(ledger.snapshot().reserved, 100);
        first.settle(Some(45));
        assert_eq!(ledger.snapshot().used, 45);
        assert_eq!(ledger.snapshot().reserved, 40);
        second.settle(Some(30));
        assert_eq!(ledger.snapshot().used, 75);
        assert_eq!(ledger.snapshot().reserved, 0);
    }

    #[test]
    fn unknown_usage_keeps_the_full_reservation_charged() {
        let ledger = RunTokenLedger::new(Some(100));
        let reservation = ledger.reserve(60).expect("reservation");
        reservation.settle(None);
        assert_eq!(ledger.snapshot().used, 60);
        assert_eq!(ledger.snapshot().reserved, 0);
    }

    #[test]
    fn tightening_includes_existing_usage_and_in_flight_reservations() {
        let ledger = RunTokenLedger::new(Some(100));
        ledger.raise_used_floor(20);
        let pending = ledger.reserve(30).expect("reservation");
        ledger.tighten_limit(Some(40));
        assert!(ledger.reserve(1).is_err());
        pending.settle(Some(10));
        assert_eq!(ledger.snapshot().used, 30);
        assert!(ledger.reserve(11).is_err());
        assert!(ledger.reserve(10).is_ok());
    }
}
