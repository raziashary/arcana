//! Shared local tracing output for the desktop host and `arcanactl`.
//!
//! Keep the filenames and JSON record shape aligned with the desktop Logs
//! reader: readable daily logs use `arcana.YYYY-MM-DD.log`, and structured
//! telemetry uses `arcana-events.YYYY-MM-DD.jsonl`. Structured output is
//! restricted to the `arcana::telemetry` target family.

use std::{
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{
    filter::filter_fn, fmt, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
    EnvFilter, Layer,
};

/// Guards that keep the background log writers alive. Keep this value in the
/// host's main scope for the full process lifetime to flush and close the
/// appenders on shutdown.
pub struct LoggingGuards {
    guards: Vec<WorkerGuard>,
    health: LoggingHealth,
}

/// Process-local counters for failures in Arcana's asynchronous log writers.
/// The logging worker remains isolated from application operations; the Logs
/// panel reads these counters through the host instead of relying on that same
/// writer to report its own failure.
#[derive(Clone, Default)]
pub struct LoggingHealth {
    readable_write_failures: Arc<AtomicU64>,
    structured_write_failures: Arc<AtomicU64>,
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct LoggingHealthSnapshot {
    pub readable_write_failures: u64,
    pub structured_write_failures: u64,
}

static PROCESS_HEALTH: OnceLock<LoggingHealth> = OnceLock::new();

/// Read the process-wide writer counters used by the desktop Logs metrics.
pub fn process_health_snapshot() -> LoggingHealthSnapshot {
    PROCESS_HEALTH
        .get_or_init(LoggingHealth::default)
        .snapshot()
}

impl LoggingHealth {
    pub fn snapshot(&self) -> LoggingHealthSnapshot {
        LoggingHealthSnapshot {
            readable_write_failures: self.readable_write_failures.load(Ordering::Relaxed),
            structured_write_failures: self.structured_write_failures.load(Ordering::Relaxed),
        }
    }
}

impl LoggingGuards {
    /// Number of active asynchronous file writers. Zero means the home log
    /// directory could not be prepared and output is console-only.
    pub fn writer_count(&self) -> usize {
        self.guards.len()
    }

    pub fn health(&self) -> LoggingHealth {
        self.health.clone()
    }
}

/// Install Arcana's standard process-wide tracing subscriber.
///
/// File output is best-effort: an unavailable home directory or log directory
/// leaves the redacted console layer active. This function must be called once
/// before other code emits tracing events.
pub fn init() -> LoggingGuards {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("arcana_lib=debug,info"));
    init_with_filter(filter)
}

/// Install the shared subscriber with a host-selected filter. The CLI uses
/// this to preserve its `--verbose` and `--quiet` behavior while sharing the
/// same readable and structured file streams as the desktop app.
pub fn init_with_filter(filter: EnvFilter) -> LoggingGuards {
    let console_layer = fmt::layer()
        .with_writer(RedactingMakeWriter(std::io::stderr))
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_ansi(false)
        .compact();

    let log_dir = resolve_log_dir();
    let Some(dir) = log_dir else {
        tracing_subscriber::registry()
            .with(console_layer.with_filter(filter))
            .init();
        tracing::info!("Arcana logging initialised — console only (no writable log dir)");
        return LoggingGuards {
            guards: Vec::new(),
            health: LoggingHealth::default(),
        };
    };

    let health = PROCESS_HEALTH.get_or_init(LoggingHealth::default).clone();
    let readable_appender = rolling_appender(&dir, "arcana", "log");
    let (readable_writer, readable_guard) = tracing_appender::non_blocking(RedactingWriter {
        inner: ObservedWriter {
            inner: readable_appender,
            failures: Arc::clone(&health.readable_write_failures),
        },
        pending: Vec::new(),
        overflowed: false,
    });
    let readable_layer = fmt::layer()
        .with_writer(readable_writer)
        .with_ansi(false)
        .with_target(true)
        .compact();

    let structured_appender = rolling_appender(&dir, "arcana-events", "jsonl");
    let (structured_writer, structured_guard) =
        tracing_appender::non_blocking(RedactingJsonWriter {
            inner: ObservedWriter {
                inner: structured_appender,
                failures: Arc::clone(&health.structured_write_failures),
            },
            pending: Vec::new(),
        });
    let structured_layer = structured_telemetry_layer(structured_writer);
    // Console/readable verbosity is user-configurable, but the structured
    // stream is the observability contract. Keep sub-10ms SQLite statements
    // and other DEBUG telemetry even under the default INFO console filter.
    let structured_filter = filter.clone().add_directive(
        "arcana::telemetry=debug"
            .parse()
            .expect("valid filter directive"),
    );

    tracing_subscriber::registry()
        .with(console_layer.with_filter(filter.clone()))
        .with(readable_layer.with_filter(filter))
        .with(structured_layer.with_filter(structured_filter))
        .init();

    tracing::info!(
        "Arcana logging initialised — daily rotation active, log dir: {}",
        dir.display()
    );
    LoggingGuards {
        guards: vec![readable_guard, structured_guard],
        health,
    }
}

fn resolve_log_dir() -> Option<PathBuf> {
    let dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(|home| PathBuf::from(home).join(".arcana").join("logs"))?;
    match std::fs::create_dir_all(&dir) {
        Ok(()) => Some(dir),
        Err(error) => {
            eprintln!(
                "[Arcana] Failed to create log dir {}: {error}",
                dir.display()
            );
            None
        }
    }
}

fn rolling_appender(
    dir: &std::path::Path,
    filename_prefix: &str,
    filename_suffix: &str,
) -> tracing_appender::rolling::RollingFileAppender {
    tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix(filename_prefix)
        .filename_suffix(filename_suffix)
        .build(dir)
        .unwrap_or_else(|error| {
            eprintln!(
                "[Arcana] Log appender {filename_prefix} failed ({error}), falling back to default rotation"
            );
            tracing_appender::rolling::daily(dir, filename_prefix)
        })
}

/// Maximum bytes retained for one readable record before the record is omitted.
const MAX_READABLE_RECORD_BYTES: usize = 256 * 1024;

/// Redacts content at the writer boundary so every producer is covered.
struct RedactingWriter<W: Write> {
    inner: W,
    pending: Vec<u8>,
    overflowed: bool,
}

impl<W: Write> RedactingWriter<W> {
    fn emit_record(&mut self, record: &[u8]) -> std::io::Result<()> {
        let text = String::from_utf8_lossy(record);
        let redacted = crate::security::redaction::redact_text(&text);
        self.inner.write_all(redacted.as_bytes())?;
        self.inner.write_all(b"\n")
    }

    fn emit_overflow_marker(&mut self) -> std::io::Result<()> {
        self.inner
            .write_all(b"[Arcana] readable log record omitted: redaction buffer limit exceeded\n")
    }
}

struct ObservedWriter<W: Write> {
    inner: W,
    failures: Arc<AtomicU64>,
}

impl<W: Write> Write for ObservedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.inner.write(buf) {
            Ok(written) => Ok(written),
            Err(error) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.failures.fetch_add(1, Ordering::Relaxed);
                Err(error)
            }
        }
    }
}

