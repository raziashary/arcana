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

#[tauri::command]
pub fn get_worker_analytics(
    worker_id: Option<String>,
    start: String,
    end: String,
) -> Result<serde_json::Value> {
    let start_date = NaiveDate::parse_from_str(&start, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;
    let end_date = NaiveDate::parse_from_str(&end, "%Y-%m-%d")
        .map_err(|e| crate::errors::ArcanaError::Agent(format!("Invalid date format: {}", e)))?;

    let manager = analytics::global();
    let db = manager.db();

    // Phase 7.6 — Query the real worker_stats table instead of returning zeros.
    // If a worker_id filter was supplied, return that worker's aggregated stats.
    // Otherwise, build a summary across all workers that appear in the DB.
    let worker_stats = if let Some(ref wid) = worker_id {
        let agg = db.get_worker_stats_range(wid, start_date, end_date)?;
        let (completed, failed, success_rate, cost, time_min, avg_dur) = agg
            .map(|r| {
                (
                    r.tasks_completed,
                    r.tasks_failed,
                    r.success_rate,
                    r.cost_usd,
                    r.time_spent_minutes,
                    r.avg_task_duration_minutes,
                )
            })
            .unwrap_or((0, 0, 0.0, 0.0, 0, 0));

        serde_json::json!({
            "worker_id": worker_id,
            "date_range": { "start": start, "end": end },
            "stats": {
                "tasks_completed": completed,
                "tasks_failed": failed,
                "success_rate": success_rate,
                "avg_task_duration_minutes": avg_dur,
                "cost_usd": cost,
                "total_time_minutes": time_min
            },
            "daily_breakdown": []
        })
    } else {
        // All workers: pull a distinct list from the live WorkersDb and aggregate each.
        let workers = crate::db::workers::WorkersDb::open()
            .ok()
            .and_then(|wdb| wdb.list().ok())
            .unwrap_or_default();

        let mut per_worker = Vec::new();
        for w in &workers {
            let agg = db.get_worker_stats_range(&w.slug, start_date, end_date)?;
            let (completed, failed, success_rate, cost, time_min, avg_dur) = agg
                .map(|r| {
                    (
                        r.tasks_completed,
                        r.tasks_failed,
                        r.success_rate,
                        r.cost_usd,
                        r.time_spent_minutes,
                        r.avg_task_duration_minutes,
                    )
                })
                .unwrap_or((0, 0, 0.0, 0.0, 0, 0));

            per_worker.push(serde_json::json!({
                "worker_id": w.slug,
                "tasks_completed": completed,
                "tasks_failed": failed,
                "success_rate": success_rate,
                "avg_task_duration_minutes": avg_dur,
                "cost_usd": cost,
                "total_time_minutes": time_min
            }));
        }

        let total_completed: i32 = per_worker
            .iter()
            .filter_map(|v| v["tasks_completed"].as_i64())
            .map(|n| n as i32)
            .sum();
        let total_failed: i32 = per_worker
            .iter()
            .filter_map(|v| v["tasks_failed"].as_i64())
            .map(|n| n as i32)
            .sum();
        let total_cost: f64 = per_worker
            .iter()
            .filter_map(|v| v["cost_usd"].as_f64())
            .sum();
        let total = total_completed + total_failed;
        let overall_success_rate = if total > 0 {
            total_completed as f64 / total as f64
        } else {
            0.0
        };

        serde_json::json!({
            "worker_id": null,
            "date_range": { "start": start, "end": end },
            "stats": {
                "tasks_completed": total_completed,
                "tasks_failed": total_failed,
                "success_rate": overall_success_rate,
                "cost_usd": total_cost,
                "avg_task_duration_minutes": 0,
                "total_time_minutes": 0
            },
            "workers": per_worker,
            "daily_breakdown": []
        })
    };

    Ok(worker_stats)
}

#[tauri::command]
pub fn get_system_logs(
    component: Option<String>,
    severity: Option<String>,
    hours: i32,
) -> Result<Vec<serde_json::Value>> {
    // Phase 7.7 — Query the real system_events table (already exists in analytics.db).
    let safe_hours = hours.clamp(1, 168); // 1h–7d guard against absurd ranges
    let manager = analytics::global();
    let db = manager.db();
    let records =
        db.get_system_events_recent(component.as_deref(), severity.as_deref(), safe_hours)?;

    let events: Vec<serde_json::Value> = records
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id":         r.id,
                "timestamp":  r.timestamp.to_rfc3339(),
                "component":  r.component,
                "event_type": r.event_type,
                "message":    r.message,
                "severity":   r.severity,
                "metadata":   r.metadata,
            })
        })
        .collect();

    Ok(events)
}
