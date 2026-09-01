// Structured logging initialisation — Phase 7.3 upgrade.
//
// Outputs to:
//   1. Console (stderr) — compact format, colour-coded
//   2. File — ~/.autodev/logs/autodev.YYYY-MM-DD (daily rolling rotation)
//             A new file is created at midnight; each day's file stays in place.
//             Old format was a single autodev-YYYY-MM-DD.log appended to forever;
//             the rolling appender creates one fresh file per calendar day.
//
// Returns an Option<WorkerGuard> that MUST be stored in main() and kept alive
// for the entire lifetime of the process.  Dropping it flushes the background
// log-writer thread and closes the file.  Dropping it early = silent log loss.
//
// Call once at app startup before any other code.

use std::io::Write;

use tracing_appender::non_blocking::WorkerGuard;

/// Redacts every byte on its way to the log file.
///
/// This wraps the **appender**, not the call sites, because the leak that
/// motivated it did not come from a call site anyone would have thought to
/// audit: a `tracing` DEBUG record emitted from the sidecar client wrote a
/// 164-character `sk-` credential into `~/.autodev/logs/` in plain text, with
/// the field name `api_key` beside it. Redacting at each `info!`/`debug!` would
/// require every author of every log line to remember; redacting the writer
/// cannot be forgotten.
///
/// `redact_text` is used rather than the stricter `redact_value_text`: log
/// lines carry commit hashes, file digests and base64 payloads that are useful
/// in a diagnostic and are not secrets.
struct RedactingWriter<W: Write> {
    inner: W,
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        let redacted = autodev_core::security::redaction::redact_text(&text);
        self.inner.write_all(redacted.as_bytes())?;
        // Report the caller's buffer as fully consumed. Redaction changes the
        // byte count, and a short write here would make `tracing` retry the
        // tail of a line it has already emitted.
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init() -> Option<WorkerGuard> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("autodev_lib=debug,info"));

    // Console layer — human-readable, compact
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .compact();

    // Resolve log directory.
    // Uses env vars directly (not autodev_dir()) so logging works before the DB
    // is ready and before any Tauri state is set up.
    let log_dir = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .and_then(|home| {
            let dir = std::path::PathBuf::from(home).join(".autodev").join("logs");
            match std::fs::create_dir_all(&dir) {
                Ok(_) => Some(dir),
                Err(e) => {
                    eprintln!("[AutoDev] Failed to create log dir {}: {e}", dir.display());
                    None
                }
            }
        });

    match log_dir {
        Some(dir) => {
            // Phase 7.3 — daily rolling file appender (tracing-appender 0.2).
            //
            // Files are named:  ~/.autodev/logs/autodev-YYYY-MM-DD.log
            // • Rotation happens at midnight (UTC).
            // • Today's file is opened in append mode so multiple restarts on the
            //   same calendar day all write to the same file.
            // • Yesterday's (and earlier) files stay on disk — delete with:
            //     Remove-Item ~/.autodev/logs/autodev-*.log -OlderThan (Get-Date).AddDays(-7)
            //
            // The non_blocking wrapper moves file I/O to a background thread so
            // the hot log path never blocks an async task or UI event.
            //
            // NOTE: builder() requires tracing-appender 0.2.3+. Cargo.lock pins 0.2.5.
            let file_appender = tracing_appender::rolling::RollingFileAppender::builder()
                .rotation(tracing_appender::rolling::Rotation::DAILY)
                .filename_prefix("autodev-")
                .filename_suffix(".log")
                .build(&dir)
                .unwrap_or_else(|e| {
                    eprintln!("[AutoDev] Log appender builder failed ({e}), falling back to default rotation");
                    tracing_appender::rolling::daily(&dir, "autodev")
                });
            // Redaction boundary (M-40, P1-21). Wrapping the appender means
            // every log line is redacted regardless of which subsystem emitted
            // it — see `RedactingWriter`.
            let (non_blocking, guard) = tracing_appender::non_blocking(RedactingWriter {
                inner: file_appender,
            });

            let file_layer = fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .compact();

            tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .init();

            tracing::info!(
                "AutoDev logging initialised — daily rotation active, log dir: {}",
                dir.display()
            );
            Some(guard)
        }

        None => {
            // No writable home directory found — console only.
            tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .init();
            tracing::info!("AutoDev logging initialised — console only (no writable log dir)");
            None
        }
    }
}
