use crate::output::OutputContext;
use anyhow::Result;
use autodev_shared::db::analytics::AnalyticsReader;
use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum AnalyticsCmd {
    /// Show cost summary
    Cost(CostArgs),
    /// Show token usage records
    Tokens(TokensArgs),
    /// Show worker statistics
    Workers(WorkersArgs),
    /// Export all analytics data to JSON
    Export {
        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Args)]
pub struct CostArgs {
    /// Time window: today, week, month, all
    #[arg(short, long, default_value = "month")]
    pub window: String,
}

#[derive(Args)]
pub struct TokensArgs {
    /// Filter by session ID
    #[arg(short, long)]
    pub session_id: Option<String>,

    /// Maximum records to return
    #[arg(short, long, default_value = "50")]
    pub limit: u32,
}

#[derive(Args)]
pub struct WorkersArgs {
    /// Time window: today, week, month, all
    #[arg(short, long, default_value = "month")]
    pub window: String,
}

pub async fn run(cmd: AnalyticsCmd, ctx: OutputContext) -> Result<()> {
    match cmd {
        AnalyticsCmd::Cost(args) => cost(args, ctx),
        AnalyticsCmd::Tokens(args) => tokens(args, ctx),
        AnalyticsCmd::Workers(args) => workers(args, ctx),
        AnalyticsCmd::Export { output } => export(output, ctx),
    }
}

fn cost(args: CostArgs, ctx: OutputContext) -> Result<()> {
    let reader = open_analytics(&ctx)?;
    let rows = reader.cost_summary(&args.window)?;

    if rows.is_empty() {
        ctx.info(&format!("No cost data for window: {}", args.window));
        return Ok(());
    }

    let headers = [
        "Date",
        "Cost (USD)",
        "Tokens",
        "Sessions",
        "Messages",
        "Tool Calls",
    ];
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.date.clone(),
                format!("${:.4}", r.total_cost_usd),
                r.total_tokens.to_string(),
                r.session_count.to_string(),
                r.message_count.to_string(),
                r.tool_call_count.to_string(),
            ]
        })
        .collect();

    ctx.print_table(&headers, table_rows);

    // Print totals
    let total_cost: f64 = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_tokens: i64 = rows.iter().map(|r| r.total_tokens).sum();
    ctx.info(&format!(
        "\nTotal: ${:.4} | {} tokens | {} sessions",
        total_cost,
        total_tokens,
        rows.iter().map(|r| r.session_count).sum::<i64>()
    ));

    Ok(())
}

fn tokens(args: TokensArgs, ctx: OutputContext) -> Result<()> {
    let reader = open_analytics(&ctx)?;
    let rows = reader.token_usage(args.session_id.as_deref(), args.limit)?;

    if rows.is_empty() {
        ctx.info("No token usage records found.");
        return Ok(());
    }

    let headers = [
        "ID",
        "Time",
        "Session",
        "Agent",
        "Model",
        "Prompt",
        "Completion",
        "Total",
        "Cost",
    ];
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.id.to_string(),
                r.timestamp[..19].to_string(),
                r.session_id[..8].to_string() + "…",
                r.agent_type.clone(),
                r.model.clone(),
                r.prompt_tokens.to_string(),
                r.completion_tokens.to_string(),
                r.total_tokens.to_string(),
                format!("${:.4}", r.cost_usd),
            ]
        })
        .collect();

    ctx.print_table(&headers, table_rows);
    Ok(())
}

fn workers(args: WorkersArgs, ctx: OutputContext) -> Result<()> {
    let reader = open_analytics(&ctx)?;
    let rows = reader.worker_stats(&args.window)?;

    if rows.is_empty() {
        ctx.info(&format!("No worker stats for window: {}", args.window));
        return Ok(());
    }

    let headers = [
        "Worker",
        "Date",
        "Completed",
        "Failed",
        "Time (min)",
        "Cost",
        "Avg Duration",
        "Success %",
    ];
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            vec![
                r.worker_id.clone(),
                r.date.clone(),
                r.tasks_completed.to_string(),
                r.tasks_failed.to_string(),
                r.time_spent_minutes.to_string(),
                format!("${:.4}", r.cost_usd),
                format!("{}m", r.avg_task_duration_minutes),
                format!("{:.1}%", r.success_rate * 100.0),
            ]
        })
        .collect();

    ctx.print_table(&headers, table_rows);
    Ok(())
}

fn export(output: Option<String>, ctx: OutputContext) -> Result<()> {
    let reader = open_analytics(&ctx)?;
    let data = reader.export_all()?;
    let json = serde_json::to_string_pretty(&data)?;

    match output {
        Some(path) => {
            std::fs::write(&path, &json)?;
            ctx.success(&format!("Analytics exported to {path}"));
        }
        None => println!("{json}"),
    }
    Ok(())
}

fn open_analytics(ctx: &OutputContext) -> Result<AnalyticsReader> {
    AnalyticsReader::open().map_err(|e| {
        ctx.error(&format!(
            "Cannot open analytics DB: {e}\n  \
             (Start the AutoDev desktop app to create and populate the database.)"
        ));
        anyhow::anyhow!("{e}")
    })
}
