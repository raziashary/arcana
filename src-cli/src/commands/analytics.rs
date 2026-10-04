use crate::output::OutputContext;
use anyhow::Result;
use arcana_core::observability::timing::OperationTimer;
use arcana_shared::db::analytics::AnalyticsReader;
use chrono::{Duration, Utc};
use clap::{Args, Subcommand};
use std::path::Path;

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
    let rows = observe_analytics_read(
        "cli_analytics_cost_summary_read",
        || reader.cost_summary(&args.window),
        |rows| rows.len(),
    )?;

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
    let rows = observe_analytics_read(
        "cli_analytics_token_usage_read",
        || reader.token_usage(args.session_id.as_deref(), args.limit),
        |rows| rows.len(),
    )?;

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
    let rows = observe_analytics_read(
        "cli_analytics_worker_stats_read",
        || reader.worker_stats(&args.window),
        |rows| rows.len(),
    )?;

    if rows.is_empty() {
        ctx.info(&format!("No worker stats for window: {}", args.window));
        return Ok(());
    }

    let headers = [
        "Worker",
        "Date",
        "Coverage",
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
                format!(
                    "{}..{} ({})",
                    r.coverage_start, r.coverage_end, r.granularity
                ),
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
    let operation = OperationTimer::start("cli_analytics_export_all").with_database("analytics");
    let reader = open_analytics(&ctx)?;
    let costs = observe_analytics_read(
        "cli_analytics_export_cost_summary_read",
        || reader.cost_summary("all"),
        |rows| rows.len(),
    )?;
    let tokens = observe_analytics_read(
        "cli_analytics_export_token_usage_read",
        || reader.token_usage_retained(),
        |rows| rows.len(),
    )?;
    let workers = observe_analytics_read(
        "cli_analytics_export_worker_stats_read",
        || reader.worker_stats("all"),
        |rows| rows.len(),
    )?;
    let failures = observe_analytics_read(
        "cli_analytics_export_api_failures_read",
        || reader.api_failures_retained(),
        |rows| rows.len(),
    )?;
    let events = observe_analytics_read(
        "cli_analytics_export_system_events_read",
        || reader.system_events_retained(),
        |rows| rows.len(),
    )?;
    let config_path = arcana_shared::db::arcana_dir()?.join("config.db");
    let hooks_db = arcana_core::hooks::db::HooksDb::open_at(&config_path)?;
    let cutoff = (Utc::now() - Duration::days(90)).to_rfc3339();
    let hook_logs = observe_database_read(
        "cli_analytics_export_hook_logs_read",
        "config",
        || hooks_db.get_logs_since(&cutoff),
        |rows| rows.len(),
    )?;
    let data = serde_json::json!({
        "cost_summary": costs,
        "token_usage": tokens,
        "worker_stats": workers,
        "api_failures": failures,
        "system_events": events,
        "hook_logs": hook_logs,
    });
    let json = serde_json::to_string_pretty(&data)?;

    match output {
        Some(path) => {
            std::fs::write(&path, &json)?;
            ctx.success(&format!("Analytics exported to {path}"));
        }
        None => println!("{json}"),
    }
    operation.with_result_count(3).succeed();
    Ok(())
}

fn observe_analytics_read<T, E>(
    operation_name: &'static str,
    read: impl FnOnce() -> std::result::Result<T, E>,
    result_count: impl FnOnce(&T) -> usize,
) -> Result<T>
where
    E: Into<anyhow::Error>,
{
    observe_database_read(operation_name, "analytics", read, result_count)
}

fn observe_database_read<T, E>(
    operation_name: &'static str,
    database: &'static str,
    read: impl FnOnce() -> std::result::Result<T, E>,
    result_count: impl FnOnce(&T) -> usize,
) -> Result<T>
where
    E: Into<anyhow::Error>,
{
    let operation = OperationTimer::start(operation_name).with_database(database);
    let value = read().map_err(Into::into)?;
    let count = result_count(&value);
    if count == 0 {
        operation.with_result_count(0).set_outcome("empty");
    } else {
        operation.with_result_count(count).succeed();
    }
    Ok(value)
}

