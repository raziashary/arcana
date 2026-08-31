use crate::output::OutputContext;
use anyhow::Result;
use autodev_shared::config::CliConfigFile;
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Create default config at ~/.airs/config.toml
    Init,
    /// Get a config value (key format: section.field)
    Get(GetArgs),
    /// Set a config value (key format: section.field)
    Set(SetArgs),
    /// List all config key-value pairs
    List,
    /// Validate the current config file
    Validate,
    /// Import config from a JSON or TOML file
    Import(ImportArgs),
    /// Export config to stdout (JSON or TOML)
    Export(ExportArgs),
}

#[derive(Args)]
pub struct GetArgs {
    /// Config key in section.field format (e.g. core.default_provider)
    pub key: String,
}

#[derive(Args)]
pub struct SetArgs {
    /// Config key in section.field format
    pub key: String,
    /// Value to set
    pub value: String,
}

#[derive(Args)]
pub struct ImportArgs {
    /// Path to TOML file to import
    pub path: String,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Output format: toml (default) or json
    #[arg(long, default_value = "toml")]
    pub format: String,
}

pub async fn run(cmd: ConfigCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        ConfigCmd::Init => init(ctx),
        ConfigCmd::Get(args) => get(args, ctx),
        ConfigCmd::Set(args) => set(args, ctx),
        ConfigCmd::List => list(ctx),
        ConfigCmd::Validate => validate(ctx),
        ConfigCmd::Import(args) => import(args, ctx),
        ConfigCmd::Export(args) => export(args, ctx),
    }
}

fn init(ctx: OutputContext) -> Result<()> {
    let path = autodev_shared::config::config_path()?;
    if path.exists() {
        ctx.warn(&format!("Config already exists at {}", path.display()));
        ctx.info("Use 'config set' to modify individual keys.");
        return Ok(());
    }
    CliConfigFile::init_default()?;
    ctx.success(&format!("Created default config at {}", path.display()));
    Ok(())
}

fn get(args: GetArgs, ctx: OutputContext) -> Result<()> {
    let cfg = CliConfigFile::load()?;
    match cfg.get_key(&args.key) {
        Some(v) => {
            if ctx.format == crate::output::OutputFormat::Json {
                println!("{}", serde_json::json!({ &args.key: v }));
            } else {
                println!("{}", v);
            }
        }
        None => ctx.error(&format!("Key '{}' not found or not set", args.key)),
    }
    Ok(())
}

fn set(args: SetArgs, ctx: OutputContext) -> Result<()> {
    let mut cfg = CliConfigFile::load()?;
    cfg.set_key(&args.key, &args.value)?;
    cfg.save()?;
    ctx.success(&format!("Set {} = {}", args.key, args.value));
    Ok(())
}

fn list(ctx: OutputContext) -> Result<()> {
    let cfg = CliConfigFile::load()?;
    let pairs = cfg.list_all();

    if pairs.is_empty() {
        ctx.info("No config values set. Run 'config init' to create defaults.");
        return Ok(());
    }

    let pairs_ref: Vec<(&str, &str)> = pairs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    ctx.print_kv(&pairs_ref);
    Ok(())
}

fn validate(ctx: OutputContext) -> Result<()> {
    let path = autodev_shared::config::config_path()?;
    if !path.exists() {
        ctx.error(&format!("Config file not found at {}", path.display()));
        ctx.info("Run 'config init' to create it.");
        return Ok(());
    }
    match CliConfigFile::load() {
        Ok(cfg) => {
            let pairs = cfg.list_all();
            ctx.success(&format!(
                "Config valid — {} keys set at {}",
                pairs.len(),
                path.display()
            ));
        }
        Err(e) => {
            ctx.error(&format!("Config parse error: {e}"));
        }
    }
    Ok(())
}

fn import(args: ImportArgs, ctx: OutputContext) -> Result<()> {
    let content = std::fs::read_to_string(&args.path)?;
    // Support TOML only for now (the config format)
    let cfg: CliConfigFile =
        toml::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse TOML: {e}"))?;
    cfg.save()?;
    ctx.success(&format!("Imported config from {}", args.path));
    Ok(())
}

fn export(args: ExportArgs, _ctx: OutputContext) -> Result<()> {
    let cfg = CliConfigFile::load()?;
    match args.format.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&cfg)?);
        }
        _ => {
            println!("{}", toml::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}
