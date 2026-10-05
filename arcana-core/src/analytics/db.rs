// Analytics database — SQLite schema and migrations for cost tracking
use super::types::*;
use crate::db::arcana_dir;
use crate::db::migrate::{self, Migration};
use crate::errors::{ArcanaError, Result};
use crate::observability::timing::OperationTimer;
use chrono::{Local, NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};

const APPEND_ONLY_INVALID_TIMESTAMPS_SQL: &str = "SELECT (SELECT COUNT(*) FROM token_usage WHERE julianday(timestamp) IS NULL) + (SELECT COUNT(*) FROM system_events WHERE julianday(timestamp) IS NULL) + (SELECT COUNT(*) FROM api_failures WHERE julianday(timestamp) IS NULL)";
const APPEND_ONLY_TOKEN_DAILY_SUMMARY_SQL: &str = "SELECT date(timestamp, 'localtime'), SUM(cost_usd), SUM(total_tokens), COUNT(DISTINCT session_id), COUNT(*), SUM(CASE WHEN request_type = 'tool_use' THEN 1 ELSE 0 END) FROM token_usage WHERE julianday(timestamp) < julianday(?1) AND id > COALESCE((SELECT covered_through_token_id FROM daily_cost_summary_coverage WHERE date = date(token_usage.timestamp, 'localtime')), 0) GROUP BY date(timestamp, 'localtime')";
const APPEND_ONLY_PROVIDER_COSTS_SQL: &str = "SELECT date(timestamp, 'localtime'), provider, SUM(cost_usd) FROM token_usage WHERE julianday(timestamp) < julianday(?1) AND id > COALESCE((SELECT covered_through_token_id FROM daily_cost_summary_coverage WHERE date = date(token_usage.timestamp, 'localtime')), 0) GROUP BY date(timestamp, 'localtime'), provider";
const APPEND_ONLY_AGENT_COSTS_SQL: &str = "SELECT date(timestamp, 'localtime'), agent_type, SUM(cost_usd) FROM token_usage WHERE julianday(timestamp) < julianday(?1) AND id > COALESCE((SELECT covered_through_token_id FROM daily_cost_summary_coverage WHERE date = date(token_usage.timestamp, 'localtime')), 0) GROUP BY date(timestamp, 'localtime'), agent_type";
const APPEND_ONLY_PRUNE_TOKEN_USAGE_SQL: &str =
    "DELETE FROM token_usage WHERE julianday(timestamp) < julianday(?1)";
const APPEND_ONLY_PRUNE_SYSTEM_EVENTS_SQL: &str =
    "DELETE FROM system_events WHERE julianday(timestamp) < julianday(?1)";
const APPEND_ONLY_PRUNE_API_FAILURES_SQL: &str =
    "DELETE FROM api_failures WHERE julianday(timestamp) < julianday(?1)";
use std::collections::HashMap;
use std::sync::Mutex;

const DAILY_COSTS_RANGE_SQL: &str =
    "SELECT date, total_cost_usd, total_tokens, session_count, message_count,
             tool_call_count, provider_breakdown, agent_type_breakdown
             FROM daily_costs WHERE date BETWEEN ?1 AND ?2 ORDER BY date ASC";
const DAILY_COST_BY_DATE_SQL: &str =
    "SELECT date, total_cost_usd, total_tokens, session_count, message_count,
             tool_call_count, provider_breakdown, agent_type_breakdown
             FROM daily_costs WHERE date = ?1";
const TOKEN_USAGE_BY_LOCAL_DATE_SQL: &str =
    "SELECT id, timestamp, session_id, agent_type, provider, model, prompt_tokens,
             completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type
       FROM (
         SELECT id, timestamp, session_id, agent_type, provider, model, prompt_tokens,
                completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type
           FROM token_usage
          WHERE timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T'
         UNION ALL
         SELECT id, timestamp, session_id, agent_type, provider, model, prompt_tokens,
                completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type
           FROM token_usage
          WHERE timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' '
         UNION ALL
         SELECT id, timestamp, session_id, agent_type, provider, model, prompt_tokens,
                completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type
           FROM token_usage WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL
       ) ORDER BY julianday(timestamp) DESC";
const TOKEN_TOTALS_RANGE_SQL: &str = "SELECT COALESCE(SUM(total_tokens),0), \
                    COALESCE(SUM(prompt_tokens),0), \
                    COALESCE(SUM(completion_tokens),0), \
                    SUM(thinking_tokens) \
             FROM token_usage \
             WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
                OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ')";
const TOKEN_ACTIVITY_SESSIONS_SQL: &str = "SELECT COUNT(DISTINCT session_id) FROM token_usage \
     WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
        OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ')";
const TOKEN_ACTIVITY_MESSAGES_SQL: &str = "SELECT COUNT(*) FROM token_usage \
     WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
        OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ')";
const TOKEN_ACTIVITY_TOOL_CALLS_SQL: &str = "SELECT COUNT(*) FROM token_usage \
             WHERE ((timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
                 OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ')) \
               AND request_type IN ('tool_use', 'tool', 'function_call')";
const TOKEN_TOP_SESSIONS_SQL: &str = "SELECT session_id, \
                    COALESCE(SUM(total_tokens),0) AS tokens, \
                    COALESCE(SUM(cost_usd),0.0)   AS cost, \
                    MAX(agent_type)               AS agent_type \
             FROM token_usage \
             WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
                OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ') \
             GROUP BY session_id \
             ORDER BY tokens DESC LIMIT 10";
const TOKEN_USAGE_FOR_SESSION_SQL: &str = "SELECT COUNT(*), \
                    COALESCE(SUM(prompt_tokens),0), \
                    COALESCE(SUM(completion_tokens),0), \
                    COALESCE(SUM(total_tokens),0), \
                    SUM(thinking_tokens), \
                    COALESCE(SUM(cost_usd),0.0) \
             FROM token_usage WHERE session_id = ?1";
const WORKER_STATS_RANGE_SQL: &str = "SELECT
                 worker_id,
                 SUM(tasks_completed)        AS tasks_completed,
                 SUM(tasks_failed)           AS tasks_failed,
                 SUM(time_spent_minutes)     AS time_spent_minutes,
                 SUM(cost_usd)               AS cost_usd,
                 AVG(avg_task_duration_minutes) AS avg_duration,
                 (SELECT history.last_heartbeat FROM worker_stats history
                  WHERE history.worker_id = worker_stats.worker_id
                    AND history.date BETWEEN ?2 AND ?3
                    AND history.last_heartbeat IS NOT NULL
                  ORDER BY julianday(history.last_heartbeat) DESC LIMIT 1) AS last_heartbeat
             FROM worker_stats
             WHERE worker_id = ?1 AND date BETWEEN ?2 AND ?3
             GROUP BY worker_id";
const API_FAILURES_RECENT_SQL: &str =
    "SELECT id, timestamp, provider, error_type, error_message, request_id, retry_count, resolved
             FROM (
               SELECT id, timestamp, provider, error_type, error_message, request_id, retry_count, resolved
                 FROM api_failures WHERE julianday(timestamp) > julianday('now', ?1 || ' hours')
               UNION ALL
               SELECT id, timestamp, provider, error_type, error_message, request_id, retry_count, resolved
                 FROM api_failures WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL
             ) ORDER BY julianday(timestamp) DESC";
const SYSTEM_EVENTS_RECENT_SQL: &str =
    "SELECT id, timestamp, component, event_type, message, severity, metadata \
             FROM ( \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE julianday(timestamp) > julianday('now', ?1) \
               UNION ALL \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL \
             ) ORDER BY julianday(timestamp) DESC LIMIT 500";
const SYSTEM_EVENTS_RECENT_BY_COMPONENT_SQL: &str =
    "SELECT id, timestamp, component, event_type, message, severity, metadata \
             FROM ( \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE julianday(timestamp) > julianday('now', ?1) AND component = ?2 \
               UNION ALL \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL AND component = ?2 \
             ) ORDER BY julianday(timestamp) DESC LIMIT 500";
const SYSTEM_EVENTS_RECENT_BY_SEVERITY_SQL: &str =
    "SELECT id, timestamp, component, event_type, message, severity, metadata \
             FROM ( \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE julianday(timestamp) > julianday('now', ?1) AND severity = ?2 \
               UNION ALL \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL AND severity = ?2 \
             ) ORDER BY julianday(timestamp) DESC LIMIT 500";
const SYSTEM_EVENTS_RECENT_BY_COMPONENT_AND_SEVERITY_SQL: &str =
    "SELECT id, timestamp, component, event_type, message, severity, metadata \
             FROM ( \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE julianday(timestamp) > julianday('now', ?1) AND component = ?2 AND severity = ?3 \
               UNION ALL \
               SELECT id, timestamp, component, event_type, message, severity, metadata \
                 FROM system_events WHERE timestamp IS NOT NULL AND julianday(timestamp) IS NULL AND component = ?2 AND severity = ?3 \
             ) ORDER BY julianday(timestamp) DESC LIMIT 500";

#[cfg(test)]
fn system_events_recent_sql(has_component: bool, has_severity: bool) -> &'static str {
    match (has_component, has_severity) {
        (false, false) => SYSTEM_EVENTS_RECENT_SQL,
        (true, false) => SYSTEM_EVENTS_RECENT_BY_COMPONENT_SQL,
        (false, true) => SYSTEM_EVENTS_RECENT_BY_SEVERITY_SQL,
        (true, true) => SYSTEM_EVENTS_RECENT_BY_COMPONENT_AND_SEVERITY_SQL,
    }
}

fn local_boundary_utc(date: NaiveDate) -> Option<chrono::DateTime<Utc>> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    for minute in 0..=24 * 60 {
        let local = midnight.checked_add_signed(chrono::Duration::minutes(minute))?;
        match Local.from_local_datetime(&local) {
            chrono::LocalResult::Single(value) => return Some(value.with_timezone(&Utc)),
            chrono::LocalResult::Ambiguous(first, second) => {
                return Some(std::cmp::min(first, second).with_timezone(&Utc));
            }
            chrono::LocalResult::None => {}
        }
    }
    None
}

/// Produce lexical UTC timestamp ranges for both canonical RFC3339 and legacy
/// SQLite `CURRENT_TIMESTAMP` rows. Both formats are written from UTC instants;
/// using a local calendar boundary keeps the previous `DATE(..., 'localtime')`
/// behavior while allowing the timestamp index to serve the query.
fn local_date_range_bounds(start: &str, end: &str) -> Option<[String; 4]> {
    let start = NaiveDate::parse_from_str(start, "%Y-%m-%d").ok()?;
    let end = NaiveDate::parse_from_str(end, "%Y-%m-%d")
        .ok()?
        .succ_opt()?;
    if end <= start {
        return None;
    }
    let start = local_boundary_utc(start)?.naive_utc();
    let end = local_boundary_utc(end)?.naive_utc();
    Some([
        start.format("%Y-%m-%dT%H:%M:%S").to_string(),
        end.format("%Y-%m-%dT%H:%M:%S").to_string(),
        start.format("%Y-%m-%d %H:%M:%S").to_string(),
        end.format("%Y-%m-%d %H:%M:%S").to_string(),
    ])
}

const TOKEN_USAGE_BY_MODEL_SQL: &str = "SELECT model, COALESCE(SUM(total_tokens),0) \
     FROM token_usage \
     WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
        OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ') \
     GROUP BY model ORDER BY 2 DESC";
const TOKEN_USAGE_BY_PROVIDER_SQL: &str = "SELECT provider, COALESCE(SUM(total_tokens),0) \
     FROM token_usage \
     WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
        OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ') \
     GROUP BY provider ORDER BY 2 DESC";
const TOKEN_USAGE_BY_AGENT_TYPE_SQL: &str = "SELECT agent_type, COALESCE(SUM(total_tokens),0) \
     FROM token_usage \
     WHERE (timestamp >= ?1 AND timestamp < ?2 AND substr(timestamp,11,1) = 'T') \
        OR (timestamp >= ?3 AND timestamp < ?4 AND substr(timestamp,11,1) = ' ') \
     GROUP BY agent_type ORDER BY 2 DESC";

pub struct AnalyticsDb {
    pub conn: Mutex<Connection>,
    // Doc 02 Phase 3.4 — surface the on-disk path so the one-shot
    // diagnostic log can answer "is this hitting the right DB?" without
    // re-deriving arcana_dir().
    pub path: std::path::PathBuf,
}

/// Aggregate returned by the production Tauri worker-range reader.
/// `granularity == "month"` means one or more monthly buckets overlap the
/// requested range; coverage bounds describe the actual rolled data and can
/// extend beyond a partial range request.
#[derive(Debug, Clone)]
pub struct WorkerStatsRange {
    pub worker_id: String,
    pub date: NaiveDate,
    pub tasks_completed: i64,
    pub tasks_failed: i64,
    pub time_spent_minutes: i64,
    pub cost_usd: f64,
    pub avg_task_duration_minutes: f64,
    pub success_rate: f64,
    pub last_heartbeat: Option<chrono::DateTime<Utc>>,
    pub coverage_start: NaiveDate,
    pub coverage_end: NaiveDate,
    pub granularity: &'static str,
}

fn token_usage_records_for_date(
    conn: &Connection,
    date: NaiveDate,
) -> Result<Vec<TokenUsageRecord>> {
    let bounds = local_date_range_bounds(&date.to_string(), &date.to_string())
        .ok_or_else(|| ArcanaError::Agent("Invalid local date for token query".into()))?;
    let mut stmt = conn
        .prepare(TOKEN_USAGE_BY_LOCAL_DATE_SQL)
        .map_err(ArcanaError::Database)?;
    let rows = stmt
        .query_map(
            params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
            |row| {
                let timestamp = crate::db::timestamps::parse_row_timestamp(row, 1)?;
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
            },
        )
        .map_err(ArcanaError::Database)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(ArcanaError::Database)
}

fn aggregate_token_usage(
    date: NaiveDate,
    records: &[TokenUsageRecord],
) -> (DailyCostRecord, Option<i64>) {
    let mut summary = DailyCostRecord {
        date,
        total_cost_usd: 0.0,
        total_tokens: 0,
        session_count: 0,
        message_count: 0,
        tool_call_count: 0,
        provider_breakdown: HashMap::new(),
        agent_type_breakdown: HashMap::new(),
    };
    let mut sessions = std::collections::HashSet::new();
    let mut max_id = None;
    for record in records {
        summary.total_cost_usd += record.cost_usd;
        summary.total_tokens += record.total_tokens as i64;
        summary.message_count += 1;
        if record.request_type == "tool_use" {
            summary.tool_call_count += 1;
        }
        if sessions.insert(record.session_id.clone()) {
            summary.session_count += 1;
        }
        *summary
            .provider_breakdown
            .entry(record.provider.clone())
            .or_default() += record.cost_usd;
        *summary
            .agent_type_breakdown
            .entry(record.agent_type.clone())
            .or_default() += record.cost_usd;
        if let Some(id) = record.id {
            max_id = Some(max_id.map_or(id, |max: i64| max.max(id)));
        }
    }
    (summary, max_id)
}

fn daily_cost_records_match(left: &DailyCostRecord, right: &DailyCostRecord) -> bool {
    fn costs_match(left: &HashMap<String, f64>, right: &HashMap<String, f64>) -> bool {
        left.len() == right.len()
            && left.iter().all(|(key, value)| {
                right
                    .get(key)
                    .is_some_and(|other| (value - other).abs() <= 1e-9)
            })
    }

    left.date == right.date
        && (left.total_cost_usd - right.total_cost_usd).abs() <= 1e-9
        && left.total_tokens == right.total_tokens
        && left.session_count == right.session_count
        && left.message_count == right.message_count
        && left.tool_call_count == right.tool_call_count
        && costs_match(&left.provider_breakdown, &right.provider_breakdown)
        && costs_match(&left.agent_type_breakdown, &right.agent_type_breakdown)
}

fn write_daily_cost_summary(
    tx: &rusqlite::Transaction<'_>,
    record: &DailyCostRecord,
) -> Result<()> {
    let provider_json =
        serde_json::to_string(&record.provider_breakdown).unwrap_or_else(|_| "{}".to_string());
    let agent_json =
        serde_json::to_string(&record.agent_type_breakdown).unwrap_or_else(|_| "{}".to_string());
    tx.execute(
        "INSERT OR REPLACE INTO daily_costs (date, total_cost_usd, total_tokens, session_count,
         message_count, tool_call_count, provider_breakdown, agent_type_breakdown)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
    )
    .map_err(ArcanaError::Database)?;
    Ok(())
}

fn mark_daily_cost_summary_coverage(
    tx: &rusqlite::Transaction<'_>,
    date: NaiveDate,
    covered_through: i64,
) -> Result<()> {
    let date = date.to_string();
    tx.execute(
        "INSERT INTO daily_cost_summary_coverage (date, covered_through_token_id)
         VALUES (?1, ?2)
         ON CONFLICT(date) DO UPDATE SET covered_through_token_id = MAX(
             daily_cost_summary_coverage.covered_through_token_id,
             excluded.covered_through_token_id
         )",
        params![date, covered_through],
    )
    .map_err(ArcanaError::Database)?;
    tx.execute(
        "INSERT OR IGNORE INTO daily_cost_summary_sessions (date, session_id)
         SELECT ?1, session_id FROM token_usage
          WHERE date(timestamp, 'localtime') = ?1",
        [&date],
    )
    .map_err(ArcanaError::Database)?;
    Ok(())
}

