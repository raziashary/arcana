use crate::output::OutputContext;
use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;

#[derive(Subcommand)]
pub enum SystemCmd {
    /// Show system and sidecar status
    Status,
    /// Show sidecar connectivity
    Sidecars,
    /// Show recent log entries
    Logs(LogsArgs),
    /// Backup ~/.autodev/ to a zip archive
    Backup(BackupArgs),
    /// Restore ~/.autodev/ from a zip archive
    Restore(RestoreArgs),
}

#[derive(Args)]
pub struct LogsArgs {
    /// Number of lines to show
    #[arg(short, long, default_value = "50")]
    pub lines: usize,

    /// Service filter: orchestrator, worker, app
    #[arg(short, long)]
    pub service: Option<String>,
}

#[derive(Args)]
pub struct BackupArgs {
    /// Output path (default: ~/.autodev/backup-<timestamp>.zip)
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Args)]
pub struct RestoreArgs {
    /// Path to backup zip
    pub path: String,

    /// Overwrite existing data without prompting
    #[arg(long)]
    pub force: bool,
}

pub async fn run(cmd: SystemCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        SystemCmd::Status => status(ctx).await,
        SystemCmd::Sidecars => sidecars(ctx).await,
        SystemCmd::Logs(args) => logs(args, ctx).await,
        SystemCmd::Backup(args) => backup(args, ctx),
        SystemCmd::Restore(args) => restore(args, ctx),
    }
}

async fn status(ctx: OutputContext) -> Result<()> {
    ctx.info("Checking system status…");

    // Probe orchestrator
    let orchestrator_ok = probe_http(&ctx.orchestrator_url, "/health").await;

    // Probe common sidecar ports
    let redis_ok = probe_tcp("127.0.0.1:6379").await;
    let postgres_ok = probe_tcp("127.0.0.1:5432").await;
    let mcp_ok = probe_tcp("127.0.0.1:3100").await;

    // AutoDev DB presence
    let autodev_db = autodev_dir_exists();

    let headers = ["Service", "Status", "Details"];
    let rows = vec![
        vec![
            "core-orchestrator".to_string(),
            bool_status(orchestrator_ok),
            ctx.orchestrator_url.clone(),
        ],
        vec![
            "redis/memurai".to_string(),
            bool_status(redis_ok),
            "127.0.0.1:6379".into(),
        ],
        vec![
            "postgres".to_string(),
            bool_status(postgres_ok),
            "127.0.0.1:5432".into(),
        ],
        vec![
            "mcp-server".to_string(),
            bool_status(mcp_ok),
            "127.0.0.1:3100".into(),
        ],
        vec![
            "autodev-db".to_string(),
            bool_status(autodev_db),
            "~/.autodev/".into(),
        ],
    ];

    ctx.print_table(&headers, rows);
    Ok(())
}

async fn sidecars(ctx: OutputContext) -> Result<()> {
    ctx.info("Probing sidecar connectivity…");

    let checks: &[(&str, &str)] = &[
        ("Redis/Memurai", "127.0.0.1:6379"),
        ("Postgres", "127.0.0.1:5432"),
        ("MCP Server", "127.0.0.1:3100"),
        ("Orchestrator", "127.0.0.1:4000"),
    ];

    let mut rows = vec![];
    for (name, addr) in checks {
        let ok = probe_tcp(addr).await;
        rows.push(vec![name.to_string(), bool_status(ok), addr.to_string()]);
    }

    ctx.print_table(&["Service", "Status", "Address"], rows);
    Ok(())
}

async fn logs(args: LogsArgs, ctx: OutputContext) -> Result<()> {
    let log_paths = find_log_files(args.service.as_deref());

    if log_paths.is_empty() {
        ctx.info("No log files found in ~/.autodev/logs/");
        return Ok(());
    }

    for path in &log_paths {
        if !ctx.quiet {
            println!("{}", format!("=== {} ===", path.display()).cyan().bold());
        }
        match read_tail(path, args.lines) {
            Ok(content) => println!("{content}"),
            Err(e) => ctx.warn(&format!("Cannot read {}: {e}", path.display())),
        }
    }
    Ok(())
}

fn backup(args: BackupArgs, ctx: OutputContext) -> Result<()> {
    let autodev_dir = get_autodev_dir()?;

    let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let output = args.output.unwrap_or_else(|| {
        autodev_dir
            .join(format!("backup-{timestamp}.zip"))
            .display()
            .to_string()
    });

    ctx.info(&format!("Backing up ~/.autodev/ to {output}…"));

    // Walk ~/.autodev/ and collect .db and .toml files
    let entries: Vec<_> = walkdir_collect(&autodev_dir);

    if entries.is_empty() {
        ctx.warn("No files found in ~/.autodev/ to backup.");
        return Ok(());
    }

    // Write zip
    let file = std::fs::File::create(&output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for entry in &entries {
        let rel = entry
            .strip_prefix(&autodev_dir)
            .unwrap_or(entry)
            .display()
            .to_string();
        if let Ok(data) = std::fs::read(entry) {
            zip.start_file(&rel, options)?;
            use std::io::Write;
            zip.write_all(&data)?;
        }
    }
    zip.finish()?;

    ctx.success(&format!(
        "Backup saved to {output} ({} files)",
        entries.len()
    ));
    Ok(())
}

fn restore(args: RestoreArgs, ctx: OutputContext) -> Result<()> {
    let autodev_dir = get_autodev_dir()?;

    if !std::path::Path::new(&args.path).exists() {
        ctx.error(&format!("Backup file not found: {}", args.path));
        return Ok(());
    }

    if !args.force {
        ctx.warn("This will overwrite existing ~/.autodev/ data.");
        ctx.warn("Re-run with --force to confirm.");
        return Ok(());
    }

    ctx.info(&format!("Restoring from {}…", args.path));

    let file = std::fs::File::open(&args.path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let count = archive.len();

    for i in 0..count {
        let mut entry = archive.by_index(i)?;
        let dest = autodev_dir.join(entry.name());
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&dest)?;
        std::io::copy(&mut entry, &mut out)?;
    }

    ctx.success(&format!("Restored {count} files to ~/.autodev/"));
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

async fn probe_http(base_url: &str, path: &str) -> bool {
    let url = format!("{base_url}{path}");
    let Ok(client) = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    else {
        return false;
    };
    client
        .get(&url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn probe_tcp(addr: &str) -> bool {
    tokio::net::TcpStream::connect(addr).await.is_ok()
}

fn bool_status(ok: bool) -> String {
    if ok {
        "up".green().to_string()
    } else {
        "down".red().to_string()
    }
}

fn autodev_dir_exists() -> bool {
    get_autodev_dir().map(|d| d.exists()).unwrap_or(false)
}

fn get_autodev_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(std::path::PathBuf::from(home).join(".autodev"))
}

fn find_log_files(service: Option<&str>) -> Vec<std::path::PathBuf> {
    let Ok(dir) = get_autodev_dir() else {
        return vec![];
    };
    let logs_dir = dir.join("logs");
    if !logs_dir.exists() {
        return vec![];
    }

    let Ok(entries) = std::fs::read_dir(&logs_dir) else {
        return vec![];
    };

    entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "log").unwrap_or(false))
        .filter(|p| {
            if let Some(svc) = service {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains(svc))
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect()
}

fn read_tail(path: &std::path::Path, lines: usize) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(lines);
    Ok(all[start..].join("\n"))
}

fn walkdir_collect(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    let mut result = vec![];
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            result.extend(walkdir_collect(&path));
        } else {
            result.push(path);
        }
    }
    result
}
