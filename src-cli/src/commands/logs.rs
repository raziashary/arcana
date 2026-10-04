use crate::output::OutputContext;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;

#[derive(Subcommand)]
pub enum LogsCmd {
    /// Tail log files from ~/.arcana/logs/
    View(ViewArgs),
    /// List available log files
    List,
    /// Clear old log files (older than N days)
    Clear(ClearArgs),
    /// Export logs to a file
    Export(ExportArgs),
}

#[derive(Args)]
pub struct ViewArgs {
    /// Component filter: orchestrator, worker, app, or a partial filename match
    #[arg(short, long)]
    pub component: Option<String>,

    /// Number of lines to tail
    #[arg(short = 'n', long, default_value = "100")]
    pub lines: usize,

    /// Follow (stream) log output
    #[arg(short, long)]
    pub follow: bool,
}

#[derive(Args)]
pub struct ClearArgs {
    /// Delete log files older than this many days
    #[arg(short, long, default_value = "7")]
    pub days: u64,

    /// Skip confirmation prompt
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output file path
    pub output: String,

    /// Component filter
    #[arg(short, long)]
    pub component: Option<String>,
}

pub async fn run(cmd: LogsCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        LogsCmd::View(args) => view(args, ctx).await,
        LogsCmd::List => list(ctx),
        LogsCmd::Clear(args) => clear(args, ctx),
        LogsCmd::Export(args) => export(args, ctx),
    }
}

async fn view(args: ViewArgs, ctx: OutputContext) -> Result<()> {
    let log_dir = get_log_dir()?;
    let files = find_logs(&log_dir, args.component.as_deref());

    if files.is_empty() {
        ctx.info(&format!("No log files found in {}", log_dir.display()));
        return Ok(());
    }

    if args.follow {
        // Poll-based follow mode: print tail then re-check every 500ms
        ctx.info(&format!(
            "Following {} log file(s). Press Ctrl+C to stop.",
            files.len()
        ));
        let mut last_sizes: Vec<u64> = files
            .iter()
            .map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0))
            .collect();

        loop {
            for (i, path) in files.iter().enumerate() {
                let meta = std::fs::metadata(path)?;
                let new_size = meta.len();
                if new_size > last_sizes[i] {
                    // Read only new bytes
                    let mut file = std::fs::File::open(path)?;
                    use std::io::{Read, Seek, SeekFrom};
                    file.seek(SeekFrom::Start(last_sizes[i]))?;
                    let mut buf = String::new();
                    file.read_to_string(&mut buf)?;
                    print!("{}", buf);
                    last_sizes[i] = new_size;
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    } else {
        for path in &files {
            if !ctx.quiet {
                println!(
                    "{}",
                    format!(
                        "=== {} ===",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    )
                    .cyan()
                    .bold()
                );
            }
            match read_tail(path, args.lines) {
                Ok(content) => println!("{content}"),
                Err(e) => ctx.warn(&format!("Cannot read {}: {e}", path.display())),
            }
        }
    }

    Ok(())
}

fn list(ctx: OutputContext) -> Result<()> {
    let log_dir = get_log_dir()?;
    let files = find_logs(&log_dir, None);

    if files.is_empty() {
        ctx.info(&format!("No log files in {}", log_dir.display()));
        return Ok(());
    }

    let headers = ["File", "Size", "Modified"];
    let rows: Vec<Vec<String>> = files
        .iter()
        .map(|p| {
            let meta = std::fs::metadata(p);
            let size = meta
                .as_ref()
                .map(|m| format_bytes(m.len()))
                .unwrap_or_else(|_| "-".into());
            let modified = meta
                .as_ref()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| format_unix_time(d.as_secs()))
                .unwrap_or_else(|| "-".into());
            vec![
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                size,
                modified,
            ]
        })
        .collect();

    ctx.print_table(&headers, rows);
    ctx.info(&format!("\nLog directory: {}", log_dir.display()));
    Ok(())
}

fn clear(args: ClearArgs, ctx: OutputContext) -> Result<()> {
    let log_dir = get_log_dir()?;
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(args.days * 86400))
        .unwrap_or(std::time::UNIX_EPOCH);

    let candidates: Vec<_> = find_logs(&log_dir, None)
        .into_iter()
        .filter(|p| {
            std::fs::metadata(p)
                .and_then(|m| m.modified())
                .map(|t| t < cutoff)
                .unwrap_or(false)
        })
        .collect();

    if candidates.is_empty() {
        ctx.info(&format!("No log files older than {} days.", args.days));
        return Ok(());
    }

    ctx.info(&format!(
        "Found {} log file(s) older than {} days.",
        candidates.len(),
        args.days
    ));
    for p in &candidates {
        ctx.info(&format!(
            "  {}",
            p.file_name().unwrap_or_default().to_string_lossy()
        ));
    }

    if !args.force {
        ctx.warn("Re-run with --force to delete these files.");
        return Ok(());
    }

    let mut deleted = 0usize;
    for p in &candidates {
        if std::fs::remove_file(p).is_ok() {
            deleted += 1;
        }
    }
    ctx.success(&format!("Deleted {deleted} log file(s)."));
    Ok(())
}

