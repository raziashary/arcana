pub mod daily;
pub mod run_tokens;
// Budget Circuit Breaker — Ruflo Feature 4.
//
// Pre-emptive per-task token and cost caps enforced inside the AgentEngine loop.
// When a worker's running token total for a task exceeds its budget, the loop
// halts and the task is escalated to Supervisor via a WorkerRequest instead of
// silently burning tokens.
//
// Reference: ADR-097 Federation Budget in Ruflo's plugin-agent-federation.
// Constraint: All logic lives in src-tauri/src/; no submodule changes.

use serde::{Deserialize, Serialize};

// ── Core Budget Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskBudget {
    pub max_tokens: Option<u64>, // None = unbounded
    pub max_usd: Option<f64>,    // None = unbounded
    pub max_turns: Option<u32>,  // Agent loop turn limit (default: 20)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetStatus {
    pub tokens_used: u64,
    pub cost_usd: f64,
    pub turns_used: u32,
    pub exceeded: bool,
    pub exceeded_reason: Option<BudgetExceededReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BudgetExceededReason {
    TokenLimitReached,
    CostLimitReached,
    TurnLimitReached,
}

impl BudgetExceededReason {
    pub fn as_event_str(&self) -> &str {
        match self {
            Self::TokenLimitReached => "BUDGET_EXCEEDED_TOKENS",
            Self::CostLimitReached => "BUDGET_EXCEEDED_USD",
            Self::TurnLimitReached => "BUDGET_EXCEEDED_TURNS",
        }
    }
}

// ── Preset Constructors ───────────────────────────────────────────────────────

impl TaskBudget {
    /// Combine task-specific caps with a worker's run cap. The stricter finite
    /// value wins, and a missing task cap leaves the worker cap unchanged.
    pub fn restricted_by(&self, task: &TaskBudget) -> Self {
        fn minimum(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            match (a, b) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            }
        }
        fn minimum_turns(a: Option<u32>, b: Option<u32>) -> Option<u32> {
            match (a, b) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            }
        }
        Self {
            max_tokens: minimum(self.max_tokens, task.max_tokens),
            max_usd: match (self.max_usd, task.max_usd) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) | (None, Some(a)) => Some(a),
                (None, None) => None,
            },
            max_turns: minimum_turns(self.max_turns, task.max_turns),
        }
    }

    pub fn default_worker() -> Self {
        Self {
            max_tokens: Some(100_000),
            max_usd: Some(0.50),
            max_turns: Some(20),
        }
    }

    pub fn default_supervisor() -> Self {
        Self {
            max_tokens: Some(50_000),
            max_usd: Some(0.25),
            max_turns: Some(10),
        }
    }

    pub fn unlimited() -> Self {
        Self {
            max_tokens: None,
            max_usd: None,
            max_turns: Some(20),
        }
    }

    /// Returns the first exceeded reason, or None if all budgets are within limits.
    /// Token check → Cost check → Turn check (priority order).
    pub fn check(&self, status: &BudgetStatus) -> Option<BudgetExceededReason> {
        if self.max_tokens.map_or(false, |m| status.tokens_used >= m) {
            return Some(BudgetExceededReason::TokenLimitReached);
        }
        if self.max_usd.map_or(false, |m| status.cost_usd >= m) {
            return Some(BudgetExceededReason::CostLimitReached);
        }
        if self.max_turns.map_or(false, |m| status.turns_used >= m) {
            return Some(BudgetExceededReason::TurnLimitReached);
        }
        None
    }
}

// ── Input Validation (mirrors ADR-097) ────────────────────────────────────────

pub const BUDGET_MAX_TOKENS_CEILING: u64 = 1_000_000;
pub const BUDGET_MAX_USD_CEILING: f64 = 100.0;
pub const BUDGET_MAX_TURNS_CEILING: u32 = 100;

/// Validate budget fields as required by ADR-097.
/// Rejects NaN, Infinity, negative values, and values exceeding hard ceilings.
/// Returns a constant-string error code — no echo of remaining values.
pub fn validate_budget(
    max_tokens: Option<u64>,
    max_usd: Option<f64>,
    max_turns: Option<u32>,
) -> Result<TaskBudget, &'static str> {
    if let Some(t) = max_tokens {
        if t == 0 || t > BUDGET_MAX_TOKENS_CEILING {
            return Err("BUDGET_INVALID_TOKENS");
        }
    }
    if let Some(u) = max_usd {
        if u.is_nan() || u.is_infinite() || u <= 0.0 || u > BUDGET_MAX_USD_CEILING {
            return Err("BUDGET_INVALID_USD");
        }
    }
    if let Some(t) = max_turns {
        if t == 0 || t > BUDGET_MAX_TURNS_CEILING {
            return Err("BUDGET_INVALID_TURNS");
        }
    }
    Ok(TaskBudget {
        max_tokens,
        max_usd,
        max_turns,
    })
}

// ── Daily Summary ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DailyBudgetSummary {
    pub total_allocated_tokens: u64,
    pub total_spent_tokens: u64,
    pub total_allocated_usd: f64,
    pub total_spent_usd: f64,
    pub tasks_exceeded: u32,
    pub tasks_with_budget: u32,
}

#[cfg(test)]
mod budget_enforcement_tests {
    use super::*;

    fn spent(tokens: u64, usd: f64, turns: u32) -> BudgetStatus {
        BudgetStatus {
            tokens_used: tokens,
            cost_usd: usd,
            turns_used: turns,
            ..Default::default()
        }
    }

    #[test]
    fn a_worker_default_stops_at_fifty_cents() {
        let b = TaskBudget::default_worker();
        assert_eq!(b.max_usd, Some(0.50));
        assert!(b.check(&spent(0, 0.49, 1)).is_none(), "under the cap runs");
        assert!(
            matches!(
                b.check(&spent(0, 0.50, 1)),
                Some(BudgetExceededReason::CostLimitReached)
            ),
            "at the cap it stops — >= not >, so the cap is a ceiling and not a \
             suggestion"
        );
    }

    #[test]
    fn each_limit_stops_independently() {
        let b = TaskBudget::default_worker();
        assert!(matches!(
            b.check(&spent(100_000, 0.0, 1)),
            Some(BudgetExceededReason::TokenLimitReached)
        ));
        assert!(matches!(
            b.check(&spent(0, 0.0, 20)),
            Some(BudgetExceededReason::TurnLimitReached)
        ));
    }

    #[test]
    fn an_unlimited_budget_still_bounds_turns() {
        // `unlimited` means unbounded SPEND, not an unbounded loop. A run with
        // no turn ceiling can spin forever on a tool that always succeeds.
        let b = TaskBudget::unlimited();
        assert_eq!(b.max_usd, None);
        assert!(b.check(&spent(u64::MAX, 1e9, 1)).is_none());
        assert!(
            b.check(&spent(0, 0.0, 20)).is_some(),
            "turns are still capped"
        );
    }

    #[test]
    fn saved_task_caps_only_tighten_the_worker_run_cap() {
        let worker = TaskBudget {
            max_tokens: Some(100_000),
            max_usd: Some(0.50),
            max_turns: Some(20),
        };
        let task = TaskBudget {
            max_tokens: Some(2_000),
            max_usd: None,
            max_turns: Some(5),
        };
        let effective = worker.restricted_by(&task);
        assert_eq!(effective.max_tokens, Some(2_000));
        assert_eq!(effective.max_usd, Some(0.50));
        assert_eq!(effective.max_turns, Some(5));
    }
}
