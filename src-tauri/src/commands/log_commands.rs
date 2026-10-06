// Log reading commands — exposes ~/.arcana/logs/ to the frontend Logs tab.
// Provides listing of available log files (by date) and content reading.
// Log files are plain-text, one tracing event per line.
//
// Tail truncation: `read_log_file` returns only the last N lines by default
// (10 000) so the frontend stays responsive when a daily log grows large
// during heavy fleet activity. The full file is still readable via
// `read_log_file_full` for download / export. See Logs.tsx for the
// truncation banner UX.

use crate::errors::ArcanaError;
use crate::errors::Result;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

#[derive(Clone, Copy)]
struct LineCountState {
    observed_len: u64,
    newline_count: u64,
    last_byte: Option<u8>,
}

static LINE_COUNTS: OnceLock<Mutex<HashMap<std::path::PathBuf, LineCountState>>> = OnceLock::new();

// ─── Return type ──────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct LogFileResult {
    /// Visible portion of the log (last N lines, joined with `\n`).
    pub content: String,
    /// Total line count in the physical file.
    pub total_lines: usize,
    /// True when `total_lines` exceeds the requested tail limit.
    pub truncated: bool,
    /// Byte size of the full physical file on disk.
    pub full_size_bytes: u64,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct EventAggregate {
    pub count: u64,
    pub failures: u64,
    pub result_count_total: u64,
    pub result_bytes_total: u64,
    pub duration_count: u64,
    pub duration_zero_count: u64,
    pub duration_total_ns: u128,
    pub duration_max_ns: u64,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ObservabilityMetrics {
    pub total_events: u64,
    pub malformed_events: u64,
    pub provider_calls: u64,
    pub provider_attempts: u64,
    pub tool_calls: u64,
    pub database_statements: u64,
    pub input_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub tool_tokens: u64,
    /// Process-lifetime write failures from the asynchronous file writers.
    /// These counters are host health, not scoped to the selected date/run.
    pub readable_writer_failures: u64,
    pub structured_writer_failures: u64,
    pub by_event: BTreeMap<String, EventAggregate>,
    pub outcomes: BTreeMap<String, u64>,
    pub by_provider_model: BTreeMap<String, ProviderModelAggregate>,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ProviderModelAggregate {
    pub calls: u64,
    pub failures: u64,
    pub duration_count: u64,
    pub duration_zero_count: u64,
    pub duration_total_ns: u128,
    pub duration_max_ns: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub result_blocks_total: u64,
}

#[derive(Clone, Default)]
struct MetricsAccumulator {
    observed_len: u64,
    pending: Vec<u8>,
    summary: ObservabilityMetrics,
    file_version: Option<MetricsFileVersion>,
    observed_modified: Option<SystemTime>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MetricsFileVersion {
    created: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl MetricsFileVersion {
    fn from_file(file: &std::fs::File, metadata: &std::fs::Metadata) -> std::io::Result<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        #[cfg(windows)]
        let (volume_serial, file_index) =
            arcana_core::platform::file_identity::windows_file_identity(file)?;

        Ok(Self {
            created: metadata.created().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(windows)]
            volume_serial,
            #[cfg(windows)]
            file_index,
        })
    }
}

const METRICS_READ_CHUNK_BYTES: usize = 16 * 1024;

impl MetricsAccumulator {
    fn ingest<R: Read>(
        &mut self,
        mut reader: R,
        correlation_id: Option<&str>,
    ) -> std::io::Result<()> {
        let mut chunk = [0_u8; METRICS_READ_CHUNK_BYTES];
        loop {
            let read = reader.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            self.pending.extend_from_slice(&chunk[..read]);
            let mut consumed = 0;
            while let Some(relative_newline) = self.pending[consumed..]
                .iter()
                .position(|byte| *byte == b'\n')
            {
                let end = consumed + relative_newline + 1;
                let line = String::from_utf8_lossy(&self.pending[consumed..end]).into_owned();
                let delta = aggregate_event_lines_for_correlation(
                    BufReader::new(line.as_bytes()),
                    correlation_id,
                )?;
                self.summary.merge(delta);
                consumed = end;
            }
            if consumed > 0 {
                self.pending.drain(..consumed);
            }
        }
        Ok(())
    }
}

impl ObservabilityMetrics {
    fn merge(&mut self, other: Self) {
        self.total_events += other.total_events;
        self.malformed_events += other.malformed_events;
        self.provider_calls += other.provider_calls;
        self.provider_attempts += other.provider_attempts;
        self.tool_calls += other.tool_calls;
        self.database_statements += other.database_statements;
        self.input_tokens += other.input_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
        self.output_tokens += other.output_tokens;
        self.reasoning_tokens += other.reasoning_tokens;
        self.tool_tokens += other.tool_tokens;
        for (outcome, count) in other.outcomes {
            *self.outcomes.entry(outcome).or_default() += count;
        }
        for (provider_model, incoming) in other.by_provider_model {
            let current = self.by_provider_model.entry(provider_model).or_default();
            current.calls += incoming.calls;
            current.failures += incoming.failures;
            current.duration_count += incoming.duration_count;
            current.duration_zero_count += incoming.duration_zero_count;
            current.duration_total_ns += incoming.duration_total_ns;
            current.duration_max_ns = current.duration_max_ns.max(incoming.duration_max_ns);
            current.input_tokens += incoming.input_tokens;
            current.output_tokens += incoming.output_tokens;
            current.result_blocks_total += incoming.result_blocks_total;
        }
        for (kind, incoming) in other.by_event {
            let current = self.by_event.entry(kind).or_default();
            current.count += incoming.count;
            current.failures += incoming.failures;
            current.result_count_total += incoming.result_count_total;
            current.result_bytes_total += incoming.result_bytes_total;
            current.duration_count += incoming.duration_count;
            current.duration_zero_count += incoming.duration_zero_count;
            current.duration_total_ns += incoming.duration_total_ns;
            current.duration_max_ns = current.duration_max_ns.max(incoming.duration_max_ns);
        }
    }
}

static METRICS_CACHE: OnceLock<
    Mutex<HashMap<(std::path::PathBuf, Option<String>), MetricsAccumulator>>,
> = OnceLock::new();

#[derive(Default)]
struct CorrelationOffsetIndex {
    indexed_len: u64,
    pending_start: u64,
    pending: Vec<u8>,
    offsets: HashMap<String, Vec<(u64, u64)>>,
    file_version: Option<MetricsFileVersion>,
    observed_modified: Option<SystemTime>,
}

impl CorrelationOffsetIndex {
    fn ingest(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
        let mut consumed = 0usize;
        while let Some(relative_newline) = self.pending[consumed..]
            .iter()
            .position(|byte| *byte == b'\n')
        {
            let end = consumed + relative_newline + 1;
            let json_end = end - 1;
            let line = &self.pending[consumed..json_end];
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) {
                let record_start = self.pending_start + consumed as u64;
                let record_end = self.pending_start + end as u64;
                for correlation in correlation_values(&event) {
                    self.offsets
                        .entry(correlation)
                        .or_default()
                        .push((record_start, record_end));
                }
            }
            consumed = end;
        }
        if consumed > 0 {
            self.pending.drain(..consumed);
            self.pending_start += consumed as u64;
        }
    }
}

static CORRELATION_OFFSETS: OnceLock<Mutex<HashMap<std::path::PathBuf, CorrelationOffsetIndex>>> =
    OnceLock::new();

const CORRELATED_EVENT_LIMIT: usize = 2_000;

#[cfg(test)]
fn aggregate_event_lines(reader: impl BufRead) -> std::io::Result<ObservabilityMetrics> {
    aggregate_event_lines_for_correlation(reader, None)
}

fn event_matches_correlation(event: &serde_json::Value, correlation_id: &str) -> bool {
    const CORRELATION_FIELDS: &[&str] = &[
        "correlation_id",
        "fleet_correlation_id",
        "trace_id",
        "run_id",
        "worker_run_id",
        "task_id",
        "session_id",
        "call_id",
        "tool_call_id",
    ];
    let matches_fields = |fields: Option<&serde_json::Map<String, serde_json::Value>>| {
        fields.is_some_and(|fields| {
            CORRELATION_FIELDS.iter().any(|key| {
                fields.get(*key).is_some_and(|value| {
                    value
                        .as_str()
                        .is_some_and(|candidate| candidate == correlation_id)
                        || value
                            .as_i64()
                            .is_some_and(|candidate| candidate.to_string() == correlation_id)
                })
            })
        })
    };
    matches_fields(event.get("fields").and_then(serde_json::Value::as_object))
        || event
            .get("spans")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|spans| {
                spans.iter().any(|span| {
                    matches_fields(span.get("fields").and_then(serde_json::Value::as_object))
                        || matches_fields(span.as_object())
                })
            })
}

fn correlation_values(event: &serde_json::Value) -> Vec<String> {
    const CORRELATION_FIELDS: &[&str] = &[
        "correlation_id",
        "fleet_correlation_id",
        "trace_id",
        "run_id",
        "worker_run_id",
        "task_id",
        "session_id",
        "call_id",
        "tool_call_id",
    ];
    let mut values = std::collections::HashSet::new();
    let mut collect = |fields: Option<&serde_json::Map<String, serde_json::Value>>| {
        if let Some(fields) = fields {
            for key in CORRELATION_FIELDS {
                if let Some(value) = fields.get(*key) {
                    let value = value
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| value.as_i64().map(|number| number.to_string()));
                    if let Some(value) = value {
                        values.insert(value);
                    }
                }
            }
        }
    };
    collect(event.get("fields").and_then(serde_json::Value::as_object));
    if let Some(spans) = event.get("spans").and_then(serde_json::Value::as_array) {
        for span in spans {
            collect(span.get("fields").and_then(serde_json::Value::as_object));
            collect(span.as_object());
        }
    }
    values.into_iter().collect()
}

fn ensure_correlation_index(
    path: &std::path::Path,
    correlation_id: &str,
) -> std::io::Result<Vec<(u64, u64)>> {
    let cache = CORRELATION_OFFSETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut indexes = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    indexes.retain(|cached_path, _| cached_path == path || cached_path.exists());
    let mut file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let version = MetricsFileVersion::from_file(&file, &metadata)?;
    let modified = metadata.modified().ok();
    let size = metadata.len();
    let index = indexes.entry(path.to_path_buf()).or_default();
    let replacement_at_same_size = size == index.indexed_len
        && index
            .observed_modified
            .zip(modified)
            .is_some_and(|(old, current)| old != current);
    if index.file_version != Some(version) || size < index.indexed_len || replacement_at_same_size {
        *index = CorrelationOffsetIndex::default();
    }

    if size > index.indexed_len {
        file.seek(SeekFrom::Start(index.indexed_len))?;
        let remaining = size - index.indexed_len;
        let mut limited = file.take(remaining);
        let mut buffer = [0u8; METRICS_READ_CHUNK_BYTES];
        loop {
            let read = limited.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            index.ingest(&buffer[..read]);
            index.indexed_len += read as u64;
        }
    }
    index.file_version = Some(version);
    index.observed_modified = modified;
    Ok(index
        .offsets
        .get(correlation_id)
        .cloned()
        .unwrap_or_default())
}

