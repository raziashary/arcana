// arcanactl plugin — marketplace, install, uninstall, update, run, info.
// arcanactl worker setup — generate / redeem setup tokens.

use anyhow::Result;
use clap::{Args, Subcommand};
use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::output::OutputContext;

// ─── Clap definitions ────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum PluginCmd {
    /// List available plugins (marketplace view)
    List(PluginListArgs),
    /// Install a plugin by name
    Install(PluginInstallArgs),
    /// Uninstall a plugin by name
    Uninstall(PluginUninstallArgs),
    /// Update an installed plugin to the latest version
    Update(PluginUpdateArgs),
    /// Verify an installed plugin's dependencies
    Verify(PluginNameArgs),
    /// Run a plugin's `run` command
    Run(PluginRunArgs),
    /// Show full info / manifest for a plugin
    Info(PluginNameArgs),
    /// Set a config value for an installed plugin
    Config(PluginConfigArgs),
}

#[derive(Args)]
pub struct PluginListArgs {
    /// Filter by tag (e.g. rust, python, devops)
    #[arg(short, long)]
    pub tag: Option<String>,
    /// Only show installed plugins
    #[arg(long)]
    pub installed: bool,
}

#[derive(Args)]
pub struct PluginInstallArgs {
    /// Plugin name (from marketplace) or path to a manifest YAML file
    pub name: String,
    /// Install from a local YAML file instead of the marketplace
    #[arg(long, short)]
    pub file: Option<String>,
}

#[derive(Args)]
pub struct PluginUninstallArgs {
    /// Plugin name
    pub name: String,
    /// Skip confirmation
    #[arg(long)]
    pub force: bool,
}

#[derive(Args)]
pub struct PluginUpdateArgs {
    /// Plugin name (or 'all' to update every installed plugin)
    pub name: String,
}

#[derive(Args)]
pub struct PluginNameArgs {
    pub name: String,
}

#[derive(Args)]
pub struct PluginRunArgs {
    /// Plugin name
    pub name: String,
    /// Extra arguments forwarded to the plugin's run command
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

#[derive(Args)]
pub struct PluginConfigArgs {
    /// Plugin name
    pub name: String,
    /// Key=value pairs to set (e.g. timeout=30 region=us-east-1)
    #[arg(required = true)]
    pub pairs: Vec<String>,
}

// ─── Worker setup token ───────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum WorkerSetupCmd {
    /// Generate a one-click setup token
    Generate(WorkerSetupGenerateArgs),
    /// Validate / decode a setup token
    Validate(WorkerSetupValidateArgs),
    /// Redeem a token on this machine (run install via token)
    Redeem(WorkerSetupRedeemArgs),
}

#[derive(Args)]
pub struct WorkerSetupGenerateArgs {
    /// Fleet ID (default: local)
    #[arg(long, default_value = "local")]
    pub fleet_id: String,
    /// Worker type (e.g. worker-backend, worker-frontend)
    #[arg(long, short = 't', default_value = "worker-backend")]
    pub worker_type: String,
    /// Allowed repositories (repeatable)
    #[arg(long, short)]
    pub repo: Vec<String>,
    /// Token TTL in hours
    #[arg(long, default_value = "24")]
    pub ttl: u64,
    /// Only print the one-liner (useful for scripts)
    #[arg(long)]
    pub one_liner: bool,
}

#[derive(Args)]
pub struct WorkerSetupValidateArgs {
    /// Raw token string
    pub token: String,
}

#[derive(Args)]
pub struct WorkerSetupRedeemArgs {
    /// Raw token string
    #[arg(long)]
    pub token: String,
}

