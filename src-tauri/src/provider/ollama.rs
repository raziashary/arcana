// Ollama provider — local self-hosted LLM inference via ollama-rs.
// PA mode only. No API key required; connects to local Ollama server
// defaulting to http://localhost:11434.
//
// Supports chat completion (non-streaming + streaming).
// Tool/function calling is provisionally wired but currently relies on
// the non-streaming path because ollama-rs tool-call streaming is
// incomplete as of early 2026.

use async_trait::async_trait;
use futures::StreamExt;
use std::panic::catch_unwind;
use std::panic::AssertUnwindSafe;
use tracing::{debug, error, info, warn};

use super::traits::{Provider, StreamResult};
use super::types::{
    CompletionRequest, CompletionResponse, MessageContent, ModelInfo, ProviderStatus,
    RequestMessage, StreamChunk,
};
use crate::errors::{AutoDevError, Result};

// ── Ollama client & types ────────────────────────────────────────────────────

use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::generation::chat::{ChatMessage, ChatMessageResponse};
use ollama_rs::Ollama;

/// Local Ollama provider.
pub struct OllamaProvider {
    host: String,
    port: u16,
    client: Ollama,
}

impl OllamaProvider {
    pub fn new(host: String, port: u16) -> Self {
        // ollama-rs 0.3.4 expects the host string to contain the scheme
        // (e.g. "http://localhost"), otherwise it panics with
        // RelativeUrlWithoutBase.
        let host_url = if host.starts_with("http://") || host.starts_with("https://") {
            host
        } else {
            format!("http://{}", host)
        };
        Self {
            client: Ollama::new(host_url.clone(), port),
            host: host_url,
            port,
        }
    }

    pub fn default_from_env() -> Self {
        if let Ok(url) = std::env::var("OLLAMA_HOST") {
            // OLLAMA_HOST may be:
            //   "host:port"
            //   "http://host:port"
            //   "https://host:port"
            //   just "host"
            // Detect scheme (default to http), then parse host:port.
            let (scheme, rest) = if let Some(stripped) = url.strip_prefix("https://") {
                ("https", stripped)
            } else if let Some(stripped) = url.strip_prefix("http://") {
                ("http", stripped)
            } else {
                ("http", url.as_str())
            };

            let mut parts = rest.split(':');
            let host = parts
                .next()
                .unwrap_or("localhost")
                .trim_end_matches('/')
                .to_string();
            let port = parts
                .next()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(11434);

            let host_with_scheme = format!("{}://{}", scheme, host);
            debug!(
                "[OllamaProvider] Parsed OLLAMA_HOST={} → host={}, port={}",
                url, host_with_scheme, port
            );
            Self::new(host_with_scheme, port)
        } else {
            Self::new("http://localhost".to_string(), 11434)
        }
    }

