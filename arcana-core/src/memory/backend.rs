// The retrieval-backend seam (`P3-30`) and its lexical leg (`P3-36`).
//
// ── Why a trait ──────────────────────────────────────────────────────────────
//
// Spec `03 §4.4` names three legs — dense vectors, keyword/BM25, metadata
// filtering — and requires the system to work when the first is unavailable:
// *"Degradation, not failure. Before the model is present — mid-download, or if
// skipped — retrieval falls back to FTS5/BM25 + metadata filtering. Memory
// works, less well, and reports that it is degraded."*
//
// That is a trait boundary, not a conditional: the lexical leg is a complete
// backend in its own right, not a branch inside the vector one. On macOS it is
// the ONLY backend (razi-ruled — no x86_64 ONNX Runtime exists upstream and the
// DMG is a universal binary), so it must be first-class rather than a fallback
// that nobody exercises.

use crate::errors::Result;
use crate::memory::types::MemoryTier;
use rusqlite::Connection;

/// One retrieved memory, before ranking.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub id: String,
    pub tier: MemoryTier,
    pub content: String,
    pub scope: String,
    pub scope_ref: String,
    pub source: String,
    pub importance: f32,
    pub confidence: f32,
    pub access_count: i64,
    pub created_at: String,
    /// Lexical relevance. Higher is better — `bm25()` returns a *negative*
    /// score where more negative is more relevant, so it is negated on the way
    /// out rather than leaving every consumer to remember that.
    pub lexical_score: f32,
    /// Vector distance where a vector backend produced it. Lower is closer.
    pub vector_distance: Option<f32>,
}

/// What to retrieve.
#[derive(Debug, Clone)]
pub struct RetrievalQuery {
    /// The user's actual question.
    ///
    /// 🔴 `P3-43`. The production call site passed the **empty string literal**
    /// here for the life of the module, and `recall_facts` did not take a query
    /// *by signature* — so the injected block was byte-identical every turn.
    /// This field exists to make that impossible to reintroduce silently: an
    /// empty query is a distinct, handled case (see `is_lexically_searchable`),
    /// not a query that matches everything.
    pub text: String,
    /// Scope pre-filter. **Mandatory** — an empty list retrieves nothing rather
    /// than everything, because "no scope" must never mean "all scopes"
    /// (`G-12`).
    pub scope_keys: Vec<String>,
    /// Tier filter; empty means all tiers.
    pub tiers: Vec<MemoryTier>,
    pub limit: usize,
}

impl RetrievalQuery {
    /// Whether this query can drive a lexical search at all.
    ///
    /// An empty or punctuation-only query is **not** a search for everything.
    /// The old procedural path did `LIKE '%%'` and matched every row, which is
    /// how "recall" degenerated into "the three most recently edited topics".
    pub fn is_lexically_searchable(&self) -> bool {
        self.text.chars().any(|c| c.is_alphanumeric())
    }
}

/// A retrieval backend. One per leg; the manager picks and reports which.
pub trait RetrievalBackend: Send + Sync {
    fn name(&self) -> &'static str;

    /// Whether this backend is operating below its full capability.
    ///
    /// ⚠ Observable state, not an inference from an empty result list. Spec
    /// `03 §4.4`: degradation must be *reported*, and `TG3-17` requires the
    /// caller be able to see it rather than deduce it.
    fn is_degraded(&self) -> bool;

    /// Why, in a sentence a user could read.
    fn degraded_reason(&self) -> Option<String>;

    fn search(&self, conn: &Connection, q: &RetrievalQuery) -> Result<Vec<Candidate>>;
}

// ─── Lexical (FTS5 / BM25) ───────────────────────────────────────────────────

/// FTS5 + BM25 over `memories`, with scope applied as a SQL pre-filter.
pub struct LexicalBackend;

impl LexicalBackend {
    /// Turn arbitrary user text into a **safe** FTS5 MATCH expression.
    ///
    /// ⚠ FTS5 has its own query syntax. Passing user text straight into MATCH
    /// is at best a syntax error on `AND`, `NEAR`, `*`, `"` or `:` — and at
    /// worst it silently changes what was asked for. Every token is therefore
    /// quoted and joined with OR, which is a relevance search rather than a
    /// syntax to be exploited.
    pub fn to_fts_query(text: &str) -> String {
        let tokens: Vec<String> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .take(32)
            .map(|t| format!("\"{}\"", t.replace('"', "")))
            .collect();
        tokens.join(" OR ")
    }
}

