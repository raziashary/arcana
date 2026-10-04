// Analytics collector — event ingestion pipeline for token usage, costs, and system events
use super::db::AnalyticsDb;
use super::types::*;
use crate::errors::Result;
use chrono::Utc;
use std::sync::Arc;
use tracing::debug;

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
        // per agent turn and per worker tick.
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
        self.record_api_failure_outcome(provider, error_type, error_message, request_id, 0, false)
    }

    /// Record one provider call that saw at least one failed attempt. A
    /// recovered call is marked resolved, with the number of retries retained
    /// for the dashboard's retry-success metric. Messages must be pre-sanitized
    /// summaries; raw provider bodies are not safe analytics data.
    pub fn record_api_failure_outcome(
        &self,
        provider: String,
        error_type: String,
        error_message: String,
        request_id: Option<String>,
        retry_count: i32,
        resolved: bool,
    ) -> Result<i64> {
        // Redaction boundary (M-40, P1-21). A provider error message is the
        // most likely place for a credential to reach durable storage from the
        // egress path: an auth failure routinely echoes the request, and some
        // providers include the key prefix in the body they return.
        let error_message = crate::security::redaction::redact_text(&error_message);

        let record = ApiFailureRecord {
            id: None,
            timestamp: Utc::now(),
            provider,
            error_type,
            error_message,
            request_id,
            retry_count: retry_count.max(0),
            resolved,
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
        // Redaction boundary (M-40, P1-21). `message` and `metadata` are free
        // text written to a database that is never rotated, unlike the log
        // files — so a secret recorded here persists indefinitely.
        let message = crate::security::redaction::redact_text(&message);
        let metadata = metadata
            .as_deref()
            .map(crate::security::redaction::redact_value_text);

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
            (tasks_completed as f64) / ((tasks_completed + tasks_failed) as f64)
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
