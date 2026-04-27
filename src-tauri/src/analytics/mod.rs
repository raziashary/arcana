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
use tokio::sync::Mutex;
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

// Global analytics manager instance
use std::sync::OnceLock;
use rusqlite::Connection;

static ANALYTICS_MANAGER: OnceLock<Mutex<Option<Arc<AnalyticsManager>>>> = OnceLock::new();

/// Install the global analytics manager
pub fn install_global(manager: Arc<AnalyticsManager>) {
    let _ = ANALYTICS_MANAGER.get_or_init(|| Mutex::new(Some(manager)));
}

/// Get the global analytics manager (blocking)
pub fn global() -> Arc<AnalyticsManager> {
    let lock = ANALYTICS_MANAGER.get_or_init(|| Mutex::new(None));
    let guard = lock.blocking_lock();
    guard
        .as_ref()
        .cloned()
        .unwrap_or_else(|| Arc::new(AnalyticsManager {
            db: Arc::new(AnalyticsDb { conn: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()) }),
            collector: Arc::new(AnalyticsCollector::new(Arc::new(AnalyticsDb { conn: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()) }))),
            aggregator: Arc::new(AnalyticsAggregator::new(Arc::new(AnalyticsDb { conn: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()) }))),
            queries: Arc::new(AnalyticsQueries::new(Arc::new(AnalyticsDb { conn: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()) }))),
            exporter: Arc::new(AnalyticsExporter::new(Arc::new(AnalyticsDb { conn: std::sync::Mutex::new(rusqlite::Connection::open_in_memory().unwrap()) }))),
        }))
}
