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

/// The scopes a recall may draw from, in the order they are declared.
///
/// 🔴 `P3-31`. Scope is a **mandatory** filter and an empty list means
/// **nothing**, never everything. The sidecar's episodic recall never forwarded
/// `agent_id` despite the callee supporting it, and procedural recall had no
/// type filter at all — "unscoped" is precisely how cross-project and
/// cross-agent contamination happened.
pub fn scope_keys_for(working_dir: &str) -> Vec<String> {
    let mut keys = vec!["global".to_string()];
    if !working_dir.trim().is_empty() {
        keys.push(format!("project:{working_dir}"));
    }
    keys
}

/// The result of a recall, including whether it was degraded.
pub struct Recalled {
    pub block: Option<String>,
    pub degraded: Option<String>,
    pub ids: Vec<String>,
}

/// Build the `## Memory` section injected into the system prompt.
///
/// 🔴 This function's third parameter was `_working_dir` — bound to an
/// underscore and discarded — and its second was passed the **empty string
/// literal** by the only production caller. Semantic recall did not take a
/// query *by signature*; episodic was recency-only; procedural received `""`
/// and did `LIKE '%%'`, matching every row. **All three tiers were
/// query-independent, so the injected block was byte-identical every turn**,
/// whatever the user asked. `TG3-12` failed by construction.
///
/// Both parameters are now load-bearing: `query` drives retrieval and
/// `working_dir` determines scope.
pub fn recall(session_id: &str, query: &str, working_dir: &str) -> Recalled {
    let _ = session_id; // reserved: session-anchored boosting (OQ-7)

    let db = match super::db::MemoryDb::open() {
        Ok(db) => db,
        Err(e) => {
            debug!("[recall] memory unavailable: {e}");
            return Recalled { block: None, degraded: Some(format!("memory unavailable: {e}")), ids: Vec::new() };
        }
    };

    let backend = super::manager::active_backend();
    let budget = super::rank::TierBudget::default();
    let q = super::backend::RetrievalQuery {
        text: query.to_string(),
        scope_keys: scope_keys_for(working_dir),
        tiers: Vec::new(),
        // Over-fetch, then let the reranker and the budget decide. Fetching
        // exactly the budget would make the budget the ranking.
        limit: budget.total() * 4,
    };

    let candidates = match backend.search(&db.conn, &q) {
        Ok(c) => c,
        Err(e) => {
            debug!("[recall] search failed: {e}");
            return Recalled { block: None, degraded: Some(format!("retrieval failed: {e}")), ids: Vec::new() };
        }
    };

    let ranked = super::rank::rank_and_budget(
        candidates,
        &budget,
        &super::rank::RerankWeights::default(),
        working_dir,
        chrono::Utc::now(),
    );

    if ranked.is_empty() {
        return Recalled {
            block: None,
            degraded: backend.degraded_reason(),
            ids: Vec::new(),
        };
    }

    let ids: Vec<String> = ranked.iter().map(|s| s.candidate.id.clone()).collect();
    record_access(&db.conn, &ids);

    Recalled {
        block: Some(render(&ranked, backend.degraded_reason().is_some())),
        degraded: backend.degraded_reason(),
        ids,
    }
}

/// Back-compatible entry point for the prompt assembler.
pub fn build_memory_context(session_id: &str, query: &str, working_dir: &str) -> Option<String> {
    recall(session_id, query, working_dir).block
}

/// Render the ranked set, grouped by tier.
fn render(ranked: &[super::rank::Scored], degraded: bool) -> String {
    let mut parts = vec!["## Memory (from previous sessions)\n".to_string()];

    if degraded {
        // ⚠ Stated in the prompt, not only in a log. The model is the consumer
        // here, and it should know that a miss may mean "not matched" rather
        // than "not known" (`TG3-17`).
        parts.push(
            "_(Retrieval is running in keyword-only mode; a differently-worded \
             memory may not have surfaced.)_\n"
                .to_string(),
        );
    }

    for (tier, heading) in [
        (MemoryTier::Semantic, "### What I know about you and this project"),
        (MemoryTier::Episodic, "### Relevant moments from earlier sessions"),
        (MemoryTier::Procedural, "### Relevant patterns & learnings"),
    ] {
        let rows: Vec<&super::rank::Scored> = ranked
            .iter()
            .filter(|s| s.candidate.tier == tier)
            .collect();
        if rows.is_empty() {
            continue;
        }
        parts.push(format!("\n{heading}\n"));
        for s in rows {
            let c = &s.candidate;
            // Provenance is rendered (`P3-38`). A memory the agent inferred and
            // one the user stated are different kinds of thing, and the model
            // cannot weigh them if the prompt hides the difference.
            let origin = match c.source.as_str() {
                "user" => "you told me",
                "repository" => "from the repository",
                "tool_result" => "from a tool result",
                "test_result" => "from a test run",
                "agent_observation" => "I observed",
                _ => "from an earlier conversation",
            };
            parts.push(format!(
                "- ({origin}) {}\n",
                truncate(&c.content, 300)
            ));
        }
    }

    parts.join("")
}

/// Bump `access_count` / `last_accessed_at` for what was actually recalled.
///
/// The anchor asks for access frequency to be tracked (§11) and `P3-42` weights
/// it. Recording it at recall — the moment the memory is *used* — is what makes
/// the term mean anything; incrementing at write time would just re-count
/// creation.
fn record_access(conn: &rusqlite::Connection, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    let placeholders = (0..ids.len()).map(|i| format!("?{}", i + 2)).collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ?1
         WHERE id IN ({placeholders})"
    );
    let now = chrono::Utc::now().to_rfc3339();
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
    for id in ids {
        params.push(Box::new(id.clone()));
    }
    let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
    if let Err(e) = conn.execute(&sql, refs.as_slice()) {
        // A failure to record usage must never fail the recall itself.
        debug!("[recall] could not record access: {e}");
    }
}

/// Cross-tier search — used by the `search_memory` builtin tool.
pub fn search_all(req: &RecallRequest) -> Result<Vec<RecallResult>> {
    let db = super::db::MemoryDb::open()?;
    let backend = super::manager::active_backend();
    let limit = req.limit.unwrap_or(20) as usize;

    let tiers = req.tier_filter.clone().unwrap_or_default();
    let q = super::backend::RetrievalQuery {
        text: req.query.clone(),
        // The tool is scoped to the caller's workspace plus global, exactly as
        // prompt assembly is. A tool that could read every scope would be a
        // way around the filter that the prompt path enforces.
        scope_keys: scope_keys_for(req.working_dir.as_deref().unwrap_or("")),
        tiers,
        limit: limit * 4,
    };

    let candidates = backend.search(&db.conn, &q)?;
    let ranked = super::rank::rank_and_budget(
        candidates,
        &super::rank::TierBudget { episodic: limit, semantic: limit, procedural: limit },
        &super::rank::RerankWeights::default(),
        req.working_dir.as_deref().unwrap_or(""),
        chrono::Utc::now(),
    );

    let ids: Vec<String> = ranked.iter().map(|s| s.candidate.id.clone()).collect();
    record_access(&db.conn, &ids);

    Ok(ranked
        .into_iter()
        .take(limit)
        .map(|s| RecallResult {
            tier: s.candidate.tier,
            id: s.candidate.id,
            content: truncate(&s.candidate.content, 400),
            score: s.score as f64,
            created_at: chrono::DateTime::parse_from_rfc3339(&s.candidate.created_at)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now()),
        })
        .collect())
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
