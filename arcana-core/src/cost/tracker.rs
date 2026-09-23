use crate::errors::Result;
use crate::provider::usage::Usage;
// Cost and token tracking per session message.
// Pricing data mirrors provider model definitions for correctness.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageCost {
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
}

pub struct CostTracker;

impl CostTracker {
    /// Cost for one request's usage, or an error naming the model.
    ///
    /// ⚠ **Returns `Result`, and that is the point.** This used to hand back a
    /// number unconditionally, because `model_pricing` ended in
    /// `_ => (1.0, 3.0)` — so an unknown model produced an invented bill that
    /// looked exactly like a real one. The audit measured 9 of 12 offered
    /// models hitting it. Spec `02 §5`: *"a wrong cost number is worse than no
    /// cost number, because it is believed."*
    ///
    /// Callers must decide what to show when cost is unknown. Showing nothing
    /// is a correct answer; showing $0.0000 is not.
    pub fn calculate(model: &str, usage: &Usage) -> Result<MessageCost> {
        Ok(MessageCost {
            tokens_in: usage.total_input(),
            tokens_out: usage.output,
            cost_usd: super::pricing::cost_usd(model, usage)?,
        })
    }

    /// Format a cost value for display (e.g. "$0.0042")
    pub fn format_cost(cost_usd: f64) -> String {
        if cost_usd < 0.001 {
            format!("${:.6}", cost_usd)
        } else if cost_usd < 1.0 {
            format!("${:.4}", cost_usd)
        } else {
            format!("${:.2}", cost_usd)
        }
    }

    /// Format tokens for display (e.g. "12.4K", "1.2M")
    pub fn format_tokens(tokens: u64) -> String {
        if tokens < 1_000 {
            tokens.to_string()
        } else if tokens < 1_000_000 {
            format!("{:.1}K", tokens as f64 / 1_000.0)
        } else {
            format!("{:.1}M", tokens as f64 / 1_000_000.0)
        }
    }
}

// The pricing table lives in `cost::pricing`. It used to be six `contains()`
// arms over a `_ => (1.0, 3.0)` default in this file; see that module for what
// that produced and why exact-id matching replaced it.
