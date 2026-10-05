// Analytics aggregator — background job for hourly and daily rollups
use super::db::AnalyticsDb;
use crate::errors::Result;
use chrono::Local;
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
        let now = chrono::Utc::now();
        let raw_cutoff = (now - chrono::Duration::days(90)).to_rfc3339();
        self.db.prune_append_only_before(&raw_cutoff)?;
        crate::hooks::db::HooksDb::open()?.prune_logs_before(&raw_cutoff)?;

        let worker_cutoff = Local::now().date_naive() - chrono::Duration::days(90);
        self.db
            .rollup_and_prune_worker_stats_before(worker_cutoff)?;

        let today = Local::now().naive_local().date();
        let yesterday = today - chrono::Duration::days(1);

        let record_count = self.db.rollup_daily_cost_for_date(yesterday)?;
        operation = operation.with_result_count(record_count);
        operation.succeed();
        Ok(())
    }
}