fn open_analytics(ctx: &OutputContext) -> Result<AnalyticsReader> {
    let path = arcana_shared::db::arcana_dir()?.join("analytics.db");
    open_analytics_at(&path).map_err(|e| {
        ctx.error(&format!(
            "Cannot open analytics DB: {e}\n  \
             (Start the Arcana desktop app to create and populate the database.)"
        ));
        anyhow::anyhow!("{e}")
    })
}

fn open_analytics_at(path: &Path) -> Result<AnalyticsReader> {
    // Use Arcana core's production connection setup so CLI analytics reads
    // report privacy-safe statement timing like desktop-owned DB paths.
    let connection = arcana_core::db::open_tuned(path)?;
    Ok(AnalyticsReader::from_connection(connection))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SharedWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for SharedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .expect("captured telemetry writer")
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn analytics_query_wrapper_emits_safe_success_empty_and_error_spans() {
        let output = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let writer_output = std::sync::Arc::clone(&output);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || SharedWriter(std::sync::Arc::clone(&writer_output)))
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            let populated = observe_analytics_read(
                "cli_analytics_test_read",
                || Ok::<_, anyhow::Error>(vec!["private-row-marker", "another-row"]),
                |rows| rows.len(),
            )
            .expect("successful analytics read");
            assert_eq!(populated.len(), 2);
            let empty = observe_analytics_read(
                "cli_analytics_empty_read",
                || Ok::<_, anyhow::Error>(Vec::<i32>::new()),
                |rows| rows.len(),
            )
            .expect("empty analytics read");
            assert!(empty.is_empty());
            let failed = observe_analytics_read::<Vec<i32>, anyhow::Error>(
                "cli_analytics_failed_read",
                || Err(anyhow::anyhow!("private-sql-error-marker")),
                |rows| rows.len(),
            );
            assert!(failed.is_err());
        });

        let output = String::from_utf8(output.lock().expect("captured telemetry").clone())
            .expect("UTF-8 telemetry");
        let events: Vec<serde_json::Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON event"))
            .collect();
        for (name, outcome, count) in [
            ("cli_analytics_test_read", "success", 2),
            ("cli_analytics_empty_read", "empty", 0),
            ("cli_analytics_failed_read", "error", 0),
        ] {
            assert!(events.iter().any(|event| {
                event["fields"]["operation"] == name
                    && event["fields"]["outcome"] == outcome
                    && event["fields"]["database"] == "analytics"
                    && event["fields"]["result_count"] == count
            }));
        }
        assert!(!output.contains("private-row-marker"));
        assert!(!output.contains("private-sql-error-marker"));
    }

    #[test]
    fn cli_analytics_opens_a_profiled_core_connection_and_reads_costs() {
        let directory = std::env::temp_dir().join(format!(
            "arcana-cli-analytics-profile-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("create isolated fixture directory");
        let path = directory.join("analytics.db");
        let setup = arcana_core::db::open_tuned(&path).expect("create isolated analytics fixture");
        setup
            .execute_batch(
                "CREATE TABLE daily_costs (
                    date TEXT PRIMARY KEY, total_cost_usd REAL NOT NULL,
                    total_tokens INTEGER NOT NULL, session_count INTEGER NOT NULL,
                    message_count INTEGER NOT NULL, tool_call_count INTEGER NOT NULL
                 );
                 INSERT INTO daily_costs VALUES ('2026-10-02', 1.25, 40, 2, 3, 4);",
            )
            .expect("populate analytics fixture");
        drop(setup);

        let reader = open_analytics_at(&path).expect("open profiled analytics reader");
        let rows = reader.cost_summary("all").expect("read cost summary");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, "2026-10-02");
        assert_eq!(rows[0].total_cost_usd, 1.25);
        assert_eq!(rows[0].total_tokens, 40);

        drop(reader);
        std::fs::remove_dir_all(directory).expect("remove isolated analytics fixture");
    }
}
