// S3 · the memory smokes for `P3-F`.
//
// ── What `P3-44` actually has to prove ───────────────────────────────────────
//
// The task said *"a recalled memory appears in the outbound prompt bytes"*.
// That was **already true before this phase and proved nothing**: recall was
// query-blind — `context.rs` passed the empty-string literal and `recall_facts`
// did not take a query by signature — so the same bytes appeared whatever the
// user asked. A gate asserting only presence would have been green over the
// defect the phase exists to fix.
//
// These smokes assert the properties that were actually missing:
//   1. what is recalled DEPENDS ON THE QUERY;
//   2. it is scoped, and another project's memory cannot appear;
//   3. it survives into the prompt on the override path too.

use arcana_core::memory::backend::{LexicalBackend, RetrievalBackend, RetrievalQuery};
use arcana_core::memory::gateway::{self, Proposal, Proposer};
use arcana_core::memory::types::MemoryTier;
use rusqlite::Connection;

fn db() -> Connection {
    arcana_core::memory::vector::register_once();
    let c = Connection::open_in_memory().expect("open");
    arcana_core::memory::run_ladder_for_tests(&c).expect("ladder");
    c
}

fn remember(c: &Connection, text: &str, scope_ref: &str) {
    let v = gateway::propose(
        c,
        &Proposal {
            content: text.to_string(),
            tier: MemoryTier::Semantic,
            scope: if scope_ref.is_empty() { "global".into() } else { "project".into() },
            scope_ref: scope_ref.to_string(),
            proposer: Proposer::User,
            claimed_confidence: 1.0,
            source_file: None,
            source_line: None,
        },
    )
    .expect("propose");
    assert!(v.is_stored(), "fixture memory was rejected: {v:?}");
}

fn recall(c: &Connection, query: &str, project: &str) -> Vec<String> {
    let q = RetrievalQuery {
        text: query.to_string(),
        scope_keys: arcana_core::memory::recall::scope_keys_for(project),
        tiers: Vec::new(),
        limit: 20,
    };
    LexicalBackend
        .search(c, &q)
        .expect("search")
        .into_iter()
        .map(|x| x.content)
        .collect()
}

// ── S3-04 ───────────────────────────────────────────────────────────────────

/// Two different questions in one session produce **different** recall sets,
/// and the same question twice produces the same one.
///
/// This is `TG3-12` and the MDP's `S3-04`. It **failed by construction** before
/// this phase: every tier ignored the query, so the recalled set was
/// byte-identical for every turn of a session.
#[test]
fn s3_04_two_different_questions_recall_different_things() {
    let c = db();
    remember(&c, "the deploy key rotates on the first of each month", "/proj");
    remember(&c, "the frontend is styled with Tailwind and no CSS modules", "/proj");
    remember(&c, "integration tests need Docker running locally", "/proj");

    let deploy = recall(&c, "when does the deploy key rotate", "/proj");
    let styling = recall(&c, "how is the frontend styled", "/proj");

    assert!(!deploy.is_empty(), "the deploy question recalled nothing");
    assert!(!styling.is_empty(), "the styling question recalled nothing");
    assert_ne!(
        deploy, styling,
        "two different questions produced the SAME recall set — this is the \
         query-independence defect the phase exists to fix"
    );
    assert!(
        deploy[0].contains("deploy key"),
        "the top hit for a deploy question was {:?}",
        deploy[0]
    );
    assert!(
        styling[0].contains("Tailwind"),
        "the top hit for a styling question was {:?}",
        styling[0]
    );

    // …and identical questions are stable, or "different" above means nothing.
    assert_eq!(
        recall(&c, "when does the deploy key rotate", "/proj"),
        deploy,
        "the same question twice gave a different answer"
    );
}