// ─── Local plugin types (mirror Tauri manifest without pulling in src-tauri) ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub plugin_type: String,
    pub os_support: Vec<String>,
    pub tags: Vec<String>,
    pub icon: String,
    pub installed: bool,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallResult {
    pub success: bool,
    pub plugin: String,
    pub version: String,
    pub message: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResult {
    pub success: bool,
    pub plugin: String,
    pub from_version: String,
    pub to_version: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub success: bool,
    pub plugin: String,
    pub checks: Vec<DependencyCheck>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCheck {
    pub name: String,
    pub required: bool,
    pub present: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupToken {
    pub token: String,
    pub worker_type: String,
    pub expires_at: String,
    pub one_liner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub fleet_id: String,
    pub worker_type: String,
    pub repos: Vec<String>,
    pub expires_at: String,
    pub issued_at: String,
}

// ─── Entry points ─────────────────────────────────────────────────────────────

pub async fn run_plugin_cmd(cmd: PluginCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        PluginCmd::List(args) => list(args, &ctx),
        PluginCmd::Install(args) => install(args, &ctx),
        PluginCmd::Uninstall(args) => uninstall(args, &ctx),
        PluginCmd::Update(args) => update(args, &ctx),
        PluginCmd::Verify(args) => verify(args, &ctx),
        PluginCmd::Run(args) => run(args, &ctx),
        PluginCmd::Info(args) => info(args, &ctx),
        PluginCmd::Config(args) => set_config(args, &ctx),
    }
}

pub async fn run_worker_setup(cmd: WorkerSetupCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        WorkerSetupCmd::Generate(args) => generate_token(args, &ctx),
        WorkerSetupCmd::Validate(args) => validate_token(args, &ctx),
        WorkerSetupCmd::Redeem(args) => redeem_token(args, &ctx),
    }
}

// ─── Plugin implementations ───────────────────────────────────────────────────

fn list(args: PluginListArgs, ctx: &OutputContext) -> Result<()> {
    // Read from the local built-in list + installed registry
    let plugins = collect_local_plugins();

    let filtered: Vec<&PluginInfo> = plugins
        .iter()
        .filter(|p| {
            if args.installed && !p.installed {
                return false;
            }
            if let Some(ref tag) = args.tag {
                return p.tags.iter().any(|t| t.contains(tag.as_str()));
            }
            true
        })
        .collect();

    if filtered.is_empty() {
        ctx.info("No plugins found.");
        return Ok(());
    }

    match ctx.format {
        crate::output::OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&filtered)?);
        }
        _ => {
            println!("{}", "Available Plugins".cyan().bold());
            println!("{}", "─".repeat(72).dimmed());
            for p in filtered {
                let status = if p.installed {
                    "✓ installed".green()
                } else {
                    "  available".dimmed()
                };
                println!(
                    "  {} {} {}  {}  [{}]",
                    p.icon,
                    p.name.yellow().bold(),
                    format!("v{}", p.version).dimmed(),
                    status,
                    p.tags.join(", ").dimmed(),
                );
                println!("     {}", p.description.dimmed());
            }
            println!("{}", "─".repeat(72).dimmed());
            println!(
                "Use {} to install",
                "arcanactl plugin install <name>".cyan()
            );
        }
    }
    Ok(())
}

fn install(args: PluginInstallArgs, ctx: &OutputContext) -> Result<()> {
    ctx.info(&format!("Installing plugin '{}'...", args.name));

    let result = if let Some(file) = &args.file {
        let yaml = std::fs::read_to_string(file)?;
        exec_install_from_yaml(&yaml)
    } else {
        exec_install_by_name(&args.name)
    };

    match result {
        Ok(r) if r.success => {
            ctx.success(&r.message);
            for w in &r.warnings {
                ctx.warn(w);
            }
        }
        Ok(r) => ctx.error(&r.message),
        Err(e) => ctx.error(&format!("Install error: {e}")),
    }
    Ok(())
}

fn uninstall(args: PluginUninstallArgs, ctx: &OutputContext) -> Result<()> {
    if !args.force && !ctx.non_interactive {
        ctx.warn(&format!(
            "This will remove plugin '{}'. Re-run with --force to confirm.",
            args.name
        ));
        return Ok(());
    }

    ctx.info(&format!("Uninstalling plugin '{}'...", args.name));
    match exec_uninstall(&args.name) {
        Ok(true) => ctx.success(&format!("Plugin '{}' removed.", args.name)),
        Ok(false) => ctx.warn(&format!("Plugin '{}' was not installed.", args.name)),
        Err(e) => ctx.error(&format!("Uninstall error: {e}")),
    }
    Ok(())
}

