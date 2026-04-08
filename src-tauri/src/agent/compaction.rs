// Conversation compaction — prevents context window overflow.
// Auto-compacts at 80% of the model's context window.
// Strategy: summarise middle messages, keep first (system) and last N (recency).
// Pattern from Goose's check_if_compaction_needed() and Claude Code's context collapse.

use crate::provider::types::RequestMessage;
use tracing::info;

const COMPACTION_THRESHOLD: f32 = 0.80;
const KEEP_RECENT_MESSAGES: usize = 10;

pub struct CompactionEngine;

impl CompactionEngine {
    /// Check if the estimated token count exceeds threshold.
    pub fn needs_compaction(estimated_tokens: u32, context_window: u32) -> bool {
        estimated_tokens as f32 / context_window as f32 > COMPACTION_THRESHOLD
    }

    /// Rough token estimator: ~4 chars per token.
    pub fn estimate_tokens(messages: &[RequestMessage]) -> u32 {
        let total_chars: usize = messages
            .iter()
            .flat_map(|m| m.content.iter())
            .map(|c| {
                let json = serde_json::to_string(c).unwrap_or_default();
                json.len()
            })
            .sum();
        (total_chars / 4) as u32
    }

    /// Compact by keeping the first message (usually context) + last N messages.
    /// Middle messages are dropped — a summary prompt should be inserted at Phase 2.
    /// Returns the compacted message list.
    pub fn compact(messages: Vec<RequestMessage>) -> Vec<RequestMessage> {
        if messages.len() <= KEEP_RECENT_MESSAGES + 1 {
            return messages;
        }

        info!(
            "Compacting conversation: {} messages → {}",
            messages.len(),
            KEEP_RECENT_MESSAGES + 1
        );

        let mut compacted = Vec::new();
        // Always keep the first message (may contain important context)
        compacted.push(messages[0].clone());
        // Keep the most recent messages for continuity
        let start = messages.len().saturating_sub(KEEP_RECENT_MESSAGES);
        compacted.extend_from_slice(&messages[start..]);
        compacted
    }
}
