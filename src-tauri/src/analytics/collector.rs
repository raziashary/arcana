// Analytics collector — event ingestion pipeline for token usage, costs, and system events
use std::sync::Arc;
use chrono::Utc;
use tracing::debug;
use crate::errors::Result;
use super::db::AnalyticsDb;
use super::types::*;

pub struct AnalyticsCollector {
    db: Arc<AnalyticsDb>,
}

impl AnalyticsCollector {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    /// Record a token usage event (called after each LLM inference)
    pub fn record_token_usage(
        &self,
        session_id: String,
        agent_type: String,
        provider: String,
        model: String,
        prompt_tokens: i32,
        completion_tokens: i32,
        thinking_tokens: Option<i32>,
        cost_usd: f64,
        request_type: String,
    ) -> Result<i64> {
        let total_tokens = prompt_tokens + completion_tokens;
        let record = TokenUsageRecord {
            id: None,
            timestamp: Utc::now(),
            session_id,
            agent_type,
            provider,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            thinking_tokens,
            cost_usd,
            request_type,
        };

        // Doc 02 Phase 2 task 2.4: log every insert at DEBUG so future audits
        // can tell write-side from read-side failures at a glance. The
        // previous run produced zero such lines in the log — proof the writes
        // never reached the DB. After Phase 1 + 2 we should see one of these
        // per Mode 1 / Mode 2 tick.
        debug!(
            "[analytics] insert token_usage session={} model={} prompt={} completion={} total={} cost_usd={:.6}",
            record.session_id, record.model,
            record.prompt_tokens, record.completion_tokens, record.total_tokens,
            record.cost_usd
        );
        self.db.insert_token_usage(&record)
    }

    /// Record an API failure event
    pub fn record_api_failure(
        &self,
        provider: String,
        error_type: String,
        error_message: String,
        request_id: Option<String>,
    ) -> Result<i64> {
        let record = ApiFailureRecord {
            id: None,
            timestamp: Utc::now(),
            provider,
            error_type,
            error_message,
            request_id,
            retry_count: 0,
            resolved: false,
        };

        self.db.insert_api_failure(&record)
    }

    /// Record a system event
    pub fn record_system_event(
        &self,
        component: String,
        event_type: String,
        message: String,
        severity: String,
        metadata: Option<String>,
    ) -> Result<i64> {
        let record = SystemEventRecord {
            id: None,
            timestamp: Utc::now(),
            component,
            event_type,
            message,
            severity,
            metadata,
        };

        self.db.insert_system_event(&record)
    }

    /// Record worker statistics
    pub fn record_worker_stats(
        &self,
        worker_id: String,
        tasks_completed: i32,
        tasks_failed: i32,
        time_spent_minutes: i32,
        cost_usd: f64,
        avg_task_duration_minutes: i32,
    ) -> Result<()> {
        let today = chrono::Local::now().naive_local().date();
        let success_rate = if tasks_completed + tasks_failed > 0 {
            (tasks_completed as f64) / ((tasks_completed + tasks_failed) as f64) * 100.0
        } else {
            0.0
        };

        let record = WorkerStatsRecord {
            worker_id,
            date: today,
            tasks_completed,
            tasks_failed,
            time_spent_minutes,
            cost_usd,
            avg_task_duration_minutes,
            success_rate,
            last_heartbeat: Some(Utc::now()),
        };

        self.db.insert_worker_stats(&record)
    }
}
