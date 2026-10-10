// Tier 3: Procedural Memory — skills, patterns, and learnings.
// Phase 4.3 — dual storage:
//   Primary:  ~/.arcana/memory/MEMORY.md (index) + topic files (*.md)
//   Mirror:   SQLite procedural_memories table (enables tag search + full-text)
//
// MEMORY.md format:
//   # Arcana Memory
//   Last updated: <ISO>
//
//   ## Topics
//   - [debugging](topics/debugging.md) — common debugging patterns
//   - [workflow-patterns](topics/workflow-patterns.md) — build + test sequences
//   ...
//
// Topic files live at ~/.arcana/memory/topics/<topic>.md

use chrono::Utc;
use rusqlite::params;
use std::path::{Path, PathBuf};
use tracing::debug;
use uuid::Uuid;

use super::db::MemoryDb;
use super::types::ProceduralMemory;
use crate::errors::{ArcanaError, Result};

pub struct ProceduralStore;

impl ProceduralStore {
    // ─── MEMORY.md file API ──────────────────────────────────────────────────

    /// Return the full contents of MEMORY.md. Creates the file with a default
    /// header if it does not exist yet.
    pub fn get_memory_md() -> Result<String> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_index_read");
        let path = memory_md_path()?;
        if !path.exists() {
            let default = default_memory_md();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, &default)?;
            timer = timer.with_result_bytes(default.len());
            timer.succeed();
            return Ok(default);
        }
        let content = std::fs::read_to_string(&path)?;
        timer = timer.with_result_bytes(content.len());
        timer.succeed();
        Ok(content)
    }

    /// Overwrite the MEMORY.md index file without changing topic records.
    /// Topic rows and their SQLite search mirror are maintained by
    /// `save_topic` and `delete_topic`; those operations regenerate this index.
    pub fn save_memory_md(content: &str) -> Result<()> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_index_write");
        let path = memory_md_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;
        timer = timer.with_result_bytes(content.len());
        debug!(
            "[ProceduralStore] saved MEMORY.md ({} bytes)",
            content.len()
        );
        timer.succeed();
        Ok(())
    }

    // ─── Topic file API ──────────────────────────────────────────────────────

    /// List all topic files found on disk (*.md in topics/ directory).
    pub fn list_topics() -> Result<Vec<ProceduralMemory>> {
        Self::list_topics_at(&topics_dir()?)
    }

    fn list_topics_at(dir: &Path) -> Result<Vec<ProceduralMemory>> {
        let timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_topics_list");
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                timer.with_result_count(0).succeed();
                return Ok(vec![]);
            }
            Err(error) => return Err(error.into()),
        };
        let mut topics = Vec::new();
        for entry in entries {
            let entry = entry?;
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
            let content = std::fs::read_to_string(&path)?;
            topics.push(ProceduralMemory {
                id: topic.clone(),
                topic: topic.clone(),
                title: title_from_content(&content).unwrap_or_else(|| topic.clone()),
                content,
                tags: vec![],
                created_at: Utc::now(),
                updated_at: entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .map(|t| {
                        let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                        chrono::DateTime::<Utc>::from_timestamp(dur.as_secs() as i64, 0)
                            .unwrap_or_else(Utc::now)
                    })
                    .unwrap_or_else(Utc::now),
            });
        }
        topics.sort_by(|a, b| a.topic.cmp(&b.topic));
        timer.with_result_count(topics.len()).succeed();
        Ok(topics)
    }

    /// Read a single topic file by name.
    pub fn get_topic(topic: &str) -> Result<ProceduralMemory> {
        let timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_topic_read");
        let path = topic_path(topic)?;
        let content = std::fs::read_to_string(&path)
            .map_err(|_| ArcanaError::Agent(format!("Topic '{topic}' not found")))?;
        let memory = ProceduralMemory {
            id: topic.to_string(),
            topic: topic.to_string(),
            title: title_from_content(&content).unwrap_or_else(|| topic.to_string()),
            content,
            tags: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        timer.with_result_count(1).succeed();
        Ok(memory)
    }

    /// Write (create or overwrite) a topic file and upsert its SQLite mirror.
    pub fn save_topic(topic: &str, title: &str, content: &str, tags: &[String]) -> Result<()> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_write")
                .with_database("memory");
        // Redaction boundary (M-40, P1-21). Memory is persistent, so a secret
        // written here is kept forever.
        let content = &crate::security::redaction::redact_text(content);

        let path = topic_path(topic)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;

        // Mirror into SQLite for search.
        let db = MemoryDb::open()?;
        let now = crate::db::timestamps::now_rfc3339();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".into());
        let id = Uuid::new_v4().to_string();

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
        timer.succeed();
        Ok(())
    }

    /// Delete a topic file and remove from SQLite mirror.
    pub fn delete_topic(topic: &str) -> Result<()> {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_topic_delete")
                .with_database("memory");
        let path = topic_path(topic)?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let db = MemoryDb::open()?;
        db.conn.execute(
            "DELETE FROM procedural_memories WHERE topic = ?1",
            params![topic],
        )?;
        refresh_memory_md_index()?;
        timer.succeed();
        Ok(())
    }

    /// Keyword search across SQLite mirror.
    pub fn search(query: &str, limit: u32) -> Result<Vec<ProceduralMemory>> {
        let timer = crate::observability::timing::OperationTimer::start("memory_procedural_search")
            .with_database("memory");
        let db = MemoryDb::open()?;
        let pattern = format!("%{}%", query.to_lowercase());
        let mut stmt = db.conn.prepare(
            "SELECT id, topic, title, content, tags, created_at, updated_at
             FROM   procedural_memories
             WHERE  LOWER(content) LIKE ?1 OR LOWER(title) LIKE ?1 OR LOWER(tags) LIKE ?1
             ORDER  BY julianday(updated_at) DESC, id DESC
             LIMIT  ?2",
        )?;
        let malformed_tag_sets = std::cell::Cell::new(0usize);
        let mapped = stmt.query_map(params![pattern, limit], |row| {
            let tags_str: String = match row.get(4) {
                Ok(tags) => tags,
                Err(_) => {
                    malformed_tag_sets.set(malformed_tag_sets.get() + 1);
                    "[]".into()
                }
            };
            let tags = match serde_json::from_str(&tags_str) {
                Ok(tags) => tags,
                Err(_) => {
                    malformed_tag_sets.set(malformed_tag_sets.get() + 1);
                    Vec::new()
                }
            };
            Ok(ProceduralMemory {
                id: row.get(0)?,
                topic: row.get(1)?,
                title: row.get(2)?,
                content: row.get(3)?,
                tags,
                created_at: crate::db::timestamps::parse_row_timestamp(row, 5)?,
                updated_at: crate::db::timestamps::parse_row_timestamp(row, 6)?,
            })
        })?;
        let mut rows = Vec::new();
        let mut skipped_rows = 0usize;
        for row in mapped {
            match row {
                Ok(memory) => rows.push(memory),
                Err(_) => skipped_rows += 1,
            }
        }
        finish_search_observation(timer, rows.len(), skipped_rows, malformed_tag_sets.get());
        Ok(rows)
    }

    pub fn count_total() -> u64 {
        Self::count_total_with_health().0
    }

    pub(crate) fn count_total_with_health() -> (u64, bool) {
        let mut timer =
            crate::observability::timing::OperationTimer::start("memory_procedural_count_total")
                .with_database("memory");
        let result = MemoryDb::open().ok().and_then(|db| {
            db.conn
                .query_row("SELECT COUNT(*) FROM procedural_memories", [], |r| {
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
}

fn finish_search_observation(
    timer: crate::observability::timing::OperationTimer,
    count: usize,
    skipped_rows: usize,
    malformed_tag_sets: usize,
) {
    tracing::info!(
        target: "arcana::telemetry",
        event_type = "memory_search_result",
        memory_tier = "procedural",
        result_count = count,
        skipped_rows,
        malformed_tag_sets,
        "Procedural memory search returned"
    );
    let mut timer = timer.with_result_count(count);
    if skipped_rows == 0 && malformed_tag_sets == 0 {
        timer.succeed();
    } else {
        timer.set_outcome("degraded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    #[test]
    fn production_procedural_search_orders_mixed_timestamps_by_instant() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        ProceduralStore::save_topic("canonical", "needle", "needle content", &[])
            .expect("write canonical topic through production method");
        ProceduralStore::save_topic("legacy", "needle", "needle content", &[])
            .expect("write legacy topic through production method");
        ProceduralStore::save_topic("offset", "needle", "needle content", &[])
            .expect("write offset topic through production method");
        let db = MemoryDb::open().expect("open production memory DB");
        db.conn
            .execute(
                "UPDATE procedural_memories SET updated_at = CASE topic
                    WHEN 'canonical' THEN '2026-10-10T09:00:00.000Z'
                    WHEN 'legacy' THEN '2026-10-10 12:00:00'
                    WHEN 'offset' THEN '2026-10-10T23:00:00+14:00'
                 END WHERE topic IN ('canonical', 'legacy', 'offset')",
                [],
            )
            .expect("retain mixed canonical and legacy timestamp shapes");
        drop(db);
        let rows = ProceduralStore::search("needle", 1).expect("reopen actual search reader");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].topic, "legacy", "limit must select newest instant");
        assert_eq!(
            rows[0].updated_at,
            crate::db::timestamps::parse_db_timestamp("2026-10-10T12:00:00Z")
                .expect("expected UTC instant")
        );
        let all = ProceduralStore::search("needle", 10).expect("read equal-instant topics");
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].topic, "legacy");
        assert_eq!(all[1].updated_at, all[2].updated_at);
        assert!(all[1].id > all[2].id, "equal instants have stable ID order");
        for topic in &all {
            assert_eq!(
                topic.created_at.timestamp_subsec_nanos() % 1_000_000,
                0,
                "the actual topic writer persists millisecond creation time"
            );
        }
        assert!(ProceduralStore::search("unmatched", 10)
            .expect("normal keyword filter")
            .is_empty());
        assert!(ProceduralStore::search("needle", 0)
            .expect("zero result limit")
            .is_empty());
    }

    #[test]
    fn procedural_search_result_and_timing_events_agree_on_result_count() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("capture telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            finish_search_observation(
                crate::observability::timing::OperationTimer::start("memory_procedural_search")
                    .with_database("memory"),
                2,
                0,
                0,
            );
        });

        let output = String::from_utf8(output.lock().expect("capture output").clone())
            .expect("UTF-8 JSON events");
        let events = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .collect::<Vec<_>>();
        let result = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "memory_search_result")
            .expect("procedural search result event");
        let timing = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "operation_timing")
            .expect("procedural search timing event");
        assert_eq!(result["fields"]["result_count"], 2);
        assert_eq!(result["fields"]["skipped_rows"], 0);
        assert_eq!(result["fields"]["malformed_tag_sets"], 0);
        assert_eq!(timing["fields"]["result_count"], 2);
        assert_eq!(timing["fields"]["database"], "memory");
        assert_eq!(timing["fields"]["outcome"], "success");
    }

    #[test]
    fn procedural_search_marks_dropped_rows_as_degraded() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = MemoryDb::open().expect("open migrated test memory DB");
        db.conn
            .execute(
                "INSERT INTO procedural_memories (id, topic, title, content, tags, created_at, updated_at)
                 VALUES ('good-row', 'good', 'needle good', 'needle content', '[]', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                [],
            )
            .expect("insert valid row");
        db.conn
            .execute(
                "INSERT INTO procedural_memories (id, topic, title, content, tags, created_at, updated_at)
                 VALUES ('bad-row', 'bad', 'needle bad', 'needle content', '[]', 'not-a-timestamp', '2026-10-03T00:00:00Z')",
                [],
            )
            .expect("insert corrupt row");
        drop(db);

        let (result, events) = crate::observability::timing::capture_json_events(|| {
            ProceduralStore::search("needle", 10)
        });
        assert_eq!(result.expect("search remains available").len(), 1);
        let result_event = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "memory_search_result")
            .expect("search result telemetry");
        let timing_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_procedural_search")
            .expect("search operation timing");
        assert_eq!(result_event["fields"]["result_count"], 1);
        assert_eq!(result_event["fields"]["skipped_rows"], 1);
        assert_eq!(result_event["fields"]["malformed_tag_sets"], 0);
        assert_eq!(timing_event["fields"]["outcome"], "degraded");
        assert_eq!(timing_event["fields"]["result_count"], 1);
    }

    #[test]
    fn malformed_tag_json_is_reported_without_calling_the_row_dropped() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home_guard = crate::db::test_support::with_home(home.path());
        let db = MemoryDb::open().expect("open migrated test memory DB");
        db.conn
            .execute(
                "INSERT INTO procedural_memories (id, topic, title, content, tags, created_at, updated_at)
                 VALUES ('tag-row', 'topic', 'needle title', 'needle content', '{invalid', '2026-10-03T00:00:00Z', '2026-10-03T00:00:00Z')",
                [],
            )
            .expect("insert row with malformed tags");
        drop(db);

        let (result, events) = crate::observability::timing::capture_json_events(|| {
            ProceduralStore::search("needle", 10)
        });
        let results = result.expect("row remains available with empty tags");
        assert_eq!(results.len(), 1);
        assert!(results[0].tags.is_empty());
        let event = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "memory_search_result")
            .expect("procedural result event");
        assert_eq!(event["fields"]["result_count"], 1);
        assert_eq!(event["fields"]["skipped_rows"], 0);
        assert_eq!(event["fields"]["malformed_tag_sets"], 1);
        let timing = events
            .iter()
            .find(|event| event["fields"]["operation"] == "memory_procedural_search")
            .expect("procedural operation event");
        assert_eq!(timing["fields"]["outcome"], "degraded");
    }

    #[test]
    fn topic_listing_reports_invalid_topic_file_and_records_error_outcome() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("capture telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let directory = tempfile::tempdir().expect("temporary topic directory");
        std::fs::write(directory.path().join("good.md"), "# Good\nReadable topic")
            .expect("write readable topic");
        std::fs::write(directory.path().join("broken.md"), [0xff, 0xfe])
            .expect("write invalid UTF-8 topic");
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        let result = tracing::subscriber::with_default(subscriber, || {
            ProceduralStore::list_topics_at(directory.path())
        });

        assert!(
            result.is_err(),
            "invalid topic data must not look like a complete list"
        );
        let output = String::from_utf8(output.lock().expect("capture output").clone())
            .expect("UTF-8 JSON events");
        let event = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .find(|event| {
                event["fields"]["event_type"] == "operation_timing"
                    && event["fields"]["operation"] == "memory_procedural_topics_list"
            })
            .expect("topic listing operation event");
        assert_eq!(event["fields"]["outcome"], "error");
    }

    #[test]
    fn index_refresh_reports_unreadable_topic_instead_of_writing_partial_index() {
        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("capture telemetry")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let root = tempfile::tempdir().expect("temporary memory directory");
        let topics = root.path().join("topics");
        std::fs::create_dir_all(&topics).expect("create topics directory");
        std::fs::write(topics.join("good.md"), "# Good\nReadable topic")
            .expect("write readable topic");
        std::fs::write(topics.join("broken.md"), [0xff, 0xfe]).expect("write invalid UTF-8 topic");
        let index = root.path().join("MEMORY.md");

        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        let result = tracing::subscriber::with_default(subscriber, || {
            refresh_memory_md_index_at(&topics, &index)
        });

        assert!(
            result.is_err(),
            "index refresh must not accept partial topic data"
        );
        assert!(!index.exists(), "partial index must not be written");
        let output = String::from_utf8(output.lock().expect("capture output").clone())
            .expect("UTF-8 JSON events");
        let event = output
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON event"))
            .find(|event| {
                event["fields"]["event_type"] == "operation_timing"
                    && event["fields"]["operation"] == "memory_procedural_index_refresh"
            })
            .expect("index refresh operation event");
        assert_eq!(event["fields"]["outcome"], "error");
    }
}

