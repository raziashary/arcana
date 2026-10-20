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
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock,
    },
};

use tracing::Subscriber;
use tracing_appender::non_blocking::{ErrorCounter, WorkerGuard};
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

/// Process-local health for Arcana's asynchronous log writers.
/// The logging worker remains isolated from application operations; the Logs
/// panel reads this record through the host instead of relying on that same
/// writer to report its own failure.
///
/// Write failures are counters. Missing sinks are a state: when the home log
/// directory cannot be prepared, no writer exists to count anything, so without
/// `file_logging_unavailable` an empty Logs view would be indistinguishable from
/// a process that simply had nothing to say. Queue drops are a third kind: the
/// lossy `NonBlocking` writer discards a fragment at capacity and reports
/// success, so only its own counter can reveal that telemetry went missing.
#[derive(Clone, Default)]
pub struct LoggingHealth {
    readable_write_failures: Arc<AtomicU64>,
    structured_write_failures: Arc<AtomicU64>,
    file_logging_unavailable: Arc<AtomicBool>,
    readable_dropped: Arc<OnceLock<ErrorCounter>>,
    structured_dropped: Arc<OnceLock<ErrorCounter>>,
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct LoggingHealthSnapshot {
    pub readable_write_failures: u64,
    pub structured_write_failures: u64,
    /// True when this process could not prepare its file log directory, so
    /// every file stream is absent and output is console-only.
    pub file_logging_unavailable: bool,
    /// Write fragments discarded by the lossy readable queue. These never reach
    /// the writer, so they cannot appear in `readable_write_failures`.
    pub readable_dropped_lines: u64,
    /// Write fragments discarded by the lossy structured queue.
    pub structured_dropped_lines: u64,
}

static PROCESS_HEALTH: OnceLock<LoggingHealth> = OnceLock::new();

/// Read the process-wide writer health used by the desktop Logs metrics.
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
            file_logging_unavailable: self.file_logging_unavailable.load(Ordering::Relaxed),
            readable_dropped_lines: dropped_lines(&self.readable_dropped),
            structured_dropped_lines: dropped_lines(&self.structured_dropped),
        }
    }

    /// Adopt the queues' own drop counters. Called once, at subscriber
    /// installation, because the counters live on the `NonBlocking` handles the
    /// layers own.
    fn install_drop_counters(&self, readable: ErrorCounter, structured: ErrorCounter) {
        let _ = self.readable_dropped.set(readable);
        let _ = self.structured_dropped.set(structured);
    }

    fn mark_file_logging_unavailable(&self) {
        self.file_logging_unavailable.store(true, Ordering::Relaxed);
    }
}

