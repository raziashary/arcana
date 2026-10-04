// Analytics read-only access for arcanactl analytics commands.
// Uses the same ~/.arcana/analytics.db written by the Tauri app.

use crate::errors::Result;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub date: String,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub session_count: i64,
    pub message_count: i64,
    pub tool_call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub id: i64,
    pub timestamp: String,
    pub session_id: String,
    pub agent_type: String,
    pub provider: String,
    pub model: String,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    pub worker_id: String,
    pub date: String,
    pub tasks_completed: i64,
    pub tasks_failed: i64,
    pub time_spent_minutes: i64,
    pub cost_usd: f64,
    pub avg_task_duration_minutes: f64,
    pub success_rate: f64,
    pub last_heartbeat: Option<String>,
    pub coverage_start: String,
    pub coverage_end: String,
    pub granularity: String,
}

pub struct AnalyticsReader {
    conn: Connection,
}

impl AnalyticsReader {
    /// Build the read-only query surface from a host-configured connection.
    /// The host owns connection policy (including statement profiling).
    pub fn from_connection(conn: Connection) -> Self {
        Self { conn }
    }

    /// Cost summary for a given time window. window = "today"|"week"|"month"|"all"
    pub fn cost_summary(&self, window: &str) -> Result<Vec<CostSummary>> {
        let sql = match window {
            "today" => COST_SUMMARY_TODAY_SQL,
            "week" => COST_SUMMARY_WEEK_SQL,
            "month" => COST_SUMMARY_MONTH_SQL,
            _ => COST_SUMMARY_ALL_SQL,
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CostSummary {
                    date: row.get(0)?,
                    total_cost_usd: row.get(1)?,
                    total_tokens: row.get(2)?,
                    session_count: row.get(3)?,
                    message_count: row.get(4)?,
                    tool_call_count: row.get(5)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Token usage records, optionally filtered by session ID.
    pub fn token_usage(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<TokenUsageRecord>> {
        let (sql, use_filter) = if session_id.is_some() {
            (TOKEN_USAGE_FILTERED_SQL, true)
        } else {
            (TOKEN_USAGE_ALL_SQL, false)
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = if use_filter {
            stmt.query_map(params![session_id.unwrap(), limit], row_to_token_usage)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![limit], row_to_token_usage)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Worker statistics for the given date window.
    pub fn worker_stats(&self, window: &str) -> Result<Vec<WorkerStats>> {
        let (raw_sql, monthly_sql) = match window {
            "today" => (WORKER_STATS_RAW_TODAY_SQL, WORKER_STATS_MONTHLY_TODAY_SQL),
            "week" => (WORKER_STATS_RAW_WEEK_SQL, WORKER_STATS_MONTHLY_WEEK_SQL),
            "month" => (WORKER_STATS_RAW_MONTH_SQL, WORKER_STATS_MONTHLY_MONTH_SQL),
            _ => (WORKER_STATS_RAW_ALL_SQL, WORKER_STATS_MONTHLY_ALL_SQL),
        };
        let sql = raw_sql;
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt
            .query_map([], |row| {
                let date: String = row.get(1)?;
                let tasks_completed: i64 = row.get(2)?;
                let tasks_failed: i64 = row.get(3)?;
                let total = tasks_completed + tasks_failed;
                Ok(WorkerStats {
                    worker_id: row.get(0)?,
                    date: date.clone(),
                    tasks_completed,
                    tasks_failed,
                    time_spent_minutes: row.get(4)?,
                    cost_usd: row.get(5)?,
                    avg_task_duration_minutes: row.get::<_, f64>(6)?,
                    success_rate: if total > 0 {
                        tasks_completed as f64 / total as f64
                    } else {
                        0.0
                    },
                    last_heartbeat: row.get(8)?,
                    coverage_start: date.clone(),
                    coverage_end: date,
                    granularity: "day".into(),
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        let has_monthly_table: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'worker_stats_monthly')",
            [],
            |row| row.get(0),
        )?;
        if has_monthly_table {
            let sql = monthly_sql;
            let mut stmt = self.conn.prepare(&sql)?;
            let monthly_rows = stmt
                .query_map([], |row| {
                    Ok(WorkerStats {
                        worker_id: row.get(0)?,
                        date: row.get(1)?,
                        coverage_start: row.get(2)?,
                        coverage_end: row.get(3)?,
                        tasks_completed: row.get(4)?,
                        tasks_failed: row.get(5)?,
                        time_spent_minutes: row.get(6)?,
                        cost_usd: row.get(7)?,
                        avg_task_duration_minutes: row.get(8)?,
                        success_rate: row.get(9)?,
                        last_heartbeat: row.get(10)?,
                        granularity: "month".into(),
                    })
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.extend(monthly_rows);
        }
        rows.sort_by(|left, right| {
            right
                .coverage_start
                .cmp(&left.coverage_start)
                .then_with(|| left.worker_id.cmp(&right.worker_id))
        });
        Ok(rows)
    }

    /// Export all analytics data as JSON.
    pub fn export_all(&self) -> Result<serde_json::Value> {
        let costs = self.cost_summary("all")?;
        let tokens = self.token_usage(None, 10_000)?;
        let workers = self.worker_stats("all")?;
        let result = serde_json::json!({
            "cost_summary":   costs,
            "token_usage":    tokens,
            "worker_stats":   workers,
        });
        Ok(result)
    }
}

const COST_SUMMARY_TODAY_SQL: &str = "SELECT date, total_cost_usd, total_tokens, session_count, message_count, tool_call_count FROM daily_costs WHERE date >= date('now') ORDER BY date DESC";
const COST_SUMMARY_WEEK_SQL: &str = "SELECT date, total_cost_usd, total_tokens, session_count, message_count, tool_call_count FROM daily_costs WHERE date >= date('now', '-7 days') ORDER BY date DESC";
const COST_SUMMARY_MONTH_SQL: &str = "SELECT date, total_cost_usd, total_tokens, session_count, message_count, tool_call_count FROM daily_costs WHERE date >= date('now', '-30 days') ORDER BY date DESC";
const COST_SUMMARY_ALL_SQL: &str = "SELECT date, total_cost_usd, total_tokens, session_count, message_count, tool_call_count FROM daily_costs ORDER BY date DESC";
const TOKEN_USAGE_FILTERED_SQL: &str = "SELECT id,timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd FROM token_usage WHERE session_id=?1 ORDER BY timestamp DESC LIMIT ?2";
const TOKEN_USAGE_ALL_SQL: &str = "SELECT id,timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd FROM token_usage ORDER BY timestamp DESC LIMIT ?1";
const WORKER_STATS_RAW_TODAY_SQL: &str = "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats WHERE date >= date('now') ORDER BY date DESC, worker_id ASC";
const WORKER_STATS_RAW_WEEK_SQL: &str = "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats WHERE date >= date('now', '-7 days') ORDER BY date DESC, worker_id ASC";
const WORKER_STATS_RAW_MONTH_SQL: &str = "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats WHERE date >= date('now', '-30 days') ORDER BY date DESC, worker_id ASC";
const WORKER_STATS_RAW_ALL_SQL: &str = "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats ORDER BY date DESC, worker_id ASC";
const WORKER_STATS_MONTHLY_TODAY_SQL: &str = "SELECT worker_id, month, covered_start, covered_end, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats_monthly WHERE covered_end >= date('now') ORDER BY covered_start DESC, worker_id ASC";
const WORKER_STATS_MONTHLY_WEEK_SQL: &str = "SELECT worker_id, month, covered_start, covered_end, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats_monthly WHERE covered_end >= date('now', '-7 days') ORDER BY covered_start DESC, worker_id ASC";
const WORKER_STATS_MONTHLY_MONTH_SQL: &str = "SELECT worker_id, month, covered_start, covered_end, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats_monthly WHERE covered_end >= date('now', '-30 days') ORDER BY covered_start DESC, worker_id ASC";
const WORKER_STATS_MONTHLY_ALL_SQL: &str = "SELECT worker_id, month, covered_start, covered_end, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat FROM worker_stats_monthly ORDER BY covered_start DESC, worker_id ASC";

fn row_to_token_usage(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenUsageRecord> {
    Ok(TokenUsageRecord {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        session_id: row.get(2)?,
        agent_type: row.get(3)?,
        provider: row.get(4)?,
        model: row.get(5)?,
        prompt_tokens: row.get(6)?,
        completion_tokens: row.get(7)?,
        total_tokens: row.get(8)?,
        cost_usd: row.get(9)?,
    })
}

#[cfg(test)]
mod worker_stats_reader_tests {
    use super::*;

    fn fixture_reader(with_monthly: bool) -> AnalyticsReader {
        let conn = Connection::open_in_memory().expect("isolated SQLite fixture");
        conn.execute_batch(
            "CREATE TABLE worker_stats (
                worker_id TEXT NOT NULL, date TEXT NOT NULL, tasks_completed INTEGER NOT NULL,
                tasks_failed INTEGER NOT NULL, time_spent_minutes INTEGER NOT NULL,
                cost_usd REAL NOT NULL, avg_task_duration_minutes INTEGER NOT NULL,
                success_rate REAL NOT NULL, last_heartbeat TEXT,
                UNIQUE(worker_id, date)
             );
             INSERT INTO worker_stats VALUES
                ('worker-a', '2025-03-02', 1, 0, 10, 0.25, 10, 100.0, '2025-03-02T10:00:00Z');",
        )
        .expect("create and populate daily fixture");
        if with_monthly {
            conn.execute_batch(
                "CREATE TABLE worker_stats_monthly (
                    worker_id TEXT NOT NULL, month TEXT NOT NULL,
                    covered_start TEXT NOT NULL, covered_end TEXT NOT NULL,
                    tasks_completed INTEGER NOT NULL, tasks_failed INTEGER NOT NULL,
                    time_spent_minutes INTEGER NOT NULL, cost_usd REAL NOT NULL,
                    avg_task_duration_minutes REAL NOT NULL, success_rate REAL NOT NULL,
                    last_heartbeat TEXT, PRIMARY KEY(worker_id, month)
                 );
                 INSERT INTO worker_stats_monthly VALUES
                    ('worker-a', '2025-01', '2025-01-04', '2025-01-20',
                     3, 1, 80, 1.0, 20.0, 0.75, '2025-01-20T20:00:00Z');",
            )
            .expect("create and populate monthly fixture");
        }
        AnalyticsReader { conn }
    }

    #[test]
    fn cli_reader_composes_monthly_and_daily_rows_with_coverage_metadata() {
        let reader = fixture_reader(true);
        let rows = reader
            .worker_stats("all")
            .expect("read fixture worker stats");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].granularity, "day");
        assert_eq!(rows[0].coverage_start, "2025-03-02");
        assert_eq!(rows[0].coverage_end, "2025-03-02");
        assert_eq!(rows[0].success_rate, 1.0);
        assert_eq!(rows[1].granularity, "month");
        assert_eq!(rows[1].date, "2025-01");
        assert_eq!(rows[1].coverage_start, "2025-01-04");
        assert_eq!(rows[1].coverage_end, "2025-01-20");
        assert_eq!(rows[1].avg_task_duration_minutes, 20.0);
        assert_eq!(rows[1].success_rate, 0.75);
    }

    #[test]
    fn cli_reader_keeps_older_schema_usable_before_monthly_migration() {
        let reader = fixture_reader(false);
        let rows = reader.worker_stats("all").expect("read legacy fixture");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].granularity, "day");
    }
}

#[cfg(test)]
mod analytics_reader_plan_tests {
    use super::*;
    use rusqlite::params_from_iter;
    use rusqlite::types::Value;

    fn migrated_reader() -> AnalyticsReader {
        let conn = Connection::open_in_memory().expect("isolated SQLite fixture");
        conn.execute_batch(
            "CREATE TABLE token_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
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
             );
             CREATE INDEX idx_token_usage_timestamp ON token_usage(timestamp);
             CREATE INDEX idx_token_usage_session ON token_usage(session_id);
             CREATE INDEX idx_token_usage_agent ON token_usage(agent_type);
             CREATE INDEX idx_token_usage_julianday ON token_usage(julianday(timestamp) DESC);
             CREATE TABLE daily_costs (
                date DATE PRIMARY KEY,
                total_cost_usd REAL NOT NULL,
                total_tokens INTEGER NOT NULL,
                session_count INTEGER NOT NULL,
                message_count INTEGER NOT NULL,
                tool_call_count INTEGER NOT NULL,
                provider_breakdown TEXT,
                agent_type_breakdown TEXT,
                created_at TEXT
             );
             CREATE TABLE worker_stats (
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
             CREATE INDEX idx_worker_stats_date ON worker_stats(date);
             CREATE INDEX idx_worker_stats_worker ON worker_stats(worker_id);
             CREATE TABLE worker_stats_monthly (
                worker_id TEXT NOT NULL,
                month TEXT NOT NULL,
                covered_start TEXT NOT NULL,
                covered_end TEXT NOT NULL,
                tasks_completed INTEGER NOT NULL,
                tasks_failed INTEGER NOT NULL,
                time_spent_minutes INTEGER NOT NULL,
                cost_usd REAL NOT NULL,
                avg_task_duration_minutes REAL NOT NULL,
                success_rate REAL NOT NULL,
                last_heartbeat TEXT,
                PRIMARY KEY(worker_id, month)
             );
             CREATE INDEX idx_worker_stats_monthly_coverage
                ON worker_stats_monthly(covered_start, covered_end);",
        )
        .expect("create production-equivalent analytics schema");

        {
            let tx = conn.unchecked_transaction().expect("fixture transaction");
            {
                let mut insert = tx
                    .prepare(
                        "INSERT INTO daily_costs
                         (date,total_cost_usd,total_tokens,session_count,message_count,tool_call_count)
                         VALUES (date('now',?1),?2,?3,?4,?5,?6)",
                    )
                    .expect("prepare daily costs");
                for age in 0..=400 {
                    insert
                        .execute(params![
                            format!("-{age} days"),
                            age as f64 / 100.0,
                            age,
                            3,
                            5,
                            2
                        ])
                        .expect("populate daily costs");
                }
            }
            {
                let mut insert = tx
                    .prepare(
                        "INSERT INTO token_usage
                         (timestamp,session_id,agent_type,provider,model,prompt_tokens,
                          completion_tokens,total_tokens,thinking_tokens,cost_usd,request_type)
                         VALUES (strftime('%Y-%m-%dT%H:%M:%fZ','now',?1),?2,'agent','test','model',
                                 ?3,?4,?5,0,0.01,'completion')",
                    )
                    .expect("prepare token usage");
                for age in 0..5000 {
                    insert
                        .execute(params![
                            format!("-{age} minutes"),
                            format!("session-{:02}", age % 17),
                            age % 100,
                            20,
                            age % 100 + 20
                        ])
                        .expect("populate token usage");
                }
            }
            {
                let mut insert = tx
                    .prepare(
                        "INSERT INTO worker_stats
                         (worker_id,date,tasks_completed,tasks_failed,time_spent_minutes,cost_usd,
                          avg_task_duration_minutes,success_rate,last_heartbeat)
                         VALUES (?1,date('now',?2),?3,1,30,0.5,10,0.75,datetime('now'))",
                    )
                    .expect("prepare daily worker stats");
                for worker in 0..48 {
                    for age in 0..=400 {
                        insert
                            .execute(params![
                                format!("worker-{worker:02}"),
                                format!("-{age} days"),
                                age % 5
                            ])
                            .expect("populate daily worker stats");
                    }
                }
            }
            {
                let mut insert = tx
                    .prepare(
                        "INSERT INTO worker_stats_monthly
                         (worker_id,month,covered_start,covered_end,tasks_completed,tasks_failed,
                          time_spent_minutes,cost_usd,avg_task_duration_minutes,success_rate,last_heartbeat)
                         VALUES (?1,strftime('%Y-%m',date('now',printf('-%d months',?2),'start of month')),
                            date('now',printf('-%d months',?2),'start of month'),
                            date(date('now',printf('-%d months',?2),'start of month'),'+1 month','-1 day'),
                            12,2,120,2.5,10,0.857,'2025-01-01T00:00:00Z')",
                    )
                    .expect("prepare monthly worker stats");
                for worker in 0..48 {
                    for months_ago in 0..=48 {
                        insert
                            .execute(params![format!("worker-{worker:02}"), months_ago])
                            .expect("populate monthly worker stats");
                    }
                }
            }
            tx.commit().expect("commit fixture rows");
        }
        conn.execute_batch("ANALYZE;")
            .expect("analyze populated fixture");
        AnalyticsReader { conn }
    }

    fn explain_details(conn: &Connection, sql: &str, params: &[Value]) -> Vec<String> {
        let explain = format!("EXPLAIN QUERY PLAN {sql}");
        let mut stmt = conn.prepare(&explain).expect("prepare query plan");
        stmt.query_map(params_from_iter(params.iter()), |row| row.get(3))
            .expect("explain production SQL")
            .collect::<std::result::Result<Vec<String>, _>>()
            .expect("collect query plan")
    }

    fn assert_index_plan(table: &str, details: &[String]) {
        let details = details.join(" | ");
        assert!(
            details.contains(&format!("{table}")) && details.contains("USING"),
            "expected an indexed plan for {table}; plan was: {details}"
        );
    }

    #[test]
    fn cli_analytics_readers_use_populated_indexes_and_return_production_results() {
        let reader = migrated_reader();

        for window in ["today", "week", "month", "all"] {
            let sql = match window {
                "today" => COST_SUMMARY_TODAY_SQL,
                "week" => COST_SUMMARY_WEEK_SQL,
                "month" => COST_SUMMARY_MONTH_SQL,
                _ => COST_SUMMARY_ALL_SQL,
            };
            assert_index_plan("daily_costs", &explain_details(&reader.conn, &sql, &[]));
            let rows = reader.cost_summary(window).expect("read production costs");
            let expected_count = match window {
                "today" => 1,
                "week" => 8,
                "month" => 31,
                _ => 401,
            };
            assert_eq!(rows.len(), expected_count, "cost window {window}");
            let today: String = reader
                .conn
                .query_row("SELECT date('now')", [], |row| row.get(0))
                .expect("read fixture date");
            assert_eq!(rows[0].date, today);
        }
        assert_eq!(reader.cost_summary("invalid-window").unwrap().len(), 401);

        for (session, sql, params) in [
            (
                Some("session-07"),
                TOKEN_USAGE_FILTERED_SQL,
                vec![Value::Text("session-07".into()), Value::Integer(9)],
            ),
            (None, TOKEN_USAGE_ALL_SQL, vec![Value::Integer(9)]),
        ] {
            assert_index_plan("token_usage", &explain_details(&reader.conn, sql, &params));
            let rows = reader
                .token_usage(session, 9)
                .expect("read production token usage");
            assert_eq!(rows.len(), 9);
            if let Some(session) = session {
                assert!(rows.iter().all(|row| row.session_id == session));
            } else {
                assert!(rows
                    .windows(2)
                    .all(|pair| pair[0].timestamp >= pair[1].timestamp));
            }
        }
        assert!(reader
            .token_usage(Some("missing-session"), 9)
            .expect("read empty filtered session")
            .is_empty());

        for window in ["today", "week", "month", "all"] {
            let (raw_sql, monthly_sql) = match window {
                "today" => (WORKER_STATS_RAW_TODAY_SQL, WORKER_STATS_MONTHLY_TODAY_SQL),
                "week" => (WORKER_STATS_RAW_WEEK_SQL, WORKER_STATS_MONTHLY_WEEK_SQL),
                "month" => (WORKER_STATS_RAW_MONTH_SQL, WORKER_STATS_MONTHLY_MONTH_SQL),
                _ => (WORKER_STATS_RAW_ALL_SQL, WORKER_STATS_MONTHLY_ALL_SQL),
            };
            assert_index_plan(
                "worker_stats",
                &explain_details(&reader.conn, &raw_sql, &[]),
            );
            assert_index_plan(
                "worker_stats_monthly",
                &explain_details(&reader.conn, &monthly_sql, &[]),
            );
            let rows = reader
                .worker_stats(window)
                .expect("read production workers");
            assert!(rows.iter().any(|row| row.granularity == "day"));
            assert!(rows.iter().any(|row| row.granularity == "month"));
            if window == "today" {
                assert!(rows
                    .iter()
                    .all(|row| { row.granularity != "day" || row.date == row.coverage_end }));
            }
        }

        let exported = reader.export_all().expect("export production analytics");
        assert_eq!(exported["cost_summary"].as_array().unwrap().len(), 401);
        assert_eq!(exported["token_usage"].as_array().unwrap().len(), 5000);
        let exported_workers = exported["worker_stats"].as_array().unwrap();
        assert_eq!(exported_workers.len(), 48 * (401 + 49));
        assert!(exported_workers
            .iter()
            .any(|row| row["granularity"] == "month"));
        assert!(exported_workers
            .iter()
            .any(|row| row["granularity"] == "day"));
    }
}
