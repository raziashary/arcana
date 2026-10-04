// Annotations database — persists inline code annotations created by users or agents.
// Database path: ~/.arcana/annotations.db
// Used by: IDE CodeViewer (display), QC Agent (write), PA (write on Ask PA flow)

use super::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    pub file_path: String, // workspace-relative path
    pub start_line: i64,
    pub end_line: i64,
    pub selected_text: Option<String>,
    pub prompt: String,
    pub response: Option<String>,
    pub source: String,   // "user" | "qc_agent" | "worker"
    pub severity: String, // "info" | "warning" | "blocking"
    pub status: String,   // "open" | "pending" | "resolved"
    pub task_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAnnotationInput {
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub selected_text: Option<String>,
    pub prompt: String,
    pub source: Option<String>,
    pub severity: Option<String>,
    pub task_id: Option<String>,
}

// ─── Database Handle ──────────────────────────────────────────────────────────

pub struct AnnotationsDb {
    conn: Connection,
}

impl AnnotationsDb {
    /// This database's schema version, as recorded by the migration ladder.
    ///
    /// Exposed so a migration can be *verified* rather than assumed — see
    /// `examples/migrate_check.rs`. Reading it through the same handle the
    /// application uses is the point: a version read from a fresh connection
    /// would prove the file's state, not that the shipped `open()` path put it
    /// there.
    pub fn schema_version(&self) -> Result<i32> {
        migrate::user_version(&self.conn)
    }

    pub fn open() -> Result<Self> {
        let db_path = arcana_dir()?.join("annotations.db");
        Self::open_at(&db_path)
    }

    fn open_at(db_path: &std::path::Path) -> Result<Self> {
        let conn = crate::db::open_tuned(db_path)
            .map_err(|e| ArcanaError::Agent(format!("annotations.db open failed: {e}")))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// `annotations.db`'s ladder. `v1` is the schema as it stands, created idempotently,
    /// so an existing database is stamped without being altered.
    const LADDER: &[Migration] = &[
        Migration {
            version: 1,
            name: "annotations-baseline",
            sql: r#"
            CREATE TABLE IF NOT EXISTS annotations (
                id            TEXT PRIMARY KEY,
                file_path     TEXT NOT NULL,
                start_line    INTEGER NOT NULL,
                end_line      INTEGER NOT NULL,
                selected_text TEXT,
                prompt        TEXT NOT NULL,
                response      TEXT,
                source        TEXT NOT NULL DEFAULT 'user',
                severity      TEXT NOT NULL DEFAULT 'info',
                status        TEXT NOT NULL DEFAULT 'open',
                task_id       TEXT,
                created_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                updated_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_ann_file   ON annotations(file_path);
            CREATE INDEX IF NOT EXISTS idx_ann_status ON annotations(status);
            CREATE TRIGGER IF NOT EXISTS update_annotations_timestamp
                AFTER UPDATE ON annotations
                BEGIN
                    UPDATE annotations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
                END;
        "#,
        },
        Migration {
            version: 2,
            name: "canonical-utc-timestamp-writers",
            sql: r#"
            DROP TRIGGER IF EXISTS update_annotations_timestamp;
            CREATE TRIGGER update_annotations_timestamp
                AFTER UPDATE ON annotations
                BEGIN
                    UPDATE annotations SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id;
                END;
            CREATE TRIGGER IF NOT EXISTS annotations_timestamp_defaults
                AFTER INSERT ON annotations
                WHEN NEW.created_at = datetime('now') OR NEW.updated_at = datetime('now')
                BEGIN
                    UPDATE annotations SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                                           updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
                    WHERE id = NEW.id;
                END;
        "#,
        },
    ];

    fn migrate(&self) -> Result<()> {
        // PRAGMAs are per-connection and are not carried by a migration.
        migrate::run_ladder(&self.conn, "annotations.db", Self::LADDER)?;
        Ok(())
    }

    // ─── Queries ──────────────────────────────────────────────────────────────

    pub fn create(&self, input: &CreateAnnotationInput) -> Result<String> {
        let id = uuid_v4();
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_create",
            event_type = "trace_span",
            operation = "database_annotation_create",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_create").with_database("annotations");
        let source = input.source.as_deref().unwrap_or("user");
        let severity = input.severity.as_deref().unwrap_or("info");
        self.conn.execute(
            "INSERT INTO annotations \
             (id, file_path, start_line, end_line, selected_text, prompt, source, severity, task_id) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id, input.file_path, input.start_line, input.end_line,
                input.selected_text, input.prompt, source, severity, input.task_id
            ],
        ).map_err(|e| ArcanaError::Agent(format!("create annotation: {e}")))?;
        timing.with_result_count(1).succeed();
        Ok(id)
    }

    pub fn list_for_file(&self, file_path: &str) -> Result<Vec<Annotation>> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_list_for_file",
            event_type = "trace_span",
            operation = "database_annotation_list_for_file",
            database = "annotations",
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_list_for_file").with_database("annotations");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, start_line, end_line, selected_text, prompt, response, \
                    source, severity, status, task_id, created_at, updated_at \
             FROM annotations WHERE file_path = ?1 AND status != 'deleted' \
             ORDER BY start_line ASC",
            )
            .map_err(|e| ArcanaError::Agent(format!("list_for_file prepare: {e}")))?;