// ─── Filesystem helpers ───────────────────────────────────────────────────────

fn memory_dir() -> Result<PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| ArcanaError::Agent("Cannot determine home directory".into()))?;
    Ok(home.join(".arcana").join("memory"))
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
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
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
        "# Arcana Memory\n\
         > Auto-generated by Arcana PA Memory System (Phase 4.3)\n\
         > Last updated: {}\n\
         > Edit topic files in `~/.arcana/memory/topics/` or use the Memory sidebar.\n\n\
         ## Topics\n\
         *(No topics yet — use `propose_memory` or the Memory sidebar to add learnings)*\n",
        crate::db::timestamps::now_rfc3339()
    )
}

/// Regenerate the MEMORY.md topic index from the topics/ directory.
fn refresh_memory_md_index() -> Result<()> {
    refresh_memory_md_index_at(&topics_dir()?, &memory_md_path()?)
}

fn refresh_memory_md_index_at(dir: &Path, path: &Path) -> Result<()> {
    let timer =
        crate::observability::timing::OperationTimer::start("memory_procedural_index_refresh");
    let entries_on_disk = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            timer.with_result_count(0).succeed();
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };

    let mut entries: Vec<(String, String)> = Vec::new();
    for entry in entries_on_disk {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let topic = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let content = std::fs::read_to_string(&path)?;
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

    let now = crate::db::timestamps::now_rfc3339();
    let content = format!(
        "# Arcana Memory\n\
         > Last updated: {now}\n\n\
         ## Topics\n{topic_lines}"
    );

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    timer.with_result_count(entries.len()).succeed();
    Ok(())
}
