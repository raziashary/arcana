// The memory gateway (`P3-39`, `P3-40`, `P3-47`).
//
// ── What this replaces, and why ──────────────────────────────────────────────
//
// `remember_this` gave any agent an **unrestricted write** into the user's
// long-term memory. Spec `03 §4.5` removes it in favour of `propose_memory`,
// behind a deterministic evaluation — the doctrine boundary from `00 §Part 2`
// at its clearest:
//
//   judging *what was learned*      → ORGAN (the model proposes)
//   deciding *whether to persist it* → DET   (this file decides)
//   storing / embedding / indexing   → DET
//
// ── 🔴 The defect that made this urgent ──────────────────────────────────────
//
// `remember_this` wrote with `confidence = 1.0` and `source_type =
// "user_explicit"`, and `recall_facts` had **no scope predicate** and ordered by
// `confidence DESC`. Every hop verified:
//
//   `runner.rs` runs recipes as `PAMode::FastDevelopment`
//     → `builtin_tools/mod.rs` registers `remember_this` in EVERY mode
//     → the write lands at confidence 1.0, labelled as the user's own words
//     → `recall_facts` sorts it to the TOP of the interactive user's
//       "What I know about you and this project"
//
// So an autonomous fleet worker could put a sentence in the user's mouth. It was
// dormant only because eight of nine recipes could not run, and `P4-31` made
// them run earlier the same day.
//
// **Confidence and provenance are therefore assigned HERE, from the caller's
// identity — never taken from the proposer.** An agent cannot claim to be the
// user, because it is not asked.

use crate::errors::Result;
use crate::memory::types::MemoryTier;

/// Who is proposing a memory.
///
/// Not a string the caller passes: it is derived from the agent identity the
/// runtime already threads through `ToolContext`, so a proposer cannot elect to
/// be someone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Proposer {
    /// The human, quoted by the assistant in an interactive session.
    User,
    /// The interactive assistant, recording its own observation.
    Assistant,
    /// An autonomous fleet worker or subagent.
    AutonomousAgent,
}

impl Proposer {
    /// The provenance value stored for this proposer (`P3-38`).
    pub fn source(self) -> &'static str {
        match self {
            Proposer::User => "user",
            Proposer::Assistant => "agent_observation",
            Proposer::AutonomousAgent => "agent_observation",
        }
    }

    /// The **ceiling** on the confidence this proposer may claim (`P3-47`).
    ///
    /// ⚠ A ceiling, not a default: a proposer may ask for less and never for
    /// more. Only something the user actually said reaches 1.0, and an
    /// autonomous agent is capped well below the `0.7` recall threshold's
    /// comfort zone so its assertions cannot outrank the user's by construction.
    pub fn confidence_ceiling(self) -> f32 {
        match self {
            Proposer::User => 1.0,
            Proposer::Assistant => 0.75,
            Proposer::AutonomousAgent => 0.6,
        }
    }

    /// Derive the proposer from the agent role the runtime already carries.
    ///
    /// The interactive assistant is `supervisor`; everything else that reaches
    /// a tool is a worker, a QC reviewer or a subagent — i.e. autonomous.
    pub fn from_agent_role(role: &str, user_stated: bool) -> Self {
        if user_stated {
            Proposer::User
        } else if role == "supervisor" || role.is_empty() {
            Proposer::Assistant
        } else {
            Proposer::AutonomousAgent
        }
    }
}

/// A proposal, before the gateway has ruled on it.
#[derive(Debug, Clone)]
pub struct Proposal {
    pub content: String,
    pub tier: MemoryTier,
    pub scope: String,
    pub scope_ref: String,
    pub proposer: Proposer,
    /// What the proposer *claims*. Clamped by the ceiling; never trusted.
    pub claimed_confidence: f32,
    pub source_file: Option<String>,
    pub source_line: Option<i64>,
}

/// The gateway's ruling.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Stored { id: String, confidence: f32 },
    Rejected { reason: String },
}

impl Verdict {
    pub fn is_stored(&self) -> bool {
        matches!(self, Verdict::Stored { .. })
    }
}

/// Minimum length for a memory to be worth keeping.
const MIN_CONTENT_CHARS: usize = 12;
/// Above this Jaccard overlap with an existing memory in the same scope, a
/// proposal is a duplicate.
const DUPLICATE_THRESHOLD: f32 = 0.85;

