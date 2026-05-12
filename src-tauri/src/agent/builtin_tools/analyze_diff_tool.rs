// analyze_diff — MCP tool for the QC Agent.
// Generates and analyzes the diff between two branches, returning structured
// risk score, classification, and review notes to guide the QC decision.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::diff_analysis::DiffAnalyzer;
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::google::GoogleProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::traits::Provider;
use crate::vault::keychain::{VaultManager, provider_key};
use super::super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct AnalyzeDiffTool;

#[async_trait]
impl Tool for AnalyzeDiffTool {
    fn name(&self) -> &str { "analyze_diff" }

    fn description(&self) -> &str {
        "Analyze a git diff between two branches and get a structured risk score, change \
        classification, and review notes to guide the QC review decision. Returns risk_level \
        ('low'|'medium'|'high'|'critical'), classification, summary, review_notes, and \
        suggested_checks."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "repo_path": {
                    "type": "string",
                    "description": "Absolute path to the git repository"
                },
                "base_branch": {
                    "type": "string",
                    "description": "Base branch to diff from (e.g. 'main')"
                },
                "head_branch": {
                    "type": "string",
                    "description": "Head branch to diff to (e.g. 'feature/my-pr')"
                }
            },
            "required": ["repo_path", "base_branch", "head_branch"],
            "additionalProperties": false
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> ToolResult {
        let repo_path = match input["repo_path"].as_str() {
            Some(p) if !p.is_empty() => p,
            _ => return ToolResult::error("'repo_path' is required and must be a non-empty string"),
        };
        let base_branch = match input["base_branch"].as_str() {
            Some(b) if !b.is_empty() => b,
            _ => return ToolResult::error("'base_branch' is required and must be a non-empty string"),
        };
        let head_branch = match input["head_branch"].as_str() {
            Some(h) if !h.is_empty() => h,
            _ => return ToolResult::error("'head_branch' is required and must be a non-empty string"),
        };

        // Build analyzer using active provider from vault
        let analyzer = match build_analyzer_from_vault() {
            Ok(a) => a,
            Err(e) => return ToolResult::error(format!("Failed to initialize analyzer: {e}")),
        };

        // Generate branch diff
        let patch = match crate::commands::diff_commands::generate_branch_diff_pub(
            repo_path,
            base_branch,
            head_branch,
        ) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Failed to generate diff: {e}")),
        };

        match analyzer.analyze(&patch).await {
            Ok(result) => {
                let summary = format!(
                    "Risk: {} ({:.2}) | Classification: {:?} | Files: {} | +{} -{}",
                    result.risk_level,
                    result.risk_score,
                    result.classification,
                    result.file_count,
                    result.lines_added,
                    result.lines_deleted,
                );
                match serde_json::to_string(&result) {
                    Ok(json) => ToolResult::success_with_hint(json, summary),
                    Err(e) => ToolResult::error(format!("Failed to serialize result: {e}")),
                }
            }
            Err(e) => ToolResult::error(format!("Diff analysis failed: {e}")),
        }
    }
}

fn build_analyzer_from_vault() -> crate::errors::Result<DiffAnalyzer> {
    use crate::errors::AutoDevError;

    // Read active provider settings from a simple config file (same pattern as sidecar)
    // Fallback: use vault to check which provider has a key
    let provider_id = std::env::var("AUTODEV_ACTIVE_PROVIDER")
        .unwrap_or_else(|_| "anthropic".to_string());
    let model = std::env::var("AUTODEV_ACTIVE_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-6".to_string());

    let api_key = if provider_id != "ollama" {
        VaultManager::get(&provider_key(&provider_id))?
            .ok_or_else(|| AutoDevError::Agent(format!("No API key for provider '{provider_id}'")))?
    } else {
        String::new()
    };

    let provider: Arc<dyn Provider> = match provider_id.as_str() {
        "anthropic" => Arc::new(AnthropicProvider::new(api_key)),
        "openai"    => Arc::new(OpenAIProvider::new(api_key)),
        "google"    => Arc::new(GoogleProvider::new(api_key)),
        "ollama"    => Arc::new(OllamaProvider::default_from_env()),
        other       => return Err(AutoDevError::Agent(format!("Unknown provider: {other}"))),
    };

    Ok(DiffAnalyzer::new(provider, model))
}
