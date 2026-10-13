// LLM-based diff classifier — single provider.complete() call.
// Uses the active provider to classify the diff and generate review notes.

use super::parser::ParsedDiff;
use super::risk_scorer::StructuralRisk;
use crate::errors::{ArcanaError, Result};
use crate::provider::usage::StopReason;
use crate::provider::{
    types::{CompletionRequest, MessageContent, RequestMessage},
    Provider,
};
use serde::{Deserialize, Serialize};

const CLASSIFICATION_MAX_OUTPUT_TOKENS: u32 = 4096;
const MAX_SUMMARY_BYTES: usize = 512;
const MAX_REVIEW_NOTES: usize = 5;
const MAX_SUGGESTED_CHECKS: usize = 5;
const MAX_NOTE_BYTES: usize = 256;

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
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffClassification {
    pub classification: ChangeClassification,
    pub summary: String,
    pub review_notes: Vec<String>,
    pub suggested_checks: Vec<String>,
}

impl DiffClassification {
    fn validate(&self) -> Result<()> {
        if self.summary.trim().is_empty() || self.summary.len() > MAX_SUMMARY_BYTES {
            return Err(ArcanaError::Agent(format!(
                "Diff classification summary must contain 1 to {MAX_SUMMARY_BYTES} UTF-8 bytes"
            )));
        }
        validate_items("review_notes", &self.review_notes, MAX_REVIEW_NOTES)?;
        validate_items(
            "suggested_checks",
            &self.suggested_checks,
            MAX_SUGGESTED_CHECKS,
        )?;
        Ok(())
    }
}

fn validate_items(field: &str, items: &[String], max_items: usize) -> Result<()> {
    if items.len() > max_items {
        return Err(ArcanaError::Agent(format!(
            "Diff classification {field} may contain at most {max_items} items"
        )));
    }
    if items
        .iter()
        .any(|item| item.trim().is_empty() || item.len() > MAX_NOTE_BYTES)
    {
        return Err(ArcanaError::Agent(format!(
            "Diff classification {field} items must contain 1 to {MAX_NOTE_BYTES} UTF-8 bytes"
        )));
    }
    Ok(())
}

