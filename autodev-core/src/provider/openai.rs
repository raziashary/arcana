// OpenAI provider — direct API calls to api.openai.com.
// PA mode only. Worker mode delegates to core-orchestrator's litellm.
// Implements the Provider trait with streaming support.

use async_trait::async_trait;
use super::usage::CachePolicy;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error};

use super::traits::{Provider, StreamResult};
use super::usage::Usage;
use super::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus, StreamChunk,
};
use crate::errors::{AutoDevError, Result};
use crate::state::lock::LockTolerant;

const OPENAI_API_BASE: &str = "https://api.openai.com/v1";

pub struct OpenAIProvider {
    api_key: String,
    client: Client,
}

impl OpenAIProvider {
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
        crate::provider::catalogue::models_for("openai")
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
                let tool_results: Vec<&MessageContent> = m
                    .content
                    .iter()
                    .filter(|c| matches!(c, MessageContent::ToolResult { .. }))
                    .collect();

                // P2-39. This used to be `if !tool_results.is_empty() { ... }
                // else { text }`, so any Text sharing a message with tool
                // results was **dropped entirely** — the model's own commentary
                // silently vanished from its history.
                if !tool_results.is_empty() {
                    // Add tool result messages (role: "tool")
                    for tr in tool_results {
                        if let MessageContent::ToolResult {
                            tool_use_id,
                            name: _,
                            content,
                            is_error,
                        } = tr
                        {
                            // P2-38. OpenAI's `tool` message carries no error
                            // flag, so failure has to be in the payload.
                            // Discarding it made a failed tool look identical
                            // to a successful one on two of three providers,
                            // and the model builds on a result that never
                            // happened.
                            let body = if *is_error {
                                format!("ERROR: {content}")
                            } else {
                                content.clone()
                            };
                            messages.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": body,
                            }));
                        }
                    }
                }

                // Text in the same message is emitted regardless of whether
                // tool results were present.
                {
                    // Regular user text message
                    let text = m
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            MessageContent::Text { text } => Some(text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    if !text.is_empty() {
                        messages.push(json!({
                            "role": "user",
                            "content": text,
                        }));
                    }
                }
            } else if m.role == "assistant" {
                // Assistant messages: separate text and tool_calls
                let text_parts: Vec<String> = m
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        MessageContent::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect();

                let tool_calls: Vec<serde_json::Value> = m.content.iter()
                    .enumerate()
                    .filter_map(|(_idx, c)| match c {
                        MessageContent::ToolUse { id, name, input, .. } => Some(json!({
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
            } else {
                // P2-40. There was no `else` at all, so a message with any
                // other role **vanished silently** — while Google coerced the
                // same message to `user`. Two adapters, two different silent
                // behaviours for identical input. Both now coerce and log.
                tracing::warn!(
                    "OpenAI has no role '{}'; sending it as 'user'. The content \
                     is preserved, the attribution is not.",
                    m.role
                );
                let text = m
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        MessageContent::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    messages.push(json!({ "role": "user", "content": text }));
                }
            }
        }

        let tools: Vec<serde_json::Value> = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name":        t.name,
                        "description": t.description,
                        "parameters":  t.input_schema,
                    }
                })
            })
            .collect();

        let mut body = json!({
            "model":    req.model,
            "messages": messages,
        });
        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = req.top_p {
            body["top_p"] = json!(top_p);
        }
        // reasoning_effort for o-series models (o1, o3, o4-mini)
        if let Some(ref effort) = req.reasoning_effort {
            body["reasoning_effort"] = json!(effort);
        }
        // P2-08/P2-45. `max_tokens` was read only by the Anthropic adapter;
        // OpenAI and Google never sent it, so the engine's ceiling silently
        // applied to one provider of three and truncation could not be
        // requested, let alone detected.
        //
        // `max_completion_tokens` is the current field name; `max_tokens` is
        // deprecated and rejected outright by the reasoning models.
        body["max_completion_tokens"] = json!(req.max_tokens);

        // P2-16. OpenAI caches automatically — there is no breakpoint to place
        // and nothing to switch on. What it does accept is a routing hint:
        // identical prefixes cache per backend node, so requests that share a
        // prefix have to land on the same node to hit it.
        //
        // `Explicit` is deliberately NOT approximated here. There is no field
        // to express it, and sending something adjacent would make a request
        // that reports as cached-by-request while caching by luck.
        if let CachePolicy::Automatic { ref affinity_key } = req.cache {
            body["prompt_cache_key"] = json!(affinity_key);
        }

        if stream {
            body["stream"] = json!(true);
            // P2-42. Without this OpenAI sends no `usage` object on a stream at
            // all, so `parse_openai_sse_line`'s usage branch was a guard that
            // could never fire — token counts AND the stop reason were both
            // structurally zero for every streaming call.
            body["stream_options"] = json!({ "include_usage": true });
        }
        body
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn id(&self) -> &str {
        "openai"
    }
    fn name(&self) -> &str {
        "OpenAI"
    }
    fn models(&self) -> Vec<ModelInfo> {
        Self::models_static()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request, false);
        debug!("OpenAI complete: model={}", request.model);

        let resp = self
            .client
            .post(format!("{OPENAI_API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            // P2-67/68. Classified here, where the body is available: a 429
            // from a spend cap and a 429 from a rate limit are the same
            // status, and only the body separates them.
            let class = crate::provider::usage::ErrorClass::classify(
                "openai",
                status.as_u16(),
                &text,
            );
            return Err(AutoDevError::Provider(format!(
                "OpenAI {status} [{class:?}]: {text}"
            )));
        }

        let data: serde_json::Value = resp.json().await?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request, true);
        debug!("OpenAI stream: model={}", request.model);

        let resp = self
            .client
            .post(format!("{OPENAI_API_BASE}/chat/completions"))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            // P2-67/68. The class is attached rather than left to be
            // re-derived downstream: a 429 from a spend cap and a 429 from a
            // rate limit are the same status and only the body tells them
            // apart, so the one place that has the body is the place to decide.
            let class = crate::provider::usage::ErrorClass::classify(
                "openai",
                status.as_u16(),
                &text,
            );
            return Err(AutoDevError::Provider(format!(
                "OpenAI stream {status} [{class:?}]: {text}"
            )));
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes = bytes_result.map_err(AutoDevError::Http)?;
                let text = String::from_utf8_lossy(&bytes);
                let mut buf = buffer.lock_tolerant();
                buf.push_str(&text);
                let mut chunks = Vec::new();
                // Only parse complete lines
                while let Some(pos) = buf.find('\n') {
                    let line = buf[..pos].trim_end_matches('\r').to_string();
                    *buf = buf[pos + 1..].to_string();
                    chunks.extend(parse_openai_sse_line(&line)?);
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
        // Use models endpoint to validate key
        let resp = self
            .client
            .get(format!("{OPENAI_API_BASE}/models"))
            .header("Authorization", format!("Bearer {}", self.api_key))
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
                // ⚠ D-6. NEVER return the raw transport error to the caller.
                //
                // `reqwest::Error`'s Display includes the request URL, and this
                // value becomes `ProviderStatus::Error` — which crosses the
                // Tauri IPC boundary and is rendered in Settings. That is
                // exactly how a key in a query string reached the renderer in
                // P2's pre-build audit; the URL fix was applied to Google and
                // this conduit was left open on the other two.
                //
                // Since P2-23, `set_provider_key` surfaces this string to the
                // user directly, so the conduit is now load-bearing.
                //
                // Logged in full (the log sink redacts), returned as a fixed
                // string.
                error!("OpenAI connection test failed: {e}");
                Ok(ProviderStatus::Error("Connection failed".into()))
            }
        }
    }
}