fn adopt_verified_legacy_daily_cost_coverage(
    tx: &rusqlite::Transaction<'_>,
    date_text: &str,
) -> Result<bool> {
    let Ok(date) = NaiveDate::parse_from_str(date_text, "%Y-%m-%d") else {
        return Ok(false);
    };
    let records = token_usage_records_for_date(tx, date)?;
    if records.is_empty() {
        return Ok(false);
    }
    let (source_summary, Some(covered_through)) = aggregate_token_usage(date, &records) else {
        return Ok(false);
    };
    let stored_summary = tx
        .query_row(
            "SELECT total_cost_usd, total_tokens, session_count, message_count,
                    tool_call_count, provider_breakdown, agent_type_breakdown
               FROM daily_costs WHERE date = ?1",
            [date_text],
            |row| {
                let provider_json: Option<String> = row.get(5)?;
                let agent_json: Option<String> = row.get(6)?;
                Ok(DailyCostRecord {
                    date,
                    total_cost_usd: row.get(0)?,
                    total_tokens: row.get(1)?,
                    session_count: row.get(2)?,
                    message_count: row.get(3)?,
                    tool_call_count: row.get(4)?,
                    provider_breakdown: provider_json
                        .as_deref()
                        .and_then(|json| serde_json::from_str(json).ok())
                        .unwrap_or_default(),
                    agent_type_breakdown: agent_json
                        .as_deref()
                        .and_then(|json| serde_json::from_str(json).ok())
                        .unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(ArcanaError::Database)?;
    let Some(stored_summary) = stored_summary else {
        return Ok(false);
    };
    if !daily_cost_records_match(&source_summary, &stored_summary) {
        return Ok(false);
    }
    mark_daily_cost_summary_coverage(tx, date, covered_through)?;
    Ok(true)
}

impl AnalyticsDb {
    /// Initialize analytics database with schema
    pub async fn new() -> Result<Self> {
        Self::new_at(&arcana_dir()?.join("analytics.db")).await
    }

    /// As `new`, against an explicit path. Exists so tests never write to the
    /// user's real `~/.arcana/analytics.db`.
    pub async fn new_at(db_path: &std::path::Path) -> Result<Self> {
        let db_path = db_path.to_path_buf();
        let conn = crate::db::open_tuned(&db_path)?;

        // Enable foreign keys
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map_err(|e| ArcanaError::Database(e))?;

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
    const LADDER: &[Migration] = &[
        Migration {
            version: 1,
            name: "analytics-baseline",
            sql: r#"
            CREATE TABLE IF NOT EXISTS token_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
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
                -- but `sessions` lives in `arcana.db`, not `analytics.db`, so the FK
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
                created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
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
                timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                provider TEXT NOT NULL,
                error_type TEXT NOT NULL,
                error_message TEXT NOT NULL,
                request_id TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                resolved BOOLEAN NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS system_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
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
        },
        Migration {
            version: 2,
            name: "canonical-utc-timestamp-defaults",
            sql: r#"
            CREATE TRIGGER IF NOT EXISTS analytics_token_usage_timestamp_default
            AFTER INSERT ON token_usage
            WHEN NEW.timestamp = datetime('now')
            BEGIN UPDATE token_usage SET timestamp = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
            CREATE TRIGGER IF NOT EXISTS analytics_api_failures_timestamp_default
            AFTER INSERT ON api_failures
            WHEN NEW.timestamp = datetime('now')
            BEGIN UPDATE api_failures SET timestamp = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
            CREATE TRIGGER IF NOT EXISTS analytics_system_events_timestamp_default
            AFTER INSERT ON system_events
            WHEN NEW.timestamp = datetime('now')
            BEGIN UPDATE system_events SET timestamp = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = NEW.id; END;
            CREATE TRIGGER IF NOT EXISTS analytics_daily_costs_created_default
            AFTER INSERT ON daily_costs
            WHEN NEW.created_at = datetime('now')
            BEGIN UPDATE daily_costs SET created_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE date = NEW.date; END;
        "#,
        },
        Migration {
            version: 3,
            name: "recent-event-julianday-indexes",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_api_failures_julianday
                ON api_failures(julianday(timestamp) DESC);
            CREATE INDEX IF NOT EXISTS idx_system_events_julianday
                ON system_events(julianday(timestamp) DESC);
            CREATE INDEX IF NOT EXISTS idx_token_usage_julianday
                ON token_usage(julianday(timestamp) DESC);
        "#,
        },
        Migration {
            version: 4,
            name: "worker-stats-monthly-rollups",
            sql: r#"
            CREATE TABLE IF NOT EXISTS worker_stats_monthly (
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
                PRIMARY KEY (worker_id, month)
            );
            CREATE INDEX IF NOT EXISTS idx_worker_stats_monthly_coverage
                ON worker_stats_monthly(covered_start, covered_end);
        "#,
        },
        Migration {
            version: 5,
            name: "token-usage-session-chronological-index",
            sql: r#"
            CREATE INDEX IF NOT EXISTS idx_token_usage_session_julianday
                ON token_usage(session_id, julianday(timestamp) DESC);
        "#,
        },
        Migration {
            version: 6,
            name: "daily-cost-retention-coverage",
            sql: r#"
            -- Coverage is written only by producers that know which source
            -- snapshot they summarized. Legacy summaries remain unmarked
            -- because their source-row coverage cannot be reconstructed.
            CREATE TABLE IF NOT EXISTS daily_cost_summary_coverage (
                date TEXT PRIMARY KEY,
                covered_through_token_id INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS daily_cost_summary_sessions (
                date TEXT NOT NULL,
                session_id TEXT NOT NULL,
                PRIMARY KEY (date, session_id)
            );
        "#,
        },
        Migration {
            version: 7,
            name: "seal-expired-daily-cost-summaries",
            sql: r#"
            CREATE TABLE IF NOT EXISTS daily_cost_summary_sealed (
                date TEXT PRIMARY KEY,
                sealed_at TEXT NOT NULL
            );
            CREATE TRIGGER IF NOT EXISTS analytics_token_usage_reject_sealed_date
            BEFORE INSERT ON token_usage
            WHEN EXISTS (
                SELECT 1 FROM daily_cost_summary_sealed
                 WHERE date = date(NEW.timestamp, 'localtime')
            )
            BEGIN
                SELECT RAISE(ABORT, 'token usage date is sealed');
            END;
        "#,
        },
        Migration {
            version: 8,
            name: "reject-token-updates-into-sealed-dates",
            sql: r#"
            CREATE TRIGGER IF NOT EXISTS analytics_token_usage_reject_sealed_date_update
            BEFORE UPDATE ON token_usage
            WHEN EXISTS (
                SELECT 1 FROM daily_cost_summary_sealed
                 WHERE date = date(NEW.timestamp, 'localtime')
            )
            BEGIN
                SELECT RAISE(ABORT, 'token usage date is sealed');
            END;
        "#,
        },
    ];

    async fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        migrate::run_ladder(&conn, "analytics.db", Self::LADDER)?;
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Token Usage Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_token_usage(&self, record: &TokenUsageRecord) -> Result<i64> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_token_usage_insert")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model, 
             prompt_tokens, completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                crate::db::timestamps::format_utc_timestamp(record.timestamp),
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
        ).map_err(|e| ArcanaError::Database(e))?;

        let id = conn.last_insert_rowid();
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(id)
    }

    pub fn get_token_usage_by_date(&self, date: NaiveDate) -> Result<Vec<TokenUsageRecord>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_token_usage_date_read")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let bounds = local_date_range_bounds(&date.to_string(), &date.to_string())
            .ok_or_else(|| ArcanaError::Agent("Invalid local date for token query".into()))?;
        let mut stmt = conn
            .prepare(TOKEN_USAGE_BY_LOCAL_DATE_SQL)
            .map_err(|e| ArcanaError::Database(e))?;

        let records = stmt
            .query_map(
                params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
                |row| {
                    let timestamp = crate::db::timestamps::parse_row_timestamp(row, 1)?;
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
                },
            )
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;

        operation = operation.with_result_count(records.len());
        operation.succeed();
        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Daily Costs Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_daily_cost(&self, record: &DailyCostRecord) -> Result<()> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_daily_cost_insert")
                .with_database("analytics");
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(ArcanaError::Database)?;
        let source_records = token_usage_records_for_date(&tx, record.date)?;
        let (source_summary, covered_through) = aggregate_token_usage(record.date, &source_records);
        write_daily_cost_summary(&tx, record)?;
        if daily_cost_records_match(&source_summary, record) {
            if let Some(covered_through) = covered_through {
                mark_daily_cost_summary_coverage(&tx, record.date, covered_through)?;
            }
        } else if !source_records.is_empty() {
            // A caller-provided summary that does not equal the raw snapshot
            // cannot establish which source rows it already includes.
            tx.execute(
                "DELETE FROM daily_cost_summary_coverage WHERE date = ?1",
                [record.date.to_string()],
            )
            .map_err(ArcanaError::Database)?;
        }
        tx.commit().map_err(ArcanaError::Database)?;

        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(())
    }

    /// Recompute and persist a complete local-day summary from one immediate
    /// transaction snapshot, including the exact token ID boundary covered.
    pub fn rollup_daily_cost_for_date(&self, date: NaiveDate) -> Result<usize> {
        let mut operation =
            OperationTimer::start("database_daily_cost_insert").with_database("analytics");
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {e}")))?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(ArcanaError::Database)?;
        let is_sealed: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM daily_cost_summary_sealed WHERE date = ?1)",
                [date.to_string()],
                |row| row.get(0),
            )
            .map_err(ArcanaError::Database)?;
        if is_sealed {
            return Err(ArcanaError::Agent(format!(
                "Analytics daily rollup refused sealed date {date}"
            )));
        }
        let records = token_usage_records_for_date(&tx, date)?;
        if records.is_empty() {
            tx.commit().map_err(ArcanaError::Database)?;
            operation = operation.with_result_count(0);
            operation.succeed();
            return Ok(0);
        }
        let count = records.len();
        let (summary, covered_through) = aggregate_token_usage(date, &records);
        write_daily_cost_summary(&tx, &summary)?;
        if let Some(covered_through) = covered_through {
            mark_daily_cost_summary_coverage(&tx, date, covered_through)?;
        }
        tx.commit().map_err(ArcanaError::Database)?;
        operation = operation.with_result_count(count);
        operation.succeed();
        Ok(count)
    }

    pub fn get_daily_cost(&self, date: NaiveDate) -> Result<Option<DailyCostRecord>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_daily_cost_read")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(DAILY_COST_BY_DATE_SQL)
            .map_err(|e| ArcanaError::Database(e))?;

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
            .map_err(|e| ArcanaError::Database(e))?;

        operation = operation.with_result_count(usize::from(record.is_some()));
        operation.succeed();
        Ok(record)
    }

    pub fn get_daily_costs_range(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<DailyCostRecord>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_daily_cost_range_read")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(DAILY_COSTS_RANGE_SQL)
            .map_err(|e| ArcanaError::Database(e))?;

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
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;

        operation = operation.with_result_count(records.len());
        operation.succeed();
        Ok(records)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // Worker Stats Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_worker_stats(&self, record: &WorkerStatsRecord) -> Result<()> {
        let operation =
            OperationTimer::start("database_worker_stats_insert").with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO worker_stats (worker_id, date, tasks_completed, tasks_failed,
             time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(worker_id, date) DO UPDATE SET
                tasks_completed = worker_stats.tasks_completed + excluded.tasks_completed,
                tasks_failed = worker_stats.tasks_failed + excluded.tasks_failed,
                time_spent_minutes = worker_stats.time_spent_minutes + excluded.time_spent_minutes,
                cost_usd = worker_stats.cost_usd + excluded.cost_usd,
                avg_task_duration_minutes = CASE
                    WHEN worker_stats.tasks_completed + worker_stats.tasks_failed
                       + excluded.tasks_completed + excluded.tasks_failed > 0
                    THEN (
                        worker_stats.avg_task_duration_minutes
                          * (worker_stats.tasks_completed + worker_stats.tasks_failed)
                        + excluded.avg_task_duration_minutes
                          * (excluded.tasks_completed + excluded.tasks_failed)
                    ) * 1.0 / (
                        worker_stats.tasks_completed + worker_stats.tasks_failed
                        + excluded.tasks_completed + excluded.tasks_failed
                    )
                    ELSE 0 END,
                success_rate = CASE
                    WHEN worker_stats.tasks_completed + worker_stats.tasks_failed
                       + excluded.tasks_completed + excluded.tasks_failed > 0
                    THEN (worker_stats.tasks_completed + excluded.tasks_completed) * 1.0 /
                         (worker_stats.tasks_completed + worker_stats.tasks_failed
                          + excluded.tasks_completed + excluded.tasks_failed)
                    ELSE 0 END,
                last_heartbeat = CASE
                    WHEN excluded.last_heartbeat IS NULL THEN worker_stats.last_heartbeat
                    WHEN worker_stats.last_heartbeat IS NULL
                      OR julianday(excluded.last_heartbeat) > julianday(worker_stats.last_heartbeat)
                    THEN excluded.last_heartbeat
                    ELSE worker_stats.last_heartbeat END",
            params![
                record.worker_id,
                record.date.to_string(),
                record.tasks_completed,
                record.tasks_failed,
                record.time_spent_minutes,
                record.cost_usd,
                record.avg_task_duration_minutes,
                if record.tasks_completed + record.tasks_failed > 0 {
                    record.tasks_completed as f64
                        / (record.tasks_completed + record.tasks_failed) as f64
                } else {
                    0.0
                },
                record
                    .last_heartbeat
                    .map(crate::db::timestamps::format_utc_timestamp),
            ],
        )
        .map_err(|e| ArcanaError::Database(e))?;

        operation.with_result_count(1).succeed();
        Ok(())
    }

    /// Move worker days older than `cutoff` into idempotent monthly buckets,
    /// then prune those raw rows in the same write transaction.
    pub fn rollup_and_prune_worker_stats_before(&self, cutoff: NaiveDate) -> Result<usize> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_stats_rollup")
                .with_database("analytics");
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(ArcanaError::Database)?;
        let cutoff = cutoff.to_string();
        let rolled_rows = {
            let mut stmt = tx
                .prepare(
                    "SELECT worker_id, strftime('%Y-%m', date) AS month,
                            MIN(date), MAX(date), SUM(tasks_completed), SUM(tasks_failed),
                            SUM(time_spent_minutes), SUM(cost_usd),
                            CASE WHEN SUM(tasks_completed + tasks_failed) > 0
                                 THEN SUM(avg_task_duration_minutes
                                          * (tasks_completed + tasks_failed)) * 1.0
                                      / SUM(tasks_completed + tasks_failed)
                                 ELSE 0 END,
                            CASE WHEN SUM(tasks_completed + tasks_failed) > 0
                                 THEN SUM(tasks_completed) * 1.0
                                      / SUM(tasks_completed + tasks_failed)
                                 ELSE 0 END,
                            (SELECT history.last_heartbeat FROM worker_stats history
                              WHERE history.worker_id = source.worker_id
                                AND strftime('%Y-%m', history.date) = strftime('%Y-%m', source.date)
                                AND history.date < ?1 AND history.last_heartbeat IS NOT NULL
                              ORDER BY julianday(history.last_heartbeat) DESC LIMIT 1)
                       FROM worker_stats source
                      WHERE date < ?1
                      GROUP BY worker_id, strftime('%Y-%m', date)",
                )
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([&cutoff], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, f64>(7)?,
                        row.get::<_, f64>(8)?,
                        row.get::<_, f64>(9)?,
                        row.get::<_, Option<String>>(10)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(ArcanaError::Database)?
        };

        let mut changed = 0;
        for row in rolled_rows {
            let (
                worker_id,
                month,
                covered_start,
                covered_end,
                completed,
                failed,
                time,
                cost,
                avg_duration,
                success_rate,
                heartbeat,
            ) = row;
            let month = month.ok_or_else(|| {
                ArcanaError::Agent(format!(
                    "cannot roll up worker stats with invalid date coverage {covered_start}..{covered_end}"
                ))
            })?;
            tx.execute(
                "INSERT INTO worker_stats_monthly
                    (worker_id, month, covered_start, covered_end, tasks_completed, tasks_failed,
                     time_spent_minutes, cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(worker_id, month) DO UPDATE SET
                    covered_start = MIN(worker_stats_monthly.covered_start, excluded.covered_start),
                    covered_end = MAX(worker_stats_monthly.covered_end, excluded.covered_end),
                    tasks_completed = worker_stats_monthly.tasks_completed + excluded.tasks_completed,
                    tasks_failed = worker_stats_monthly.tasks_failed + excluded.tasks_failed,
                    time_spent_minutes = worker_stats_monthly.time_spent_minutes + excluded.time_spent_minutes,
                    cost_usd = worker_stats_monthly.cost_usd + excluded.cost_usd,
                    avg_task_duration_minutes = CASE
                        WHEN worker_stats_monthly.tasks_completed + worker_stats_monthly.tasks_failed
                           + excluded.tasks_completed + excluded.tasks_failed > 0
                        THEN (worker_stats_monthly.avg_task_duration_minutes
                                * (worker_stats_monthly.tasks_completed + worker_stats_monthly.tasks_failed)
                              + excluded.avg_task_duration_minutes
                                * (excluded.tasks_completed + excluded.tasks_failed)) * 1.0
                             / (worker_stats_monthly.tasks_completed + worker_stats_monthly.tasks_failed
                                + excluded.tasks_completed + excluded.tasks_failed)
                        ELSE 0 END,
                    success_rate = CASE
                        WHEN worker_stats_monthly.tasks_completed + worker_stats_monthly.tasks_failed
                           + excluded.tasks_completed + excluded.tasks_failed > 0
                        THEN (worker_stats_monthly.tasks_completed + excluded.tasks_completed) * 1.0
                             / (worker_stats_monthly.tasks_completed + worker_stats_monthly.tasks_failed
                                + excluded.tasks_completed + excluded.tasks_failed)
                        ELSE 0 END,
                    last_heartbeat = CASE
                        WHEN excluded.last_heartbeat IS NULL THEN worker_stats_monthly.last_heartbeat
                        WHEN worker_stats_monthly.last_heartbeat IS NULL
                          OR julianday(excluded.last_heartbeat) > julianday(worker_stats_monthly.last_heartbeat)
                        THEN excluded.last_heartbeat
                        ELSE worker_stats_monthly.last_heartbeat END",
                params![worker_id, month, covered_start, covered_end, completed, failed, time,
                    cost, avg_duration, success_rate, heartbeat],
            )
            .map_err(ArcanaError::Database)?;
            changed += 1;
        }

        tx.execute("DELETE FROM worker_stats WHERE date < ?1", [&cutoff])
            .map_err(ArcanaError::Database)?;
        tx.commit().map_err(ArcanaError::Database)?;
        operation = operation.with_result_count(changed);
        operation.succeed();
        Ok(changed)
    }

    /// Prune raw analytics detail older than `cutoff` while keeping the
    /// already-produced daily cost history intact. Invalid timestamps fail
    /// closed so cleanup cannot silently misclassify row age.
    pub fn prune_append_only_before(&self, cutoff: &str) -> Result<usize> {
        let mut operation =
            OperationTimer::start("database_analytics_retention_prune").with_database("analytics");
        let mut conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {e}")))?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(ArcanaError::Database)?;
        let invalid_rows: i64 = tx
            .query_row(APPEND_ONLY_INVALID_TIMESTAMPS_SQL, [], |row| row.get(0))
            .map_err(ArcanaError::Database)?;
        if invalid_rows > 0 {
            return Err(ArcanaError::Agent(
                "Analytics retention refused rows with invalid timestamps".into(),
            ));
        }
        let ambiguous_summary_dates = {
            let mut stmt = tx
                .prepare(
                    "SELECT DISTINCT daily_costs.date
                       FROM token_usage
                       JOIN daily_costs
                         ON daily_costs.date = date(token_usage.timestamp, 'localtime')
                       LEFT JOIN daily_cost_summary_coverage
                         ON daily_cost_summary_coverage.date = daily_costs.date
                      WHERE julianday(token_usage.timestamp) < julianday(?1)
                        AND daily_cost_summary_coverage.date IS NULL
                      ORDER BY daily_costs.date",
                )
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([cutoff], |row| row.get::<_, String>(0))
                .map_err(ArcanaError::Database)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(ArcanaError::Database)?
        };
        for date in ambiguous_summary_dates {
            if !adopt_verified_legacy_daily_cost_coverage(&tx, &date)? {
                return Err(ArcanaError::Agent(
                    "Analytics retention refused expired token rows with ambiguous daily cost coverage"
                        .into(),
                ));
            }
        }

        struct DailySummary {
            sessions: i64,
            total_cost: f64,
            total_tokens: i64,
            message_count: i64,
            tool_call_count: i64,
            provider_costs: HashMap<String, f64>,
            agent_costs: HashMap<String, f64>,
        }
        // This ledger makes distinct session counts additive across separate
        // retention runs without counting a session twice.
        tx.execute(
            "INSERT OR IGNORE INTO daily_cost_summary_sessions (date, session_id)
             SELECT date(timestamp, 'localtime'), session_id
               FROM token_usage
              WHERE julianday(timestamp) < julianday(?1)
                AND id > COALESCE((
                    SELECT covered_through_token_id
                      FROM daily_cost_summary_coverage
                     WHERE date = date(token_usage.timestamp, 'localtime')
                ), 0)",
            [cutoff],
        )
        .map_err(ArcanaError::Database)?;
        let mut daily_summaries = HashMap::<String, DailySummary>::new();
        {
            let mut stmt = tx
                .prepare(APPEND_ONLY_TOKEN_DAILY_SUMMARY_SQL)
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([cutoff], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            for row in rows {
                let (date, total_cost, total_tokens, sessions, message_count, tool_calls) =
                    row.map_err(ArcanaError::Database)?;
                let date = date.ok_or_else(|| {
                    ArcanaError::Agent(
                        "Analytics retention could not derive token usage date".into(),
                    )
                })?;
                daily_summaries.insert(
                    date,
                    DailySummary {
                        sessions,
                        total_cost,
                        total_tokens,
                        message_count,
                        tool_call_count: tool_calls,
                        provider_costs: HashMap::new(),
                        agent_costs: HashMap::new(),
                    },
                );
            }
        }
        {
            let mut stmt = tx
                .prepare(APPEND_ONLY_PROVIDER_COSTS_SQL)
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([cutoff], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            for row in rows {
                let (date, provider, cost) = row.map_err(ArcanaError::Database)?;
                let date = date.ok_or_else(|| {
                    ArcanaError::Agent(
                        "Analytics retention could not derive token usage date".into(),
                    )
                })?;
                let summary = daily_summaries.get_mut(&date).ok_or_else(|| {
                    ArcanaError::Agent("Analytics retention summary was inconsistent".into())
                })?;
                *summary.provider_costs.entry(provider).or_default() += cost;
            }
        }
        {
            let mut stmt = tx
                .prepare(APPEND_ONLY_AGENT_COSTS_SQL)
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([cutoff], |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, f64>(2)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            for row in rows {
                let (date, agent, cost) = row.map_err(ArcanaError::Database)?;
                let date = date.ok_or_else(|| {
                    ArcanaError::Agent(
                        "Analytics retention could not derive token usage date".into(),
                    )
                })?;
                let summary = daily_summaries.get_mut(&date).ok_or_else(|| {
                    ArcanaError::Agent("Analytics retention summary was inconsistent".into())
                })?;
                *summary.agent_costs.entry(agent).or_default() += cost;
            }
        }
        for (date, mut summary) in daily_summaries {
            let existing = tx
                .query_row(
                    "SELECT total_cost_usd, total_tokens, message_count, tool_call_count,
                            provider_breakdown, agent_type_breakdown
                       FROM daily_costs WHERE date = ?1",
                    [&date],
                    |row| {
                        Ok((
                            row.get::<_, f64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, Option<String>>(4)?,
                            row.get::<_, Option<String>>(5)?,
                        ))
                    },
                )
                .optional()
                .map_err(ArcanaError::Database)?;
            let had_existing_summary = existing.is_some();
            if let Some((cost, tokens, messages, tools, providers, agents)) = existing {
                summary.total_cost += cost;
                summary.total_tokens += tokens;
                summary.message_count += messages;
                summary.tool_call_count += tools;
                let providers: HashMap<String, f64> = providers
                    .as_deref()
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_default();
                let agents: HashMap<String, f64> = agents
                    .as_deref()
                    .and_then(|json| serde_json::from_str(json).ok())
                    .unwrap_or_default();
                for (provider, value) in providers {
                    *summary.provider_costs.entry(provider).or_default() += value;
                }
                for (agent, value) in agents {
                    *summary.agent_costs.entry(agent).or_default() += value;
                }
            }
            let tracked_sessions: i64 = tx
                .query_row(
                    "SELECT COUNT(*) FROM daily_cost_summary_sessions WHERE date = ?1",
                    [&date],
                    |row| row.get(0),
                )
                .map_err(ArcanaError::Database)?;
            // The ledger is the exact union represented by this summary. The
            // current batch's distinct count may overlap any earlier batch.
            summary.sessions = tracked_sessions;
            let provider_breakdown = serde_json::to_string(&summary.provider_costs)
                .map_err(|e| ArcanaError::Agent(format!("Analytics retention summary: {e}")))?;
            let agent_breakdown = serde_json::to_string(&summary.agent_costs)
                .map_err(|e| ArcanaError::Agent(format!("Analytics retention summary: {e}")))?;
            tx.execute(
                "INSERT OR REPLACE INTO daily_costs
                    (date, total_cost_usd, total_tokens, session_count, message_count,
                     tool_call_count, provider_breakdown, agent_type_breakdown)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    date,
                    summary.total_cost,
                    summary.total_tokens,
                    summary.sessions,
                    summary.message_count,
                    summary.tool_call_count,
                    provider_breakdown,
                    agent_breakdown,
                ],
            )
            .map_err(ArcanaError::Database)?;
            if !had_existing_summary {
                // The rows included in this retention-created summary are
                // deleted in this same transaction. A zero boundary leaves
                // any later expired source rows eligible, regardless of ID
                // ordering relative to their timestamps.
                tx.execute(
                    "INSERT OR IGNORE INTO daily_cost_summary_coverage
                     (date, covered_through_token_id) VALUES (?1, 0)",
                    [&date],
                )
                .map_err(ArcanaError::Database)?;
            }
        }

        let mut deleted = 0usize;
        deleted += tx
            .execute(APPEND_ONLY_PRUNE_TOKEN_USAGE_SQL, [cutoff])
            .map_err(ArcanaError::Database)?;
        deleted += tx
            .execute(APPEND_ONLY_PRUNE_SYSTEM_EVENTS_SQL, [cutoff])
            .map_err(ArcanaError::Database)?;
        deleted += tx
            .execute(APPEND_ONLY_PRUNE_API_FAILURES_SQL, [cutoff])
            .map_err(ArcanaError::Database)?;

        // Seal only dates with known summary coverage, after every raw token
        // row for that local date has been removed in this same transaction.
        // Unmarked ambiguous legacy summaries remain preserved and unsealed.
        let sealable_dates: Vec<String> = {
            let mut stmt = tx
                .prepare(
                    "SELECT c.date
                       FROM daily_cost_summary_coverage c
                       JOIN daily_costs d ON d.date = c.date
                      WHERE c.date < date(?1, 'localtime')
                        AND NOT EXISTS (
                            SELECT 1 FROM token_usage t
                             WHERE date(t.timestamp, 'localtime') = c.date
                        )",
                )
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map([cutoff], |row| row.get(0))
                .map_err(ArcanaError::Database)?;
            rows.collect::<std::result::Result<Vec<String>, _>>()
                .map_err(ArcanaError::Database)?
        };
        for date in sealable_dates {
            tx.execute(
                "INSERT OR IGNORE INTO daily_cost_summary_sealed (date, sealed_at)
                 VALUES (?1, ?2)",
                params![date, cutoff],
            )
            .map_err(ArcanaError::Database)?;
            tx.execute(
                "DELETE FROM daily_cost_summary_sessions WHERE date = ?1",
                [&date],
            )
            .map_err(ArcanaError::Database)?;
            tx.execute(
                "DELETE FROM daily_cost_summary_coverage WHERE date = ?1",
                [&date],
            )
            .map_err(ArcanaError::Database)?;
        }
        tx.commit().map_err(ArcanaError::Database)?;
        operation = operation.with_result_count(deleted);
        operation.succeed();
        Ok(deleted)
    }

    /// Aggregate worker stats for a single worker over a date range.
    /// Returns summed tasks_completed, tasks_failed, time_spent_minutes, cost_usd
    /// and a calculated success_rate. Returns None if no rows exist for the range.
    pub fn get_worker_stats_range(
        &self,
        worker_id: &str,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Option<WorkerStatsRange>> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_worker_stats_range_read")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let start_str = start.to_string();
        let end_str = end.to_string();
        let mut tasks_completed = 0i64;
        let mut tasks_failed = 0i64;
        let mut time_spent_minutes = 0i64;
        let mut cost_usd = 0.0;
        let mut duration_weighted_sum = 0.0;
        let mut duration_weight = 0i64;
        let mut last_heartbeat: Option<chrono::DateTime<Utc>> = None;
        let mut coverage_start: Option<NaiveDate> = None;
        let mut coverage_end: Option<NaiveDate> = None;
        let mut has_rows = false;
        let mut has_monthly = false;

        {
            let mut stmt = conn
                .prepare(
                    "SELECT date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd,
                            avg_task_duration_minutes, last_heartbeat
                       FROM worker_stats
                      WHERE worker_id = ?1 AND date BETWEEN ?2 AND ?3",
                )
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map(params![worker_id, start_str, end_str], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, f64>(4)?,
                        row.get::<_, f64>(5)?,
                        parse_optional_timestamp(row, 6)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            for row in rows {
                let (date, completed, failed, time, cost, avg_duration, heartbeat) =
                    row.map_err(ArcanaError::Database)?;
                let date = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .map_err(|e| ArcanaError::Agent(format!("invalid worker stats date: {e}")))?;
                has_rows = true;
                tasks_completed += completed;
                tasks_failed += failed;
                time_spent_minutes += time;
                cost_usd += cost;
                let weight = completed + failed;
                duration_weighted_sum += avg_duration * weight as f64;
                duration_weight += weight;
                coverage_start = Some(coverage_start.map_or(date, |current| current.min(date)));
                coverage_end = Some(coverage_end.map_or(date, |current| current.max(date)));
                if heartbeat.is_some_and(|heartbeat| {
                    last_heartbeat
                        .as_ref()
                        .is_none_or(|current| heartbeat > *current)
                }) {
                    last_heartbeat = heartbeat;
                }
            }
        }

        {
            let mut stmt = conn
                .prepare(
                    "SELECT covered_start, covered_end, tasks_completed, tasks_failed,
                            time_spent_minutes, cost_usd, avg_task_duration_minutes, last_heartbeat
                       FROM worker_stats_monthly
                      WHERE worker_id = ?1 AND covered_end >= ?2 AND covered_start <= ?3",
                )
                .map_err(ArcanaError::Database)?;
            let rows = stmt
                .query_map(params![worker_id, start_str, end_str], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, f64>(5)?,
                        row.get::<_, f64>(6)?,
                        parse_optional_timestamp(row, 7)?,
                    ))
                })
                .map_err(ArcanaError::Database)?;
            for row in rows {
                let (
                    covered_start,
                    covered_end,
                    completed,
                    failed,
                    time,
                    cost,
                    avg_duration,
                    heartbeat,
                ) = row.map_err(ArcanaError::Database)?;
                let covered_start =
                    NaiveDate::parse_from_str(&covered_start, "%Y-%m-%d").map_err(|e| {
                        ArcanaError::Agent(format!("invalid worker stats coverage start: {e}"))
                    })?;
                let covered_end =
                    NaiveDate::parse_from_str(&covered_end, "%Y-%m-%d").map_err(|e| {
                        ArcanaError::Agent(format!("invalid worker stats coverage end: {e}"))
                    })?;
                has_rows = true;
                has_monthly = true;
                tasks_completed += completed;
                tasks_failed += failed;
                time_spent_minutes += time;
                cost_usd += cost;
                let weight = completed + failed;
                duration_weighted_sum += avg_duration * weight as f64;
                duration_weight += weight;
                coverage_start = Some(
                    coverage_start.map_or(covered_start, |current| current.min(covered_start)),
                );
                coverage_end =
                    Some(coverage_end.map_or(covered_end, |current| current.max(covered_end)));
                if heartbeat.is_some_and(|heartbeat| {
                    last_heartbeat
                        .as_ref()
                        .is_none_or(|current| heartbeat > *current)
                }) {
                    last_heartbeat = heartbeat;
                }
            }
        }

        if !has_rows {
            operation = operation.with_result_count(0);
            operation.succeed();
            return Ok(None);
        }
        let total = tasks_completed + tasks_failed;
        let stats = WorkerStatsRange {
            worker_id: worker_id.to_string(),
            date: end,
            tasks_completed,
            tasks_failed,
            time_spent_minutes,
            cost_usd,
            avg_task_duration_minutes: if duration_weight > 0 {
                duration_weighted_sum / duration_weight as f64
            } else {
                0.0
            },
            success_rate: if total > 0 {
                tasks_completed as f64 / total as f64
            } else {
                0.0
            },
            last_heartbeat,
            coverage_start: coverage_start.expect("rows establish a coverage start"),
            coverage_end: coverage_end.expect("rows establish a coverage end"),
            granularity: if has_monthly { "month" } else { "day" },
        };
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(Some(stats))
    }

    pub fn get_worker_stats(
        &self,
        worker_id: &str,
        date: NaiveDate,
    ) -> Result<Option<WorkerStatsRecord>> {
        let operation =
            OperationTimer::start("database_worker_stats_point_read").with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn.prepare(
            "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd, 
             avg_task_duration_minutes, success_rate, last_heartbeat
             FROM worker_stats WHERE worker_id = ? AND date = ?"
        ).map_err(|e| ArcanaError::Database(e))?;

        let date_str = date.to_string();
        let record = stmt
            .query_row(params![worker_id, date_str], |row| {
                let date_str: String = row.get(1)?;
                let last_heartbeat = parse_optional_timestamp(row, 8)?;
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
            .map_err(|e| ArcanaError::Database(e))?;

        if record.is_some() {
            operation.with_result_count(1).succeed();
        } else {
            operation.with_result_count(0).set_outcome("not_found");
        }
        Ok(record)
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // API Failures Operations
    // ═══════════════════════════════════════════════════════════════════════════

    pub fn insert_api_failure(&self, record: &ApiFailureRecord) -> Result<i64> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_api_failure_insert")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO api_failures (timestamp, provider, error_type, error_message, request_id, retry_count, resolved)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                crate::db::timestamps::format_utc_timestamp(record.timestamp),
                record.provider,
                record.error_type,
                record.error_message,
                record.request_id,
                record.retry_count,
                record.resolved,
            ],
        ).map_err(|e| ArcanaError::Database(e))?;

        let id = conn.last_insert_rowid();
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(id)
    }

    pub fn get_api_failures_recent(&self, hours: i32) -> Result<Vec<ApiFailureRecord>> {
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_api_failures_recent_read",
        )
        .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        let mut stmt = conn
            .prepare(API_FAILURES_RECENT_SQL)
            .map_err(|e| ArcanaError::Database(e))?;

        let records = stmt
            .query_map(params![format!("-{}", hours)], |row| {
                let timestamp = crate::db::timestamps::parse_row_timestamp(row, 1)?;
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
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;

        operation = operation.with_result_count(records.len());
        operation.succeed();
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
        let operation =
            OperationTimer::start("database_today_activity_read").with_database("analytics");
        let today_str = chrono::Local::now().naive_local().date().to_string();
        let Some(bounds) = local_date_range_bounds(&today_str, &today_str) else {
            operation.with_result_count(0).set_outcome("fallback");
            return (0, 0, 0);
        };
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => {
                operation.with_result_count(0).set_outcome("fallback");
                return (0, 0, 0);
            }
        };

        let mut degraded = false;
        let sessions: i32 = match conn.query_row(
            TOKEN_ACTIVITY_SESSIONS_SQL,
            params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
            |r| r.get(0),
        ) {
            Ok(value) => value,
            Err(_) => {
                degraded = true;
                0
            }
        };

        let messages: i32 = match conn.query_row(
            TOKEN_ACTIVITY_MESSAGES_SQL,
            params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
            |r| r.get(0),
        ) {
            Ok(value) => value,
            Err(_) => {
                degraded = true;
                0
            }
        };

        let tool_calls: i32 = match conn.query_row(
            TOKEN_ACTIVITY_TOOL_CALLS_SQL,
            params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
            |r| r.get(0),
        ) {
            Ok(value) => value,
            Err(_) => {
                degraded = true;
                0
            }
        };

        if degraded {
            operation.with_result_count(3).set_outcome("fallback");
        } else {
            operation.with_result_count(3).succeed();
        }
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
        let operation =
            OperationTimer::start("database_token_usage_range_read").with_database("analytics");
        let s = start.to_string();
        let e = end.to_string();
        let totals = self.token_totals(&s, &e);
        let by_model = self.token_by_group("model", &s, &e)?;
        let by_provider = self.token_by_group("provider", &s, &e)?;
        let by_agent = self.token_by_group("agent_type", &s, &e)?;
        let top_sessions = self.token_top_sessions(&s, &e)?;
        let (total, prompt, completion, thinking) = totals;
        operation.with_result_count(top_sessions.len()).succeed();
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

    /// Read provider usage attributed to one durable fleet session.
    pub fn get_token_usage_for_session(
        &self,
        session_id: &str,
    ) -> Result<SessionTokenUsageSummary> {
        let mut operation =
            OperationTimer::start("database_token_usage_session_read").with_database("analytics");
        let result = (|| {
            let conn = self
                .conn
                .lock()
                .map_err(|error| ArcanaError::Agent(format!("Mutex lock failed: {error}")))?;
            conn.query_row(TOKEN_USAGE_FOR_SESSION_SQL, [session_id], |row| {
                Ok(SessionTokenUsageSummary {
                    request_count: row.get(0)?,
                    prompt_tokens: row.get(1)?,
                    completion_tokens: row.get(2)?,
                    total_tokens: row.get(3)?,
                    thinking_tokens: row.get(4)?,
                    cost_usd: row.get(5)?,
                })
            })
            .map_err(ArcanaError::Database)
        })();
        match result {
            Ok(summary) => {
                operation
                    .with_result_count(summary.request_count as usize)
                    .succeed();
                Ok(summary)
            }
            Err(error) => {
                operation.set_outcome("error");
                Err(error)
            }
        }
    }

    // ── private helpers — one lock/stmt per function (mirrors existing methods) ──

    fn token_totals(&self, start: &str, end: &str) -> (i64, i64, i64, Option<i64>) {
        let operation =
            OperationTimer::start("database_token_usage_totals_read").with_database("analytics");
        let Some(bounds) = local_date_range_bounds(start, end) else {
            operation.with_result_count(0).set_outcome("fallback");
            return (0, 0, 0, None);
        };
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => {
                operation.with_result_count(0).set_outcome("fallback");
                return (0, 0, 0, None);
            }
        };
        let totals = conn.query_row(
            TOKEN_TOTALS_RANGE_SQL,
            params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        );
        match totals {
            Ok(totals) => {
                operation.with_result_count(1).succeed();
                totals
            }
            Err(_) => {
                operation.with_result_count(0).set_outcome("fallback");
                (0, 0, 0, None)
            }
        }
    }

    fn token_by_group(&self, col: &str, start: &str, end: &str) -> Result<HashMap<String, i64>> {
        let operation_name = match col {
            "model" => "database_token_usage_by_model_read",
            "provider" => "database_token_usage_by_provider_read",
            "agent_type" => "database_token_usage_by_agent_read",
            _ => "database_token_usage_by_unsupported_group_read",
        };
        let operation = OperationTimer::start(operation_name).with_database("analytics");
        // Use fixed SQL for the three internal grouping dimensions so the
        // query-plan registry can prove each production form directly.
        if !matches!(col, "model" | "provider" | "agent_type") {
            return Err(ArcanaError::Agent(
                "Unsupported token usage grouping dimension".into(),
            ));
        }
        let sql = if col == "model" {
            TOKEN_USAGE_BY_MODEL_SQL
        } else if col == "provider" {
            TOKEN_USAGE_BY_PROVIDER_SQL
        } else if col == "agent_type" {
            TOKEN_USAGE_BY_AGENT_TYPE_SQL
        } else {
            // The allowlist check above makes this branch unreachable; keep
            // the SQL expression statically resolvable for source auditing.
            TOKEN_USAGE_BY_AGENT_TYPE_SQL
        };
        let bounds = local_date_range_bounds(start, end).ok_or_else(|| {
            ArcanaError::Agent("Invalid local date range for token aggregation".into())
        })?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {e}")))?;
        let mut stmt = conn.prepare(&sql).map_err(|e| ArcanaError::Database(e))?;
        let rows = stmt
            .query_map(
                params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)),
            )
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;
        operation.with_result_count(rows.len()).succeed();
        Ok(rows.into_iter().collect())
    }

    fn token_top_sessions(&self, start: &str, end: &str) -> Result<Vec<SessionTokenUsage>> {
        let operation = OperationTimer::start("database_token_usage_top_sessions_read")
            .with_database("analytics");
        let bounds = local_date_range_bounds(start, end).ok_or_else(|| {
            ArcanaError::Agent("Invalid local date range for token aggregation".into())
        })?;
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {e}")))?;
        let mut stmt = conn
            .prepare(TOKEN_TOP_SESSIONS_SQL)
            .map_err(|e| ArcanaError::Database(e))?;
        // Collect into an owned Vec before stmt/conn drop — same pattern as
        // get_daily_costs_range and other methods in this file.
        let rows = stmt
            .query_map(
                params![&bounds[0], &bounds[1], &bounds[2], &bounds[3]],
                |r| {
                    Ok(SessionTokenUsage {
                        session_id: r.get(0)?,
                        tokens: r.get(1)?,
                        cost_usd: r.get(2)?,
                        agent_type: r
                            .get::<_, Option<String>>(3)?
                            .unwrap_or_else(|| "unknown".into()),
                    })
                },
            )
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;
        operation.with_result_count(rows.len()).succeed();
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
        let mut operation = crate::observability::timing::OperationTimer::start(
            "database_system_events_recent_read",
        )
        .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;

        // Build the cutoff timestamp string. `hours` is clamped to 1–168 by the
        // command layer so this format is safe to embed in SQL.
        let cutoff = format!("-{hours} hours");

        let sql = match (component.is_some(), severity.is_some()) {
            (false, false) => SYSTEM_EVENTS_RECENT_SQL,
            (true, false) => SYSTEM_EVENTS_RECENT_BY_COMPONENT_SQL,
            (false, true) => SYSTEM_EVENTS_RECENT_BY_SEVERITY_SQL,
            (true, true) => SYSTEM_EVENTS_RECENT_BY_COMPONENT_AND_SEVERITY_SQL,
        };
        let mut values = vec![rusqlite::types::Value::Text(cutoff)];
        if let Some(component) = component {
            values.push(rusqlite::types::Value::Text(component.to_owned()));
        }
        if let Some(severity) = severity {
            values.push(rusqlite::types::Value::Text(severity.to_owned()));
        }

        let mut stmt = conn.prepare(&sql).map_err(|e| ArcanaError::Database(e))?;

        let records = stmt
            .query_map(rusqlite::params_from_iter(values.iter()), |row| {
                let timestamp = crate::db::timestamps::parse_row_timestamp(row, 1)?;
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
            .map_err(|e| ArcanaError::Database(e))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| ArcanaError::Database(e))?;

        operation = operation.with_result_count(records.len());
        operation.succeed();
        Ok(records)
    }

    pub fn insert_system_event(&self, record: &SystemEventRecord) -> Result<i64> {
        let mut operation =
            crate::observability::timing::OperationTimer::start("database_system_event_insert")
                .with_database("analytics");
        let conn = self
            .conn
            .lock()
            .map_err(|e| ArcanaError::Agent(format!("Mutex lock failed: {}", e)))?;
        conn.execute(
            "INSERT INTO system_events (timestamp, component, event_type, message, severity, metadata)
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                crate::db::timestamps::format_utc_timestamp(record.timestamp),
                record.component,
                record.event_type,
                record.message,
                record.severity,
                record.metadata,
            ],
        ).map_err(|e| ArcanaError::Database(e))?;

        let id = conn.last_insert_rowid();
        operation = operation.with_result_count(1);
        operation.succeed();
        Ok(id)
    }
}

