// Analytics exporter — CSV and JSON export for reports
use std::sync::Arc;
use std::path::Path;
use chrono::NaiveDate;
use crate::errors::{AutoDevError, Result};
use super::db::AnalyticsDb;
use super::types::*;

pub struct AnalyticsExporter {
    db: Arc<AnalyticsDb>,
}

impl AnalyticsExporter {
    pub fn new(db: Arc<AnalyticsDb>) -> Self {
        Self { db }
    }

    /// Export daily costs to CSV
    pub fn export_costs_csv(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        path: &Path,
    ) -> Result<ExportResult> {
        let records = self.db.get_daily_costs_range(start, end)?;

        let mut csv_content = String::from("date,total_cost_usd,total_tokens,session_count,message_count,tool_call_count\n");

        for record in &records {
            csv_content.push_str(&format!(
                "{},{:.4},{},{},{},{}\n",
                record.date,
                record.total_cost_usd,
                record.total_tokens,
                record.session_count,
                record.message_count,
                record.tool_call_count,
            ));
        }

        let file_size = csv_content.len() as i64;
        std::fs::write(path, csv_content)
            .map_err(|e| AutoDevError::Io(e))?;

        Ok(ExportResult {
            format: "csv".to_string(),
            path: path.to_string_lossy().to_string(),
            rows_exported: records.len() as i32,
            file_size_bytes: file_size,
        })
    }

    /// Export daily costs to JSON
    pub fn export_costs_json(
        &self,
        start: NaiveDate,
        end: NaiveDate,
        path: &Path,
    ) -> Result<ExportResult> {
        let records = self.db.get_daily_costs_range(start, end)?;

        let json = serde_json::to_string_pretty(&records)
            .map_err(|e| AutoDevError::Agent(format!("JSON serialization failed: {}", e)))?;

        let file_size = json.len() as i64;
        std::fs::write(path, json)
            .map_err(|e| AutoDevError::Io(e))?;

        Ok(ExportResult {
            format: "json".to_string(),
            path: path.to_string_lossy().to_string(),
            rows_exported: records.len() as i32,
            file_size_bytes: file_size,
        })
    }
}
