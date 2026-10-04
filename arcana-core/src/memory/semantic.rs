// Tier 2: Semantic Memory — extracted facts and knowledge.
// Phase 4.2 — stores user preferences, architectural decisions, coding patterns,
// and reference locations. Supports contradiction detection via fact_history.
//
// Confidence thresholds (per spec):
//   ≥ 0.7  → auto-stored from extraction
//   = 1.0  → user-explicit ("remember this")
//
// Fact extraction uses two paths:
//   1. Rule-based: regex patterns for common preference statements
//   2. Explicit:   `propose_memory` proposes; the gateway decides (P3-39).
//                  It was `remember_this`, which wrote DIRECTLY at
//                  confidence 1.0 labelled `user_explicit` — so any agent
//                  could put a sentence in the user's mouth.

use rusqlite::params;

const RECALL_FACTS_SQL: &str =
    "SELECT id, fact_text, category, confidence, source_type, source_session,
        working_dir, tags, is_active, created_at, updated_at
 FROM semantic_memories
 WHERE is_active = 1 AND confidence >= 0.7
 ORDER BY confidence DESC, julianday(updated_at) DESC, id DESC
 LIMIT ?1";
const USER_PREFERENCES_SQL: &str =
    "SELECT id, fact_text, category, confidence, source_type, source_session,
        working_dir, tags, is_active, created_at, updated_at
 FROM semantic_memories
 WHERE is_active = 1 AND category IN ('preference', 'user_profile')
 ORDER BY confidence DESC, julianday(updated_at) DESC, id DESC
 LIMIT 20";
use tracing::{debug, info};
use uuid::Uuid;

use super::db::MemoryDb;
use super::types::{FactCategory, SemanticFact};
use crate::errors::Result;

pub struct SemanticStore;

impl SemanticStore {
    // ─── Write operations ────────────────────────────────────────────────────

    /// Store a new fact. If a very similar fact already exists (same category,
    /// same first 80 chars normalized), update it and record history instead
    /// of creating a duplicate.
    pub fn store_fact(
        fact_text: &str,
        category: FactCategory,
        confidence: f64,
        source_type: &str,
        source_session: Option<&str>,
        working_dir: Option<&str>,
        tags: &[String],
    ) -> Result<String> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_write")
                .with_database("memory")
                .with_session_id(source_session.unwrap_or_default());
        // Redaction boundary (M-40, P1-21). Memory is persistent, so a secret
        // written here is kept forever.
        let fact_text = &crate::security::redaction::redact_text(fact_text);

        let db = MemoryDb::open()?;
        let now = crate::db::timestamps::now_rfc3339();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".into());

