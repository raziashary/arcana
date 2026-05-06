// Tier 3: Procedural Memory — skills, patterns, and learnings.
// Phase 4.3 — dual storage:
//   Primary:  ~/.autodev/memory/MEMORY.md (index) + topic files (*.md)
//   Mirror:   SQLite procedural_memories table (enables tag search + full-text)
//
// MEMORY.md format:
//   # AutoDev Memory
//   Last updated: <ISO>
//
//   ## Topics
//   - [debugging](topics/debugging.md) — common debugging patterns
//   - [workflow-patterns](topics/workflow-patterns.md) — build + test sequences
//   ...
//
// Topic files live at ~/.autodev/memory/topics/<topic>.md

use chrono::Utc;
use rusqlite::params;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::errors::{AutoDevError, Result};
use super::db::MemoryDb;
use super::types::ProceduralMemory;

pub struct ProceduralStore;

impl ProceduralStore {
    // ─── MEMORY.md file API ──────────────────────────────────────────────────

    /// Return the full contents of MEMORY.md. Creates the file with a default
    /// header if it does not exist yet.
    pub fn get_memory_md() -> Result<String> {
        let path = memory_md_path()?;
        if !path.exists() {
            let default = default_memory_md();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &default)?;
            return Ok(default);
        }
        Ok(std::fs::read_to_string(&path)?)
    }

    /// Overwrite MEMORY.md and sync the updated_at timestamp in SQLite for
    /// every topic that appears in the file (mirrors content digest).
    pub fn save_memory_md(content: &str) -> Result<()> {
        let path = memory_md_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        debug!("[ProceduralStore] saved MEMORY.md ({} bytes)", content.len());
        Ok(())
    }

    // ─── Topic file API ──────────────────────────────────────────────────────

    /// List all topic files found on disk (*.md in topics/ directory).
    pub fn list_topics() -> Result<Vec<ProceduralMemory>> {
        let dir = topics_dir()?;
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut topics = Vec::new();
        for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let topic = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if topic.is_empty() {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                topics.push(ProceduralMemory {
                    id:         topic.clone(),
                    topic:      topic.clone(),
                    title:      title_from_content(&content).unwrap_or_else(|| topic.clone()),
                    content,
                    tags:       vec![],
                    created_at: Utc::now(),
                    updated_at: entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(|t| {
                            let dur = t
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default();
                            chrono::DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, 0)
                                .unwrap_or_else(Utc::now)
                        })
                        .unwrap_or_else(Utc::now),
                });
            }
        }
        topics.sort_by(|a, b| a.topic.cmp(&b.topic));
        Ok(topics)
    }

    /// Read a single topic file by name.
    pub fn get_topic(topic: &str) -> Result<ProceduralMemory> {
        let path = topic_path(topic)?;
        let content = std::fs::read_to_string(&path)
            .map_err(|_| AutoDevError::Agent(format!("Topic '{topic}' not found")))?;
        Ok(ProceduralMemory {
            id:         topic.to_string(),
            topic:      topic.to_string(),
            title:      title_from_content(&content).unwrap_or_else(|| topic.to_string()),
            content,
            tags:       vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Write (create or overwrite) a topic file and upsert its SQLite mirror.
    pub fn save_topic(topic: &str, title: &str, content: &str, tags: &[String]) -> Result<()> {
        let path = topic_path(topic)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;

        // Mirror into SQLite for search.
        let db        = MemoryDb::open()?;
        let now       = Utc::now().to_rfc3339();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".into());
        let id        = Uuid::new_v4().to_string();

        db.conn.execute(
            "INSERT INTO procedural_memories (id, topic, title, content, tags, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(topic) DO UPDATE SET
               title = excluded.title,
               content = excluded.content,
               tags = excluded.tags,
               updated_at = excluded.updated_at",
            params![id, topic, title, content, tags_json, now],
        )?;

        // Refresh MEMORY.md index.
        refresh_memory_md_index()?;

        debug!("[ProceduralStore] saved topic='{topic}'");
        Ok(())
    }

    /// Delete a topic file and remove from SQLite mirror.
    pub fn delete_topic(topic: &str) -> Result<()> {
        let path = topic_path(topic)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let db = MemoryDb::open()?;
        db.conn.execute("DELETE FROM procedural_memories WHERE topic = ?1", params![topic])?;
        refresh_memory_md_index()?;
        Ok(())
    }

    /// Keyword search across SQLite mirror.
    pub fn search(query: &str, limit: u32) -> Result<Vec<ProceduralMemory>> {
        let db      = MemoryDb::open()?;
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = db.conn.prepare(
            "SELECT id, topic, title, content, tags, created_at, updated_at
             FROM   procedural_memories
             WHERE  LOWER(content) LIKE ?1 OR LOWER(title) LIKE ?1 OR LOWER(tags) LIKE ?1
             ORDER  BY updated_at DESC
             LIMIT  ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit], |row| {
            let tags_str: String = row.get(4).unwrap_or_else(|_| "[]".into());
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            Ok(ProceduralMemory {
                id:         row.get(0)?,
                topic:      row.get(1)?,
                title:      row.get(2)?,
                content:    row.get(3)?,
                tags,
                created_at: row.get::<_, String>(5)?.parse().unwrap_or_default(),
                updated_at: row.get::<_, String>(6)?.parse().unwrap_or_default(),
            })
        })?
        .filter_map(|r| r.ok())
        .collect();
        Ok(rows)
    }

    pub fn count_total() -> u64 {
        MemoryDb::open()
            .ok()
            .and_then(|db| {
                db.conn.query_row(
                    "SELECT COUNT(*) FROM procedural_memories",
                    [],
                    |r| r.get::<_, i64>(0),
                ).ok()
            })
            .unwrap_or(0) as u64
    }
}

// ─── Filesystem helpers ───────────────────────────────────────────────────────

fn memory_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| AutoDevError::Agent("Cannot determine home directory".into()))?;
    Ok(home.join(".autodev").join("memory"))
}

