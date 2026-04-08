// Structured logging initialisation using tracing + tracing-subscriber.
// Call init() once at app startup in main.rs.

use tracing::Level;
use tracing_subscriber::{fmt, EnvFilter};

pub fn init() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("autodev=debug,warn"));

    fmt::Subscriber::builder()
        .with_env_filter(env_filter)
        .with_max_level(Level::DEBUG)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .compact()
        .init();

    tracing::info!("AutoDev logging initialised");
}
