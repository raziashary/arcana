use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;
mod noninteractive;
mod output;
mod repl;

use commands::{analytics, config, consent, fleet, logs, plan, plugin, system, task, worker};
use output::{OutputContext, OutputFormat};

#[derive(Parser)]
#[command(
    name = "arcanactl",
    version = "1.0.0",
    author = "Arcana",
    about = "Arcana headless CLI for automation, scripting, and CI/CD",
    long_about = None,
)]
pub struct Cli {
    /// Path to config file (default: ~/.arcana/config.toml)
    #[arg(long, global = true, env = "ARCANACTL_CONFIG")]
    pub config: Option<String>,

    /// Working directory for agent operations
    #[arg(long, global = true, env = "ARCANACTL_WORKING_DIR")]
    pub working_dir: Option<String>,

    /// LLM provider override (e.g. google, anthropic, openai)
    #[arg(long, global = true, env = "ARCANA_PROVIDER")]
    pub provider: Option<String>,

    /// Model override (e.g. claude-sonnet-4-6)
    #[arg(long, global = true, env = "ARCANACTL_MODEL")]
    pub model: Option<String>,

    /// Disable interactive prompts
    #[arg(long, global = true, env = "ARCANACTL_NON_INTERACTIVE")]
    pub non_interactive: bool,

    /// Output format: table, json, plain
    #[arg(long, global = true, default_value = "table", env = "ARCANACTL_OUTPUT")]
    pub output_format: String,

    /// Suppress all non-essential output
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Enable verbose/debug output
    #[arg(long, short, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage tasks
    Task {
        #[command(subcommand)]
        cmd: task::TaskCmd,
    },
    /// Manage workers
    Worker {
        #[command(subcommand)]
        cmd: worker::WorkerCmd,
    },
    /// Run a task through the production worker Fleet
    Fleet {
        #[command(subcommand)]
        cmd: fleet::FleetCmd,
    },
    /// Manage persisted workspace egress consent
    Consent {
        #[command(subcommand)]
        cmd: consent::ConsentCmd,
    },
    /// View and edit configuration
    Config {
        #[command(subcommand)]
        cmd: config::ConfigCmd,
    },
    /// Manage plans and steps
    Plan {
        #[command(subcommand)]
        cmd: plan::PlanCmd,
    },
    /// System status, logs, and maintenance
    System {
        #[command(subcommand)]
        cmd: system::SystemCmd,
    },
    /// View analytics and cost data
    Analytics {
        #[command(subcommand)]
        cmd: analytics::AnalyticsCmd,
    },
    /// View and manage log files
    Logs {
        #[command(subcommand)]
        cmd: logs::LogsCmd,
    },
    /// Manage plugins and the worker marketplace
    Plugin {
        #[command(subcommand)]
        cmd: plugin::PluginCmd,
    },
    /// Worker setup token management
    WorkerSetup {
        #[command(subcommand)]
        cmd: plugin::WorkerSetupCmd,
    },
    /// Launch interactive REPL
    Repl,
}

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(exit_code) = arcana_core::platform::process_sandbox::run_bridge_entrypoint() {
        std::process::exit(exit_code);
    }
    let cli = Cli::parse();

    // Prevent provider credentials exported by the invoking shell from being
    // inherited by agent-spawned commands. Providers obtain keys from the OS
    // vault; only variable names are safe to report here.
    let removed_credentials = arcana_core::security::env_scrub::scrub_process_env();
    if !removed_credentials.is_empty() {
        eprintln!(
            "[Arcana] Removed {} credential-shaped variable(s) from the process environment: {}",
            removed_credentials.len(),
            removed_credentials.join(", ")
        );
    }

    let _log_guard = init_tracing(cli.verbose, cli.quiet);
    arcana_core::observability::log_retention::prune_default();

    // Declare this host's event policy before anything in the agent runtime can
    // emit. `arcanactl` has no window to deliver events to, and discarding them
    // is the correct behaviour for a headless process rather than an error to
    // report on every event. Stating it explicitly means a future host that
    // *should* render events (a TUI, a log stream) has one obvious place to
    // change, instead of inheriting the default by accident.
    arcana_core::events::install_sink(std::sync::Arc::new(arcana_core::events::NullEventSink));

    let cfg = load_config(&cli)?;
    let fmt = resolve_output_format(&cli, &cfg);

    let ctx = OutputContext {
        format: fmt,
        quiet: cli.quiet,
        non_interactive: cli.non_interactive,
        provider: cli
            .provider
            .clone()
            .or_else(|| cfg.core.as_ref().and_then(|c| c.default_provider.clone())),
        model: cli.model.clone(),
        working_dir: cli
            .working_dir
            .clone()
            .or_else(|| cfg.core.as_ref().and_then(|c| c.working_dir.clone())),
    };

    // Resolve effective non-interactive mode
    let effective_non_interactive = noninteractive::is_non_interactive(cli.non_interactive);

    // The run/audit ledger (P1-18). Recorded per invocation, and closed with
    // the invocation's real outcome below, so a failed command is visible in
    // the ledger as failed rather than merely absent.
    //
    // The ledger is an audit trail, not a dependency: if it cannot be opened,
    // the command still runs.
    let ledger = arcana_core::db::runs::RunsDb::open().ok();
    let run_id = ledger.as_ref().and_then(|db| {
        let _ = db.reap_abandoned();
        db.begin(arcana_core::db::runs::kind::CLI, Some("arcanactl"))
            .ok()
    });

    let dispatched = dispatch(cli.command, ctx, effective_non_interactive).await;

    if let (Some(db), Some(id)) = (ledger.as_ref(), run_id.as_ref()) {
        let result = if dispatched.is_ok() {
            arcana_core::db::runs::outcome::OK
        } else {
            arcana_core::db::runs::outcome::FAILED
        };
        if let Err(e) = db.finish(id, result) {
            tracing::warn!("[runs] could not close run {id}: {e}");
        }
    }

    dispatched
}