fn parse_optional_timestamp(
    row: &rusqlite::Row<'_>,
    column: usize,
) -> rusqlite::Result<Option<chrono::DateTime<Utc>>> {
    use rusqlite::types::Type;

    let raw: Option<String> = row.get(column)?;
    raw.map(|raw| {
        crate::db::timestamps::parse_db_timestamp(&raw).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error.to_string(),
                )),
            )
        })
    })
    .transpose()
}

#[cfg(test)]
mod append_only_retention_tests {
    use super::*;

    #[tokio::test]
    async fn populated_pre_retention_analytics_upgrade_preserves_all_existing_rows() {
        let directory = tempfile::tempdir().expect("owned legacy profile");
        let path = directory.path().join("analytics.db");
        let conn = Connection::open(&path).expect("legacy analytics file");
        migrate::run_ladder(&conn, "analytics.db", &AnalyticsDb::LADDER[..1])
            .expect("production baseline migration");
        conn.execute_batch(
            "INSERT INTO token_usage (timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd,request_type)
             VALUES ('2026-07-06 12:00:00','legacy-session','worker','google','gemini',2,3,5,0.01,'chat');
             INSERT INTO daily_costs (date,total_cost_usd,total_tokens,session_count,message_count,tool_call_count,provider_breakdown,agent_type_breakdown,created_at)
             VALUES ('2026-07-06',0.01,5,1,1,0,'{\"google\":0.01}','{\"worker\":0.01}','2026-07-07 00:00:00');
             INSERT INTO worker_stats (worker_id,date,tasks_completed,tasks_failed,time_spent_minutes,cost_usd,last_heartbeat)
             VALUES ('worker-backend','2026-07-06',2,1,7,0.25,'2026-07-06 12:00:00');
             INSERT INTO api_failures (timestamp,provider,error_type,error_message,retry_count)
             VALUES ('2026-07-06 12:00:00','google','fixture','legacy failure',2);
             INSERT INTO system_events (timestamp,component,event_type,message,severity,metadata)
             VALUES ('2026-07-06 12:00:00','fixture','event','legacy event','info','{}');"
        ).expect("populate every pre-retention analytics table");
        fn snapshot(conn: &Connection, sql: &str) -> Vec<Vec<rusqlite::types::Value>> {
            let mut statement = conn.prepare(sql).expect("snapshot statement");
            let columns = statement.column_count();
            statement
                .query_map([], |row| {
                    (0..columns).map(|column| row.get(column)).collect()
                })
                .expect("snapshot query")
                .collect::<rusqlite::Result<_>>()
                .expect("snapshot rows")
        }
        let queries = [
            "SELECT * FROM token_usage ORDER BY id",
            "SELECT * FROM daily_costs ORDER BY date",
            "SELECT id,worker_id,date,tasks_completed,tasks_failed,time_spent_minutes,cost_usd,avg_task_duration_minutes,success_rate,last_heartbeat FROM worker_stats ORDER BY id",
            "SELECT * FROM api_failures ORDER BY id",
            "SELECT * FROM system_events ORDER BY id",
        ];
        let before = queries
            .iter()
            .map(|query| snapshot(&conn, query))
            .collect::<Vec<_>>();
        drop(conn);
        let db = AnalyticsDb::new_at(&path)
            .await
            .expect("production latest-schema opener");
        let conn = db.conn.lock().expect("upgraded analytics connection");
        assert_eq!(
            migrate::user_version(&conn).unwrap(),
            AnalyticsDb::LADDER.last().unwrap().version
        );
        for (query, before) in queries.iter().zip(before) {
            assert_eq!(snapshot(&conn, query), before, "upgrade preserves {query}");
        }
        drop(conn);
        let date = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();
        assert_eq!(
            db.get_daily_costs_range(date, date).unwrap()[0].total_tokens,
            5
        );
        let usage_date = crate::db::timestamps::parse_db_timestamp("2026-07-06 12:00:00")
            .unwrap()
            .with_timezone(&Local)
            .date_naive();
        assert_eq!(
            db.get_token_usage_by_date(usage_date).unwrap()[0].session_id,
            "legacy-session"
        );
    }

