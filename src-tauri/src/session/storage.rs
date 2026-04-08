// SQLite-backed storage for sessions and messages.
// Database path: ~/.airs/autodev.db

use rusqlite::{Connection, params};
use std::path::PathBuf;
use crate::errors::{AutoDevError, Result};
use super::types::{Message, MessageRole, Session, ToolCall, ToolResult};

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
                created_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id           TEXT PRIMARY KEY,
                session_id   TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                role         TEXT NOT NULL,
                content      TEXT NOT NULL,
                tool_calls   TEXT NOT NULL DEFAULT '[]',
                tool_results TEXT NOT NULL DEFAULT '[]',
                tokens_in    INTEGER NOT NULL DEFAULT 0,
                tokens_out   INTEGER NOT NULL DEFAULT 0,
                cost_usd     REAL NOT NULL DEFAULT 0.0,
                created_at   TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, created_at);
            ",
        )?;
        Ok(())
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, title, working_dir, provider, model, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               title=excluded.title,
               working_dir=excluded.working_dir,
               provider=excluded.provider,
               model=excluded.model,
               updated_at=excluded.updated_at",
            params![
                session.id,
                session.title,
                session.working_dir,
                session.provider,
                session.model,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_message(&self, msg: &Message) -> Result<()> {
        let tool_calls   = serde_json::to_string(&msg.tool_calls)?;
        let tool_results = serde_json::to_string(&msg.tool_results)?;
        self.conn.execute(
            "INSERT INTO messages
             (id, session_id, role, content, tool_calls, tool_results, tokens_in, tokens_out, cost_usd, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                msg.id,
                msg.session_id,
                format!("{:?}", msg.role).to_lowercase(),
                msg.content,
                tool_calls,
                tool_results,
                msg.tokens_in,
                msg.tokens_out,
                msg.cost_usd,
                msg.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn update_message(&self, msg: &Message) -> Result<()> {
        let tool_calls   = serde_json::to_string(&msg.tool_calls)?;
        let tool_results = serde_json::to_string(&msg.tool_results)?;
        self.conn.execute(
            "UPDATE messages SET
               content=?2, tool_calls=?3, tool_results=?4,
               tokens_in=?5, tokens_out=?6, cost_usd=?7
             WHERE id=?1",
            params![
                msg.id,
                msg.content,
                tool_calls,
                tool_results,
                msg.tokens_in,
                msg.tokens_out,
                msg.cost_usd,
            ],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, working_dir, provider, model, created_at, updated_at
             FROM sessions ORDER BY updated_at DESC LIMIT 50"
        )?;
        let sessions: Vec<Session> = stmt.query_map([], |row| {
            Ok(Session {
                id:          row.get(0)?,
                title:       row.get(1)?,
                working_dir: row.get(2)?,
                provider:    row.get(3)?,
                model:       row.get(4)?,
                created_at:  row.get::<_, String>(5)?.parse().unwrap_or_default(),
                updated_at:  row.get::<_, String>(6)?.parse().unwrap_or_default(),
                messages:    vec![],
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(sessions)
    }

    pub fn get_session_with_messages(&self, session_id: &str) -> Result<Option<Session>> {
        let session = self.conn.query_row(
            "SELECT id, title, working_dir, provider, model, created_at, updated_at
             FROM sessions WHERE id=?1",
            params![session_id],
            |row| Ok(Session {
                id:          row.get(0)?,
                title:       row.get(1)?,
                working_dir: row.get(2)?,
                provider:    row.get(3)?,
                model:       row.get(4)?,
                created_at:  row.get::<_, String>(5)?.parse().unwrap_or_default(),
                updated_at:  row.get::<_, String>(6)?.parse().unwrap_or_default(),
                messages:    vec![],
            }),
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
            "SELECT id, session_id, role, content, tool_calls, tool_results,
                    tokens_in, tokens_out, cost_usd, created_at
             FROM messages WHERE session_id=?1 ORDER BY created_at ASC"
        )?;
        let msgs: Vec<Message> = stmt.query_map(params![session_id], |row| {
            let role_str: String = row.get(2)?;
            let role = match role_str.as_str() {
                "assistant" => MessageRole::Assistant,
                "tool"      => MessageRole::Tool,
                _           => MessageRole::User,
            };
            let tool_calls: Vec<ToolCall> = serde_json::from_str(
                &row.get::<_, String>(4).unwrap_or_default()
            ).unwrap_or_default();
            let tool_results: Vec<ToolResult> = serde_json::from_str(
                &row.get::<_, String>(5).unwrap_or_default()
            ).unwrap_or_default();
            Ok(Message {
                id:           row.get(0)?,
                session_id:   row.get(1)?,
                role,
                content:      row.get(3)?,
                tool_calls,
                tool_results,
                tokens_in:    row.get(6)?,
                tokens_out:   row.get(7)?,
                cost_usd:     row.get(8)?,
                created_at:   row.get::<_, String>(9)?.parse().unwrap_or_default(),
            })
        })?.filter_map(|r| r.ok()).collect();
        Ok(msgs)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM sessions WHERE id=?1", params![session_id])?;
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