fn update(args: PluginUpdateArgs, ctx: &OutputContext) -> Result<()> {
    if args.name == "all" {
        ctx.info("Updating all installed plugins...");
        let plugins = collect_local_plugins();
        let installed: Vec<_> = plugins.iter().filter(|p| p.installed).collect();
        for p in installed {
            ctx.info(&format!("  Updating {}...", p.name));
            match exec_update(&p.name) {
                Ok(r) => {
                    if r.success {
                        ctx.success(&format!("  {} → v{}", p.name, r.to_version));
                    } else {
                        ctx.error(&format!("  {}: {}", p.name, r.message));
                    }
                }
                Err(e) => ctx.error(&format!("  {}: {e}", p.name)),
            }
        }
    } else {
        ctx.info(&format!("Updating plugin '{}'...", args.name));
        match exec_update(&args.name) {
            Ok(r) if r.success => ctx.success(&r.message),
            Ok(r) => ctx.error(&r.message),
            Err(e) => ctx.error(&format!("Update error: {e}")),
        }
    }
    Ok(())
}

fn verify(args: PluginNameArgs, ctx: &OutputContext) -> Result<()> {
    ctx.info(&format!("Verifying plugin '{}'...", args.name));
    match exec_verify(&args.name) {
        Ok(r) => match ctx.format {
            crate::output::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&r)?),
            _ => {
                let overall = if r.success {
                    "✓ PASS".green()
                } else {
                    "✗ FAIL".red()
                };
                println!("{} {}", overall, r.plugin.bold());
                for check in &r.checks {
                    let icon = if check.present {
                        "✓".green()
                    } else if check.required {
                        "✗".red()
                    } else {
                        "~".yellow()
                    };
                    let ver = check.version.as_deref().unwrap_or("").dimmed();
                    println!("  {} {} {}", icon, check.name.yellow(), ver);
                    if !check.present {
                        println!("    → {}", check.message.dimmed());
                    }
                }
                println!("{}", r.message.dimmed());
            }
        },
        Err(e) => ctx.error(&format!("Verify error: {e}")),
    }
    Ok(())
}

fn run(args: PluginRunArgs, ctx: &OutputContext) -> Result<()> {
    ctx.info(&format!("Running plugin '{}'...", args.name));
    match exec_run(&args.name, &args.args) {
        Ok(r) => {
            if !r.stdout.is_empty() {
                print!("{}", r.stdout);
            }
            if !r.stderr.is_empty() {
                eprint!("{}", r.stderr);
            }
            if !r.success {
                ctx.error(&format!("Plugin exited with code {}", r.exit_code));
            }
        }
        Err(e) => ctx.error(&format!("Run error: {e}")),
    }
    Ok(())
}

fn info(args: PluginNameArgs, ctx: &OutputContext) -> Result<()> {
    let plugins = collect_local_plugins();
    match plugins.iter().find(|p| p.name == args.name) {
        None => ctx.error(&format!("Plugin '{}' not found.", args.name)),
        Some(p) => match ctx.format {
            crate::output::OutputFormat::Json => println!("{}", serde_json::to_string_pretty(p)?),
            _ => {
                println!("{} {}", p.icon, p.name.yellow().bold());
                println!("  Version:  {}", p.version);
                println!("  Type:     {}", p.plugin_type);
                println!("  Author:   {}", p.author);
                println!("  Tags:     {}", p.tags.join(", "));
                println!("  OS:       {}", p.os_support.join(", "));
                println!("  Desc:     {}", p.description);
                println!(
                    "  Status:   {}",
                    if p.installed {
                        "installed".green()
                    } else {
                        "not installed".dimmed()
                    }
                );
                println!("  Built-in: {}", p.builtin);
            }
        },
    }
    Ok(())
}