/// Redact structured records as JSON values so sensitive fields stay valid
/// JSON after replacement. The writer buffers until a newline because the
/// formatting layer may split one event across several `write` calls.
struct RedactingJsonWriter<W: Write> {
    inner: W,
    pending: Vec<u8>,
}

impl<W: Write> RedactingJsonWriter<W> {
    fn write_record(&mut self, record: &[u8]) -> std::io::Result<()> {
        let redacted = match serde_json::from_slice::<serde_json::Value>(record) {
            Ok(value) => serde_json::to_vec(&redact_telemetry_record(&value))
                .map_err(std::io::Error::other)?,
            Err(_) => crate::security::redaction::redact_text(&String::from_utf8_lossy(record))
                .into_bytes(),
        };
        self.inner.write_all(&redacted)?;
        self.inner.write_all(b"\n")
    }
}

fn redact_telemetry_record(value: &serde_json::Value) -> serde_json::Value {
    let mut redacted = crate::security::redaction::redact_json(value);
    let Some(target) = value["target"].as_str() else {
        return redacted;
    };
    if target != "arcana::telemetry" && !target.starts_with("arcana::telemetry::") {
        return redacted;
    }
    for field in ["fields", "span"] {
        if let (Some(original), Some(output)) = (value.get(field), redacted.get_mut(field)) {
            restore_registered_labels(original, output);
        }
    }
    if let (Some(original), Some(output)) = (
        value.get("spans").and_then(serde_json::Value::as_array),
        redacted
            .get_mut("spans")
            .and_then(serde_json::Value::as_array_mut),
    ) {
        for (span, redacted_span) in original.iter().zip(output) {
            restore_registered_labels(span, redacted_span);
        }
    }
    redacted
}