    async fn database() -> (tempfile::TempDir, AnalyticsDb) {
        let directory = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&directory.path().join("analytics.db"))
            .await
            .expect("open production analytics database");
        (directory, db)
    }

    fn insert_rows(db: &AnalyticsDb, table: &str, timestamp: &str) {
        let conn = db.conn.lock().expect("analytics connection");
        match table {
            "token_usage" => {
                conn.execute(
                    "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                         prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                     VALUES (?1, 'session', 'worker', 'google', 'gemini', 2, 3, 5, 0.01, 'chat')",
                    [timestamp],
                )
                .expect("insert token usage");
            }
            "system_events" => {
                conn.execute(
                    "INSERT INTO system_events (timestamp, component, event_type, message, severity)
                     VALUES (?1, 'test', 'event', 'detail', 'info')",
                    [timestamp],
                )
                .expect("insert system event");
            }
            "api_failures" => {
                conn.execute(
                    "INSERT INTO api_failures (timestamp, provider, error_type, error_message)
                     VALUES (?1, 'google', 'test', 'detail')",
                    [timestamp],
                )
                .expect("insert API failure");
            }
            _ => panic!("unhandled test table"),
        }
    }

    fn insert_retention_cost_row(
        db: &AnalyticsDb,
        timestamp: &str,
        provider: &str,
        agent_type: &str,
    ) {
        insert_retention_cost_row_for_session(
            db,
            timestamp,
            "retention-session",
            provider,
            agent_type,
        );
    }

    fn insert_retention_cost_row_for_session(
        db: &AnalyticsDb,
        timestamp: &str,
        session_id: &str,
        provider: &str,
        agent_type: &str,
    ) {
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                     prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                 VALUES (?1, ?2, ?3, ?4, 'test-model', 2, 3, 5, 0.01, 'chat')",
                params![timestamp, session_id, agent_type, provider],
            )
            .expect("insert retention cost row");
    }

    #[tokio::test]
    async fn append_only_retention_readers_use_populated_timestamp_indexes() {
        let (_directory, db) = database().await;
        {
            let conn = db.conn.lock().expect("analytics connection");
            conn.execute_batch(
                "WITH RECURSIVE n(value) AS (
                    SELECT 1 UNION ALL SELECT value + 1 FROM n WHERE value < 5000
                 )
                 INSERT INTO token_usage
                    (timestamp,session_id,agent_type,provider,model,prompt_tokens,
                     completion_tokens,total_tokens,cost_usd,request_type)
                 SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || value || ' days'),
                        'session-' || (value % 8), 'worker-' || (value % 4),
                        'provider-' || (value % 6), 'gemini-test', 2, 3, 5, 0.01, 'chat'
                 FROM n;
                 WITH RECURSIVE n(value) AS (
                    SELECT 1 UNION ALL SELECT value + 1 FROM n WHERE value < 5000
                 )
                 INSERT INTO api_failures
                    (timestamp,provider,error_type,error_message)
                 SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || value || ' days'),
                        'google','retryable','fixture' FROM n;
                 WITH RECURSIVE n(value) AS (
                    SELECT 1 UNION ALL SELECT value + 1 FROM n WHERE value < 5000
                 )
                 INSERT INTO system_events
                    (timestamp,component,event_type,message,severity)
                 SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now','-' || value || ' days'),
                        'worker','completed','fixture','info' FROM n;
                 ANALYZE;",
            )
            .expect("populate retention query fixture");
        }

        let conn = db.conn.lock().expect("analytics connection");
        for (table, sql) in [
            ("token_usage", APPEND_ONLY_INVALID_TIMESTAMPS_SQL),
            ("token_usage", APPEND_ONLY_TOKEN_DAILY_SUMMARY_SQL),
            ("token_usage", APPEND_ONLY_PROVIDER_COSTS_SQL),
            ("token_usage", APPEND_ONLY_AGENT_COSTS_SQL),
        ] {
            let explain = format!("EXPLAIN QUERY PLAN {sql}");
            let mut stmt = conn.prepare(&explain).expect("prepare production plan");
            let details = if sql.contains("?1") {
                stmt.query_map(["2026-01-01T00:00:00Z"], |row| row.get::<_, String>(3))
                    .expect("explain production query")
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .expect("collect production query plan")
                    .join(" | ")
            } else {
                stmt.query_map([], |row| row.get::<_, String>(3))
                    .expect("explain production query")
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .expect("collect production query plan")
                    .join(" | ")
            };
            assert!(
                details.contains(table) && details.contains("USING"),
                "expected indexed retention plan for {table}; plan was: {details}"
            );
            if sql == APPEND_ONLY_INVALID_TIMESTAMPS_SQL {
                for index in [
                    "idx_token_usage_julianday",
                    "idx_system_events_julianday",
                    "idx_api_failures_julianday",
                ] {
                    assert!(
                        details.contains(index),
                        "expected invalid-timestamp retention check to use {index}; plan was: {details}"
                    );
                }
            } else {
                assert!(
                    details.contains("idx_token_usage_julianday"),
                    "expected token retention reader to use its timestamp index; plan was: {details}"
                );
            }
        }
    }

    #[tokio::test]
    async fn advancing_retention_cutoffs_preserve_a_days_complete_cost_history() {
        assert_advancing_retention_preserves_history(false).await;
    }

    #[tokio::test]
    async fn advancing_retention_does_not_replace_a_preexisting_full_day_summary() {
        assert_advancing_retention_preserves_history(true).await;
    }

    async fn assert_advancing_retention_preserves_history(preexisting_summary: bool) {
        let (_directory, db) = database().await;
        let first = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .expect("local noon")
            .and_local_timezone(chrono::Local)
            .single()
            .expect("unambiguous local noon")
            .with_timezone(&Utc);
        let second = first + chrono::Duration::hours(1);
        insert_retention_cost_row(&db, &first.to_rfc3339(), "google", "worker");
        insert_retention_cost_row(&db, &second.to_rfc3339(), "anthropic", "planner");
        let date = first.with_timezone(&chrono::Local).date_naive();
        if preexisting_summary {
            db.insert_daily_cost(&DailyCostRecord {
                date,
                total_cost_usd: 0.02,
                total_tokens: 10,
                session_count: 1,
                message_count: 2,
                tool_call_count: 0,
                provider_breakdown: HashMap::from([
                    ("google".into(), 0.01),
                    ("anthropic".into(), 0.01),
                ]),
                agent_type_breakdown: HashMap::from([
                    ("worker".into(), 0.01),
                    ("planner".into(), 0.01),
                ]),
            })
            .expect("production full-day summary writer");
        }
        assert_eq!(
            db.prune_append_only_before(&(first + chrono::Duration::minutes(30)).to_rfc3339())
                .unwrap(),
            1
        );
        assert_eq!(
            db.prune_append_only_before(&(second + chrono::Duration::minutes(30)).to_rfc3339())
                .unwrap(),
            1
        );
        let summaries = db
            .get_daily_costs_range(date, date)
            .expect("production history reader");
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0].total_tokens, 10,
            "both removed batches remain represented"
        );
        assert!((summaries[0].total_cost_usd - 0.02).abs() < 1e-12);
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].tool_call_count, 0);
        assert_eq!(summaries[0].provider_breakdown["google"], 0.01);
        assert_eq!(summaries[0].provider_breakdown["anthropic"], 0.01);
        assert_eq!(summaries[0].agent_type_breakdown["worker"], 0.01);
        assert_eq!(summaries[0].agent_type_breakdown["planner"], 0.01);
        assert_eq!(
            summaries[0].session_count, 1,
            "same session is not counted twice across pruning batches"
        );
        assert_eq!(
            db.prune_append_only_before(&(second + chrono::Duration::minutes(30)).to_rfc3339())
                .unwrap(),
            0
        );
        let after_repeat = db.get_daily_costs_range(date, date).unwrap();
        assert_eq!(after_repeat[0].total_tokens, 10);
    }

    #[tokio::test]
    async fn daily_summary_coverage_leaves_later_rows_for_retention_reconciliation() {
        let (_directory, db) = database().await;
        let first = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .expect("local noon")
            .and_local_timezone(chrono::Local)
            .single()
            .expect("unambiguous local noon")
            .with_timezone(&Utc);
        let second = first + chrono::Duration::hours(1);
        let date = first.with_timezone(&chrono::Local).date_naive();
        insert_retention_cost_row(&db, &first.to_rfc3339(), "google", "worker");
        assert_eq!(
            db.rollup_daily_cost_for_date(date)
                .expect("atomically roll up the first source snapshot"),
            1
        );
        insert_retention_cost_row(&db, &second.to_rfc3339(), "anthropic", "planner");

        assert_eq!(
            db.prune_append_only_before(&(second + chrono::Duration::minutes(30)).to_rfc3339())
                .expect("reconcile source rows inserted after the daily rollup"),
            2
        );
        let summary = db
            .get_daily_costs_range(date, date)
            .expect("read reconciled daily costs")
            .pop()
            .expect("daily cost summary remains available");
        assert_eq!(summary.total_tokens, 10);
        assert_eq!(summary.message_count, 2);
        assert_eq!(summary.session_count, 1);
        assert_eq!(summary.provider_breakdown["google"], 0.01);
        assert_eq!(summary.provider_breakdown["anthropic"], 0.01);
        assert_eq!(summary.agent_type_breakdown["worker"], 0.01);
        assert_eq!(summary.agent_type_breakdown["planner"], 0.01);
    }

    #[tokio::test]
    async fn retention_session_count_uses_the_union_of_prior_and_batch_sessions() {
        let (_directory, db) = database().await;
        let first = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .expect("local noon")
            .and_local_timezone(chrono::Local)
            .single()
            .expect("unambiguous local noon")
            .with_timezone(&Utc);
        let date = first.with_timezone(&chrono::Local).date_naive();
        let prior_b = first + chrono::Duration::minutes(5);
        let batch_a = first + chrono::Duration::hours(1);
        let batch_b = first + chrono::Duration::hours(1) + chrono::Duration::minutes(5);
        let batch_c = first + chrono::Duration::hours(1) + chrono::Duration::minutes(10);
        insert_retention_cost_row_for_session(
            &db,
            &first.to_rfc3339(),
            "session-a",
            "google",
            "worker",
        );
        insert_retention_cost_row_for_session(
            &db,
            &prior_b.to_rfc3339(),
            "session-b",
            "google",
            "worker",
        );
        assert_eq!(
            db.rollup_daily_cost_for_date(date)
                .expect("roll up the initial two-session snapshot"),
            2
        );
        for (timestamp, session) in [
            (&batch_a, "session-a"),
            (&batch_b, "session-b"),
            (&batch_c, "session-c"),
        ] {
            insert_retention_cost_row_for_session(
                &db,
                &timestamp.to_rfc3339(),
                session,
                "anthropic",
                "planner",
            );
        }

        assert_eq!(
            db.prune_append_only_before(&(batch_c + chrono::Duration::minutes(30)).to_rfc3339())
                .expect("reconcile the later three-message batch"),
            5
        );
        let summary = db
            .get_daily_cost(date)
            .expect("read retained daily summary")
            .expect("daily summary remains available");
        assert_eq!(summary.message_count, 5);
        assert_eq!(summary.session_count, 3);
    }

    #[tokio::test]
    async fn retention_seals_expired_dates_and_expires_session_ids() {
        let (_directory, db) = database().await;
        let date = Local::now().date_naive() - chrono::Duration::days(1);
        let timestamp = local_boundary_utc(date)
            .expect("local date boundary")
            .checked_add_signed(chrono::Duration::hours(12))
            .expect("local noon")
            .to_rfc3339();
        let cutoff = local_boundary_utc(date.succ_opt().expect("next date"))
            .expect("next local date boundary")
            .to_rfc3339();
        insert_retention_cost_row_for_session(
            &db,
            &timestamp,
            "expired-session",
            "google",
            "worker",
        );
        assert_eq!(
            db.rollup_daily_cost_for_date(date)
                .expect("roll up the date before sealing"),
            1
        );

        assert_eq!(
            db.prune_append_only_before(&cutoff)
                .expect("prune and seal the fully expired date"),
            1
        );
        let summary = db
            .get_daily_cost(date)
            .expect("read retained summary")
            .expect("daily summary remains after sealing");
        assert_eq!(summary.total_tokens, 5);
        assert_eq!(summary.session_count, 1);
        {
            let conn = db.conn.lock().expect("analytics connection");
            let raw_rows: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM token_usage WHERE date(timestamp, 'localtime') = ?1",
                    [date.to_string()],
                    |row| row.get(0),
                )
                .expect("count remaining source rows");
            let session_ids: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM daily_cost_summary_sessions WHERE date = ?1",
                    [date.to_string()],
                    |row| row.get(0),
                )
                .expect("count retained session identifiers");
            let coverage: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM daily_cost_summary_coverage WHERE date = ?1",
                    [date.to_string()],
                    |row| row.get(0),
                )
                .expect("count retained source watermarks");
            let sealed: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM daily_cost_summary_sealed WHERE date = ?1",
                    [date.to_string()],
                    |row| row.get(0),
                )
                .expect("check persisted sealed marker");
            assert_eq!(raw_rows, 0);
            assert_eq!(session_ids, 0);
            assert_eq!(coverage, 0);
            assert_eq!(sealed, 1);

            let direct_insert = conn.execute(
                "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                     prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                 VALUES (?1, 'late-session', 'worker', 'google', 'test-model', 1, 1, 2,
                         0.01, 'chat')",
                [&timestamp],
            );
            assert!(direct_insert.is_err(), "database trigger rejects late rows");

            let current_timestamp = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                     prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                 VALUES (?1, 'movable-session', 'worker', 'google', 'test-model', 1, 1, 2,
                         0.01, 'chat')",
                [&current_timestamp],
            )
            .expect("insert a row on an unsealed date");
            let movable_id = conn.last_insert_rowid();
            let direct_update = conn.execute(
                "UPDATE token_usage SET timestamp = ?1 WHERE id = ?2",
                rusqlite::params![&timestamp, movable_id],
            );
            assert!(
                direct_update.is_err(),
                "database trigger rejects moving a row into a sealed date"
            );
        }
        let rollup_error = db
            .rollup_daily_cost_for_date(date)
            .expect_err("rollup API refuses sealed dates");
        assert!(rollup_error.to_string().contains("sealed date"));
    }

    #[tokio::test]
    async fn stale_daily_summary_refuses_to_claim_uncovered_source_rows() {
        let (_directory, db) = database().await;
        let first = chrono::Local::now()
            .date_naive()
            .and_hms_opt(12, 0, 0)
            .expect("local noon")
            .and_local_timezone(chrono::Local)
            .single()
            .expect("unambiguous local noon")
            .with_timezone(&Utc);
        let second = first + chrono::Duration::hours(1);
        let date = first.with_timezone(&chrono::Local).date_naive();
        insert_retention_cost_row(&db, &first.to_rfc3339(), "google", "worker");
        insert_retention_cost_row(&db, &second.to_rfc3339(), "anthropic", "planner");
        db.insert_daily_cost(&DailyCostRecord {
            date,
            total_cost_usd: 0.01,
            total_tokens: 5,
            session_count: 1,
            message_count: 1,
            tool_call_count: 0,
            provider_breakdown: HashMap::from([("google".into(), 0.01)]),
            agent_type_breakdown: HashMap::from([("worker".into(), 0.01)]),
        })
        .expect("persist stale caller-provided summary without claiming coverage");

        assert!(db
            .prune_append_only_before(&(second + chrono::Duration::minutes(30)).to_rfc3339())
            .is_err());
        let remaining: i64 = db
            .conn
            .lock()
            .expect("analytics connection")
            .query_row("SELECT COUNT(*) FROM token_usage", [], |row| row.get(0))
            .expect("read rows retained after fail-closed refusal");
        assert_eq!(remaining, 2);
    }

    #[tokio::test]
    async fn partial_legacy_daily_summary_refuses_to_guess_coverage() {
        let directory = tempfile::tempdir().expect("owned legacy analytics profile");
        let path = directory.path().join("analytics.db");
        let conn = Connection::open(&path).expect("legacy analytics file");
        migrate::run_ladder(&conn, "analytics.db", &AnalyticsDb::LADDER[..1])
            .expect("production baseline migration");
        conn.execute_batch(
            "INSERT INTO token_usage (timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd,request_type)
             VALUES ('2026-07-06T12:00:00Z','session-a','worker','google','model',2,3,5,0.01,'chat'),
                    ('2026-07-06T13:00:00Z','session-b','planner','anthropic','model',2,3,5,0.01,'chat');
             INSERT INTO daily_costs (date,total_cost_usd,total_tokens,session_count,message_count,tool_call_count,provider_breakdown,agent_type_breakdown)
             VALUES ('2026-07-06',0.01,5,1,1,0,'{\"google\":0.01}','{\"worker\":0.01}');",
        )
        .expect("create ambiguous partial legacy summary");
        drop(conn);

        let db = AnalyticsDb::new_at(&path)
            .await
            .expect("production latest-schema opener preserves ambiguous summary");
        assert!(db.prune_append_only_before("2026-07-07T00:00:00Z").is_err());
        let (raw_count, summary_tokens): (i64, i64) = db
            .conn
            .lock()
            .expect("analytics connection")
            .query_row(
                "SELECT (SELECT COUNT(*) FROM token_usage),
                        (SELECT total_tokens FROM daily_costs WHERE date = '2026-07-06')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("verify fail-closed legacy preservation");
        assert_eq!((raw_count, summary_tokens), (2, 5));
    }

    #[tokio::test]
    async fn complete_legacy_daily_summary_is_adopted_before_pruning() {
        let directory = tempfile::tempdir().expect("owned legacy analytics profile");
        let path = directory.path().join("analytics.db");
        let conn = Connection::open(&path).expect("legacy analytics file");
        migrate::run_ladder(&conn, "analytics.db", &AnalyticsDb::LADDER[..1])
            .expect("production baseline migration");
        conn.execute_batch(
            "INSERT INTO token_usage (timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd,request_type)
             VALUES ('2026-07-06T12:00:00Z','session-a','worker','google','model',2,3,5,0.01,'chat'),
                    ('2026-07-06T13:00:00Z','session-b','planner','anthropic','model',2,3,5,0.01,'chat');
             INSERT INTO daily_costs (date,total_cost_usd,total_tokens,session_count,message_count,tool_call_count,provider_breakdown,agent_type_breakdown)
             VALUES ('2026-07-06',0.02,10,2,2,0,'{\"google\":0.01,\"anthropic\":0.01}','{\"worker\":0.01,\"planner\":0.01}');",
        )
        .expect("create complete legacy summary");
        drop(conn);

        let db = AnalyticsDb::new_at(&path)
            .await
            .expect("production latest-schema opener");
        assert_eq!(
            db.prune_append_only_before("2026-07-07T00:00:00Z")
                .expect("adopt only the legacy summary matching all live source aggregates"),
            2
        );
        let summary = db
            .get_daily_costs_range(
                NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
            )
            .expect("read adopted legacy summary")
            .pop()
            .expect("legacy cost row remains available");
        assert_eq!(summary.total_tokens, 10);
        assert_eq!(summary.session_count, 2);
        assert_eq!(summary.provider_breakdown["google"], 0.01);
        assert_eq!(summary.provider_breakdown["anthropic"], 0.01);
        assert_eq!(summary.agent_type_breakdown["worker"], 0.01);
        assert_eq!(summary.agent_type_breakdown["planner"], 0.01);
    }

    #[tokio::test]
    async fn raw_analytics_retention_prunes_strictly_before_cutoff_and_keeps_daily_history() {
        let (_directory, db) = database().await;
        let cutoff = "2026-07-06T12:00:00Z";
        for table in ["token_usage", "system_events", "api_failures"] {
            insert_rows(&db, table, "2026-07-06T11:59:59Z");
            insert_rows(&db, table, cutoff);
            insert_rows(&db, table, "2026-07-06T12:00:01Z");
        }
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO daily_costs (date, total_cost_usd, total_tokens, session_count,
                    message_count, tool_call_count) VALUES ('2026-01-01', 0.01, 5, 1, 1, 0)",
                [],
            )
            .expect("seed historical daily cost aggregate");

        assert_eq!(
            db.prune_append_only_before(cutoff).expect("prune raw data"),
            3
        );
        let conn = db.conn.lock().expect("analytics connection");
        for table in ["token_usage", "system_events", "api_failures"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count retained rows");
            assert_eq!(count, 2, "cutoff and newer rows remain in {table}");
        }
        let historical_cost: i64 = conn
            .query_row("SELECT COUNT(*) FROM daily_costs", [], |row| row.get(0))
            .expect("read historical aggregates");
        assert_eq!(
            historical_cost, 2,
            "daily historical cost remains exportable"
        );
        let local_date: String = conn
            .query_row(
                "SELECT date('2026-07-06T11:59:59Z', 'localtime')",
                [],
                |row| row.get(0),
            )
            .expect("derive fixture's production local day");
        let (cost, tokens, sessions, messages): (f64, i64, i64, i64) = conn
            .query_row(
                "SELECT total_cost_usd, total_tokens, session_count, message_count
                   FROM daily_costs WHERE date = ?1",
                [&local_date],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read the backfilled daily token summary");
        assert_eq!((cost, tokens, sessions, messages), (0.01, 5, 1, 1));
        drop(conn);
        assert_eq!(
            db.prune_append_only_before(cutoff)
                .expect("repeat cleanup idempotently"),
            0
        );
    }

    #[tokio::test]
    async fn analytics_retention_serializes_with_an_active_database_writer() {
        let directory = tempfile::tempdir().expect("isolated analytics database directory");
        let path = directory.path().join("analytics.db");
        let db = std::sync::Arc::new(
            AnalyticsDb::new_at(&path)
                .await
                .expect("open production analytics database"),
        );
        let mut writer = crate::db::open_tuned(&path).expect("open concurrent writer");
        let tx = writer
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .expect("begin active writer transaction");
        tx.execute(
            "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
             VALUES ('2020-01-01T00:00:00Z', 'session', 'worker', 'google', 'gemini',
                2, 3, 5, 0.01, 'chat')",
            [],
        )
        .expect("write expired token row while holding writer lock");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let prune_db = db.clone();
        let pruner = std::thread::spawn(move || {
            started_tx.send(()).expect("signal pruning thread start");
            prune_db.prune_append_only_before("2026-07-06T12:00:00Z")
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("pruning thread started");
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            !pruner.is_finished(),
            "cleanup waits for the active SQLite writer transaction"
        );
        tx.commit().expect("commit active writer");
        assert_eq!(pruner.join().expect("join retention worker").unwrap(), 1);
        let remaining: i64 = db
            .conn
            .lock()
            .expect("analytics connection")
            .query_row("SELECT COUNT(*) FROM token_usage", [], |row| row.get(0))
            .expect("read after concurrent retention");
        assert_eq!(
            remaining, 0,
            "the committed expired row is summarized then pruned"
        );
    }

    #[tokio::test]
    async fn invalid_timestamp_aborts_all_analytics_retention_deletes() {
        let (_directory, db) = database().await;
        for table in ["token_usage", "system_events", "api_failures"] {
            insert_rows(&db, table, "2020-01-01T00:00:00Z");
        }
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO system_events (timestamp, component, event_type, message, severity)
                 VALUES ('malformed', 'test', 'event', 'detail', 'info')",
                [],
            )
            .expect("insert malformed timestamp fixture");

        let (error, events) = crate::observability::timing::capture_json_events(|| {
            db.prune_append_only_before("2026-07-06T12:00:00Z")
                .expect_err("unknown row age fails closed")
        });
        assert!(error.to_string().contains("invalid timestamps"));
        assert!(events.iter().any(|event| {
            event["fields"]["operation"] == "database_analytics_retention_prune"
                && event["fields"]["outcome"] == "error"
                && event["fields"]["database"] == "analytics"
        }));
        let conn = db.conn.lock().expect("analytics connection");
        for table in ["token_usage", "system_events", "api_failures"] {
            let count: i64 = conn
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .expect("count after aborted cleanup");
            assert!(count >= 1, "cleanup remains atomic for {table}");
        }
    }
}

