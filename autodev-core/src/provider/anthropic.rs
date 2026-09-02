// Anthropic provider — direct API calls to api.anthropic.com.
// PA mode only. Worker mode delegates to core-orchestrator's litellm.
// Implements the Provider trait with streaming support.

use async_trait::async_trait;
use super::usage::CachePolicy;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use super::streaming::parse_sse_line;
use super::traits::{Provider, StreamResult};
use super::usage::Usage;
use super::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus, StreamChunk,
};
use crate::errors::{AutoDevError, Result};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    /// The models this provider serves.
    ///
    /// ⚠ This used to be a hand-written list here, and by 2026-09-02 all three
    /// adapters' lists had drifted a full generation behind `src/data/models.ts`
    /// — the list the user actually picks from. The overlap between the two was
    /// one model. Four entries here were retired in February 2026.
    ///
    /// Reads the shared catalogue now, so a lookup by model id cannot come back
    /// empty for a model the app offers. See `provider/catalogue.rs`.
    fn models_static() -> Vec<ModelInfo> {
        crate::provider::catalogue::models_for("anthropic")
    }
    /// The request body, for conformance tests.
    ///
    /// Exposes the real builder rather than letting a test reimplement it —
    /// a test that constructs its own body proves only that the test agrees
    /// with itself.
    #[cfg(test)]
    pub fn build_body_for_test(&self, req: &CompletionRequest, _stream: bool) -> serde_json::Value {
        self.build_body(req, _stream)
    }

    fn build_body(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        // Phase 7.1 — normalize all model identifiers to bare direct-API IDs.
        //
        // History: workers.db may store old slug names ("claude-4-5-haiku") from
        // before the canonical naming was settled, or LiteLLM proxy-format names
        // ("anthropic/claude-haiku-4-5-20251001") that crept in via earlier code.
        // Both are wrong for direct api.anthropic.com calls which require the bare
        // model ID only. This caused 9 × "404 not_found_error" in the 2026-04-29
        // run (log lines 5665, 18916, 21223, 21226, 29402, 37933, 39528, 53037,
        // 53040) for qc-agent and supervisor whenever Mode 1 fallback fired.
        //
        // Canonical IDs accepted by api.anthropic.com/v1/messages:
        //   claude-haiku-4-5  |  claude-sonnet-4-5  |  claude-opus-4-5
        let model = match req.model.as_str() {
            // Old slug names → canonical direct-API IDs
            "claude-4-5-haiku" | "claude-haiku-4-5" => "claude-haiku-4-5",
            "claude-4-5-sonnet" | "claude-sonnet-4-5" => "claude-sonnet-4-5",
            "claude-4-5-opus" | "claude-opus-4-5" => "claude-opus-4-5",
            // Strip any LiteLLM "anthropic/" prefix that may have crept in via config
            s if s.starts_with("anthropic/") => s.trim_start_matches("anthropic/"),
            other => other,
        };

        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| {
                let content: Vec<serde_json::Value> = m
                    .content
                    .iter()
                    .map(|c| match c {
                        MessageContent::Text { text } => json!({ "type": "text", "text": text }),
                        MessageContent::ToolUse { id, name, input } => json!({
                            "type": "tool_use",
                            "id":   id,
                            "name": name,
                            "input": input,
                        }),
                        MessageContent::ToolResult {
                            tool_use_id,
                            // Anthropic keys tool results on the id, not the
                            // name, so the name is unused here — but it is
                            // matched explicitly rather than with `..` so a
                            // future field cannot be silently ignored.
                            name: _,
                            content,
                            is_error,
                        } => json!({
                            "type":        "tool_result",
                            "tool_use_id": tool_use_id,
                            "content":     content,
                            "is_error":    is_error,
                        }),
                    })
                    .collect();
                json!({ "role": m.role, "content": content })
            })
            .collect();

        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name":         t.name,
                    "description":  t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();

        let mut body = json!({
            "model":       model,
            "max_tokens":  req.max_tokens,
            "messages":    messages,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
        }
        if let Some(sys) = &req.system {
            // P2-63. Sent as a one-element content-block ARRAY rather than a
            // bare string.
            //
            // Anthropic accepts both, but `cache_control` can only be attached
            // to a block object — so as a bare string there was nowhere to hang
            // a cache breakpoint, and `P2-16` would have had to change this
            // shape first. Changing it here, before the cache work, keeps that
            // change out of the commit that has to prove caching works.
            // P2-16. The breakpoint goes on the system block when the caller
            // asked for `Explicit` caching.
            //
            // Anthropic hashes `tools → system → messages` and caches
            // everything up to the marked block, so marking the LAST system
            // block caches the tool definitions and the whole system prompt in
            // one entry. Volatile content is not here to be marked around:
            // `cache::prefix` keeps it out of this region entirely (P2-14).
            //
            // Below the model's minimum Anthropic caches nothing and returns
            // no error, which is why `cache::prefix::will_cache` is consulted
            // before a policy of `Explicit` is ever set.
            let mut block = json!({ "type": "text", "text": sys });
            if let CachePolicy::Explicit { ref ttl, .. } = req.cache {
                block["cache_control"] = json!({
                    "type": "ephemeral",
                    "ttl": ttl.as_wire_str(),
                });
            }
            body["system"] = json!([block]);
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
    fn id(&self) -> &str {
        "anthropic"
    }
    fn name(&self) -> &str {
        "Anthropic"
    }
    fn models(&self) -> Vec<ModelInfo> {
        Self::models_static()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request, false);
        debug!("Anthropic complete: model={}", request.model);

        let resp = self
            .client
            .post(format!("{ANTHROPIC_API_BASE}/messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!(
                "Anthropic {status}: {text}"
            )));
        }

        let data: serde_json::Value = resp.json().await?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request, true);
        debug!("Anthropic stream: model={}", request.model);

        let resp = self
            .client
            .post(format!("{ANTHROPIC_API_BASE}/messages"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AutoDevError::Provider(format!(
                "Anthropic stream {status}: {text}"
            )));
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries so SSE
        // data lines are never parsed mid-line (fixes "EOF while parsing" crash
        // that occurs when a long tool response is split across two HTTP chunks).
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes = bytes_result.map_err(AutoDevError::Http)?;
                let text = String::from_utf8_lossy(&bytes);
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
                    Err(e) => vec![Err(e)],
                };
                futures::stream::iter(items)
            });

        Ok(Box::pin(chunk_stream))
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        // Use models endpoint — minimal cost, validates the key
        let resp = self
            .client
            .get(format!("{ANTHROPIC_API_BASE}/models"))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderStatus::Connected),
            Ok(r) if r.status() == 401 => Ok(ProviderStatus::Error("Invalid API key".into())),
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
    let message_id = data["id"].as_str().unwrap_or("").to_string();
    let stop_reason = data["stop_reason"]
        .as_str()
        .unwrap_or("end_turn")
        .to_string();
    let model = data["model"].as_str().unwrap_or("").to_string();
    // Anthropic already EXCLUDES cached tokens from `input_tokens`, which is
    // the meaning `Usage` fixes — so these are used as reported, with no
    // subtraction. Doing what OpenAI/Google need here would under-count.
    let u = &data["usage"];
    let usage = Usage {
        input: u["input_tokens"].as_u64().unwrap_or(0) as u32,
        cached_read: u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32,
        cache_write: u["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32,
        output: u["output_tokens"].as_u64().unwrap_or(0) as u32,
        reasoning: 0,
        tool: 0,
    };

    let content = data["content"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|block| match block["type"].as_str()? {
            "text" => Some(MessageContent::Text {
                text: block["text"].as_str().unwrap_or("").to_string(),
            }),
            "tool_use" => Some(MessageContent::ToolUse {
                id: block["id"].as_str().unwrap_or("").to_string(),
                name: block["name"].as_str().unwrap_or("").to_string(),
                input: block["input"].clone(),
            }),
            _ => None,
        })
        .collect();

    Ok(CompletionResponse {
        message_id,
        content,
        stop_reason,
        usage,
        model,
    })
}
