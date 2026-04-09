// OpenAI provider — direct API calls to api.openai.com.
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

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

pub struct OpenAIProvider {
    api_key: String,
    client:  Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    fn models_static() -> Vec<ModelInfo> {
        vec![
            ModelInfo {
                id:              "gpt-4.1".into(),
                name:            "GPT-4.1".into(),
                context_window:  1_000_000,
                supports_vision: true,
                cost_in_per_1m:  5.0,   // Placeholder — update with actual pricing
                cost_out_per_1m: 15.0,
            },
            ModelInfo {
                id:              "gpt-4.1-mini".into(),
                name:            "GPT-4.1 Mini".into(),
                context_window:  1_000_000,
                supports_vision: true,
                cost_in_per_1m:  1.0,
                cost_out_per_1m: 3.0,
            },
            ModelInfo {
                id:              "gpt-4o".into(),
                name:            "GPT-4o".into(),
                context_window:  128_000,
                supports_vision: true,
                cost_in_per_1m:  2.50,
                cost_out_per_1m: 10.0,
            },
        ]
    }

    fn build_body(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        let mut messages: Vec<serde_json::Value> = Vec::new();

        // Add system prompt as first message if present
        if let Some(sys) = &req.system {
            messages.push(json!({
                "role": "system",
                "content": sys
            }));
        }

        // Convert messages - OpenAI format differs from Anthropic
        for m in &req.messages {
            if m.role == "user" {
                // User messages with tool results become "tool" role messages in OpenAI
                let tool_results: Vec<&MessageContent> = m.content.iter()
                    .filter(|c| matches!(c, MessageContent::ToolResult { .. }))
                    .collect();

                if !tool_results.is_empty() {
                    // Add tool result messages (role: "tool")
                    for tr in tool_results {
                        if let MessageContent::ToolResult { tool_use_id, content, is_error: _ } = tr {
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                    }
                } else {
                    // Regular user text message
                    let text = m.content.iter()
                        .filter_map(|c| match c {
                            MessageContent::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    
                    messages.push(json!({
                        "role": "user",
                        "content": text,
                    }));
                }
            } else if m.role == "assistant" {
                // Assistant messages: separate text and tool_calls
                let text_parts: Vec<String> = m.content.iter()
                    .filter_map(|c| match c {
                        MessageContent::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();

                let tool_calls: Vec<serde_json::Value> = m.content.iter()
                    .enumerate()
                    .filter_map(|(_idx, c)| match c {
                        MessageContent::ToolUse { id, name, input } => Some(json!({
                            "id": id,
                            "type": "function",
                            "function": {
                                "name": name,
                                "arguments": serde_json::to_string(input).unwrap_or_else(|_| "{}".to_string()),
                            }
                        })),
                        _ => None,
                    })
                    .collect();

                let mut msg = json!({
                    "role": "assistant",
                    "content": if text_parts.is_empty() { json!(null) } else { json!(text_parts.join("\n")) },
                });

                if !tool_calls.is_empty() {
                    msg["tool_calls"] = json!(tool_calls);
                }

                messages.push(msg);
            }
        }

        let tools: Vec<serde_json::Value> = req.tools.iter().map(|t| json!({
            "type": "function",
            "function": {
                "name":        t.name,
                "description": t.description,
                "parameters":  t.input_schema,
            }
        })).collect();

        let mut body = json!({
            "model":    req.model,
            "messages": messages,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        if stream {
            body["stream"] = json!(true);
        }
        body
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn id(&self)   -> &str { "openai" }
    fn name(&self) -> &str { "OpenAI" }
    fn models(&self) -> Vec<ModelInfo> { Self::models_static() }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request, false);
        debug!("OpenAI complete: model={}", request.model);

        let resp = self.client
            .post(format!("{OPENAI_API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type",  "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!("OpenAI {status}: {text}")));
        }

        let data: serde_json::Value = resp.json().await?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request, true);
        debug!("OpenAI stream: model={}", request.model);

        let resp = self.client
            .post(format!("{OPENAI_API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type",  "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!("OpenAI stream {status}: {text}")));
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes = bytes_result.map_err(AutoDevError::Http)?;
                let text  = String::from_utf8_lossy(&bytes);
                let mut buf = buffer.lock().unwrap();
                buf.push_str(&text);
                let mut chunks = Vec::new();
                // Only parse complete lines
                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim_end_matches('\r').to_string();
                    *buf = buf[pos + 1..].to_string();
                    if let Some(chunk) = parse_openai_sse_line(&line)? {
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
        // Use models endpoint to validate key
        let resp = self.client
            .get(format!("{OPENAI_API_BASE}/models"))
            .header("Authorization", format!("Bearer {}", self.api_key))
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
                error!("OpenAI connection test failed: {e}");
                Ok(ProviderStatus::Error(e.to_string()))
            }
        }
    }
}

fn parse_openai_sse_line(line: &str) -> Result<Option<StreamChunk>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(None);
    }
    let data = match line.strip_prefix("data: ") {
        Some(d) => d,
        None => return Ok(None),
    };
    if data == "[DONE]" {
        return Ok(Some(StreamChunk::Done));
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v)  => parse_openai_chunk(&v),
        Err(e) => Err(AutoDevError::Provider(format!("SSE parse error: {e} — data: {data}"))),
    }
}

fn parse_openai_chunk(v: &serde_json::Value) -> Result<Option<StreamChunk>> {
    let choices = match v["choices"].as_array() {
        Some(c) => c,
        None => return Ok(None),
    };
    if choices.is_empty() {
        return Ok(None);
    }

    let delta = &choices[0]["delta"];
    
    // Text content delta
    if let Some(content) = delta["content"].as_str() {
        return Ok(Some(StreamChunk::TextDelta {
            text: content.to_string(),
        }));
    }

    // Tool calls delta
    if let Some(tool_calls) = delta["tool_calls"].as_array() {
        for tc in tool_calls {
            let index = tc["index"].as_u64().unwrap_or(0);
            
            // Tool use start
            if let Some(id) = tc["id"].as_str() {
                if let Some(func) = tc["function"].as_object() {
                    if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                        return Ok(Some(StreamChunk::ToolUseStart {
                            id: id.to_string(),
                            name: name.to_string(),
                            block_index: index,
                        }));
                    }
                }
            }

            // Tool input delta
            if let Some(func) = tc["function"].as_object() {
                if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                    if !args.is_empty() {
                        return Ok(Some(StreamChunk::ToolInputDelta {
                            block_index: index,
                            partial_json: args.to_string(),
                        }));
                    }
                }
            }
        }
    }

    // Message completion
    if let Some(finish_reason) = choices[0]["finish_reason"].as_str() {
        if finish_reason != "null" && !finish_reason.is_empty() {
            // Extract usage if present
            if let Some(usage) = v["usage"].as_object() {
                let tokens_in  = usage.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                let tokens_out = usage.get("completion_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                return Ok(Some(StreamChunk::MessageDelta {
                    stop_reason: finish_reason.to_string(),
                    tokens_in,
                    tokens_out,
                }));
            }
        }
    }