#[cfg(test)]
mod token_usage_observability_tests {
    use super::*;

    fn daily_cost(date: &str) -> DailyCostRecord {
        DailyCostRecord {
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("fixture date"),
            total_cost_usd: 12.34,
            total_tokens: 1234,
            session_count: 3,
            message_count: 8,
            tool_call_count: 5,
            provider_breakdown: HashMap::from([("provider-private-marker".into(), 12.34)]),
            agent_type_breakdown: HashMap::from([("agent-private-marker".into(), 12.34)]),
        }
    }

    #[tokio::test]
    async fn daily_cost_production_operations_emit_safe_result_spans() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let first = daily_cost("2026-10-01");
        let second = daily_cost("2026-10-02");

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let missing = db
                .get_daily_cost(first.date)
                .expect("read absent daily cost");
            db.insert_daily_cost(&first)
                .expect("insert first daily cost");
            db.insert_daily_cost(&second)
                .expect("insert second daily cost");
            let present = db.get_daily_cost(first.date).expect("read daily cost");
            let range = db
                .get_daily_costs_range(first.date, second.date)
                .expect("read cost range");
            db.conn
                .lock()
                .expect("analytics connection")
                .execute_batch(
                    "CREATE TRIGGER reject_test_daily_cost BEFORE INSERT ON daily_costs
                     BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
                )
                .expect("install isolated failure trigger");
            let failed = db.insert_daily_cost(&daily_cost("2026-10-03")).is_err();
            (missing, present, range, failed)
        });

        assert!(observed.0.is_none());
        assert!(observed.1.is_some());
        assert_eq!(observed.2.len(), 2);
        assert!(observed.3);
        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        let expected = [
            ("database_daily_cost_read", "success", 0),
            ("database_daily_cost_insert", "success", 1),
            ("database_daily_cost_insert", "success", 1),
            ("database_daily_cost_read", "success", 1),
            ("database_daily_cost_range_read", "success", 2),
            ("database_daily_cost_insert", "error", 0),
        ];
        assert_eq!(operation_events.len(), expected.len());
        for (event, (operation, outcome, count)) in operation_events.iter().zip(expected) {
            assert_eq!(event["fields"]["operation"], operation);
            assert_eq!(event["fields"]["outcome"], outcome);
            assert_eq!(event["fields"]["database"], "analytics");
            assert_eq!(event["fields"]["result_count"], count);
            assert_eq!(
                event["fields"]["result_count_observed"],
                outcome != "error" || count > 0
            );
        }
        let serialized = serde_json::to_string(&events).expect("serialize events");
        assert!(!serialized.contains("provider-private-marker"));
        assert!(!serialized.contains("agent-private-marker"));
        assert!(!serialized.contains("injected failure"));
    }

    #[tokio::test]
    async fn analytics_failure_and_system_event_paths_emit_private_spans() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let api_failure = ApiFailureRecord {
            id: None,
            timestamp: Utc::now(),
            provider: "provider-private-marker".into(),
            error_type: "private-error-type".into(),
            error_message: "private-error-message".into(),
            request_id: Some("private-request-id".into()),
            retry_count: 2,
            resolved: false,
        };
        let system_event = SystemEventRecord {
            id: None,
            timestamp: Utc::now(),
            component: "component-private-marker".into(),
            event_type: "event-private-marker".into(),
            message: "private-system-message".into(),
            severity: "warning".into(),
            metadata: Some("private-system-metadata".into()),
        };

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let api_id = db
                .insert_api_failure(&api_failure)
                .expect("insert API failure");
            let api_rows = db
                .get_api_failures_recent(24)
                .expect("read recent API failures");
            let system_id = db
                .insert_system_event(&system_event)
                .expect("insert system event");
            let system_rows = db
                .get_system_events_recent(Some("component-private-marker"), None, 24)
                .expect("read recent filtered system events");
            db.conn
                .lock()
                .expect("analytics connection")
                .execute_batch(
                    "CREATE TRIGGER reject_test_api_failure BEFORE INSERT ON api_failures
                     BEGIN SELECT RAISE(ABORT, 'private-trigger-error'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_write = db.insert_api_failure(&api_failure).is_err();
            (api_id, api_rows, system_id, system_rows, failed_write)
        });

        assert!(observed.0 > 0);
        assert_eq!(observed.1.len(), 1);
        assert!(observed.2 > 0);
        assert_eq!(observed.3.len(), 1);
        assert!(observed.4);
        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        let expected = [
            ("database_api_failure_insert", "success", 1),
            ("database_api_failures_recent_read", "success", 1),
            ("database_system_event_insert", "success", 1),
            ("database_system_events_recent_read", "success", 1),
            ("database_api_failure_insert", "error", 0),
        ];
        assert_eq!(operation_events.len(), expected.len());
        for (event, (operation, outcome, count)) in operation_events.iter().zip(expected) {
            assert_eq!(event["fields"]["operation"], operation);
            assert_eq!(event["fields"]["outcome"], outcome);
            assert_eq!(event["fields"]["database"], "analytics");
            assert_eq!(event["fields"]["result_count"], count);
            assert_eq!(event["fields"]["result_count_observed"], outcome != "error");
        }
        let serialized = serde_json::to_string(&events).expect("serialize events");
        for private in [
            "provider-private-marker",
            "private-error-type",
            "private-error-message",
            "private-request-id",
            "component-private-marker",
            "event-private-marker",
            "private-system-message",
            "private-system-metadata",
            "private-trigger-error",
        ] {
            assert!(!serialized.contains(private), "telemetry leaked {private}");
        }
    }

    #[tokio::test]
    async fn token_usage_writes_and_reads_emit_safe_result_spans() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let date = Local::now().date_naive();
        let record = TokenUsageRecord {
            id: None,
            timestamp: Local::now().with_timezone(&Utc),
            session_id: "sensitive-session-marker".into(),
            agent_type: "PA".into(),
            provider: "test-provider-marker".into(),
            model: "test-model-marker".into(),
            prompt_tokens: 11,
            completion_tokens: 7,
            total_tokens: 18,
            thinking_tokens: Some(3),
            cost_usd: 0.01,
            request_type: "chat".into(),
        };

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            let id = db.insert_token_usage(&record).expect("insert token usage");
            let rows = db
                .get_token_usage_by_date(date)
                .expect("read token usage through production reader");
            db.conn
                .lock()
                .expect("analytics connection")
                .execute_batch(
                    "CREATE TRIGGER reject_test_token_usage BEFORE INSERT ON token_usage
                     BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
                )
                .expect("install isolated failure trigger");
            let failed = db.insert_token_usage(&record).is_err();
            (id, rows, failed)
        });

        assert!(observed.0 > 0);
        assert_eq!(observed.1.len(), 1);
        assert!(
            observed.2,
            "the injected writer failure must remain visible"
        );
        let operation_events: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        assert_eq!(operation_events.len(), 3);
        let insert_success = operation_events
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_token_usage_insert"
                    && event["fields"]["outcome"] == "success"
            })
            .expect("successful insert result span");
        assert_eq!(insert_success["fields"]["database"], "analytics");
        assert_eq!(insert_success["fields"]["result_count"], 1);
        assert!(insert_success["fields"]["duration_ns"].as_u64().is_some());
        let read = operation_events
            .iter()
            .find(|event| event["fields"]["operation"] == "database_token_usage_date_read")
            .expect("reader result span");
        assert_eq!(read["fields"]["outcome"], "success");
        assert_eq!(read["fields"]["result_count"], 1);
        let failed_insert = operation_events
            .iter()
            .find(|event| {
                event["fields"]["operation"] == "database_token_usage_insert"
                    && event["fields"]["outcome"] == "error"
            })
            .expect("failed insert result span");
        assert_eq!(failed_insert["fields"]["database"], "analytics");
        let serialized = serde_json::to_string(&events).expect("serialize captured events");
        for sensitive in [
            "sensitive-session-marker",
            "test-provider-marker",
            "test-model-marker",
        ] {
            assert!(
                !serialized.contains(sensitive),
                "telemetry leaked {sensitive}"
            );
        }
    }

    #[tokio::test]
    async fn worker_stats_and_live_token_aggregates_emit_safe_result_spans() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let date = Local::now().date_naive();
        let worker = WorkerStatsRecord {
            worker_id: "private-worker-id-marker".into(),
            date,
            tasks_completed: 2,
            tasks_failed: 1,
            time_spent_minutes: 30,
            cost_usd: 0.75,
            avg_task_duration_minutes: 10,
            success_rate: 2.0 / 3.0,
            last_heartbeat: None,
        };
        let token = TokenUsageRecord {
            id: None,
            timestamp: Local::now().with_timezone(&Utc),
            session_id: "private-session-id-marker".into(),
            agent_type: "private-agent-marker".into(),
            provider: "private-provider-marker".into(),
            model: "private-model-marker".into(),
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
            thinking_tokens: Some(2),
            cost_usd: 0.02,
            request_type: "tool_use".into(),
        };

        let (observed, events) = crate::observability::timing::capture_json_events(|| {
            db.insert_worker_stats(&worker)
                .expect("insert worker stats");
            let present = db
                .get_worker_stats(&worker.worker_id, date)
                .expect("read worker stats");
            let missing = db
                .get_worker_stats("private-missing-worker-id-marker", date)
                .expect("read absent worker stats");
            db.insert_token_usage(&token).expect("insert token usage");
            let activity = db.get_today_activity();
            let range = db
                .get_token_usage_for_range(date, date)
                .expect("read token aggregates");
            db.conn
                .lock()
                .expect("analytics connection")
                .execute_batch(
                    "CREATE TRIGGER reject_test_worker_stats BEFORE INSERT ON worker_stats
                     BEGIN SELECT RAISE(ABORT, 'private-worker-stats-trigger-marker'); END;",
                )
                .expect("install isolated failure trigger");
            let failed_write = db.insert_worker_stats(&worker).is_err();
            (present, missing, activity, range, failed_write)
        });

        assert_eq!(observed.0.as_ref().map(|row| row.tasks_completed), Some(2));
        assert!(observed.1.is_none());
        assert_eq!(observed.2, (1, 1, 1));
        assert_eq!(observed.3 .0, 15);
        assert_eq!(observed.3 .1, 10);
        assert_eq!(observed.3 .2, 5);
        assert_eq!(observed.3 .7.len(), 1);
        assert!(observed.4);

        let operations: Vec<_> = events
            .iter()
            .filter(|event| event["fields"]["event_type"] == "operation_timing")
            .collect();
        for operation in [
            "database_worker_stats_insert",
            "database_worker_stats_point_read",
            "database_today_activity_read",
            "database_token_usage_insert",
            "database_token_usage_range_read",
            "database_token_usage_totals_read",
            "database_token_usage_by_model_read",
            "database_token_usage_by_provider_read",
            "database_token_usage_by_agent_read",
            "database_token_usage_top_sessions_read",
        ] {
            assert!(
                operations
                    .iter()
                    .any(|event| event["fields"]["operation"] == operation),
                "missing analytics span {operation}"
            );
        }
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_worker_stats_point_read"
                && event["fields"]["outcome"] == "not_found"
                && event["fields"]["result_count"] == 0
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_worker_stats_insert"
                && event["fields"]["outcome"] == "error"
        }));
        assert!(operations.iter().any(|event| {
            event["fields"]["operation"] == "database_today_activity_read"
                && event["fields"]["outcome"] == "success"
                && event["fields"]["result_count"] == 3
        }));
        for operation in [
            "database_token_usage_by_model_read",
            "database_token_usage_by_provider_read",
            "database_token_usage_by_agent_read",
            "database_token_usage_top_sessions_read",
        ] {
            assert!(operations.iter().any(|event| {
                event["fields"]["operation"] == operation
                    && event["fields"]["outcome"] == "success"
                    && event["fields"]["result_count"].as_u64().is_some()
            }));
        }
        let serialized = serde_json::to_string(&events).expect("serialize analytics telemetry");
        for private_value in [
            "private-worker-id-marker",
            "private-missing-worker-id-marker",
            "private-session-id-marker",
            "private-agent-marker",
            "private-provider-marker",
            "private-model-marker",
            "private-worker-stats-trigger-marker",
        ] {
            assert!(
                !serialized.contains(private_value),
                "leaked {private_value}"
            );
        }
    }
}

