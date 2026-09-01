// SQLite-backed storage for sessions and messages.
// Database path: ~/.airs/autodev.db

use super::types::{Message, MessageRole, Session};
use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;

pub struct SessionStorage {
    conn: Connection,
}

impl SessionStorage {
    pub fn open() -> Result<Self> {
        let db_path = db_path()?;
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        let storage = Self { conn };
        storage.migrate()?;
        Ok(storage)
    }

    fn migrate(&self) -> Result<()> {
        // Initial setup
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS sessions (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL,
                working_dir TEXT NOT NULL,
                provider    TEXT NOT NULL,
                model       TEXT NOT NULL,
                pa_mode     TEXT NOT NULL DEFAULT 'fast_development',
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );
            ",
        )?;

        // Phase 3.0 R3 — migrate existing `sessions` rows to include the new
        // `pa_mode` column. SQLite has no `ADD COLUMN IF NOT EXISTS`, so the
        // error-on-duplicate is swallowed intentionally (same best-effort
        // pattern used below for the `messages` table).
        let _ = self.conn.execute(
            "ALTER TABLE sessions ADD COLUMN pa_mode TEXT NOT NULL DEFAULT 'fast_development'",
            [],
        );

        // Check if messages table exists
        let table_exists: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if !table_exists {
            // Create new table with V2 schema
            self.conn.execute_batch(
                "
                CREATE TABLE messages (
                    id           TEXT PRIMARY KEY,
                    session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                    role         TEXT NOT NULL,
                    content      TEXT NOT NULL,
                    tool_name    TEXT,
                    tool_input   TEXT,
                    tool_call_id TEXT,
                    is_error     INTEGER NOT NULL DEFAULT 0,
                    tokens_in    INTEGER NOT NULL DEFAULT 0,
                    tokens_out   INTEGER NOT NULL DEFAULT 0,
                    cost_usd     REAL NOT NULL DEFAULT 0.0,
                    created_at   TEXT NOT NULL
                );
                CREATE INDEX idx_messages_session ON messages(session_id, created_at);
                CREATE INDEX idx_messages_tool ON messages(tool_call_id);
                ",
            )?;
        } else {
            // Migrate existing table: add new columns if they don't exist
            // SQLite doesn't have "ADD COLUMN IF NOT EXISTS", so we catch errors
            let _ = self
                .conn
                .execute("ALTER TABLE messages ADD COLUMN tool_name TEXT", []);
            let _ = self
                .conn
                .execute("ALTER TABLE messages ADD COLUMN tool_input TEXT", []);
            let _ = self
                .conn
                .execute("ALTER TABLE messages ADD COLUMN tool_call_id TEXT", []);
            let _ = self.conn.execute(
                "ALTER TABLE messages ADD COLUMN is_error INTEGER NOT NULL DEFAULT 0",
                [],
            );

            // Create indexes (these use IF NOT EXISTS)
            let _ = self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at)",
                []
            );
            let _ = self.conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_messages_tool ON messages(tool_call_id)",
                [],
            );
        }

        Ok(())
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, title, working_dir, provider, model, pa_mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               title=excluded.title,
               working_dir=excluded.working_dir,
               provider=excluded.provider,
               model=excluded.model,
               pa_mode=excluded.pa_mode,
               updated_at=excluded.updated_at",
            params![
                session.id,
                session.title,
                session.working_dir,
                session.provider,
                session.model,
                session.pa_mode,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Phase 3.0 R3 — targeted write for the PA mode column. Used by the
    /// `set_session_pa_mode` Tauri command so a mode toggle doesn't have to
    /// round-trip the entire session row.
    pub fn update_session_pa_mode(&self, session_id: &str, pa_mode: &str) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE sessions SET pa_mode = ?1, updated_at = ?2 WHERE id = ?3",
            params![pa_mode, chrono::Utc::now().to_rfc3339(), session_id],
        )?;
        if updated == 0 {
            return Err(AutoDevError::Session(format!(
                "Session '{session_id}' not found — cannot set pa_mode"
            )));
        }
        Ok(())
    }

    pub fn insert_message(&self, msg: &Message) -> Result<()> {
        let tool_input = msg
            .tool_input
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default();

        self.conn.execute(
            "INSERT INTO messages
             (id, session_id, role, content, tool_name, tool_input, tool_call_id, is_error, tokens_in, tokens_out, cost_usd, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                msg.id,
                msg.session_id,
                format!("{:?}", msg.role).to_lowercase(),
                msg.content,
                msg.tool_name.as_ref().map(String::as_str),
                if tool_input.is_empty() { None } else { Some(tool_input.as_str()) },
                msg.tool_call_id.as_ref().map(String::as_str),
                msg.is_error as i32,
                msg.tokens_in,
                msg.tokens_out,
                msg.cost_usd,
                msg.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        let tool_input = msg
            .tool_input
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default();

        self.conn.execute(
            "UPDATE messages SET
               content=?2, tool_name=?3, tool_input=?4, tool_call_id=?5, is_error=?6,
               tokens_in=?7, tokens_out=?8, cost_usd=?9
             WHERE id=?1",
            params![
                msg.id,
                msg.content,
                msg.tool_name.as_ref().map(String::as_str),
                if tool_input.is_empty() {
                    None
                } else {
                    Some(tool_input.as_str())
                },
                msg.tool_call_id.as_ref().map(String::as_str),
                msg.is_error as i32,
                msg.tokens_in,
                msg.tokens_out,
                msg.cost_usd,
            ],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, working_dir, provider, model, pa_mode, created_at, updated_at
             FROM sessions ORDER BY updated_at DESC LIMIT 50",
        )?;
        let sessions: Vec<Session> = stmt
            .query_map([], |row| {
                Ok(Session {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    working_dir: row.get(2)?,
                    provider: row.get(3)?,
                    model: row.get(4)?,
                    pa_mode: row
                        .get::<_, Option<String>>(5)?
                        .unwrap_or_else(|| "fast_development".into()),
                    created_at: row.get::<_, String>(6)?.parse().unwrap_or_default(),
                    updated_at: row.get::<_, String>(7)?.parse().unwrap_or_default(),
                    messages: vec![],
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(sessions)
    }

    pub fn get_session_with_messages(&self, session_id: &str) -> Result<Option<Session>> {
        let session = self.conn.query_row(
            "SELECT id, title, working_dir, provider, model, pa_mode, created_at, updated_at
             FROM sessions WHERE id=?1",
            params![session_id],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    working_dir: row.get(2)?,
                    provider: row.get(3)?,
                    model: row.get(4)?,
                    pa_mode: row
                        .get::<_, Option<String>>(5)?
                        .unwrap_or_else(|| "fast_development".into()),
                    created_at: row.get::<_, String>(6)?.parse().unwrap_or_default(),
                    updated_at: row.get::<_, String>(7)?.parse().unwrap_or_default(),
                    messages: vec![],
                })
            },
        );
        match session {
            Ok(mut s) => {
                s.messages = self.load_messages(session_id)?;
                Ok(Some(s))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AutoDevError::Database(e)),
        }
    }

    fn load_messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, tool_name, tool_input, tool_call_id, is_error,
                    tokens_in, tokens_out, cost_usd, created_at
             FROM messages WHERE session_id=?1 ORDER BY created_at ASC",
        )?;
        let msgs: Vec<Message> = stmt
            .query_map(params![session_id], |row| {
                let role_str: String = row.get(2)?;
                let role = match role_str.as_str() {
                    "assistant" => MessageRole::Assistant,
                    "tool" => MessageRole::Tool,
                    _ => MessageRole::User,
                };
                // Parse tool fields for tool messages
                let tool_name: Option<String> = row.get(4)?;
                let tool_input: Option<serde_json::Value> = row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| serde_json::from_str(&s).ok());
                let tool_call_id: Option<String> = row.get(6)?;
                let is_error: bool = row.get::<_, i32>(7)? != 0;

                Ok(Message {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role,
                    content: row.get(3)?,
                    tool_name,
                    tool_input,
                    tool_call_id,
                    is_error,
                    tokens_in: row.get(8)?,
                    tokens_out: row.get(9)?,
                    cost_usd: row.get(10)?,
                    created_at: row.get::<_, String>(11)?.parse().unwrap_or_default(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(msgs)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM sessions WHERE id=?1", params![session_id])?;
        Ok(())
    }
}

fn db_path() -> Result<PathBuf> {
    let home = dirs_next::home_dir()
        .ok_or_else(|| AutoDevError::Vault("Cannot find home directory".into()))?;
    Ok(home.join(".airs").join("autodev.db"))
}

// Lightweight stub for dirs_next — we can't add a dep mid-build, use std::env
mod dirs_next {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()
            .map(std::path::PathBuf::from)
    }
}
