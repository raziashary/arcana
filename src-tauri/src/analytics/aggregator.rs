// Analytics aggregator — background job for hourly and daily rollups
use super::db::AnalyticsDb;
use super::types::*;
use crate::errors::Result;
use chrono::{Local, NaiveDate};
use std::collections::HashMap;
use std::sync::Arc;

pub struct AnalyticsAggregator {
    db: Arc<AnalyticsDb>,
}

impl AnalyticsAggregator {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    /// Run hourly rollup — aggregate token usage into hourly buckets
    pub fn run_hourly_rollup(&self) -> Result<()> {
        // This would aggregate token_usage records into hourly buckets
        // For now, this is a placeholder for future implementation
        Ok(())
    }

    /// Run daily rollup — aggregate token usage into daily costs
    pub fn run_daily_rollup(&self) -> Result<()> {
        let today = Local::now().naive_local().date();
        let yesterday = today - chrono::Duration::days(1);

        // Get all token usage for yesterday
        let records = self.db.get_token_usage_by_date(yesterday)?;

        if records.is_empty() {
            return Ok(());
        }

        // Aggregate data
        let mut total_cost = 0.0;
        let mut total_tokens = 0i64;
        let mut session_count = 0;
        let mut message_count = 0;
        let mut tool_call_count = 0;
        let mut provider_breakdown: HashMap<String, f64> = HashMap::new();
        let mut agent_type_breakdown: HashMap<String, f64> = HashMap::new();
        let mut seen_sessions = std::collections::HashSet::new();

        for record in records {
            total_cost += record.cost_usd;
            total_tokens += record.total_tokens as i64;
            message_count += 1;

            if record.request_type == "tool_use" {
                tool_call_count += 1;
            }

            // Track unique sessions
            if seen_sessions.insert(record.session_id.clone()) {
                session_count += 1;
            }

            // Provider breakdown
            *provider_breakdown
                .entry(record.provider.clone())
                .or_insert(0.0) += record.cost_usd;

            // Agent type breakdown
            *agent_type_breakdown
                .entry(record.agent_type.clone())
                .or_insert(0.0) += record.cost_usd;
        }

        // Insert daily cost record
        let daily_cost = DailyCostRecord {
            date: yesterday,
            total_cost_usd: total_cost,
            total_tokens,
            session_count,
            message_count,
            tool_call_count,
            provider_breakdown,
            agent_type_breakdown,
        };

        self.db.insert_daily_cost(&daily_cost)?;
        Ok(())
    }
}
