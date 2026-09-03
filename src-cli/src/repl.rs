use anyhow::Result;
use colored::Colorize;
use rustyline::{error::ReadlineError, DefaultEditor};

use crate::commands;
use crate::output::OutputContext;

const HISTORY_FILE: &str = ".airs_history";

pub async fn run(ctx: OutputContext) -> Result<()> {
    println!("{}", "airsctl REPL — type 'help' or 'exit'".cyan().bold());
    println!(
        "{}",
        "Use arrow keys for history, Tab for fuzzy completion.".dimmed()
    );

    let mut rl = DefaultEditor::new()?;

    let history_path = history_path();
    if let Some(p) = &history_path {
        let _ = rl.load_history(p);
    }

    loop {
        let line = rl.readline("airsctl> ");
        match line {
            Ok(input) => {
                let input = input.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(&input);

                match input.as_str() {
                    "exit" | "quit" | "q" => {
                        println!("{}", "Goodbye.".dimmed());
                        break;
                    }
                    "help" | "?" => print_help(),
                    _ => {
                        if let Err(e) = dispatch(&input, ctx.clone()).await {
                            ctx.error(&format!("Error: {e}"));
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{}", "^C".dimmed());
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(e) => {
                ctx.error(&format!("Readline error: {e}"));
                break;
            }
        }
    }

    if let Some(p) = &history_path {
        let _ = rl.save_history(p);
    }

    Ok(())
}

fn history_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    Some(std::path::PathBuf::from(home).join(HISTORY_FILE))
}

fn print_help() {
    println!("{}", "Available commands:".cyan().bold());
    let cmds = [
        ("agent run <task>", "Run an agent with the given task"),
        ("agent list", "List recent agent runs"),
        ("task create <title>", "Create a new task"),
        ("task list", "List tasks"),
        ("task get <id>", "Get task details"),
        ("worker list", "List registered workers"),
        ("worker status <id>", "Show worker status"),
        ("config list", "Show all config keys"),
        ("config get <key>", "Get a config value"),
        ("config set <key> <value>", "Set a config value"),
        ("plan list", "List all plans"),
        ("plan create <title>", "Create a new plan"),
        ("system status", "Show system and sidecar status"),
        ("analytics cost", "Show cost summary"),
        ("analytics tokens", "Show token usage"),
        ("plugin list", "List marketplace plugins"),
        ("plugin install <name>", "Install a plugin"),
        ("plugin verify <name>", "Verify plugin dependencies"),
        ("plugin info <name>", "Show plugin details"),
        ("help", "Show this help"),
        ("exit / quit", "Exit the REPL"),
    ];
    for (cmd, desc) in &cmds {
        println!("  {:<32} {}", cmd.yellow(), desc);
    }
}

async fn dispatch(input: &str, ctx: OutputContext) -> Result<()> {
    // Prepend "airsctl" so clap can parse the subcommand
    let mut args = vec!["airsctl"];
    let parts: Vec<&str> = input.split_whitespace().collect();
    args.extend_from_slice(&parts);

    // Parse and dispatch manually to avoid re-parsing global flags
    let subcommand = parts.first().copied().unwrap_or("");
    let rest = &parts[1..];

    match subcommand {
        "task" => dispatch_task(rest, ctx).await,
        "worker" => dispatch_worker(rest, ctx).await,
        "config" => dispatch_config(rest, ctx).await,
        "plan" => dispatch_plan(rest, ctx).await,
        "system" => dispatch_system(rest, ctx).await,
        "analytics" => dispatch_analytics(rest, ctx).await,
        "plugin" => dispatch_plugin(rest, ctx).await,
        other => {
            ctx.error(&format!(
                "Unknown command: '{other}'. Type 'help' for available commands."
            ));
            Ok(())
        }
    }
}

// P8-44 — `dispatch_agent` is deleted with the `agent` verb it drove.
// Every one of its subcommands was HTTP to the orchestrator.


async fn dispatch_task(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::task::{CreateArgs, GetArgs, ListArgs, TaskCmd};
    let cmd = match args.first().copied() {
        Some("create") => {
            let title = args.get(1..).unwrap_or_default().join(" ");
            if title.is_empty() {
                ctx.error("Usage: task create <title>");
                return Ok(());
            }
            TaskCmd::Create(CreateArgs {
                title,
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: None,
            })
        }
        Some("list") => TaskCmd::List(ListArgs {
            status: None,
            worker: None,
            limit: 20,
        }),
        Some("get") => {
            let id: i64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            TaskCmd::Get(GetArgs { id })
        }
        Some(other) => {
            ctx.error(&format!("Unknown task subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: task <create|list|get|assign|claim|complete|fail>");
            return Ok(());
        }
    };
    commands::task::run(cmd, ctx).await
}

async fn dispatch_worker(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::worker::{ListArgs, StatusArgs, WorkerCmd};
    let cmd = match args.first().copied() {
        Some("list") => WorkerCmd::List(ListArgs {
            worker_type: None,
            status: None,
        }),
        Some("status") => {
            let slug = args.get(1).map(|s| s.to_string());
            WorkerCmd::Status(StatusArgs { slug })
        }
        Some(other) => {
            ctx.error(&format!("Unknown worker subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: worker <register|list|status|heartbeat|unregister>");
            return Ok(());
        }
    };
    commands::worker::run(cmd, ctx).await
}

async fn dispatch_config(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::config::{ConfigCmd, GetArgs, SetArgs};
    let cmd = match args.first().copied() {
        Some("list") => ConfigCmd::List,
        Some("get") => {
            let key = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            ConfigCmd::Get(GetArgs { key })
        }
        Some("set") => {
            let key = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            let value = args.get(2..).unwrap_or_default().join(" ");
            ConfigCmd::Set(SetArgs { key, value })
        }
        Some(other) => {
            ctx.error(&format!("Unknown config subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: config <init|get|set|list|validate|import|export>");
            return Ok(());
        }
    };
    commands::config::run(cmd, ctx).await
}

async fn dispatch_plan(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::plan::{CreateArgs, GetArgs, ListArgs, PlanCmd};
    let cmd = match args.first().copied() {
        Some("list") => PlanCmd::List(ListArgs { status: None }),
        Some("create") => {
            let title = args.get(1..).unwrap_or_default().join(" ");
            if title.is_empty() {
                ctx.error("Usage: plan create <title>");
                return Ok(());
            }
            PlanCmd::Create(CreateArgs {
                title,
                description: None,
                scope: None,
            })
        }
        Some("get") => {
            // Plan ids are UUIDs. The previous version parsed an i64 and fell
            // back to 0 when that failed, so a typo silently asked for a plan
            // that cannot exist and reported "not found" as if the id were
            // real.
            match args.get(1) {
                Some(id) if !id.is_empty() => PlanCmd::Get(GetArgs {
                    id: (*id).to_string(),
                }),
                _ => {
                    ctx.error("Usage: plan get <plan-id>");
                    return Ok(());
                }
            }
        }
        Some(other) => {
            ctx.error(&format!("Unknown plan subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: plan <create|list|get|add-step|update-step|activate>");
            return Ok(());
        }
    };
    commands::plan::run(cmd, ctx).await
}

async fn dispatch_system(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::system::{LogsArgs, SystemCmd};
    let cmd = match args.first().copied() {
        Some("status") => SystemCmd::Status,
        Some("logs") => SystemCmd::Logs(LogsArgs {
            lines: 50,
            service: None,
        }),
        Some(other) => {
            ctx.error(&format!("Unknown system subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: system <status|logs|sidecars|backup|restore>");
            return Ok(());
        }
    };
    commands::system::run(cmd, ctx).await
}

async fn dispatch_analytics(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::analytics::{AnalyticsCmd, CostArgs, TokensArgs, WorkersArgs};
    let cmd = match args.first().copied() {
        Some("cost") => AnalyticsCmd::Cost(CostArgs {
            window: "month".into(),
        }),
        Some("tokens") => AnalyticsCmd::Tokens(TokensArgs {
            session_id: None,
            limit: 50,
        }),
        Some("workers") => AnalyticsCmd::Workers(WorkersArgs {
            window: "month".into(),
        }),
        Some("export") => AnalyticsCmd::Export { output: None },
        Some(other) => {
            ctx.error(&format!("Unknown analytics subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: analytics <cost|tokens|workers|export>");
            return Ok(());
        }
    };
    commands::analytics::run(cmd, ctx).await
}

async fn dispatch_plugin(args: &[&str], ctx: OutputContext) -> Result<()> {
    use crate::commands::plugin::{
        PluginCmd, PluginInstallArgs, PluginListArgs, PluginNameArgs, PluginRunArgs,
        PluginUninstallArgs, PluginUpdateArgs,
    };
    let cmd = match args.first().copied() {
        Some("list") => PluginCmd::List(PluginListArgs {
            tag: None,
            installed: false,
        }),
        Some("install") => {
            let name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            if name.is_empty() {
                ctx.error("Usage: plugin install <name>");
                return Ok(());
            }
            PluginCmd::Install(PluginInstallArgs { name, file: None })
        }
        Some("uninstall") => {
            let name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            PluginCmd::Uninstall(PluginUninstallArgs { name, force: false })
        }
        Some("update") => {
            let name = args
                .get(1)
                .map(|s| s.to_string())
                .unwrap_or_else(|| "all".to_string());
            PluginCmd::Update(PluginUpdateArgs { name })
        }
        Some("verify") => {
            let name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            PluginCmd::Verify(PluginNameArgs { name })
        }
        Some("info") => {
            let name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            PluginCmd::Info(PluginNameArgs { name })
        }
        Some("run") => {
            let name = args.get(1).map(|s| s.to_string()).unwrap_or_default();
            let extra = args
                .get(2..)
                .unwrap_or_default()
                .iter()
                .map(|s| s.to_string())
                .collect();
            PluginCmd::Run(PluginRunArgs { name, args: extra })
        }
        Some(other) => {
            ctx.error(&format!("Unknown plugin subcommand: '{other}'"));
            return Ok(());
        }
        None => {
            ctx.error("Usage: plugin <list|install|uninstall|update|verify|info|run>");
            return Ok(());
        }
    };
    commands::plugin::run_plugin_cmd(cmd, ctx).await
}
