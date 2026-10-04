//! Bounded retention for Arcana's readable and structured daily logs.
//!
//! The policy is intentionally fixed for this desktop release: no more than
//! five files per stream and no files dated more than seven days ago. Only
//! Arcana's recognized daily filenames are eligible for deletion. Symlinks,
//! malformed names, and unrelated files are left untouched.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use tracing::{info, warn};

const MAX_FILES_PER_STREAM: usize = 5;
const MAX_LOG_AGE_DAYS: i64 = 7;

#[derive(Debug, Default)]
struct PruneSummary {
    dropped: u64,
    bytes: u64,
    failures: Vec<String>,
}

/// Prune recognized Arcana daily logs once at host startup. Cleanup failures
/// are reported and never prevent the application from starting.
pub fn prune(log_dir: &Path) {
    let summary = prune_at(log_dir, chrono::Utc::now().date_naive());
    for failure in &summary.failures {
        warn!("[log_retention] {failure}");
    }
    info!(
        "[log_retention] policy=5-files/7-days; removed {} file(s), reclaimed {} bytes, {} cleanup failure(s)",
        summary.dropped,
        summary.bytes,
        summary.failures.len()
    );
}

/// Apply the shared retention policy to Arcana's default user log directory.
/// Hosts call this after their log appenders are initialized so the cleanup
/// result is written to the new active file.
pub fn prune_default() {
    let Some(home) = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
    else {
        return;
    };
    prune(&PathBuf::from(home).join(".arcana").join("logs"));
}

fn prune_at(log_dir: &Path, today: NaiveDate) -> PruneSummary {
    prune_at_with_limit(log_dir, today, MAX_FILES_PER_STREAM)
}

fn prune_at_with_limit(
    log_dir: &Path,
    today: NaiveDate,
    max_files_per_stream: usize,
) -> PruneSummary {
    let mut summary = PruneSummary::default();
    match std::fs::symlink_metadata(log_dir) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            summary
                .failures
                .push("log directory is not a plain directory".into());
            return summary;
        }
        Err(error) => {
            summary
                .failures
                .push(format!("could not inspect log directory: {error}"));
            return summary;
        }
    }

    let mut streams: [Vec<(NaiveDate, bool, PathBuf, u64)>; 2] = [Vec::new(), Vec::new()];
    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) => {
            summary
                .failures
                .push(format!("could not list log directory: {error}"));
            return summary;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                summary
                    .failures
                    .push(format!("could not inspect a log entry: {error}"));
                continue;
            }
        };
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                summary
                    .failures
                    .push(format!("could not inspect a log entry: {error}"));
                continue;
            }
        };
        if !file_type.is_file() || file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let Some((structured, date)) = parse_log_filename(&name) else {
            continue;
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(error) => {
                summary
                    .failures
                    .push(format!("could not inspect a recognized log file: {error}"));
                continue;
            }
        };
        if (today - date).num_days() > MAX_LOG_AGE_DAYS {
            remove_log(&path, metadata.len(), &mut summary, "age limit");
        } else {
            let active_name = if structured {
                format!("arcana-events.{today}.jsonl")
            } else {
                format!("arcana.{today}.log")
            };
            let is_active = path.file_name().and_then(|name| name.to_str()) == Some(&active_name);
            streams[usize::from(structured)].push((date, is_active, path, metadata.len()));
        }
    }

    for stream in &mut streams {
        stream.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.0.cmp(&left.0))
                .then_with(|| left.2.file_name().cmp(&right.2.file_name()))
        });
        if stream.len() > max_files_per_stream {
            for (_, _, path, size) in stream.drain(max_files_per_stream..) {
                remove_log(&path, size, &mut summary, "file-count limit");
            }
        }
    }
    summary
}

fn remove_log(path: &Path, size: u64, summary: &mut PruneSummary, reason: &str) {
    match std::fs::remove_file(path) {
        Ok(()) => {
            summary.dropped += 1;
            summary.bytes += size;
        }
        Err(error) => summary.failures.push(format!(
            "could not remove recognized log ({reason}): {error}"
        )),
    }
}