/// Compact diff summary for the LLM prompt — avoids sending full diff text.
fn build_prompt(diff: &ParsedDiff, structural: &StructuralRisk) -> String {
    let mut prompt = String::new();

    prompt.push_str(&format!(
        "Files changed: {}, Lines added: {}, Lines deleted: {}\n\n",
        diff.file_count, diff.total_lines_added, diff.total_lines_deleted,
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
            prompt.push_str(&format!(
                "  - {} (weight={:.2}): {}\n",
                factor.name, factor.contribution, factor.detail
            ));
        }
    }

    // Sample first 50 added and 50 deleted lines across all hunks
    let mut added_sample: Vec<String> = Vec::new();
    let mut deleted_sample: Vec<String> = Vec::new();

    'outer_add: for file in &diff.files {
        for hunk in &file.hunks {
            for line in &hunk.added_lines {
                if added_sample.len() >= 50 {
                    break 'outer_add;
                }
                added_sample.push(format!("+ {}", line));
            }
        }
    }

    'outer_del: for file in &diff.files {
        for hunk in &file.hunks {
            for line in &hunk.deleted_lines {
                if deleted_sample.len() >= 50 {
                    break 'outer_del;
                }
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

Use a summary of at most 512 UTF-8 bytes, and at most five review notes and five
suggested checks. Each note or check must be non-empty and at most 256 UTF-8
bytes. Keep the response within these limits. Do not include any text outside
the JSON object."#;

    let user_content = format!(
        "Analyze this diff summary and classify the change:\n\n{}",
        build_prompt(diff, structural)
    );

    // P3-25: a one-shot classification with no tool history — nothing to clear.
    let request = CompletionRequest {
        model: model.to_string(),
        messages: vec![RequestMessage {
            role: "user".into(),
            content: vec![MessageContent::Text { text: user_content }],
        }],
        system: Some(system.to_string()),
        tools: vec![],
        // The response contract permits up to 3,072 UTF-8 bytes of field text,
        // plus JSON structure and escaping. 4,096 output tokens leaves room for
        // that complete contract while keeping provider output bounded.
        max_tokens: CLASSIFICATION_MAX_OUTPUT_TOKENS,
        temperature: Some(0.1),
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        // One-shot classification over a diff that differs every time.
        // Nothing to reuse, so nothing to cache.
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    };

    let response = provider.complete(request).await?;

    if StopReason::parse(provider.id(), &response.stop_reason).is_truncated() {
        return Err(ArcanaError::Agent(
            "Diff classification response was truncated by the provider".to_string(),
        ));
    }

    // Extract text content from the response
    let text = response
        .content
        .iter()
        .find_map(|c| {
            if let MessageContent::Text { text } = c {
                Some(text.as_str())
            } else {
                None
            }
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

    let classification = serde_json::from_str::<DiffClassification>(json_str)
        .map_err(|e| ArcanaError::Agent(format!("Failed to parse classification JSON: {e}")))?;
    classification.validate()?;
    Ok(classification)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{
        traits::StreamResult,
        types::{CompletionResponse, ModelInfo, ProviderStatus, StreamChunk},
        usage::Usage,
    };
    use async_trait::async_trait;
    use std::sync::Mutex;

    struct FixedResponseProvider {
        text: String,
        stop_reason: String,
        request: Mutex<Option<CompletionRequest>>,
    }

    impl FixedResponseProvider {
        fn new(text: impl Into<String>, stop_reason: impl Into<String>) -> Self {
            Self {
                text: text.into(),
                stop_reason: stop_reason.into(),
                request: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl Provider for FixedResponseProvider {
        fn id(&self) -> &str {
            "test"
        }

        fn name(&self) -> &str {
            "Fixed response test provider"
        }

        fn models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }

        async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
            *self.request.lock().expect("request mutex") = Some(request);
            Ok(CompletionResponse {
                message_id: "test-message".to_string(),
                content: vec![MessageContent::Text {
                    text: self.text.clone(),
                }],
                stop_reason: self.stop_reason.clone(),
                usage: Usage::default(),
                model: "test-model".to_string(),
            })
        }

        async fn stream(&self, _request: CompletionRequest) -> Result<StreamResult> {
            Ok(Box::pin(futures::stream::empty::<Result<StreamChunk>>()))
        }

        async fn test_connection(&self) -> Result<ProviderStatus> {
            Ok(ProviderStatus::Connected)
        }
    }

    fn fixture_diff() -> ParsedDiff {
        ParsedDiff {
            files: Vec::new(),
            total_lines_added: 0,
            total_lines_deleted: 0,
            file_count: 0,
        }
    }

    fn fixture_risk() -> StructuralRisk {
        StructuralRisk {
            score: 0.0,
            factors: Vec::new(),
        }
    }

    #[tokio::test]
    async fn sends_bounded_output_budget_and_accepts_contract_maxima() {
        let notes = vec!["n".repeat(MAX_NOTE_BYTES); MAX_REVIEW_NOTES];
        let checks = vec!["c".repeat(MAX_NOTE_BYTES); MAX_SUGGESTED_CHECKS];
        let response = serde_json::json!({
            "classification": "Feature",
            "summary": "s".repeat(MAX_SUMMARY_BYTES),
            "review_notes": notes,
            "suggested_checks": checks,
        })
        .to_string();
        let provider = FixedResponseProvider::new(response, "stop");

        let result = classify_diff(&provider, &fixture_diff(), &fixture_risk(), "test-model")
            .await
            .expect("contract maximum should fit the output budget");

        assert_eq!(result.summary.len(), MAX_SUMMARY_BYTES);
        assert_eq!(result.review_notes.len(), MAX_REVIEW_NOTES);
        assert_eq!(result.suggested_checks.len(), MAX_SUGGESTED_CHECKS);
        assert_eq!(
            provider
                .request
                .lock()
                .expect("request mutex")
                .as_ref()
                .expect("captured request")
                .max_tokens,
            CLASSIFICATION_MAX_OUTPUT_TOKENS
        );
    }

    #[tokio::test]
    async fn rejects_provider_truncation_before_parsing_response() {
        let provider = FixedResponseProvider::new("{}", "MAX_TOKENS");

        let error = classify_diff(&provider, &fixture_diff(), &fixture_risk(), "test-model")
            .await
            .expect_err("truncated completion cannot satisfy the output contract");

        assert!(error.to_string().contains("response was truncated"));
    }

    #[tokio::test]
    async fn rejects_over_limit_fields_without_echoing_provider_text() {
        let private_text = "private-provider-output";
        let response = serde_json::json!({
            "classification": "Feature",
            "summary": "ok",
            "review_notes": vec![private_text; MAX_REVIEW_NOTES + 1],
            "suggested_checks": [],
        })
        .to_string();
        let provider = FixedResponseProvider::new(response, "stop");

        let error = classify_diff(&provider, &fixture_diff(), &fixture_risk(), "test-model")
            .await
            .expect_err("too many items violate the output contract");

        let message = error.to_string();
        assert!(message.contains("at most 5 items"));
        assert!(!message.contains(private_text));
    }
}
