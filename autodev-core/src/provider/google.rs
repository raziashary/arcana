// Google Gemini provider — direct API calls to generativelanguage.googleapis.com.
// PA mode only. Worker mode delegates to core-orchestrator's litellm.
// Implements the Provider trait with streaming support.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use super::traits::{Provider, StreamResult};
use super::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus, StreamChunk,
};
use crate::errors::{AutoDevError, Result};

const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com";

pub struct GoogleProvider {
    api_key: String,
    client: Client,
}

impl GoogleProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    fn models_static() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id: "gemini-2.0-flash-thinking-exp-01-21".into(),
                name: "Gemini 2.0 Flash Thinking".into(),
                context_window: 1_000_000,
                supports_vision: true,
                supports_thinking: true,
                cost_in_per_1m: 0.10,
                cost_out_per_1m: 0.40,
            },
            ModelInfo {
                id: "gemini-2.5-pro".into(),
                name: "Gemini 2.5 Pro".into(),
                context_window: 1_000_000,
                supports_vision: true,
                supports_thinking: true,
                cost_in_per_1m: 1.25,
                cost_out_per_1m: 5.0,
            },
            ModelInfo {
                id: "gemini-2.5-flash".into(),
                name: "Gemini 2.5 Flash".into(),
                context_window: 1_000_000,
                supports_vision: true,
                supports_thinking: false,
                cost_in_per_1m: 0.075,
                cost_out_per_1m: 0.30,
            },
        ]
    }

    fn build_body(&self, req: &CompletionRequest) -> serde_json::Value {
        // Convert messages to Gemini format
        let mut contents = Vec::new();
        let mut system_instruction = None;

        for msg in &req.messages {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                _ => "user",
            };

            let parts: Vec<serde_json::Value> = msg
                .content
                .iter()
                .map(|c| match c {
                    MessageContent::Text { text } => json!({ "text": text }),
                    MessageContent::ToolUse { id: _, name, input } => json!({
                        "functionCall": {
                            "name": name,
                            "args": input,
                        }
                    }),
                    MessageContent::ToolResult {
                        tool_use_id,
                        content,
                        is_error: _,
                    } => json!({
                        "functionResponse": {
                            "name": tool_use_id,
                            "response": { "result": content }
                        }
                    }),
                })
                .collect();

            contents.push(json!({
                "role": role,
                "parts": parts,
            }));
        }

        // Extract system prompt
        if let Some(sys) = &req.system {
            system_instruction = Some(json!({
                "parts": [{ "text": sys }]
            }));
        }

        // Convert tools to Gemini format
        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "functionDeclarations": [{
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }]
                })
            })
            .collect();

        let mut body = json!({
            "contents": contents,
        });

        if let Some(sys_instr) = system_instruction {
            body["systemInstruction"] = sys_instr;
        }

        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }

        // generationConfig: temperature, topP, thinkingConfig
        let mut gen_config = serde_json::Map::new();
        if let Some(temp) = req.temperature {
            gen_config.insert("temperature".into(), json!(temp));
        }
        if let Some(top_p) = req.top_p {
            gen_config.insert("topP".into(), json!(top_p));
        }
        if req.thinking_budget_tokens.is_some() {
            gen_config.insert(
                "thinkingConfig".into(),
                json!({ "thinkingBudget": req.thinking_budget_tokens }),
            );
        }
        if !gen_config.is_empty() {
            body["generationConfig"] = serde_json::Value::Object(gen_config);
        }

        body
    }
}

#[async_trait]
impl Provider for GoogleProvider {
    fn id(&self) -> &str {
        "google"
    }
    fn name(&self) -> &str {
        "Google"
    }
    fn models(&self) -> Vec<ModelInfo> {
        Self::models_static()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request);
        debug!("Google complete: model={}", request.model);

