// Cost and token tracking per session message.
// Pricing data mirrors provider model definitions for correctness.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageCost {
    pub tokens_in:  u32,
    pub tokens_out: u32,
    pub cost_usd:   f64,
}

pub struct CostTracker;

impl CostTracker {
    /// Compute cost for a given model + token counts.
    /// Pricing in USD per 1 million tokens.
    pub fn calculate(model: &str, tokens_in: u32, tokens_out: u32) -> MessageCost {
        let (cost_in_per_1m, cost_out_per_1m) = model_pricing(model);
        let cost_usd = (tokens_in  as f64 / 1_000_000.0) * cost_in_per_1m
                     + (tokens_out as f64 / 1_000_000.0) * cost_out_per_1m;
        MessageCost { tokens_in, tokens_out, cost_usd }
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

/// Pricing table (USD per 1M tokens): (input_price, output_price)
fn model_pricing(model: &str) -> (f64, f64) {
    match model {
        m if m.contains("claude-opus")   => (15.0, 75.0),
        m if m.contains("claude-sonnet") => (3.0, 15.0),
        m if m.contains("claude-haiku")  => (0.25, 1.25),
        m if m.contains("gpt-4o")        => (2.5, 10.0),
        m if m.contains("gpt-4o-mini")   => (0.15, 0.6),
        m if m.contains("gemini-1.5-pro") => (1.25, 5.0),
        _                                 => (1.0, 3.0),  // Safe default
    }
}
