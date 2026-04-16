// Structured logging initialisation using tracing + tracing-subscriber.
// Outputs to:
//   1. Console (stderr) — compact format, colour-coded
//   2. File — ~/.autodev/logs/autodev-YYYY-MM-DD.log (plain text, appended)
// Call init() once at app startup in main.rs before anything else.

use std::sync::Mutex;
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("autodev_lib=debug,info"));

    // Console layer — human-readable, compact
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .compact();

    // File layer — ~/.autodev/logs/autodev-YYYY-MM-DD.log
    // Mutex<File> implements MakeWriter<'_> in tracing-subscriber 0.3
    // Uses env vars directly (not autodev_dir()) so logging works before DB is ready.
    let file_writer: Option<Mutex<std::fs::File>> = (|| -> Option<_> {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .ok()?;
        let log_dir = std::path::PathBuf::from(home).join(".autodev").join("logs");
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("[AutoDev] Failed to create log dir {}: {e}", log_dir.display());
            return None;
        }
        let date = chrono::Local::now().format("%Y-%m-%d");
        let log_file = log_dir.join(format!("autodev-{date}.log"));
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
            .map_err(|e| eprintln!("[AutoDev] Failed to open log file {}: {e}", log_file.display()))
            .ok()
            .map(Mutex::new)
    })();

    match file_writer {
        Some(writer) => {
            let file_layer = fmt::layer()
                .with_writer(writer)
                .with_ansi(false)
                .with_target(true)
                .compact();
            tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .init();
        }
        None => {
            tracing_subscriber::registry()
                .with(env_filter)
                .with(console_layer)
                .init();
        }
    }

    tracing::info!("AutoDev logging initialised — file logging active: {}",
        std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))
            .map(|h| format!("{h}\\.autodev\\logs\\"))
            .unwrap_or_else(|_| "(no home dir)".to_string())
    );
}
