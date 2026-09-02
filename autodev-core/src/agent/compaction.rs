// Conversation compaction — prevents context window overflow.
// Auto-compacts at 80% of the model's context window.
// Strategy:
//   1. Keep the first message (context/system instructions)
//   2. Summarize middle messages using the LLM (async variant)
//   3. Insert summary as an assistant message
//   4. Keep the last KEEP_RECENT_MESSAGES for recency
// Pattern from Goose's check_if_compaction_needed() and Claude Code's context collapse.

use crate::provider::types::{MessageContent, RequestMessage};
use tracing::info;

const COMPACTION_THRESHOLD: f32 = 0.80;
const KEEP_RECENT_MESSAGES: usize = 10;

/// Instruction injected before the messages being summarised
const SUMMARIZE_INSTRUCTION: &str =
    "Please provide a concise summary of the following conversation so far. \
     Focus on: key decisions made, files edited, commands run, and current task status. \
     This summary will replace the conversation history to save context space.";

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

    /// Synchronous fallback compact: keeps first + last N messages.
    /// Used when the async LLM summary path is unavailable or fails.
    pub fn compact_truncate(messages: Vec<RequestMessage>) -> Vec<RequestMessage> {
        if messages.len() <= KEEP_RECENT_MESSAGES + 1 {
            return messages;
        }

        info!(
            "Compacting (truncate): {} → {} messages",
            messages.len(),
            KEEP_RECENT_MESSAGES + 2
        );

        let mut compacted = Vec::new();
        compacted.push(messages[0].clone());

        // Insert a notice in place of dropped messages
        let dropped = messages.len() - KEEP_RECENT_MESSAGES - 1;
        compacted.push(RequestMessage {
            role: "assistant".to_string(),
            content: vec![MessageContent::Text {
                text: format!(
                    "[{dropped} earlier messages were truncated due to context limits. \
                     Continuing from the most recent {KEEP_RECENT_MESSAGES} messages.]"
                ),
            }],
        });

        let start = messages.len().saturating_sub(KEEP_RECENT_MESSAGES);
        compacted.extend_from_slice(&messages[start..]);
        compacted
    }

    /// Async LLM-based compaction — calls the provider to summarize middle messages.
    /// Falls back to truncation if the LLM call fails.
    pub async fn compact_with_summary(
        messages: Vec<RequestMessage>,
        provider: &dyn crate::provider::traits::Provider,
        model: &str,
    ) -> Vec<RequestMessage> {
        if messages.len() <= KEEP_RECENT_MESSAGES + 1 {
            return messages;
        }

        let total = messages.len();
        let keep_tail_start = total.saturating_sub(KEEP_RECENT_MESSAGES);

        // Messages to summarize: everything between [0] and the tail
        let middle: Vec<&RequestMessage> = messages[1..keep_tail_start].iter().collect();

        // Build summarization request
        let mut summary_conversation = Vec::new();
        summary_conversation.push(RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: SUMMARIZE_INSTRUCTION.to_string(),
            }],
        });
        for msg in &middle {
            summary_conversation.push((*msg).clone());
        }
        summary_conversation.push(RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "Please write the summary now.".to_string(),
            }],
        });

        // P2-66. Compaction is deliberately UNCACHED.
        //
        // This call summarises the conversation so far, so its prompt is the
        // conversation — different every time by construction. Requesting a
        // cache here would write an entry that can never be read back, and a
        // cache write is billed ABOVE the input rate: the request would cost
        // strictly more than not asking.
        //
        // It is also the call that invalidates everything downstream. Rewriting
        // the message history changes the bytes after the breakpoint, so the
        // next agent turn starts from a cold messages region regardless. That
        // cost is real and currently unaccounted; the accounting is P2-66's
        // remaining half, and it needs the monitor's numbers to be worth
        // writing.
        let summary_request = crate::provider::types::CompletionRequest {
            model: model.to_string(),
            messages: summary_conversation,
            system: None,
            tools: vec![],
            max_tokens: 1024,
            temperature: Some(0.3),
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: crate::provider::usage::CachePolicy::Off,
        };

        // Try to get a summary from the LLM
        let summary_text = match provider.complete(summary_request).await {
            Ok(response) => response
                .content
                .into_iter()
                .find_map(|block| {
                    if let MessageContent::Text { text } = block {
                        Some(text)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    format!(
                        "[{} messages summarized — summary unavailable]",
                        middle.len()
                    )
                }),
            Err(e) => {
                info!("LLM summary failed ({e}), falling back to truncation");
                return Self::compact_truncate(messages);
            }
        };

        info!(
            "Compacting (summarized): {} → {} messages + summary",
            total,
            KEEP_RECENT_MESSAGES + 2
        );

        // Reconstruct: first message + summary + tail
        let mut compacted = Vec::new();
        compacted.push(messages[0].clone());
        compacted.push(RequestMessage {
            role: "assistant".to_string(),
            content: vec![MessageContent::Text {
                text: format!("[Summary of earlier conversation]\n\n{summary_text}"),
            }],
        });
        compacted.extend_from_slice(&messages[keep_tail_start..]);
        compacted
    }

    /// Legacy alias — synchronous truncation only (used where async is unavailable).
    pub fn compact(messages: Vec<RequestMessage>) -> Vec<RequestMessage> {
        Self::compact_truncate(messages)
    }
}
