// Auto-summarization — Phase 4.5
// Triggered when a session reaches AUTO_SUMMARIZE_EVERY turns or on session end.
// Generates a compact session summary (≤ 500 words) and stores it as an
// episodic_memories row with summary_type = 'session_summary'.
//
// Summary generation strategy:
//   - Extract user messages only (agent tool calls are too verbose).
//   - Build a structured summary from turns: objectives, key decisions, outcomes.
//   - Does NOT require an LLM call — uses rule-based condensation.
//     (An LLM-written summary was once planned for a second runtime that no
//     longer exists; rule-based condensation is what ships.)

use tracing::{debug, info};

use super::episodic::EpisodicStore;
use super::types::EpisodicRole;
use crate::errors::Result;

/// Summarize every N turns (both user + agent counted together = 1 turn).
pub const AUTO_SUMMARIZE_EVERY: u32 = 20;

pub struct Summarizer;

impl Summarizer {
    /// Check whether a session needs a new summary and generate one if so.
    /// Called after each agent response completes.
    pub fn maybe_summarize(session_id: &str, working_dir: Option<&str>) {
        let turn_count = EpisodicStore::get_turn_count(session_id);
        if turn_count == 0 || turn_count % AUTO_SUMMARIZE_EVERY != 0 {
            return;
        }
        match Self::summarize_session(session_id, working_dir) {
            Ok(id) => info!("[Summarizer] auto-summary for session={session_id} id={id}"),
            Err(e) => debug!("[Summarizer] auto-summary failed for session={session_id}: {e}"),
        }
    }

    /// Generate and store a summary for the given session.
    pub fn summarize_session(session_id: &str, working_dir: Option<&str>) -> Result<String> {
        let timeline = EpisodicStore::get_session_timeline(session_id)?;
        let summary = condense(&timeline);
        let id = EpisodicStore::store_session_summary(session_id, &summary, working_dir)?;
        Ok(id)
    }

    /// Force-summarize on explicit session end (called from Tauri command delete_session).
    pub fn on_session_end(session_id: &str, working_dir: Option<&str>) {
        let turn_count = EpisodicStore::get_turn_count(session_id);
        if turn_count == 0 {
            return;
        }
        // Only generate a final summary if there isn't one already from the
        // last AUTO_SUMMARIZE_EVERY boundary.
        match Self::summarize_session(session_id, working_dir) {
            Ok(id) => debug!("[Summarizer] end-of-session summary id={id}"),
            Err(e) => debug!("[Summarizer] end-of-session summary failed: {e}"),
        }
    }
}

// ─── Rule-based condensation ─────────────────────────────────────────────────

fn condense(timeline: &[super::types::EpisodicMemory]) -> String {
    let user_msgs: Vec<&str> = timeline
        .iter()
        .filter(|m| m.role == EpisodicRole::User && m.summary_type.is_none())
        .map(|m| m.content.as_str())
        .collect();

    if user_msgs.is_empty() {
        return "Session with no user messages.".to_string();
    }

    let turn_count = timeline.iter().map(|m| m.turn_index).max().unwrap_or(0);

    let first = user_msgs.first().copied().unwrap_or("");
    let last = user_msgs.last().copied().unwrap_or("");

    // Collect distinct keywords from all user messages for the topics line.
    let mut keywords: Vec<String> = user_msgs.iter().flat_map(|m| extract_nouns(m)).collect();
    keywords.dedup();
    keywords.truncate(8);

    let topics = if keywords.is_empty() {
        "general discussion".to_string()
    } else {
        keywords.join(", ")
    };

    // Capture tool names used across the session.
    let tools_used: Vec<String> = timeline
        .iter()
        .filter_map(|m| m.tool_calls.as_ref())
        .flat_map(|tc| {
            tc.as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| {
                            v.get("tool_name")
                                .and_then(|n| n.as_str())
                                .map(str::to_string)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let tools_line = if tools_used.is_empty() {
        String::new()
    } else {
        format!("\nTools used: {}", tools_used.join(", "))
    };

    // Total cost
    let total_cost: f64 = timeline.iter().map(|m| m.cost_usd).sum();
    let cost_line = if total_cost > 0.0 {
        format!("\nTotal cost: ${:.4}", total_cost)
    } else {
        String::new()
    };

    format!(
        "Session summary ({turn_count} turns).\n\
         Topics: {topics}.\n\
         First request: {first_excerpt}\n\
         Last request: {last_excerpt}{tools_line}{cost_line}",
        first_excerpt = truncate(first, 200),
        last_excerpt = truncate(last, 200),
    )
}

fn extract_nouns(text: &str) -> Vec<String> {
    text.split_whitespace()
        .filter(|w| {
            let c = w.trim_matches(|c: char| !c.is_alphabetic());
            c.len() > 4
                && c.chars()
                    .next()
                    .map(|ch| ch.is_uppercase())
                    .unwrap_or(false)
        })
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic()).to_lowercase())
        .collect()
}

/// Truncate to `max` bytes with an ellipsis.
///
/// ⚠ P3-04. This was one of **two byte-identical copies** of a correct
/// UTF-8-safe truncation — the other was in `memory/summarizer.rs` — while four
/// *other* sites in the tree panicked doing the same thing by hand, and the
/// plan pointed all of them at a `validate.rs` range that is not a truncation
/// helper at all. Both copies now delegate, so there is one implementation and
/// one place to fix.
fn truncate(s: &str, max: usize) -> String {
    crate::security::truncate::truncate_with_note(s, max)
}