fn set_config(args: PluginConfigArgs, ctx: &OutputContext) -> Result<()> {
    let mut values = serde_json::Map::new();
    for pair in &args.pairs {
        let (k, v) = pair.split_once('=').unwrap_or((pair.as_str(), "true"));
        values.insert(k.to_string(), serde_json::Value::String(v.to_string()));
    }
    match exec_set_config(&args.name, serde_json::Value::Object(values)) {
        Ok(()) => ctx.success(&format!("Config updated for plugin '{}'.", args.name)),
        Err(e) => ctx.error(&format!("Config error: {e}")),
    }
    Ok(())
}

// ─── Worker setup implementations ─────────────────────────────────────────────

fn generate_token(args: WorkerSetupGenerateArgs, ctx: &OutputContext) -> Result<()> {
    ctx.info(&format!(
        "Generating setup token for worker type '{}'...",
        args.worker_type
    ));
    match exec_generate_token(&args.fleet_id, &args.worker_type, &args.repo, args.ttl) {
        Ok(token) => {
            if args.one_liner {
                println!("{}", token.one_liner);
            } else {
                match ctx.format {
                    crate::output::OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&token)?)
                    }
                    _ => {
                        println!("\n{}", "Setup Token".cyan().bold());
                        println!("{}", "─".repeat(60).dimmed());
                        println!("Token:      {}", token.token.yellow());
                        println!("Type:       {}", token.worker_type);
                        println!("Expires:    {}", token.expires_at);
                        println!();
                        println!("{}", "One-liner (run on target machine):".cyan());
                        println!("  {}", token.one_liner.yellow());
                    }
                }
            }
        }
        Err(e) => ctx.error(&format!("Token generation error: {e}")),
    }
    Ok(())
}

fn validate_token(args: WorkerSetupValidateArgs, ctx: &OutputContext) -> Result<()> {
    match exec_validate_token(&args.token) {
        Ok(claims) => match ctx.format {
            crate::output::OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&claims)?)
            }
            _ => {
                println!("{}", "Token Claims".cyan().bold());
                println!("Fleet ID:    {}", claims.fleet_id);
                println!("Worker type: {}", claims.worker_type);
                println!("Repos:       {}", claims.repos.join(", "));
                println!("Issued:      {}", claims.issued_at);
                println!("Expires:     {}", claims.expires_at);
                ctx.success("Token is valid.");
            }
        },
        Err(e) => ctx.error(&format!("Token invalid: {e}")),
    }
    Ok(())
}

fn redeem_token(args: WorkerSetupRedeemArgs, ctx: &OutputContext) -> Result<()> {
    // Validate the token first
    let claims = match exec_validate_token(&args.token) {
        Ok(c) => c,
        Err(e) => {
            ctx.error(&format!("Token invalid: {e}"));
            return Ok(());
        }
    };

    ctx.info(&format!(
        "Redeeming token — installing worker type '{}'...",
        claims.worker_type
    ));

    match exec_install_by_name(&claims.worker_type) {
        Ok(r) if r.success => {
            ctx.success(&format!(
                "Worker '{}' installed. Fleet ID: {}",
                claims.worker_type, claims.fleet_id
            ));
        }
        Ok(r) => ctx.error(&r.message),
        Err(e) => ctx.error(&format!("Install error: {e}")),
    }
    Ok(())
}

// ─── Local execution helpers (mirror the Tauri plugin::PluginManager) ─────────
// We re-implement a thin client-side version here so the CLI doesn't depend on
// the full Tauri crate. It calls the same filesystem paths.