async fn dispatch(
    command: Option<Commands>,
    ctx: OutputContext,
    effective_non_interactive: bool,
) -> Result<()> {
    match command {
        None => {
            if effective_non_interactive {
                Cli::parse_from(["arcanactl", "--help"]);
            } else {
                repl::run(ctx).await?;
            }
        }
        Some(Commands::Task { cmd }) => task::run(cmd, ctx).await?,
        Some(Commands::Worker { cmd }) => worker::run(cmd, ctx).await?,
        Some(Commands::Fleet { cmd }) => fleet::run(cmd, ctx).await?,
        Some(Commands::Consent { cmd }) => consent::run(cmd, ctx)?,
        Some(Commands::Config { cmd }) => config::run(cmd, ctx).await?,
        Some(Commands::Plan { cmd }) => plan::run(cmd, ctx).await?,
        Some(Commands::System { cmd }) => system::run(cmd, ctx).await?,
        Some(Commands::Analytics { cmd }) => analytics::run(cmd, ctx).await?,
        Some(Commands::Logs { cmd }) => logs::run(cmd, ctx).await?,
        Some(Commands::Plugin { cmd }) => plugin::run_plugin_cmd(cmd, ctx).await?,
        Some(Commands::WorkerSetup { cmd }) => plugin::run_worker_setup(cmd, ctx).await?,
        Some(Commands::Repl) => repl::run(ctx).await?,
    }

    Ok(())
}

fn init_tracing(verbose: bool, quiet: bool) -> arcana_core::observability::logging::LoggingGuards {
    let filter = tracing_filter(verbose, quiet);
    arcana_core::observability::logging::init_with_filter(filter)
}

fn tracing_filter(verbose: bool, quiet: bool) -> EnvFilter {
    if verbose {
        EnvFilter::new("debug")
    } else if quiet {
        // Shared logging adds a telemetry-only DEBUG directive for the
        // structured stream. Keep the host filter itself at ERROR so telemetry
        // INFO does not leak to quiet-mode console or readable output.
        EnvFilter::new("error")
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("warn,arcana::telemetry=info"))
    }
}

#[cfg(test)]
mod tracing_filter_tests {
    use super::*;

    #[test]
    fn quiet_filter_suppresses_telemetry_from_console_and_readable_layers() {
        assert_eq!(tracing_filter(false, true).to_string(), "error");
    }
}

fn load_config(cli: &Cli) -> Result<arcana_shared::config::CliConfigFile> {
    if let Some(path) = &cli.config {
        let content = std::fs::read_to_string(path)?;
        let cfg: arcana_shared::config::CliConfigFile = toml::from_str(&content)?;
        return Ok(cfg);
    }
    Ok(arcana_shared::config::CliConfigFile::load()?)
}

fn resolve_output_format(cli: &Cli, cfg: &arcana_shared::config::CliConfigFile) -> OutputFormat {
    let raw = if cli.output_format != "table" {
        cli.output_format.clone()
    } else {
        cfg.output_format()
    };
    match raw.as_str() {
        "json" => OutputFormat::Json,
        "plain" => OutputFormat::Plain,
        _ => OutputFormat::Table,
    }
}