        // ⚠ The key goes in a HEADER, never the URL. See `auth_header`.
        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            GOOGLE_API_BASE, request.model
        );

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!("Google {status}: {text}")));
        }

        let data: serde_json::Value = resp.json().await?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request);
        debug!("Google stream: model={}", request.model);

        // ⚠ The key goes in a HEADER, never the URL. See `auth_header`.
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            GOOGLE_API_BASE, request.model
        );

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!(
                "Google stream {status}: {text}"
            )));
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes = bytes_result.map_err(AutoDevError::Http)?;
                let text = String::from_utf8_lossy(&bytes);
                let mut buf = buffer.lock().unwrap();
                buf.push_str(&text);
                let mut chunks = Vec::new();
                // Only parse complete lines
                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim_end_matches('\r').to_string();
                    *buf = buf[pos + 1..].to_string();
                    if let Some(chunk) = parse_google_sse_line(&line)? {
                        chunks.push(chunk);
                    }
                }
                Ok(chunks)
            })
            .flat_map(|result| {
                let items: Vec<Result<StreamChunk>> = match result {
                    Ok(chunks) => chunks.into_iter().map(Ok).collect(),
                    Err(e) => vec![Err(e)],
                };
                futures::stream::iter(items)
            });

        Ok(Box::pin(chunk_stream))
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        // Use a minimal generateContent call to test
        // ⚠ The key goes in a HEADER, never the URL. See `auth_header`.
        let url = format!(
            "{}/v1beta/models/gemini-2.5-flash:generateContent",
            GOOGLE_API_BASE
        );

        let test_body = json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": "Hi" }]
            }]
        });

        let resp = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&test_body)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderStatus::Connected),
            Ok(r) if r.status() == 403 => Ok(ProviderStatus::Error("Invalid API key".into())),
            Ok(r) => {
                let code = r.status();
                Ok(ProviderStatus::Error(format!("HTTP {code}")))
            }
            Err(e) => {
                // Redacted before it is logged OR returned. `reqwest`'s Error
                // Display includes the request URL, and this value crosses the
                // Tauri IPC boundary into the renderer (`Settings.tsx`). Even
                // with the key now in a header rather than the URL, an error
                // string that reaches the UI is not a place to trust a
                // transport library's formatting.
                let safe = crate::security::redaction::redact_text(&e.to_string());
                error!("Google connection test failed: {safe}");
                Ok(ProviderStatus::Error(safe))
            }
        }
    }
}

fn parse_google_sse_line(line: &str) -> Result<Option<StreamChunk>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(None);
    }
    let data = match line.strip_prefix("data: ") {
        Some(d) => d,
        None => return Ok(None),
    };
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => parse_google_chunk(&v),
        Err(e) => Err(AutoDevError::Provider(format!(
            "SSE parse error: {e} — data: {data}"
        ))),
    }
}

fn parse_google_chunk(v: &serde_json::Value) -> Result<Option<StreamChunk>> {
    let candidates = match v["candidates"].as_array() {
        Some(c) => c,
        None => return Ok(None),
    };
    if candidates.is_empty() {
        return Ok(None);
    }

    let content = &candidates[0]["content"];
    let parts = match content["parts"].as_array() {
        Some(p) => p,
        None => return Ok(None),
    };

    for part in parts {
        // Text delta
        if let Some(text) = part["text"].as_str() {
            return Ok(Some(StreamChunk::TextDelta {
                text: text.to_string(),
            }));
        }

        // Function call (tool use)
        if let Some(func_call) = part["functionCall"].as_object() {
            let name = func_call
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let _args = func_call.get("args").cloned().unwrap_or(json!({}));

            // Gemini sends complete function calls, not deltas
            return Ok(Some(StreamChunk::ToolUseStart {
                id: format!("gemini-{}", name),
                name,
                block_index: 0,
            }));
        }
    }

    // Check for finish reason
    if let Some(finish_reason) = candidates[0]["finishReason"].as_str() {
        // Extract usage metadata if present
        let usage_metadata = &v["usageMetadata"];
        let tokens_in = usage_metadata["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let tokens_out = usage_metadata["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;

        if tokens_in > 0 || tokens_out > 0 {
            return Ok(Some(StreamChunk::MessageDelta {
                stop_reason: finish_reason.to_string(),
                tokens_in,
                tokens_out,
            }));
        }
    }

    Ok(None)
}

fn parse_completion_response(data: serde_json::Value) -> Result<CompletionResponse> {
    let empty_array = vec![];
    let candidates = data["candidates"].as_array().unwrap_or(&empty_array);
    let stop_reason = candidates
        .get(0)
        .and_then(|c| c["finishReason"].as_str())
        .unwrap_or("STOP")
        .to_string();

    let usage_metadata = &data["usageMetadata"];
    let tokens_in = usage_metadata["promptTokenCount"].as_u64().unwrap_or(0) as u32;
    let tokens_out = usage_metadata["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;

    let mut content = Vec::new();
    if let Some(candidate) = candidates.get(0) {
        if let Some(parts) = candidate["content"]["parts"].as_array() {
            for part in parts {
                // Text content
                if let Some(text) = part["text"].as_str() {
                    content.push(MessageContent::Text {
                        text: text.to_string(),
                    });
                }

                // Function call
                if let Some(func_call) = part["functionCall"].as_object() {
                    let name = func_call
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let args = func_call.get("args").cloned().unwrap_or(json!({}));

                    content.push(MessageContent::ToolUse {
                        id: format!("gemini-{}", name),
                        name,
                        input: args,
                    });
                }
            }
        }
    }

    Ok(CompletionResponse {
        message_id: "".to_string(),
        content,
        stop_reason,
        tokens_in,
        tokens_out,
        model: "gemini".to_string(),
    })
}
