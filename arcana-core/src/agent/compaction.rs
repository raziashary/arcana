// Conversation compaction — prevents context window overflow.
// Auto-compacts at 80% of the model's context window.
// Strategy:
//   1. Keep the first message (context/system instructions)
//   2. Summarize middle messages using the LLM (async variant)
//   3. Insert summary as an assistant message
//   4. Keep the last KEEP_RECENT_MESSAGES for recency
// Pattern from Goose's check_if_compaction_needed() and Claude Code's context collapse.

use crate::provider::types::{ClearToolUses, MessageContent, RequestMessage};
use tracing::info;

/// Fraction of the context window at which the conversation is compacted.
///
/// ⚠ **A chosen number, not a measured one** (`P2-66`), and the trade it makes
/// is asymmetric:
///
///   * Too LOW wastes money continuously — it throws away context the model
///     could still hold, and pays for a summarisation call to do it.
///   * Too HIGH fails the turn outright: the request overflows and the whole
///     turn is lost, along with everything spent producing it.
///
/// 0.80 leaves a fifth of the window for the turn's own output plus the tool
/// results it will accumulate before the next check, which is why the error is
/// taken on the low side.
///
/// It became meaningfully safer in `P2-57`: this fraction was previously
/// applied to a hardcoded 200,000 regardless of model, so on Gemini's 1M window
/// it fired at **16% of the real capacity**. It now reads the model's actual
/// window from the catalogue.
const COMPACTION_THRESHOLD: f32 = 0.80;

/// Where the server-side clear layer starts, as a fraction of the window.
///
/// P3-25/P3-28. **Must stay below `COMPACTION_THRESHOLD`** -- the whole point
/// of the layer is that the cheap server-side clear gets its chance before the
/// expensive summarisation call. Asserted, not merely intended.
const CLEAR_TRIGGER_FRACTION: f32 = 0.60;

/// Most-recent tool uses kept intact when clearing.
const CLEAR_KEEP_TOOL_USES: u32 = 3;

/// Do not clear unless it frees at least this fraction of the window.
const CLEAR_MIN_FRACTION: f32 = 0.05;
const KEEP_RECENT_MESSAGES: usize = 10;

/// Instruction injected before the messages being summarised
const SUMMARIZE_INSTRUCTION: &str =
    "Please provide a concise summary of the following conversation so far. \
     Focus on: key decisions made, files edited, commands run, and current task status. \
     This summary will replace the conversation history to save context space.";

pub struct CompactionEngine;

impl CompactionEngine {
    /// Check if the estimated token count exceeds threshold.
    /// The clear policy for a model with this context window (`P3-25`/`P3-28`).
    ///
    /// ── Why these numbers ────────────────────────────────────────────────
    ///
    /// `P3-28` asks for the threshold to come from the measured cache-write
    /// cost. **That trade-off is vacuous here, and the measurement is what
    /// establishes it:** clearing changes the message region, so it would only
    /// cost a cache write if a breakpoint were set there — and
    /// `anthropic.rs` marks the SYSTEM block only. Nothing in the message
    /// region is cached, so clearing forfeits no cache write at all.
    ///
    /// What clearing does cost is tool-result content the model may still
    /// need. So the numbers are chosen against that, not against cache:
    ///
    /// * `trigger` at 60% of the window, against `COMPACTION_THRESHOLD` at
    ///   80%. The 20-point band is where the cheap server-side layer works
    ///   before the expensive summarisation layer is allowed to fire.
    /// * `keep` = 3 most recent tool uses -- enough for a read -> edit ->
    ///   verify cycle, which is the shortest loop that needs its own history.
    /// * `clear_at_least` = 5% of the window, so a clear that would free
    ///   almost nothing does not happen at all.
    ///
    /// ⚠ **If a message-region cache breakpoint is ever added, these must be
    /// re-derived** -- at that point clearing DOES forfeit a cache write, the
    /// trade-off stops being vacuous, and 60% is no longer justified by
    /// anything.
    pub fn clear_policy(context_window: u32) -> Option<ClearToolUses> {
        let trigger = (context_window as f32 * CLEAR_TRIGGER_FRACTION) as u32;
        let at_least = (context_window as f32 * CLEAR_MIN_FRACTION) as u32;
        if trigger == 0 {
            return None;
        }
        Some(ClearToolUses {
            trigger_input_tokens: trigger,
            keep_tool_uses: CLEAR_KEEP_TOOL_USES,
            clear_at_least_input_tokens: at_least,
        })
    }

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
    /// Summarise the middle of the conversation, returning the compacted
    /// history **and what it cost** (`P2-66`).
    ///
    /// ── Why the usage comes back ─────────────────────────────────────────────
    ///
    /// This performs a real, billed `complete()` call, and its `usage` was
    /// **discarded** — the response was consumed for its text and dropped. So
    /// compaction spent money that never reached the cost tracker, never
    /// counted toward the daily ceiling, and never appeared in analytics.
    ///
    /// It is not a rounding error. Compaction fires on long sessions, which are
    /// the expensive ones, and it summarises the entire middle of the
    /// conversation — the input side of this call is by construction most of a
    /// context window.
    ///
    /// Returning the usage lets the caller fold it into the turn's total, which
    /// is the only place with a whole-run view.
    pub async fn compact_with_summary(
        messages: Vec<RequestMessage>,
        provider: &dyn crate::provider::traits::Provider,
        model: &str,
    ) -> (Vec<RequestMessage>, crate::provider::usage::Usage) {
        if messages.len() <= KEEP_RECENT_MESSAGES + 1 {
            return (messages, crate::provider::usage::Usage::default());
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
        // P3-25: no clear layer here. This call exists to summarise the
        // history; asking the API to drop parts of that history first would
        // summarise something other than what was asked for.
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
            clear_tool_uses: None,
        };

        // Try to get a summary from the LLM
        let mut compaction_usage = crate::provider::usage::Usage::default();
        let summary_text = match provider.complete(summary_request).await {
            Ok(response) => response
                .usage_recorded(&mut compaction_usage)
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
                // No usage to report — the call did not complete. Truncation is
                // free, which is the one good thing about this path.
                return (Self::compact_truncate(messages), compaction_usage);
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
        (compacted, compaction_usage)
    }

    /// Legacy alias — synchronous truncation only (used where async is unavailable).
    pub fn compact(messages: Vec<RequestMessage>) -> Vec<RequestMessage> {
        Self::compact_truncate(messages)
    }
}

#[cfg(test)]
mod p3_25_28_tests {
    use super::*;

