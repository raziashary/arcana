// LLM-based diff classifier — single provider.complete() call.
// Uses the active provider to classify the diff and generate review notes.

use serde::{Deserialize, Serialize};
use crate::errors::{AutoDevError, Result};
use crate::provider::{Provider, types::{CompletionRequest, MessageContent, RequestMessage}};
use super::parser::ParsedDiff;
use super::risk_scorer::StructuralRisk;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChangeClassification {
    Feature,
    Bugfix,
    Refactor,
    Documentation,
    Test,
    Config,
    Security,
    Performance,
    Unknown,
}

impl Default for ChangeClassification {
    fn default() -> Self { Self::Unknown }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffClassification {
    pub classification:  ChangeClassification,
    pub summary:         String,
    pub review_notes:    Vec<String>,
    pub suggested_checks: Vec<String>,
}

/// Compact diff summary for the LLM prompt — avoids sending full diff text.
fn build_prompt(diff: &ParsedDiff, structural: &StructuralRisk) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "Files changed: {}, Lines added: {}, Lines deleted: {}\n\n",
        diff.file_count,
        diff.total_lines_added,
        diff.total_lines_deleted,
    ));

    prompt.push_str("Changed files:\n");
    for f in &diff.files {
        prompt.push_str(&format!(
            "  {} (+{} -{}){}{}{}\n",
            f.path,
            f.lines_added,
            f.lines_deleted,
            if f.is_new_file { " [new]" } else { "" },
            if f.is_deleted_file { " [deleted]" } else { "" },
            if f.is_renamed { " [renamed]" } else { "" },
        ));
    }

    if !structural.factors.is_empty() {
        prompt.push_str("\nRisk factors:\n");
        for factor in &structural.factors {
            prompt.push_str(&format!("  - {} (weight={:.2}): {}\n", factor.name, factor.contribution, factor.detail));
        }
    }

    // Sample first 50 added and 50 deleted lines across all hunks
    let mut added_sample: Vec<String> = Vec::new();
    let mut deleted_sample: Vec<String> = Vec::new();

    'outer_add: for file in &diff.files {
        for hunk in &file.hunks {
            for line in &hunk.added_lines {
                if added_sample.len() >= 50 { break 'outer_add; }
                added_sample.push(format!("+ {}", line));
            }
        }
    }

    'outer_del: for file in &diff.files {
        for hunk in &file.hunks {
            for line in &hunk.deleted_lines {
                if deleted_sample.len() >= 50 { break 'outer_del; }
                deleted_sample.push(format!("- {}", line));
            }
        }
    }

    if !added_sample.is_empty() {
        prompt.push_str("\nSample added lines:\n");
        for l in &added_sample {
            prompt.push_str(&format!("  {}\n", l));
        }
    }

    if !deleted_sample.is_empty() {
        prompt.push_str("\nSample deleted lines:\n");
        for l in &deleted_sample {
            prompt.push_str(&format!("  {}\n", l));
        }
    }

    prompt
}

pub async fn classify_diff(
    provider: &dyn Provider,
    diff: &ParsedDiff,
    structural: &StructuralRisk,
    model: &str,
) -> Result<DiffClassification> {
    let system = r#"You are a code review assistant. Analyze the provided diff summary and return a JSON object.

Return ONLY valid JSON with this exact structure:
{
  "classification": "Feature" | "Bugfix" | "Refactor" | "Documentation" | "Test" | "Config" | "Security" | "Performance" | "Unknown",
  "summary": "1-2 sentence human-readable description of what this diff does",
  "review_notes": ["specific thing to check 1", "specific thing to check 2"],
  "suggested_checks": ["Run integration tests", "Verify auth middleware", ...]
}

Do not include any text outside the JSON object."#;

    let user_content = format!(
        "Analyze this diff summary and classify the change:\n\n{}",
        build_prompt(diff, structural)
    );

    let request = CompletionRequest {
        model:                  model.to_string(),
        messages:               vec![RequestMessage {
            role:    "user".into(),
            content: vec![MessageContent::Text { text: user_content }],
        }],
        system:                 Some(system.to_string()),
        tools:                  vec![],
        max_tokens:             512,
        temperature:            Some(0.1),
        top_p:                  None,
        reasoning_effort:       None,
        thinking_budget_tokens: None,
    };

    let response = provider.complete(request).await?;

    // Extract text content from the response
    let text = response.content.iter()
        .find_map(|c| {
            if let MessageContent::Text { text } = c { Some(text.as_str()) } else { None }
        })
        .unwrap_or("");

    // Parse JSON — strip any markdown code fences if present
    let json_str = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    serde_json::from_str::<DiffClassification>(json_str)
        .map_err(|e| AutoDevError::Agent(format!("Failed to parse classification JSON: {e}\nRaw: {json_str}")))
}