#[cfg(test)]
mod worker_stats_retention_tests {
    use super::*;
    use chrono::{Datelike, Duration, Local, Timelike};

    fn record(
        date: &str,
        completed: i32,
        failed: i32,
        duration: i32,
        heartbeat: &str,
    ) -> WorkerStatsRecord {
        WorkerStatsRecord {
            worker_id: "worker-a".into(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("fixture date"),
            tasks_completed: completed,
            tasks_failed: failed,
            time_spent_minutes: duration * (completed + failed),
            cost_usd: 0.25,
            avg_task_duration_minutes: duration,
            success_rate: 0.0,
            last_heartbeat: Some(
                chrono::DateTime::parse_from_rfc3339(heartbeat)
                    .expect("fixture heartbeat")
                    .with_timezone(&Utc),
            ),
        }
    }

    #[tokio::test]
    async fn worker_stats_accumulate_roll_up_atomically_and_read_with_coverage() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");

        db.insert_worker_stats(&record("2025-01-05", 1, 0, 10, "2025-01-05T10:00:00Z"))
            .expect("insert first daily tick");
        db.insert_worker_stats(&record("2025-01-05", 0, 1, 20, "2025-01-05T11:00:00Z"))
            .expect("accumulate second tick on same day");
        db.insert_worker_stats(&record("2025-01-10", 2, 0, 30, "2025-01-10T12:00:00Z"))
            .expect("insert another daily row in the same month");
        db.insert_worker_stats(&record("2025-02-02", 1, 0, 5, "2025-02-02T12:00:00Z"))
            .expect("insert a recent raw row");

        let daily = db
            .get_worker_stats("worker-a", NaiveDate::from_ymd_opt(2025, 1, 5).unwrap())
            .expect("read accumulated worker day")
            .expect("daily row exists");
        assert_eq!((daily.tasks_completed, daily.tasks_failed), (1, 1));
        assert_eq!(daily.avg_task_duration_minutes, 15);
        assert_eq!(daily.success_rate, 0.5);
        assert_eq!(daily.last_heartbeat.unwrap().hour(), 11);

        let cutoff = NaiveDate::from_ymd_opt(2025, 2, 1).expect("fixture cutoff");
        assert_eq!(db.rollup_and_prune_worker_stats_before(cutoff).unwrap(), 1);
        assert_eq!(db.rollup_and_prune_worker_stats_before(cutoff).unwrap(), 0);

        let conn = db.conn.lock().expect("analytics connection");
        let (completed, failed, coverage_start, coverage_end, avg_duration): (
            i64,
            i64,
            String,
            String,
            f64,
        ) = conn
            .query_row(
                "SELECT tasks_completed, tasks_failed, covered_start, covered_end,
                        avg_task_duration_minutes
                   FROM worker_stats_monthly WHERE worker_id = 'worker-a' AND month = '2025-01'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("read monthly rollup from the fixture database");
        assert_eq!((completed, failed), (3, 1));
        assert_eq!(
            (coverage_start.as_str(), coverage_end.as_str()),
            ("2025-01-05", "2025-01-10")
        );
        assert!((avg_duration - 22.5).abs() < f64::EPSILON);
        let raw_old: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM worker_stats WHERE date < ?1",
                [cutoff.to_string()],
                |row| row.get(0),
            )
            .expect("count pruned raw rows");
        assert_eq!(raw_old, 0);
        drop(conn);

        let full_month = db
            .get_worker_stats_range(
                "worker-a",
                NaiveDate::from_ymd_opt(2025, 1, 7).unwrap(),
                NaiveDate::from_ymd_opt(2025, 2, 3).unwrap(),
            )
            .expect("read mixed monthly and daily worker stats")
            .expect("combined rows exist");
        assert_eq!(
            (full_month.tasks_completed, full_month.tasks_failed),
            (4, 1)
        );
        assert_eq!(full_month.granularity, "month");
        assert_eq!(
            full_month.coverage_start,
            NaiveDate::from_ymd_opt(2025, 1, 5).unwrap()
        );
        assert_eq!(
            full_month.coverage_end,
            NaiveDate::from_ymd_opt(2025, 2, 2).unwrap()
        );
        assert!((full_month.avg_task_duration_minutes - 19.0).abs() < f64::EPSILON);
        assert_eq!(full_month.last_heartbeat.unwrap().day(), 2);
    }

    #[tokio::test]
    async fn worker_stats_rollup_failure_is_atomic_and_preserves_cutoff_day() {
        let temp = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let cutoff = NaiveDate::from_ymd_opt(2025, 3, 1).expect("fixture cutoff");

        db.insert_worker_stats(&record("2025-01-31", 1, 0, 10, "2025-01-31T10:00:00Z"))
            .expect("insert January row before cutoff");
        db.insert_worker_stats(&record("2025-02-28", 0, 1, 20, "2025-02-28T10:00:00Z"))
            .expect("insert February row before cutoff");
        db.insert_worker_stats(&record("2025-03-01", 2, 0, 5, "2025-03-01T10:00:00Z"))
            .expect("insert exact-cutoff row");

        {
            let conn = db.conn.lock().expect("analytics connection");
            conn.execute_batch(
                "CREATE TRIGGER fail_february_rollup
                 BEFORE INSERT ON worker_stats_monthly
                 WHEN NEW.month = '2025-02'
                 BEGIN SELECT RAISE(ABORT, 'injected February rollup failure'); END;",
            )
            .expect("install second-month failure injection");
        }

        assert!(db.rollup_and_prune_worker_stats_before(cutoff).is_err());
        {
            let conn = db.conn.lock().expect("analytics connection");
            let daily_rows: i64 = conn
                .query_row("SELECT COUNT(*) FROM worker_stats", [], |row| row.get(0))
                .expect("count daily rows after failed transaction");
            let monthly_rows: i64 = conn
                .query_row("SELECT COUNT(*) FROM worker_stats_monthly", [], |row| {
                    row.get(0)
                })
                .expect("count monthly rows after failed transaction");
            assert_eq!(daily_rows, 3, "failed rollup must preserve all source rows");
            assert_eq!(
                monthly_rows, 0,
                "failed rollup must not leave a partial month"
            );
            conn.execute_batch("DROP TRIGGER fail_february_rollup")
                .expect("remove failure injection");
        }

        assert_eq!(
            db.rollup_and_prune_worker_stats_before(cutoff)
                .expect("retry the complete rollup"),
            2,
            "both old month buckets should be committed"
        );
        let conn = db.conn.lock().expect("analytics connection");
        let retained_cutoff_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM worker_stats WHERE date >= ?1",
                [cutoff.to_string()],
                |row| row.get(0),
            )
            .expect("count rows on or after the cutoff");
        let old_daily_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM worker_stats WHERE date < ?1",
                [cutoff.to_string()],
                |row| row.get(0),
            )
            .expect("count rows before the cutoff");
        let monthly_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM worker_stats_monthly", [], |row| {
                row.get(0)
            })
            .expect("count committed monthly rows");
        assert_eq!(retained_cutoff_rows, 1, "cutoff-day data remains daily");
        assert_eq!(
            old_daily_rows, 0,
            "only rows strictly before cutoff are pruned"
        );
        assert_eq!(monthly_rows, 2, "both prior months were rolled up");
    }

    #[tokio::test]
    async fn analytics_manager_runs_append_only_and_worker_retention_on_startup() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let manager = crate::analytics::AnalyticsManager::new()
            .await
            .expect("open production analytics manager");
        let db = manager.db();
        let cutoff = Local::now().date_naive() - Duration::days(90);
        let old_date = cutoff - Duration::days(1);

        db.insert_worker_stats(&record(
            &old_date.to_string(),
            1,
            0,
            10,
            "2025-01-01T10:00:00Z",
        ))
        .expect("insert old row to be rolled up");
        db.insert_worker_stats(&record(
            &cutoff.to_string(),
            0,
            1,
            20,
            "2025-01-02T10:00:00Z",
        ))
        .expect("insert exact-cutoff daily row");

        let raw_old = (Utc::now() - Duration::days(91)).to_rfc3339();
        let raw_current = Utc::now().to_rfc3339();
        {
            let conn = db.conn.lock().expect("analytics connection");
            conn.execute(
                "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                    prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                 VALUES (?1, 'old-session', 'worker', 'google', 'gemini', 2, 3, 5, 0.01, 'chat')",
                [&raw_old],
            )
            .expect("seed old token usage");
            conn.execute(
                "INSERT INTO token_usage (timestamp, session_id, agent_type, provider, model,
                    prompt_tokens, completion_tokens, total_tokens, cost_usd, request_type)
                 VALUES (?1, 'current-session', 'worker', 'google', 'gemini', 2, 3, 5, 0.01, 'chat')",
                [&raw_current],
            )
            .expect("seed current token usage");
            conn.execute(
                "INSERT INTO system_events (timestamp, component, event_type, message, severity)
                 VALUES (?1, 'test', 'old', 'old', 'info'), (?2, 'test', 'current', 'current', 'info')",
                rusqlite::params![&raw_old, &raw_current],
            )
            .expect("seed system event retention fixtures");
            conn.execute(
                "INSERT INTO api_failures (timestamp, provider, error_type, error_message)
                 VALUES (?1, 'google', 'old', 'old'), (?2, 'google', 'current', 'current')",
                rusqlite::params![&raw_old, &raw_current],
            )
            .expect("seed API failure retention fixtures");
        }
        let config = crate::db::config_db::open_at(&home.path().join(".arcana/config.db"))
            .expect("open isolated config database");
        config
            .execute(
                "INSERT INTO hook_logs (hook_id, hook_name, hook_type, outcome, executed_at)
                 VALUES ('old', 'old', 'pre_tool_use', 'executed', ?1),
                        ('current', 'current', 'pre_tool_use', 'executed', ?2)",
                rusqlite::params![&raw_old, &raw_current],
            )
            .expect("seed hook-log retention fixtures");

        manager.start_aggregation_job();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let monthly_count: i64 = db
                    .conn
                    .lock()
                    .expect("analytics connection")
                    .query_row(
                        "SELECT COUNT(*) FROM worker_stats_monthly WHERE worker_id = 'worker-a'",
                        [],
                        |row| row.get(0),
                    )
                    .expect("read rollup written by scheduled job");
                let token_count: i64 = db
                    .conn
                    .lock()
                    .expect("analytics connection")
                    .query_row("SELECT COUNT(*) FROM token_usage", [], |row| row.get(0))
                    .expect("read token usage retention result");
                let event_count: i64 = db
                    .conn
                    .lock()
                    .expect("analytics connection")
                    .query_row("SELECT COUNT(*) FROM system_events", [], |row| row.get(0))
                    .expect("read event retention result");
                let failure_count: i64 = db
                    .conn
                    .lock()
                    .expect("analytics connection")
                    .query_row("SELECT COUNT(*) FROM api_failures", [], |row| row.get(0))
                    .expect("read API failure retention result");
                let hook_count: i64 = config
                    .query_row("SELECT COUNT(*) FROM hook_logs", [], |row| row.get(0))
                    .expect("read hook-log retention result");
                if monthly_count == 1
                    && token_count == 1
                    && event_count == 1
                    && failure_count == 1
                    && hook_count == 1
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("startup rollup should finish within the test bound");
        manager.stop_aggregation_job();

        assert!(db
            .get_worker_stats("worker-a", old_date)
            .expect("read old daily point")
            .is_none());
        assert!(db
            .get_worker_stats("worker-a", cutoff)
            .expect("read cutoff daily point")
            .is_some());
        assert_eq!(
            db.get_system_events_recent(None, None, 168)
                .expect("read retained current system event")
                .len(),
            1
        );
        assert_eq!(
            db.get_api_failures_recent(168)
                .expect("read retained current API failure")
                .len(),
            1
        );
        assert!(
            db.get_daily_costs_range(old_date, old_date)
                .expect("read daily aggregate from expired token row")
                .iter()
                .any(|row| row.total_tokens == 5),
            "old token history is visible after raw detail pruning"
        );
        let hook_logs = crate::hooks::db::HooksDb::open()
            .expect("open production hook-log reader")
            .get_logs(None, 10)
            .expect("read retained current hook log");
        assert_eq!(hook_logs.len(), 1);
        assert_eq!(hook_logs[0].hook_name, "current");
        let range = db
            .get_worker_stats_range("worker-a", old_date, old_date)
            .expect("read rolled historical point")
            .expect("monthly historical coverage exists");
        assert_eq!(range.granularity, "month");
        assert_eq!(range.coverage_start, old_date);
        assert_eq!(range.coverage_end, old_date);
    }
}

