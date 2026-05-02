// Analytics read-only access for airsctl analytics commands.
// Uses the same ~/.autodev/analytics.db written by the Tauri app.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use super::autodev_dir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub date:            String,
    pub total_cost_usd:  f64,
    pub total_tokens:    i64,
    pub session_count:   i64,
    pub message_count:   i64,
    pub tool_call_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageRecord {
    pub id:                i64,
    pub timestamp:         String,
    pub session_id:        String,
    pub agent_type:        String,
    pub provider:          String,
    pub model:             String,
    pub prompt_tokens:     i64,
    pub completion_tokens: i64,
    pub total_tokens:      i64,
    pub cost_usd:          f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStats {
    pub worker_id:               String,
    pub date:                    String,
    pub tasks_completed:         i64,
    pub tasks_failed:            i64,
    pub time_spent_minutes:      i64,
    pub cost_usd:                f64,
    pub avg_task_duration_minutes: i64,
    pub success_rate:            f64,
    pub last_heartbeat:          Option<String>,
}

pub struct AnalyticsReader {
    conn: Connection,
}

impl AnalyticsReader {
    pub fn open() -> Result<Self> {
        let path = autodev_dir()?.join("analytics.db");
        // Analytics DB may not exist yet — open creates it but don't run migrations
        // (the Tauri app is responsible for the schema)
        let conn = Connection::open(&path)?;
        Ok(Self { conn })
    }

    /// Cost summary for a given time window. window = "today"|"week"|"month"|"all"
    pub fn cost_summary(&self, window: &str) -> Result<Vec<CostSummary>> {
        let date_filter = match window {
            "today" => "WHERE date >= date('now')",
            "week"  => "WHERE date >= date('now', '-7 days')",
            "month" => "WHERE date >= date('now', '-30 days')",
            _       => "",
        };
        let sql = format!(
            "SELECT date, total_cost_usd, total_tokens, session_count, message_count, tool_call_count
             FROM daily_costs {date_filter} ORDER BY date DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok(CostSummary {
            date:            row.get(0)?,
            total_cost_usd:  row.get(1)?,
            total_tokens:    row.get(2)?,
            session_count:   row.get(3)?,
            message_count:   row.get(4)?,
            tool_call_count: row.get(5)?,
        }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Token usage records, optionally filtered by session ID.
    pub fn token_usage(&self, session_id: Option<&str>, limit: u32) -> Result<Vec<TokenUsageRecord>> {
        let (sql, use_filter) = if session_id.is_some() {
            ("SELECT id,timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd FROM token_usage WHERE session_id=?1 ORDER BY timestamp DESC LIMIT ?2", true)
        } else {
            ("SELECT id,timestamp,session_id,agent_type,provider,model,prompt_tokens,completion_tokens,total_tokens,cost_usd FROM token_usage ORDER BY timestamp DESC LIMIT ?1", false)
        };
        let mut stmt = self.conn.prepare(sql)?;
        let rows = if use_filter {
            stmt.query_map(params![session_id.unwrap(), limit], row_to_token_usage)?.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![limit], row_to_token_usage)?.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Worker statistics for the given date window.
    pub fn worker_stats(&self, window: &str) -> Result<Vec<WorkerStats>> {
        let date_filter = match window {
            "today" => "WHERE date >= date('now')",
            "week"  => "WHERE date >= date('now', '-7 days')",
            "month" => "WHERE date >= date('now', '-30 days')",
            _       => "",
        };
        let sql = format!(
            "SELECT worker_id, date, tasks_completed, tasks_failed, time_spent_minutes,
                    cost_usd, avg_task_duration_minutes, success_rate, last_heartbeat
             FROM worker_stats {date_filter} ORDER BY date DESC, worker_id ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok(WorkerStats {
            worker_id:                 row.get(0)?,
            date:                      row.get(1)?,
            tasks_completed:           row.get(2)?,
            tasks_failed:              row.get(3)?,
            time_spent_minutes:        row.get(4)?,
            cost_usd:                  row.get(5)?,
            avg_task_duration_minutes: row.get(6)?,
            success_rate:              row.get(7)?,
            last_heartbeat:            row.get(8)?,
        }))?.collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Export all analytics data as JSON.
    pub fn export_all(&self) -> Result<serde_json::Value> {
        let costs   = self.cost_summary("all")?;
        let tokens  = self.token_usage(None, 10_000)?;
        let workers = self.worker_stats("all")?;
        Ok(serde_json::json!({
            "cost_summary":   costs,
            "token_usage":    tokens,
            "worker_stats":   workers,
        }))
    }
}

fn row_to_token_usage(row: &rusqlite::Row<'_>) -> rusqlite::Result<TokenUsageRecord> {
    Ok(TokenUsageRecord {
        id:                row.get(0)?,
        timestamp:         row.get(1)?,
        session_id:        row.get(2)?,
        agent_type:        row.get(3)?,
        provider:          row.get(4)?,
        model:             row.get(5)?,
        prompt_tokens:     row.get(6)?,
        completion_tokens: row.get(7)?,
        total_tokens:      row.get(8)?,
        cost_usd:          row.get(9)?,
    })
}