fn aggregate_event_lines_for_correlation(
    reader: impl BufRead,
    correlation_id: Option<&str>,
) -> std::io::Result<ObservabilityMetrics> {
    let mut summary = ObservabilityMetrics::default();
    for line in reader.lines() {
        let line = line?;
        let event: serde_json::Value = match serde_json::from_str(&line) {
            Ok(event) => event,
            Err(_) => {
                if correlation_id.is_none() {
                    summary.malformed_events += 1;
                }
                continue;
            }
        };
        if correlation_id.is_some_and(|id| !event_matches_correlation(&event, id)) {
            continue;
        }
        summary.total_events += 1;
        let fields = event.get("fields").and_then(serde_json::Value::as_object);
        let event_type = fields
            .and_then(|fields| fields.get("event_type"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("untyped")
            .to_owned();
        let duration = fields
            .and_then(|fields| fields.get("duration_ns"))
            .and_then(serde_json::Value::as_u64);
        let outcome = fields
            .and_then(|fields| fields.get("outcome"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unspecified");
        *summary.outcomes.entry(outcome.to_owned()).or_default() += 1;
        let failed = matches!(
            outcome,
            "error"
                | "failure"
                | "failed"
                | "stopped"
                | "timeout"
                | "spawn_error"
                | "cancelled"
                | "degraded"
                | "fallback"
                | "partial"
                | "unavailable"
                | "permanent_failure"
                | "exhausted"
                | "connection_refused"
                | "invalid_response"
                | "http_status"
                | "request_error"
                | "denied"
        ) || fields
            .and_then(|fields| fields.get("is_error"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let aggregate = summary.by_event.entry(event_type.clone()).or_default();
        aggregate.count += 1;
        aggregate.failures += u64::from(failed);
        aggregate.result_count_total += fields
            .and_then(|fields| fields.get("result_count"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        aggregate.result_bytes_total += fields
            .and_then(|fields| {
                fields
                    .get("output_bytes")
                    .or_else(|| fields.get("result_bytes"))
            })
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        if let Some(duration) = duration {
            if duration == 0 {
                aggregate.duration_zero_count += 1;
            } else {
                aggregate.duration_count += 1;
                aggregate.duration_total_ns += u128::from(duration);
                aggregate.duration_max_ns = aggregate.duration_max_ns.max(duration);
            }
        }
        if event_type == "provider_call" {
            let provider = fields
                .and_then(|fields| fields.get("provider"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let model = fields
                .and_then(|fields| fields.get("model"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let key = format!("{provider}/{model}");
            let aggregate = summary.by_provider_model.entry(key).or_default();
            aggregate.calls += 1;
            aggregate.failures += u64::from(failed);
            if let Some(duration) = duration {
                if duration == 0 {
                    aggregate.duration_zero_count += 1;
                } else {
                    aggregate.duration_count += 1;
                    aggregate.duration_total_ns += u128::from(duration);
                    aggregate.duration_max_ns = aggregate.duration_max_ns.max(duration);
                }
            }
            aggregate.input_tokens += fields
                .and_then(|fields| fields.get("input_tokens"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            aggregate.output_tokens += fields
                .and_then(|fields| fields.get("output_tokens"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            aggregate.result_blocks_total += fields
                .and_then(|fields| fields.get("result_blocks"))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
        }
        match event_type.as_str() {
            "provider_call" => summary.provider_calls += 1,
            // `provider_attempt` records each physical adapter invocation.
            // `provider_retry` records the policy decision for that same
            // failed invocation and must not inflate the attempt count.
            "provider_attempt" => summary.provider_attempts += 1,
            "tool_call_result" => summary.tool_calls += 1,
            "database_statement" => summary.database_statements += 1,
            _ => {}
        }
        let add = |field: &str| {
            fields
                .and_then(|fields| fields.get(field))
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0)
        };
        summary.input_tokens += add("input_tokens");
        summary.cache_read_tokens += add("cache_read_tokens");
        summary.cache_write_tokens += add("cache_write_tokens");
        summary.output_tokens += add("output_tokens");
        summary.reasoning_tokens += add("reasoning_tokens");
        summary.tool_tokens += add("tool_tokens");
    }
    Ok(summary)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn logs_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(std::path::PathBuf::from(home).join(".arcana").join("logs"))
}

fn resolve_log_path(date: &str) -> Result<std::path::PathBuf> {
    let dir =
        logs_dir().ok_or_else(|| ArcanaError::Agent("Cannot determine home directory".into()))?;
    resolve_log_path_in(&dir, date)
}

fn resolve_log_path_in(dir: &std::path::Path, date: &str) -> Result<std::path::PathBuf> {
    let date_str = if date.is_empty() {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    } else {
        date.to_owned()
    };
    let parsed = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| ArcanaError::Agent("Log date must use YYYY-MM-DD format".into()))?;
    if parsed.format("%Y-%m-%d").to_string() != date_str {
        return Err(ArcanaError::Agent(
            "Log date must use YYYY-MM-DD format".into(),
        ));
    }
    let candidates = [
        dir.join(format!("arcana.{date_str}.log")),
        dir.join(format!("arcana-.{date_str}..log")),
        dir.join(format!("arcana-{date_str}.log")),
    ];
    Ok(candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone()))
}

fn resolve_structured_path(date: &str) -> Result<std::path::PathBuf> {
    let date = if date.is_empty() {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    } else {
        date.to_owned()
    };
    chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| ArcanaError::Agent("Log date must use YYYY-MM-DD format".into()))?;
    let dir =
        logs_dir().ok_or_else(|| ArcanaError::Agent("Cannot determine home directory".into()))?;
    let candidates = [
        dir.join(format!("arcana-events.{date}.jsonl")),
        dir.join(format!("arcana-events-.{date}..jsonl")),
        dir.join(format!("arcana-events-{date}.jsonl")),
    ];
    Ok(candidates
        .iter()
        .find(|path| path.exists())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone()))
}

fn read_full_log(path: &std::path::Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

fn ensure_regular_log_file(path: &std::path::Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| ArcanaError::Agent(format!("Cannot inspect log file: {error}")))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ArcanaError::Agent(
            "Log path is not a regular Arcana log file".into(),
        ));
    }
    Ok(())
}

/// Count lines incrementally: after the initial scan, each live refresh reads
/// only bytes appended since the last refresh. A size decrease resets the
/// cache, covering log replacement or rotation.
fn cached_line_count(path: &std::path::Path, size: u64) -> std::io::Result<usize> {
    let cache = LINE_COUNTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut states = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut state = states.get(path).copied().unwrap_or(LineCountState {
        observed_len: 0,
        newline_count: 0,
        last_byte: None,
    });

    if size < state.observed_len {
        state = LineCountState {
            observed_len: 0,
            newline_count: 0,
            last_byte: None,
        };
    }

    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::Start(state.observed_len))?;
    let remaining = size.saturating_sub(state.observed_len);
    let mut limited = file.take(remaining);
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = limited.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        state.newline_count += buffer[..read].iter().filter(|byte| **byte == b'\n').count() as u64;
        state.last_byte = Some(buffer[read - 1]);
        state.observed_len += read as u64;
    }

    states.insert(path.to_path_buf(), state);
    let trailing_unterminated = state.observed_len > 0 && state.last_byte != Some(b'\n');
    Ok((state.newline_count + u64::from(trailing_unterminated)) as usize)
}

/// Read only enough data from the end of the file to return the requested tail.
fn read_tail(path: &std::path::Path, limit: usize) -> std::io::Result<String> {
    if limit == 0 {
        return Ok(String::new());
    }
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size == 0 {
        return Ok(String::new());
    }

    let mut position = size;
    let mut chunks = Vec::new();
    let mut newline_count = 0usize;
    let mut trailing_newline = false;
    let mut buffer = vec![0u8; 16 * 1024];
    while position > 0 && newline_count <= limit {
        let read_size = position.min(buffer.len() as u64) as usize;
        position -= read_size as u64;
        file.seek(SeekFrom::Start(position))?;
        file.read_exact(&mut buffer[..read_size])?;
        let chunk = &buffer[..read_size];
        if chunks.is_empty() {
            trailing_newline = chunk.last() == Some(&b'\n');
        }
        newline_count += chunk.iter().filter(|byte| **byte == b'\n').count();
        chunks.push(chunk.to_vec());
    }

    let mut bytes = Vec::with_capacity((size - position) as usize);
    for chunk in chunks.into_iter().rev() {
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    if trailing_newline && lines.last() == Some(&"") {
        lines.pop();
    }
    Ok(lines
        .into_iter()
        .rev()
        .take(limit)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n"))
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// List available log files as date strings (e.g. ["2026-04-15", "2026-04-14"]).
/// Returns empty vec if no log directory exists yet.
#[tauri::command]
pub fn list_log_files() -> Result<Vec<String>> {
    let Some(dir) = logs_dir() else {
        return Ok(vec![]);
    };
    if !dir.exists() {
        return Ok(vec![]);
    }
    list_log_dates_from_directory(&dir).map_err(|e| ArcanaError::Agent(e.to_string()))
}

fn list_log_dates_from_directory(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut dates: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let is_regular_file = e
                .file_type()
                .map(|file_type| file_type.is_file() && !file_type.is_symlink())
                .unwrap_or(false);
            is_regular_file
                && matches!(
                    e.path().extension().and_then(|x| x.to_str()),
                    Some("log" | "jsonl")
                )
        })
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let structured = e.path().extension().and_then(|x| x.to_str()) == Some("jsonl");
            log_date_from_filename(&name, structured)
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Sort descending (newest first)
    dates.sort_by(|a, b| b.cmp(a));
    Ok(dates)
}

fn log_date_from_filename(name: &str, structured: bool) -> Option<String> {
    let patterns: &[(&str, &str)] = if structured {
        &[
            ("arcana-events.", ".jsonl"),
            ("arcana-events-.", "..jsonl"),
            ("arcana-events-", ".jsonl"),
        ]
    } else {
        &[
            ("arcana.", ".log"),
            ("arcana-.", "..log"),
            ("arcana-", ".log"),
        ]
    };
    patterns.iter().find_map(|(prefix, suffix)| {
        let date = name.strip_prefix(prefix)?.strip_suffix(suffix)?;
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .ok()
            .map(|date| date.format("%Y-%m-%d").to_string())
    })
}

fn readable_lines_for_correlation(contents: &str, correlation_id: &str) -> String {
    contents
        .lines()
        .filter(|line| line.contains(correlation_id))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Read the **tail** of a log file for a given date (e.g. "2026-04-15").
/// Pass an empty string to read today's log.
/// Only the last `tail_lines` (default 3 000) are returned over IPC so the
/// frontend stays responsive even when a single-day log grows large.
/// The full file can be fetched via `read_log_file_full` for download.
#[tauri::command]
pub fn read_log_file(date: String, tail_lines: Option<usize>) -> Result<LogFileResult> {
    let Some(_dir) = logs_dir() else {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    };

    let path = resolve_log_path(&date)?;
    if !path.exists() {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    }
    ensure_regular_log_file(&path)?;

    let limit = tail_lines.unwrap_or(3_000);
    let full_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    let total = cached_line_count(&path, full_size)
        .map_err(|e| ArcanaError::Agent(format!("Failed to count log lines: {e}")))?;
    let truncated = total > limit;
    let content = read_tail(&path, limit)
        .map_err(|e| ArcanaError::Agent(format!("Failed to read log tail: {e}")))?;

    Ok(LogFileResult {
        content,
        total_lines: total,
        truncated,
        full_size_bytes: full_size,
    })
}

/// Read the **entire** log file for a given date (e.g. "2026-04-15").
/// Pass an empty string for today's log.
/// Used by the "Download full log" button in Logs.tsx.
#[tauri::command]
pub fn read_log_file_full(date: String) -> Result<String> {
    let Some(dir) = logs_dir() else {
        return Ok(String::new());
    };

    read_log_file_full_from_dir(&dir, &date)
}

fn read_log_file_full_from_dir(dir: &std::path::Path, date: &str) -> Result<String> {
    let path = resolve_log_path_in(dir, date)?;
    if !path.exists() {
        return Ok(String::new());
    }
    ensure_regular_log_file(&path)?;

    read_full_log(&path).map_err(|e| ArcanaError::Agent(format!("Failed to read log file: {e}")))
}

/// Read the bounded tail of the structured telemetry file for a date.
#[tauri::command]
pub fn read_structured_events(
    date: String,
    tail_lines: Option<usize>,
    correlation_id: Option<String>,
) -> Result<LogFileResult> {
    let Some(_dir) = logs_dir() else {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    };
    let path = resolve_structured_path(&date)?;
    if !path.exists() {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    }
    ensure_regular_log_file(&path)?;
    let limit = tail_lines
        .unwrap_or(CORRELATED_EVENT_LIMIT)
        .min(CORRELATED_EVENT_LIMIT);
    let full_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let correlation_id = correlation_id.filter(|id| !id.trim().is_empty());
    let (content, total) = if let Some(correlation_id) = correlation_id.as_deref() {
        read_structured_events_for_correlation(&path, correlation_id, limit)?
    } else {
        let total = cached_line_count(&path, full_size)
            .map_err(|e| ArcanaError::Agent(format!("Failed to count structured events: {e}")))?;
        (
            read_tail(&path, limit).map_err(|e| {
                ArcanaError::Agent(format!("Failed to read structured events: {e}"))
            })?,
            total,
        )
    };
    Ok(LogFileResult {
        content,
        total_lines: total,
        truncated: total > limit,
        full_size_bytes: full_size,
    })
}

fn read_structured_events_for_correlation(
    path: &std::path::Path,
    correlation_id: &str,
    limit: usize,
) -> Result<(String, usize)> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(ArcanaError::Agent(
                "Log path is not a regular Arcana log file".into(),
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((String::new(), 0));
        }
        Err(error) => {
            return Err(ArcanaError::Agent(format!(
                "Cannot inspect log file: {error}"
            )));
        }
    }

    let matching_offsets = ensure_correlation_index(path, correlation_id).map_err(|error| {
        ArcanaError::Agent(format!("Failed to read structured events: {error}"))
    })?;
    let total = matching_offsets.len();
    let mut file = std::fs::File::open(path).map_err(|error| {
        ArcanaError::Agent(format!("Failed to read structured events: {error}"))
    })?;
    let mut matching = Vec::with_capacity(limit.min(total));
    for (start, end) in matching_offsets.iter().rev().take(limit).rev() {
        let length = end.saturating_sub(*start);
        let mut bytes = vec![0; length as usize];
        file.seek(SeekFrom::Start(*start))
            .and_then(|_| file.read_exact(&mut bytes))
            .map_err(|error| {
                ArcanaError::Agent(format!("Failed to read indexed structured event: {error}"))
            })?;
        let line_bytes = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
        let line_bytes = line_bytes.strip_suffix(b"\r").unwrap_or(line_bytes);
        let event: serde_json::Value = serde_json::from_slice(line_bytes).map_err(|error| {
            ArcanaError::Agent(format!(
                "Indexed structured event is no longer valid JSON: {error}"
            ))
        })?;
        if !event_matches_correlation(&event, correlation_id) {
            return Err(ArcanaError::Agent(
                "Structured log changed after its correlation index was built; refresh the Logs view to rebuild the index".into(),
            ));
        }
        matching.push(String::from_utf8_lossy(line_bytes).into_owned());
    }
    let content = matching.join("\n");
    Ok((
        if content.is_empty() {
            content
        } else {
            format!("{content}\n")
        },
        total,
    ))
}

/// Read the complete structured telemetry file for a date.
#[tauri::command]
pub fn read_structured_events_full(date: String) -> Result<String> {
    let Some(_dir) = logs_dir() else {
        return Ok(String::new());
    };
    let path = resolve_structured_path(&date)?;
    if !path.exists() {
        return Ok(String::new());
    }
    ensure_regular_log_file(&path)?;
    read_full_log(&path)
        .map_err(|e| ArcanaError::Agent(format!("Failed to read structured events: {e}")))
}

/// Aggregate the complete daily structured event file without sending it over
/// IPC. Malformed JSON lines are counted and skipped; event payloads are never
/// returned from this command.
#[tauri::command]
pub fn summarize_observability_events(
    date: String,
    correlation_id: Option<String>,
) -> Result<ObservabilityMetrics> {
    let path = resolve_structured_path(&date)?;
    let correlation_id = correlation_id.filter(|id| !id.trim().is_empty());
    let cache = METRICS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut entries = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if correlation_id.is_some() {
        entries.retain(|(cached_path, cached_correlation), _| {
            cached_path != &path
                || cached_correlation.is_none()
                || cached_correlation == &correlation_id
        });
    }
    let accumulator = entries
        .entry((path.clone(), correlation_id.clone()))
        .or_default();
    let mut metrics =
        refresh_metrics_from_file_for_correlation(&path, accumulator, correlation_id.as_deref())
            .map_err(|e| {
                ArcanaError::Agent(format!("Failed to aggregate structured events: {e}"))
            })?;
    let writer_health = arcana_core::observability::logging::process_health_snapshot();
    metrics.readable_writer_failures = writer_health.readable_write_failures;
    metrics.structured_writer_failures = writer_health.structured_write_failures;
    Ok(metrics)
}

#[cfg(test)]
fn refresh_metrics_from_file(
    path: &std::path::Path,
    accumulator: &mut MetricsAccumulator,
) -> std::io::Result<ObservabilityMetrics> {
    refresh_metrics_from_file_for_correlation(path, accumulator, None)
}

fn refresh_metrics_from_file_for_correlation(
    path: &std::path::Path,
    accumulator: &mut MetricsAccumulator,
    correlation_id: Option<&str>,
) -> std::io::Result<ObservabilityMetrics> {
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            *accumulator = MetricsAccumulator::default();
            return Ok(accumulator.summary.clone());
        }
        Err(error) => return Err(error),
    };
    let metadata = file.metadata()?;
    let size = metadata.len();
    let file_version = MetricsFileVersion::from_file(&file, &metadata)?;
    if size < accumulator.observed_len
        || accumulator
            .file_version
            .is_some_and(|known| known != file_version)
        || (size == accumulator.observed_len
            && accumulator.observed_len > 0
            && metadata.modified().ok() != accumulator.observed_modified)
    {
        *accumulator = MetricsAccumulator::default();
    }
    if size == accumulator.observed_len {
        return Ok(accumulator.summary.clone());
    }
    file.seek(SeekFrom::Start(accumulator.observed_len))?;
    let remaining = size.saturating_sub(accumulator.observed_len);
    let mut limited = file.take(remaining);
    let mut updated = accumulator.clone();
    updated.ingest(&mut limited, correlation_id)?;
    updated.observed_len = size;
    updated.file_version = Some(file_version);
    updated.observed_modified = metadata.modified().ok();
    *accumulator = updated;
    Ok(accumulator.summary.clone())
}

/// Delete all .log files in ~/.arcana/logs/.
/// Returns the number of files deleted.
#[tauri::command]
pub fn clear_log_files() -> Result<usize> {
    let Some(dir) = logs_dir() else {
        return Ok(0);
    };
    if !dir.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| ArcanaError::Agent(e.to_string()))?;
    let mut deleted = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if matches!(
            path.extension().and_then(|x| x.to_str()),
            Some("log" | "jsonl")
        ) {
            if std::fs::remove_file(&path).is_ok() {
                deleted += 1;
            }
        }
    }
    Ok(deleted)
}

/// Write a log file out to a user-chosen destination.
///
/// The write happens here rather than in the frontend deliberately. `Logs.tsx`
/// used `@tauri-apps/plugin-fs`'s `writeTextFile`, but the Rust half of that
/// plugin was never added to Cargo.toml, never registered in `main.rs`, and
/// never granted a capability — so the call threw at runtime while TypeScript
/// compiled cleanly and CI stayed green. The export simply did nothing when a
/// user clicked it, and the failure went to `console.error`.
///
/// Doing it in Rust also avoids granting the frontend a broad filesystem scope:
/// the caller supplies only a destination path, and the source is resolved from
/// the logs directory here.
///
/// `full = false` writes the tail that the UI is displaying; `full = true`
/// writes the entire file.
#[tauri::command]
pub fn export_log_file(
    date: String,
    dest_path: String,
    full: bool,
    tail_lines: Option<usize>,
) -> Result<String> {
    if full {
        return match logs_dir() {
            Some(dir) => export_full_log_file_from_dir(&dir, &date, &dest_path),
            None => write_log_export(&dest_path, ""),
        };
    }
    let contents = read_log_file(date, tail_lines)?.content;
    write_log_export(&dest_path, &contents)
}

fn export_full_log_file_from_dir(
    log_dir: &std::path::Path,
    date: &str,
    dest_path: &str,
) -> Result<String> {
    let contents = read_log_file_full_from_dir(log_dir, date)?;
    write_log_export(dest_path, &contents)
}

fn write_log_export(dest_path: &str, contents: &str) -> Result<String> {
    let dest = std::path::PathBuf::from(dest_path);
    if dest.as_os_str().is_empty() {
        return Err(ArcanaError::Agent("No destination path given".into()));
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(ArcanaError::Agent(format!(
                "Destination folder does not exist: {}",
                parent.display()
            )));
        }
    }

    std::fs::write(&dest, contents.as_bytes())
        .map_err(|e| ArcanaError::Agent(format!("Failed to write {}: {e}", dest.display())))?;

    Ok(dest.to_string_lossy().into_owned())
}

/// Export the complete readable log and structured telemetry for one date.
#[tauri::command]
pub fn export_observability_bundle(
    date: String,
    dest_path: String,
    correlation_id: Option<String>,
) -> Result<String> {
    let mut timer =
        arcana_core::observability::timing::OperationTimer::start("observability_bundle_export");
    let correlation_id = correlation_id.filter(|id| !id.trim().is_empty());
    let structured_events = if let Some(correlation_id) = correlation_id.as_deref() {
        read_structured_events_for_correlation(
            &resolve_structured_path(&date)?,
            correlation_id,
            usize::MAX,
        )?
        .0
    } else {
        read_structured_events_full(date.clone())?
    };
    let metrics = summarize_observability_events(date.clone(), correlation_id.clone())?;
    // Readable compact logs do not have a structured span model. When the
    // caller requests one run, include only lines that explicitly carry its
    // correlation ID instead of silently bundling the entire day's activity.
    let mut readable_logs = read_log_file_full(date.clone())?;
    if let Some(correlation_id) = correlation_id.as_deref() {
        readable_logs = readable_lines_for_correlation(&readable_logs, correlation_id);
    }
    let bundle = serde_json::json!({
        "date": if date.is_empty() { chrono::Local::now().format("%Y-%m-%d").to_string() } else { date.clone() },
        "correlation_id": correlation_id,
        "logs": readable_logs,
        "structured_events_jsonl": structured_events,
        "metrics": metrics,
    });
    let dest = std::path::PathBuf::from(&dest_path);
    if dest.as_os_str().is_empty() {
        return Err(ArcanaError::Agent("No destination path given".into()));
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(ArcanaError::Agent(format!(
                "Destination folder does not exist: {}",
                parent.display()
            )));
        }
    }
    let bytes = serde_json::to_vec_pretty(&bundle).map_err(|e| {
        ArcanaError::Agent(format!("Could not serialize observability export: {e}"))
    })?;
    std::fs::write(&dest, bytes)
        .map_err(|e| ArcanaError::Agent(format!("Failed to write {}: {e}", dest.display())))?;
    let bytes_written = std::fs::metadata(&dest)
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    timer = timer.with_result_bytes(usize::try_from(bytes_written).unwrap_or(usize::MAX));
    timer.succeed();
    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_event_lines, aggregate_event_lines_for_correlation, cached_line_count,
        ensure_correlation_index, event_matches_correlation, list_log_dates_from_directory,
        log_date_from_filename, read_full_log, read_structured_events_for_correlation, read_tail,
        refresh_metrics_from_file, refresh_metrics_from_file_for_correlation, resolve_log_path,
        resolve_structured_path, write_log_export, CorrelationOffsetIndex, MetricsAccumulator,
        CORRELATION_OFFSETS, METRICS_READ_CHUNK_BYTES,
    };
    use std::io::{BufReader, Cursor, Read, Write};

    #[test]
    fn log_export_failure_is_reported_without_creating_partial_output() {
        let dir = tempfile::tempdir().unwrap();
        let parent_file = dir.path().join("not-a-directory");
        std::fs::write(&parent_file, "preserve existing file").unwrap();
        let destination = parent_file.join("observability.jsonl");

        let error = write_log_export(destination.to_str().unwrap(), "complete export")
            .expect_err("export to a file used as a parent must fail");

        assert!(error.to_string().contains("Failed to write"));
        assert_eq!(
            std::fs::read_to_string(parent_file).unwrap(),
            "preserve existing file"
        );
        assert!(!destination.exists());
    }

    struct ChunkTrackingReader<R> {
        inner: R,
        max_requested: usize,
        calls: usize,
    }

    impl<R: Read> Read for ChunkTrackingReader<R> {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.max_requested = self.max_requested.max(buffer.len());
            self.calls += 1;
            self.inner.read(buffer)
        }
    }

    #[cfg(unix)]
    #[test]
    fn log_reader_refuses_external_symlink_files() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let external = tempfile::NamedTempFile::new().unwrap();
        let link = dir.path().join("arcana.2026-09-28.log");
        symlink(external.path(), &link).unwrap();
        assert!(super::ensure_regular_log_file(&link).is_err());
        assert!(read_structured_events_for_correlation(&link, "fleet-run", 10).is_err());
        assert!(external.path().exists());
    }

    #[test]
    fn filtered_reader_refuses_non_regular_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_structured_events_for_correlation(dir.path(), "fleet-run", 10).is_err());
    }

    #[test]
    fn daily_log_reader_recognizes_current_and_legacy_appender_names() {
        for name in [
            "arcana.2026-09-28.log",
            "arcana-.2026-09-28..log",
            "arcana-2026-09-28.log",
        ] {
            assert_eq!(
                log_date_from_filename(name, false).as_deref(),
                Some("2026-09-28"),
                "must find date in {name}"
            );
        }
        for name in [
            "arcana-events.2026-09-28.jsonl",
            "arcana-events-.2026-09-28..jsonl",
            "arcana-events-2026-09-28.jsonl",
        ] {
            assert_eq!(
                log_date_from_filename(name, true).as_deref(),
                Some("2026-09-28"),
                "must find date in {name}"
            );
        }
        assert_eq!(log_date_from_filename("arcana.backup.log", false), None);
    }

    #[test]
    fn date_discovery_includes_structured_only_log_days() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("arcana-events.2026-10-02.jsonl"), "{}\n").unwrap();
        std::fs::write(dir.path().join("arcana.2026-10-01.log"), "INFO ready\n").unwrap();
        std::fs::write(dir.path().join("unrelated.jsonl"), "{}\n").unwrap();

        assert_eq!(
            list_log_dates_from_directory(dir.path()).unwrap(),
            vec!["2026-10-02", "2026-10-01"]
        );
    }

    #[test]
    fn correlation_scoped_readable_export_omits_other_run_lines() {
        let lines = concat!(
            "INFO fleet tick correlation_id=fleet-a\n",
            "INFO unrelated task correlation_id=fleet-b\n",
            "WARN fleet retry correlation_id=fleet-a\n",
            "INFO correlation id absent\n"
        );

        let filtered = super::readable_lines_for_correlation(lines, "fleet-a");
        assert!(filtered.contains("fleet tick"));
        assert!(filtered.contains("fleet retry"));
        assert!(!filtered.contains("fleet-b"));
        assert!(!filtered.contains("id absent"));
    }

    #[test]
    fn tail_reader_returns_only_the_requested_lines_for_newline_variants() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sample.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(read_tail(&path, 2).unwrap(), "two\nthree");
        std::fs::write(&path, "one\ntwo\nthree").unwrap();
        assert_eq!(read_tail(&path, 2).unwrap(), "two\nthree");
        assert_eq!(read_tail(&path, 0).unwrap(), "");
    }

    #[test]
    fn full_log_reader_returns_lines_beyond_the_live_tail_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("full.log");
        let complete = (0..3_005)
            .map(|n| format!("line-{n}\n"))
            .collect::<String>();
        std::fs::write(&path, &complete).unwrap();
        assert_eq!(read_tail(&path, 3_000).unwrap().lines().count(), 3_000);
        assert_eq!(read_full_log(&path).unwrap(), complete);
    }

    #[test]
    fn retention_keeps_the_active_date_discoverable_and_fully_exportable() {
        let dir = tempfile::tempdir().unwrap();
        let today = chrono::Utc::now().date_naive();
        let active_name = format!("arcana.{today}.log");
        let active_path = dir.path().join(&active_name);
        let complete = (0..3_005)
            .map(|n| format!("fleet-operation-{n}\n"))
            .collect::<String>();
        std::fs::write(&active_path, &complete).unwrap();
        std::fs::write(
            dir.path().join(format!("arcana-events.{today}.jsonl")),
            "{\"event_type\":\"fleet_started\"}\n",
        )
        .unwrap();
        for offset in 1..=7 {
            let date = (today - chrono::Duration::days(offset)).format("%Y-%m-%d");
            std::fs::write(
                dir.path().join(format!("arcana.{date}.log")),
                "older readable event\n",
            )
            .unwrap();
        }

        arcana_core::observability::log_retention::prune(dir.path());

        assert!(
            active_path.is_file(),
            "retention removed today's active log"
        );
        let dates = list_log_dates_from_directory(dir.path()).unwrap();
        assert_eq!(dates.first(), Some(&today.format("%Y-%m-%d").to_string()));
        assert!(dates.len() <= 5, "log date list exceeded the retention cap");
        assert_eq!(read_full_log(&active_path).unwrap(), complete);

        let exported = dir.path().join("calendar-test-full-log-export.log");
        assert_eq!(
            super::export_full_log_file_from_dir(
                dir.path(),
                &today.format("%Y-%m-%d").to_string(),
                exported.to_str().unwrap(),
            )
            .unwrap(),
            exported.to_string_lossy()
        );
        assert_eq!(
            std::fs::read_to_string(exported).unwrap(),
            complete,
            "full export must retain every line beyond the live tail limit"
        );
    }

    #[test]
    fn line_count_cache_reads_appends_and_handles_unterminated_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("append.log");
        std::fs::write(&path, "first\nsecond").unwrap();
        assert_eq!(cached_line_count(&path, 12).unwrap(), 2);

        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"\nthird\n").unwrap();
        assert_eq!(cached_line_count(&path, 20).unwrap(), 3);
    }

    #[test]
    fn log_date_cannot_escape_the_logs_directory() {
        assert!(resolve_log_path("../secrets").is_err());
        assert!(resolve_structured_path("../secrets").is_err());
        assert!(resolve_log_path("2026-99-99").is_err());
    }

    #[test]
    fn daily_metrics_aggregate_all_valid_events_and_skip_malformed_lines() {
        let jsonl = concat!(
            "{\"fields\":{\"event_type\":\"provider_call\",\"provider\":\"google\",\"model\":\"gemini-3.8-flash\",\"duration_ns\":200,\"input_tokens\":7,\"output_tokens\":3,\"result_blocks\":4,\"result_count\":2,\"result_bytes\":64,\"outcome\":\"success\"}}\n",
            "{\"fields\":{\"event_type\":\"database_statement\",\"duration_ns\":50,\"outcome\":\"completed\"}}\n",
            "{\"fields\":{\"event_type\":\"provider_call\",\"provider\":\"google\",\"model\":\"gemini-3.8-flash\",\"duration_ns\":300,\"input_tokens\":2,\"outcome\":\"cancelled\"}}\n",
            "{\"fields\":{\"event_type\":\"tool_call_result\",\"is_error\":true,\"output_bytes\":128}}\n",
            "{\"fields\":{\"event_type\":\"mcp_reconnect_attempt\",\"outcome\":\"connection_refused\",\"duration_ns\":20}}\n",
            "not-json\n"
        );
        let summary = aggregate_event_lines(BufReader::new(jsonl.as_bytes())).unwrap();
        assert_eq!(summary.total_events, 5);
        assert_eq!(summary.malformed_events, 1);
        assert_eq!(summary.provider_calls, 2);
        assert_eq!(summary.database_statements, 1);
        assert_eq!(summary.input_tokens, 9);
        assert_eq!(summary.output_tokens, 3);
        let providers = &summary.by_event["provider_call"];
        assert_eq!(providers.count, 2);
        assert_eq!(providers.failures, 1);
        assert_eq!(providers.duration_count, 2);
        assert_eq!(providers.duration_total_ns, 500);
        assert_eq!(providers.duration_max_ns, 300);
        assert_eq!(providers.result_count_total, 2);
        assert_eq!(providers.result_bytes_total, 64);
        assert_eq!(summary.by_event["tool_call_result"].failures, 1);
        assert_eq!(summary.by_event["tool_call_result"].result_bytes_total, 128);
        assert_eq!(summary.by_event["mcp_reconnect_attempt"].failures, 1);
        assert_eq!(summary.outcomes["success"], 1);
        assert_eq!(summary.outcomes["cancelled"], 1);
        assert_eq!(summary.outcomes["unspecified"], 1);
        assert_eq!(summary.outcomes["connection_refused"], 1);
        let provider_model = &summary.by_provider_model["google/gemini-3.8-flash"];
        assert_eq!(provider_model.calls, 2);
        assert_eq!(provider_model.failures, 1);
        assert_eq!(provider_model.input_tokens, 9);
        assert_eq!(provider_model.output_tokens, 3);
        assert_eq!(provider_model.result_blocks_total, 4);
    }

    #[tokio::test]
    async fn retention_cleanup_failure_is_visible_through_structured_logs_reader() {
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let directory = tempfile::tempdir().expect("isolated retention log fixture");
        let db =
            arcana_core::analytics::db::AnalyticsDb::new_at(&directory.path().join("analytics.db"))
                .await
                .expect("open production analytics database");
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO system_events (timestamp, component, event_type, message, severity)
                 VALUES ('malformed', 'private-component', 'private-event', 'private-row-marker', 'info')",
                [],
            )
            .expect("insert malformed retention fixture");

        let log_path = directory.path().join("arcana-events.test.jsonl");
        let file = std::fs::File::create(&log_path).expect("create structured log fixture");
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(move || file.try_clone().expect("clone structured log writer"))
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    && *metadata.level() == tracing::Level::INFO
            }));
        let subscriber = Registry::default().with(layer);
        let error = tracing::subscriber::with_default(subscriber, || {
            db.prune_append_only_before("2026-07-06T12:00:00Z")
                .expect_err("malformed timestamps must fail closed")
        });
        assert!(error.to_string().contains("invalid timestamps"));

        let persisted = read_full_log(&log_path).expect("read persisted production JSONL");
        assert!(persisted.contains("database_analytics_retention_prune"));
        assert!(persisted.contains("\"outcome\":\"error\""));
        assert!(persisted.contains("\"database\":\"analytics\""));
        assert!(!persisted.contains("private-row-marker"));
        assert!(!persisted.contains("private-component"));

        let metrics = aggregate_event_lines(BufReader::new(persisted.as_bytes()))
            .expect("Logs metrics reader aggregates the persisted event");
        let retention = &metrics.by_event["operation_timing"];
        assert_eq!(retention.count, 1);
        assert_eq!(retention.failures, 1);
        assert_eq!(metrics.outcomes["error"], 1);
    }

    #[test]
    fn hook_retention_failure_is_visible_through_structured_logs_reader() {
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let directory = tempfile::tempdir().expect("isolated hook retention log fixture");
        let config_path = directory.path().join("config.db");
        let mut hooks = arcana_core::hooks::db::HooksDb::open_at(&config_path)
            .expect("open production hook database");
        let config = arcana_core::db::config_db::open_at(&config_path)
            .expect("open schema-owned config connection");
        config
            .execute_batch(
                "CREATE TRIGGER reject_hook_retention
                 BEFORE DELETE ON hook_logs
                 BEGIN
                     SELECT RAISE(ABORT, 'private-trigger-marker');
                 END;
                 INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, executed_at)
                 VALUES ('private-hook', 'private-name', 'pre_tool_use', 'executed', '2020-01-01T00:00:00Z');",
            )
            .expect("seed isolated hook cleanup failure");
        drop(config);

        let log_path = directory.path().join("arcana-events.test.jsonl");
        let file = std::fs::File::create(&log_path).expect("create structured log fixture");
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_writer(move || file.try_clone().expect("clone structured log writer"))
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    && *metadata.level() == tracing::Level::INFO
            }));
        let subscriber = Registry::default().with(layer);
        let error = tracing::subscriber::with_default(subscriber, || {
            hooks
                .prune_logs_before("2026-07-06T12:00:00Z")
                .expect_err("injected storage failure must be returned")
        });
        assert!(error.to_string().contains("private-trigger-marker"));

        let persisted = read_full_log(&log_path).expect("read persisted production JSONL");
        assert!(persisted.contains("database_hook_logs_retention_prune"));
        assert!(persisted.contains("\"outcome\":\"error\""));
        assert!(persisted.contains("\"database\":\"config\""));
        assert!(!persisted.contains("private-trigger-marker"));
        assert!(!persisted.contains("private-hook"));
        assert!(!persisted.contains("private-name"));

        let metrics = aggregate_event_lines(BufReader::new(persisted.as_bytes()))
            .expect("Logs metrics reader aggregates the persisted event");
        let retention = &metrics.by_event["operation_timing"];
        assert_eq!(retention.count, 1);
        assert_eq!(retention.failures, 1);
        assert_eq!(metrics.outcomes["error"], 1);
    }

    #[test]
    fn metrics_count_build_evidence_failure_outcome_as_failure() {
        let jsonl = "{\"fields\":{\"event_type\":\"build_evidence_persistence\",\"outcome\":\"failure\",\"duration_ns\":17}}\n";
        let summary = aggregate_event_lines(BufReader::new(jsonl.as_bytes())).unwrap();
        let event = &summary.by_event["build_evidence_persistence"];

        assert_eq!(event.count, 1);
        assert_eq!(event.failures, 1);
        assert_eq!(event.duration_total_ns, 17);
    }

    #[test]
    fn metrics_count_partial_graph_results_as_degraded_failures() {
        let jsonl = "{\"fields\":{\"event_type\":\"graph_build\",\"outcome\":\"partial\",\"error_count\":2,\"duration_ns\":31}}\n";
        let summary = aggregate_event_lines(BufReader::new(jsonl.as_bytes())).unwrap();
        let graph = &summary.by_event["graph_build"];

        assert_eq!(graph.count, 1);
        assert_eq!(graph.failures, 1);
        assert_eq!(graph.duration_total_ns, 31);
        assert_eq!(summary.outcomes["partial"], 1);
    }

    #[test]
    fn provider_attempts_count_physical_calls_once_and_denials_are_failures() {
        let jsonl = concat!(
            "{\"fields\":{\"event_type\":\"provider_attempt\",\"outcome\":\"error\"}}\n",
            "{\"fields\":{\"event_type\":\"provider_retry\",\"outcome\":\"retrying\"}}\n",
            "{\"fields\":{\"event_type\":\"provider_attempt\",\"outcome\":\"success\"}}\n",
            "{\"fields\":{\"event_type\":\"tool_call_result\",\"outcome\":\"denied\"}}\n",
        );
        let summary = aggregate_event_lines(BufReader::new(jsonl.as_bytes())).unwrap();

        assert_eq!(summary.provider_attempts, 2);
        assert_eq!(summary.by_event["provider_retry"].count, 1);
        assert_eq!(summary.by_event["tool_call_result"].failures, 1);
        assert_eq!(summary.outcomes["denied"], 1);
    }

    #[test]
    fn zero_duration_is_counted_as_an_event_but_not_as_a_measured_sample() {
        let jsonl = concat!(
            "{\"fields\":{\"event_type\":\"database_statement\",\"duration_ns\":0,\"outcome\":\"duration_observed\"}}\n",
            "{\"fields\":{\"event_type\":\"provider_call\",\"provider\":\"google\",\"model\":\"gemini-3.8-flash\",\"duration_ns\":0,\"outcome\":\"success\"}}\n",
        );
        let summary = aggregate_event_lines(BufReader::new(jsonl.as_bytes())).unwrap();

        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.database_statements, 1);
        let sqlite = &summary.by_event["database_statement"];
        assert_eq!(sqlite.count, 1);
        assert_eq!(sqlite.duration_count, 0);
        assert_eq!(sqlite.duration_zero_count, 1);
        assert_eq!(sqlite.duration_total_ns, 0);
        assert_eq!(sqlite.duration_max_ns, 0);

        let provider = &summary.by_provider_model["google/gemini-3.8-flash"];
        assert_eq!(provider.calls, 1);
        assert_eq!(provider.duration_count, 0);
        assert_eq!(provider.duration_zero_count, 1);
        assert_eq!(provider.duration_total_ns, 0);
        assert_eq!(provider.duration_max_ns, 0);
    }

    #[test]
    fn absent_task_id_does_not_match_the_synthetic_zero_correlation() {
        let event = serde_json::json!({"fields": {"event_type": "agent_turn", "task_id": ""}});
        assert!(!event_matches_correlation(&event, "0"));
        assert!(!event_matches_correlation(&event, "42"));

        let task_event =
            serde_json::json!({"fields": {"event_type": "agent_turn", "task_id": "42"}});
        assert!(event_matches_correlation(&task_event, "42"));
    }

    #[test]
    fn incremental_metrics_wait_for_complete_lines_and_merge_new_events() {
        let mut metrics = MetricsAccumulator::default();
        metrics
            .ingest(
                Cursor::new(b"{\"fields\":{\"event_type\":\"provider_call\",\"input_tokens\":4}}"),
                None,
            )
            .unwrap();
        assert_eq!(
            metrics.summary.total_events, 0,
            "partial JSONL must not be counted early"
        );
        metrics
            .ingest(
                Cursor::new(b"\n{\"fields\":{\"event_type\":\"database_statement\"}}\n"),
                None,
            )
            .unwrap();
        assert_eq!(metrics.summary.total_events, 2);
        assert_eq!(metrics.summary.input_tokens, 4);
        assert_eq!(metrics.summary.provider_calls, 1);
        assert_eq!(metrics.summary.database_statements, 1);
    }

    #[test]
    fn metrics_cache_resets_after_equal_size_file_recreation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arcana-events.2026-09-29.jsonl");
        let old = b"{\"fields\":{\"event_type\":\"old_event\"}}\n";
        let replacement = b"{\"fields\":{\"event_type\":\"new_event\"}}\n";
        assert_eq!(old.len(), replacement.len());
        std::fs::write(&path, old).unwrap();

        let mut cached = MetricsAccumulator::default();
        assert_eq!(
            refresh_metrics_from_file(&path, &mut cached)
                .unwrap()
                .total_events,
            1
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            refresh_metrics_from_file(&path, &mut cached)
                .unwrap()
                .total_events,
            0
        );
        std::fs::write(&path, replacement).unwrap();

        let summary = refresh_metrics_from_file(&path, &mut cached).unwrap();
        assert_eq!(summary.total_events, 1);
        assert!(!summary.by_event.contains_key("old_event"));
        assert_eq!(summary.by_event["new_event"].count, 1);
    }

    #[test]
    fn metrics_cache_resets_after_larger_file_recreation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arcana-events.2026-09-29.jsonl");
        std::fs::write(&path, b"{\"fields\":{\"event_type\":\"old\"}}\n").unwrap();

        let mut cached = MetricsAccumulator::default();
        assert_eq!(
            refresh_metrics_from_file(&path, &mut cached)
                .unwrap()
                .total_events,
            1
        );
        std::fs::remove_file(&path).unwrap();
        refresh_metrics_from_file(&path, &mut cached).unwrap();
        std::fs::write(
            &path,
            b"{\"fields\":{\"event_type\":\"replacement_event\"}}\n",
        )
        .unwrap();

        let summary = refresh_metrics_from_file(&path, &mut cached).unwrap();
        assert_eq!(summary.total_events, 1);
        assert!(!summary.by_event.contains_key("old"));
        assert_eq!(summary.by_event["replacement_event"].count, 1);
    }

    #[test]
    fn metrics_cache_resets_when_file_is_replaced_without_observing_deletion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arcana-events.2026-09-29.jsonl");
        let replacement_path = dir.path().join("replacement.jsonl");
        std::fs::write(&path, b"{\"fields\":{\"event_type\":\"old_event\"}}\n").unwrap();

        let mut cached = MetricsAccumulator::default();
        assert_eq!(
            refresh_metrics_from_file(&path, &mut cached)
                .unwrap()
                .total_events,
            1
        );
        std::fs::write(
            &replacement_path,
            b"{\"fields\":{\"event_type\":\"new_event\"}}\n",
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&replacement_path, &path).unwrap();

        let summary = refresh_metrics_from_file(&path, &mut cached).unwrap();
        assert_eq!(summary.total_events, 1);
        assert!(!summary.by_event.contains_key("old_event"));
        assert_eq!(summary.by_event["new_event"].count, 1);
    }

    #[test]
    fn metrics_ingestion_reads_large_files_in_fixed_size_chunks() {
        let jsonl = (0..40_000)
            .map(|_| "{\"fields\":{\"event_type\":\"provider_call\"}}\n")
            .collect::<String>();
        let mut reader = ChunkTrackingReader {
            inner: Cursor::new(jsonl.as_bytes()),
            max_requested: 0,
            calls: 0,
        };
        let mut metrics = MetricsAccumulator::default();

        metrics.ingest(&mut reader, None).unwrap();

        assert_eq!(metrics.summary.total_events, 40_000);
        assert_eq!(metrics.summary.provider_calls, 40_000);
        assert!(
            reader.calls > 1,
            "large fixture must require multiple reads"
        );
        assert_eq!(reader.max_requested, METRICS_READ_CHUNK_BYTES);
        assert!(metrics.pending.is_empty());
    }

    #[test]
    fn correlation_filter_matches_event_fields_and_parent_span_fields() {
        let jsonl = concat!(
            "{\"fields\":{\"event_type\":\"provider_call\",\"input_tokens\":4,\"correlation_id\":\"fleet-a\"}}\n",
            "{\"fields\":{\"event_type\":\"database_statement\",\"duration_ns\":7},\"spans\":[{\"fields\":{\"fleet_correlation_id\":\"fleet-b\"}}]}\n",
            "{\"fields\":{\"event_type\":\"tool_call_result\",\"output_bytes\":10},\"spans\":[{\"fields\":{\"fleet_correlation_id\":\"fleet-a\"}}]}\n",
            "{\"fields\":{\"event_type\":\"project_command_result\",\"tool_call_id\":\"fleet-a\",\"outcome\":\"success\"}}\n",
        );
        let summary = aggregate_event_lines_for_correlation(
            BufReader::new(jsonl.as_bytes()),
            Some("fleet-a"),
        )
        .unwrap();

        assert_eq!(summary.total_events, 3);
        assert_eq!(summary.provider_calls, 1);
        assert_eq!(summary.input_tokens, 4);
        assert_eq!(summary.tool_calls, 1);
        assert_eq!(summary.by_event["tool_call_result"].result_bytes_total, 10);
        assert_eq!(summary.by_event["project_command_result"].count, 1);
        assert!(!summary.by_event.contains_key("database_statement"));
    }

    #[test]
    fn correlation_filter_matches_flattened_production_tracing_spans() {
        let jsonl = concat!(
            "{\"fields\":{\"event_type\":\"database_statement\",\"duration_ns\":7},\"spans\":[{\"name\":\"database_task_assignment\",\"database\":\"tasks\",\"operation\":\"database_task_assign_with_routing\",\"task_id\":72,\"worker\":\"worker-a\"}]}\n",
            "{\"fields\":{\"event_type\":\"database_statement\",\"duration_ns\":8},\"spans\":[{\"name\":\"database_task_assignment\",\"database\":\"tasks\",\"task_id\":73}]}\n",
        );
        let summary =
            aggregate_event_lines_for_correlation(BufReader::new(jsonl.as_bytes()), Some("72"))
                .unwrap();

        assert_eq!(summary.total_events, 1);
        assert_eq!(summary.database_statements, 1);
    }

    #[test]
    fn correlation_event_reader_returns_only_matching_recent_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"fields\":{\"event_type\":\"old\",\"correlation_id\":\"fleet-a\"}}\n",
                "{\"fields\":{\"event_type\":\"other\",\"correlation_id\":\"fleet-b\"}}\n",
                "{\"fields\":{\"event_type\":\"new\"},\"spans\":[{\"fields\":{\"fleet_correlation_id\":\"fleet-a\"}}]}\n",
            ),
        )
        .unwrap();

        let (content, total) = read_structured_events_for_correlation(&path, "fleet-a", 1).unwrap();
        assert_eq!(total, 2);
        assert!(content.contains("\"event_type\":\"new\""));
        assert!(!content.contains("\"event_type\":\"old\""));
        assert!(!content.contains("fleet-b"));
    }

    #[test]
    fn correlation_offset_index_handles_spans_and_partial_append_incrementally() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"fields\":{\"event_type\":\"first\",\"correlation_id\":\"fleet-a\"}}\n",
                "{\"fields\":{\"event_type\":\"other\",\"correlation_id\":\"fleet-b\"}}\n",
                "{\"fields\":{\"event_type\":\"span\"},\"spans\":[{\"fields\":{\"fleet_correlation_id\":\"fleet-a\"}}]}\n",
            ),
        )
        .unwrap();

        let (content, total) =
            read_structured_events_for_correlation(&path, "fleet-a", 10).unwrap();
        assert_eq!(total, 2);
        assert!(content.contains("\"event_type\":\"first\""));
        assert!(content.contains("\"event_type\":\"span\""));

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        file.write_all(b"{\"fields\":{\"event_type\":\"partial\",\"task_id\":73}")
            .unwrap();
        let (content, total) = read_structured_events_for_correlation(&path, "73", 10).unwrap();
        assert_eq!(total, 0, "unterminated appended bytes are not indexed");
        assert!(content.is_empty());

        file.write_all(b"}\n").unwrap();
        let (content, total) = read_structured_events_for_correlation(&path, "73", 10).unwrap();
        assert_eq!(total, 1, "the completed appended record is indexed once");
        assert!(content.contains("\"event_type\":\"partial\""));
    }

    #[test]
    fn correlation_offset_index_resets_after_file_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let replacement = dir.path().join("replacement.jsonl");
        let first = b"{\"fields\":{\"event_type\":\"old\",\"correlation_id\":\"fleet-a\"}}\n";
        let second = b"{\"fields\":{\"event_type\":\"new\",\"correlation_id\":\"fleet-a\"}}\n";
        assert_eq!(first.len(), second.len());
        std::fs::write(&path, first).unwrap();
        let (content, total) =
            read_structured_events_for_correlation(&path, "fleet-a", 10).unwrap();
        assert_eq!(total, 1);
        assert!(content.contains("\"event_type\":\"old\""));

        std::fs::write(&replacement, second).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        let (content, total) =
            read_structured_events_for_correlation(&path, "fleet-a", 10).unwrap();
        assert_eq!(total, 1);
        assert!(content.contains("\"event_type\":\"new\""));
        assert!(!content.contains("\"event_type\":\"old\""));

        std::fs::write(&path, b"{\"fields\":{\"event_type\":\"truncated\"}}\n").unwrap();
        let (content, total) =
            read_structured_events_for_correlation(&path, "fleet-a", 10).unwrap();
        assert_eq!(total, 0, "truncation rebuilds the index from current bytes");
        assert!(content.is_empty());
    }

    #[test]
    fn correlation_offset_index_prunes_deleted_log_files() {
        let dir = tempfile::tempdir().unwrap();
        let current_path = dir.path().join("current.jsonl");
        let removed_path = dir.path().join("removed.jsonl");
        std::fs::write(&current_path, b"{\"fields\":{\"run_id\":\"current\"}}\n").unwrap();

        let cache = CORRELATION_OFFSETS
            .get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
        cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(removed_path.clone(), CorrelationOffsetIndex::default());
        ensure_correlation_index(&current_path, "current").unwrap();

        assert!(!cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains_key(&removed_path));
    }

    #[test]
    fn production_operation_event_persists_and_reads_through_tauri_log_consumers() {
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let dir = tempfile::tempdir().expect("isolated log directory");
        let path = dir.path().join("arcana-events.2026-10-03.jsonl");
        let file = std::fs::File::create(&path).expect("create isolated structured log");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    || metadata.target().starts_with("arcana::telemetry::")
            }));
        let subscriber = Registry::default().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            let mut operation =
                arcana_core::observability::timing::OperationTimer::start("logs_roundtrip_fixture")
                    .with_run_id("run-logs-roundtrip")
                    .with_trace_id("trace-logs-roundtrip")
                    .with_result_count(4);
            operation.succeed();
        });
        drop(guard);

        let (content, total) =
            read_structured_events_for_correlation(&path, "run-logs-roundtrip", 10)
                .expect("Tauri structured-event reader should read persisted producer output");
        assert_eq!(total, 1);
        let event: serde_json::Value =
            serde_json::from_str(content.trim()).expect("reader returns valid JSON event");
        assert_eq!(event["fields"]["event_type"], "operation_timing");
        assert_eq!(event["fields"]["operation"], "logs_roundtrip_fixture");
        assert_eq!(event["fields"]["run_id"], "run-logs-roundtrip");
        assert_eq!(event["fields"]["trace_id"], "trace-logs-roundtrip");
        assert_eq!(event["fields"]["outcome"], "success");
        assert_eq!(event["fields"]["result_count"], 4);
        assert!(event["fields"]["duration_ns"].as_u64().unwrap_or_default() > 0);

        let mut metrics = MetricsAccumulator::default();
        let metrics = refresh_metrics_from_file_for_correlation(
            &path,
            &mut metrics,
            Some("run-logs-roundtrip"),
        )
        .expect("metrics consumer should aggregate persisted producer output");
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.outcomes.get("success"), Some(&1));
        let aggregate = metrics
            .by_event
            .get("operation_timing")
            .expect("operation aggregate");
        assert_eq!(aggregate.count, 1);
        assert_eq!(aggregate.result_count_total, 4);
        assert_eq!(aggregate.duration_count, 1);
        assert!(!content.contains("source contents"));
    }

    #[tokio::test]
    async fn production_memory_search_event_persists_and_reads_through_tauri_logs() {
        use arcana_core::agent::tool_trait::{AgentIdentity, Tool, ToolContext};
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let log_dir = home.path().join(".arcana").join("logs");
        std::fs::create_dir_all(&log_dir).expect("create isolated production log directory");

        let memory_db = arcana_core::memory::db::MemoryDb::open()
            .expect("open migrated production Memory database");
        memory_db
            .conn
            .execute(
                "INSERT INTO memories(id, tier, scope, scope_ref, scope_key, content, created_at, updated_at)
                 VALUES ('memory-observability', 'semantic', 'project', '/project', 'project:/project',
                         'arcana-observability-private-memory-canary', '2026-10-04T00:00:00Z', '2026-10-04T00:00:00Z')",
                [],
            )
            .expect("seed isolated production Memory row");
        drop(memory_db);

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let path = log_dir.join(format!("arcana-events.{date}.jsonl"));
        let file = std::fs::File::create(&path).expect("create production structured log path");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    || metadata.target().starts_with("arcana::telemetry::")
            }));
        let subscriber = Registry::default().with(layer);
        let context = ToolContext {
            session_id: "session-memory-observability".into(),
            working_dir: "/project".into(),
            call_id: "call-memory-observability".into(),
            run_id: Some("run-memory-observability".into()),
            task_id: Some(42),
            permission_mode: arcana_core::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,
            run_token_budget: None,
        };
        let tool_result = tracing::subscriber::with_default(subscriber, || {
            futures::executor::block_on(
                arcana_core::agent::builtin_tools::search_memory::SearchMemoryTool.execute(
                    serde_json::json!({
                        "query": "arcana-observability-private-memory-canary"
                    }),
                    &context,
                ),
            )
        });
        assert!(!tool_result.is_error, "{}", tool_result.output);
        drop(guard);

        let result =
            super::read_structured_events(date, Some(10), Some("run-memory-observability".into()))
                .expect("production Tauri command should read the persisted correlated event");
        assert_eq!(result.total_lines, 1);
        let event: serde_json::Value =
            serde_json::from_str(result.content.trim()).expect("valid persisted telemetry JSON");
        assert_eq!(event["fields"]["event_type"], "operation_timing");
        assert_eq!(event["fields"]["operation"], "memory_search_all");
        assert_eq!(event["fields"]["database"], "memory");
        assert_eq!(event["fields"]["call_id"], "call-memory-observability");
        assert_eq!(event["fields"]["run_id"], "run-memory-observability");
        assert_eq!(event["fields"]["task_id"], "42");
        assert_eq!(event["fields"]["outcome"], "fallback");
        assert_eq!(event["fields"]["result_count"], 1);
        assert!(event["fields"]["duration_ns"].as_u64().unwrap_or_default() > 0);
        assert!(!result
            .content
            .contains("arcana-observability-private-memory-canary"));

        let mut metrics = MetricsAccumulator::default();
        let metrics = refresh_metrics_from_file_for_correlation(
            &path,
            &mut metrics,
            Some("run-memory-observability"),
        )
        .expect("production Logs metrics reader aggregates persisted Memory telemetry");
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.outcomes.get("fallback"), Some(&1));
        let aggregate = metrics
            .by_event
            .get("operation_timing")
            .expect("Memory operation metrics");
        assert_eq!(aggregate.count, 1);
        assert_eq!(aggregate.result_count_total, 1);
        assert_eq!(aggregate.duration_count, 1);
    }

    #[test]
    fn production_project_command_stages_persist_and_read_through_tauri_logs() {
        use arcana_core::agent::tool_trait::{AgentIdentity, Tool, ToolContext};
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let root = tempfile::tempdir().expect("isolated test root");
        let home = root.path().join("home");
        let _home = crate::test_support::isolate_home(&home);
        let log_dir = home.join(".arcana").join("logs");
        std::fs::create_dir_all(&log_dir).expect("create isolated structured log directory");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(workspace.join("src")).expect("create project source directory");
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"logs-project-command-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write dependency-free Cargo manifest");
        std::fs::write(
            workspace.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"logs-project-command-fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write stable dependency-free Cargo lockfile");
        std::fs::write(workspace.join("src/main.rs"), "fn main() {}\n")
            .expect("write dependency-free Rust source");

        let tasks = arcana_core::db::tasks::TasksDb::open().expect("open isolated tasks database");
        let task_id = tasks
            .create(arcana_core::db::tasks::CreateTaskInput {
                title: "Logs project-command timing fixture".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a production build stage is visible in Logs".into(),
                created_by: Some("p4-50-logs-test".into()),
            })
            .expect("create build task")
            .id;
        tasks
            .update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign build task");
        tasks
            .claim_task_with_pid(task_id, "worker-backend", std::process::id())
            .expect("claim build task");
        drop(tasks);

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let call_id = "call-project-command-logs-proof";
        let log_path = log_dir.join(format!("arcana-events.{date}.jsonl"));
        let file = std::fs::File::create(&log_path).expect("create production structured log");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    || metadata.target().starts_with("arcana::telemetry::")
                    || metadata.target() == "arcana::project_command"
            }));
        let subscriber = Registry::default().with(layer);
        // The production worker executor owns this lease for the full command run.
        // Holding a real executor here keeps the Windows sandbox scope identical.
        let _executor = arcana_core::agent::tool_executor::ToolExecutor::new(
            arcana_core::agent::permission::PermissionMode::Autonomous,
            arcana_core::agent::permission::new_pending_permissions(),
            "session-project-command-logs-proof".into(),
            workspace.to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-backend"),
        );
        let context = ToolContext {
            session_id: "session-project-command-logs-proof".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: call_id.into(),
            run_id: Some("run-project-command-logs-proof".into()),
            task_id: Some(task_id),
            permission_mode: arcana_core::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::worker("worker-backend").role,
            run_token_budget: None,
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create isolated command runtime");
        let result = tracing::subscriber::with_default(subscriber, || {
            runtime.block_on(arcana_core::agent::builtin_tools::project_command::ProjectCommandTool.execute(
                serde_json::json!({"intent": "build", "toolchain": "cargo", "timeout_secs": 120}),
                &context,
            ))
        });
        assert!(
            !result.is_error,
            "production build failed: {}",
            result.output
        );
        drop(guard);

        let persisted = super::read_structured_events(date, Some(100), Some(call_id.to_string()))
            .expect("production Tauri Logs command reads correlated build events");
        let events = persisted
            .content
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid event JSON"))
            .collect::<Vec<_>>();
        let evidence = events
            .iter()
            .find(|event| {
                event["fields"]["event_type"] == "project_command_stage"
                    && event["fields"]["stage"] == "evidence_persistence"
            })
            .expect("the production evidence-persistence stage is visible in Logs");
        assert_eq!(evidence["fields"]["call_id"], call_id);
        assert_eq!(evidence["fields"]["task_id"], task_id.to_string());
        assert_eq!(evidence["fields"]["outcome"], "success");
        assert!(evidence["fields"]["duration_ns"].as_u64().is_some());
        assert!(
            events.iter().any(|event| {
                event["fields"]["event_type"] == "project_command_result"
                    && event["fields"]["tool_call_id"] == call_id
                    && event["fields"]["outcome"] == "success"
            }),
            "the same call's successful spawned-process result must be present: {events:#?}"
        );
        assert!(!persisted.content.contains("fn main()"));
        assert!(!persisted.content.contains("Cargo.toml"));

        let mut metrics = MetricsAccumulator::default();
        let metrics =
            refresh_metrics_from_file_for_correlation(&log_path, &mut metrics, Some(call_id))
                .expect("production Logs metrics reader consumes build timing");
        assert!(metrics.total_events >= 3);
        assert!(metrics.outcomes.get("success").copied().unwrap_or_default() > 0);
        assert!(metrics
            .by_event
            .get("project_command_stage")
            .is_some_and(|aggregate| aggregate.count > 0 && aggregate.duration_count > 0));
        assert!(metrics
            .by_event
            .get("project_command_result")
            .is_some_and(|aggregate| aggregate.count == 1 && aggregate.duration_count == 1));
    }

    #[test]
    fn production_mcp_consent_denial_persists_and_reads_through_tauri_logs() {
        use arcana_core::agent::tool_trait::{AgentIdentity, Tool, ToolContext};
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let log_dir = home.path().join(".arcana").join("logs");
        std::fs::create_dir_all(&log_dir).expect("create isolated structured log directory");
        let workspace = home.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create isolated workspace");

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let call_id = "call-mcp-consent-denial-logs-proof";
        let log_path = log_dir.join(format!("arcana-events.{date}.jsonl"));
        let file = std::fs::File::create(&log_path).expect("create production structured log");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    || metadata.target().starts_with("arcana::telemetry::")
            }));
        let subscriber = Registry::default().with(layer);
        let context = ToolContext {
            session_id: "session-mcp-consent-denial-logs-proof".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: call_id.into(),
            run_id: Some("run-mcp-consent-denial-logs-proof".into()),
            task_id: None,
            permission_mode: arcana_core::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,
            run_token_budget: None,
        };
        let result = tracing::subscriber::with_default(subscriber, || {
            futures::executor::block_on(
                arcana_core::agent::builtin_tools::external_mcp::McpListToolsTool
                    .execute(serde_json::json!({}), &context),
            )
        });
        assert!(
            result.is_error,
            "an unconsented MCP request must fail closed"
        );
        assert!(result.output.contains("no active consent"));
        drop(guard);

        let persisted = super::read_structured_events(date, Some(20), Some(call_id.to_string()))
            .expect("production Tauri Logs command reads correlated MCP denial");
        let events = persisted
            .content
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid event JSON"))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        let event = &events[0]["fields"];
        assert_eq!(event["event_type"], "mcp_call");
        assert_eq!(event["operation"], "mcp_tools_list");
        assert_eq!(event["connector_id"], "github-mcp");
        assert_eq!(event["outcome"], "authorization_failed");
        assert_eq!(event["call_id"], call_id);
        assert!(event["elapsed_ns"].as_u64().is_some());

        let mut metrics = MetricsAccumulator::default();
        let metrics =
            refresh_metrics_from_file_for_correlation(&log_path, &mut metrics, Some(call_id))
                .expect("production Logs metrics reader aggregates MCP denial");
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.outcomes.get("authorization_failed"), Some(&1));
        assert_eq!(metrics.by_event["mcp_call"].count, 1);
        assert!(!persisted.content.contains("server_url"));
    }

    #[test]
    fn production_mcp_call_persists_and_reads_through_tauri_logs() {
        use arcana_core::agent::tool_trait::{AgentIdentity, Tool, ToolContext};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        use tracing_subscriber::{filter::filter_fn, layer::SubscriberExt, Layer, Registry};

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::test_support::isolate_home(home.path());
        let log_dir = home.path().join(".arcana").join("logs");
        std::fs::create_dir_all(&log_dir).expect("create isolated structured log directory");
        let workspace = home.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create isolated workspace");

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("create isolated MCP runtime");
        let listener = runtime
            .block_on(TcpListener::bind("127.0.0.1:0"))
            .expect("bind local-only MCP fixture");
        let address = listener.local_addr().expect("read local fixture address");
        let endpoint = format!("http://{address}/mcp");
        let server = runtime.spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                tokio::spawn(async move {
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 4096];
                    let header_end = loop {
                        let count = socket.read(&mut buffer).await.unwrap_or(0);
                        if count == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..count]);
                        if let Some(index) =
                            request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                        {
                            break index + 4;
                        }
                    };
                    let headers = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    while request.len() < header_end + content_length {
                        let count = socket.read(&mut buffer).await.unwrap_or(0);
                        if count == 0 {
                            return;
                        }
                        request.extend_from_slice(&buffer[..count]);
                    }
                    let body: serde_json::Value = serde_json::from_slice(
                        &request[header_end..header_end + content_length],
                    )
                    .expect("valid MCP request JSON");
                    let method = body["method"].as_str().unwrap_or_default();
                    if method.starts_with("notifications/") {
                        let _ = socket
                            .write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                            .await;
                        return;
                    }
                    let result = match method {
                        "server/discover" => serde_json::json!({
                            "resultType":"complete",
                            "supportedVersions":["2026-07-28"],
                            "capabilities":{},
                            "ttlMs":0,
                            "cacheScope":"private"
                        }),
                        "tools/call" => serde_json::json!({
                            "content": [{"type":"text","text":"fixture response"}],
                            "isError": false
                        }),
                        other => panic!("unexpected MCP request method {other}"),
                    };
                    let payload = serde_json::json!({
                        "jsonrpc":"2.0",
                        "id":body["id"],
                        "result":result
                    })
                    .to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        payload.len(), payload
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });

        arcana_core::db::app_settings::set(
            "github_mcp_config",
            &serde_json::json!({"enabled":true,"server_url":endpoint}).to_string(),
        )
        .expect("persist local connector configuration");
        arcana_core::egress_consent::WorkspaceEgressConsentDb::open()
            .expect("open isolated egress-consent database")
            .grant(&workspace, "mcp:github-mcp")
            .expect("grant this isolated workspace MCP consent");

        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let call_id = "call-mcp-success-logs-proof";
        let log_path = log_dir.join(format!("arcana-events.{date}.jsonl"));
        let file = std::fs::File::create(&log_path).expect("create production structured log");
        let (writer, guard) = tracing_appender::non_blocking(file);
        let layer = tracing_subscriber::fmt::layer()
            .json()
            .with_target(true)
            .with_ansi(false)
            .with_current_span(true)
            .with_span_list(true)
            .with_writer(writer)
            .with_filter(filter_fn(|metadata| {
                metadata.target() == "arcana::telemetry"
                    || metadata.target().starts_with("arcana::telemetry::")
            }));
        let subscriber = Registry::default().with(layer);
        let context = ToolContext {
            session_id: "session-mcp-success-logs-proof".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: call_id.into(),
            run_id: Some("run-mcp-success-logs-proof".into()),
            task_id: Some(73),
            permission_mode: arcana_core::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::worker("worker-backend").role,
            run_token_budget: None,
        };
        let result = tracing::subscriber::with_default(subscriber, || {
            runtime.block_on(async {
                arcana_core::agent::builtin_tools::external_mcp::McpCallTool
                    .execute(
                        serde_json::json!({
                            "name":"fixture_search",
                            "arguments":{"query":"private-query-marker"}
                        }),
                        &context,
                    )
                    .await
            })
        });
        assert!(
            !result.is_error,
            "local MCP invocation failed: {}",
            result.output
        );
        drop(guard);
        server.abort();
        runtime.block_on(async {
            let _ = server.await;
        });

        let payload: serde_json::Value =
            serde_json::from_str(&result.output).expect("bounded MCP result JSON");
        assert_eq!(payload["content"][0]["text"], "fixture response");
        let persisted = super::read_structured_events(date, Some(20), Some(call_id.to_string()))
            .expect("production Tauri Logs command reads correlated MCP success");
        let events = persisted
            .content
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid event JSON"))
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        let event = &events[0]["fields"];
        assert_eq!(event["event_type"], "mcp_call");
        assert_eq!(event["operation"], "mcp_tools_call");
        assert_eq!(event["connector_id"], "github-mcp");
        assert_eq!(event["outcome"], "success");
        assert_eq!(event["task_id"], 73);
        assert_eq!(event["run_id"], "run-mcp-success-logs-proof");
        assert_eq!(event["call_id"], call_id);
        assert!(event["output_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0));
        assert!(event["elapsed_ns"].as_u64().is_some());
        assert!(!persisted.content.contains("private-query-marker"));
        assert!(!persisted.content.contains("fixture response"));

        let mut metrics = MetricsAccumulator::default();
        let metrics =
            refresh_metrics_from_file_for_correlation(&log_path, &mut metrics, Some(call_id))
                .expect("production Logs metrics reader aggregates MCP success");
        assert_eq!(metrics.total_events, 1);
        assert_eq!(metrics.outcomes.get("success"), Some(&1));
        assert_eq!(metrics.by_event["mcp_call"].count, 1);
        assert!(metrics.by_event["mcp_call"].result_bytes_total > 0);
    }

    #[test]
    fn measure_metrics_refresh_tail_and_full_export_on_reproducible_jsonl() {
        use std::io::BufWriter;
        use std::time::Instant;

        const EVENTS: usize = 25_000;
        const APPENDED: usize = 1_000;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arcana-events-cost-fixture.jsonl");
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut writer = BufWriter::new(file);
            for index in 0..EVENTS {
                let record = serde_json::json!({
                    "fields": {
                        "event_type": "database_statement",
                        "outcome": "success",
                        "database": "tasks",
                        "operation": "database_task_status_transition",
                        "run_id": "run-0123456789abcdef",
                        "trace_id": "trace-0123456789abcdef",
                        "result_count": 1,
                        "input_tokens": 3,
                        "output_tokens": 2,
                        "duration_ns": 1_250_000,
                        "sequence": index,
                    }
                });
                serde_json::to_writer(&mut writer, &record).unwrap();
                writer.write_all(b"\n").unwrap();
            }
            writer.flush().unwrap();
        }

        let mut cache = MetricsAccumulator::default();
        let started = Instant::now();
        let summary = refresh_metrics_from_file(&path, &mut cache).unwrap();
        let initial_refresh = started.elapsed();
        assert_eq!(summary.total_events, EVENTS as u64);
        assert_eq!(summary.database_statements, EVENTS as u64);
        assert_eq!(summary.input_tokens, (EVENTS * 3) as u64);

        {
            let file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap();
            let mut writer = BufWriter::new(file);
            for index in 0..APPENDED {
                writeln!(
                    writer,
                    "{{\"fields\":{{\"event_type\":\"database_statement\",\"database\":\"tasks\",\"operation\":\"database_task_status_transition\",\"run_id\":\"run-0123456789abcdef\",\"trace_id\":\"trace-0123456789abcdef\",\"result_count\":1,\"input_tokens\":3,\"sequence\":{index}}}}}"
                )
                .unwrap();
            }
            writer.flush().unwrap();
        }
        let started = Instant::now();
        let updated = refresh_metrics_from_file(&path, &mut cache).unwrap();
        let incremental_refresh = started.elapsed();
        assert_eq!(updated.total_events, (EVENTS + APPENDED) as u64);
        assert_eq!(updated.database_statements, (EVENTS + APPENDED) as u64);

        let started = Instant::now();
        let tail = read_tail(&path, 2_000).unwrap();
        let tail_read = started.elapsed();
        assert_eq!(tail.lines().count(), 2_000);

        let started = Instant::now();
        let exported = read_full_log(&path).unwrap();
        let full_export = started.elapsed();
        let full_size = std::fs::metadata(&path).unwrap().len();
        assert_eq!(exported.len() as u64, full_size);

        eprintln!(
            "structured_log_cost fixture_bytes={full_size} initial_events={EVENTS} appended_events={APPENDED} initial_metrics_ns={} incremental_refresh_ns={} tail_2000_ns={} full_read_export_ns={}",
            initial_refresh.as_nanos(),
            incremental_refresh.as_nanos(),
            tail_read.as_nanos(),
            full_export.as_nanos(),
        );
    }
}
