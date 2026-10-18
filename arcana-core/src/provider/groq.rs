// Groq provider — direct API calls to api.groq.com/openai/v1.
// Implements the Provider trait with streaming support.
//
// ── Why this adapter is thin ─────────────────────────────────────────────────
//
// Groq serves the OpenAI chat-completions wire format at its own base URL, so
// the request body, the SSE parser and the unary response parser are the
// **shared OpenAI-compatible layer** (`super::openai`), not copies. What this
// file owns is exactly the vendor-specific part:
//
//   * the base URL and `Authorization: Bearer` header,
//   * which models the catalogue serves (`qwen/` id space),
//   * the vendored error/vendor strings in diagnostics,
//   * the fact that `prompt_cache_key` is NOT sent (the affinity hint is an
//     OpenAI field; Groq's model pages do not document it — sending an
//     undocumented field risks a 400 for zero benefit),
//   * free key validation against `/models`.
//
// ── Wire facts (verified 2026-10-17, Groq docs; see _verify receipt) ────────
//
//   * model id `qwen/qwen3.8-27b` — the only id this model ships under. It
//     replaced `qwen/qwen3.6-27b`, deprecated 2026-09-14.
//   * context window 131,072; max completion tokens 16,384.
//   * `reasoning_effort` accepts `none` / `low` / `medium` / `high` for this
//     model (`high` selects the native `xhigh` mode). Values outside the
//     supported set are rejected with a 400, so anything the engine supplies
//     passes through only if it is in `CompletionRequest::reasoning_effort`
//     already — the shared body builder forwards it unmodified when present.
//   * priced $0.80 input / $4.00 output per 1M tokens (`cost::pricing`).
//
// The keys go in a HEADER, never the URL (see `auth_header` history on the
// other adapters). Error bodies are never echoed raw into errors or telemetry
// — the shared `safe_error_response` path holds that invariant.

use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use tracing::{debug, error};

use super::openai::{
    build_chat_completions_body, parse_completion_response, parse_openai_sse_line,
};
use super::traits::{Provider, StreamResult};
use super::types::{CompletionRequest, CompletionResponse, ModelInfo, ProviderStatus, StreamChunk};
use super::usage::{safe_decode_error, safe_error_response, safe_transport_error};
use crate::errors::{ArcanaError, Result};
use crate::state::lock::LockTolerant;

const GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";

pub struct GroqProvider {
    api_key: String,
    client: Client,
    api_base: String,
}

impl GroqProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
            api_base: GROQ_API_BASE.to_string(),
        }
    }

    /// The models this provider serves, from the shared catalogue.
    fn models_static() -> Vec<ModelInfo> {
        crate::provider::catalogue::models_for("groq")
    }

    #[cfg(test)]
    pub fn build_body_for_test(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        self.build_body(req, stream)
    }

    fn build_body(&self, req: &CompletionRequest, stream: bool) -> serde_json::Value {
        // Groq is called with `None` affinity: `prompt_cache_key` is an OpenAI
        // field, and the shared builder sends it only when asked. See the
        // module comment.
        build_chat_completions_body(req, stream, None)
    }
}

#[async_trait]
impl Provider for GroqProvider {
    fn id(&self) -> &str {
        "groq"
    }
    fn name(&self) -> &str {
        "Groq"
    }
    fn models(&self) -> Vec<ModelInfo> {
        Self::models_static()
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let body = self.build_body(&request, false);
        debug!("Groq complete: model={}", request.model);

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| safe_transport_error("Groq", "complete"))?;

        let status = resp.status();
        if !status.is_success() {
            let (class, summary) = safe_error_response("groq", status, resp).await;
            // P2-67/68, same as the other adapters: classify where the body is
            // available, because a 429 from a spend cap and a 429 from a rate
            // limit are the same status and only the body separates them.
            return Err(ArcanaError::ProviderClassified {
                class,
                message: format!("Groq {status}: {summary}"),
            });
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|_| safe_decode_error("Groq", "complete"))?;
        parse_completion_response(data)
    }

    async fn stream(&self, request: CompletionRequest) -> Result<StreamResult> {
        let body = self.build_body(&request, true);
        debug!("Groq stream: model={}", request.model);

        let resp = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|_| safe_transport_error("Groq", "stream"))?;

        let status = resp.status();
        if !status.is_success() {
            let (class, summary) = safe_error_response("groq", status, resp).await;
            return Err(ArcanaError::ProviderClassified {
                class,
                message: format!("Groq stream {status}: {summary}"),
            });
        }

        let byte_stream = resp.bytes_stream();
        // Buffer accumulates raw bytes across TCP chunk boundaries
        let buffer = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let chunk_stream = byte_stream
            .map(move |bytes_result| -> Result<Vec<StreamChunk>> {
                let bytes =
                    bytes_result.map_err(|_| safe_transport_error("Groq", "stream_body"))?;
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
        // The models endpoint, like the other adapters: free, needs no model
        // id, and cannot be satisfied by a syntactically plausible key.
        let resp = self
            .client
            .get(format!("{}/models", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await;

        match resp {
            Ok(r) if r.status().is_success() => Ok(ProviderStatus::Connected),
            // Groq answers an unauthorised key on the models endpoint with
            // 401; 403 is held for quota/permission shapes. 400 is included
            // with the same justification as Google's adapter: a malformed
            // request deserves the actionable answer.
            Ok(r) if r.status() == 400 || r.status() == 401 || r.status() == 403 => {
                Ok(ProviderStatus::Error("Invalid API key".into()))
            }
            Ok(r) => {
                let code = r.status();
                Ok(ProviderStatus::Error(format!("Groq returned {code}")))
            }
            Err(_) => {
                // Never interpolate the reqwest error here. Its Display
                // includes the request URL, and this value crosses the Tauri
                // IPC boundary into Settings (D-6/D-10, P2 pre-build audit).
                error!("Groq connection test failed");
                Ok(ProviderStatus::Error("Connection failed".into()))
            }
        }
    }
}

#[cfg(test)]
pub fn parse_groq_sse_line_for_test(line: &str) -> Result<Vec<StreamChunk>> {
    parse_openai_sse_line(line)
}

#[cfg(test)]
#[path = "groq_tests.rs"]
mod groq_tests;
