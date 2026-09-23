// Analytics module — comprehensive cost tracking, token usage, worker metrics, and system health.
// Phase 5.4: Dashboard + Analytics
//
// Databases:
//   ~/.arcana/analytics.db — Token usage, costs, worker stats, API failures, system events
//
// Data Collection Pipeline:
//   Events (Tauri, CLI, Workers) → Event Bus → Analytics DB → Aggregator → Dashboard UI

pub mod aggregator;
pub mod collector;
pub mod db;
pub mod exporter;
pub mod queries;
pub mod types;

pub use aggregator::AnalyticsAggregator;
pub use collector::AnalyticsCollector;
pub use db::AnalyticsDb;
pub use exporter::AnalyticsExporter;
pub use queries::AnalyticsQueries;
pub use types::*;

use crate::errors::Result;
use std::sync::Arc;

/// Global analytics manager — singleton for event collection and queries
pub struct AnalyticsManager {
    db: Arc<AnalyticsDb>,
    collector: Arc<AnalyticsCollector>,
    aggregator: Arc<AnalyticsAggregator>,
    queries: Arc<AnalyticsQueries>,
    exporter: Arc<AnalyticsExporter>,
}

impl AnalyticsManager {
    /// Initialize analytics manager with database
    pub async fn new() -> Result<Self> {
        let db = Arc::new(AnalyticsDb::new().await?);
        let collector = Arc::new(AnalyticsCollector::new(db.clone()));
        let aggregator = Arc::new(AnalyticsAggregator::new(db.clone()));
        let queries = Arc::new(AnalyticsQueries::new(db.clone()));
        let exporter = Arc::new(AnalyticsExporter::new(db.clone()));

        Ok(Self {
            db,
            collector,
            aggregator,
            queries,
            exporter,
        })
    }

    pub fn db(&self) -> Arc<AnalyticsDb> {
        self.db.clone()
    }

    pub fn collector(&self) -> Arc<AnalyticsCollector> {
        self.collector.clone()
    }

    pub fn aggregator(&self) -> Arc<AnalyticsAggregator> {
        self.aggregator.clone()
    }

    pub fn queries(&self) -> Arc<AnalyticsQueries> {
        self.queries.clone()
    }

    pub fn exporter(&self) -> Arc<AnalyticsExporter> {
        self.exporter.clone()
    }

    /// Doc 02 Phase 3.4 — surface the on-disk DB path for the one-shot
    /// diagnostic log emitted on first dashboard hit. Returns None only
    /// in the (impossible-since-Phase-1) case that install_global was
    /// bypassed.
    pub fn db_path(&self) -> Option<std::path::PathBuf> {
        Some(self.db.path().to_path_buf())
    }

    /// Start the background daily rollup.
    ///
    /// ⚠ **Runs once immediately, then hourly (P2-30).** It used to sleep an
    /// hour *before* its first run, so a desktop app used in sub-hour sessions
    /// — which is most of them — **never completed a single rollup**. The job
    /// existed, was scheduled, logged nothing, and produced nothing.
    ///
    /// The hourly rollup is gone entirely; see `aggregator.rs` for why it was
    /// deleted rather than implemented.
    pub fn start_aggregation_job(&self) {
        let aggregator = self.aggregator.clone();
        std::thread::spawn(move || loop {
            // Before the sleep, not after.
            if let Err(e) = aggregator.run_daily_rollup() {
                // `tracing`, not `eprintln!`: stderr bypasses the log layer
                // entirely, so this never reached the log file and — until
                // 2026-09-02 — never passed the redactor either.
                tracing::error!("Daily aggregation failed: {e}");
            }
            std::thread::sleep(std::time::Duration::from_secs(3600));
        });
    }
}

// Global analytics manager instance — OnceLock<Arc<>> requires no locking primitives;
// safe to call from any context including async tasks.
use std::sync::OnceLock;

static ANALYTICS_MANAGER: OnceLock<Arc<AnalyticsManager>> = OnceLock::new();

/// Install the global analytics manager (called once at startup, before any async tasks).
pub fn install_global(manager: Arc<AnalyticsManager>) {
    let _ = ANALYTICS_MANAGER.set(manager);
}

/// Get the global analytics manager — zero-cost read from OnceLock, never blocks.
///
/// **Panics if called before `install_global`.** The previous implementation
/// returned a throw-away in-memory manager with no schema applied, which
/// silently swallowed every write and every read for the entire app
/// lifetime — see `docs/endtoendrun/14may2026/02-dashboard-not-live.md`.
/// Loud failure is strictly cheaper than another silent-data audit cycle.
pub fn global() -> Arc<AnalyticsManager> {
    ANALYTICS_MANAGER.get().cloned().expect(
        "analytics::install_global was not called during startup. \
             Arcana cannot run without a real analytics DB — see \
             docs/endtoendrun/14may2026/02-dashboard-not-live.md.",
    )
}