/// Importance, derived rather than claimed (`P3-40`).
///
/// Deterministic policy, per the doctrine table: the model may judge *what* was
/// learned, but *whether it is worth keeping* is arithmetic we can audit.
pub fn importance_of(p: &Proposal) -> f32 {
    let mut score: f32 = 0.4;

    // Something the user stated is more durable than something inferred.
    score += match p.proposer {
        Proposer::User => 0.3,
        Proposer::Assistant => 0.1,
        Proposer::AutonomousAgent => 0.0,
    };

    // A memory tied to a place in the repository is checkable later.
    if p.source_file.is_some() {
        score += 0.1;
    }

    // Project- and task-scoped memories are more specific than global ones.
    if p.scope != "global" {
        score += 0.1;
    }

    score.clamp(0.0, 1.0)
}

/// Word-level Jaccard overlap. Cheap, order-insensitive, and good enough to
/// catch a restatement — which is what duplication means here.
fn overlap(a: &str, b: &str) -> f32 {
    let norm = |s: &str| -> std::collections::BTreeSet<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect()
    };
    let (x, y) = (norm(a), norm(b));
    if x.is_empty() || y.is_empty() {
        return 0.0;
    }
    let inter = x.intersection(&y).count() as f32;
    let union = x.union(&y).count() as f32;
    inter / union
}

