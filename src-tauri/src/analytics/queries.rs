// Analytics queries — data retrieval for dashboard and reports
use std::sync::Arc;
use chrono::{Local, NaiveDate, Duration};
use std::collections::HashMap;
use crate::errors::Result;
use super::db::AnalyticsDb;
use super::types::*;

pub struct AnalyticsQueries {
    db: Arc<AnalyticsDb>,
}

impl AnalyticsQueries {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    /// Get dashboard summary (today vs yesterday)
    pub fn get_dashboard_summary(&self) -> Result<DashboardSummary> {
        let today = Local::now().naive_local().date();
        let yesterday = today - Duration::days(1);

        let today_cost = self.db.get_daily_cost(today)?
            .map(|r| r.total_cost_usd)
            .unwrap_or(0.0);

        let yesterday_cost = self.db.get_daily_cost(yesterday)?
            .map(|r| r.total_cost_usd)
            .unwrap_or(0.0);

        let cost_change_percent = if yesterday_cost > 0.0 {
            ((today_cost - yesterday_cost) / yesterday_cost) * 100.0
        } else {
            0.0
        };

        let today_tokens = self.db.get_daily_cost(today)?
            .map(|r| r.total_tokens)
            .unwrap_or(0);

        let yesterday_tokens = self.db.get_daily_cost(yesterday)?
            .map(|r| r.total_tokens)
            .unwrap_or(0);

        let token_change_percent = if yesterday_tokens > 0 {
            ((today_tokens - yesterday_tokens) as f64 / yesterday_tokens as f64) * 100.0
        } else {
            0.0
        };

        // Phase 7.6 — Pull worker counts from the live WorkersDb instead of hardcoding zeros.
        let (total_workers, active_workers, workers_added_today) = {
            match crate::db::workers::WorkersDb::open() {
                Ok(wdb) => {
                    let all = wdb.list().unwrap_or_default();
                    let total = all.len() as i32;
                    let active = all.iter()
                        .filter(|w| w.status != "paused" && w.status != "deleted")
                        .count() as i32;
                    let today_str = today.to_string();
                    let added = all.iter()
                        .filter(|w| w.created_at.starts_with(&today_str))
                        .count() as i32;
                    (total, active, added)
                }
                Err(_) => (0, 0, 0),
            }
        };

        // Phase 7.6 — Count distinct sessions recorded in token_usage today.
        let sessions_today = self.db.get_daily_cost(today)?
            .map(|r| r.session_count)
            .unwrap_or(0);

        Ok(DashboardSummary {
            today_cost,
            yesterday_cost,
            cost_change_percent,
            today_tokens,
            yesterday_tokens,
            token_change_percent,
            active_workers,
            total_workers,
            workers_added_today,
            active_sessions: active_workers,  // proxy: any worker in non-paused state
            sessions_today,
            messages_today: self.db.get_daily_cost(today)?
                .map(|r| r.message_count)
                .unwrap_or(0),
            tool_calls_today: self.db.get_daily_cost(today)?
                .map(|r| r.tool_call_count)
                .unwrap_or(0),
        })
    }

    /// Get cost analytics for a date range
    pub fn get_cost_analytics(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        _granularity: &str,
    ) -> Result<CostAnalytics> {
        let records = self.db.get_daily_costs_range(start, end)?;

        let mut data_points = Vec::new();
        let mut total_cost = 0.0;
        let mut peak_cost = 0.0;
        let mut peak_day = None;
        let mut lowest_cost = f64::MAX;
        let mut lowest_day = None;

        for record in records {
            total_cost += record.total_cost_usd;

            if record.total_cost_usd > peak_cost {
                peak_cost = record.total_cost_usd;
                peak_day = Some(record.date.to_string());
            }

            if record.total_cost_usd < lowest_cost {
                lowest_cost = record.total_cost_usd;
                lowest_day = Some(record.date.to_string());
            }

            data_points.push(CostDataPoint {
                timestamp: record.date.to_string(),
                cost_usd: record.total_cost_usd,
                tokens: record.total_tokens,
                sessions: record.session_count,
            });
        }

        let days = (end - start).num_days().max(1) as f64;
        let average_daily_cost = total_cost / days;

        Ok(CostAnalytics {
            date_range: format!("{} to {}", start, end),
            granularity: "daily".to_string(),
            data_points,
            total_cost,
            average_daily_cost,
            peak_cost_day: peak_day,
            lowest_cost_day: lowest_day,
        })
    }

    /// Get token usage analytics
    pub fn get_token_usage(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<TokenUsageAnalytics> {
        let records = self.db.get_daily_costs_range(start, end)?;

        let mut total_tokens = 0i64;
        let mut by_agent_type: HashMap<String, i64> = HashMap::new();
        let mut by_model: HashMap<String, i64> = HashMap::new();
        let mut by_provider: HashMap<String, i64> = HashMap::new();

        for record in records {
            total_tokens += record.total_tokens;

            for (agent, cost) in &record.agent_type_breakdown {
                // Estimate tokens from cost (rough approximation)
                *by_agent_type.entry(agent.clone()).or_insert(0) += (cost * 1000.0) as i64;
            }

            for (provider, cost) in &record.provider_breakdown {
                *by_provider.entry(provider.clone()).or_insert(0) += (cost * 1000.0) as i64;
            }
        }

        Ok(TokenUsageAnalytics {
            date_range: format!("{} to {}", start, end),
            total_tokens,
            prompt_tokens: (total_tokens as f64 * 0.6) as i64,  // Rough estimate
            completion_tokens: (total_tokens as f64 * 0.4) as i64,
            thinking_tokens: None,
            by_agent_type,
            by_model,
            by_provider,
            top_sessions: Vec::new(),  // TODO: Implement session-level tracking
        })
    }

    /// Get API failures
    pub fn get_api_failures(&self, hours: i32) -> Result<ApiFailureAnalytics> {
        let failures = self.db.get_api_failures_recent(hours)?;

        let mut by_provider: HashMap<String, i32> = HashMap::new();
        let mut by_type: HashMap<String, i32> = HashMap::new();

        for failure in &failures {
            *by_provider.entry(failure.provider.clone()).or_insert(0) += 1;
            *by_type.entry(failure.error_type.clone()).or_insert(0) += 1;
        }

        Ok(ApiFailureAnalytics {
            date_range: format!("Last {} hours", hours),
            total_failures: failures.len() as i32,
            failures_by_provider: by_provider,
            failures_by_type: by_type,
            recent_failures: failures,
            auto_retry_success_rate: 0.85,  // TODO: Calculate from actual retry data
        })
    }

    /// Predict monthly cost based on current usage
    pub fn predict_monthly_cost(&self) -> Result<CostPrediction> {
        let today = Local::now().naive_local().date();
        let seven_days_ago = today - Duration::days(7);

        let records = self.db.get_daily_costs_range(seven_days_ago, today)?;

        if records.is_empty() {
            return Ok(CostPrediction {
                current_daily_average: 0.0,
                projected_monthly_cost: 0.0,
                confidence_percent: 0.0,
                based_on_days: 0,
            });
        }

        let total_cost: f64 = records.iter().map(|r| r.total_cost_usd).sum();
        let days = records.len() as f64;
        let daily_average = total_cost / days;
        let projected_monthly = daily_average * 30.0;

        Ok(CostPrediction {
            current_daily_average: daily_average,
            projected_monthly_cost: projected_monthly,
            confidence_percent: 75.0,  // Based on 7-day sample
            based_on_days: days as i32,
        })
    }
}
