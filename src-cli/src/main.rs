use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt};

mod commands;
mod noninteractive;
mod output;
mod repl;

use commands::{agent, analytics, config, logs, plan, plugin, system, task, worker};
use output::{OutputFormat, OutputContext};

#[derive(Parser)]
#[command(
    name = "airsctl",
    version = "1.0.0",
    author = "AIRS Foundry",
    about = "AutoDev headless CLI for automation, scripting, and CI/CD",
    long_about = None,
)]
pub struct Cli {
    /// Path to config file (default: ~/.airs/config.toml)
    #[arg(long, global = true, env = "AIRSCTL_CONFIG")]
    pub config: Option<String>,

    /// Working directory for agent operations
    #[arg(long, global = true, env = "AIRSCTL_WORKING_DIR")]
    pub working_dir: Option<String>,

    /// LLM provider override (e.g. anthropic, openai, ollama)
    #[arg(long, global = true, env = "AIRSDEV_PROVIDER")]
    pub provider: Option<String>,

    /// Model override (e.g. claude-sonnet-4-6)
    #[arg(long, global = true, env = "AIRSCTL_MODEL")]
    pub model: Option<String>,

    /// Disable interactive prompts
    #[arg(long, global = true, env = "AIRSCTL_NON_INTERACTIVE")]
    pub non_interactive: bool,

    /// Output format: table, json, plain
    #[arg(long, global = true, default_value = "table", env = "AIRSCTL_OUTPUT")]
    pub output_format: String,

    /// Suppress all non-essential output
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Enable verbose/debug output
    #[arg(long, short, global = true)]
    pub verbose: bool,

    /// Core-orchestrator base URL
    #[arg(long, global = true, env = "AIRSCTL_ORCHESTRATOR_URL")]
    pub orchestrator_url: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage and run AI agents
    Agent {
        #[command(subcommand)]
        cmd: agent::AgentCmd,
    },
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
    let cli = Cli::parse();

    init_tracing(cli.verbose, cli.quiet);

    let cfg = load_config(&cli)?;
    let fmt = resolve_output_format(&cli, &cfg);
    let orchestrator_url = resolve_orchestrator_url(&cli, &cfg);

    let ctx = OutputContext {
        format: fmt,
        quiet: cli.quiet,
        non_interactive: cli.non_interactive,
        orchestrator_url: orchestrator_url.clone(),
        provider: cli.provider.clone().or_else(|| {
            cfg.core.as_ref().and_then(|c| c.default_provider.clone())
        }),
        model: cli.model.clone(),
        working_dir: cli.working_dir.clone().or_else(|| {
            cfg.core.as_ref().and_then(|c| c.working_dir.clone())
        }),
    };

    // Resolve effective non-interactive mode
    let effective_non_interactive = noninteractive::is_non_interactive(cli.non_interactive);

    match cli.command {
        None => {
            if effective_non_interactive {
                Cli::parse_from(["airsctl", "--help"]);
            } else {
                repl::run(ctx).await?;
            }
        }
        Some(Commands::Agent { cmd }) => agent::run(cmd, ctx).await?,
        Some(Commands::Task { cmd }) => task::run(cmd, ctx).await?,
        Some(Commands::Worker { cmd }) => worker::run(cmd, ctx).await?,
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

fn init_tracing(verbose: bool, quiet: bool) {
    let filter = if verbose {
        EnvFilter::new("debug")
    } else if quiet {
        EnvFilter::new("error")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };
    fmt().with_env_filter(filter).with_target(false).init();
}

fn load_config(cli: &Cli) -> Result<autodev_shared::config::CliConfigFile> {
    if let Some(path) = &cli.config {
        let content = std::fs::read_to_string(path)?;
        let cfg: autodev_shared::config::CliConfigFile = toml::from_str(&content)?;
        return Ok(cfg);
    }
    Ok(autodev_shared::config::CliConfigFile::load()?)
}

fn resolve_output_format(cli: &Cli, cfg: &autodev_shared::config::CliConfigFile) -> OutputFormat {
    let raw = if cli.output_format != "table" {
        cli.output_format.clone()
    } else {
        cfg.output_format()
    };
    match raw.as_str() {
        "json"  => OutputFormat::Json,
        "plain" => OutputFormat::Plain,
        _       => OutputFormat::Table,
    }
}

fn resolve_orchestrator_url(cli: &Cli, cfg: &autodev_shared::config::CliConfigFile) -> String {
    cli.orchestrator_url.clone().unwrap_or_else(|| cfg.orchestrator_url())
}