fn export(args: ExportArgs, ctx: OutputContext) -> Result<()> {
    let log_dir = get_log_dir()?;
    let files = find_logs(&log_dir, args.component.as_deref());

    if files.is_empty() {
        ctx.error("No matching log files found.");
        return Ok(());
    }

    let mut combined = String::new();
    for path in &files {
        let header = format!(
            "=== {} ===\n",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        combined.push_str(&header);
        match std::fs::read_to_string(path) {
            Ok(content) => combined.push_str(&content),
            Err(e) => combined.push_str(&format!("[Error reading file: {e}]\n")),
        }
        combined.push('\n');
    }

    std::fs::write(&args.output, &combined)?;
    ctx.success(&format!(
        "Exported {} log file(s) to {}",
        files.len(),
        args.output
    ));
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn get_log_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    let dir = std::path::PathBuf::from(home).join(".arcana").join("logs");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn find_logs(dir: &std::path::Path, component: Option<&str>) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|extension| extension == "log" || extension == "jsonl")
        })
        .filter(|p| {
            if let Some(comp) = component {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains(comp))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();
    // Sort by modification time descending (newest first)
    files.sort_by(|a, b| {
        let mt_a = std::fs::metadata(a).and_then(|m| m.modified()).ok();
        let mt_b = std::fs::metadata(b).and_then(|m| m.modified()).ok();
        mt_b.cmp(&mt_a)
    });
    files
}

fn read_tail(path: &std::path::Path, lines: usize) -> Result<String> {
    use std::io::{BufRead, BufReader};

    if lines == 0 {
        return Ok(String::new());
    }

    let file = std::fs::File::open(path)?;
    let mut tail = std::collections::VecDeque::with_capacity(lines.min(1024));
    for line in BufReader::new(file).lines() {
        let line = line?;
        if tail.len() == lines {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    Ok(tail.into_iter().collect::<Vec<_>>().join("\n"))
}

fn format_bytes(b: u64) -> String {
    if b >= 1_048_576 {
        format!("{:.1} MB", b as f64 / 1_048_576.0)
    } else if b >= 1_024 {
        format!("{:.1} KB", b as f64 / 1_024.0)
    } else {
        format!("{b} B")
    }
}

fn format_unix_time(secs: u64) -> String {
    // Simple YYYY-MM-DD HH:MM from seconds since epoch
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    // Days since 1970-01-01
    let y400 = days / 146097;
    let rem = days % 146097;
    let y100 = rem / 36524;
    let rem = rem % 36524;
    let y4 = rem / 1461;
    let rem = rem % 1461;
    let y1 = rem / 365;
    let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1970;
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mdays = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut doy = rem % 365 + if y1 == 4 { 365 } else { 0 };
    let mut month = 1u64;
    for md in &mdays {
        if doy < *md {
            break;
        }
        doy -= md;
        month += 1;
    }
    let day = doy + 1;
    format!("{year}-{month:02}-{day:02} {h:02}:{m:02}:{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_listing_includes_readable_and_structured_streams() {
        let dir = std::env::temp_dir().join(format!("arcana-cli-logs-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create isolated log directory");
        std::fs::write(dir.join("arcana.2026-10-02.log"), "readable\n")
            .expect("write readable log");
        std::fs::write(
            dir.join("arcana-events.2026-10-02.jsonl"),
            "{\"fields\":{\"event_type\":\"provider_call\"}}\n",
        )
        .expect("write structured log");
        std::fs::write(dir.join("notes.txt"), "ignore\n").expect("write unrelated file");

        let files = find_logs(&dir, None);
        let names = files
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>();
        assert!(names.contains(&"arcana.2026-10-02.log"));
        assert!(names.contains(&"arcana-events.2026-10-02.jsonl"));
        assert!(!names.contains(&"notes.txt"));

        std::fs::remove_dir_all(&dir).expect("remove isolated log directory");
    }

    #[test]
    fn log_tail_retains_only_the_requested_last_lines() {
        let path =
            std::env::temp_dir().join(format!("arcana-cli-tail-{}.log", uuid::Uuid::new_v4()));
        let content = (0..50_000)
            .map(|line| format!("event-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content).expect("write large isolated log");

        assert_eq!(
            read_tail(&path, 3).expect("read bounded log tail"),
            "event-49997\nevent-49998\nevent-49999"
        );
        assert_eq!(read_tail(&path, 0).expect("read empty tail"), "");

        std::fs::remove_file(&path).expect("remove isolated log file");
    }
}
