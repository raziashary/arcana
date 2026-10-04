// ARCANA.md instruction storage — SQLite-backed, not filesystem-scattered.
// Database path: ~/.arcana/config.db
// Two scopes: 'global' (applies to all sessions) and 'repo' (per working dir).
// See: DEVELOPMENT_PLAN.md Phase 2.1.2 — ARCANA.md System

use crate::errors::{ArcanaError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const MAX_CONTENT_CHARS: usize = 40_000;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArcanaInstruction {
    pub id: i64,
    pub scope: String, // "global" | "repo"
    pub repo_path: Option<String>,
    pub content: String,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

// ─── Repository ───────────────────────────────────────────────────────────────

pub struct ArcanaInstructionsDb {
    conn: Connection,
}

impl ArcanaInstructionsDb {
    pub fn open() -> Result<Self> {
        // `config.db`'s schema and version belong to `db::config_db`, which
        // owns the file; this table is one of three it holds.
        Ok(Self {
            conn: crate::db::config_db::open()?,
        })
    }

    /// Fetch content for a given scope/repo. Returns empty string if not found.
    pub fn get(&self, scope: &str, repo_path: Option<&str>) -> Result<String> {
        let mut timing =
            crate::observability::timing::OperationTimer::start("config_instruction_read")
                .with_database("config");
        let result = if let Some(path) = repo_path {
            self.conn.query_row(
                "SELECT content FROM arcana_instructions WHERE scope = ?1 AND repo_path = ?2 AND is_active = 1",
                params![scope, path],
                |r| r.get::<_, String>(0),
            ).optional()?
        } else {
            self.conn.query_row(
                "SELECT content FROM arcana_instructions WHERE scope = ?1 AND repo_path IS NULL AND is_active = 1",
                params![scope],
                |r| r.get::<_, String>(0),
            ).optional()?
        };
        timing = timing.with_result_count(usize::from(result.is_some()));
        timing.succeed();
        Ok(result.unwrap_or_default())
    }

    /// Insert or update instructions for a scope.
    pub fn set(&self, scope: &str, repo_path: Option<&str>, content: &str) -> Result<()> {
        let mut timing =
            crate::observability::timing::OperationTimer::start("config_instruction_write")
                .with_database("config");
        if content.len() > MAX_CONTENT_CHARS {
            return Err(ArcanaError::Agent(format!(
                "ARCANA.md content exceeds {} character limit ({} chars)",
                MAX_CONTENT_CHARS,
                content.len()
            )));
        }
        if let Some(path) = repo_path {
            self.conn.execute(
                "INSERT INTO arcana_instructions (scope, repo_path, content)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(scope, repo_path) DO UPDATE SET content = excluded.content, is_active = 1",
                params![scope, path, content],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO arcana_instructions (scope, repo_path, content)
                 VALUES (?1, NULL, ?2)
                 ON CONFLICT(scope, repo_path) DO UPDATE SET content = excluded.content, is_active = 1",
                params![scope, content],
            )?;
        }
        timing = timing.with_result_count(1);
        timing.succeed();
        Ok(())
    }

    pub fn delete_repo(&self, repo_path: &str) -> Result<()> {
        let mut timing =
            crate::observability::timing::OperationTimer::start("config_instruction_delete")
                .with_database("config");
        let affected = self.conn.execute(
            "UPDATE arcana_instructions SET is_active = 0 WHERE scope = 'repo' AND repo_path = ?1",
            params![repo_path],
        )?;
        timing = timing.with_result_count(affected);
        timing.succeed();
        Ok(())
    }

    pub fn exists(&self, scope: &str, repo_path: Option<&str>) -> Result<bool> {
        let mut timing =
            crate::observability::timing::OperationTimer::start("config_instruction_exists")
                .with_database("config");
        let content = self.get(scope, repo_path)?;
        let exists = !content.is_empty();
        timing = timing.with_result_count(usize::from(exists));
        timing.succeed();
        Ok(exists)
    }

    pub fn char_count(&self, scope: &str, repo_path: Option<&str>) -> Result<usize> {
        let mut timing =
            crate::observability::timing::OperationTimer::start("config_instruction_length")
                .with_database("config");
        let length = self.get(scope, repo_path)?.len();
        timing = timing.with_result_count(1);
        timing.succeed();
        Ok(length)
    }
}

#[cfg(test)]
mod observability_tests {
    use super::*;

    #[test]
    fn instruction_reads_and_writes_emit_counts_without_content_or_paths() {
        let directory = tempfile::tempdir().expect("isolated config directory");
        let db = ArcanaInstructionsDb {
            conn: crate::db::config_db::open_at(&directory.path().join("config.db"))
                .expect("open isolated config database"),
        };
        let repository = "C:/private/project";
        let secret_content = "private instructions must not enter telemetry";

        let (_, events) = crate::observability::timing::capture_json_events(|| {
            assert!(db
                .get("repo", Some(repository))
                .expect("missing read")
                .is_empty());
            db.set("repo", Some(repository), secret_content)
                .expect("write instructions");
            assert_eq!(
                db.get("repo", Some(repository)).expect("read instructions"),
                secret_content
            );
            assert!(db.exists("repo", Some(repository)).expect("exists"));
            assert_eq!(
                db.char_count("repo", Some(repository)).expect("length"),
                secret_content.len()
            );
            db.delete_repo(repository).expect("delete instructions");
        });

        let operations = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 8);
        for event in &operations {
            assert_eq!(event["fields"]["database"], "config");
            assert_eq!(event["fields"]["outcome"], "success");
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "config_instruction_read"
                && event["fields"]["result_count"] == 0
        }));
        let serialized = serde_json::to_string(&events).expect("serialize telemetry");
        assert!(!serialized.contains(secret_content));
        assert!(!serialized.contains(repository));
    }
}