/// Evaluate and, if it passes, persist.
///
/// The four criteria are the anchor's §18 list — importance, novelty,
/// duplication, confidence — applied as `DET` policy.
pub fn propose(conn: &rusqlite::Connection, p: &Proposal) -> Result<Verdict> {
    let content = p.content.trim();

    // ── Substance ────────────────────────────────────────────────────────
    if content.chars().filter(|c| c.is_alphanumeric()).count() < MIN_CONTENT_CHARS {
        return Ok(Verdict::Rejected {
            reason: format!(
                "too short to be a memory ({} characters of content; {MIN_CONTENT_CHARS} required)",
                content.len()
            ),
        });
    }

    // ── Confidence, assigned rather than accepted (P3-47) ────────────────
    let ceiling = p.proposer.confidence_ceiling();
    let confidence = p.claimed_confidence.clamp(0.0, ceiling);

    // ── Duplication, within the proposal's own scope ─────────────────────
    // Scoped deliberately: the same sentence about two different projects is
    // two memories, not one.
    let mut stmt = conn.prepare(
        "SELECT id, content FROM memories WHERE scope_key = ?1 AND tier = ?2 LIMIT 500",
    )?;
    let scope_key = scope_key_of(&p.scope, &p.scope_ref);
    let existing: Vec<(String, String)> = stmt
        .query_map(
            rusqlite::params![scope_key, crate::memory::backend::tier_name(p.tier)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?
        .filter_map(|r| r.ok())
        .collect();

    for (id, other) in &existing {
        if overlap(content, other) >= DUPLICATE_THRESHOLD {
            return Ok(Verdict::Rejected {
                reason: format!("duplicate of an existing memory ({id})"),
            });
        }
    }

    // ── Store ────────────────────────────────────────────────────────────
    let id = format!("mem-{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO memories
            (id, tier, scope, scope_ref, scope_key, content, source, source_file, source_line,
             importance, confidence, access_count, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,0,?12,?12)",
        rusqlite::params![
            id,
            crate::memory::backend::tier_name(p.tier),
            p.scope,
            p.scope_ref,
            scope_key,
            content,
            p.proposer.source(),
            p.source_file,
            p.source_line,
            importance_of(p),
            confidence,
            now,
        ],
    )?;

    Ok(Verdict::Stored { id, confidence })
}

pub fn scope_key_of(scope: &str, scope_ref: &str) -> String {
    if scope == "global" || scope_ref.is_empty() {
        "global".to_string()
    } else {
        format!("{scope}:{scope_ref}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> rusqlite::Connection {
        crate::memory::vector::register_once();
        let c = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", crate::memory::db::MemoryDb::LADDER)
            .unwrap();
        c
    }

    fn proposal(content: &str, who: Proposer) -> Proposal {
        Proposal {
            content: content.to_string(),
            tier: MemoryTier::Semantic,
            scope: "project".into(),
            scope_ref: "/proj".into(),
            proposer: who,
            claimed_confidence: 1.0,
            source_file: None,
            source_line: None,
        }
    }

    /// 🔴 `P3-47`. An agent cannot put words in the user's mouth.
    #[test]
    fn an_autonomous_agent_cannot_claim_the_users_confidence() {
        let c = db();
        let v = propose(&c, &proposal("the deploy key rotates monthly", Proposer::AutonomousAgent))
            .unwrap();

        match v {
            Verdict::Stored { confidence, .. } => assert!(
                confidence <= 0.6,
                "an autonomous agent stored at confidence {confidence} — it asked \
                 for 1.0 and the ceiling is what stops it outranking the user"
            ),
            other => panic!("expected storage, got {other:?}"),
        }

        // …and it is recorded as an observation, never as the user's words.
        let source: String = c
            .query_row("SELECT source FROM memories LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(source, "agent_observation");
    }

    #[test]
    fn only_the_user_reaches_full_confidence() {
        let c = db();
        let v = propose(&c, &proposal("I prefer tabs over spaces", Proposer::User)).unwrap();
        match v {
            Verdict::Stored { confidence, .. } => assert_eq!(confidence, 1.0),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_proposer_may_ask_for_less_but_never_for_more() {
        let c = db();
        let mut p = proposal("the build is usually green on Fridays", Proposer::Assistant);
        p.claimed_confidence = 0.2;
        match propose(&c, &p).unwrap() {
            // A ceiling, not a default: modesty is honoured.
            Verdict::Stored { confidence, .. } => assert!((confidence - 0.2).abs() < 1e-6),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_proposer_is_derived_from_the_agent_role_not_chosen() {
        assert_eq!(Proposer::from_agent_role("supervisor", false), Proposer::Assistant);
        assert_eq!(Proposer::from_agent_role("worker-backend", false), Proposer::AutonomousAgent);
        assert_eq!(Proposer::from_agent_role("qc-agent", false), Proposer::AutonomousAgent);
        // Only an explicit user statement reaches User, and that flag is set by
        // the tool from the conversation, not by the model's say-so.
        assert_eq!(Proposer::from_agent_role("worker-backend", true), Proposer::User);
    }

    #[test]
    fn a_duplicate_is_rejected_within_its_scope() {
        let c = db();
        let p = proposal("the deploy key rotates monthly on the first", Proposer::User);
        assert!(propose(&c, &p).unwrap().is_stored());

        // A restatement, not a byte-identical repeat.
        let again = proposal("the deploy key rotates on the first, monthly", Proposer::User);
        match propose(&c, &again).unwrap() {
            Verdict::Rejected { reason } => assert!(reason.contains("duplicate"), "{reason}"),
            other => panic!("a restatement was stored again: {other:?}"),
        }
    }

    #[test]
    fn the_same_sentence_about_a_different_project_is_not_a_duplicate() {
        // Duplication is scoped on purpose: two projects may share a fact and
        // each needs its own, or one project's memory answers for the other.
        let c = db();
        assert!(propose(&c, &proposal("the api runs on port 8080", Proposer::User)).unwrap().is_stored());

        let mut other = proposal("the api runs on port 8080", Proposer::User);
        other.scope_ref = "/different".into();
        assert!(
            propose(&c, &other).unwrap().is_stored(),
            "a different project's identical fact was rejected as a duplicate"
        );
    }

    #[test]
    fn trivial_content_is_refused() {
        let c = db();
        for junk in ["ok", "yes", "   ", "!!!", "a b"] {
            match propose(&c, &proposal(junk, Proposer::User)).unwrap() {
                Verdict::Rejected { .. } => {}
                other => panic!("{junk:?} was stored: {other:?}"),
            }
        }
    }

    #[test]
    fn importance_is_derived_and_rewards_what_is_checkable() {
        let user = proposal("the deploy key rotates monthly", Proposer::User);
        let agent = proposal("the deploy key rotates monthly", Proposer::AutonomousAgent);
        assert!(
            importance_of(&user) > importance_of(&agent),
            "a user's statement should outweigh an agent's inference"
        );

        let mut cited = agent.clone();
        cited.source_file = Some("src/deploy.rs".into());
        assert!(
            importance_of(&cited) > importance_of(&agent),
            "a memory tied to a place in the repository is checkable and should score higher"
        );
    }

    #[test]
    fn a_stored_proposal_is_immediately_searchable() {
        // The FTS5 triggers must fire for gateway writes too — a memory that
        // cannot be found is not stored in any useful sense.
        let c = db();
        assert!(propose(&c, &proposal("the deploy key rotates monthly", Proposer::User))
            .unwrap()
            .is_stored());

        let n: i64 = c
            .query_row(
                "SELECT count(*) FROM memories_fts WHERE memories_fts MATCH 'deploy'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "a gateway write was not indexed for search");
    }
}