fn restore_registered_labels(original: &serde_json::Value, output: &mut serde_json::Value) {
    if let (Some(original), Some(output)) = (original.as_object(), output.as_object_mut()) {
        for (field, value) in original {
            let Some(value) = value.as_str() else {
                continue;
            };
            let is_registered_label = matches!(field.as_str(), "event_type" | "operation")
                && super::telemetry_labels::is_registered(field, value);
            let is_correlation_uuid = is_correlation_field(field) && is_canonical_uuid(value);
            if is_registered_label || is_correlation_uuid {
                output.insert(field.clone(), serde_json::Value::String(value.to_owned()));
            } else if matches!(field.as_str(), "call_id" | "tool_call_id") && !value.is_empty() {
                output.insert(
                    field.clone(),
                    serde_json::Value::String(telemetry_call_id(value)),
                );
            }
        }
    }
}

/// Stable, privacy-safe identity for an Arcana telemetry tool call.
///
/// Canonical UUIDs retain their existing identity. Compound session/provider
/// IDs become domain-separated SHA-256-derived UUIDs; their raw text never
/// bypasses logging redaction. Provider and tool-result IDs are unchanged.
/// Only designated call fields on Arcana telemetry records use this mapping.
pub fn telemetry_call_id(raw: &str) -> String {
    use sha2::{Digest, Sha256};

    if is_canonical_uuid(raw) {
        return raw.to_owned();
    }
    let mut hash = Sha256::new();
    hash.update(b"arcana.telemetry.tool-call.v1\0");
    hash.update(raw.as_bytes());
    let digest = hash.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // RFC 9562 version 8 identifies an application-defined deterministic UUID.
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

/// Correlation fields that have Arcana telemetry producers and are consumed
/// by the structured log reader. Keep this list finite: an arbitrary UUID in a
/// payload is still subject to the normal structured-value redaction rules.
fn is_correlation_field(field: &str) -> bool {
    matches!(
        field,
        "correlation_id"
            | "fleet_correlation_id"
            | "trace_id"
            | "run_id"
            | "session_id"
            | "call_id"
            | "tool_call_id"
    )
}

/// Accept only the canonical lowercase, hyphenated UUID spelling. Parsing a
/// UUID alone would also accept alternate spellings that the producer never
/// emits and would broaden the redaction exception unnecessarily.
fn is_canonical_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes.iter().enumerate().all(|(index, byte)| {
            matches!(index, 8 | 13 | 18 | 23)
                || byte.is_ascii_digit()
                || (b'a'..=b'f').contains(byte)
        })
}

impl<W: Write> Write for RedactingJsonWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(newline) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut record = self.pending.drain(..=newline).collect::<Vec<_>>();
            record.pop();
            self.write_record(&record)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if !self.pending.is_empty() {
            let record = std::mem::take(&mut self.pending);
            self.write_record(&record)?;
        }
        self.inner.flush()
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for segment in buf.split_inclusive(|byte| *byte == b'\n') {
            let complete = segment.last() == Some(&b'\n');
            if self.overflowed {
                if complete {
                    self.emit_overflow_marker()?;
                    self.overflowed = false;
                }
                continue;
            }

            if self.pending.len().saturating_add(segment.len()) > MAX_READABLE_RECORD_BYTES {
                self.pending.clear();
                self.overflowed = true;
                if complete {
                    self.emit_overflow_marker()?;
                    self.overflowed = false;
                }
                continue;
            }

            self.pending.extend_from_slice(segment);
            if complete {
                self.pending.pop();
                let record = std::mem::take(&mut self.pending);
                self.emit_record(&record)?;
            }
        }
        // Report the whole input consumed because the writer may retain it
        // until its complete record is available for cross-write redaction.
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // Keep an incomplete record buffered across flushes. Redacting a
        // fragment here could miss a credential whose remaining bytes arrive
        // in the next write. Drop finalizes any remaining bounded record.
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        if self.overflowed {
            let _ = self.emit_overflow_marker();
        } else if !self.pending.is_empty() {
            let record = std::mem::take(&mut self.pending);
            let _ = self.emit_record(&record);
        }
        let _ = self.inner.flush();
    }
}

