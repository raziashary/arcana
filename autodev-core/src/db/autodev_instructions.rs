// AUTODEV.md instruction storage — SQLite-backed, not filesystem-scattered.
// Database path: ~/.autodev/config.db
// Two scopes: 'global' (applies to all sessions) and 'repo' (per working dir).
// See: DEVELOPMENT_PLAN.md Phase 2.1.2 — AUTODEV.md System

use crate::errors::{AutoDevError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MAX_CONTENT_CHARS: usize = 40_000;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutodevInstruction {
    pub id: i64,
    pub scope: String, // "global" | "repo"
    pub repo_path: Option<String>,
    pub content: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct AutodevInstructionsDb {
    conn: Connection,
}

impl AutodevInstructionsDb {
    pub fn open() -> Result<Self> {
        // `config.db`'s schema and version belong to `db::config_db`, which
        // owns the file; this table is one of three it holds.
        Ok(Self {
            conn: crate::db::config_db::open()?,
        })
    }

    /// Fetch content for a given scope/repo. Returns empty string if not found.
    pub fn get(&self, scope: &str, repo_path: Option<&str>) -> Result<String> {
        let result = if let Some(path) = repo_path {
            self.conn.query_row(
                "SELECT content FROM autodev_instructions WHERE scope = ?1 AND repo_path = ?2 AND is_active = 1",
                params![scope, path],
                |r| r.get::<_, String>(0),
            ).optional()?
        } else {
            self.conn.query_row(
                "SELECT content FROM autodev_instructions WHERE scope = ?1 AND repo_path IS NULL AND is_active = 1",
                params![scope],
                |r| r.get::<_, String>(0),
            ).optional()?
        };
        Ok(result.unwrap_or_default())
    }

    /// Insert or update instructions for a scope.
    pub fn set(&self, scope: &str, repo_path: Option<&str>, content: &str) -> Result<()> {
        if content.len() > MAX_CONTENT_CHARS {
            return Err(AutoDevError::Agent(format!(
                "AUTODEV.md content exceeds {} character limit ({} chars)",
                MAX_CONTENT_CHARS,
                content.len()
            )));
        }
        if let Some(path) = repo_path {
            self.conn.execute(
                "INSERT INTO autodev_instructions (scope, repo_path, content)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(scope, repo_path) DO UPDATE SET content = excluded.content, is_active = 1",
                params![scope, path, content],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO autodev_instructions (scope, repo_path, content)
                 VALUES (?1, NULL, ?2)
                 ON CONFLICT(scope, repo_path) DO UPDATE SET content = excluded.content, is_active = 1",
                params![scope, content],
            )?;
        }
        Ok(())
    }

    pub fn delete_repo(&self, repo_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE autodev_instructions SET is_active = 0 WHERE scope = 'repo' AND repo_path = ?1",
            params![repo_path],
        )?;
        Ok(())
    }

    pub fn exists(&self, scope: &str, repo_path: Option<&str>) -> Result<bool> {
        let content = self.get(scope, repo_path)?;
        Ok(!content.is_empty())
    }

    pub fn char_count(&self, scope: &str, repo_path: Option<&str>) -> Result<usize> {
        Ok(self.get(scope, repo_path)?.len())
    }
}
