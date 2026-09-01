// Analytics database — SQLite schema and migrations for cost tracking
use super::types::*;
use crate::db::autodev_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{AutoDevError, Result};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Mutex;

pub struct AnalyticsDb {
    pub conn: Mutex<Connection>,
    // Doc 02 Phase 3.4 — surface the on-disk path so the one-shot
    // diagnostic log can answer "is this hitting the right DB?" without
    // re-deriving autodev_dir().
    pub path: std::path::PathBuf,
}

impl AnalyticsDb {
    /// Initialize analytics database with schema
    pub async fn new() -> Result<Self> {
        Self::new_at(&autodev_dir()?.join("analytics.db")).await
    }

    /// As `new`, against an explicit path. Exists so tests never write to the
    /// user's real `~/.autodev/analytics.db`.
    pub async fn new_at(db_path: &std::path::Path) -> Result<Self> {
        let db_path = db_path.to_path_buf();
        let conn = crate::db::open_tuned(&db_path)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| AutoDevError::Database(e))?;

        let db = Self {
            conn: Mutex::new(conn),
            path: db_path,
        };
        db.migrate().await?;
        Ok(db)
    }

    /// Doc 02 Phase 3.4 — accessor for the diagnostic log.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Run migrations to create tables if they don't exist
    /// `analytics.db`'s ladder. `v1` is the schema as it stands, created
    /// idempotently, so an existing database is stamped without being altered.
    const LADDER: &[Migration] = &[Migration {
        version: 1,
        name: "analytics-baseline",
        sql: r#"
            CREATE TABLE IF NOT EXISTS token_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                session_id TEXT NOT NULL,
                agent_type TEXT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                prompt_tokens INTEGER NOT NULL,
                completion_tokens INTEGER NOT NULL,
                total_tokens INTEGER NOT NULL,
                thinking_tokens INTEGER,
                cost_usd REAL NOT NULL,
                request_type TEXT NOT NULL
                -- Note: previous schema declared FOREIGN KEY(session_id) REFERENCES sessions(id)
                -- but `sessions` lives in `autodev.db`, not `analytics.db`, so the FK
                -- always pointed at a non-existent table. With foreign_keys=ON this
                -- meant every insert failed. session_id remains a plain TEXT index
                -- column (see idx_token_usage_session below), which is all the
                -- queries layer needs. See docs/endtoendrun/14may2026/02 §Phase 1 §1.3.
            );

            CREATE TABLE IF NOT EXISTS daily_costs (
                date DATE PRIMARY KEY,
                total_cost_usd REAL NOT NULL,
                total_tokens INTEGER NOT NULL,
                session_count INTEGER NOT NULL,
                message_count INTEGER NOT NULL,
                tool_call_count INTEGER NOT NULL,
                provider_breakdown TEXT,
                agent_type_breakdown TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS worker_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                worker_id TEXT NOT NULL,
                date DATE NOT NULL,
                tasks_completed INTEGER NOT NULL DEFAULT 0,
                tasks_failed INTEGER NOT NULL DEFAULT 0,
                time_spent_minutes INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                avg_task_duration_minutes INTEGER NOT NULL DEFAULT 0,
                success_rate REAL NOT NULL DEFAULT 0.0,
                last_heartbeat DATETIME,
                UNIQUE(worker_id, date)
            );

            CREATE TABLE IF NOT EXISTS api_failures (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                provider TEXT NOT NULL,
                error_type TEXT NOT NULL,
                error_message TEXT NOT NULL,
                request_id TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                resolved BOOLEAN NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS system_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                component TEXT NOT NULL,
                event_type TEXT NOT NULL,
                message TEXT NOT NULL,
                severity TEXT NOT NULL,
                metadata TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_token_usage_timestamp ON token_usage(timestamp);
            CREATE INDEX IF NOT EXISTS idx_token_usage_session ON token_usage(session_id);
            CREATE INDEX IF NOT EXISTS idx_token_usage_agent ON token_usage(agent_type);
            CREATE INDEX IF NOT EXISTS idx_api_failures_timestamp ON api_failures(timestamp);
            CREATE INDEX IF NOT EXISTS idx_api_failures_provider ON api_failures(provider);
            CREATE INDEX IF NOT EXISTS idx_worker_stats_date ON worker_stats(date);
            CREATE INDEX IF NOT EXISTS idx_worker_stats_worker ON worker_stats(worker_id);
            CREATE INDEX IF NOT EXISTS idx_system_events_timestamp ON system_events(timestamp);
        "#,
    }];

    async fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        migrate::run_ladder(&conn, "analytics.db", Self::LADDER)?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Token Usage Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_token_usage(&self, record: &TokenUsageRecord) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model, 
             prompt_tokens, completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                record.timestamp.to_rfc3339(),
                record.session_id,
                record.agent_type,
                record.provider,
                record.model,
                record.prompt_tokens,
                record.completion_tokens,
                record.total_tokens,
                record.thinking_tokens,
                record.cost_usd,
                record.request_type,
            ],
        ).map_err(|e| AutoDevError::Database(e))?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_token_usage_by_date(&self, date: NaiveDate) -> Result<Vec<TokenUsageRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, session_id, agent_type, provider, model, prompt_tokens, 
             completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type
             FROM token_usage WHERE DATE(timestamp) = ? ORDER BY timestamp DESC",
            )
            .map_err(|e| AutoDevError::Database(e))?;

        let date_str = date.to_string();
        let records = stmt
            .query_map(params![date_str], |row| {
                let ts_str: String = row.get(1)?;
                let timestamp =
                    crate::db::timestamps::parse_db_timestamp(&ts_str).unwrap_or_else(|e| {
                        // Substituting "now" makes an unreadable record look
                        // brand new, which is worse than useless in a cost or
                        // failure report. The fallback stays — these records
                        // are display-only and a hard error would lose the rest
                        // of the row — but it is no longer silent.
                        tracing::warn!("[analytics] unreadable timestamp {ts_str:?}: {e}");
                        Utc::now()
                    });
                Ok(TokenUsageRecord {
                    id: row.get(0)?,
                    timestamp,
                    session_id: row.get(2)?,
                    agent_type: row.get(3)?,
                    provider: row.get(4)?,
                    model: row.get(5)?,
                    prompt_tokens: row.get(6)?,
                    completion_tokens: row.get(7)?,
                    total_tokens: row.get(8)?,
                    thinking_tokens: row.get(9)?,
                    cost_usd: row.get(10)?,
                    request_type: row.get(11)?,
                })
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Daily Costs Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_daily_cost(&self, record: &DailyCostRecord) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let provider_json =
            serde_json::to_string(&record.provider_breakdown).unwrap_or_else(|_| "{}".to_string());
        let agent_json = serde_json::to_string(&record.agent_type_breakdown)
            .unwrap_or_else(|_| "{}".to_string());

        conn.execute(
            "INSERT OR REPLACE INTO daily_costs (date, total_cost_usd, total_tokens, session_count, 
             message_count, tool_call_count, provider_breakdown, agent_type_breakdown)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                record.date.to_string(),
                record.total_cost_usd,
                record.total_tokens,
                record.session_count,
                record.message_count,
                record.tool_call_count,
                provider_json,
                agent_json,
            ],
        ).map_err(|e| AutoDevError::Database(e))?;

        Ok(())
    }

    pub fn get_daily_cost(&self, date: NaiveDate) -> Result<Option<DailyCostRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(
                "SELECT date, total_cost_usd, total_tokens, session_count, message_count, 
             tool_call_count, provider_breakdown, agent_type_breakdown
             FROM daily_costs WHERE date = ?",
            )
            .map_err(|e| AutoDevError::Database(e))?;

        let date_str = date.to_string();
        let record = stmt
            .query_row(params![date_str], |row| {
                let provider_json: String = row.get(6)?;
                let agent_json: String = row.get(7)?;
                let date_str: String = row.get(0)?;
                Ok(DailyCostRecord {
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap_or(date),
                    total_cost_usd: row.get(1)?,
                    total_tokens: row.get(2)?,
                    session_count: row.get(3)?,
                    message_count: row.get(4)?,
                    tool_call_count: row.get(5)?,
                    provider_breakdown: serde_json::from_str(&provider_json).unwrap_or_default(),
                    agent_type_breakdown: serde_json::from_str(&agent_json).unwrap_or_default(),
                })
            })
            .optional()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(record)
    }

    pub fn get_daily_costs_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<DailyCostRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(
                "SELECT date, total_cost_usd, total_tokens, session_count, message_count, 
             tool_call_count, provider_breakdown, agent_type_breakdown
             FROM daily_costs WHERE date BETWEEN ? AND ? ORDER BY date ASC",
            )
            .map_err(|e| AutoDevError::Database(e))?;

        let start_str = start.to_string();
        let end_str = end.to_string();
        let records = stmt
            .query_map(params![start_str, end_str], |row| {
                let provider_json: String = row.get(6)?;
                let agent_json: String = row.get(7)?;
                let date_str: String = row.get(0)?;
                Ok(DailyCostRecord {
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap_or(start),
                    total_cost_usd: row.get(1)?,
                    total_tokens: row.get(2)?,
                    session_count: row.get(3)?,
                    message_count: row.get(4)?,
                    tool_call_count: row.get(5)?,
                    provider_breakdown: serde_json::from_str(&provider_json).unwrap_or_default(),
                    agent_type_breakdown: serde_json::from_str(&agent_json).unwrap_or_default(),
                })
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Worker Stats Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_worker_stats(&self, record: &WorkerStatsRecord) -> Result<()> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT OR REPLACE INTO worker_stats (worker_id, date, tasks_completed, tasks_failed, 
             time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                record.worker_id,
                record.date.to_string(),
                record.tasks_completed,
                record.tasks_failed,
                record.time_spent_minutes,
                record.cost_usd,
                record.avg_task_duration_minutes,
                record.success_rate,
                record.last_heartbeat.map(|dt| dt.to_rfc3339()),
            ],
        )
        .map_err(|e| AutoDevError::Database(e))?;

        Ok(())
    }

    /// Aggregate worker stats for a single worker over a date range.
    /// Returns summed tasks_completed, tasks_failed, time_spent_minutes, cost_usd
    /// and a calculated success_rate. Returns None if no rows exist for the range.
    pub fn get_worker_stats_range(
        &self,
        worker_id: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Option<WorkerStatsRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(
                "SELECT
                 worker_id,
                 SUM(tasks_completed)        AS tasks_completed,
                 SUM(tasks_failed)           AS tasks_failed,
                 SUM(time_spent_minutes)     AS time_spent_minutes,
                 SUM(cost_usd)               AS cost_usd,
                 AVG(avg_task_duration_minutes) AS avg_duration,
                 MAX(last_heartbeat)         AS last_heartbeat
             FROM worker_stats
             WHERE worker_id = ? AND date BETWEEN ? AND ?
             GROUP BY worker_id",
            )
            .map_err(|e| AutoDevError::Database(e))?;

        let start_str = start.to_string();
        let end_str = end.to_string();
        let record = stmt
            .query_row(params![worker_id, start_str, end_str], |row| {
                let tasks_completed: i32 = row.get(1)?;
                let tasks_failed: i32 = row.get(2)?;
                let total = tasks_completed + tasks_failed;
                let success_rate = if total > 0 {
                    tasks_completed as f64 / total as f64
                } else {
                    0.0
                };
                let hb_str: Option<String> = row.get(6)?;
                let last_heartbeat =
                    hb_str.and_then(|s| crate::db::timestamps::parse_db_timestamp(&s).ok());
                Ok(WorkerStatsRecord {
                    worker_id: row.get(0)?,
                    date: end, // range end as the "as of" date
                    tasks_completed,
                    tasks_failed,
                    time_spent_minutes: row.get(3)?,
                    cost_usd: row.get(4)?,
                    avg_task_duration_minutes: row.get::<_, f64>(5).unwrap_or(0.0) as i32,
                    success_rate,
                    last_heartbeat,
                })
            })
            .optional()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(record)
    }

    pub fn get_worker_stats(
        &self,
        worker_id: &str,
        date: NaiveDate,
    ) -> Result<Option<WorkerStatsRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn.prepare(
            "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, 
             avg_task_duration_minutes, success_rate, last_heartbeat
             FROM worker_stats WHERE worker_id = ? AND date = ?"
        ).map_err(|e| AutoDevError::Database(e))?;

        let date_str = date.to_string();
        let record = stmt
            .query_row(params![worker_id, date_str], |row| {
                let date_str: String = row.get(1)?;
                let hb_str: Option<String> = row.get(8)?;
                let last_heartbeat =
                    hb_str.and_then(|s| crate::db::timestamps::parse_db_timestamp(&s).ok());
                Ok(WorkerStatsRecord {
                    worker_id: row.get(0)?,
                    date: NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").unwrap_or(date),
                    tasks_completed: row.get(2)?,
                    tasks_failed: row.get(3)?,
                    time_spent_minutes: row.get(4)?,
                    cost_usd: row.get(5)?,
                    avg_task_duration_minutes: row.get(6)?,
                    success_rate: row.get(7)?,
                    last_heartbeat,
                })
            })
            .optional()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(record)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // API Failures Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_api_failure(&self, record: &ApiFailureRecord) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO api_failures (timestamp, provider, error_type, error_message, request_id, retry_count, resolved)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                record.timestamp.to_rfc3339(),
                record.provider,
                record.error_type,
                record.error_message,
                record.request_id,
                record.retry_count,
                record.resolved,
            ],
        ).map_err(|e| AutoDevError::Database(e))?;

        Ok(conn.last_insert_rowid())
    }

    pub fn get_api_failures_recent(&self, hours: i32) -> Result<Vec<ApiFailureRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, provider, error_type, error_message, request_id, retry_count, resolved
             FROM api_failures WHERE timestamp > datetime('now', ? || ' hours') ORDER BY timestamp DESC"
        ).map_err(|e| AutoDevError::Database(e))?;

        let records = stmt
            .query_map(params![format!("-{}", hours)], |row| {
                let ts_str: String = row.get(1)?;
                let timestamp =
                    crate::db::timestamps::parse_db_timestamp(&ts_str).unwrap_or_else(|e| {
                        // Substituting "now" makes an unreadable record look
                        // brand new, which is worse than useless in a cost or
                        // failure report. The fallback stays — these records
                        // are display-only and a hard error would lose the rest
                        // of the row — but it is no longer silent.
                        tracing::warn!("[analytics] unreadable timestamp {ts_str:?}: {e}");
                        Utc::now()
                    });
                Ok(ApiFailureRecord {
                    id: row.get(0)?,
                    timestamp,
                    provider: row.get(2)?,
                    error_type: row.get(3)?,
                    error_message: row.get(4)?,
                    request_id: row.get(5)?,
                    retry_count: row.get(6)?,
                    resolved: row.get(7)?,
                })
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Live Activity Queries (token_usage table — written in real-time)
    // ═══════════════════════════════════════════════════════════════════════════

    /// Count today's activity directly from the live `token_usage` table.
    /// Unlike `daily_costs`, this table is written on every API call so it
    /// has today's data without waiting for the overnight rollup.
    /// Returns `(sessions_today, messages_today, tool_calls_today)`.
    pub fn get_today_activity(&self) -> (i32, i32, i32) {
        let today_str = chrono::Local::now().naive_local().date().to_string();
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return (0, 0, 0),
        };

        let sessions: i32 = conn
            .query_row(
                "SELECT COUNT(DISTINCT session_id) FROM token_usage WHERE DATE(timestamp) = ?1",
                params![today_str],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let messages: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM token_usage WHERE DATE(timestamp) = ?1",
                params![today_str],
                |r| r.get(0),
            )
            .unwrap_or(0);

        let tool_calls: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM token_usage \
             WHERE DATE(timestamp) = ?1 \
               AND request_type IN ('tool_use', 'tool', 'function_call')",
                params![today_str],
                |r| r.get(0),
            )
            .unwrap_or(0);

        (sessions, messages, tool_calls)
    }

    /// Aggregate the `token_usage` table for a date range.
    /// Split into one method per query — each method acquires its own Mutex lock
    /// so the borrow checker can see that `stmt` is fully consumed before `conn`
    /// drops. This is the same pattern used by all other AnalyticsDb methods.
    pub fn get_token_usage_for_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<(
        i64,
        i64,
        i64,
        Option<i64>,
        HashMap<String, i64>,
        HashMap<String, i64>,
        HashMap<String, i64>,
        Vec<SessionTokenUsage>,
    )> {
        let s = start.to_string();
        let e = end.to_string();
        let totals = self.token_totals(&s, &e);
        let by_model = self.token_by_group("model", &s, &e)?;
        let by_provider = self.token_by_group("provider", &s, &e)?;
        let by_agent = self.token_by_group("agent_type", &s, &e)?;
        let top_sessions = self.token_top_sessions(&s, &e)?;
        let (total, prompt, completion, thinking) = totals;
        Ok((
            total,
            prompt,
            completion,
            thinking,
            by_agent,
            by_model,
            by_provider,
            top_sessions,
        ))
    }

    // ── private helpers — one lock/stmt per function (mirrors existing methods) ──

    fn token_totals(&self, start: &str, end: &str) -> (i64, i64, i64, Option<i64>) {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return (0, 0, 0, None),
        };
        conn.query_row(
            "SELECT COALESCE(SUM(total_tokens),0), \
                    COALESCE(SUM(prompt_tokens),0), \
                    COALESCE(SUM(completion_tokens),0), \
                    SUM(thinking_tokens) \
             FROM token_usage \
             WHERE DATE(timestamp) BETWEEN ?1 AND ?2",
            params![start, end],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap_or((0, 0, 0, None))
    }

    fn token_by_group(&self, col: &str, start: &str, end: &str) -> Result<HashMap<String, i64>> {
        // `col` is always a trusted internal string ("model" | "provider" | "agent_type"),
        // never user input, so safe to interpolate into the query.
        let sql = format!(
            "SELECT {col}, COALESCE(SUM(total_tokens),0) \
             FROM token_usage \
             WHERE DATE(timestamp) BETWEEN ?1 AND ?2 \
             GROUP BY {col} ORDER BY 2 DESC"
        );
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {e}")))?;
        let mut stmt = conn.prepare(&sql).map_err(|e| AutoDevError::Database(e))?;
        let rows = stmt
            .query_map(params![start, end], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;
        Ok(rows.into_iter().collect())
    }

    fn token_top_sessions(&self, start: &str, end: &str) -> Result<Vec<SessionTokenUsage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT session_id, \
                    COALESCE(SUM(total_tokens),0) AS tokens, \
                    COALESCE(SUM(cost_usd),0.0)   AS cost, \
                    MAX(agent_type)               AS agent_type \
             FROM token_usage \
             WHERE DATE(timestamp) BETWEEN ?1 AND ?2 \
             GROUP BY session_id \
             ORDER BY tokens DESC LIMIT 10",
            )
            .map_err(|e| AutoDevError::Database(e))?;
        // Collect into an owned Vec before stmt/conn drop — same pattern as
        // get_daily_costs_range and other methods in this file.
        let rows = stmt
            .query_map(params![start, end], |r| {
                Ok(SessionTokenUsage {
                    session_id: r.get(0)?,
                    tokens: r.get(1)?,
                    cost_usd: r.get(2)?,
                    agent_type: r
                        .get::<_, Option<String>>(3)?
                        .unwrap_or_else(|| "unknown".into()),
                })
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;
        Ok(rows)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // System Events Operations
    // ═══════════════════════════════════════════════════════════════════════════

    /// Query recent system events, optionally filtered by component and/or severity.
    /// Returns events from the last `hours` hours, newest first, capped at 500 rows.
    pub fn get_system_events_recent(
        &self,
        component: Option<&str>,
        severity: Option<&str>,
        hours: i32,
    ) -> Result<Vec<SystemEventRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;

        // Build the cutoff timestamp string. `hours` is clamped to 1–168 by the
        // command layer so this format is safe to embed in SQL.
        let cutoff = format!("-{hours} hours");

        // Optional filters — embed as literal strings in the WHERE clause.
        // Values come from internal Tauri commands (not user web input) so SQL
        // injection risk is low; we additionally sanitise by rejecting non-ASCII
        // and stripping quotes.
        let sanitize = |s: &str| -> String {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                .collect()
        };

        let component_filter = component
            .map(|c| format!(" AND component = '{}'", sanitize(c)))
            .unwrap_or_default();
        let severity_filter = severity
            .map(|s| format!(" AND severity = '{}'", sanitize(s)))
            .unwrap_or_default();

        let sql = format!(
            "SELECT id, timestamp, component, event_type, message, severity, metadata \
             FROM system_events \
             WHERE timestamp > datetime('now', ?){component_filter}{severity_filter} \
             ORDER BY timestamp DESC LIMIT 500"
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| AutoDevError::Database(e))?;

        let records = stmt
            .query_map(params![cutoff], |row| {
                let ts_str: String = row.get(1)?;
                let timestamp =
                    crate::db::timestamps::parse_db_timestamp(&ts_str).unwrap_or_else(|e| {
                        // Substituting "now" makes an unreadable record look
                        // brand new, which is worse than useless in a cost or
                        // failure report. The fallback stays — these records
                        // are display-only and a hard error would lose the rest
                        // of the row — but it is no longer silent.
                        tracing::warn!("[analytics] unreadable timestamp {ts_str:?}: {e}");
                        Utc::now()
                    });
                Ok(SystemEventRecord {
                    id: row.get(0)?,
                    timestamp,
                    component: row.get(2)?,
                    event_type: row.get(3)?,
                    message: row.get(4)?,
                    severity: row.get(5)?,
                    metadata: row.get(6)?,
                })
            })
            .map_err(|e| AutoDevError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AutoDevError::Database(e))?;

        Ok(records)
    }

    pub fn insert_system_event(&self, record: &SystemEventRecord) -> Result<i64> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| AutoDevError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO system_events (timestamp, component, event_type, message, severity, metadata)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                record.timestamp.to_rfc3339(),
                record.component,
                record.event_type,
                record.message,
                record.severity,
                record.metadata,
            ],
        ).map_err(|e| AutoDevError::Database(e))?;

        Ok(conn.last_insert_rowid())
    }
}
