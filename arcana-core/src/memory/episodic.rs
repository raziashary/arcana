// Tier 1: Episodic Memory — raw conversation storage.
// Phase 4.1 — stores every user message and agent response with 1-indexed
// turn tracking per session. Session summaries are stored as special rows
// (summary_type = 'session_summary') and used for cross-session recall.

use rusqlite::params;
use tracing::debug;
use uuid::Uuid;

use super::db::MemoryDb;
use super::types::{EpisodicMemory, EpisodicRole};
use crate::errors::Result;

pub struct EpisodicStore;

impl EpisodicStore {
    // ─── Write operations ────────────────────────────────────────────────────

    /// Store a user message. Increments the session turn counter.
    pub fn store_user_message(
        session_id: &str,
        content: &str,
        working_dir: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_write")
                .with_database("memory")
                .with_session_id(session_id);
        // Redaction boundary (M-40, P1-21). Memory is persistent, so a secret
        // written here is a secret kept forever — `G4:486` recorded exactly
        // that happening with tool arguments. Redacted on the way in rather
        // than on the way out, because a store that never held the secret
        // cannot leak it through a path nobody thought of.
        let content = &crate::security::redaction::redact_text(content);
        let metadata = metadata.map(|m| crate::security::redaction::redact_json(&m));
        let metadata = metadata.as_ref();

        let db = MemoryDb::open()?;
        let id = Uuid::new_v4().to_string();
        let turn = next_turn_index(&db, session_id)?;
        let now = crate::db::timestamps::now_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, working_dir, metadata, created_at)
             VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6, ?7)",
            params![
                id,
                session_id,
                turn,
                content,
                working_dir,
                metadata.as_ref().map(|v| v.to_string()),
                now,
            ],
        )?;
        debug!("[EpisodicStore] stored user message session={session_id} turn={turn}");
        timer.succeed();
        Ok(id)
    }

    /// Store an agent response. Uses the current (last) turn index because
    /// the response belongs to the same turn as the immediately preceding
    /// user message.
    pub fn store_agent_response(
        session_id: &str,
        content: &str,
        model: Option<&str>,
        cost_usd: f64,
        tokens_in: u32,
        tokens_out: u32,
        thinking: Option<&str>,
        tool_calls: Option<serde_json::Value>,
        working_dir: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_write")
                .with_database("memory")
                .with_session_id(session_id);
        // Redaction boundary (M-40, P1-21). `tool_calls` is the field
        // `G4:486` recorded landing in memory verbatim; `thinking` is model
        // reasoning that can quote a secret it just read.
        let content = &crate::security::redaction::redact_text(content);
        // `thinking` gets the stricter rule, including the long-token
        // heuristic. It is model reasoning, not source code — losing a hash
        // from it costs far less than keeping a key in it, and unlike
        // `content` it is never the code the agent hands back to the user.
        let thinking = thinking.map(crate::security::redaction::redact_value_text);
        let thinking = thinking.as_deref();
        let tool_calls = tool_calls.map(|v| crate::security::redaction::redact_json(&v));
        let metadata = metadata.map(|m| crate::security::redaction::redact_json(&m));

        let db = MemoryDb::open()?;
        let id = Uuid::new_v4().to_string();
        let turn = current_turn_index(&db, session_id);
        let now = crate::db::timestamps::now_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, model, cost_usd,
              tokens_in, tokens_out, thinking, tool_calls, working_dir, metadata, created_at)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                session_id,
                turn,
                content,
                model,
                cost_usd,
                tokens_in,
                tokens_out,
                thinking,
                tool_calls.as_ref().map(|v| v.to_string()),
                working_dir,
                metadata.as_ref().map(|v| v.to_string()),
                now,
            ],
        )?;
        debug!("[EpisodicStore] stored agent response session={session_id} turn={turn}");
        timer.succeed();
        Ok(id)
    }

    /// Store an auto-generated session summary (summary_type = 'session_summary').
    /// These are used for cross-session recall so the PA knows what happened in
    /// past sessions without replaying every individual message.
    pub fn store_session_summary(
        session_id: &str,
        summary_text: &str,
        working_dir: Option<&str>,
    ) -> Result<String> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_summary")
                .with_database("memory")
                .with_session_id(session_id);
        // Redaction boundary (M-40, P1-21). Memory is persistent, so a secret
        // written here is kept forever.
        let summary_text = &crate::security::redaction::redact_text(summary_text);

        let db = MemoryDb::open()?;
        let id = Uuid::new_v4().to_string();
        let turn = current_turn_index(&db, session_id);
        let now = crate::db::timestamps::now_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, working_dir, summary_type, created_at)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5, 'session_summary', ?6)",
            params![id, session_id, turn, summary_text, working_dir, now],
        )?;
        debug!("[EpisodicStore] stored session summary session={session_id}");
        timer.succeed();
        Ok(id)
    }

    // ─── Read operations ─────────────────────────────────────────────────────

    /// Full ordered timeline for one session.
    pub fn get_session_timeline(session_id: &str) -> Result<Vec<EpisodicMemory>> {
        let timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_timeline_read")
                .with_database("memory")
                .with_session_id(session_id);
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(
            "SELECT id, session_id, turn_index, role, content, model, cost_usd,
                    tokens_in, tokens_out, thinking, tool_calls, working_dir,
                    summary_type, metadata, created_at
             FROM   episodic_memories
             WHERE  session_id = ?1
             ORDER  BY turn_index ASC, julianday(created_at) ASC, id ASC",
        )?;
        let rows = map_rows(&mut stmt, params![session_id])?;
        timer.with_result_count(rows.len()).succeed();
        Ok(rows)
    }

    /// Recent session summaries across ALL sessions except the current one.
    /// Used for cross-session context injection into the system prompt.
    pub fn get_recent_summaries(exclude_session: &str, limit: u32) -> Result<Vec<EpisodicMemory>> {
        let timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_summaries_read")
                .with_database("memory")
                .with_session_id(exclude_session);
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(
            "SELECT id, session_id, turn_index, role, content, model, cost_usd,
                    tokens_in, tokens_out, thinking, tool_calls, working_dir,
                    summary_type, metadata, created_at
             FROM   episodic_memories
             WHERE  session_id  != ?1
               AND  summary_type = 'session_summary'
             ORDER  BY julianday(created_at) DESC, id DESC
             LIMIT  ?2",
        )?;
        let rows = map_rows(&mut stmt, params![exclude_session, limit])?;
        timer.with_result_count(rows.len()).succeed();
        Ok(rows)
    }

    /// Total turn count for a session — used by auto-summarization scheduler.
    pub fn get_turn_count(session_id: &str) -> u32 {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_turn_count")
                .with_database("memory")
                .with_session_id(session_id);
        let result = MemoryDb::open()
            .ok()
            .map(|db| current_turn_index(&db, session_id));
        if let Some(count) = result {
            timer.with_result_count(count as usize).succeed();
            count
        } else {
            timer.set_outcome("fallback");
            0
        }
    }

    /// Number of episodic rows in the database.
    pub fn count_total() -> u64 {
        Self::count_total_with_health().0
    }

    pub(crate) fn count_total_with_health() -> (u64, bool) {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_count_total")
                .with_database("memory");
        let result = MemoryDb::open().ok().and_then(|db| {
            db.conn
                .query_row("SELECT COUNT(*) FROM episodic_memories", [], |r| {
                    r.get::<_, i64>(0)
                })
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

    /// Count of distinct sessions.
    pub fn count_sessions() -> u64 {
        Self::count_sessions_with_health().0
    }

    pub(crate) fn count_sessions_with_health() -> (u64, bool) {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_episodic_count_sessions")
                .with_database("memory");
        let result = MemoryDb::open().ok().and_then(|db| {
            db.conn
                .query_row(
                    "SELECT COUNT(DISTINCT session_id) FROM episodic_memories",
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
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn next_turn_index(db: &MemoryDb, session_id: &str) -> Result<u32> {
    Ok(current_turn_index(db, session_id) + 1)
}

fn current_turn_index(db: &MemoryDb, session_id: &str) -> u32 {
    db.conn
        .query_row(
            "SELECT COALESCE(MAX(turn_index), 0) FROM episodic_memories WHERE session_id = ?1",
            params![session_id],
            |row| row.get::<_, u32>(0),
        )
        .unwrap_or(0)
}

fn map_rows(
    stmt: &mut rusqlite::Statement,
    params: impl rusqlite::Params,
) -> Result<Vec<EpisodicMemory>> {
    let rows = stmt
        .query_map(params, |row| {
            let role_str: String = row.get(3)?;
            let role = match role_str.as_str() {
                "agent" => EpisodicRole::Agent,
                _ => EpisodicRole::User,
            };
            Ok(EpisodicMemory {
                id: row.get(0)?,
                session_id: row.get(1)?,
                turn_index: row.get(2)?,
                role,
                content: row.get(4)?,
                model: row.get(5)?,
                cost_usd: row.get(6)?,
                tokens_in: row.get(7)?,
                tokens_out: row.get(8)?,
                thinking: row.get(9)?,
                tool_calls: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                working_dir: row.get(11)?,
                summary_type: row.get(12)?,
                metadata: row
                    .get::<_, Option<String>>(13)?
                    .and_then(|s| serde_json::from_str(&s).ok()),
                created_at: crate::db::timestamps::parse_row_timestamp(row, 14)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[cfg(test)]
mod timestamp_order_tests {
    use super::*;
    use chrono::{DateTime, Utc};

    #[test]
    fn production_timeline_and_summary_readers_order_mixed_timestamp_formats_by_instant() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());

        let user_id = EpisodicStore::store_user_message("session", "user", None, None)
            .expect("write user message through production method");
        let agent_id = EpisodicStore::store_agent_response(
            "session", "agent", None, 0.0, 0, 0, None, None, None, None,
        )
        .expect("write agent response through production method");

        let db = MemoryDb::open().expect("open production Memory database");
        db.conn
            .execute(
                "UPDATE episodic_memories SET created_at = CASE id
                    WHEN ?1 THEN '2026-10-04 10:00:00'
                    WHEN ?2 THEN '2026-10-04T09:00:00Z'
                 END WHERE id IN (?1, ?2)",
                params![user_id, agent_id],
            )
            .expect("seed mixed legacy and canonical timestamps");
        db.conn
            .execute(
                "INSERT INTO episodic_memories
                    (id, session_id, turn_index, role, content, summary_type, created_at)
                 VALUES
                    ('summary-legacy', 'older-session', 1, 'agent', 'older', 'session_summary', '2026-10-04 10:00:00'),
                    ('summary-canonical', 'newer-session', 1, 'agent', 'newer', 'session_summary', '2026-10-04T11:00:00Z')",
                [],
            )
            .expect("seed summary timestamp variants");
        drop(db);

        let timeline = EpisodicStore::get_session_timeline("session")
            .expect("read timeline through production method");
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].id, agent_id);
        assert_eq!(timeline[0].created_at, parse("2026-10-04T09:00:00Z"));
        assert_eq!(timeline[1].id, user_id);
        assert_eq!(timeline[1].created_at, parse("2026-10-04T10:00:00Z"));

        let summaries = EpisodicStore::get_recent_summaries("session", 2)
            .expect("read summaries through production method");
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "summary-canonical");
        assert_eq!(summaries[0].created_at, parse("2026-10-04T11:00:00Z"));
        assert_eq!(summaries[1].id, "summary-legacy");
        assert_eq!(summaries[1].created_at, parse("2026-10-04T10:00:00Z"));
    }

    fn parse(value: &str) -> DateTime<Utc> {
        crate::db::timestamps::parse_db_timestamp(value).expect("valid timestamp fixture")
    }
}
