// Analytics aggregator — background job for hourly and daily rollups
use super::db::AnalyticsDb;
use super::types::*;
use crate::errors::Result;
use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;

pub struct AnalyticsAggregator {
    db: Arc<AnalyticsDb>,
}

impl AnalyticsAggregator {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    // ⚠ `run_hourly_rollup` was DELETED in P2-55, not implemented.
    //
    // It was a stub returning `Ok(())`, called on a schedule — so it logged no
    // error, looked healthy, and produced nothing. The obvious fix is to write
    // the aggregation. That would have been wrong twice over:
    //
    //   1. It aggregated into `hourly_costs`, **a table nothing has ever
    //      created** and which is absent from the live `analytics.db` — see
    //      `queries.rs`, where the same table made a diagnostic report `-1`
    //      forever.
    //   2. **Nothing reads hourly buckets.** The dashboard reads daily costs and
    //      live in-process counters.
    //
    // Implementing it would have produced a writer with no reader, writing to a
    // table that does not exist — `L-C12`, built deliberately. Deleting the stub
    // is the honest answer; if hourly granularity is ever wanted, it arrives
    // with the reader that wants it and the migration that creates its table.

    /// Run daily rollup — aggregate token usage into daily costs
    pub fn run_daily_rollup(&self) -> Result<()> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_daily_analytics_rollup")
                .with_database("analytics");
        let worker_cutoff = Local::now().date_naive() - chrono::Duration::days(90);
        self.db
            .rollup_and_prune_worker_stats_before(worker_cutoff)?;

        let today = Local::now().naive_local().date();
        let yesterday = today - chrono::Duration::days(1);

        // Get all token usage for yesterday
        let records = self.db.get_token_usage_by_date(yesterday)?;

        if records.is_empty() {
            operation = operation.with_result_count(0);
            operation.succeed();
            return Ok(());
        }
        let record_count = records.len();

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
        operation = operation.with_result_count(record_count);
        operation.succeed();
        Ok(())
    }
}