        // Look for a near-duplicate (same category, text starts same way).
        let normalized = normalize(fact_text);
        let existing: Option<(String, String)> = db
            .conn
            .query_row(
                "SELECT id, fact_text FROM semantic_memories
             WHERE  category = ?1
               AND  is_active = 1
               AND  LOWER(SUBSTR(fact_text, 1, 80)) = LOWER(SUBSTR(?2, 1, 80))",
                params![category.as_str(), &normalized],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .ok();

        if let Some((existing_id, old_text)) = existing {
            // Update + record history if text changed meaningfully.
            if old_text.trim() != fact_text.trim() {
                let hist_id = Uuid::new_v4().to_string();
                db.conn.execute(
                    "INSERT INTO fact_history (id, fact_id, old_text, new_text, changed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![hist_id, existing_id, old_text, fact_text, now],
                )?;
                db.conn.execute(
                    "UPDATE semantic_memories
                     SET fact_text = ?1, confidence = MAX(confidence, ?2),
                         updated_at = ?3
                     WHERE id = ?4",
                    params![fact_text, confidence, now, existing_id],
                )?;
                info!("[SemanticStore] updated fact id={existing_id}");
            }
            timer.succeed();
            return Ok(existing_id);
        }

        let id = Uuid::new_v4().to_string();
        db.conn.execute(
            "INSERT INTO semantic_memories
             (id, fact_text, category, confidence, source_type, source_session,
              working_dir, tags, is_active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
            params![
                id,
                fact_text,
                category.as_str(),
                confidence,
                source_type,
                source_session,
                working_dir,
                tags_json,
                now,
            ],
        )?;
        debug!(
            "[SemanticStore] stored fact category={} id={id}",
            category.as_str()
        );
        timer.succeed();
        Ok(id)
    }

    /// Soft-delete (deactivate) a fact — never hard-delete so history is preserved.
    pub fn deactivate_fact(id: &str) -> Result<()> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_deactivate")
                .with_database("memory");
        let db = MemoryDb::open()?;
        let now = crate::db::timestamps::now_rfc3339();
        db.conn.execute(
            "UPDATE semantic_memories SET is_active = 0, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
        timer.succeed();
        Ok(())
    }

    /// Update the text of an existing fact (records history automatically).
    pub fn update_fact(id: &str, new_text: &str, new_confidence: Option<f64>) -> Result<()> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_update")
                .with_database("memory");
        let db = MemoryDb::open()?;
        let now = crate::db::timestamps::now_rfc3339();

        let old_text: String = db.conn.query_row(
            "SELECT fact_text FROM semantic_memories WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;

        let hist_id = Uuid::new_v4().to_string();
        db.conn.execute(
            "INSERT INTO fact_history (id, fact_id, old_text, new_text, changed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![hist_id, id, old_text, new_text, now],
        )?;

        if let Some(conf) = new_confidence {
            db.conn.execute(
                "UPDATE semantic_memories SET fact_text=?1, confidence=?2, updated_at=?3 WHERE id=?4",
                params![new_text, conf, now, id],
            )?;
        } else {
            db.conn.execute(
                "UPDATE semantic_memories SET fact_text=?1, updated_at=?2 WHERE id=?3",
                params![new_text, now, id],
            )?;
        }
        timer.succeed();
        Ok(())
    }

    // ─── Read operations ─────────────────────────────────────────────────────

    /// Return active facts ordered by confidence (descending), then recency.
    pub fn recall_facts(limit: u32) -> Result<Vec<SemanticFact>> {
        let timer = crate::observability::timing::OperationTimer::start("memory_semantic_recall")
            .with_database("memory");
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(RECALL_FACTS_SQL)?;
        let facts = map_facts(&mut stmt, params![limit])?;
        timer.with_result_count(facts.len()).succeed();
        Ok(facts)
    }

    /// Return facts in the 'preference' and 'user_profile' categories.
    pub fn get_user_preferences() -> Result<Vec<SemanticFact>> {
        let timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_preferences_read")
                .with_database("memory");
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(USER_PREFERENCES_SQL)?;
        let facts = map_facts(&mut stmt, params![])?;
        timer.with_result_count(facts.len()).succeed();
        Ok(facts)
    }

    /// Simple keyword search across all active facts.
    pub fn search(query: &str, limit: u32) -> Result<Vec<SemanticFact>> {
        let timer = crate::observability::timing::OperationTimer::start("memory_semantic_search")
            .with_database("memory");
        let db = MemoryDb::open()?;
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = db.conn.prepare(
            "SELECT id, fact_text, category, confidence, source_type, source_session,
                    working_dir, tags, is_active, created_at, updated_at
             FROM   semantic_memories
             WHERE  is_active = 1
               AND  LOWER(fact_text) LIKE ?1
             ORDER  BY confidence DESC
             LIMIT  ?2",
        )?;
        let facts = map_facts(&mut stmt, params![pattern, limit])?;
        timer.with_result_count(facts.len()).succeed();
        Ok(facts)
    }

    pub fn count_total() -> u64 {
        Self::count_total_with_health().0
    }

    pub(crate) fn count_total_with_health() -> (u64, bool) {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_count_total")
                .with_database("memory");
        let result = MemoryDb::open().ok().and_then(|db| {
            db.conn
                .query_row(
                    "SELECT COUNT(*) FROM semantic_memories WHERE is_active = 1",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .ok()
        });
        if let Some(count) = result {
            timer.with_result_count(count.max(0) as usize).succeed();
            (count.max(0) as u64, true)
        } else {
            timer.set_outcome("fallback");
            (0, false)
        }
    }

    // ─── Rule-based extraction ────────────────────────────────────────────────

    /// Scan a user message for preference/knowledge patterns and auto-store
    /// any hits with confidence 0.75 (below the explicit 1.0 threshold).
    /// Patterns are conservative to avoid false positives.
    pub fn extract_and_store(content: &str, session_id: &str, working_dir: Option<&str>) {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_semantic_extract")
                .with_database("memory")
                .with_session_id(session_id);
        let extractions = extract_facts(content);
        let attempted = extractions.len();
        let (stored, failed) = store_extracted_facts(extractions, |text, category| {
            Self::store_fact(
                text,
                category,
                0.75,
                "extracted",
                Some(session_id),
                working_dir,
                &[],
            )
            .map(|_| ())
        });
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "memory_semantic_extract_result",
            attempted_count = attempted,
            stored_count = stored,
            failed_count = failed,
            outcome = if failed == 0 { "success" } else { "degraded" },
            "Semantic memory extraction completed"
        );
        timer = timer.with_result_count(stored);
        timer.set_outcome(if failed == 0 { "success" } else { "degraded" });
    }
}

fn store_extracted_facts<F>(
    extractions: Vec<(String, FactCategory)>,
    mut store: F,
) -> (usize, usize)
where
    F: FnMut(&str, FactCategory) -> crate::errors::Result<()>,
{
    let mut stored = 0;
    let mut failed = 0;
    for (text, category) in extractions {
        if store(&text, category).is_ok() {
            stored += 1;
        } else {
            failed += 1;
        }
    }
    (stored, failed)
}

// ─── Rule-based fact extraction ──────────────────────────────────────────────

fn extract_facts(content: &str) -> Vec<(String, FactCategory)> {
    let mut results = Vec::new();

    // Preference signals: "I prefer X", "I like X over Y", "always use X"
    let pref_patterns = [
        "i prefer ",
        "i like ",
        "please always ",
        "always use ",
        "never use ",
        "i want you to ",
        "make sure to always",
        "i use ",
    ];
    for &pat in &pref_patterns {
        if let Some(pos) = content.to_lowercase().find(pat) {
            let excerpt = content[pos..].splitn(2, '\n').next().unwrap_or("").trim();
            if excerpt.len() > 10 && excerpt.len() < 200 {
                results.push((excerpt.to_string(), FactCategory::Preference));
            }
        }
    }

    // Architecture signals: "use X not Y", "X is our pattern"
    let arch_patterns = [
        "use x not",
        "our pattern is",
        "we use ",
        "the project uses ",
    ];
    for &pat in &arch_patterns {
        if let Some(pos) = content.to_lowercase().find(pat) {
            let excerpt = content[pos..].splitn(2, '\n').next().unwrap_or("").trim();
            if excerpt.len() > 10 && excerpt.len() < 200 {
                results.push((excerpt.to_string(), FactCategory::Architecture));
            }
        }
    }

    results
}

#[cfg(test)]
mod observability_tests {
    use super::*;

