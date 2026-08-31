// Log reading commands — exposes ~/.autodev/logs/ to the frontend Logs tab.
// Provides listing of available log files (by date) and content reading.
// Log files are plain-text, one tracing event per line.
//
// Tail truncation: `read_log_file` returns only the last N lines by default
// (10 000) so the frontend stays responsive when a daily log grows large
// during heavy fleet activity. The full file is still readable via
// `read_log_file_full` for download / export. See Logs.tsx for the
// truncation banner UX.

use crate::errors::AutoDevError;
use crate::errors::Result;

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

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn logs_dir() -> Option<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(std::path::PathBuf::from(home).join(".autodev").join("logs"))
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
    let mut dates: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| AutoDevError::Agent(e.to_string()))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x == "log")
                .unwrap_or(false)
        })
        .filter_map(|e| {
            // Extract date from filename: autodev-YYYY-MM-DD.log
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_prefix("autodev-")
                .and_then(|s| s.strip_suffix(".log"))
                .map(|s| s.to_string())
        })
        .collect();

    // Sort descending (newest first)
    dates.sort_by(|a, b| b.cmp(a));
    Ok(dates)
}

/// Read the **tail** of a log file for a given date (e.g. "2026-04-15").
/// Pass an empty string to read today's log.
/// Only the last `tail_lines` (default 3 000) are returned over IPC so the
/// frontend stays responsive even when a single-day log grows large.
/// The full file can be fetched via `read_log_file_full` for download.
#[tauri::command]
pub fn read_log_file(date: String, tail_lines: Option<usize>) -> Result<LogFileResult> {
    let Some(dir) = logs_dir() else {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    };

    let date_str = if date.is_empty() {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    } else {
        date
    };

    let path = dir.join(format!("autodev-{date_str}.log"));
    if !path.exists() {
        return Ok(LogFileResult {
            content: String::new(),
            total_lines: 0,
            truncated: false,
            full_size_bytes: 0,
        });
    }

    let limit = tail_lines.unwrap_or(3_000);
    let full_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    let content = std::fs::read_to_string(&path)
        .map_err(|e| AutoDevError::Agent(format!("Failed to read log file: {e}")))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let truncated = total > limit;

    let visible = if truncated {
        lines
            .into_iter()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
    } else {
        lines
    };

    Ok(LogFileResult {
        content: visible.join("\n"),
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

    let date_str = if date.is_empty() {
        chrono::Local::now().format("%Y-%m-%d").to_string()
    } else {
        date
    };

    let path = dir.join(format!("autodev-{date_str}.log"));
    if !path.exists() {
        return Ok(String::new());
    }

    std::fs::read_to_string(&path)
        .map_err(|e| AutoDevError::Agent(format!("Failed to read log file: {e}")))
}

/// Delete all .log files in ~/.autodev/logs/.
/// Returns the number of files deleted.
#[tauri::command]
pub fn clear_log_files() -> Result<usize> {
    let Some(dir) = logs_dir() else {
        return Ok(0);
    };
    if !dir.exists() {
        return Ok(0);
    }
    let entries = std::fs::read_dir(&dir).map_err(|e| AutoDevError::Agent(e.to_string()))?;
    let mut deleted = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) == Some("log") {
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
    let contents = if full {
        read_log_file_full(date)?
    } else {
        read_log_file(date, tail_lines)?.content
    };

    let dest = std::path::PathBuf::from(&dest_path);
    if dest.as_os_str().is_empty() {
        return Err(AutoDevError::Agent("No destination path given".into()));
    }
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return Err(AutoDevError::Agent(format!(
                "Destination folder does not exist: {}",
                parent.display()
            )));
        }
    }

    std::fs::write(&dest, contents.as_bytes())
        .map_err(|e| AutoDevError::Agent(format!("Failed to write {}: {e}", dest.display())))?;

    Ok(dest.to_string_lossy().into_owned())
}