fn collect_local_plugins() -> Vec<PluginInfo> {
    // Read ~/.arcana/plugins/ for installed plugins
    let installed = scan_installed_names();

    // Return the 5 built-in plugins + anything installed on top
    let mut builtins = vec![
        mk_builtin(
            "worker-backend",
            "1.0.0",
            "Backend worker environment: Rust toolchain, Python + uv, Docker (optional)",
            "⚙️",
            &["rust", "python", "backend", "docker"],
            &installed,
        ),
        mk_builtin(
            "worker-frontend",
            "1.0.0",
            "Frontend worker environment: Node.js, npm, and common build tools",
            "🌐",
            &["node", "npm", "frontend", "react"],
            &installed,
        ),
        mk_builtin(
            "worker-devops",
            "1.0.0",
            "DevOps worker environment: Docker, kubectl, Terraform, and CI/CD tools",
            "🚀",
            &["docker", "kubernetes", "terraform", "devops", "ci"],
            &installed,
        ),
        mk_builtin(
            "worker-qa",
            "1.0.0",
            "QA worker environment: Python, pytest, playwright, and testing tools",
            "🧪",
            &["testing", "pytest", "playwright", "qa"],
            &installed,
        ),
        mk_builtin(
            "worker-security",
            "1.0.0",
            "Security worker environment: static analysis, secret scanning, and CVE tooling",
            "🔒",
            &["security", "sast", "secrets", "audit"],
            &installed,
        ),
    ];

    // Append any user-installed non-builtin plugins
    let builtin_names: std::collections::HashSet<&str> = [
        "worker-backend",
        "worker-frontend",
        "worker-devops",
        "worker-qa",
        "worker-security",
    ]
    .iter()
    .copied()
    .collect();
    for name in installed.iter() {
        if !builtin_names.contains(name.as_str()) {
            builtins.push(PluginInfo {
                name: name.clone(),
                version: "?".into(),
                description: "(user-installed plugin)".into(),
                author: "?".into(),
                plugin_type: "custom".into(),
                os_support: vec!["windows".into(), "linux".into(), "macos".into()],
                tags: vec![],
                icon: "🔌".into(),
                installed: true,
                builtin: false,
            });
        }
    }
    builtins
}

fn mk_builtin(
    name: &str,
    ver: &str,
    desc: &str,
    icon: &str,
    tags: &[&str],
    installed: &[String],
) -> PluginInfo {
    PluginInfo {
        name: name.into(),
        version: ver.into(),
        description: desc.into(),
        author: "Arcana".into(),
        plugin_type: "worker-setup".into(),
        os_support: vec!["windows".into(), "linux".into(), "macos".into()],
        tags: tags.iter().map(|s| s.to_string()).collect(),
        icon: icon.into(),
        installed: installed.iter().any(|n| n == name),
        builtin: true,
    }
}

fn scan_installed_names() -> Vec<String> {
    let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) else {
        return vec![];
    };
    let dir = std::path::PathBuf::from(home)
        .join(".arcana")
        .join("plugins");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return vec![];
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

fn plugin_install_dir(name: &str) -> anyhow::Result<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".arcana")
        .join("plugins")
        .join(name))
}

fn exec_install_by_name(name: &str) -> anyhow::Result<InstallResult> {
    // Find the manifest for the named plugin in builtins or installed registry
    let install_dir = plugin_install_dir(name)?;
    std::fs::create_dir_all(&install_dir)?;

    // Get the inline command from builtins map
    let cmd =
        builtin_install_cmd(name).ok_or_else(|| anyhow::anyhow!("Plugin '{name}' not found"))?;

    let result = exec_shell_cmd(&cmd);
    if !result.success {
        let _ = std::fs::remove_dir_all(&install_dir);
        return Ok(InstallResult {
            success: false,
            plugin: name.to_string(),
            version: "?".into(),
            message: format!("Install failed: {}", result.stderr.trim()),
            warnings: vec![],
        });
    }

    // Write a minimal manifest
    let yaml = format!(
        "name: {name}\nversion: 1.0.0\ntype: worker-setup\nauthor: Arcana\ndescription: Built-in plugin\nbuiltin: true\nos_support: [windows, linux, macos]\n"
    );
    let _ = std::fs::write(install_dir.join("manifest.yaml"), &yaml);

    Ok(InstallResult {
        success: true,
        plugin: name.to_string(),
        version: "1.0.0".into(),
        message: format!("Plugin '{name}' installed."),
        warnings: vec![],
    })
}

