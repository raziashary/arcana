// Analytics queries — data retrieval for dashboard and reports
use super::db::AnalyticsDb;
use super::types::*;
use crate::errors::Result;
use chrono::{Duration, Local, NaiveDate};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::warn;

pub struct AnalyticsQueries {
    db: Arc<AnalyticsDb>,
}

/// The tables `row_counts_for_diagnostic` reports on.
///
/// Must match what `AnalyticsDb::migrate` creates. This list used to include
/// `hourly_costs`, a table nothing has ever created and which is absent from
/// the live `analytics.db`. Because the count is read with `.unwrap_or(-1)`,
/// the missing table reported `-1` forever instead of failing — a diagnostic
/// that had been quietly wrong since it was written.
/// `every_diagnostic_table_actually_exists` pins the two together.
pub const DIAGNOSTIC_TABLES: &[&str] = &[
    "token_usage",
    "api_failures",
    "system_events",
    "worker_stats",
    "daily_costs",
];

impl AnalyticsQueries {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    /// Doc 02 Phase 3.4 — row-count snapshot for the one-shot
    /// diagnostic log emitted on first dashboard hit. Returns a HashMap of
    /// table name → row count for the tables the dashboard reads. Errors
    /// are swallowed silently — diagnostic logs must never fail the host.
    pub fn row_counts_for_diagnostic(&self) -> Result<std::collections::HashMap<String, i64>> {
        let mut out = std::collections::HashMap::new();
        let conn =
            self.db.conn.lock().map_err(|e| {
                crate::errors::AutoDevError::Agent(format!("Mutex lock failed: {e}"))
            })?;
        for table in DIAGNOSTIC_TABLES {
            // `-1` means "could not be counted". With the table list pinned to
            // the schema it is a real signal (a corrupt or missing database)
            // rather than the permanent state it was for `hourly_costs`.
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                .unwrap_or(-1);
            out.insert((*table).to_string(), count);
        }
        Ok(out)
    }

