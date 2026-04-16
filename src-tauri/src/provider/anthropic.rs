// Anthropic provider — direct API calls to api.anthropic.com.
// PA mode only. Worker mode delegates to core-orchestrator's litellm.
// Implements the Provider trait with streaming support.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use crate::errors::{AutoDevError, Result};
use super::traits::{Provider, StreamResult};
use super::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo,
    ProviderStatus, StreamChunk,
};
use super::streaming::parse_sse_line;

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION:   &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    client:  Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    fn models_static() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id:                "claude-opus-4-5".into(),
                name:              "Claude Opus 4.5".into(),
                context_window:    200_000,
                supports_vision:   true,
                supports_thinking: true,
                cost_in_per_1m:    15.0,
                cost_out_per_1m:   75.0,
            },
            ModelInfo {
                id:                "claude-sonnet-4-5".into(),
                name:              "Claude Sonnet 4.5".into(),
                context_window:    200_000,
                supports_vision:   true,
                supports_thinking: true,
                cost_in_per_1m:    3.0,
                cost_out_per_1m:   15.0,
            },
            ModelInfo {
                id:                "claude-haiku-4-5".into(),
                name:              "Claude Haiku 4.5".into(),
                context_window:    200_000,
                supports_vision:   true,
                supports_thinking: false,
                cost_in_per_1m:    0.25,
                cost_out_per_1m:   1.25,
            },
        ]
    }

    fn build_body(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = req.messages.iter().map(|m| {
            let content: Vec<serde_json::Value> = m.content.iter().map(|c| match c {
                MessageContent::Text { text } => json!({ "type": "text", "text": text }),
                MessageContent::ToolUse { id, name, input } => json!({
                    "type": "tool_use",
                    "id":   id,
                    "name": name,
                    "input": input,
                }),
                MessageContent::ToolResult { tool_use_id, content, is_error } => json!({
                    "type":        "tool_result",
                    "tool_use_id": tool_use_id,
                    "content":     content,
                    "is_error":    is_error,
                }),
            }).collect();
            json!({ "role": m.role, "content": content })
        }).collect();

        let tools: Vec<serde_json::Value> = req.tools.iter().map(|t| json!({
            "name":         t.name,
            "description":  t.description,
            "input_schema": t.input_schema,
        })).collect();

        let mut body = json!({
            "model":       req.model,
            "max_tokens":  req.max_tokens,
            "messages":    messages,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(sys) = &req.system {
            body["system"] = json!(sys);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
        // Extended thinking (Anthropic): inject thinking block before tools
        if let Some(budget) = req.thinking_budget_tokens {
            body["thinking"] = json!({ "type": "enabled", "budget_tokens": budget });
            // Anthropic requires temperature=1 when thinking is enabled
            body["temperature"] = json!(1.0);
        }
        if stream {
            body["stream"] = json!(true);
        }
        body
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self)   -> &str { "anthropic" }
    fn name(&self) -> &str { "Anthropic" }
    fn models(&self) -> Vec<ModelInfo> { Self::models_static() }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request, false);
        debug!("Anthropic complete: model={}", request.model);

        let resp = self.client
            .post(format!("{ANTHROPIC_API_BASE}/messages"))
            .header("x-api-key",         &self.api_key)
            .header("anthropic-version",  ANTHROPIC_VERSION)
            .header("content-type",       "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!("Anthropic {status}: {text}")));
        }

        let data: serde_json::Value = resp.json().await?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request, true);
        debug!("Anthropic stream: model={}", request.model);

        let resp = self.client
            .post(format!("{ANTHROPIC_API_BASE}/messages"))
            .header("x-api-key",         &self.api_key)
            .header("anthropic-version",  ANTHROPIC_VERSION)
            .header("content-type",       "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!("Anthropic stream {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries so SSE
        // data lines are never parsed mid-line (fixes "EOF while parsing" crash
        // that occurs when a long tool response is split across two HTTP chunks).
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes = bytes_result.map_err(AutoDevError::Http)?;
                let text  = String::from_utf8_lossy(&bytes);
                let mut buf = buffer.lock().unwrap();
                buf.push_str(&text);
                let mut chunks = Vec::new();
                // Only parse complete lines — leave any trailing partial line in the buffer.
                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim_end_matches('\r').to_string();
                    *buf = buf[pos + 1..].to_string();
                    if let Some(chunk) = parse_sse_line(&line)? {
                        chunks.push(chunk);
                    }
                }
                Ok(chunks)
            })
            .flat_map(|result| {
                let items: Vec<Result<StreamChunk>> = match result {
                    Ok(chunks) => chunks.into_iter().map(Ok).collect(),
                    Err(e)     => vec![Err(e)],
                };
                futures::stream::iter(items)
            });

        Ok(Box::pin(chunk_stream))
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        // Use models endpoint — minimal cost, validates the key
        let resp = self.client
            .get(format!("{ANTHROPIC_API_BASE}/models"))
            .header("x-api-key",         &self.api_key)
            .header("anthropic-version",  ANTHROPIC_VERSION)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderStatus::Connected),
            Ok(r) if r.status() == 401       => Ok(ProviderStatus::Error("Invalid API key".into())),
            Ok(r) => {
                let code = r.status();
                Ok(ProviderStatus::Error(format!("HTTP {code}")))
            }
            Err(e) => {
                error!("Anthropic connection test failed: {e}");
                Ok(ProviderStatus::Error(e.to_string()))
            }
        }
    }
}

fn parse_completion_response(data: serde_json::Value) -> Result<CompletionResponse> {
    let message_id  = data["id"].as_str().unwrap_or("").to_string();
    let stop_reason = data["stop_reason"].as_str().unwrap_or("end_turn").to_string();
    let model       = data["model"].as_str().unwrap_or("").to_string();
    let tokens_in   = data["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
    let tokens_out  = data["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

    let content = data["content"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|block| {
            match block["type"].as_str()? {
                "text" => Some(MessageContent::Text {
                    text: block["text"].as_str().unwrap_or("").to_string(),
                }),
                "tool_use" => Some(MessageContent::ToolUse {
                    id:    block["id"].as_str().unwrap_or("").to_string(),
                    name:  block["name"].as_str().unwrap_or("").to_string(),
                    input: block["input"].clone(),
                }),
                _ => None,
            }
        })
        .collect();

    Ok(CompletionResponse { message_id, content, stop_reason, tokens_in, tokens_out, model })
}