impl RetrievalBackend for LexicalBackend {
    fn name(&self) -> &'static str {
        "fts5-bm25"
    }

    /// Always degraded **relative to the full triple**, and it says so.
    ///
    /// This is not self-deprecation: `TG3-17` requires the caller to observe
    /// degradation rather than infer it, and a lexical-only system genuinely
    /// cannot answer a paraphrase. Reporting `false` here because results came
    /// back would be the `P3-18` shape — output that looks complete and is not.
    fn is_degraded(&self) -> bool {
        true
    }

    fn degraded_reason(&self) -> Option<String> {
        Some(
            "Semantic (vector) search is unavailable; results are keyword-matched only, \
             so a question phrased differently from the stored text may not match."
                .to_string(),
        )
    }

    fn search(&self, conn: &Connection, q: &RetrievalQuery) -> Result<Vec<Candidate>> {
        // Scope is mandatory and empty means NOTHING, never everything.
        if q.scope_keys.is_empty() {
            return Ok(Vec::new());
        }

        let scope_params: Vec<String> = q.scope_keys.clone();
        let scope_placeholders = (0..scope_params.len())
            .map(|i| format!("?{}", i + 2))
            .collect::<Vec<_>>()
            .join(",");

        let tier_clause = if q.tiers.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = q
                .tiers
                .iter()
                .map(|t| format!("'{}'", tier_name(*t)))
                .collect();
            format!(" AND m.tier IN ({})", names.join(","))
        };

        // Two shapes, deliberately distinct.
        //
        // With a searchable query: FTS5 MATCH ranked by BM25.
        // Without one: recency within scope — NOT a match-everything scan.
        // `P3-43`'s defect was exactly the second case silently behaving like
        // the first.
        let (sql, use_match) = if q.is_lexically_searchable() {
            (
                format!(
                    "SELECT m.id, m.tier, m.content, m.scope, m.scope_ref, m.source,
                            m.importance, m.confidence, m.access_count, m.created_at,
                            bm25(memories_fts) AS score
                     FROM memories_fts
                     JOIN memories m ON m.rowid = memories_fts.rowid
                     WHERE memories_fts MATCH ?1
                       AND m.scope_key IN ({scope_placeholders}){tier_clause}
                     ORDER BY score
                     LIMIT ?{}",
                    scope_params.len() + 2
                ),
                true,
            )
        } else {
            (
                format!(
                    "SELECT m.id, m.tier, m.content, m.scope, m.scope_ref, m.source,
                            m.importance, m.confidence, m.access_count, m.created_at,
                            0.0 AS score
                     FROM memories m
                     WHERE m.scope_key IN ({scope_placeholders}){tier_clause}
                     ORDER BY m.created_at DESC
                     LIMIT ?{}",
                    scope_params.len() + 2
                ),
                false,
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if use_match {
            params.push(Box::new(Self::to_fts_query(&q.text)));
        } else {
            // Placeholder ?1 is unused in the recency shape; bind a constant so
            // the positional numbering below stays identical between shapes.
            params.push(Box::new(String::new()));
        }
        for s in &scope_params {
            params.push(Box::new(s.clone()));
        }
        params.push(Box::new(q.limit as i64));

        let refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |r| {
            Ok(Candidate {
                id: r.get(0)?,
                tier: tier_from_name(&r.get::<_, String>(1)?),
                content: r.get(2)?,
                scope: r.get(3)?,
                scope_ref: r.get(4)?,
                source: r.get(5)?,
                importance: r.get::<_, f64>(6)? as f32,
                confidence: r.get::<_, f64>(7)? as f32,
                access_count: r.get(8)?,
                created_at: r.get(9)?,
                // bm25() is negative-is-better; negate so every consumer can
                // treat "higher is more relevant" uniformly.
                lexical_score: -(r.get::<_, f64>(10)? as f32),
                vector_distance: None,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

pub(crate) fn tier_name(t: MemoryTier) -> &'static str {
    match t {
        MemoryTier::Episodic => "episodic",
        MemoryTier::Semantic => "semantic",
        MemoryTier::Procedural => "procedural",
    }
}

pub(crate) fn tier_from_name(s: &str) -> MemoryTier {
    match s {
        "semantic" => MemoryTier::Semantic,
        "procedural" => MemoryTier::Procedural,
        _ => MemoryTier::Episodic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        crate::memory::vector::register_once();
        let c = Connection::open_in_memory().unwrap();
        crate::db::migrate::run_ladder(&c, "memory.db(test)", crate::memory::db::MemoryDb::LADDER)
            .unwrap();
        c
    }

    fn insert(c: &Connection, id: &str, tier: &str, scope_key: &str, content: &str) {
        c.execute(
            "INSERT INTO memories(id,tier,scope,scope_ref,scope_key,content,created_at,updated_at)
             VALUES (?1,?2,'project','x',?3,?4,?5,?5)",
            rusqlite::params![id, tier, scope_key, content, "2026-09-02T00:00:00Z"],
        )
        .unwrap();
    }

    fn q(text: &str, scopes: &[&str]) -> RetrievalQuery {
        RetrievalQuery {
            text: text.to_string(),
            scope_keys: scopes.iter().map(|s| s.to_string()).collect(),
            tiers: Vec::new(),
            limit: 10,
        }
    }

    #[test]
    fn different_queries_return_different_results() {
        // 🔴 The defect this whole task exists for. Recall was byte-identical
        // every turn because the query never reached any tier. `TG3-12` asks
        // exactly this, and it failed by construction before today.
        let c = db();
        insert(
            &c,
            "m1",
            "semantic",
            "global",
            "the deploy key rotates monthly",
        );
        insert(&c, "m2", "semantic", "global", "the frontend uses Tailwind");

        let a = LexicalBackend
            .search(&c, &q("deploy key", &["global"]))
            .unwrap();
        let b = LexicalBackend
            .search(&c, &q("Tailwind", &["global"]))
            .unwrap();

        assert_eq!(a.len(), 1, "got {a:?}");
        assert_eq!(b.len(), 1, "got {b:?}");
        assert_ne!(
            a[0].id, b[0].id,
            "two different questions returned the same memory — this is the \
             query-independence defect"
        );
    }

    #[test]
    fn the_same_query_twice_returns_the_same_set() {
        // The other half of TG3-12: different queries differ, identical queries
        // do not.
        let c = db();
        insert(
            &c,
            "m1",
            "semantic",
            "global",
            "the deploy key rotates monthly",
        );
        let a = LexicalBackend
            .search(&c, &q("deploy", &["global"]))
            .unwrap();
        let b = LexicalBackend
            .search(&c, &q("deploy", &["global"]))
            .unwrap();
        assert_eq!(
            a.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            b.iter().map(|c| c.id.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn scope_is_a_pre_filter_and_an_empty_scope_returns_nothing() {
        let c = db();
        insert(&c, "mine", "semantic", "project:/a", "shared secret alpha");
        insert(&c, "theirs", "semantic", "project:/b", "shared secret beta");

        let a = LexicalBackend
            .search(&c, &q("shared secret", &["project:/a"]))
            .unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].id, "mine", "another project's memory leaked in");

        // ⚠ No scope must mean NOTHING, never everything. "Unscoped" is how
        // cross-project and cross-agent contamination happens.
        let none = LexicalBackend.search(&c, &q("shared secret", &[])).unwrap();
        assert!(none.is_empty(), "an empty scope list retrieved {none:?}");
    }

    #[test]
    fn an_empty_query_does_not_match_everything() {
        // 🔴 `LIKE '%%'` matched every row, which is how procedural "recall"
        // became "the most recently edited topics". An empty query now takes a
        // *different, declared* path — recency within scope — rather than
        // silently behaving like a match-all search.
        let c = db();
        for i in 0..5 {
            insert(
                &c,
                &format!("m{i}"),
                "semantic",
                "global",
                &format!("fact {i}"),
            );
        }
        let query = q("", &["global"]);
        assert!(!query.is_lexically_searchable());

        let r = LexicalBackend.search(&c, &query).unwrap();
        assert_eq!(r.len(), 5, "recency shape should still return rows");
        // and it is ordered by recency, not by a meaningless relevance score
        assert!(r.iter().all(|c| c.lexical_score == 0.0));
    }

    #[test]
    fn fts_syntax_in_the_user_query_cannot_break_the_search() {
        // The query is model- and user-supplied. Raw FTS5 syntax must not reach
        // MATCH: `AND`, `NEAR`, `*`, `"` and `:` are all operators there.
        let c = db();
        insert(&c, "m1", "semantic", "global", "the deploy key rotates");

        for hostile in [
            "deploy AND key",
            "deploy NEAR key",
            "\"unterminated",
            "col:deploy",
            "deploy*",
            "*",
            "((((",
        ] {
            let r = LexicalBackend.search(&c, &q(hostile, &["global"]));
            assert!(
                r.is_ok(),
                "FTS5 syntax {hostile:?} reached MATCH and errored: {:?}",
                r.err()
            );
        }
    }

    #[test]
    fn degradation_is_reported_rather_than_inferred() {
        // TG3-17: the caller must be able to SEE that retrieval is degraded,
        // not deduce it from an empty list.
        assert!(LexicalBackend.is_degraded());
        let why = LexicalBackend.degraded_reason().expect("a reason");
        assert!(
            why.to_lowercase().contains("keyword"),
            "the reason must say what is actually limited: {why}"
        );
    }

    #[test]
    fn bm25_is_returned_so_that_higher_means_more_relevant() {
        let c = db();
        insert(
            &c,
            "strong",
            "semantic",
            "global",
            "deploy deploy deploy key",
        );
        insert(
            &c,
            "weak",
            "semantic",
            "global",
            "deploy something unrelated entirely",
        );
        let r = LexicalBackend
            .search(&c, &q("deploy", &["global"]))
            .unwrap();
        assert!(r.len() >= 2);
        assert!(
            r[0].lexical_score >= r[1].lexical_score,
            "results are not ordered by descending relevance: {:?}",
            r.iter()
                .map(|c| (c.id.clone(), c.lexical_score))
                .collect::<Vec<_>>()
        );
    }
}
