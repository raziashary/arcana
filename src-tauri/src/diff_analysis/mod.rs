// Diff Analysis module — Feature 3 of Ruflo improvements.
// Provides structural risk scoring and LLM-based classification of git diffs.
// Used by the QC Agent via the analyze_diff MCP tool.

pub mod parser;
pub mod risk_scorer;
pub mod classifier;

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::errors::Result;
use crate::provider::Provider;
use classifier::ChangeClassification;
use risk_scorer::is_critical_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffAnalysisResult {
    pub risk_score:       f32,
    pub risk_level:       String,   // "low" | "medium" | "high" | "critical"
    pub classification:   ChangeClassification,
    pub summary:          String,
    pub review_notes:     Vec<String>,
    pub suggested_checks: Vec<String>,
    pub file_count:       usize,
    pub lines_added:      usize,
    pub lines_deleted:    usize,
    pub critical_files:   Vec<String>,
    pub analyzed_at:      String,
}

pub struct DiffAnalyzer {
    provider: Arc<dyn Provider>,
    model:    String,
}

impl DiffAnalyzer {
    pub fn new(provider: Arc<dyn Provider>, model: String) -> Self {
        Self { provider, model }
    }

    pub async fn analyze(&self, diff_text: &str) -> Result<DiffAnalysisResult> {
        let parsed = parser::parse_unified_diff(diff_text)?;
        let structural = risk_scorer::compute_structural_risk(&parsed);
        let classification = classifier::classify_diff(
            &*self.provider,
            &parsed,
            &structural,
            &self.model,
        ).await?;

        let risk_level = match structural.score {
            s if s >= 0.75 => "critical",
            s if s >= 0.50 => "high",
            s if s >= 0.25 => "medium",
            _              => "low",
        }.to_string();

        let critical_files: Vec<String> = parsed.files.iter()
            .filter(|f| is_critical_path(&f.path))
            .map(|f| f.path.clone())
            .collect();

        Ok(DiffAnalysisResult {
            risk_score:       structural.score,
            risk_level,
            classification:   classification.classification,
            summary:          classification.summary,
            review_notes:     classification.review_notes,
            suggested_checks: classification.suggested_checks,
            file_count:       parsed.file_count,
            lines_added:      parsed.total_lines_added,
            lines_deleted:    parsed.total_lines_deleted,
            critical_files,
            analyzed_at:      chrono::Utc::now().to_rfc3339(),
        })
    }
}
