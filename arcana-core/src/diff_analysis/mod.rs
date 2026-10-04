// Diff Analysis module — Feature 3 of Ruflo improvements.
// Provides structural risk scoring and LLM-based classification of git diffs.
// Used by the QC Agent via the analyze_diff MCP tool.

pub mod classifier;
pub mod parser;
pub mod risk_scorer;

use crate::errors::Result;
use crate::provider::Provider;
use classifier::ChangeClassification;
use risk_scorer::is_critical_path;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffAnalysisResult {
    pub risk_score: f32,
    pub risk_level: String, // "low" | "medium" | "high" | "critical"
    pub classification: ChangeClassification,
    pub summary: String,
    pub review_notes: Vec<String>,
    pub suggested_checks: Vec<String>,
    pub file_count: usize,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub critical_files: Vec<String>,
    pub analyzed_at: String,
    /// `deterministic_only` means the patch was scored locally because
    /// workspace/destination consent was absent or revoked.
    pub analysis_mode: String,
}

pub struct DiffAnalyzer {
    provider: Arc<dyn Provider>,
    model: String,
}

impl DiffAnalyzer {
    pub fn new(provider: Arc<dyn Provider>, model: String) -> Self {
        Self { provider, model }
    }

    pub async fn analyze(&self, diff_text: &str) -> Result<DiffAnalysisResult> {
        let parsed = parser::parse_unified_diff(diff_text)?;
        let structural = risk_scorer::compute_structural_risk(&parsed);
        let classification =
            classifier::classify_diff(&*self.provider, &parsed, &structural, &self.model).await?;

        let risk_level = match structural.score {
            s if s >= 0.75 => "critical",
            s if s >= 0.50 => "high",
            s if s >= 0.25 => "medium",
            _ => "low",
        }
        .to_string();

        let critical_files: Vec<String> = parsed
            .files
            .iter()
            .filter(|f| is_critical_path(&f.path))
            .map(|f| f.path.clone())
            .collect();

        Ok(DiffAnalysisResult {
            risk_score: structural.score,
            risk_level,
            classification: classification.classification,
            summary: classification.summary,
            review_notes: classification.review_notes,
            suggested_checks: classification.suggested_checks,
            file_count: parsed.file_count,
            lines_added: parsed.total_lines_added,
            lines_deleted: parsed.total_lines_deleted,
            critical_files,
            analyzed_at: crate::db::timestamps::now_rfc3339(),
            analysis_mode: "llm_assisted".to_string(),
        })
    }

    /// Score diff structure locally without sending its contents to a provider.
    pub fn analyze_structural(diff_text: &str) -> Result<DiffAnalysisResult> {
        let parsed = parser::parse_unified_diff(diff_text)?;
        let structural = risk_scorer::compute_structural_risk(&parsed);
        let risk_level = match structural.score {
            s if s >= 0.75 => "critical",
            s if s >= 0.50 => "high",
            s if s >= 0.25 => "medium",
            _ => "low",
        }
        .to_string();
        let critical_files: Vec<String> = parsed
            .files
            .iter()
            .filter(|file| is_critical_path(&file.path))
            .map(|file| file.path.clone())
            .collect();

        Ok(DiffAnalysisResult {
            risk_score: structural.score,
            risk_level,
            classification: ChangeClassification::Unknown,
            summary: "Deterministic structural score only; diff contents were not sent because workspace egress consent is absent or revoked.".to_string(),
            review_notes: vec![
                "LLM diff classification was skipped because no matching workspace and destination consent is active.".to_string(),
            ],
            suggested_checks: Vec::new(),
            file_count: parsed.file_count,
            lines_added: parsed.total_lines_added,
            lines_deleted: parsed.total_lines_deleted,
            critical_files,
            analyzed_at: crate::db::timestamps::now_rfc3339(),
            analysis_mode: "deterministic_only".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_fallback_returns_risk_without_provider_access() {
        let patch = "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1,2 @@\n-old\n+new\n+added\n";
        let result = DiffAnalyzer::analyze_structural(patch).expect("local score");

        assert_eq!(result.analysis_mode, "deterministic_only");
        assert!(result.summary.contains("were not sent"));
        assert!(matches!(
            result.classification,
            ChangeClassification::Unknown
        ));
        assert_eq!(result.file_count, 1);
        assert_eq!(result.lines_added, 2);
        assert_eq!(result.lines_deleted, 1);
    }
}
