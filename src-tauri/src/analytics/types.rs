// Analytics types — data structures for cost tracking, token usage, worker metrics
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, NaiveDate};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// Token Usage & Cost Tracking
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub agent_type: String,  // PA, Supervisor, Worker, QC
    pub provider: String,
    pub model: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub thinking_tokens: Option<i32>,
    pub cost_usd: f64,
    pub request_type: String,  // chat, tool_use, summarization
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyCostRecord {
    pub date: NaiveDate,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub session_count: i32,
    pub message_count: i32,
    pub tool_call_count: i32,
    pub provider_breakdown: HashMap<String, f64>,  // {"anthropic": 1.23, "openai": 0.45}
    pub agent_type_breakdown: HashMap<String, f64>, // {"PA": 0.80, "Worker": 0.88}
}

// ═══════════════════════════════════════════════════════════════════════════
// Worker Analytics
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatsRecord {
    pub worker_id: String,
    pub date: NaiveDate,
    pub tasks_completed: i32,
    pub tasks_failed: i32,
    pub time_spent_minutes: i32,
    pub cost_usd: f64,
    pub avg_task_duration_minutes: i32,
    pub success_rate: f64,
    pub last_heartbeat: Option<DateTime<Utc>>,
}

// ═══════════════════════════════════════════════════════════════════════════
// API Failures & System Events
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFailureRecord {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub provider: String,
    pub error_type: String,  // rate_limit, auth, timeout, model_error
    pub error_message: String,
    pub request_id: Option<String>,
    pub retry_count: i32,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEventRecord {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub component: String,  // sidecar, provider, tool, agent
    pub event_type: String, // start, stop, error, warning
    pub message: String,
    pub severity: String,   // info, warning, error, critical
    pub metadata: Option<String>,  // JSON
}

// ═══════════════════════════════════════════════════════════════════════════
// Dashboard Summary & Analytics
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSummary {
    pub today_cost: f64,
    pub yesterday_cost: f64,
    pub cost_change_percent: f64,
    pub today_tokens: i64,
    pub yesterday_tokens: i64,
    pub token_change_percent: f64,
    pub active_workers: i32,
    pub total_workers: i32,
    pub workers_added_today: i32,
    pub active_sessions: i32,
    pub sessions_today: i32,
    pub messages_today: i32,
    pub tool_calls_today: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnalytics {
    pub date_range: String,
    pub granularity: String,  // daily, hourly
    pub data_points: Vec<CostDataPoint>,
    pub total_cost: f64,
    pub average_daily_cost: f64,
    pub peak_cost_day: Option<String>,
    pub lowest_cost_day: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDataPoint {
    pub timestamp: String,
    pub cost_usd: f64,
    pub tokens: i64,
    pub sessions: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageAnalytics {
    pub date_range: String,
    pub total_tokens: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub thinking_tokens: Option<i64>,
    pub by_agent_type: HashMap<String, i64>,
    pub by_model: HashMap<String, i64>,
    pub by_provider: HashMap<String, i64>,
    pub top_sessions: Vec<SessionTokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTokenUsage {
    pub session_id: String,
    pub tokens: i64,
    pub cost_usd: f64,
    pub agent_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAnalytics {
    pub worker_id: Option<String>,
    pub date_range: String,
    pub workers: Vec<WorkerMetrics>,
    pub total_tasks: i32,
    pub total_cost: f64,
    pub average_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetrics {
    pub worker_id: String,
    pub tasks_completed: i32,
    pub tasks_failed: i32,
    pub success_rate: f64,
    pub avg_duration_minutes: i32,
    pub cost_per_task: f64,
    pub total_cost: f64,
    pub utilization_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFailureAnalytics {
    pub date_range: String,
    pub total_failures: i32,
    pub failures_by_provider: HashMap<String, i32>,
    pub failures_by_type: HashMap<String, i32>,
    pub recent_failures: Vec<ApiFailureRecord>,
    pub auto_retry_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostPrediction {
    pub current_daily_average: f64,
    pub projected_monthly_cost: f64,
    pub confidence_percent: f64,
    pub based_on_days: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub format: String,  // csv, json
    pub path: String,
    pub rows_exported: i32,
    pub file_size_bytes: i64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Query Filters
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageFilter {
    pub date_range: Option<DateRange>,
    pub agent_type: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub request_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiFailureFilter {
    pub date_range: Option<DateRange>,
    pub provider: Option<String>,
    pub error_type: Option<String>,
    pub resolved: Option<bool>,
}