fn dropped_lines(counter: &OnceLock<ErrorCounter>) -> u64 {
    counter
        .get()
        .map(|counter| counter.dropped_lines() as u64)
        .unwrap_or(0)
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
            health: console_only_health(),
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
    let readable_dropped = readable_writer.error_counter();
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
    health.install_drop_counters(readable_dropped, structured_writer.error_counter());
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

/// The console-only fallback's health record.
///
/// Taken from the process-wide slot so the Logs metrics reader sees the same
/// value, and marked unavailable because no file writer will exist to count
/// failures or to prove that an empty view means "nothing happened".
fn console_only_health() -> LoggingHealth {
    let health = PROCESS_HEALTH.get_or_init(LoggingHealth::default).clone();
    health.mark_file_logging_unavailable();
    health
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

    /// TG4-24 telemetry-failure injection, reader half: the injected structured
    /// write failure is counted on the **process-global** health handle the
    /// production initialiser wires into its writer (`PROCESS_HEALTH`, see
    /// `init_with_filter` → `ObservedWriter { failures: Arc::clone(...) }`), and
    /// the loss is read back through `process_health_snapshot()` — the exact
    /// function the Tauri Logs summariser calls. A real production
    /// `TasksDb::create` still returns its true success result and the row
    /// survives reopening the database file, so a lost telemetry record cannot
    /// be confused with a lost task.
    ///
    /// The existing `telemetry_writer_failure_is_counted_without_losing_database_work`
    /// proves the writer-side counter on a *locally* constructed `LoggingHealth`;
    /// this test proves the reader the Logs UI consumes observes the same handle
    /// the shipped initialiser installs.
    #[test]
    fn process_health_reports_injected_structured_write_failure_and_task_result_survives() {
        use crate::db::tasks::{CreateTaskInput, TasksDb};
        use crate::db::test_support::with_home;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = with_home(home.path());

        let baseline = process_health_snapshot().structured_write_failures;

        // Open through the shipped home-based entry point; the row's durability
        // below is therefore a property of the production access path.
        let database = TasksDb::open().expect("open isolated tasks database");
        let database_path = home.path().join(".arcana").join("tasks.db");

        // Mirror the production structured writer stack exactly, but against
        // the process-global health handle instead of a private clone.
        let health = PROCESS_HEALTH.get_or_init(LoggingHealth::default).clone();
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
                    title: "durable task despite process-wide telemetry failure".into(),
                    description: Some("private-description-canary".into()),
                    priority: "p2".into(),
                    task_type: "feature".into(),
                    acceptance_criteria: "the task result survives telemetry loss".into(),
                    created_by: Some("tg4-24-telemetry-failure".into()),
                })
                .expect("a telemetry write failure must not fail the task operation")
        });
        drop(guard);

        // (i) The operation's own result is the true success result.
        assert!(task.id > 0);
        assert_eq!(task.status.as_str(), "ready");
        assert_eq!(
            task.title,
            "durable task despite process-wide telemetry failure"
        );

        // (iv) The authoritative row is persisted: reopen the database file and
        // read it back without the original connection.
        drop(database);
        let reopened = TasksDb::open_at(&database_path).expect("reopen the tasks database");
        let persisted = reopened
            .get_by_id(task.id)
            .expect("read the task back after reopen")
            .expect("the telemetry failure must not have prevented the insert");
        assert_eq!(persisted.title, task.title);
        assert_eq!(persisted.status, task.status);
        assert_eq!(persisted.id, task.id);

        // (ii)+(iii) The telemetry loss is surfaced through the process-global
        // health snapshot the Logs reader consumes; nothing above panicked.
        let after = process_health_snapshot().structured_write_failures;
        assert!(
            after > baseline,
            "process health must expose the injected structured telemetry failure \
             (baseline {baseline}, after {after})"
        );
    }

    /// TG4-24 telemetry-failure injection, no-sink half. When `~/.arcana/logs`
    /// cannot be prepared as a directory, `resolve_log_dir` returns `None` and
    /// `init_with_filter` takes its console-only early return, which registers
    /// **no** writer and therefore no failure counter. Core task execution and
    /// its result are unaffected and the row stays durable; the missing sink is
    /// reported as process health (see
    /// `console_only_logging_reports_an_unavailable_file_sink_to_the_logs_reader`)
    /// rather than as a structured event, because there is no structured sink to
    /// carry one.
    #[test]
    fn unpreparable_log_directory_degrades_to_console_only_without_blocking_core_work() {
        use crate::db::tasks::{CreateTaskInput, TasksDb};
        use crate::db::test_support::with_home;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = with_home(home.path());

        // A file where the log directory must be is the Windows-safe,
        // in-process way to make directory preparation fail.
        std::fs::create_dir_all(home.path().join(".arcana")).expect("create arcana directory");
        std::fs::write(home.path().join(".arcana").join("logs"), "not a directory")
            .expect("occupy the log directory path with a regular file");

        assert!(
            resolve_log_dir().is_none(),
            "an unpreparable log directory must fall back to console-only logging"
        );

        // Core work proceeds through the shipped home-based entry point and
        // keeps its real result even though no telemetry sink can be installed.
        let database = TasksDb::open().expect("core task database still opens");
        let task = database
            .create(CreateTaskInput {
                title: "durable task without any telemetry sink".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "the task result must not depend on the sink".into(),
                created_by: Some("tg4-24-telemetry-no-sink".into()),
            })
            .expect("task execution must not depend on telemetry persistence");
        assert_eq!(task.status.as_str(), "ready");
        let readback = database
            .get_by_id(task.id)
            .expect("read the task back")
            .expect("the row must be persisted");
        assert_eq!(readback.title, task.title);
    }

    /// TG4-24 no-sink visibility. The empty-view ambiguity was the defect: with
    /// no writer at all, nothing counted failures and nothing recorded that the
    /// sink was missing, so the Logs panel could not tell "this process dropped
    /// its telemetry" from "this process did nothing worth logging".
    #[test]
    fn console_only_logging_reports_an_unavailable_file_sink_to_the_logs_reader() {
        // `console_only_health` is the exact helper `init_with_filter`'s
        // console-only early return uses, so this exercises the production
        // wiring without installing a second process-wide subscriber.
        let health = console_only_health();
        assert!(
            health.snapshot().file_logging_unavailable,
            "the guards returned to the host must carry the missing-sink state"
        );
        assert!(
            process_health_snapshot().file_logging_unavailable,
            "the Logs metrics reader must be able to say 'no file sink' instead of \
             leaving an empty view to mean 'no activity'"
        );
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

    fn emit_producer_cost_event(index: usize, secret: Option<&str>) {
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "producer_cost_fixture",
            operation = "fixture_operation",
            session_id = "producer-cost-session",
            task_id = (index as u64 % 7) + 1,
            duration_ns = (index as u64 % 1_000_000) + 1,
            outcome = "success",
            api_key = secret.unwrap_or(""),
            "producer cost fixture"
        );
    }

    /// Wait for the asynchronous writer to drain. `WorkerGuard::drop` waits only
    /// a bounded time, so reading immediately after it can observe a partial
    /// file and be mistaken for telemetry loss.
    fn wait_for_jsonl_lines(path: &std::path::Path, expected: usize) -> usize {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            let count = std::fs::read_to_string(path)
                .map(|jsonl| jsonl.lines().filter(|line| !line.is_empty()).count())
                .unwrap_or(0);
            if count >= expected || std::time::Instant::now() >= deadline {
                return count;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    /// TG4-24 producer-cost measurement, and the large-volume completeness check
    /// in the same fixture. The gate asks for producer, database instrumentation,
    /// refresh and export cost on reproducible fixtures; the other three are
    /// measured by `sqlite::measure_profile_callback_overhead_and_duration_resolution`
    /// and `log_commands::measure_metrics_refresh_tail_and_full_export_on_reproducible_jsonl`.
    /// This one measures what a *producer* pays on its own thread to emit through
    /// the shipped stack, against the same formatting into a discarded sink, and
    /// confirms a 20k-event burst loses nothing.
    #[test]
    fn measure_structured_producer_cost_on_a_reproducible_fixture() {
        use std::io::BufWriter;
        use std::time::Instant;

        const EVENTS: usize = 20_000;
        const SECRET: &str = "sk-abcdefghijklmnopqrstuvwxyz012345";

        // Baseline: identical JSON formatting, discarded. No redaction buffer,
        // no channel, no file.
        let sink_subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::DEBUG)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(std::io::sink)
            .finish();
        let sink_started = Instant::now();
        tracing::subscriber::with_default(sink_subscriber, || {
            for index in 0..EVENTS {
                emit_producer_cost_event(index, None);
            }
        });
        let sink_elapsed = sink_started.elapsed();

        // Shipped stack, shipped constructor: RedactingJsonWriter ->
        // ObservedWriter -> non_blocking -> buffered file. The rolling appender
        // buffers per file, so the fixture does too; otherwise 20k tiny writes
        // would measure syscalls.
        let dir = tempfile::tempdir().expect("temporary log directory");
        let path = dir.path().join("producer-cost.jsonl");
        let file = std::fs::File::create(&path).expect("create producer fixture");
        let health = LoggingHealth::default();
        let (writer, guard) = tracing_appender::non_blocking(RedactingJsonWriter {
            inner: ObservedWriter {
                inner: BufWriter::new(file),
                failures: Arc::clone(&health.structured_write_failures),
            },
            pending: Vec::new(),
        });
        let dropped = writer.error_counter();
        let subscriber = Registry::default().with(structured_telemetry_layer(writer));
        let producer_started = Instant::now();
        tracing::subscriber::with_default(subscriber, || {
            for index in 0..EVENTS {
                // Every hundredth event carries a planted credential, so the
                // measured path includes the redaction work it really does.
                let secret = (index % 100 == 0).then_some(SECRET);
                emit_producer_cost_event(index, secret);
            }
        });
        let producer_elapsed = producer_started.elapsed();
        let drain_started = Instant::now();
        drop(guard);
        let line_count = wait_for_jsonl_lines(&path, EVENTS);
        let drain_elapsed = drain_started.elapsed();

        let jsonl = std::fs::read_to_string(&path).expect("read producer fixture");
        let records = jsonl
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSONL record"))
            .collect::<Vec<_>>();
        assert_eq!(
            line_count, EVENTS,
            "the structured writer must not drop events under a 20k burst"
        );
        assert_eq!(records.len(), EVENTS);
        assert!(records
            .iter()
            .all(|record| record["fields"]["event_type"] == "producer_cost_fixture"));
        assert!(
            !jsonl.contains(SECRET),
            "the writer boundary must redact planted credentials"
        );
        assert_eq!(
            health.snapshot().structured_write_failures,
            0,
            "no write failure may be counted on a healthy fixture"
        );
        assert_eq!(
            dropped.dropped_lines(),
            0,
            "a 20k burst must fit the shipped queue; a non-zero drop count here \
             means the queue was exhausted, not that formatting got slower"
        );

        let producer_ns_per_event = producer_elapsed.as_nanos() / EVENTS as u128;
        let sink_ns_per_event = sink_elapsed.as_nanos() / EVENTS as u128;
        // A synchronous per-event file flush is the regression this catches; the
        // cap leaves more than an order of magnitude of headroom over formatting.
        assert!(
            producer_ns_per_event < 250_000,
            "producer cost regressed to {producer_ns_per_event} ns/event \
             (sink baseline {sink_ns_per_event} ns/event)"
        );
        eprintln!(
            "structured_producer_cost events={EVENTS} producer_total_ns={} producer_ns_per_event={producer_ns_per_event} sink_total_ns={} sink_ns_per_event={sink_ns_per_event} drain_ns={} persisted_bytes={} dropped_lines={}",
            producer_elapsed.as_nanos(),
            sink_elapsed.as_nanos(),
            drain_elapsed.as_nanos(),
            jsonl.len(),
            dropped.dropped_lines(),
        );
    }

    /// TG4-24 loss visibility. `tracing_appender`'s lossy queue discards a write
    /// fragment at capacity and still reports success, so nothing reaches
    /// `ObservedWriter` and no write failure is counted. The crate does expose
    /// the drop count; without wiring it into `LoggingHealth` a reader could not
    /// tell a complete burst from a truncated one.
    #[test]
    fn dropped_telemetry_is_reported_through_the_health_record() {
        use std::io::Write as _;
        use std::sync::mpsc;

        struct SlowWriter {
            /// Released on a separate thread so the worker stays busy long
            /// enough for the one-slot queue to overflow.
            _gate: mpsc::Receiver<()>,
        }
        impl Write for SlowWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                std::thread::sleep(std::time::Duration::from_millis(50));
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let (_release, gate) = mpsc::channel();
        let (mut writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
            .buffered_lines_limit(1)
            .finish(SlowWriter { _gate: gate });
        // Enqueue far more fragments than the one-slot queue plus one in-flight
        // worker can hold; the rest are dropped by the lossy writer.
        for index in 0..64 {
            writer
                .write_all(format!("fragment {index}\n").as_bytes())
                .expect("lossy writes report success");
        }
        let dropped = writer.error_counter().dropped_lines();
        assert!(
            dropped > 0,
            "the fixture must overflow the queue before this test can prove anything"
        );

        let health = LoggingHealth::default();
        health.install_drop_counters(
            tracing_appender::non_blocking::NonBlockingBuilder::default()
                .buffered_lines_limit(1)
                .finish(std::io::sink())
                .0
                .error_counter(),
            writer.error_counter(),
        );
        let snapshot = health.snapshot();
        assert_eq!(snapshot.structured_dropped_lines as usize, dropped);
        assert_eq!(
            snapshot.structured_write_failures, 0,
            "a dropped fragment is not a write failure; conflating them would hide \
             one behind the other"
        );
        drop(guard);
        let _ = writer; // keep the channel open until the assertions are done
    }
}