    Ok(None)
}

fn parse_completion_response(data: serde_json::Value) -> Result<CompletionResponse> {
    let message_id  = data["id"].as_str().unwrap_or("").to_string();
    let model       = data["model"].as_str().unwrap_or("").to_string();
    let tokens_in   = data["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
    let tokens_out  = data["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;

    let empty_array = vec![];
    let choices = data["choices"].as_array().unwrap_or(&empty_array);
    let stop_reason = choices.get(0)
        .and_then(|c| c["finish_reason"].as_str())
        .unwrap_or("stop")
        .to_string();

    let message = choices.get(0).and_then(|c| c.get("message"));
    let mut content = Vec::new();

    if let Some(msg) = message {
        // Text content
        if let Some(text) = msg["content"].as_str() {
            content.push(MessageContent::Text {
                text: text.to_string(),
            });
        }

        // Tool calls
        if let Some(tool_calls) = msg["tool_calls"].as_array() {
            for tc in tool_calls {
                let id = tc["id"].as_str().unwrap_or("").to_string();
                let func = &tc["function"];
                let name = func["name"].as_str().unwrap_or("").to_string();
                let args_str = func["arguments"].as_str().unwrap_or("{}");
                let input = serde_json::from_str(args_str).unwrap_or_else(|_| json!({}));
                
                content.push(MessageContent::ToolUse { id, name, input });
            }
        }
    }

    Ok(CompletionResponse {
        message_id,
        content,
        stop_reason,
        tokens_in,
        tokens_out,
        model,
    })
}