struct RedactingMakeWriter<M>(M);

impl<'a, M: fmt::MakeWriter<'a>> fmt::MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter {
            inner: self.0.make_writer(),
            pending: Vec::new(),
            overflowed: false,
        }
    }
}

fn structured_telemetry_layer<S, W>(writer: W) -> impl Layer<S> + Send + Sync + 'static
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    for<'a> W: fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    fmt::layer()
        .json()
        .with_writer(writer)
        .with_target(true)
        .with_ansi(false)
        .with_current_span(true)
        .with_span_list(true)
        .with_filter(filter_fn(|metadata| {
            metadata.target() == "arcana::telemetry"
                || metadata.target().starts_with("arcana::telemetry::")
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::{layer::SubscriberExt, Registry};

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected telemetry write failure"))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("injected telemetry flush failure"))
        }
    }

    #[test]
    fn telemetry_writer_failure_is_counted_without_losing_database_work() {
        use crate::db::tasks::{CreateTaskInput, TasksDb};

        let dir = tempfile::tempdir().expect("isolated task DB directory");
        let database = TasksDb::open_at(&dir.path().join("tasks.db")).expect("open isolated DB");
        let health = LoggingHealth::default();
        let writer = ObservedWriter {
            inner: FailingWriter,
            failures: Arc::clone(&health.structured_write_failures),
        };
        let (writer, guard) = tracing_appender::non_blocking(RedactingJsonWriter {
            inner: writer,
            pending: Vec::new(),
        });
        let subscriber = Registry::default().with(structured_telemetry_layer(writer));

        let task = tracing::subscriber::with_default(subscriber, || {
            database
                .create(CreateTaskInput {
                    title: "durable task despite telemetry failure".into(),
                    description: None,
                    priority: "p2".into(),
                    task_type: "feature".into(),
                    acceptance_criteria: "persist the task".into(),
                    created_by: Some("test".into()),
                })
                .expect("task operation must not depend on telemetry persistence")
        });
        drop(guard);

        assert_eq!(
            database
                .get_by_id(task.id)
                .expect("read task")
                .unwrap()
                .title,
            task.title
        );
        assert!(health.snapshot().structured_write_failures > 0);
    }

    #[test]
    fn readable_writer_redacts_credentials_before_emitting_bytes() {
        let secret = "sk-abcdefghijklmnopqrstuvwxyz012345";
        let mut writer = RedactingWriter {
            inner: Vec::new(),
            pending: Vec::new(),
            overflowed: false,
        };
        writer
            .write_all(format!("provider error: {secret}\n").as_bytes())
            .expect("write readable log record");
        writer.flush().expect("flush readable log record");

        let output =
            String::from_utf8(std::mem::take(&mut writer.inner)).expect("redacted UTF-8 output");
        assert!(!output.contains(secret));
        assert!(output.contains("REDACTED"));
    }

    #[test]
    fn readable_writer_redacts_credentials_split_across_writes_and_flushes() {
        let secret = "sk-abcdefghijklmnopqrstuvwxyz012345";
        let mut writer = RedactingWriter {
            inner: Vec::new(),
            pending: Vec::new(),
            overflowed: false,
        };
        writer
            .write_all(b"provider error: sk-")
            .expect("write credential prefix");
        writer
            .flush()
            .expect("flush without emitting a partial record");
        assert!(
            writer.inner.is_empty(),
            "an incomplete secret must stay buffered"
        );
        writer
            .write_all(b"abcdefghijklmnopqrstuvwxyz012345\n")
            .expect("write credential suffix");
        writer.flush().expect("flush complete record");

        let output =
            String::from_utf8(std::mem::take(&mut writer.inner)).expect("redacted UTF-8 output");
        assert!(!output.contains(secret));
        assert!(output.contains("REDACTED_KEY"));
    }

    #[test]
    fn readable_writer_omits_records_that_exceed_the_redaction_buffer_bound() {
        let mut writer = RedactingWriter {
            inner: Vec::new(),
            pending: Vec::new(),
            overflowed: false,
        };
        let oversized = vec![b'x'; MAX_READABLE_RECORD_BYTES + 1];
        writer
            .write_all(&oversized)
            .expect("consume oversized record without emitting it");
        writer.write_all(b"\n").expect("finish oversized record");
        let output =
            String::from_utf8(std::mem::take(&mut writer.inner)).expect("ASCII omission marker");
        assert!(output.contains("readable log record omitted"));
        assert!(!output.contains("xxxxx"));
    }

    #[test]
    fn structured_writer_preserves_logs_reader_jsonl_contract_and_redacts_secrets() {
        let dir = tempfile::tempdir().expect("temporary log directory");
        let structured_path = dir.path().join("events.jsonl");
        let file = std::fs::File::create(&structured_path).expect("create JSONL fixture");
        let (writer, guard) = tracing_appender::non_blocking(RedactingJsonWriter {
            inner: file,
            pending: Vec::new(),
        });
        let subscriber = Registry::default().with(structured_telemetry_layer(writer));
        let secret = "sk-abcdefghijklmnopqrstuvwxyz012345";

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "arcana::telemetry", event_type = "writer_contract_test", run_id = "run-fixture", api_key = secret, "safe summary");
            tracing::info!(target: "arcana::ordinary", api_key = secret, "ordinary event is excluded from JSONL");
        });
        drop(guard);

        let jsonl = std::fs::read_to_string(structured_path).expect("read persisted JSONL");
        let records = jsonl
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSONL record"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1, "only typed telemetry belongs in JSONL");
        assert_eq!(records[0]["target"], "arcana::telemetry");
        assert_eq!(records[0]["fields"]["event_type"], "writer_contract_test");
        assert_eq!(records[0]["fields"]["run_id"], "run-fixture");
        assert!(
            !jsonl.contains(secret),
            "writer boundary must redact credentials"
        );
        assert!(records[0]["fields"]["api_key"]
            .as_str()
            .unwrap_or_default()
            .contains("REDACTED"));
    }

    #[test]
    fn structured_writer_preserves_registered_long_labels_without_exempting_secrets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let file = std::fs::File::create(&path).unwrap();
        let (writer, guard) = tracing_appender::non_blocking(RedactingJsonWriter {
            inner: file,
            pending: Vec::new(),
        });
        let subscriber = Registry::default().with(structured_telemetry_layer(writer));
        let secret = "unknown_untrusted_operation_label_12345";
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!(target: "arcana::telemetry", "database_operation", operation = "database_schema_migration_ladder", api_key = "database_schema_migration_ladder");
            let _entered = span.enter();
            tracing::info!(target: "arcana::telemetry", event_type = "operation_timing", operation = "scheduler_recoverable_work_check", api_key = "scheduler_recoverable_work_check", "safe summary");
            tracing::info!(target: "arcana::telemetry", event_type = "database_migration_backup_created", operation = "database_schema_migration_ladder", "safe summary");
            tracing::info!(target: "arcana::telemetry", event_type = secret, operation = secret, "safe summary");
        });
        drop(guard);
        let contents = std::fs::read_to_string(&path).unwrap();
        let records: Vec<serde_json::Value> = contents
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 3);
        assert_eq!(
            records[0]["fields"]["operation"],
            "scheduler_recoverable_work_check"
        );
        assert_eq!(records[0]["fields"]["api_key"], "[REDACTED]");
        assert_eq!(
            records[0]["span"]["operation"],
            "database_schema_migration_ladder"
        );
        assert_eq!(
            records[0]["spans"][0]["operation"],
            "database_schema_migration_ladder"
        );
        assert_eq!(records[0]["span"]["api_key"], "[REDACTED]");
        assert_eq!(
            records[1]["fields"]["event_type"],
            "database_migration_backup_created"
        );
        assert_eq!(
            records[1]["fields"]["operation"],
            "database_schema_migration_ladder"
        );
        assert!(!contents.contains(secret));
        let foreign = serde_json::json!({"target":"foreign", "fields":{"operation":"scheduler_recoverable_work_check"}});
        assert_eq!(
            redact_telemetry_record(&foreign)["fields"]["operation"],
            "[REDACTED_KEY]"
        );
    }

    #[test]
    fn structured_writer_derives_call_identity_without_exposing_opaque_values() {
        let raw = "opaque-session-sk-abcdefghijklmnopqrstuvwxyz012345-tool-call";
        let expected = telemetry_call_id(raw);
        assert!(is_canonical_uuid(&expected));
        assert_eq!(telemetry_call_id(raw), expected);
        assert_ne!(telemetry_call_id(&format!("{raw}-other")), expected);
        let canonical = uuid::Uuid::new_v4().to_string();
        assert_eq!(telemetry_call_id(&canonical), canonical);
        let mut writer = RedactingJsonWriter {
            inner: Vec::new(),
            pending: Vec::new(),
        };
        let source = serde_json::json!({
            "target": "arcana::telemetry",
            "fields": {"call_id":raw, "tool_call_id":raw, "unknown_call_id":raw, "api_key":raw},
            "span": {"call_id":raw},
            "spans": [{"tool_call_id":raw}]
        });
        writer.write_all(format!("{source}\n").as_bytes()).unwrap();
        writer.flush().unwrap();
        let bytes = String::from_utf8(writer.inner).unwrap();
        assert!(!bytes.contains(raw), "opaque identity must not be exposed");
        let result: serde_json::Value = serde_json::from_str(bytes.trim()).unwrap();
        assert_eq!(result["fields"]["call_id"], expected);
        assert_eq!(result["fields"]["tool_call_id"], expected);
        assert_eq!(result["span"]["call_id"], expected);
        assert_eq!(result["spans"][0]["tool_call_id"], expected);
        assert_ne!(result["fields"]["unknown_call_id"], expected);
        assert_eq!(result["fields"]["api_key"], "[REDACTED]");
        let foreign = serde_json::json!({"target":"foreign::telemetry", "fields":{"call_id":raw}});
        assert_ne!(
            redact_telemetry_record(&foreign)["fields"]["call_id"],
            expected
        );
    }

    #[test]
    fn structured_writer_preserves_only_canonical_correlation_uuids_for_arcana_telemetry() {
        fn write_through_redactor(value: &serde_json::Value) -> serde_json::Value {
            let mut writer = RedactingJsonWriter {
                inner: Vec::new(),
                pending: Vec::new(),
            };
            writer
                .write_all(format!("{}\n", serde_json::to_string(value).unwrap()).as_bytes())
                .expect("write structured record");
            writer.flush().expect("flush structured record");
            let output =
                String::from_utf8(std::mem::take(&mut writer.inner)).expect("redacted JSON output");
            serde_json::from_str(output.trim()).expect("redacted JSON record")
        }
        fn assert_redacted(value: &serde_json::Value, context: &str) {
            let value = value.as_str().expect("redacted string value");
            assert!(
                value.contains("REDACTED") || value.contains("[CARD]"),
                "{context} should retain the normal redaction marker, got {value:?}"
            );
        }

        let uuid = |digit: char| {
            format!("{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}-{digit}{digit}{digit}{digit}-{digit}{digit}{digit}{digit}-{digit}{digit}{digit}{digit}-{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}{digit}")
        };
        let values = [
            uuid('a'),
            uuid('b'),
            uuid('c'),
            uuid('d'),
            uuid('e'),
            uuid('f'),
            uuid('1'),
        ];
        let source = serde_json::json!({
            "target": "arcana::telemetry::test",
            "fields": {
                "correlation_id": values[0],
                "fleet_correlation_id": values[1],
                "trace_id": values[2],
                "run_id": values[3],
                "session_id": values[4],
                "call_id": values[5],
                "tool_call_id": values[6],
                "unknown_parent_run_id": uuid('2'),
                "api_key": "sk-abcdefghijklmnopqrstuvwxyz012345"
            },
            "span": {"name": "worker_run", "run_id": uuid('3')},
            "spans": [{"name": "worker_run", "session_id": uuid('4')}]
        });

        let output = write_through_redactor(&source);
        for (field, expected) in [
            "correlation_id",
            "fleet_correlation_id",
            "trace_id",
            "run_id",
            "session_id",
            "call_id",
            "tool_call_id",
        ]
        .into_iter()
        .zip(values.iter())
        {
            assert_eq!(
                output["fields"][field].as_str(),
                Some(expected.as_str()),
                "field {field}"
            );
        }
        assert_eq!(output["span"]["run_id"], uuid('3'));
        assert_eq!(output["spans"][0]["session_id"], uuid('4'));
        assert_redacted(
            &output["fields"]["unknown_parent_run_id"],
            "unknown correlation field",
        );
        assert_eq!(output["fields"]["api_key"], "[REDACTED]");

        let malformed = serde_json::json!({
            "target": "arcana::telemetry",
            "fields": {"run_id": uuid('a').to_uppercase()}
        });
        assert_redacted(
            &write_through_redactor(&malformed)["fields"]["run_id"],
            "noncanonical UUID text",
        );

        let sensitive = serde_json::json!({
            "target": "arcana::telemetry",
            "fields": {"run_id": "sk-abcdefghijklmnopqrstuvwxyz012345"}
        });
        assert_eq!(
            write_through_redactor(&sensitive)["fields"]["run_id"],
            "[REDACTED_KEY]",
            "sensitive values must stay redacted even in correlation fields"
        );

        let foreign = serde_json::json!({
            "target": "foreign::telemetry",
            "fields": {"run_id": uuid('a')}
        });
        assert_redacted(
            &write_through_redactor(&foreign)["fields"]["run_id"],
            "foreign telemetry target UUID",
        );
    }

    #[test]
    fn default_filter_keeps_fast_telemetry_in_jsonl_without_debug_readable_logs() {
        let dir = tempfile::tempdir().expect("temporary log directory");
        let structured_path = dir.path().join("events.jsonl");
        let file = std::fs::File::create(&structured_path).expect("create JSONL fixture");
        let (writer, guard) = tracing_appender::non_blocking(RedactingJsonWriter {
            inner: file,
            pending: Vec::new(),
        });
        let base_filter = EnvFilter::new("arcana_lib=debug,info");
        let structured_filter = base_filter
            .clone()
            .add_directive("arcana::telemetry=debug".parse().expect("filter directive"));
        let subscriber = Registry::default()
            .with(structured_telemetry_layer(writer).with_filter(structured_filter));

        tracing::subscriber::with_default(subscriber, || {
            tracing::debug!(target: "arcana::telemetry", event_type = "database_statement", duration_ns = 1_000, "fast query");
            tracing::debug!(target: "arcana::ordinary", "ordinary debug should not be structured");
        });
        drop(guard);

        let jsonl = std::fs::read_to_string(structured_path).expect("read JSONL fixture");
        let records = jsonl
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSONL record"))
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["fields"]["event_type"], "database_statement");
        assert_eq!(records[0]["fields"]["duration_ns"], 1_000);
    }

    #[test]
    fn rolling_appender_uses_daily_logs_reader_names() {
        let dir = tempfile::tempdir().expect("temporary log directory");
        let mut readable = rolling_appender(dir.path(), "arcana", "log");
        let mut structured = rolling_appender(dir.path(), "arcana-events", "jsonl");
        readable
            .write_all(b"readable\n")
            .expect("write readable record");
        structured
            .write_all(b"{}\n")
            .expect("write structured record");
        let today = chrono::Utc::now().format("%Y-%m-%d");

        assert!(dir.path().join(format!("arcana.{today}.log")).exists());
        assert!(dir
            .path()
            .join(format!("arcana-events.{today}.jsonl"))
            .exists());
    }
}
