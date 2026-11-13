// Analytics Tauri commands — dashboard data retrieval and export
use crate::analytics::{self, types::*};
use crate::errors::Result;
use crate::state::AppState;
use arcana_core::state::lock::LockTolerant;
use chrono::NaiveDate;
use std::sync::{Arc, Mutex, OnceLock};
use tauri::State;
use tracing::info;

// Doc 02 Phase 3.4 — one-shot diagnostic log on first dashboard hit.
// On-call debugging frequently needs to know "is the dashboard actually
// hitting the right analytics DB, and how many rows does it have?". This
// answers both with a single info line the first time the dashboard is
// queried after host startup. Idempotent via OnceLock — repeated invokes
// don't reissue the line.
static FIRST_HIT_LOGGED: OnceLock<()> = OnceLock::new();

fn log_first_hit_once() {
    let _ = FIRST_HIT_LOGGED.get_or_init(|| {
        let mgr = analytics::global();
        let counts = mgr
            .queries()
            .row_counts_for_diagnostic()
            .unwrap_or_default();
        let path = mgr
            .db_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(unknown)".into());
        info!("[analytics_commands] first dashboard hit — db_path={path} row_counts={counts:?}");
    });
}

#[tauri::command]
pub fn get_dashboard_summary(state: State<'_, Arc<Mutex<AppState>>>) -> Result<DashboardSummary> {
    log_first_hit_once();
    // Read live in-process counters — daily_costs is only populated by yesterday rollup,
    // so reading it for today would always yield 0.
    let (live_cost_usd, live_tokens) = {
        let mut s = state.lock_tolerant();
        (s.cost_today(), s.tokens_in_today + s.tokens_out_today)
    };
    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_dashboard_summary(live_cost_usd, live_tokens as i64)
}

#[tauri::command]
pub fn get_cost_analytics(
    start: String,
    end: String,
    granularity: String,
) -> Result<CostAnalytics> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_cost_analytics(start_date, end_date, &granularity)
}

#[tauri::command]
pub fn get_token_usage(start: String, end: String) -> Result<TokenUsageAnalytics> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_token_usage(start_date, end_date)
}

#[tauri::command]
pub fn get_api_failures(hours: i32) -> Result<ApiFailureAnalytics> {
    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_api_failures(hours)
}

#[tauri::command]
pub fn predict_monthly_cost() -> Result<CostPrediction> {
    let manager = analytics::global();
    let queries = manager.queries();
    queries.predict_monthly_cost()
}

#[tauri::command]
pub fn export_costs_csv(start: String, end: String, path: String) -> Result<ExportResult> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let exporter = manager.exporter();
    exporter.export_costs_csv(start_date, end_date, std::path::Path::new(&path))
}

#[tauri::command]
pub fn export_costs_json(start: String, end: String, path: String) -> Result<ExportResult> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let exporter = manager.exporter();
    exporter.export_costs_json(start_date, end_date, std::path::Path::new(&path))
}