#[cfg(test)]
mod local_day_tests {
    use super::*;
    use chrono::{Duration, Local, Timelike};

    fn local_day_start_utc(date: NaiveDate) -> UtcDateTime {
        for minute in 0..=180 {
            let local_time = date.and_hms_opt(0, 0, 0).expect("valid local midnight")
                + Duration::minutes(minute);
            match local_time.and_local_timezone(Local) {
                chrono::LocalResult::Single(value) => return value.with_timezone(&Utc),
                chrono::LocalResult::Ambiguous(first, _) => return first.with_timezone(&Utc),
                chrono::LocalResult::None => continue,
            }
        }
        panic!("could not find the first valid local instant for {date}");
    }

    type UtcDateTime = chrono::DateTime<Utc>;

    fn token_usage(
        timestamp: UtcDateTime,
        session_id: &str,
        request_type: &str,
    ) -> TokenUsageRecord {
        TokenUsageRecord {
            id: None,
            timestamp,
            session_id: session_id.into(),
            agent_type: "worker".into(),
            provider: "google".into(),
            model: "gemini-test".into(),
            prompt_tokens: 4,
            completion_tokens: 3,
            total_tokens: 7,
            thinking_tokens: None,
            cost_usd: 0.01,
            request_type: request_type.into(),
        }
    }

    #[tokio::test]
    async fn utc_instants_are_grouped_by_the_users_local_calendar_day() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("create isolated analytics database");
        let today = Local::now().date_naive();
        let start = local_day_start_utc(today);
        let inside_today = start + Duration::minutes(1);
        let previous_local_day = start - Duration::seconds(1);
        let today_id = db
            .insert_token_usage(&token_usage(inside_today, "today-session", "tool_use"))
            .expect("insert usage after local midnight");
        let previous_id = db
            .insert_token_usage(&token_usage(previous_local_day, "previous-session", "chat"))
            .expect("insert usage before local midnight");

        let today_records = db
            .get_token_usage_by_date(today)
            .expect("read current local day");
        assert_eq!(today_records.len(), 1);
        assert_eq!(today_records[0].id, Some(today_id));
        assert_eq!(
            db.get_token_usage_by_date(today - Duration::days(1))
                .expect("read previous local day")
                .iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![Some(previous_id)]
        );

        assert_eq!(db.get_today_activity(), (1, 1, 1));
        let (today_tokens, ..) = db
            .get_token_usage_for_range(today, today)
            .expect("aggregate current local day");
        let (previous_tokens, ..) = db
            .get_token_usage_for_range(today - Duration::days(1), today - Duration::days(1))
            .expect("aggregate previous local day");
        assert_eq!(today_tokens, 7);
        assert_eq!(previous_tokens, 7);
    }

    #[tokio::test]
    async fn recent_failure_reader_handles_legacy_rows_and_old_new_cutover() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("create isolated analytics database");
        let now = Utc::now();
        // Pick a cutoff that leaves room before midnight so the deliberately
        // old RFC3339 row shares the cutoff's calendar date. That is the case
        // where lexical comparison with SQLite's space-separated cutoff fails.
        let (hours, cutoff) = (1..=168)
            .map(|hours| (hours, now - Duration::hours(hours as i64)))
            .find(|(_, cutoff)| cutoff.hour() >= 2 && cutoff.hour() < 23)
            .expect("a suitable recent cutoff exists");
        let old_canonical = cutoff - Duration::minutes(1);
        let recent_legacy = cutoff + Duration::minutes(1);
        let recent_canonical = cutoff + Duration::minutes(2);
        let conn = db.conn.lock().expect("analytics connection");
        for (timestamp, id) in [
            (
                crate::db::timestamps::format_utc_timestamp(old_canonical),
                "old",
            ),
            (
                recent_legacy.format("%Y-%m-%d %H:%M:%S").to_string(),
                "legacy",
            ),
            (
                crate::db::timestamps::format_utc_timestamp(recent_canonical),
                "new",
            ),
        ] {
            conn.execute(
                "INSERT INTO api_failures (timestamp, provider, error_type, error_message, request_id)
                 VALUES (?1, 'google', 'test', ?2, ?2)",
                params![timestamp, id],
            )
            .expect("seed representation boundary");
        }
        drop(conn);

        let rows = db
            .get_api_failures_recent(hours)
            .expect("production recent-window reader");
        assert_eq!(rows.len(), 2, "old RFC3339 row must be outside cutoff");
        assert_eq!(
            rows.iter()
                .map(|row| row.request_id.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("new"), Some("legacy")],
            "rows: {:?}",
            rows.iter()
                .map(|row| (row.request_id.as_deref(), row.timestamp))
                .collect::<Vec<_>>(),
        );
    }

    #[tokio::test]
    async fn existing_analytics_default_is_normalized_by_startup_trigger() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let path = temp.path().join("analytics.db");
        {
            let conn = Connection::open(&path).expect("create legacy analytics DB");
            conn.execute_batch(
                "CREATE TABLE api_failures (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    provider TEXT NOT NULL, error_type TEXT NOT NULL,
                    error_message TEXT NOT NULL, request_id TEXT,
                    retry_count INTEGER NOT NULL DEFAULT 0,
                    resolved BOOLEAN NOT NULL DEFAULT 0
                );
                PRAGMA user_version = 0;",
            )
            .expect("legacy schema");
        }

        let db = AnalyticsDb::new_at(&path)
            .await
            .expect("production analytics opener");
        let has_session_chronology_index: bool = db
            .conn
            .lock()
            .expect("analytics connection")
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='index'
                 AND name='idx_token_usage_session_julianday')",
                [],
                |row| row.get(0),
            )
            .expect("read migrated chronology index");
        assert!(
            has_session_chronology_index,
            "production analytics migration installs the session chronology index"
        );
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO api_failures (provider, error_type, error_message)
                 VALUES ('google', 'test', 'default timestamp')",
                [],
            )
            .expect("insert using existing legacy default");
        let timestamp: String = db
            .conn
            .lock()
            .expect("analytics connection")
            .query_row("SELECT timestamp FROM api_failures", [], |row| row.get(0))
            .expect("read default timestamp");
        assert_eq!(timestamp.len(), 24);
        assert!(timestamp.ends_with('Z'));
        assert_eq!(db.get_api_failures_recent(168).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn malformed_recent_timestamp_is_returned_as_a_reader_error() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("create isolated analytics database");
        db.conn
            .lock()
            .expect("analytics connection")
            .execute(
                "INSERT INTO api_failures (timestamp, provider, error_type, error_message)
                 VALUES ('not-a-timestamp', 'google', 'test', 'malformed')",
                [],
            )
            .expect("seed malformed persisted value");

        let error = db
            .get_api_failures_recent(24)
            .expect_err("malformed timestamp must not become a plausible current time");
        assert!(error.to_string().contains("unrecognised timestamp"));
    }

    #[tokio::test]
    async fn recent_system_events_keep_time_window_and_filters() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("create isolated analytics database");
        let now = Utc::now();
        for (timestamp, component) in [
            (now - Duration::days(2), "provider"),
            (now - Duration::minutes(10), "provider"),
            (now - Duration::minutes(5), "fleet"),
        ] {
            db.insert_system_event(&SystemEventRecord {
                id: None,
                timestamp,
                component: component.into(),
                event_type: "test".into(),
                message: "fixture".into(),
                severity: "info".into(),
                metadata: None,
            })
            .expect("insert fixture system event");
        }

        let rows = db
            .get_system_events_recent(Some("provider"), Some("info"), 24)
            .expect("read recent filtered system events");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].component, "provider");
        assert_eq!(rows[0].message, "fixture");
    }

    #[tokio::test]
    async fn recent_system_event_filter_variants_bind_values_and_return_exact_rows() {
        let temp = tempfile::tempdir().expect("isolated analytics database directory");
        let db = AnalyticsDb::new_at(&temp.path().join("analytics.db"))
            .await
            .expect("create isolated analytics database");
        let now = Utc::now();
        for (component, severity) in [
            ("provider", "info"),
            ("provider", "error"),
            ("fleet", "info"),
            ("fleet", "error"),
        ] {
            db.insert_system_event(&SystemEventRecord {
                id: None,
                timestamp: now,
                component: component.into(),
                event_type: "fixture".into(),
                message: "recent".into(),
                severity: severity.into(),
                metadata: None,
            })
            .expect("insert recent event variant");
        }
        db.insert_system_event(&SystemEventRecord {
            id: None,
            timestamp: now - Duration::days(2),
            component: "provider".into(),
            event_type: "fixture".into(),
            message: "outside window".into(),
            severity: "error".into(),
            metadata: None,
        })
        .expect("insert old event");

        assert_eq!(
            db.get_system_events_recent(None, None, 24).unwrap().len(),
            4
        );
        assert_eq!(
            db.get_system_events_recent(Some("provider"), None, 24)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            db.get_system_events_recent(None, Some("error"), 24)
                .unwrap()
                .len(),
            2
        );
        let filtered = db
            .get_system_events_recent(Some("provider"), Some("error"), 24)
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].component, "provider");
        assert_eq!(filtered[0].severity, "error");
        assert!(db
            .get_system_events_recent(Some("provider' OR 1=1 --"), None, 24)
            .unwrap()
            .is_empty());
    }
}

#[cfg(test)]
mod query_plan_tests {
    use super::*;
    use chrono::Duration;
    use rusqlite::ToSql;

    fn explain(connection: &Connection, sql: &str, values: &[&dyn ToSql]) -> Vec<String> {
        let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = connection
            .prepare(&explain_sql)
            .expect("prepare production query plan");
        statement
            .query_map(values, |row| row.get::<_, String>(3))
            .expect("explain production query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan")
    }

