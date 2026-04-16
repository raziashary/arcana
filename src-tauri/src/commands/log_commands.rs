// Log reading commands — exposes ~/.autodev/logs/ to the frontend Logs tab.
// Provides listing of available log files (by date) and content reading.
// Log files are plain-text, one tracing event per line.

use crate::errors::Result;
use crate::errors::AutoDevError;

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
            e.path().extension()
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

/// Read the content of a log file for a given date (e.g. "2026-04-15").
/// Pass an empty string to read today's log.
/// Returns empty string if the file does not exist yet.
#[tauri::command]
pub fn read_log_file(date: String) -> Result<String> {
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
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| AutoDevError::Agent(e.to_string()))?;
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