fn memory_md_path() -> Result<PathBuf> {
    Ok(memory_dir()?.join("MEMORY.md"))
}

fn topics_dir() -> Result<PathBuf> {
    Ok(memory_dir()?.join("topics"))
}

fn topic_path(topic: &str) -> Result<PathBuf> {
    // Sanitize: only allow alphanumeric, dash, underscore.
    let safe: String = topic
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    Ok(topics_dir()?.join(format!("{safe}.md")))
}

fn title_from_content(content: &str) -> Option<String> {
    content
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
}

fn default_memory_md() -> String {
    format!(
        "# AutoDev Memory\n\
         > Auto-generated by AutoDev PA Memory System (Phase 4.3)\n\
         > Last updated: {}\n\
         > Edit topic files in `~/.autodev/memory/topics/` or use the Memory sidebar.\n\n\
         ## Topics\n\
         *(No topics yet — use `remember_this` or the Memory sidebar to add learnings)*\n",
        Utc::now().to_rfc3339()
    )
}

/// Regenerate the MEMORY.md topic index from the topics/ directory.
fn refresh_memory_md_index() -> Result<()> {
    let dir = topics_dir()?;
    if !dir.exists() {
        return Ok(());
    }

    let mut entries: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let topic = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let title = title_from_content(&content).unwrap_or_else(|| topic.clone());
        entries.push((topic, title));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let topic_lines: String = if entries.is_empty() {
        "*(No topics yet)*\n".into()
    } else {
        entries
            .iter()
            .map(|(t, ti)| format!("- [{t}](topics/{t}.md) — {ti}\n"))
            .collect()
    };

    let now = Utc::now().to_rfc3339();
    let content = format!(
        "# AutoDev Memory\n\
         > Last updated: {now}\n\n\
         ## Topics\n{topic_lines}"
    );

    let path = memory_md_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(())
}