    #[tokio::test]
    async fn worker_session_token_usage_uses_session_index_and_returns_persisted_totals() {
        let directory = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&directory.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let connection = db.conn.lock().expect("analytics connection");
        for index in 0..500 {
            connection
                .execute(
                    "INSERT INTO token_usage
                     (timestamp, session_id, agent_type, provider, model, prompt_tokens,
                      completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
                     VALUES ('2025-01-01T00:00:00Z', ?1, 'worker', 'google',
                             'gemini-3.8-flash', 2, 3, 5, 1, 0.02, 'chat')",
                    [format!("session-{index}")],
                )
                .expect("populate session-index fixture");
        }
        connection
            .execute(
                "INSERT INTO token_usage
                 (timestamp, session_id, agent_type, provider, model, prompt_tokens,
                  completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
                 VALUES ('2025-01-01T00:00:00Z', 'worker-session', 'worker', 'google',
                         'gemini-3.8-flash', 11, 7, 18, 2, 0.12, 'chat')",
                [],
            )
            .expect("insert first worker session usage");
        connection
            .execute(
                "INSERT INTO token_usage
                 (timestamp, session_id, agent_type, provider, model, prompt_tokens,
                  completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
                 VALUES ('2025-01-01T00:01:00Z', 'worker-session', 'worker', 'google',
                         'gemini-3.8-flash', 5, 4, 9, NULL, 0.08, 'tool_use')",
                [],
            )
            .expect("insert second worker session usage");
        connection
            .execute_batch("ANALYZE")
            .expect("analyze populated session index");

        let details = explain(
            &connection,
            TOKEN_USAGE_FOR_SESSION_SQL,
            &[&"worker-session"],
        );
        assert!(
            details.iter().any(|detail| {
                detail.contains("SEARCH token_usage") && detail.contains("idx_token_usage_session")
            }),
            "worker token reader must use the session index; plan: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN token_usage")),
            "worker token reader scanned token_usage; plan: {details:?}"
        );
        drop(connection);

        assert_eq!(
            db.get_token_usage_for_session("worker-session")
                .expect("read through production worker-health consumer"),
            SessionTokenUsageSummary {
                request_count: 2,
                prompt_tokens: 16,
                completion_tokens: 11,
                total_tokens: 27,
                thinking_tokens: Some(2),
                cost_usd: 0.20,
            }
        );
        assert_eq!(
            db.get_token_usage_for_session("missing-session")
                .expect("empty session is a known zero aggregate"),
            SessionTokenUsageSummary {
                request_count: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                thinking_tokens: None,
                cost_usd: 0.0,
            }
        );
    }

    #[tokio::test]
    async fn dashboard_daily_cost_range_uses_primary_key_index_with_populated_rows() {
        let directory = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&directory.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let connection = db.conn.lock().expect("analytics connection");
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).expect("fixture date");
        for offset in 0..180 {
            let date = start + Duration::days(offset);
            connection
                .execute(
                    "INSERT INTO daily_costs
                     (date, total_cost_usd, total_tokens, session_count, message_count,
                      tool_call_count, provider_breakdown, agent_type_breakdown)
                     VALUES (?1, 1.0, 10, 1, 1, 0, '{}', '{}')",
                    [date.to_string()],
                )
                .expect("populate dashboard daily costs");
        }
        connection
            .execute_batch("ANALYZE")
            .expect("analyze fixture");

        let explain_sql = format!("EXPLAIN QUERY PLAN {DAILY_COSTS_RANGE_SQL}");
        let mut statement = connection
            .prepare(&explain_sql)
            .expect("prepare production dashboard query plan");
        let details = statement
            .query_map(
                [start.to_string(), (start + Duration::days(89)).to_string()],
                |row| row.get::<_, String>(3),
            )
            .expect("explain production daily costs query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect query plan");

        assert!(
            details.iter().any(|detail| {
                detail.contains("SEARCH daily_costs")
                    && (detail.contains("sqlite_autoindex_daily_costs_1")
                        || detail.contains("PRIMARY KEY"))
            }),
            "daily-cost range query should use the date primary key; plan: {details:?}"
        );
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains("SCAN daily_costs")),
            "daily-cost range query scanned the dashboard table; plan: {details:?}"
        );
        drop(statement);

        let point_plan_sql = format!("EXPLAIN QUERY PLAN {DAILY_COST_BY_DATE_SQL}");
        let mut point_statement = connection
            .prepare(&point_plan_sql)
            .expect("prepare point daily-cost plan");
        let point_details = point_statement
            .query_map([start.to_string()], |row| row.get::<_, String>(3))
            .expect("explain point daily-cost query")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("collect point daily-cost plan");
        assert!(
            point_details.iter().any(|detail| {
                detail.contains("SEARCH daily_costs")
                    && (detail.contains("sqlite_autoindex_daily_costs_1")
                        || detail.contains("PRIMARY KEY"))
            }),
            "daily-cost point lookup should use the date primary key: {point_details:?}"
        );
        drop(point_statement);
        drop(connection);
        let record = db
            .get_daily_cost(start)
            .expect("production daily-cost point lookup")
            .expect("populated daily-cost row");
        assert_eq!(record.total_tokens, 10);
        assert_eq!(record.session_count, 1);
    }

    #[tokio::test]
    async fn dashboard_live_analytics_plans_classify_scans_and_worker_index_use() {
        let directory = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&directory.path().join("analytics.db"))
            .await
            .expect("open isolated analytics database");
        let connection = db.conn.lock().expect("analytics connection");
        let start = NaiveDate::from_ymd_opt(2025, 1, 1).expect("fixture date");
        // Keep the requested window selective enough that SQLite's cost model
        // should choose the timestamp index over a full scan.
        let end = start + Duration::days(2);
        let timestamp_start = chrono::DateTime::parse_from_rfc3339("2025-01-01T00:00:00Z")
            .expect("fixture timestamp")
            .with_timezone(&Utc);

        for offset in 0..2400 {
            let instant = timestamp_start + Duration::hours(offset);
            let timestamp = if offset % 17 == 0 {
                instant.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                crate::db::timestamps::format_utc_timestamp(instant)
            };
            connection
                .execute(
                    "INSERT INTO token_usage
                     (timestamp, session_id, agent_type, provider, model, prompt_tokens,
                      completion_tokens, total_tokens, cost_usd, request_type)
                     VALUES (?1, ?2, ?3, ?4, ?5, 4, 3, 7, 0.01, ?6)",
                    params![
                        timestamp,
                        format!("session-{}", offset % 60),
                        format!("agent-{}", offset % 4),
                        format!("provider-{}", offset % 3),
                        format!("model-{}", offset % 8),
                        if offset % 5 == 0 { "tool_use" } else { "chat" },
                    ],
                )
                .expect("populate token usage");
        }

        let now = Utc::now();
        for offset in 0..1200 {
            let date = start + Duration::days(offset / 12);
            let heartbeat =
                crate::db::timestamps::format_utc_timestamp(now - Duration::minutes(offset as i64));
            connection
                .execute(
                    "INSERT INTO worker_stats
                     (worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                      cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                     VALUES (?1, ?2, 1, 0, 5, 0.1, 5, 1.0, ?3)",
                    params![
                        format!("worker-{}", offset % 12),
                        date.to_string(),
                        heartbeat
                    ],
                )
                .expect("populate worker stats");
        }

        for offset in 0..1200 {
            let timestamp =
                crate::db::timestamps::format_utc_timestamp(now - Duration::minutes(offset as i64));
            connection
                .execute(
                    "INSERT INTO api_failures
                     (timestamp, provider, error_type, error_message, request_id)
                     VALUES (?1, 'provider', 'test', 'fixture', ?2)",
                    params![timestamp, format!("failure-{offset}")],
                )
                .expect("populate API failures");
            connection
                .execute(
                    "INSERT INTO system_events
                     (timestamp, component, event_type, message, severity)
                     VALUES (?1, 'analytics', 'test', 'fixture', 'info')",
                    [timestamp],
                )
                .expect("populate system events");
        }

        connection
            .execute_batch("ANALYZE")
            .expect("analyze populated analytics fixture");

        let start_text = start.to_string();
        let end_text = end.to_string();
        let range_bounds = local_date_range_bounds(&start_text, &end_text)
            .expect("resolve local-date query bounds");
        let range_parameters: [&dyn ToSql; 4] = [
            &range_bounds[0],
            &range_bounds[1],
            &range_bounds[2],
            &range_bounds[3],
        ];
        let token_detail_plan = explain(
            &connection,
            TOKEN_USAGE_BY_LOCAL_DATE_SQL,
            &range_parameters,
        );
        assert!(
            token_detail_plan
                .iter()
                .any(|detail| detail.contains("idx_token_usage_timestamp")),
            "local-day token detail should seek the timestamp index for canonical/legacy rows; plan: {token_detail_plan:?}"
        );
        assert!(
            token_detail_plan
                .iter()
                .any(|detail| detail.contains("idx_token_usage_julianday")),
            "local-day token detail should find malformed rows through its expression index; plan: {token_detail_plan:?}"
        );
        assert!(
            !token_detail_plan
                .iter()
                .any(|detail| detail.contains("SCAN token_usage")),
            "local-day token detail scanned the table; plan: {token_detail_plan:?}"
        );
        for (name, sql) in [
            ("token totals", TOKEN_TOTALS_RANGE_SQL),
            ("top sessions", TOKEN_TOP_SESSIONS_SQL),
        ] {
            let details = explain(&connection, sql, &range_parameters);
            assert!(
                details.iter().any(|detail| {
                    detail.contains("idx_token_usage_timestamp")
                        && (detail.contains("SEARCH") || detail.contains("INDEX") )
                }),
                "{name} should use the token timestamp index for local-day ranges; plan: {details:?}"
            );
            assert!(
                !details.iter().any(|detail| detail.contains("SCAN token_usage")),
                "{name} scanned token_usage instead of using its timestamp index; plan: {details:?}"
            );
        }

        for (column, sql) in [
            ("model", TOKEN_USAGE_BY_MODEL_SQL),
            ("provider", TOKEN_USAGE_BY_PROVIDER_SQL),
            ("agent_type", TOKEN_USAGE_BY_AGENT_TYPE_SQL),
        ] {
            let details = explain(&connection, &sql, &range_parameters);
            assert!(
                details.iter().any(|detail| {
                    detail.contains("idx_token_usage_timestamp")
                        && (detail.contains("SEARCH") || detail.contains("INDEX"))
                }),
                "token grouping by {column} should use the timestamp index; plan: {details:?}"
            );
            assert!(
                details
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE")),
                "token grouping by {column} requires a grouping/order temporary B-tree; plan: {details:?}"
            );
        }

        let today = chrono::Local::now().naive_local().date().to_string();
        let today_bounds = local_date_range_bounds(&today, &today).expect("resolve today bounds");
        let today_parameters: [&dyn ToSql; 4] = [
            &today_bounds[0],
            &today_bounds[1],
            &today_bounds[2],
            &today_bounds[3],
        ];
        for (name, sql) in [
            ("today sessions", TOKEN_ACTIVITY_SESSIONS_SQL),
            ("today messages", TOKEN_ACTIVITY_MESSAGES_SQL),
            ("today tool calls", TOKEN_ACTIVITY_TOOL_CALLS_SQL),
        ] {
            let details = explain(&connection, sql, &today_parameters);
            assert!(
                details.iter().any(|detail| {
                    detail.contains("idx_token_usage_timestamp")
                        && (detail.contains("SEARCH") || detail.contains("INDEX"))
                }),
                "{name} should use the token timestamp index; plan: {details:?}"
            );
            assert!(
                !details.iter().any(|detail| detail.contains("SCAN token_usage")),
                "{name} scanned token_usage instead of using its timestamp index; plan: {details:?}"
            );
        }

        let worker_id = "worker-3";
        let worker_details = explain(
            &connection,
            WORKER_STATS_RANGE_SQL,
            &[&worker_id, &start_text, &end_text],
        );
        assert!(
            worker_details.iter().any(|detail| {
                detail.contains("SEARCH worker_stats")
                    && (detail.contains("idx_worker_stats_worker")
                        || detail.contains("sqlite_autoindex_worker_stats"))
            }),
            "worker stats range should use its worker lookup index; plan: {worker_details:?}"
        );
        assert!(
            !worker_details
                .iter()
                .any(|detail| detail.contains("SCAN worker_stats")),
            "worker stats range must not scan the worker_stats table; plan: {worker_details:?}"
        );

        let hours = "-168";
        let failure_details = explain(&connection, API_FAILURES_RECENT_SQL, &[&hours]);
        assert!(
            failure_details
                .iter()
                .any(|detail| detail.contains("idx_api_failures_julianday")),
            "recent API failures should use the julianday expression index; plan: {failure_details:?}"
        );
        assert!(
            !failure_details
                .iter()
                .any(|detail| detail.contains("SCAN api_failures")),
            "recent API failures scanned the table; plan: {failure_details:?}"
        );

        let cutoff = "-168 hours";
        let component = "analytics";
        let severity = "info";
        for (has_component, has_severity) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            let mut parameters: Vec<&dyn ToSql> = vec![&cutoff];
            if has_component {
                parameters.push(&component);
            }
            if has_severity {
                parameters.push(&severity);
            }
            let event_details = explain(
                &connection,
                system_events_recent_sql(has_component, has_severity),
                &parameters,
            );
            assert!(
                event_details
                    .iter()
                    .any(|detail| detail.contains("idx_system_events_julianday")),
                "recent system event variant ({has_component}, {has_severity}) should use the julianday expression index; plan: {event_details:?}"
            );
            assert!(
                event_details
                    .iter()
                    .any(|detail| detail.contains("USE TEMP B-TREE FOR ORDER BY")),
                "recent system event variant ({has_component}, {has_severity}) requires a temporary timestamp sort; plan: {event_details:?}"
            );
        }

        // Exercise the same production consumer used by the live token
        // analytics reader. These isolated August rows avoid the larger
        // January plan fixture, making every expected aggregate explicit.
        for (
            timestamp,
            session,
            agent,
            provider,
            model,
            prompt,
            completion,
            total,
            thinking,
            cost,
        ) in [
            (
                "2025-08-01T12:00:00Z",
                "session-a",
                "builder",
                "openai",
                "gpt-x",
                6,
                4,
                10,
                Some(2),
                0.10,
            ),
            (
                "2025-08-01T13:00:00Z",
                "session-a",
                "builder",
                "anthropic",
                "claude",
                5,
                5,
                10,
                Some(1),
                0.20,
            ),
            (
                "2025-08-01T14:00:00Z",
                "session-b",
                "planner",
                "openai",
                "gpt-x",
                20,
                10,
                30,
                None,
                0.30,
            ),
            (
                "2025-08-01T15:00:00Z",
                "session-c",
                "coder",
                "google",
                "gemini",
                7,
                3,
                10,
                Some(4),
                0.40,
            ),
            // Adjacent-day data proves the consumer applies the requested range.
            (
                "2025-08-03T12:00:00Z",
                "outside",
                "other",
                "other",
                "other",
                100,
                100,
                200,
                Some(9),
                9.00,
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO token_usage
                     (timestamp, session_id, agent_type, provider, model, prompt_tokens,
                      completion_tokens, total_tokens, thinking_tokens, cost_usd, request_type)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'chat')",
                    params![
                        timestamp, session, agent, provider, model, prompt, completion, total,
                        thinking, cost,
                    ],
                )
                .expect("insert explicit token analytics fixture");
        }
        drop(connection);

        let analytics = db
            .get_token_usage_for_range(
                NaiveDate::from_ymd_opt(2025, 8, 1).expect("consumer start date"),
                NaiveDate::from_ymd_opt(2025, 8, 1).expect("consumer end date"),
            )
            .expect("read token analytics through production consumer");
        let (total, prompt, completion, thinking, by_agent, by_model, by_provider, top_sessions) =
            analytics;
        assert_eq!((total, prompt, completion, thinking), (60, 38, 22, Some(7)));
        assert_eq!(
            by_agent,
            HashMap::from([
                ("builder".to_string(), 20),
                ("planner".to_string(), 30),
                ("coder".to_string(), 10),
            ])
        );
        assert_eq!(
            by_model,
            HashMap::from([
                ("gpt-x".to_string(), 40),
                ("claude".to_string(), 10),
                ("gemini".to_string(), 10),
            ])
        );
        assert_eq!(
            by_provider,
            HashMap::from([
                ("openai".to_string(), 40),
                ("anthropic".to_string(), 10),
                ("google".to_string(), 10),
            ])
        );
        assert_eq!(
            top_sessions
                .iter()
                .map(|session| (
                    session.session_id.as_str(),
                    session.tokens,
                    session.agent_type.as_str()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("session-b", 30, "planner"),
                ("session-a", 20, "builder"),
                ("session-c", 10, "coder"),
            ]
        );
        for (session, expected_cost) in top_sessions.iter().zip([0.30, 0.30, 0.40]) {
            assert!((session.cost_usd - expected_cost).abs() < 1e-9);
        }
    }

    #[tokio::test]
    async fn worker_stats_readers_use_populated_indexes_and_bound_daily_monthly_results() {
        let directory = tempfile::tempdir().expect("isolated analytics directory");
        let db = AnalyticsDb::new_at(&directory.path().join("analytics.db"))
            .await
            .expect("open isolated migrated analytics database");
        let connection = db.conn.lock().expect("analytics connection");

        // Populate enough rows for SQLite's planner to choose selective worker
        // indexes, while keeping the target worker's expected results explicit.
        let noise_start = NaiveDate::from_ymd_opt(2023, 1, 1).expect("noise start");
        for offset in 0..1200 {
            let date = noise_start + Duration::days(offset / 12);
            connection
                .execute(
                    "INSERT INTO worker_stats
                     (worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                      cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                     VALUES (?1, ?2, 1, 0, 5, 0.1, 5, 1.0, '2023-01-01T00:00:00Z')",
                    params![format!("noise-worker-{}", offset % 12), date.to_string()],
                )
                .expect("populate worker-stat plan fixture");
        }
        let recent_noise_start = NaiveDate::from_ymd_opt(2025, 1, 12).expect("recent noise start");
        for offset in 0..1200 {
            let date = recent_noise_start + Duration::days(offset / 12);
            connection
                .execute(
                    "INSERT INTO worker_stats
                     (worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                      cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                     VALUES (?1, ?2, 1, 0, 5, 0.1, 5, 1.0, '2025-01-12T00:00:00Z')",
                    params![format!("noise-worker-{}", offset % 12), date.to_string()],
                )
                .expect("populate retained worker-stat plan fixture");
        }

        let worker_id = "worker-target";
        let target_rows = [
            ("2025-01-02", 2, 1, 30, 0.2, 10, "2025-01-02T10:00:00Z"),
            ("2025-01-05", 1, 1, 10, 0.1, 20, "2025-01-05T11:00:00Z"),
            ("2025-01-10", 3, 0, 60, 0.3, 30, "2025-01-10T12:00:00Z"),
            ("2025-02-02", 4, 1, 10, 0.5, 12, "2025-02-02T13:00:00Z"),
            ("2025-02-04", 90, 0, 900, 9.0, 90, "2025-02-04T14:00:00Z"),
        ];
        for (date, completed, failed, minutes, cost, avg_duration, heartbeat) in target_rows {
            connection
                .execute(
                    "INSERT INTO worker_stats
                     (worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                      cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        worker_id,
                        date,
                        completed,
                        failed,
                        minutes,
                        cost,
                        avg_duration,
                        if completed + failed > 0 {
                            completed as f64 / (completed + failed) as f64
                        } else {
                            0.0
                        },
                        heartbeat,
                    ],
                )
                .expect("insert target worker-stat row");
        }
        connection
            .execute(
                "INSERT INTO worker_stats
                 (worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                  cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat)
                 VALUES ('worker-other', '2025-02-02', 700, 0, 700, 7.0, 70, 1.0,
                         '2025-02-02T14:00:00Z')",
                [],
            )
            .expect("insert other worker row inside requested date range");
        connection
            .execute_batch("ANALYZE")
            .expect("analyze populated worker-stat daily fixture");

        let point_date = NaiveDate::from_ymd_opt(2025, 1, 5).expect("point date");
        let point_sql =
            "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd,
             avg_task_duration_minutes, success_rate, last_heartbeat
             FROM worker_stats WHERE worker_id = ? AND date = ?";
        let point_plan = explain(
            &connection,
            point_sql,
            &[&worker_id, &point_date.to_string()],
        );
        assert!(
            point_plan.iter().any(|detail| {
                detail.contains("SEARCH worker_stats")
                    && detail.contains("sqlite_autoindex_worker_stats")
            }),
            "worker/date point lookup should use the UNIQUE(worker_id, date) index; plan: {point_plan:?}"
        );
        assert!(
            !point_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_stats")),
            "worker/date point lookup must not scan worker_stats; plan: {point_plan:?}"
        );

        let daily_range_sql =
            "SELECT date, tasks_completed, tasks_failed, time_spent_minutes, cost_usd,
                            avg_task_duration_minutes, last_heartbeat
                       FROM worker_stats
                      WHERE worker_id = ?1 AND date BETWEEN ?2 AND ?3";
        let start = NaiveDate::from_ymd_opt(2025, 1, 7).expect("range start");
        let end = NaiveDate::from_ymd_opt(2025, 2, 3).expect("range end");
        let daily_plan = explain(
            &connection,
            daily_range_sql,
            &[&worker_id, &start.to_string(), &end.to_string()],
        );
        assert!(
            daily_plan.iter().any(|detail| {
                detail.contains("SEARCH worker_stats")
                    && (detail.contains("idx_worker_stats_worker")
                        || detail.contains("sqlite_autoindex_worker_stats"))
            }),
            "daily range reader should seek a worker index before applying its date bounds; plan: {daily_plan:?}"
        );
        assert!(
            !daily_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_stats")),
            "daily range reader must not scan worker_stats; plan: {daily_plan:?}"
        );

        drop(connection);

        let exact_day = db
            .get_worker_stats(worker_id, point_date)
            .expect("read worker/day through production reader")
            .expect("populated worker/day exists");
        assert_eq!(exact_day.worker_id, worker_id);
        assert_eq!(exact_day.date, point_date);
        assert_eq!((exact_day.tasks_completed, exact_day.tasks_failed), (1, 1));
        assert_eq!(exact_day.time_spent_minutes, 10);
        assert_eq!(exact_day.cost_usd, 0.1);
        assert!(
            db.get_worker_stats("worker-other", point_date)
                .expect("read other worker point")
                .is_none(),
            "point lookup must not return another worker's same-date row"
        );
        assert!(
            db.get_worker_stats(worker_id, end + Duration::days(2))
                .expect("read absent point")
                .is_none(),
            "point lookup must return None when the worker/date pair is absent"
        );

        assert!(
            db.rollup_and_prune_worker_stats_before(
                NaiveDate::from_ymd_opt(2025, 1, 12).expect("rollup cutoff")
            )
            .expect("roll old worker days into monthly summaries")
                > 0,
            "old populated worker days should produce monthly rows"
        );

        let connection = db.conn.lock().expect("analytics connection");
        connection
            .execute_batch("ANALYZE")
            .expect("analyze populated daily and monthly worker-stat fixtures");
        let monthly_range_sql = "SELECT covered_start, covered_end, tasks_completed, tasks_failed,
                            time_spent_minutes, cost_usd, avg_task_duration_minutes, last_heartbeat
                       FROM worker_stats_monthly
                      WHERE worker_id = ?1 AND covered_end >= ?2 AND covered_start <= ?3";
        let monthly_plan = explain(
            &connection,
            monthly_range_sql,
            &[&worker_id, &start.to_string(), &end.to_string()],
        );
        assert!(
            monthly_plan.iter().any(|detail| {
                detail.contains("SEARCH worker_stats_monthly")
                    && detail.contains("sqlite_autoindex_worker_stats_monthly")
            }),
            "monthly range reader should use the (worker_id, month) primary key to select the worker; plan: {monthly_plan:?}"
        );
        assert!(
            !monthly_plan
                .iter()
                .any(|detail| detail.contains("SCAN worker_stats_monthly")),
            "monthly range reader must not scan worker_stats_monthly; plan: {monthly_plan:?}"
        );
        drop(connection);

        let monthly_and_daily = db
            .get_worker_stats_range(worker_id, start, end)
            .expect("read bounded worker range through production reader")
            .expect("monthly and daily worker rows overlap the range");
        assert_eq!(monthly_and_daily.worker_id, worker_id);
        assert_eq!(
            (
                monthly_and_daily.tasks_completed,
                monthly_and_daily.tasks_failed
            ),
            (10, 3),
            "the intersecting January rollup and in-range February day are included"
        );
        assert_eq!(monthly_and_daily.time_spent_minutes, 110);
        assert!((monthly_and_daily.cost_usd - 1.1).abs() < f64::EPSILON);
        assert_eq!(
            (
                monthly_and_daily.coverage_start,
                monthly_and_daily.coverage_end
            ),
            (
                NaiveDate::from_ymd_opt(2025, 1, 2).expect("monthly coverage start"),
                NaiveDate::from_ymd_opt(2025, 2, 2).expect("daily coverage end"),
            ),
            "monthly coverage retains its actual bounds even when the request starts mid-month"
        );
        assert_eq!(monthly_and_daily.granularity, "month");
        assert_eq!(
            monthly_and_daily.last_heartbeat.unwrap().date_naive(),
            end - Duration::days(1)
        );
        let other_worker = db
            .get_worker_stats_range("worker-other", start, end)
            .expect("read range for another worker")
            .expect("other worker row is inside the date range");
        assert_eq!(other_worker.tasks_completed, 700);
        assert_eq!(other_worker.tasks_failed, 0);
        assert!(
            db.get_worker_stats_range(
                worker_id,
                NaiveDate::from_ymd_opt(2025, 3, 1).expect("empty range start"),
                NaiveDate::from_ymd_opt(2025, 3, 2).expect("empty range end"),
            )
            .expect("read empty date range")
            .is_none(),
            "range query must return None when neither daily nor monthly coverage overlaps"
        );
        assert!(
            db.get_worker_stats(worker_id, point_date)
                .expect("read pruned point")
                .is_none(),
            "the point reader reads retained daily rows only after rollup pruning"
        );
    }
}