fn exec_install_from_yaml(yaml: &str) -> anyhow::Result<InstallResult> {
    // Minimal parse to get name
    let name = yaml
        .lines()
        .find(|l| l.starts_with("name:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let dir = plugin_install_dir(&name)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("manifest.yaml"), yaml)?;

    Ok(InstallResult {
        success: true,
        plugin: name.clone(),
        version: "custom".into(),
        message: format!("Plugin '{name}' installed from custom manifest."),
        warnings: vec![],
    })
}

fn exec_uninstall(name: &str) -> anyhow::Result<bool> {
    let dir = plugin_install_dir(name)?;
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(true)
}

fn exec_update(name: &str) -> anyhow::Result<UpdateResult> {
    let old_ver = read_installed_version(name);
    let _ = exec_uninstall(name);
    let install = exec_install_by_name(name)?;
    Ok(UpdateResult {
        success: install.success,
        plugin: name.to_string(),
        from_version: old_ver,
        to_version: install.version,
        message: install.message,
    })
}

fn exec_verify(name: &str) -> anyhow::Result<VerifyResult> {
    // Run the verify command for the named plugin
    let cmd = builtin_verify_cmd(name).unwrap_or_else(|| "echo 'no verify command'".into());
    let result = exec_shell_cmd(&cmd);
    Ok(VerifyResult {
        success: result.success,
        plugin: name.to_string(),
        checks: vec![],
        message: if result.success {
            result.stdout.trim().to_string()
        } else {
            result.stderr.trim().to_string()
        },
    })
}

fn exec_run(name: &str, args: &[String]) -> anyhow::Result<RunResult> {
    let dir = plugin_install_dir(name)?;
    #[cfg(target_os = "windows")]
    let script = dir.join("run.ps1");
    #[cfg(not(target_os = "windows"))]
    let script = dir.join("run.sh");

    let cmd = if script.exists() {
        #[cfg(target_os = "windows")]
        {
            format!(
                "powershell -ExecutionPolicy Bypass -File \"{}\" {}",
                script.display(),
                args.join(" ")
            )
        }
        #[cfg(not(target_os = "windows"))]
        {
            format!("bash \"{}\" {}", script.display(), args.join(" "))
        }
    } else {
        return Err(anyhow::anyhow!("Plugin '{}' has no run command", name));
    };

    Ok(exec_shell_cmd(&cmd))
}

fn exec_set_config(name: &str, values: serde_json::Value) -> anyhow::Result<()> {
    let dir = plugin_install_dir(name)?;
    let meta_path = dir.join(".install_meta.json");
    let mut meta: serde_json::Value = if meta_path.exists() {
        let s = std::fs::read_to_string(&meta_path)?;
        serde_json::from_str(&s).unwrap_or_default()
    } else {
        serde_json::json!({})
    };
    meta["config"] = values;
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

fn exec_generate_token(
    fleet_id: &str,
    worker_type: &str,
    repos: &[String],
    ttl: u64,
) -> anyhow::Result<SetupToken> {
    // Delegate to the token module logic via inline re-impl
    // (CLI doesn't link src-tauri; replicate the one-liner format)
    let base_url = "https://raw.githubusercontent.com/raziashary/arcana/main/src-cli";
    let token_data = serde_json::json!({
        "fleet_id": fleet_id,
        "worker_type": worker_type,
        "repos": repos,
        "ttl_hours": ttl,
        "issued_at": unix_now_str(),
    });
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let token = URL_SAFE_NO_PAD.encode(serde_json::to_string(&token_data)?.as_bytes());
    let expires_at = format!("in {ttl}h");
    let one_liner = format!(
        "curl -fsSL {base_url}/setup.sh | bash -s -- --token \"{token}\" --type \"{worker_type}\""
    );
    Ok(SetupToken {
        token,
        worker_type: worker_type.to_string(),
        expires_at,
        one_liner,
    })
}

fn exec_validate_token(token: &str) -> anyhow::Result<TokenClaims> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    // Strip any .signature suffix (the Tauri version uses HMAC; CLI just decodes)
    let payload = token.split('.').next().unwrap_or(token);
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| anyhow::anyhow!("Invalid token: {e}"))?;
    let val: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| anyhow::anyhow!("Invalid token JSON: {e}"))?;
    Ok(TokenClaims {
        fleet_id: val["fleet_id"].as_str().unwrap_or("?").into(),
        worker_type: val["worker_type"].as_str().unwrap_or("?").into(),
        repos: val["repos"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        expires_at: val["expires_at"].as_str().unwrap_or("?").into(),
        issued_at: val["issued_at"].as_str().unwrap_or("?").into(),
    })
}

// ─── Shell execution ──────────────────────────────────────────────────────────

fn exec_shell_cmd(cmd: &str) -> RunResult {
    #[cfg(target_os = "windows")]
    let mut c = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", cmd]);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut c = {
        let mut c = std::process::Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    c.stdout(std::process::Stdio::piped());
    c.stderr(std::process::Stdio::piped());
    match c.output() {
        Ok(out) => RunResult {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            exit_code: out.status.code().unwrap_or(-1),
        },
        Err(e) => RunResult {
            success: false,
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: -1,
        },
    }
}

// ─── Builtin command lookup ────────────────────────────────────────────────────

fn builtin_install_cmd(name: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    match name {
        "worker-backend"  => Some("winget install Rustlang.Rustup; winget install astral-sh.uv".into()),
        "worker-frontend" => Some("winget install OpenJS.NodeJS.LTS".into()),
        "worker-devops"   => Some("winget install Docker.DockerDesktop; winget install Hashicorp.Terraform".into()),
        "worker-qa"       => Some("winget install astral-sh.uv; uv pip install pytest pytest-asyncio playwright; playwright install chromium".into()),
        "worker-security" => Some("uv pip install bandit trufflehog semgrep; cargo install cargo-audit".into()),
        _ => None,
    }
    #[cfg(target_os = "macos")]
    match name {
        "worker-backend"  => Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
        "worker-frontend" => Some("brew install node@20".into()),
        "worker-devops"   => Some("brew install --cask docker && brew install kubectl terraform".into()),
        "worker-qa"       => Some("curl -LsSf https://astral.sh/uv/install.sh | sh && uv pip install pytest pytest-asyncio playwright && playwright install chromium".into()),
        "worker-security" => Some("uv pip install bandit trufflehog semgrep && cargo install cargo-audit".into()),
        _ => None,
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    match name {
        "worker-backend"  => Some("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && curl -LsSf https://astral.sh/uv/install.sh | sh".into()),
        "worker-frontend" => Some("curl -fsSL https://fnm.vercel.app/install | bash && fnm install 20 && fnm use 20".into()),
        "worker-devops"   => Some("apt-get update && apt-get install -y docker.io && systemctl enable docker".into()),
        "worker-qa"       => Some("curl -LsSf https://astral.sh/uv/install.sh | sh && uv pip install pytest pytest-asyncio playwright && playwright install --with-deps chromium".into()),
        "worker-security" => Some("uv pip install bandit trufflehog semgrep && cargo install cargo-audit".into()),
        _ => None,
    }
}

fn builtin_verify_cmd(name: &str) -> Option<String> {
    match name {
        "worker-backend" => Some("rustc --version && python3 --version && uv --version".into()),
        "worker-frontend" => Some("node --version && npm --version".into()),
        "worker-devops" => Some("docker --version".into()),
        "worker-qa" => Some("python3 -m pytest --version && playwright --version".into()),
        "worker-security" => Some("bandit --version && semgrep --version".into()),
        _ => None,
    }
}

fn read_installed_version(name: &str) -> String {
    let Ok(dir) = plugin_install_dir(name) else {
        return "unknown".into();
    };
    let yaml_path = dir.join("manifest.yaml");
    if !yaml_path.exists() {
        return "unknown".into();
    }
    let Ok(yaml) = std::fs::read_to_string(&yaml_path) else {
        return "unknown".into();
    };
    yaml.lines()
        .find(|l| l.starts_with("version:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn unix_now_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}