    #[test]
    fn extraction_reports_successes_and_failures_without_exposing_fact_text() {
        let extractions = vec![
            (
                "private preference one".to_owned(),
                FactCategory::Preference,
            ),
            (
                "private preference two".to_owned(),
                FactCategory::Preference,
            ),
        ];
        let mut calls = 0;
        let counts = store_extracted_facts(extractions, |_text, _category| {
            calls += 1;
            if calls == 1 {
                Ok(())
            } else {
                Err(crate::errors::ArcanaError::Agent(
                    "private storage failure".into(),
                ))
            }
        });
        assert_eq!(counts, (1, 1));
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn map_facts(
    stmt: &mut rusqlite::Statement,
    params: impl rusqlite::Params,
) -> Result<Vec<SemanticFact>> {
    let rows = stmt
        .query_map(params, |row| {
            let tags_str: String = row.get(7).unwrap_or_else(|_| "[]".into());
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(SemanticFact {
                id: row.get(0)?,
                fact_text: row.get(1)?,
                category: row.get(2)?,
                confidence: row.get(3)?,
                source_type: row.get(4)?,
                source_session: row.get(5)?,
                working_dir: row.get(6)?,
                tags,
                is_active: row.get::<_, i32>(8)? != 0,
                created_at: crate::db::timestamps::parse_row_timestamp(row, 9)?,
                updated_at: crate::db::timestamps::parse_row_timestamp(row, 10)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[cfg(test)]
mod timestamp_order_tests {
    use super::*;

    #[test]
    fn production_semantic_readers_order_mixed_timestamps_by_instant() {
        let home = tempfile::tempdir().expect("isolated home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = MemoryDb::open().expect("open migrated MemoryDb");
        db.conn
            .execute_batch(
                "WITH RECURSIVE n(value) AS (
                    SELECT 1 UNION ALL SELECT value + 1 FROM n WHERE value < 5000
                 )
                 INSERT INTO semantic_memories
                    (id,fact_text,category,confidence,source_type,tags,is_active,created_at,updated_at)
                 SELECT 'old-' || value, 'old fact', 'preference', 0.8, 'test', '[]', 1,
                        strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || value || ' days'),
                        strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || value || ' days')
                 FROM n;
                 INSERT INTO semantic_memories
                    (id,fact_text,category,confidence,source_type,tags,is_active,created_at,updated_at)
                 VALUES
                    ('canonical-0900','canonical fact','preference',0.99,'test','[]',1,
                     '2026-09-02T09:00:00Z','2026-09-02T09:00:00Z'),
                    ('legacy-1200','legacy fact','user_profile',0.99,'test','[]',1,
                     '2026-09-02 12:00:00','2026-09-02 12:00:00');
                 ANALYZE;",
            )
            .expect("seed populated mixed-format semantic facts");

        for sql in [RECALL_FACTS_SQL, USER_PREFERENCES_SQL] {
            let plan_sql = format!("EXPLAIN QUERY PLAN {sql}");
            let mut statement = db.conn.prepare(&plan_sql).expect("prepare production plan");
            let details = if sql.contains("?1") {
                statement
                    .query_map([50], |row| row.get::<_, String>(3))
                    .expect("explain production query")
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .expect("collect query plan")
                    .join(" | ")
            } else {
                statement
                    .query_map([], |row| row.get::<_, String>(3))
                    .expect("explain production query")
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .expect("collect query plan")
                    .join(" | ")
            };
            assert!(
                details.contains("idx_sem_active_confidence_updated_julianday"),
                "expected the semantic instant-order index; plan was: {details}"
            );
        }
        drop(db);

        let recalled = SemanticStore::recall_facts(2).expect("read production fact recall");
        assert_eq!(recalled[0].id, "legacy-1200");
        assert_eq!(recalled[1].id, "canonical-0900");

        let preferences =
            SemanticStore::get_user_preferences().expect("read production preferences");
        assert_eq!(preferences[0].id, "legacy-1200");
        assert_eq!(preferences[1].id, "canonical-0900");
    }
}
