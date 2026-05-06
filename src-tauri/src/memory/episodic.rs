// Tier 1: Episodic Memory — raw conversation storage.
// Phase 4.1 — stores every user message and agent response with 1-indexed
// turn tracking per session. Session summaries are stored as special rows
// (summary_type = 'session_summary') and used for cross-session recall.

use chrono::Utc;
use rusqlite::params;
use tracing::debug;
use uuid::Uuid;

use crate::errors::Result;
use super::db::MemoryDb;
use super::types::{EpisodicMemory, EpisodicRole};

pub struct EpisodicStore;

impl EpisodicStore {
    // ─── Write operations ────────────────────────────────────────────────────

    /// Store a user message. Increments the session turn counter.
    pub fn store_user_message(
        session_id:  &str,
        content:     &str,
        working_dir: Option<&str>,
        metadata:    Option<serde_json::Value>,
    ) -> Result<String> {
        let db  = MemoryDb::open()?;
        let id  = Uuid::new_v4().to_string();
        let turn = next_turn_index(&db, session_id)?;
        let now  = Utc::now().to_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, working_dir, metadata, created_at)
             VALUES (?1, ?2, ?3, 'user', ?4, ?5, ?6, ?7)",
            params![
                id, session_id, turn, content, working_dir,
                metadata.as_ref().map(|v| v.to_string()),
                now,
            ],
        )?;
        debug!("[EpisodicStore] stored user message session={session_id} turn={turn}");
        Ok(id)
    }

    /// Store an agent response. Uses the current (last) turn index because
    /// the response belongs to the same turn as the immediately preceding
    /// user message.
    pub fn store_agent_response(
        session_id:  &str,
        content:     &str,
        model:       Option<&str>,
        cost_usd:    f64,
        tokens_in:   u32,
        tokens_out:  u32,
        thinking:    Option<&str>,
        tool_calls:  Option<serde_json::Value>,
        working_dir: Option<&str>,
        metadata:    Option<serde_json::Value>,
    ) -> Result<String> {
        let db   = MemoryDb::open()?;
        let id   = Uuid::new_v4().to_string();
        let turn = current_turn_index(&db, session_id);
        let now  = Utc::now().to_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, model, cost_usd,
              tokens_in, tokens_out, thinking, tool_calls, working_dir, metadata, created_at)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id, session_id, turn, content, model, cost_usd,
                tokens_in, tokens_out, thinking,
                tool_calls.as_ref().map(|v| v.to_string()),
                working_dir,
                metadata.as_ref().map(|v| v.to_string()),
                now,
            ],
        )?;
        debug!("[EpisodicStore] stored agent response session={session_id} turn={turn}");
        Ok(id)
    }

    /// Store an auto-generated session summary (summary_type = 'session_summary').
    /// These are used for cross-session recall so the PA knows what happened in
    /// past sessions without replaying every individual message.
    pub fn store_session_summary(
        session_id:   &str,
        summary_text: &str,
        working_dir:  Option<&str>,
    ) -> Result<String> {
        let db   = MemoryDb::open()?;
        let id   = Uuid::new_v4().to_string();
        let turn = current_turn_index(&db, session_id);
        let now  = Utc::now().to_rfc3339();

        db.conn.execute(
            "INSERT INTO episodic_memories
             (id, session_id, turn_index, role, content, working_dir, summary_type, created_at)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5, 'session_summary', ?6)",
            params![id, session_id, turn, summary_text, working_dir, now],
        )?;
        debug!("[EpisodicStore] stored session summary session={session_id}");
        Ok(id)
    }

    // ─── Read operations ─────────────────────────────────────────────────────

    /// Full ordered timeline for one session.
    pub fn get_session_timeline(session_id: &str) -> Result<Vec<EpisodicMemory>> {
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(
            "SELECT id, session_id, turn_index, role, content, model, cost_usd,
                    tokens_in, tokens_out, thinking, tool_calls, working_dir,
                    summary_type, metadata, created_at
             FROM   episodic_memories
             WHERE  session_id = ?1
             ORDER  BY turn_index ASC, created_at ASC",
        )?;
        map_rows(&mut stmt, params![session_id])
    }

    /// Recent session summaries across ALL sessions except the current one.
    /// Used for cross-session context injection into the system prompt.
    pub fn get_recent_summaries(exclude_session: &str, limit: u32) -> Result<Vec<EpisodicMemory>> {
        let db = MemoryDb::open()?;
        let mut stmt = db.conn.prepare(
            "SELECT id, session_id, turn_index, role, content, model, cost_usd,
                    tokens_in, tokens_out, thinking, tool_calls, working_dir,
                    summary_type, metadata, created_at
             FROM   episodic_memories
             WHERE  session_id  != ?1
               AND  summary_type = 'session_summary'
             ORDER  BY created_at DESC
             LIMIT  ?2",
        )?;
        map_rows(&mut stmt, params![exclude_session, limit])
    }

    /// Total turn count for a session — used by auto-summarization scheduler.
    pub fn get_turn_count(session_id: &str) -> u32 {
        MemoryDb::open()
            .ok()
            .map(|db| current_turn_index(&db, session_id))
            .unwrap_or(0)
    }

    /// Number of episodic rows in the database.
    pub fn count_total() -> u64 {
        MemoryDb::open()
            .ok()
            .and_then(|db| {
                db.conn.query_row(
                    "SELECT COUNT(*) FROM episodic_memories",
                    [],
                    |r| r.get::<_, i64>(0),
                ).ok()
            })
            .unwrap_or(0) as u64
    }

    /// Count of distinct sessions.
    pub fn count_sessions() -> u64 {
        MemoryDb::open()
            .ok()
            .and_then(|db| {
                db.conn.query_row(
                    "SELECT COUNT(DISTINCT session_id) FROM episodic_memories",
                    [],
                    |r| r.get::<_, i64>(0),
                ).ok()
            })
            .unwrap_or(0) as u64
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn next_turn_index(db: &MemoryDb, session_id: &str) -> Result<u32> {
    Ok(current_turn_index(db, session_id) + 1)
}

fn current_turn_index(db: &MemoryDb, session_id: &str) -> u32 {
    db.conn.query_row(
        "SELECT COALESCE(MAX(turn_index), 0) FROM episodic_memories WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, u32>(0),
    ).unwrap_or(0)
}

fn map_rows(
    stmt:   &mut rusqlite::Statement,
    params: impl rusqlite::Params,
) -> Result<Vec<EpisodicMemory>> {
    let rows = stmt.query_map(params, |row| {
        let role_str: String = row.get(3)?;
        let role = match role_str.as_str() {
            "agent" => EpisodicRole::Agent,
            _       => EpisodicRole::User,
        };
        Ok(EpisodicMemory {
            id:           row.get(0)?,
            session_id:   row.get(1)?,
            turn_index:   row.get(2)?,
            role,
            content:      row.get(4)?,
            model:        row.get(5)?,
            cost_usd:     row.get(6)?,
            tokens_in:    row.get(7)?,
            tokens_out:   row.get(8)?,
            thinking:     row.get(9)?,
            tool_calls:   row.get::<_, Option<String>>(10)?
                             .and_then(|s| serde_json::from_str(&s).ok()),
            working_dir:  row.get(11)?,
            summary_type: row.get(12)?,
            metadata:     row.get::<_, Option<String>>(13)?
                             .and_then(|s| serde_json::from_str(&s).ok()),
            created_at:   row.get::<_, String>(14)?
                             .parse()
                             .unwrap_or_default(),
        })
    })?
    .filter_map(|r| r.ok())
    .collect();
    Ok(rows)
}