/// Test accessor for the SSE parser.
///
/// The parser is private because nothing outside the adapter should be feeding
/// it raw wire text — but it is also where this adapter's hardest bugs lived
/// (P2-41, P2-42, F-7), so the fixture suite needs a way in.
#[cfg(test)]
pub fn parse_openai_sse_line_for_test(line: &str) -> Result<Vec<StreamChunk>> {
    parse_openai_sse_line(line)
}

fn parse_openai_sse_line(line: &str) -> Result<Vec<StreamChunk>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(Vec::new());
    }
    let data = match line.strip_prefix("data: ") {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    if data == "[DONE]" {
        return Ok(vec![StreamChunk::Done]);
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => parse_openai_chunk(&v),
        Err(e) => Err(AutoDevError::Provider(format!(
            "SSE parse error: {e} — data: {data}"
        ))),
    }
}

/// Parse one OpenAI SSE frame into **every** chunk it carries.
///
/// ── Why this returns a Vec (F-7, P2 post-phase audit) ────────────────────────
///
/// It used to return `Option<StreamChunk>` — at most one chunk per frame — and
/// `return`ed on the first thing it recognised. That is the exact defect
/// `P2-35` fixed on the Google adapter, left live on this one:
///
///   * a frame carrying several `tool_calls` entries emitted only the first,
///     so **parallel tool calls were silently dropped** — the model asked for
///     three tools and the engine saw one;
///   * a frame carrying both text and a tool call emitted only the text.
///
/// The engine's `assemble_in_wire_order` is keyed by block index precisely so
/// several blocks can arrive together; a parser that can only ever return one
/// makes that machinery unreachable.
fn parse_openai_chunk(v: &serde_json::Value) -> Result<Vec<StreamChunk>> {
    let mut out = Vec::new();

    let choices = match v["choices"].as_array() {
        Some(c) => c,
        None => return Ok(out),
    };

    // P2-42, second half. OpenAI delivers the usage frame as a final chunk
    // with `choices: []`. Returning here discarded it, so adding
    // `stream_options` alone would still have produced nothing — two edits were
    // required, and a fix that made only one would have looked complete.
    if choices.is_empty() {
        let u = &v["usage"];
        if u.is_object() {
            let prompt_total = u["prompt_tokens"].as_u64().unwrap_or(0) as u32;
            let cached_read = u["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .unwrap_or(0) as u32;
            let output = u["completion_tokens"].as_u64().unwrap_or(0) as u32;
            let reasoning = u["completion_tokens_details"]["reasoning_tokens"]
                .as_u64()
                .unwrap_or(0) as u32;
            out.push(StreamChunk::MessageDelta {
                // This frame carries usage, not a stop reason; the reason
                // arrived on the chunk that closed the choice.
                stop_reason: String::new(),
                usage: Usage::from_inclusive_input(prompt_total, cached_read, 0, output)
                    .with_reasoning(reasoning),
            });
        }
        return Ok(out);
    }

    let delta = &choices[0]["delta"];

    // Text content delta.
    if let Some(content) = delta["content"].as_str() {
        // OpenAI's chat stream carries no per-block index for content — the
        // text of one message arrives as one logical block, and tool calls carry
        // their own `index` separately. Block 0 is therefore correct here, not a
        // placeholder.
        //
        // No longer an early return: a frame can carry text AND a tool call.
        out.push(StreamChunk::TextDelta {
            block_index: 0,
            text: content.to_string(),
        });
    }

    // Tool calls delta — EVERY entry, not just the first.
    if let Some(tool_calls) = delta["tool_calls"].as_array() {
        for tc in tool_calls {
            let index = tc["index"].as_u64().unwrap_or(0);

            // The id and name appear only on the first fragment for an index.
            if let Some(id) = tc["id"].as_str() {
                if let Some(name) = tc["function"]["name"].as_str() {
                    out.push(StreamChunk::ToolUseStart {
                        id: id.to_string(),
                        name: name.to_string(),
                        block_index: index,
                        // OpenAI issues no echo token.
                        opaque: None,
                    });
                }
            }

            // Argument fragments, keyed by index alone.
            if let Some(args) = tc["function"]["arguments"].as_str() {
                if !args.is_empty() {
                    out.push(StreamChunk::ToolInputDelta {
                        block_index: index,
                        partial_json: args.to_string(),
                    });
                }
            }
        }
    }

    // P2-41. The stop reason. This compared `finish_reason != "null"` against
    // the four-character STRING; JSON `null` yields `None` from `as_str()` and
    // never reached that comparison, so the check was inert.
    if let Some(finish_reason) = choices[0]["finish_reason"].as_str() {
        if !finish_reason.is_empty() {
            let u = &v["usage"];
            let usage = if u.is_object() {
                let prompt_total = u["prompt_tokens"].as_u64().unwrap_or(0) as u32;
                let cached_read = u["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                let output = u["completion_tokens"].as_u64().unwrap_or(0) as u32;
                let reasoning = u["completion_tokens_details"]["reasoning_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32;
                Usage::from_inclusive_input(prompt_total, cached_read, 0, output)
                    .with_reasoning(reasoning)
            } else {
                Usage::default()
            };

            out.push(StreamChunk::MessageDelta {
                stop_reason: finish_reason.to_string(),
                usage,
            });
        }
    }

    Ok(out)
}

fn parse_completion_response(data: serde_json::Value) -> Result<CompletionResponse> {
    let message_id = data["id"].as_str().unwrap_or("").to_string();
    let model = data["model"].as_str().unwrap_or("").to_string();
    // INCLUSIVE vendor — see the streaming path above.
    let u = &data["usage"];
    let usage = Usage::from_inclusive_input(
        u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        u["prompt_tokens_details"]["cached_tokens"].as_u64().unwrap_or(0) as u32,
        0,
        u["completion_tokens"].as_u64().unwrap_or(0) as u32,
    )
    .with_reasoning(
        u["completion_tokens_details"]["reasoning_tokens"]
            .as_u64()
            .unwrap_or(0) as u32,
    );

    let empty_array = vec![];
    let choices = data["choices"].as_array().unwrap_or(&empty_array);
    let stop_reason = choices
        .get(0)
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

                content.push(MessageContent::ToolUse {
                    id,
                    name,
                    input,
                    opaque: None,
                });
            }
        }
    }

    Ok(CompletionResponse {
        message_id,
        content,
        stop_reason,
        usage,
        model,
    })
}
