// Ranking and budgeting for recall (`P3-41`, `P3-42`).
//
// ── Two layers, deliberately separate ────────────────────────────────────────
//
// `P3-41` is **per-tier budgeting**: a simple, predictable share per tier so no
// tier can crowd out the others. `P3-42` is **the anchor's §10 reranker**,
// which orders candidates *within* what the budget allows.
//
// ⚠ `P3-42` exists as its own work item because the plan's earlier claim — that
// the anchor's reranker *is* the ported competitive model — is false. Both were
// read side by side during the pre-build verification:
//
//   anchor §10   semantic_relevance · lexical_relevance · importance · recency
//                · access_frequency · scope_match · task_similarity · confidence
//   competitive  relevance · authority · recency · type_bonus · priority
//                · correction · session_anchor · procedural_fit · length_penalty
//
// **Two terms overlap. Six of the anchor's eight are absent** from the
// competitive model — which is why this needed its own schema columns
// (`importance`, `access_count`, `last_accessed_at`, `scope`) rather than a
// port. Spec `03 §4.6` said otherwise and has been corrected.

use crate::memory::backend::Candidate;
use crate::memory::types::MemoryTier;

/// Per-tier caps (`P3-41`).
///
/// Simple and stated, not emergent. The competitive model lets a tier's share
/// fall out of `type_bonus`, `authority` and `session_anchor` — which is
/// sophisticated and, on a recall path that was returning the same rows every
/// turn, unprovable. `ADR-01-08` ships quotas first for exactly that reason;
/// the reranker below is interface-compatible with a later swap.
#[derive(Debug, Clone, Copy)]
pub struct TierBudget {
    pub episodic: usize,
    pub semantic: usize,
    pub procedural: usize,
}

impl Default for TierBudget {
    /// The limits the previous implementation used, preserved deliberately.
    ///
    /// They were the only part of the old recall path that was actually right,
    /// and changing them in the same change that fixes retrieval would make it
    /// impossible to attribute any behaviour difference to the fix.
    fn default() -> Self {
        Self {
            episodic: 5,
            semantic: 10,
            procedural: 3,
        }
    }
}

impl TierBudget {
    pub fn for_tier(&self, t: MemoryTier) -> usize {
        match t {
            MemoryTier::Episodic => self.episodic,
            MemoryTier::Semantic => self.semantic,
            MemoryTier::Procedural => self.procedural,
        }
    }

    pub fn total(&self) -> usize {
        self.episodic + self.semantic + self.procedural
    }
}

/// Weights for the anchor's §10 terms.
///
/// The anchor is explicit that *"the exact weighting should be determined
/// experimentally"*, so these are a starting point that is **stated rather than
/// buried**, and every term is one the schema can actually supply. A term with
/// no column would be a weight multiplied by a constant, which is a weight of
/// zero wearing a name.
#[derive(Debug, Clone, Copy)]
pub struct RerankWeights {
    pub semantic_relevance: f32,
    pub lexical_relevance: f32,
    pub importance: f32,
    pub recency: f32,
    pub access_frequency: f32,
    pub scope_match: f32,
    pub confidence: f32,
}

impl Default for RerankWeights {
    fn default() -> Self {
        Self {
            semantic_relevance: 0.30,
            lexical_relevance: 0.25,
            importance: 0.15,
            recency: 0.10,
            access_frequency: 0.05,
            scope_match: 0.10,
            confidence: 0.05,
        }
    }
}