    /// Get dashboard summary (today vs yesterday).
    ///
    /// `live_cost_usd` and `live_tokens` are the real-time in-process AppState
    /// counters passed from the Tauri command.  We use them for today’s values
    /// because the `daily_costs` table is only populated by the overnight
    /// rollup job — reading it for today always yields 0.
    pub fn get_dashboard_summary(
        &self,
        live_cost_usd: f64,
        live_tokens: i64,
    ) -> Result<DashboardSummary> {
        let today = Local::now().naive_local().date();
        let yesterday = today - Duration::days(1);

        // Today: live AppState counters (daily_costs row for today never exists intra-day)
        let today_cost = live_cost_usd;
        let today_tokens = live_tokens;

        // Yesterday: persisted rollup
        let yesterday_cost = self
            .db
            .get_daily_cost(yesterday)?
            .map(|r| r.total_cost_usd)
            .unwrap_or(0.0);
        let yesterday_tokens = self
            .db
            .get_daily_cost(yesterday)?
            .map(|r| r.total_tokens)
            .unwrap_or(0);

        let cost_change_percent = if yesterday_cost > 0.0 {
            ((today_cost - yesterday_cost) / yesterday_cost) * 100.0
        } else {
            0.0
        };
        let token_change_percent = if yesterday_tokens > 0 {
            ((today_tokens - yesterday_tokens) as f64 / yesterday_tokens as f64) * 100.0
        } else {
            0.0
        };

        // Workers: open workers.db READ-ONLY to avoid write-lock contention.
        // WorkersDb::open() calls seed_defaults() which needs a write lock.
        // The recipe scheduler holds that lock on every tick, causing a race
        // that silently returns (0, 0, 0). A read-only connection never blocks.
        let (total_workers, active_workers, workers_added_today) = {
            use rusqlite::{Connection, OpenFlags};
            let path_result = crate::db::autodev_dir().map(|d| d.join("workers.db"));
            match path_result {
                Ok(path) => match Connection::open_with_flags(
                    &path,
                    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                ) {
                    Ok(conn) => {
                        let today_str = today.to_string();
                        let total: i32 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM workers WHERE status != 'deleted'",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        let active: i32 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM workers \
                             WHERE status NOT IN ('paused', 'deleted')",
                                [],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        let added: i32 = conn
                            .query_row(
                                "SELECT COUNT(*) FROM workers WHERE date(created_at) = ?1",
                                rusqlite::params![today_str],
                                |r| r.get(0),
                            )
                            .unwrap_or(0);
                        (total, active, added)
                    }
                    Err(e) => {
                        warn!("[DashboardSummary] workers.db read-only open failed: {e}");
                        (0, 0, 0)
                    }
                },
                Err(e) => {
                    warn!("[DashboardSummary] autodev_dir resolution failed: {e}");
                    (0, 0, 0)
                }
            }
        };

        // Today's session / message / tool-call counts from the live token_usage
        // table (written in real-time; daily_costs is only a rolled-up snapshot).
        let (sessions_today, messages_today, tool_calls_today) = self.db.get_today_activity();

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
            active_sessions: sessions_today,
            sessions_today,
            messages_today,
            tool_calls_today,
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

    /// Get token usage analytics.
    ///
    /// Queries the live `token_usage` table directly (not the daily_costs rollup)
    /// so that per-model, per-provider, and per-session breakdowns are real data
    /// with accurate prompt/completion splits — not estimated ratios.
    pub fn get_token_usage(&self, start: NaiveDate, end: NaiveDate) -> Result<TokenUsageAnalytics> {
        let (
            total_tokens,
            prompt_tokens,
            completion_tokens,
            thinking_tokens,
            by_agent_type,
            by_model,
            by_provider,
            top_sessions,
        ) = self.db.get_token_usage_for_range(start, end)?;

        Ok(TokenUsageAnalytics {
            date_range: format!("{} to {}", start, end),
            total_tokens,
            prompt_tokens,
            completion_tokens,
            thinking_tokens,
            by_agent_type,
            by_model,
            by_provider,
            top_sessions,
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

        // Calculate retry success rate from actual data:
        //   resolved failures that had at least one retry / total failures with retries
        let total_with_retries = failures.iter().filter(|f| f.retry_count > 0).count();
        let resolved_with_retries = failures
            .iter()
            .filter(|f| f.retry_count > 0 && f.resolved)
            .count();
        let auto_retry_success_rate = if total_with_retries > 0 {
            resolved_with_retries as f64 / total_with_retries as f64
        } else {
            0.0 // No retry history yet — show 0, not a fake 85%
        };

        Ok(ApiFailureAnalytics {
            date_range: format!("Last {} hours", hours),
            total_failures: failures.len() as i32,
            failures_by_provider: by_provider,
            failures_by_type: by_type,
            recent_failures: failures,
            auto_retry_success_rate,
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
            confidence_percent: 75.0, // Based on 7-day sample
            based_on_days: days as i32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A diagnostic that reports on a table nobody creates is not a
    /// diagnostic. This builds a fresh analytics database from the real
    /// migration and asserts every name in `DIAGNOSTIC_TABLES` is actually
    /// there, so the list cannot drift from the schema again.
    #[tokio::test]
    async fn every_diagnostic_table_actually_exists() {
        let dir = std::env::temp_dir().join("autodev-analytics-diag-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("analytics.db");

        let db = crate::analytics::db::AnalyticsDb::new_at(&path)
            .await
            .expect("analytics db");

        let conn = db.conn.lock().expect("lock");
        for table in DIAGNOSTIC_TABLES {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
                    [table],
                    |r| r.get(0),
                )
                .expect("sqlite_master query");
            assert_eq!(
                n, 1,
                "DIAGNOSTIC_TABLES names '{table}', which the schema never creates"
            );
        }
        drop(conn);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
