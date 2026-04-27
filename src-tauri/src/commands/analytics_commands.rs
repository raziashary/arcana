// Analytics Tauri commands — dashboard data retrieval and export
use chrono::NaiveDate;
use crate::analytics::{self, types::*};
use crate::errors::Result;

#[tauri::command]
pub fn get_dashboard_summary() -> Result<DashboardSummary> {
    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_dashboard_summary()
}

#[tauri::command]
pub fn get_cost_analytics(
    start: String,
    end: String,
    granularity: String,
) -> Result<CostAnalytics> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let queries = manager.queries();
    queries.get_cost_analytics(start_date, end_date, &granularity)
}

#[tauri::command]
pub fn get_token_usage(
    start: String,
    end: String,
) -> Result<TokenUsageAnalytics> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;

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
pub fn export_costs_csv(
    start: String,
    end: String,
    path: String,
) -> Result<ExportResult> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let exporter = manager.exporter();
    exporter.export_costs_csv(start_date, end_date, std::path::Path::new(&path))
}

#[tauri::command]
pub fn export_costs_json(
    start: String,
    end: String,
    path: String,
) -> Result<ExportResult> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let exporter = manager.exporter();
    exporter.export_costs_json(start_date, end_date, std::path::Path::new(&path))
}

#[tauri::command]
pub fn get_worker_analytics(
    worker_id: Option<String>,
    start: String,
    end: String,
) -> Result<serde_json::Value> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let db = manager.db();
    
    // Get daily costs for the range to extract worker data
    let records = db.get_daily_costs_range(start_date, end_date)?;
    
    // Build worker analytics from available data
    let worker_stats = serde_json::json!({
        "worker_id": worker_id,
        "date_range": {
            "start": start,
            "end": end
        },
        "stats": {
            "tasks_completed": 0,
            "tasks_failed": 0,
            "success_rate": 0.0,
            "avg_task_duration_minutes": 0,
            "cost_usd": 0.0,
            "total_time_minutes": 0
        },
        "daily_breakdown": records.iter().map(|r| serde_json::json!({
            "date": r.date.to_string(),
            "cost": r.total_cost_usd,
            "tokens": r.total_tokens
        })).collect::<Vec<_>>()
    });
    
    Ok(worker_stats)
}

#[tauri::command]
pub fn get_system_logs(
    _component: Option<String>,
    _severity: Option<String>,
    _hours: i32,
) -> Result<Vec<serde_json::Value>> {
    // Get recent system events (mock implementation - returns empty for now)
    // TODO: Implement system_events table queries
    Ok(vec![])
}