/// A scored candidate, with its terms retained.
///
/// The breakdown is kept rather than collapsed to one number because a ranking
/// nobody can explain is a ranking nobody can debug — and because the anchor
/// asks for the weighting to be tuned, which needs the terms visible.
#[derive(Debug, Clone)]
pub struct Scored {
    pub candidate: Candidate,
    pub score: f32,
    pub terms: [(&'static str, f32); 7],
}

/// Recency with a 14-day half-life, per spec `03 §4.2`.
///
/// `exp(-ln2/14 · age_days)` — 1.0 today, 0.5 at a fortnight, and it never
/// reaches zero, because *"old memories should generally be deprioritized
/// before they are destroyed"* (anchor §11).
pub fn recency_score(created_at: &str, now: chrono::DateTime<chrono::Utc>) -> f32 {
    let parsed = crate::db::timestamps::parse_db_timestamp(created_at).ok();
    let Some(created) = parsed else {
        // An unparseable timestamp must not score as "brand new" — that would
        // make a malformed row outrank every real one.
        return 0.0;
    };
    let age_days = (now - created).num_seconds().max(0) as f32 / 86_400.0;
    (-std::f32::consts::LN_2 / 14.0 * age_days).exp()
}

/// Squash an unbounded count into 0..1 without a cliff.
fn saturating(x: f32, half_at: f32) -> f32 {
    if x <= 0.0 {
        0.0
    } else {
        x / (x + half_at)
    }
}

/// Score one candidate against the anchor's §10 terms.
///
/// `query_scope_ref` is the scope the caller asked from, so `scope_match` can
/// reward a memory that belongs to *this* project over one inherited from
/// `global`.
pub fn score(
    c: &Candidate,
    w: &RerankWeights,
    query_scope_ref: &str,
    now: chrono::DateTime<chrono::Utc>,
    max_lexical: f32,
) -> Scored {
    // Normalise lexical against the best in this result set; BM25 is unbounded
    // and comparing raw scores across queries is meaningless.
    let lexical = if max_lexical > 0.0 {
        (c.lexical_score / max_lexical).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // A vector backend reports distance (lower is closer); absent it, this term
    // is a true zero rather than a fabricated similarity.
    let semantic = match c.vector_distance {
        Some(d) => (1.0 / (1.0 + d)).clamp(0.0, 1.0),
        None => 0.0,
    };

    let scope_match = if !query_scope_ref.is_empty() && c.scope_ref == query_scope_ref {
        1.0
    } else if c.scope == "global" {
        0.5
    } else {
        0.0
    };

    let terms = [
        ("semantic_relevance", semantic),
        ("lexical_relevance", lexical),
        ("importance", c.importance.clamp(0.0, 1.0)),
        ("recency", recency_score(&c.created_at, now)),
        ("access_frequency", saturating(c.access_count as f32, 5.0)),
        ("scope_match", scope_match),
        ("confidence", c.confidence.clamp(0.0, 1.0)),
    ];

    let score = terms[0].1 * w.semantic_relevance
        + terms[1].1 * w.lexical_relevance
        + terms[2].1 * w.importance
        + terms[3].1 * w.recency
        + terms[4].1 * w.access_frequency
        + terms[5].1 * w.scope_match
        + terms[6].1 * w.confidence;

    Scored {
        candidate: c.clone(),
        score,
        terms,
    }
}

/// Rank, then apply the per-tier budget.
///
/// Order matters: rank **first**, budget **second**. Budgeting before ranking
/// would take an arbitrary N per tier and then order them, which is a recency
/// cut wearing a relevance label.
pub fn rank_and_budget(
    candidates: Vec<Candidate>,
    budget: &TierBudget,
    weights: &RerankWeights,
    query_scope_ref: &str,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<Scored> {
    let max_lexical = candidates
        .iter()
        .map(|c| c.lexical_score)
        .fold(0.0f32, f32::max);

    let mut scored: Vec<(Scored, Option<chrono::DateTime<chrono::Utc>>)> = candidates
        .iter()
        .map(|c| {
            (
                score(c, weights, query_scope_ref, now, max_lexical),
                crate::db::timestamps::parse_db_timestamp(&c.created_at).ok(),
            )
        })
        .collect();

    scored.sort_by(|(a, a_time), (b, b_time)| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            // Ties break on recency, then on id — so the order is TOTAL and a
            // recall is reproducible. A nondeterministic tie-break would make
            // `TG3-12`'s "the same query twice returns the same set" flaky for
            // reasons nobody could reproduce.
            .then_with(|| match (a_time, b_time) {
                (Some(a_time), Some(b_time)) => b_time.cmp(a_time),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.candidate.created_at.cmp(&a.candidate.created_at),
            })
            .then_with(|| a.candidate.id.cmp(&b.candidate.id))
    });

    let mut taken = std::collections::HashMap::<&'static str, usize>::new();
    let mut out = Vec::new();
    for (s, _) in scored {
        let key = crate::memory::backend::tier_name(s.candidate.tier);
        let used = taken.entry(key).or_insert(0);
        if *used < budget.for_tier(s.candidate.tier) {
            *used += 1;
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn cand(id: &str, tier: MemoryTier) -> Candidate {
        Candidate {
            id: id.into(),
            tier,
            content: "c".into(),
            scope: "project".into(),
            scope_ref: "/proj".into(),
            source: "user".into(),
            importance: 0.5,
            confidence: 0.7,
            access_count: 0,
            created_at: Utc::now().to_rfc3339(),
            lexical_score: 1.0,
            vector_distance: None,
        }
    }

    #[test]
    fn recency_halves_at_a_fortnight() {
        let now = Utc::now();
        let fresh = recency_score(&now.to_rfc3339(), now);
        let two_weeks = recency_score(&(now - Duration::days(14)).to_rfc3339(), now);
        let a_month = recency_score(&(now - Duration::days(28)).to_rfc3339(), now);

        assert!(
            (fresh - 1.0).abs() < 0.01,
            "today should score ~1.0, got {fresh}"
        );
        assert!(
            (two_weeks - 0.5).abs() < 0.02,
            "14 days should halve, got {two_weeks}"
        );
        assert!(
            (a_month - 0.25).abs() < 0.02,
            "28 days should quarter, got {a_month}"
        );
        assert!(
            a_month > 0.0,
            "decay must never reach zero — deprioritise, do not destroy"
        );
    }

    #[test]
    fn an_unparseable_timestamp_scores_zero_not_new() {
        // A malformed row must not outrank every real one.
        assert_eq!(recency_score("not-a-date", Utc::now()), 0.0);
    }

    #[test]
    fn equal_score_ties_order_mixed_timestamp_formats_by_instant() {
        let mut canonical = cand("canonical-0900", MemoryTier::Semantic);
        canonical.created_at = "2026-09-02T09:00:00Z".into();
        let mut legacy = cand("legacy-1200", MemoryTier::Semantic);
        legacy.created_at = "2026-09-02 12:00:00".into();
        let weights = RerankWeights {
            recency: 0.0,
            ..RerankWeights::default()
        };

        let ranked = rank_and_budget(
            vec![canonical, legacy],
            &TierBudget {
                semantic: 2,
                ..TierBudget::default()
            },
            &weights,
            "/proj",
            Utc::now(),
        );

        assert_eq!(ranked[0].candidate.id, "legacy-1200");
        assert_eq!(ranked[1].candidate.id, "canonical-0900");
    }

    #[test]
    fn the_budget_caps_each_tier_independently() {
        let mut cs = Vec::new();
        for i in 0..20 {
            cs.push(cand(&format!("e{i}"), MemoryTier::Episodic));
        }
        for i in 0..20 {
            cs.push(cand(&format!("s{i}"), MemoryTier::Semantic));
        }
        let budget = TierBudget::default();
        let out = rank_and_budget(cs, &budget, &RerankWeights::default(), "/proj", Utc::now());

        let n_epi = out
            .iter()
            .filter(|s| matches!(s.candidate.tier, MemoryTier::Episodic))
            .count();
        let n_sem = out
            .iter()
            .filter(|s| matches!(s.candidate.tier, MemoryTier::Semantic))
            .count();
        assert_eq!(n_epi, budget.episodic);
        assert_eq!(n_sem, budget.semantic);
    }

    #[test]
    fn one_tier_cannot_crowd_out_another() {
        // The point of quotas: 100 episodic rows must not starve semantic.
        let mut cs: Vec<Candidate> = (0..100)
            .map(|i| {
                let mut c = cand(&format!("e{i}"), MemoryTier::Episodic);
                c.lexical_score = 10.0; // and they all outrank on relevance
                c
            })
            .collect();
        cs.push(cand("s1", MemoryTier::Semantic));

        let out = rank_and_budget(
            cs,
            &TierBudget::default(),
            &RerankWeights::default(),
            "/proj",
            Utc::now(),
        );
        assert!(
            out.iter().any(|s| s.candidate.id == "s1"),
            "the single semantic memory was crowded out by episodic ones"
        );
    }

    #[test]
    fn a_memory_from_this_project_outranks_an_equal_global_one() {
        // `scope_match` is one of the six terms the competitive model lacks,
        // and this is what it buys.
        let mut mine = cand("mine", MemoryTier::Semantic);
        mine.scope_ref = "/proj".into();
        let mut theirs = cand("global", MemoryTier::Semantic);
        theirs.scope = "global".into();
        theirs.scope_ref = String::new();

        let out = rank_and_budget(
            vec![theirs, mine],
            &TierBudget::default(),
            &RerankWeights::default(),
            "/proj",
            Utc::now(),
        );
        assert_eq!(
            out[0].candidate.id, "mine",
            "scope_match did not discriminate"
        );
    }

    #[test]
    fn ranking_is_total_so_the_same_query_is_reproducible() {
        // TG3-12's second half depends on this: identical inputs must give an
        // identical order, or "the same query twice returns the same set" is
        // flaky for reasons nobody can reproduce.
        let cs: Vec<Candidate> = (0..10)
            .map(|i| cand(&format!("m{i}"), MemoryTier::Semantic))
            .collect();
        let a = rank_and_budget(
            cs.clone(),
            &TierBudget::default(),
            &RerankWeights::default(),
            "/proj",
            Utc::now(),
        );
        let b = rank_and_budget(
            cs,
            &TierBudget::default(),
            &RerankWeights::default(),
            "/proj",
            Utc::now(),
        );
        assert_eq!(
            a.iter().map(|s| s.candidate.id.clone()).collect::<Vec<_>>(),
            b.iter().map(|s| s.candidate.id.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_weighted_term_is_one_the_schema_can_supply() {
        // A term with no column is a weight times a constant — a zero wearing a
        // name. This asserts each term actually varies with its input.
        let now = Utc::now();
        let base = cand("b", MemoryTier::Semantic);

        let mut important = base.clone();
        important.importance = 1.0;
        assert!(
            score(&important, &RerankWeights::default(), "/proj", now, 1.0).score
                > score(&base, &RerankWeights::default(), "/proj", now, 1.0).score,
            "importance does not move the score"
        );

        let mut accessed = base.clone();
        accessed.access_count = 50;
        assert!(
            score(&accessed, &RerankWeights::default(), "/proj", now, 1.0).score
                > score(&base, &RerankWeights::default(), "/proj", now, 1.0).score,
            "access_frequency does not move the score"
        );

        let mut confident = base.clone();
        confident.confidence = 1.0;
        assert!(
            score(&confident, &RerankWeights::default(), "/proj", now, 1.0).score
                > score(&base, &RerankWeights::default(), "/proj", now, 1.0).score,
            "confidence does not move the score"
        );
    }

    #[test]
    fn a_vector_distance_raises_the_semantic_term_and_its_absence_is_a_true_zero() {
        let now = Utc::now();
        let mut near = cand("near", MemoryTier::Semantic);
        near.vector_distance = Some(0.05);
        let without = cand("none", MemoryTier::Semantic);

        let s_near = score(&near, &RerankWeights::default(), "/proj", now, 1.0);
        let s_none = score(&without, &RerankWeights::default(), "/proj", now, 1.0);

        assert!(s_near.terms[0].1 > 0.9, "a near vector should score high");
        assert_eq!(
            s_none.terms[0].1, 0.0,
            "no vector backend must give a TRUE zero, not a fabricated similarity"
        );
    }
}