    #[test]
    fn the_clear_layer_fires_before_the_compaction_layer() {
        // 🔴 THE POINT OF THE LAYER. Clearing is server-side and costs one
        // field on a request we were sending anyway; compaction costs a whole
        // summarisation call and discards more. If the clear trigger were at
        // or above the compaction threshold, the cheap layer would never get
        // its chance and P3-25 would be decoration.
        assert!(
            CLEAR_TRIGGER_FRACTION < COMPACTION_THRESHOLD,
            "clear fires at {CLEAR_TRIGGER_FRACTION} and compaction at \
             {COMPACTION_THRESHOLD} — the cheap layer must come first"
        );

        // Stated concretely on a real window, because a fraction comparison
        // that holds can still produce a useless band.
        let window = 200_000u32;
        let policy = CompactionEngine::clear_policy(window).expect("a policy for 200k");
        let compact_at = (window as f32 * COMPACTION_THRESHOLD) as u32;

        assert!(
            policy.trigger_input_tokens < compact_at,
            "clear at {} vs compaction at {compact_at}",
            policy.trigger_input_tokens
        );
        assert!(
            compact_at - policy.trigger_input_tokens > 10_000,
            "the band between the layers is {} tokens — too narrow for the \
             cheap layer to do anything before the expensive one fires",
            compact_at - policy.trigger_input_tokens
        );
    }

    #[test]
    fn the_policy_scales_with_the_model_not_with_a_constant() {
        // P2-57's lesson, one layer up: a hardcoded threshold means Gemini's
        // 1M window gets treated as if it were 200k.
        let small = CompactionEngine::clear_policy(200_000).unwrap();
        let large = CompactionEngine::clear_policy(1_000_000).unwrap();

        assert!(
            large.trigger_input_tokens > small.trigger_input_tokens * 4,
            "a 1M window must not clear at a 200k threshold: {} vs {}",
            large.trigger_input_tokens,
            small.trigger_input_tokens
        );
    }

    #[test]
    fn recent_tool_uses_are_kept() {
        // Clearing everything would break the read → edit → verify loop, which
        // is the shortest sequence that genuinely needs its own history.
        let p = CompactionEngine::clear_policy(200_000).unwrap();
        assert!(p.keep_tool_uses >= 3, "kept only {}", p.keep_tool_uses);
    }

    #[test]
    fn a_clear_that_would_free_almost_nothing_does_not_happen() {
        let p = CompactionEngine::clear_policy(200_000).unwrap();
        assert!(
            p.clear_at_least_input_tokens > 0,
            "without a floor, the API churns applied_edits for no benefit"
        );
        assert!(
            p.clear_at_least_input_tokens < p.trigger_input_tokens,
            "the floor must be reachable once the trigger fires"
        );
    }
}