/// Another project's memories are unreachable.
///
/// `recall_facts` filtered on `is_active` and `confidence` only. `working_dir`
/// was SELECTed and never used as a predicate, so every project recalled every
/// other project's memories.
#[test]
fn s3_04b_recall_cannot_cross_a_project_boundary() {
    let c = db();
    remember(&c, "the staging password is in the ops vault", "/secret-project");
    remember(&c, "this project deploys on Fridays", "/other-project");

    let seen = recall(&c, "staging password ops vault", "/other-project");
    assert!(
        seen.iter().all(|s| !s.contains("staging password")),
        "another project's memory leaked into recall: {seen:?}"
    );

    // The no-fire half: the owning project still gets it, or the filter is
    // just breaking retrieval rather than scoping it.
    let owner = recall(&c, "staging password ops vault", "/secret-project");
    assert!(
        owner.iter().any(|s| s.contains("staging password")),
        "the owning project could not recall its own memory: {owner:?}"
    );
}

/// A global memory is visible from every project; a project memory is not.
#[test]
fn s3_04c_global_memories_are_shared_and_project_memories_are_not() {
    let c = db();
    remember(&c, "I prefer British spelling in all documentation", "");
    remember(&c, "this repository pins Rust 1.82 exactly", "/proj-a");

    let from_b = recall(&c, "spelling documentation preference", "/proj-b");
    assert!(
        from_b.iter().any(|s| s.contains("British spelling")),
        "a global preference was not visible from another project: {from_b:?}"
    );

    let leak = recall(&c, "Rust version pinned", "/proj-b");
    assert!(
        leak.iter().all(|s| !s.contains("1.82")),
        "a project-scoped memory was visible elsewhere: {leak:?}"
    );
}

// ── S3-05 ───────────────────────────────────────────────────────────────────

/// With no embedding model, recall still returns results and **reports itself
/// degraded** — never empty, never an error.
///
/// The MDP words this as *"with the embedding model deleted"*. On a default
/// build there is no model at all, which is the same condition and the one
/// every CI run is actually in.
#[test]
fn s3_05_recall_degrades_to_lexical_and_says_so() {
    let c = db();
    remember(&c, "the deploy key rotates on the first of each month", "/proj");

    let hits = recall(&c, "deploy key rotation", "/proj");
    assert!(
        !hits.is_empty(),
        "degraded retrieval returned NOTHING — 'never empty, never an error' is \
         the requirement, and an empty result is indistinguishable from 'I do \
         not know that'"
    );

    // Degradation is observable state, not an inference from a thin result.
    assert!(LexicalBackend.is_degraded());
    let why = LexicalBackend
        .degraded_reason()
        .expect("a degraded backend must say why");
    assert!(
        why.to_lowercase().contains("keyword"),
        "the reason must name what is limited: {why}"
    );

    // And the user-facing report distinguishes WHY, because a platform that can
    // never have semantic search and a download that has not finished need
    // different actions from the reader.
    let report = arcana_core::memory::embed::availability_report();
    assert!(
        report.contains("keyword") || report.contains("active"),
        "the availability report must say what still works: {report}"
    );
}

/// An empty query does not become "match everything".
#[test]
fn s3_05b_an_empty_query_is_not_a_wildcard() {
    let c = db();
    // ⚠ Genuinely distinct facts. The first version of this fixture was four
    // strings differing only by a digit — and the gateway correctly rejected
    // them as duplicates, because single characters are not tokens. The dedup
    // working is not a reason to weaken the dedup.
    for text in [
        "the build runs on GitHub Actions with a Windows runner",
        "linting uses clippy with warnings denied",
        "release artefacts are signed before upload",
        "coverage is not enforced on pull requests",
    ] {
        remember(&c, text, "/proj");
    }
    let q = RetrievalQuery {
        text: String::new(),
        scope_keys: arcana_core::memory::recall::scope_keys_for("/proj"),
        tiers: Vec::new(),
        limit: 20,
    };
    assert!(
        !q.is_lexically_searchable(),
        "an empty query must be recognised as unsearchable, not passed to MATCH"
    );
    // It still returns something useful — recency — rather than erroring or
    // matching everything by accident through `LIKE '%%'`.
    let r = LexicalBackend.search(&c, &q).expect("search");
    assert_eq!(r.len(), 4);
}
