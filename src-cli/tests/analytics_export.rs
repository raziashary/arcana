use arcana_core::analytics::{
    db::AnalyticsDb,
    types::{ApiFailureRecord, SystemEventRecord, TokenUsageRecord, WorkerStatsRecord},
};
use arcana_core::hooks::types::{HookExecutionLog, HookOutcome};
use chrono::{NaiveDate, Utc};
use std::process::Command;

struct IsolatedProfile(std::path::PathBuf);

impl IsolatedProfile {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("arcana-analytics-export-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(path.join(".arcana")).expect("create isolated Arcana profile");
        std::fs::create_dir_all(path.join("workspace"))
            .expect("create isolated selected workspace");
        Self(path)
    }
}

impl Drop for IsolatedProfile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn analytics_cli_export_reads_pruned_worker_history_with_monthly_coverage() {
    let profile = IsolatedProfile::new();
    let arcana_dir = profile.0.join(".arcana");
    let analytics_path = arcana_dir.join("analytics.db");
    let db = AnalyticsDb::new_at(&analytics_path)
        .await
        .expect("open production analytics schema");
    let historical_usage_timestamp = chrono::DateTime::parse_from_rfc3339("2025-01-05T12:00:00Z")
        .expect("historical usage fixture timestamp")
        .with_timezone(&Utc);
    db.insert_token_usage(&TokenUsageRecord {
        id: None,
        timestamp: historical_usage_timestamp,
        session_id: "expired-token-session".into(),
        agent_type: "Worker".into(),
        provider: "google".into(),
        model: "gemini-3.8-flash".into(),
        prompt_tokens: 100,
        completion_tokens: 25,
        total_tokens: 125,
        thinking_tokens: Some(5),
        cost_usd: 0.0125,
        request_type: "chat".into(),
    })
    .expect("write production token-usage detail");
    db.insert_worker_stats(&WorkerStatsRecord {
        worker_id: "worker-retention-proof".into(),
        date: NaiveDate::from_ymd_opt(2025, 1, 5).expect("fixture date"),
        tasks_completed: 3,
        tasks_failed: 1,
        time_spent_minutes: 45,
        cost_usd: 0.75,
        avg_task_duration_minutes: 11,
        success_rate: 0.75,
        last_heartbeat: Some(
            chrono::DateTime::parse_from_rfc3339("2025-01-05T12:30:00Z")
                .expect("canonical UTC fixture")
                .with_timezone(&Utc),
        ),
    })
    .expect("write production daily worker stat");
    let today = Utc::now().date_naive();
    db.insert_worker_stats(&WorkerStatsRecord {
        worker_id: "worker-retention-proof".into(),
        date: today,
        tasks_completed: 1,
        tasks_failed: 0,
        time_spent_minutes: 8,
        cost_usd: 0.10,
        avg_task_duration_minutes: 8,
        success_rate: 1.0,
        last_heartbeat: Some(Utc::now()),
    })
    .expect("write current-day worker stat");
    db.insert_api_failure(&ApiFailureRecord {
        id: None,
        timestamp: Utc::now(),
        provider: "google".into(),
        error_type: "retryable".into(),
        error_message: "retained provider failure".into(),
        request_id: Some("retained-request-id".into()),
        retry_count: 1,
        resolved: true,
    })
    .expect("write retained API failure");
    db.insert_system_event(&SystemEventRecord {
        id: None,
        timestamp: Utc::now(),
        component: "worker".into(),
        event_type: "completed".into(),
        message: "retained worker event".into(),
        severity: "info".into(),
        metadata: Some("{}".into()),
    })
    .expect("write retained system event");
    let hooks = arcana_core::hooks::db::HooksDb::open_at(&arcana_dir.join("config.db"))
        .expect("open production config schema");
    hooks
        .insert_log(&HookExecutionLog {
            id: 0,
            hook_id: "hook-retention-proof".into(),
            hook_name: "retained hook".into(),
            hook_type: "pre_tool_use".into(),
            outcome: HookOutcome::Executed,
            output: None,
            error: None,
            executed_at: Utc::now().to_rfc3339(),
            duration_ms: 4,
        })
        .expect("write retained hook log");
    assert_eq!(
        db.rollup_and_prune_worker_stats_before(
            NaiveDate::from_ymd_opt(2025, 1, 6).expect("cutoff date")
        )
        .expect("roll up old daily row"),
        1
    );
    assert_eq!(
        db.prune_append_only_before(&(Utc::now() - chrono::Duration::days(90)).to_rfc3339())
            .expect("apply production append-only retention"),
        1
    );
    let expected_usage_date = historical_usage_timestamp
        .with_timezone(&chrono::Local)
        .date_naive();
    drop(db);

    let output_path = profile.0.join("analytics-export.json");
    let output = Command::new(env!("CARGO_BIN_EXE_arcanactl"))
        .env("USERPROFILE", &profile.0)
        .env("HOME", &profile.0)
        .args(["--output-format", "json", "analytics", "export", "--output"])
        .arg(&output_path)
        .output()
        .expect("run production CLI analytics export");
    assert!(
        output.status.success(),
        "CLI failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let exported: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&output_path).expect("read CLI-produced export file"),
    )
    .expect("parse exported analytics JSON");
    assert!(
        exported["token_usage"].as_array().unwrap().is_empty(),
        "expired per-request token detail is pruned from full export"
    );
    let historical_cost = exported["cost_summary"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["date"] == expected_usage_date.to_string())
        .expect("CLI export retains the daily summary for pruned token usage");
    assert_eq!(historical_cost["total_cost_usd"], 0.0125);
    assert_eq!(historical_cost["total_tokens"], 125);
    assert_eq!(historical_cost["session_count"], 1);
    assert_eq!(historical_cost["message_count"], 1);
    let rows = exported["worker_stats"]
        .as_array()
        .expect("worker stats export array");
    let monthly = rows
        .iter()
        .find(|row| row["worker_id"] == "worker-retention-proof" && row["granularity"] == "month")
        .expect("CLI export reads the retained monthly summary");
    assert_eq!(monthly["granularity"], "month");
    assert_eq!(monthly["date"], "2025-01");
    assert_eq!(monthly["coverage_start"], "2025-01-05");
    assert_eq!(monthly["coverage_end"], "2025-01-05");
    assert_eq!(monthly["tasks_completed"], 3);
    assert_eq!(monthly["tasks_failed"], 1);
    assert_eq!(monthly["time_spent_minutes"], 45);
    assert_eq!(monthly["cost_usd"], 0.75);
    assert_eq!(monthly["last_heartbeat"], "2025-01-05T12:30:00.000Z");
    let daily = rows
        .iter()
        .find(|row| row["granularity"] == "day")
        .expect("CLI export preserves the current daily row");
    assert_eq!(daily["date"], today.to_string());
    assert_eq!(daily["coverage_start"], today.to_string());
    assert_eq!(daily["coverage_end"], today.to_string());
    assert_eq!(daily["tasks_completed"], 1);
    assert_eq!(daily["tasks_failed"], 0);
    assert_eq!(exported["api_failures"].as_array().unwrap().len(), 1);
    assert_eq!(
        exported["api_failures"][0]["request_id"],
        "retained-request-id"
    );
    assert_eq!(exported["system_events"].as_array().unwrap().len(), 1);
    assert_eq!(
        exported["system_events"][0]["message"],
        "retained worker event"
    );
    assert_eq!(exported["hook_logs"].as_array().unwrap().len(), 1);
    assert_eq!(exported["hook_logs"][0]["hook_id"], "hook-retention-proof");
}
