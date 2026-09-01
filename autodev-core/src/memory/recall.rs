// Memory recall + context assembler — Phase 4.4
// Combines all three tiers into a structured memory block injected into the
// system prompt before every PA invocation.
//
// Injection limits (per spec):
//   Episodic   — 5 most-recent session summaries from OTHER sessions
//   Semantic   — 10 highest-confidence facts
//   Procedural — 3 most-relevant topic excerpts (keyword match on query)
//
// The assembled block is appended to the base system prompt only when at
// least one tier has content, so prompts are not polluted with empty sections.

use tracing::debug;

use super::episodic::EpisodicStore;
use super::procedural::ProceduralStore;
use super::semantic::SemanticStore;
use super::types::{MemoryStats, MemoryTier, RecallRequest, RecallResult};
use crate::errors::Result;

// ─── Per-spec injection limits ───────────────────────────────────────────────

const EPISODIC_LIMIT: u32 = 5;
const SEMANTIC_LIMIT: u32 = 10;
const PROCEDURAL_LIMIT: u32 = 3;

// ─── Public API ──────────────────────────────────────────────────────────────

/// Build the `## Memory` section injected into the system prompt.
/// Returns `None` when all tiers are empty (avoids a blank section).
pub fn build_memory_context(session_id: &str, query: &str, _working_dir: &str) -> Option<String> {
    let summaries =
        EpisodicStore::get_recent_summaries(session_id, EPISODIC_LIMIT).unwrap_or_default();
    let facts = SemanticStore::recall_facts(SEMANTIC_LIMIT).unwrap_or_default();
    let procs = ProceduralStore::search(query, PROCEDURAL_LIMIT).unwrap_or_default();

    if summaries.is_empty() && facts.is_empty() && procs.is_empty() {
        return None;
    }

    let mut parts = vec!["## Memory (from previous sessions)\n".to_string()];

    if !summaries.is_empty() {
        parts.push("### Recent Session Summaries\n".to_string());
        for s in &summaries {
            let date = s.created_at.format("%Y-%m-%d").to_string();
            parts.push(format!("- [{date}] {}\n", truncate(&s.content, 300)));
        }
    }

    if !facts.is_empty() {
        parts.push("\n### What I know about you and this project\n".to_string());
        for f in &facts {
            let conf_label = if f.confidence >= 0.95 {
                "confirmed"
            } else {
                "likely"
            };
            parts.push(format!("- ({conf_label}) {}\n", f.fact_text));
        }
    }

    if !procs.is_empty() {
        parts.push("\n### Relevant patterns & learnings\n".to_string());
        for p in &procs {
            let excerpt = truncate(&p.content, 200);
            parts.push(format!("- **{}**: {excerpt}\n", p.title));
        }
    }

    let block = parts.join("");
    debug!("[recall] assembled memory block ({} chars)", block.len());
    Some(block)
}

/// Cross-tier search — used by the `search_memory` builtin tool.
pub fn search_all(req: &RecallRequest) -> Result<Vec<RecallResult>> {
    let limit = req.limit.unwrap_or(20);
    let mut results: Vec<RecallResult> = Vec::new();

    let allowed = |tier: MemoryTier| -> bool {
        req.tier_filter
            .as_ref()
            .map(|f| f.contains(&tier))
            .unwrap_or(true)
    };

    if allowed(MemoryTier::Episodic) {
        let summaries = EpisodicStore::get_recent_summaries(&req.session_id, limit)?;
        for s in summaries {
            let score = keyword_score(&s.content, &req.query);
            if score > 0.0 {
                results.push(RecallResult {
                    tier: MemoryTier::Episodic,
                    id: s.id,
                    content: truncate(&s.content, 400),
                    score,
                    created_at: s.created_at,
                });
            }
        }
    }

    if allowed(MemoryTier::Semantic) {
        let facts = SemanticStore::search(&req.query, limit)?;
        for f in facts {
            let score = keyword_score(&f.fact_text, &req.query) * f.confidence;
            results.push(RecallResult {
                tier: MemoryTier::Semantic,
                id: f.id,
                content: f.fact_text,
                score,
                created_at: f.created_at,
            });
        }
    }

    if allowed(MemoryTier::Procedural) {
        let procs = ProceduralStore::search(&req.query, limit)?;
        for p in procs {
            let score = keyword_score(&p.content, &req.query);
            results.push(RecallResult {
                tier: MemoryTier::Procedural,
                id: p.id,
                content: format!("**{}**: {}", p.title, truncate(&p.content, 300)),
                score,
                created_at: p.created_at,
            });
        }
    }

    // Sort by score descending, then by recency.
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    results.truncate(limit as usize);
    Ok(results)
}

/// Aggregate statistics across all three tiers.
pub fn get_stats() -> MemoryStats {
    let last_updated = super::db::MemoryDb::open().ok().and_then(|db| {
        db.conn
            .query_row("SELECT MAX(created_at) FROM episodic_memories", [], |r| {
                r.get::<_, Option<String>>(0)
            })
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
    });

    MemoryStats {
        episodic_count: EpisodicStore::count_total(),
        semantic_count: SemanticStore::count_total(),
        procedural_count: ProceduralStore::count_total(),
        session_count: EpisodicStore::count_sessions(),
        last_updated,
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Naïve keyword relevance: fraction of query words found in text (0.0–1.0).
fn keyword_score(text: &str, query: &str) -> f64 {
    if query.trim().is_empty() {
        return 1.0; // empty query: everything matches
    }
    let text_lower = text.to_lowercase();
    let query_words: Vec<&str> = query.split_whitespace().filter(|w| w.len() > 2).collect();
    if query_words.is_empty() {
        return 1.0;
    }
    let hits = query_words
        .iter()
        .filter(|w| text_lower.contains(*w))
        .count();
    hits as f64 / query_words.len() as f64
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