fn parse_log_filename(name: &str) -> Option<(bool, NaiveDate)> {
    const READABLE: &[(&str, &str)] = &[
        ("arcana.", ".log"),
        ("arcana-.", "..log"),
        ("arcana-", ".log"),
    ];
    const STRUCTURED: &[(&str, &str)] = &[
        ("arcana-events.", ".jsonl"),
        ("arcana-events-.", "..jsonl"),
        ("arcana-events-", ".jsonl"),
    ];
    for (structured, patterns) in [(false, READABLE), (true, STRUCTURED)] {
        if let Some(date) = patterns.iter().find_map(|(prefix, suffix)| {
            name.strip_prefix(prefix)
                .and_then(|name| name.strip_suffix(suffix))
                .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        }) {
            return Some((structured, date));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parser_accepts_current_and_legacy_names_only() {
        for (name, structured) in [
            ("arcana.2026-09-28.log", false),
            ("arcana-.2026-09-28..log", false),
            ("arcana-2026-09-28.log", false),
            ("arcana-events.2026-09-28.jsonl", true),
            ("arcana-events-.2026-09-28..jsonl", true),
            ("arcana-events-2026-09-28.jsonl", true),
        ] {
            assert_eq!(
                parse_log_filename(name).map(|value| value.0),
                Some(structured)
            );
        }
        assert!(parse_log_filename("arcana.not-a-date.log").is_none());
        assert!(parse_log_filename("notes-2026-09-28.log").is_none());
    }

    #[test]
    fn policy_enforces_age_and_five_files_per_stream_without_touching_other_files() {
        let dir = tempfile::tempdir().expect("temporary logs");
        let today = NaiveDate::from_ymd_opt(2026, 9, 28).unwrap();
        for offset in 0..10 {
            let date = (today - chrono::Duration::days(offset)).format("%Y-%m-%d");
            std::fs::write(dir.path().join(format!("arcana.{date}.log")), "readable").unwrap();
            std::fs::write(
                dir.path().join(format!("arcana-events.{date}.jsonl")),
                "structured",
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("arcana.not-a-date.log"), "keep").unwrap();
        std::fs::write(dir.path().join("user-notes.log"), "keep").unwrap();

        let summary = prune_at(dir.path(), today);

        assert!(summary.failures.is_empty(), "{:?}", summary.failures);
        assert_eq!(summary.dropped, 10);
        for structured in [false, true] {
            let retained = std::fs::read_dir(dir.path())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .and_then(parse_log_filename)
                        .map(|parsed| parsed.0 == structured)
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(retained, MAX_FILES_PER_STREAM);
        }
        assert!(dir.path().join("arcana.not-a-date.log").exists());
        assert!(dir.path().join("user-notes.log").exists());
    }

    #[test]
    fn file_cap_preserves_the_current_active_log_ahead_of_future_dated_files() {
        let dir = tempfile::tempdir().expect("temporary logs");
        let today = NaiveDate::from_ymd_opt(2026, 10, 4).unwrap();
        let active_name = format!("arcana.{today}.log");
        let active_path = dir.path().join(active_name);
        let mut active_file = std::fs::File::create(&active_path).expect("create active log");
        active_file
            .write_all(b"before-prune\n")
            .expect("seed active log");

        for offset in 1..=5 {
            let future = (today + chrono::Duration::days(offset)).format("%Y-%m-%d");
            std::fs::write(
                dir.path().join(format!("arcana.{future}.log")),
                "future-dated recognized file",
            )
            .expect("seed recognized future file");
        }

        let summary = prune_at_with_limit(dir.path(), today, MAX_FILES_PER_STREAM);

        assert!(summary.failures.is_empty(), "{:?}", summary.failures);
        assert!(active_path.exists(), "active log path must not be removed");
        active_file
            .write_all(b"after-prune\n")
            .expect("writer still targets the retained active file");
        active_file.flush().expect("flush active log");
        let contents = std::fs::read_to_string(&active_path).expect("read active log");
        assert!(contents.contains("before-prune"));
        assert!(contents.contains("after-prune"));
    }

    #[test]
    fn policy_keeps_current_files_when_legacy_names_share_the_active_date() {
        let dir = tempfile::tempdir().expect("temporary logs");
        let today = NaiveDate::from_ymd_opt(2026, 9, 29).unwrap();
        let active_readable = dir.path().join(format!("arcana.{today}.log"));
        let active_structured = dir.path().join(format!("arcana-events.{today}.jsonl"));
        std::fs::write(&active_readable, "active readable").unwrap();
        std::fs::write(&active_structured, "active structured").unwrap();
        std::fs::write(
            dir.path().join(format!("arcana-.{today}..log")),
            "legacy readable",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(format!("arcana-events-.{today}..jsonl")),
            "legacy structured",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(format!("arcana-{today}.log")),
            "legacy readable",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(format!("arcana-events-{today}.jsonl")),
            "legacy structured",
        )
        .unwrap();

        // A tighter test-only cap makes the same-date tie falsifiable while
        // production still uses the fixed five-file policy.
        let summary = prune_at_with_limit(dir.path(), today, 1);

        assert!(summary.failures.is_empty(), "{:?}", summary.failures);
        assert!(active_readable.exists(), "active readable log was pruned");
        assert!(
            active_structured.exists(),
            "active structured log was pruned"
        );
        assert_eq!(summary.dropped, 4);
    }

    #[test]
    fn active_writer_remains_writable_while_retention_prunes_old_files() {
        let dir = tempfile::tempdir().expect("temporary logs");
        let today = chrono::Utc::now().date_naive();
        let active = dir.path().join(format!("arcana.{today}.log"));
        std::fs::write(&active, "before prune\n").expect("seed active log");
        for offset in 1..=MAX_FILES_PER_STREAM {
            let date = (today - chrono::Duration::days(offset as i64)).format("%Y-%m-%d");
            std::fs::write(dir.path().join(format!("arcana.{date}.log")), "older log\n")
                .expect("seed older log");
        }

        // Hold the production-style append handle open across pruning. The
        // retention pass may delete older recognized files, but must preserve
        // the active stream so its existing writer can continue.
        let mut writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&active)
            .expect("open active log writer");
        let summary = prune_at(dir.path(), today);
        assert!(summary.failures.is_empty(), "{:?}", summary.failures);
        assert_eq!(summary.dropped, 1);
        assert!(active.exists(), "retention removed the active log");

        use std::io::Write;
        writeln!(writer, "after prune").expect("active writer continues after pruning");
        writer.flush().expect("flush active writer");
        let contents = std::fs::read_to_string(&active).expect("read complete active log");
        assert!(contents.contains("before prune\n"));
        assert!(contents.contains("after prune\n"));
    }

    #[test]
    fn startup_cleanup_failure_is_logged_without_panicking() {
        use std::io::Write;
        use std::sync::{Arc, Mutex};

        struct SharedWriter(Arc<Mutex<Vec<u8>>>);

        impl Write for SharedWriter {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0
                    .lock()
                    .expect("captured log output")
                    .extend_from_slice(bytes);
                Ok(bytes.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let not_a_directory = tempfile::NamedTempFile::new().expect("temporary file");
        let output = Arc::new(Mutex::new(Vec::new()));
        let writer_output = Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(move || SharedWriter(Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || prune(not_a_directory.path()));

        let output = String::from_utf8(output.lock().expect("captured output").clone())
            .expect("UTF-8 log output");
        assert!(
            output.contains("log directory is not a plain directory"),
            "failure detail was not logged: {output}"
        );
        assert!(
            output.contains("1 cleanup failure(s)"),
            "cleanup summary was not logged: {output}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn policy_does_not_follow_or_unlink_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().expect("temporary logs");
        let external = tempfile::NamedTempFile::new().expect("external log target");
        let link = dir.path().join("arcana.2026-09-01.log");
        symlink(external.path(), &link).expect("create symlink");

        let summary = prune_at(dir.path(), NaiveDate::from_ymd_opt(2026, 9, 28).unwrap());

        assert!(summary.failures.is_empty());
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(external.path().exists());
    }
}
