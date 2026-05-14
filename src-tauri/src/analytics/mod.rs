// Analytics module — comprehensive cost tracking, token usage, worker metrics, and system health.
// Phase 5.4: Dashboard + Analytics
//
// Databases:
//   ~/.autodev/analytics.db — Token usage, costs, worker stats, API failures, system events
//
// Data Collection Pipeline:
//   Events (Tauri, CLI, Workers) → Event Bus → Analytics DB → Aggregator → Dashboard UI

pub mod db;
pub mod collector;
pub mod aggregator;
pub mod queries;
pub mod exporter;
pub mod types;

pub use db::AnalyticsDb;
pub use collector::AnalyticsCollector;
pub use aggregator::AnalyticsAggregator;
pub use queries::AnalyticsQueries;
pub use exporter::AnalyticsExporter;
pub use types::*;

use std::sync::Arc;
use crate::errors::Result;

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

    /// Start background aggregation job (hourly and daily rollups)
    pub fn start_aggregation_job(&self) {
        let aggregator = self.aggregator.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
                if let Err(e) = aggregator.run_hourly_rollup() {
                    eprintln!("Hourly aggregation failed: {}", e);
                }
                if let Err(e) = aggregator.run_daily_rollup() {
                    eprintln!("Daily aggregation failed: {}", e);
                }
            }
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
    ANALYTICS_MANAGER
        .get()
        .cloned()
        .expect(
            "analytics::install_global was not called during startup. \
             AutoDev cannot run without a real analytics DB — see \
             docs/endtoendrun/14may2026/02-dashboard-not-live.md."
        )
}