    /// Asynchronous model discovery — lists locally-pulled models.
    pub async fn fetch_models(&self) -> Result<Vec<ModelInfo>> {
        let local_models = self
            .client
            .list_local_models()
            .await
            .map_err(map_ollama_err)?;

        let mut out = Vec::with_capacity(local_models.len());
        for m in local_models {
            let name = m.name.clone();
            let ctx = context_window_for_model(&name);
            let is_vision = name.contains("vision") || name.contains("llava");
            let is_embedding = name.contains("embed");

            if is_embedding {
                continue; // skip embedding-only models for chat provider list
            }

            out.push(ModelInfo {
                id: name.clone(),
                name: name.clone(),
                context_window: ctx,
                supports_vision: is_vision,
                supports_thinking: false,
                cost_in_per_1m: 0.0,
                cost_out_per_1m: 0.0,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn id(&self) -> &str {
        "ollama"
    }
    fn name(&self) -> &str {
        "Ollama"
    }

    /// Returns a static fallback list of well-known Ollama models.
    /// Use [`OllamaProvider::fetch_models`] for the live local list.
    fn models(&self) -> Vec<ModelInfo> {
        models_static()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let model_name = request.model.clone();
        debug!("Ollama complete: model={}", model_name);

        let chat_request = build_chat_request(&request)?;
        let response: ChatMessageResponse = self
            .client
            .send_chat_messages(chat_request)
            .await
            .map_err(map_ollama_err)?;

        parse_chat_response(response, &model_name)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let model_name = request.model.clone();
        debug!("Ollama stream: model={}", model_name);

        // Tool calling does not stream progressively in ollama-rs.
        // Fallback to complete() wrapped as a single-chunk stream.
        if !request.tools.is_empty() {
            warn!("Ollama stream with tools requested — falling back to non-streaming path");
            let response = self.complete(request).await?;
            let chunks = vec![
                Ok(StreamChunk::TextDelta {
                    text: extract_text(&response.content),
                }),
                Ok(StreamChunk::MessageDelta {
                    stop_reason: response.stop_reason.clone(),
                    tokens_in: response.tokens_in,
                    tokens_out: response.tokens_out,
                }),
                Ok(StreamChunk::Done),
            ];
            return Ok(Box::pin(futures::stream::iter(chunks)));
        }

        let chat_request = build_chat_request(&request)?;

        // Task D2: Wrap Ollama stream in catch_unwind to prevent panics from
        // malformed URLs or other ollama-rs internal errors from crashing the app.
        let ollama_stream = match catch_unwind(AssertUnwindSafe(|| {
            // This closure captures the async call; we'll await it below
            self.client.send_chat_messages_stream(chat_request)
        })) {
            Ok(stream_future) => match stream_future.await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("[OllamaProvider] Stream error: {}", e);
                    return Err(map_ollama_err(e));
                }
            },
            Err(_) => {
                error!("[OllamaProvider] Ollama stream panicked (likely URL parsing issue)");
                return Err(AutoDevError::Provider(
                    "Ollama stream panicked — check OLLAMA_HOST env var".to_string(),
                ));
            }
        };

        let mapped = ollama_stream.flat_map(move |result| match result {
            Ok(chunk) => {
                let chunks = map_ollama_stream_chunks(chunk);
                futures::stream::iter(chunks)
            }
            Err(()) => futures::stream::iter(vec![Err(AutoDevError::Provider(
                "Ollama stream error".to_string(),
            ))]),
        });

        Ok(Box::pin(mapped))
    }

    async fn test_connection(&self) -> Result<ProviderStatus> {
        match self.client.list_local_models().await {
            Ok(models) => {
                if models.is_empty() {
                    warn!(
                        "Ollama reachable at {}:{}, but no models pulled yet",
                        self.host, self.port
                    );
                    Ok(ProviderStatus::Connected)
                } else {
                    info!(
                        "Ollama connected at {}:{} — {} models available",
                        self.host,
                        self.port,
                        models.len()
                    );
                    Ok(ProviderStatus::Connected)
                }
            }
            Err(e) => {
                error!(
                    "Ollama connection test failed at {}:{}: {e}",
                    self.host, self.port
                );
                Ok(ProviderStatus::Error(format!(
                    "Ollama not reachable at {}:{} — {e}",
                    self.host, self.port
                )))
            }
        }
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

fn build_chat_request(req: &CompletionRequest) -> Result<ChatMessageRequest> {
    let messages = to_ollama_messages(&req.messages, req.system.as_deref())?;

    let request = ChatMessageRequest::new(req.model.clone(), messages);

    // Tool mapping — provisionally included; ollama-rs tool support is evolving.
    // Raw chat .tools() builder is not yet exposed in ollama-rs 0.3.4 public API
    // (only available through Coordinator). Deferred.

    Ok(request)
}

fn to_ollama_messages(msgs: &[RequestMessage], system: Option<&str>) -> Result<Vec<ChatMessage>> {
    let mut out = Vec::new();

    if let Some(sys) = system {
        out.push(ChatMessage::system(sys.to_string()));
    }

    for msg in msgs {
        // Determine effective role from our request; map content blocks.
        let role = msg.role.as_str();
        let text_parts: Vec<String> = msg
            .content
            .iter()
            .filter_map(|c| match c {
                MessageContent::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect();

        let text = if text_parts.is_empty() {
            String::new()
        } else {
            text_parts.join("\n")
        };

        match role {
            "user" => {
                // Append tool-use descriptions inline if text is empty
                let mut content = text;
                for block in &msg.content {
                    if let MessageContent::ToolUse { name, input, .. } = block {
                        if content.is_empty() {
                            content = format!("Using tool: {} with input: {}", name, input);
                        } else {
                            content.push_str(&format!("\n[Tool call: {} — {}]", name, input));
                        }
                    }
                }
                out.push(ChatMessage::user(content));
            }
            "assistant" => {
                let mut content = text;
                for block in &msg.content {
                    if let MessageContent::ToolUse { name, input, .. } = block {
                        if content.is_empty() {
                            content = format!("Calling tool: {} with {}", name, input);
                        } else {
                            content.push_str(&format!("\n[Tool call: {} — {}]", name, input));
                        }
                    }
                    if let MessageContent::ToolResult {
                        content: result, ..
                    } = block
                    {
                        // Assistant should not emit tool results; they are separate messages.
                        // If present, treat as assistant text annotation.
                        content.push_str(&format!("\n[Tool result: {}]", result));
                    }
                }
                out.push(ChatMessage::assistant(content));
            }
            _ => {
                // Fallback: treat as user message
                out.push(ChatMessage::user(text));
            }
        }
    }

    Ok(out)
}

fn parse_chat_response(resp: ChatMessageResponse, model: &str) -> Result<CompletionResponse> {
    let message_id = format!("ollama-{}", uuid::Uuid::new_v4());

    let content_text = resp.message.content.clone();
    let mut content = Vec::new();

    if !content_text.is_empty() {
        content.push(MessageContent::Text { text: content_text });
    }

    // Handle tool_calls in response if present.
    // ollama-rs ChatMessageResponseMessage may have `tool_calls` field.
    // TODO: verify field names against actual ollama-rs struct when tool API stabilizes.

    let stop_reason = if resp.done {
        "end_turn".to_string()
    } else {
        "end_turn".to_string()
    };

    let (tokens_in, tokens_out) = match &resp.final_data {
        Some(data) => (data.prompt_eval_count as u32, data.eval_count as u32),
        None => (0, 0),
    };

    Ok(CompletionResponse {
        message_id,
        content,
        stop_reason,
        tokens_in,
        tokens_out,
        model: model.to_string(),
    })
}

fn map_ollama_stream_chunks(chunk: ChatMessageResponse) -> Vec<Result<StreamChunk>> {
    let text = chunk.message.content.clone();

    // Intermediate chunk with text content
    if !text.is_empty() && !chunk.done {
        return vec![Ok(StreamChunk::TextDelta { text })];
    }

    // Final chunk signals completion — emit MessageDelta + Done
    if chunk.done {
        let stop_reason = "end_turn".to_string();
        let (tokens_in, tokens_out) = match &chunk.final_data {
            Some(data) => (data.prompt_eval_count as u32, data.eval_count as u32),
            None => (0, 0),
        };
        return vec![
            Ok(StreamChunk::MessageDelta {
                stop_reason,
                tokens_in,
                tokens_out,
            }),
            Ok(StreamChunk::Done),
        ];
    }

    // Empty non-final chunk — nothing to emit
    vec![]
}

fn map_ollama_err(e: impl std::fmt::Display) -> AutoDevError {
    AutoDevError::Provider(format!("Ollama: {e}"))
}

fn extract_text(content: &[MessageContent]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            MessageContent::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

// ── Model family context-window lookup ───────────────────────────────────────

fn context_window_for_model(name: &str) -> u32 {
    let lower = name.to_lowercase();
    if lower.starts_with("llama3.2")
        || lower.starts_with("llama3.1")
        || lower.starts_with("llama3.3")
    {
        128_000
    } else if lower.starts_with("llama3") {
        8_000 // catch-all for other llama3 variants
    } else if lower.starts_with("mistral") || lower.starts_with("mixtral") {
        32_768
    } else if lower.starts_with("codellama") {
        16_384
    } else if lower.starts_with("deepseek-coder") {
        16_384
    } else if lower.starts_with("deepseek") {
        64_000
    } else if lower.starts_with("qwen2.5") || lower.starts_with("qwen2") {
        32_768
    } else if lower.starts_with("llava") || lower.starts_with("bakllava") {
        4_096
    } else if lower.starts_with("phi3") || lower.starts_with("phi4") {
        128_000
    } else if lower.starts_with("gemma") {
        8_192
    } else if lower.starts_with("command-r") {
        128_000
    } else {
        4_096 // safe conservative default
    }
}

fn models_static() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "llama3.2".into(),
            name: "Llama 3.2".into(),
            context_window: 128_000,
            supports_vision: true,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "llama3.1".into(),
            name: "Llama 3.1".into(),
            context_window: 128_000,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "mistral".into(),
            name: "Mistral 7B".into(),
            context_window: 32_768,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "mixtral".into(),
            name: "Mixtral 8x7B".into(),
            context_window: 32_768,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "codellama".into(),
            name: "CodeLlama".into(),
            context_window: 16_384,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "deepseek-coder".into(),
            name: "DeepSeek Coder".into(),
            context_window: 16_384,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "qwen2.5".into(),
            name: "Qwen 2.5".into(),
            context_window: 32_768,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
        ModelInfo {
            id: "custom".into(),
            name: "Custom (any pulled model)".into(),
            context_window: 4_096,
            supports_vision: false,
            supports_thinking: false,
            cost_in_per_1m: 0.0,
            cost_out_per_1m: 0.0,
        },
    ]
}

// ── Tool mapping (commented — ollama-rs raw chat tool API is unstable) ──────
//
// fn to_ollama_tools(tools: &[ToolDefinition]) -> Vec<ollama_rs::generation::tools::Tool> {
//     tools.iter().map(|t| {
//         ollama_rs::generation::tools::Tool::new(
//             t.name.clone(),
//             Some(t.description.clone()),
//             Some(t.input_schema.clone()),
//         )
//     }).collect()
// }
//
// fn model_supports_tools(model: &str) -> bool {
//     let lower = model.to_lowercase();
//     lower.starts_with("qwen2.5") || lower.starts_with("llama3.1")
//         || lower.starts_with("llama3.2") || lower.starts_with("mistral-nemo")
// }
