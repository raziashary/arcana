// Memory SQLite database — ~/.autodev/memory.db
// All 3 tiers live in one database: episodic, semantic, procedural.
// Migration is purely additive via CREATE TABLE IF NOT EXISTS so new columns
// can be appended in later schema revisions without breaking old installs.

use crate::errors::{AutoDevError, Result};
use rusqlite::Connection;
use std::path::PathBuf;

pub struct MemoryDb {
    pub conn: Connection,
}

impl MemoryDb {
    pub fn open() -> Result<Self> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            -- ─────────────────────────────────────────────────────────────
            -- Tier 1: Episodic Memory (raw conversation experience)
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS episodic_memories (
                id           TEXT PRIMARY KEY,
                session_id   TEXT NOT NULL,
                turn_index   INTEGER NOT NULL DEFAULT 1,
                role         TEXT NOT NULL,       -- 'user' | 'agent'
                content      TEXT NOT NULL,
                model        TEXT,
                cost_usd     REAL NOT NULL DEFAULT 0.0,
                tokens_in    INTEGER NOT NULL DEFAULT 0,
                tokens_out   INTEGER NOT NULL DEFAULT 0,
                thinking     TEXT,                -- agent extended thinking
                tool_calls   TEXT,                -- JSON array of tool call objects
                working_dir  TEXT,
                summary_type TEXT,               -- NULL | 'session_summary'
                metadata     TEXT,               -- JSON blob
                created_at   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_epi_session
                ON episodic_memories(session_id, turn_index);
            CREATE INDEX IF NOT EXISTS idx_epi_created
                ON episodic_memories(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_epi_summary
                ON episodic_memories(summary_type)
                WHERE summary_type IS NOT NULL;

            -- ─────────────────────────────────────────────────────────────
            -- Tier 2: Semantic Memory (extracted facts + knowledge)
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS semantic_memories (
                id             TEXT PRIMARY KEY,
                fact_text      TEXT NOT NULL,
                category       TEXT NOT NULL DEFAULT 'preference',
                confidence     REAL NOT NULL DEFAULT 0.7,
                source_type    TEXT NOT NULL DEFAULT 'extracted',
                source_session TEXT,
                working_dir    TEXT,
                tags           TEXT NOT NULL DEFAULT '[]',
                is_active      INTEGER NOT NULL DEFAULT 1,
                created_at     TEXT NOT NULL,
                updated_at     TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sem_category
                ON semantic_memories(category, is_active);
            CREATE INDEX IF NOT EXISTS idx_sem_confidence
                ON semantic_memories(confidence DESC)
                WHERE is_active = 1;

            -- Fact history — contradiction tracking (Phase 4.2)
            CREATE TABLE IF NOT EXISTS fact_history (
                id         TEXT PRIMARY KEY,
                fact_id    TEXT NOT NULL REFERENCES semantic_memories(id),
                old_text   TEXT NOT NULL,
                new_text   TEXT NOT NULL,
                changed_at TEXT NOT NULL
            );

            -- ─────────────────────────────────────────────────────────────
            -- Tier 3: Procedural Memory (skills + patterns — SQLite mirror)
            -- Primary storage is the MEMORY.md file tree; this table enables
            -- fast full-text search and tag-based filtering.
            -- ─────────────────────────────────────────────────────────────
            CREATE TABLE IF NOT EXISTS procedural_memories (
                id         TEXT PRIMARY KEY,
                topic      TEXT NOT NULL UNIQUE,  -- e.g. 'debugging', 'workflow-patterns'
                title      TEXT NOT NULL,
                content    TEXT NOT NULL,          -- full markdown body
                tags       TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_proc_topic
                ON procedural_memories(topic);
            CREATE INDEX IF NOT EXISTS idx_proc_updated
                ON procedural_memories(updated_at DESC);
            ",
        )?;
        Ok(())
    }
}

pub fn db_path() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| AutoDevError::Agent("Cannot determine home directory for memory.db".into()))?;
    Ok(home.join(".autodev").join("memory.db"))
}