        let rows = stmt
            .query_map(params![file_path], |row| {
                Ok(Annotation {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    start_line: row.get(2)?,
                    end_line: row.get(3)?,
                    selected_text: row.get(4)?,
                    prompt: row.get(5)?,
                    response: row.get(6)?,
                    source: row.get(7)?,
                    severity: row.get(8)?,
                    status: row.get(9)?,
                    task_id: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                })
            })
            .map_err(|e| ArcanaError::Agent(format!("list_for_file query: {e}")))?;

        let annotations = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Agent(format!("list_for_file collect: {e}")))?;
        timing.with_result_count(annotations.len()).succeed();
        Ok(annotations)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Annotation>> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_read",
            event_type = "trace_span",
            operation = "database_annotation_read",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing = OperationTimer::start("database_annotation_read").with_database("annotations");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, file_path, start_line, end_line, selected_text, prompt, response, \
                    source, severity, status, task_id, created_at, updated_at \
             FROM annotations WHERE id = ?1",
            )
            .map_err(|e| ArcanaError::Agent(format!("get_by_id prepare: {e}")))?;

        stmt.query_row(params![id], |row| {
            Ok(Annotation {
                id: row.get(0)?,
                file_path: row.get(1)?,
                start_line: row.get(2)?,
                end_line: row.get(3)?,
                selected_text: row.get(4)?,
                prompt: row.get(5)?,
                response: row.get(6)?,
                source: row.get(7)?,
                severity: row.get(8)?,
                status: row.get(9)?,
                task_id: row.get(10)?,
                created_at: row.get(11)?,
                updated_at: row.get(12)?,
            })
        })
        .optional()
        .map_err(|e| ArcanaError::Agent(format!("get_by_id: {e}")))
        .map(|annotation| {
            timing
                .with_result_count(usize::from(annotation.is_some()))
                .succeed();
            annotation
        })
    }

    pub fn update_response(&self, id: &str, response: Option<&str>) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_response_update",
            event_type = "trace_span",
            operation = "database_annotation_response_update",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing = OperationTimer::start("database_annotation_response_update")
            .with_database("annotations");
        let changed = self
            .conn
            .execute(
                "UPDATE annotations SET response = ?1 WHERE id = ?2",
                params![response, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_response: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn update_task_id(&self, id: &str, task_id: &str) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_task_link",
            event_type = "trace_span",
            operation = "database_annotation_task_link",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_task_link").with_database("annotations");
        let changed = self
            .conn
            .execute(
                "UPDATE annotations SET task_id = ?1 WHERE id = ?2",
                params![task_id, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_task_id: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn update_status(&self, id: &str, status: &str) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_status_update",
            event_type = "trace_span",
            operation = "database_annotation_status_update",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_status_update").with_database("annotations");
        let changed = self
            .conn
            .execute(
                "UPDATE annotations SET status = ?1 WHERE id = ?2",
                params![status, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("update_status: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn resolve(&self, id: &str, response: Option<&str>) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_resolve",
            event_type = "trace_span",
            operation = "database_annotation_resolve",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_resolve").with_database("annotations");
        let changed = self
            .conn
            .execute(
                "UPDATE annotations SET status = 'resolved', response = ?1 WHERE id = ?2",
                params![response, id],
            )
            .map_err(|e| ArcanaError::Agent(format!("resolve: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let _trace = tracing::info_span!(
            target: "arcana::telemetry",
            "database_annotation_delete",
            event_type = "trace_span",
            operation = "database_annotation_delete",
            database = "annotations",
            annotation_id = %id,
        )
        .entered();
        let timing =
            OperationTimer::start("database_annotation_delete").with_database("annotations");
        let changed = self
            .conn
            .execute(
                "UPDATE annotations SET status = 'deleted' WHERE id = ?1",
                params![id],
            )
            .map_err(|e| ArcanaError::Agent(format!("delete: {e}")))?;
        timing.with_result_count(changed).succeed();
        Ok(())
    }
}

// ─── UUID helper ──────────────────────────────────────────────────────────────

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    // Simple but collision-resistant enough for local annotations:
    // combine timestamp micros + thread id + nanos
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros();
    format!("ann-{:016x}-{:08x}", ts, nanos)
}

#[cfg(test)]
mod timestamp_tests {
    use super::*;

    fn input() -> CreateAnnotationInput {
        CreateAnnotationInput {
            file_path: "src/lib.rs".into(),
            start_line: 1,
            end_line: 1,
            selected_text: None,
            prompt: "review".into(),
            source: None,
            severity: None,
            task_id: None,
        }
    }

    #[test]
    fn annotation_database_operations_emit_safe_spans_and_results() {
        let dir = tempfile::tempdir().expect("isolated annotation DB directory");
        let db = AnnotationsDb::open_at(&dir.path().join("annotations.db"))
            .expect("open production annotation database");
        let mut input = input();
        input.file_path = "private/project/file.rs".into();
        input.prompt = "private annotation prompt".into();

        let (created, events) =
            crate::observability::timing::capture_json_events(|| db.create(&input));
        created.expect("create annotation");
        let create_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_annotation_create")
            .expect("annotation create result");
        assert_eq!(create_event["fields"]["database"], "annotations");
        assert_eq!(create_event["fields"]["outcome"], "success");
        assert_eq!(create_event["fields"]["result_count"], 1);
        assert!(!create_event.to_string().contains("private/project/file.rs"));
        assert!(!create_event
            .to_string()
            .contains("private annotation prompt"));

        let (listed, events) = crate::observability::timing::capture_json_events(|| {
            db.list_for_file(&input.file_path)
        });
        assert_eq!(listed.expect("list annotations").len(), 1);
        let statement = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "database_statement")
            .expect("profiled annotation SQL event");
        assert!(statement["spans"].as_array().is_some_and(|spans| {
            spans.iter().any(|span| {
                span["operation"] == "database_annotation_list_for_file"
                    && span["database"] == "annotations"
            })
        }));
        assert!(!statement.to_string().contains("private/project/file.rs"));

        let (missing, events) = crate::observability::timing::capture_json_events(|| {
            db.get_by_id("missing-annotation")
        });
        assert!(missing
            .expect("missing annotation is a valid empty read")
            .is_none());
        let read_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_annotation_read")
            .expect("empty annotation read result");
        assert_eq!(read_event["fields"]["outcome"], "success");
        assert_eq!(read_event["fields"]["result_count"], 0);

        db.conn
            .execute_batch(
                "CREATE TRIGGER fail_annotation_insert BEFORE INSERT ON annotations
                 BEGIN SELECT RAISE(ABORT, 'injected annotation failure'); END;",
            )
            .expect("inject annotation database failure");
        let (failed, events) =
            crate::observability::timing::capture_json_events(|| db.create(&input));
        assert!(failed.is_err());
        let error_event = events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_annotation_create")
            .expect("annotation create error result");
        assert_eq!(error_event["fields"]["outcome"], "error");
        assert!(!error_event.to_string().contains("private/project/file.rs"));
        assert!(!error_event
            .to_string()
            .contains("private annotation prompt"));
    }

    #[test]
    fn legacy_rows_remain_readable_and_new_default_and_trigger_writes_are_canonical() {
        let dir = tempfile::tempdir().expect("isolated annotation DB directory");
        let _home = crate::db::test_support::with_home(dir.path());
        let path = dir.path().join("annotations.db");
        {
            let conn = Connection::open(&path).expect("create legacy db");
            conn.execute_batch(
                "CREATE TABLE annotations (
                    id TEXT PRIMARY KEY, file_path TEXT NOT NULL, start_line INTEGER NOT NULL,
                    end_line INTEGER NOT NULL, selected_text TEXT, prompt TEXT NOT NULL,
                    response TEXT, source TEXT NOT NULL DEFAULT 'user', severity TEXT NOT NULL DEFAULT 'info',
                    status TEXT NOT NULL DEFAULT 'open', task_id TEXT,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO annotations (id, file_path, start_line, end_line, prompt, created_at, updated_at)
                VALUES ('legacy', 'src/old.rs', 2, 2, 'old', '2020-02-03 04:05:06', '2020-02-03 04:05:06');",
            )
            .expect("seed legacy row");
        }

        let db =
            AnnotationsDb::open_at(&path).expect("migrate legacy DB through production opener");
        let old = db
            .get_by_id("legacy")
            .expect("read legacy row")
            .expect("row exists");
        assert_eq!(old.created_at, "2020-02-03 04:05:06");
        assert_eq!(old.updated_at, "2020-02-03 04:05:06");

        let id = db
            .create(&input())
            .expect("insert through production writer");
        let created = db.get_by_id(&id).expect("reader path").expect("new row");
        assert!(created.created_at.ends_with('Z'));
        assert_eq!(created.created_at.len(), 24);
        assert_eq!(created.updated_at, created.created_at);

        db.update_response("legacy", Some("changed"))
            .expect("update legacy row");
        let updated = db
            .get_by_id("legacy")
            .expect("read updated legacy row")
            .unwrap();
        assert_eq!(updated.created_at, "2020-02-03 04:05:06");
        assert!(updated.updated_at.ends_with('Z'));
    }
}
