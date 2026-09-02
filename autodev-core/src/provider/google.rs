// Google Gemini provider — direct API calls to generativelanguage.googleapis.com.
// PA mode only. Worker mode delegates to core-orchestrator's litellm.
// Implements the Provider trait with streaming support.

use async_trait::async_trait;
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
        crate::provider::catalogue::models_for("google")
    }
    /// The request body, for conformance tests.
    ///
    /// Exposes the real builder rather than letting a test reimplement it —
    /// a test that constructs its own body proves only that the test agrees
    /// with itself.
    #[cfg(test)]
    pub fn build_body_for_test(&self, req: &CompletionRequest, _stream: bool) -> serde_json::Value {
        self.build_body(req)
    }

    fn build_body(&self, req: &CompletionRequest) -> serde_json::Value {
        // Convert messages to Gemini format
        let mut contents = Vec::new();
        let mut system_instruction = None;

        for msg in &req.messages {
            let role = match msg.role.as_str() {
                "user" => "user",
                "assistant" => "model",
                // P2-40. Gemini knows only `user` and `model`. Coercing is the
                // right call — dropping the message loses context the model
                // needs — but it was silent, and OpenAI dropped the same
                // message instead. Two adapters, two different silent
                // behaviours. Now coerced *and* logged, on both.
                other => {
                    tracing::warn!(
                        "Gemini has no role '{other}'; sending it as 'user'. \
                         The content is preserved, the attribution is not."
                    );
                    "user"
                }
            };

            let parts: Vec<serde_json::Value> = msg
                .content
                .iter()
                .map(|c| match c {
                    MessageContent::Text { text } => json!({ "text": text }),
                    MessageContent::ToolUse {
                        id: _,
                        name,
                        input,
                        opaque,
                    } => {
                        // The signature must be echoed back verbatim or Gemini
                        // rejects the whole turn with a 400. It is attached to
                        // the PART, not to the functionCall object.
                        let mut part = json!({
                            "functionCall": {
                                "name": name,
                                "args": input,
                            }
                        });
                        if let Some(sig) = opaque {
                            part["thoughtSignature"] = json!(sig);
                        }
                        part
                    }
                    MessageContent::ToolResult {
                        tool_use_id: _,
                        name,
                        content,
                        is_error,
                    } => json!({
                        "functionResponse": {
                            // P2-37: the NAME, not the id. Sending the id told
                            // Gemini that a function called `gemini-read_file-0`
                            // had replied.
                            "name": name,
                            // P2-38: Gemini's wire format carries no error flag,
                            // so failure is expressed in the payload. Dropping
                            // it made a failed tool indistinguishable from a
                            // successful one, and the model would build on a
                            // result that never happened.
                            "response": if *is_error {
                                json!({ "error": content })
                            } else {
                                json!({ "result": content })
                            }
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

        // generationConfig: maxOutputTokens, temperature, topP, thinkingConfig
        let mut gen_config = serde_json::Map::new();
        // P2-08/P2-45. `maxOutputTokens` appeared NOWHERE in the tree, so
        // `max_tokens` was honoured by the Anthropic adapter alone. A Gemini
        // turn had no output ceiling at all and could not report hitting one.
        // Unconditional: the engine always supplies a ceiling, and an absent
        // one is what silent truncation is made of.
        gen_config.insert("maxOutputTokens".into(), json!(req.max_tokens));
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
        // Unconditional: `maxOutputTokens` is always inserted above, so the
        // emptiness check that used to guard this could no longer fail. A guard
        // that cannot fire is worse than no guard — it reads as a condition.
        body["generationConfig"] = serde_json::Value::Object(gen_config);

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
            // P2-67/68. Classified here, where the body is available: a 429
            // from a spend cap and a 429 from a rate limit are the same
            // status, and only the body separates them.
            let class = crate::provider::usage::ErrorClass::classify(
                "google",
                status.as_u16(),
                &text,
            );
            return Err(AutoDevError::Provider(format!(
                "Google {status} [{class:?}]: {text}"
            )));
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
            // P2-67/68. The class is attached rather than left to be
            // re-derived downstream: a 429 from a spend cap and a 429 from a
            // rate limit are the same status and only the body tells them
            // apart, so the one place that has the body is the place to decide.
            let class = crate::provider::usage::ErrorClass::classify(
                "google",
                status.as_u16(),
                &text,
            );
            return Err(AutoDevError::Provider(format!(
                "Google stream {status} [{class:?}]: {text}"
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
                    chunks.extend(parse_google_sse_line(&line)?);
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
        // ⚠ D-10 (P2 post-phase audit). This used to POST a `generateContent`
        // call to a hardcoded `gemini-2.5-flash` — a model this tree's own
        // `the_retired_generation_is_gone` test lists as withdrawn. So key
        // validation 404'd for every user, and since `P2-23` now refuses to
        // store a key the provider rejects, a **valid Google key could not be
        // saved at all** — on the provider chosen as primary for rollout 1.
        //
        // It was also a billed completion, while `vault_commands` told the user
        // validation is "a models list, not a completion. It costs nothing",
        // and `capped.rs` exempts `test_connection` from the daily ceiling
        // *because* of that claim. Two documents describing behaviour the code
        // did not have.
        //
        // Now the models-list endpoint, which is free, needs no model id, and
        // is what the other two adapters already used. A validation call that
        // names a specific model can only rot again.
        //
        // ⚠ The key goes in a HEADER, never the URL (P2 pre-build audit).
        let url = format!("{GOOGLE_API_BASE}/v1beta/models");

        match self
            .client
            .get(&url)
            .header("x-goog-api-key", &self.api_key)
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => Ok(ProviderStatus::Connected),
            // ⚠ 400 too, not just 401/403.
            //
            // Google answers a malformed API key on the models endpoint with
            // **400 Bad Request**, where the other two return 401. Measured
            // live: without 400 here the user sees "Google returned 400 Bad
            // Request" for a mistyped key — technically accurate and useless,
            // when the actionable answer is that the key is wrong.
            Ok(r) if r.status() == 400 || r.status() == 401 || r.status() == 403 => {
                Ok(ProviderStatus::Error("Invalid API key".into()))
            }
            Ok(r) => {
                let code = r.status();
                Ok(ProviderStatus::Error(format!("Google returned {code}")))
            }
            Err(e) => {
                // ⚠ Never interpolate the reqwest error here. Its Display
                // includes the request URL, and that is how a key in a query
                // string reached the renderer before (P2 pre-build audit).
                // The URL carries no key now, but the habit is the guard.
                error!("Google connection test failed: {e}");
                Ok(ProviderStatus::Error("Connection failed".into()))
            }
        }
    }
}

fn parse_google_sse_line(line: &str) -> Result<Vec<StreamChunk>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(Vec::new());
    }
    let data = match line.strip_prefix("data: ") {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => parse_google_chunk(&v),
        Err(e) => Err(AutoDevError::Provider(format!(
            "SSE parse error: {e} — data: {data}"
        ))),
    }
}

/// Turn one Gemini SSE payload into every chunk it carries.
///
/// **Returns a `Vec`, and that is the fix.** The previous version returned at
/// most one chunk and `return`ed on the first matching part, which produced
/// three defects at once (`P2-34`, `P2-35`, `P2-44`):
///
/// 1. **Tool arguments were discarded.** `let _args = ...` computed them and
///    dropped them, and no `ToolInputDelta` was ever emitted for Google. The
///    engine then assembled the call from an empty accumulator, `from_str("")`
///    failed, and `.unwrap_or` substituted `{}` — so **every** Gemini tool call
///    ran with empty input, not merely parallel ones.
/// 2. **Parallel calls were dropped.** Gemini emits them as several
///    `functionCall` parts of one candidate; returning on the first discarded
///    the rest.
/// 3. **The usage handler was unreachable.** Both routes to it were blocked —
///    a chunk with no `parts` returned early, and a chunk with text returned on
///    the first text part — so `promptTokenCount` never arrived and token
///    counts stayed zero.
fn parse_google_chunk(v: &serde_json::Value) -> Result<Vec<StreamChunk>> {
    let mut out = Vec::new();

    let candidates = match v["candidates"].as_array() {
        Some(c) => c,
        None => return Ok(out),
    };
    let Some(candidate) = candidates.first() else {
        return Ok(out);
    };

    // `parts` may be absent on a pure finish/usage frame. That is not a reason
    // to return — the usage below is exactly what such a frame carries.
    if let Some(parts) = candidate["content"]["parts"].as_array() {
        for (idx, part) in parts.iter().enumerate() {
            if let Some(text) = part["text"].as_str() {
                out.push(StreamChunk::TextDelta {
                    text: text.to_string(),
                    block_index: idx as u64,
                });
                continue;
            }

            if let Some(func_call) = part["functionCall"].as_object() {
                let name = func_call
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = func_call.get("args").cloned().unwrap_or_else(|| json!({}));

                // The part index is the block index. Hardcoding 0 made two
                // calls in one message collide in the engine's correlation map,
                // and made the id collide whenever one tool was called twice.
                let block_index = idx as u64;
                // ⚠ Captured, not ignored. Gemini attaches a `thoughtSignature`
                // to the PART (not to the functionCall object) and **rejects
                // the turn with a 400** when a replayed call is missing it:
                //
                //   "Function call is missing a thought_signature in
                //    functionCall parts. This is required for tools to work
                //    correctly."
                //
                // Dropping it broke every multi-turn Gemini tool conversation
                // on the second request. Found by the live smoke; no fixture
                // could have caught it, because the captured payloads predate
                // the requirement and go on passing forever.
                let opaque = part
                    .get("thoughtSignature")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);

                out.push(StreamChunk::ToolUseStart {
                    id: format!("gemini-{name}-{block_index}"),
                    name,
                    block_index,
                    opaque,
                });

                // Gemini sends complete calls rather than deltas, so the whole
                // argument object goes in one delta. `ToolInputDelta` already
                // carries it — no enum change is needed, and the engine's
                // accumulator handles it identically to Anthropic's partials.
                out.push(StreamChunk::ToolInputDelta {
                    block_index,
                    partial_json: args.to_string(),
                });
            }
        }
    }

    if let Some(finish_reason) = candidate["finishReason"].as_str() {
        let usage = &v["usageMetadata"];
        let prompt_total = usage["promptTokenCount"].as_u64().unwrap_or(0) as u32;
        let cached_read = usage["cachedContentTokenCount"].as_u64().unwrap_or(0) as u32;
        let output = usage["candidatesTokenCount"].as_u64().unwrap_or(0) as u32;
        // Billed but not part of output. Never read before this change.
        let reasoning = usage["thoughtsTokenCount"].as_u64().unwrap_or(0) as u32;

        // Emitted even when every count is zero: the stop reason is the only
        // signal that distinguishes a `max_tokens` truncation from a clean
        // finish, and gating it on a non-zero token count — as the previous
        // version did — threw that away whenever usage was absent.
        out.push(StreamChunk::MessageDelta {
            stop_reason: finish_reason.to_string(),
            usage: Usage::from_inclusive_input(prompt_total, cached_read, 0, output).with_reasoning(reasoning),
        });
    }

    Ok(out)
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
    // INCLUSIVE vendor: `promptTokenCount` contains the cached tokens.
    let usage = Usage::from_inclusive_input(
        usage_metadata["promptTokenCount"].as_u64().unwrap_or(0) as u32,
        usage_metadata["cachedContentTokenCount"].as_u64().unwrap_or(0) as u32,
        0,
        usage_metadata["candidatesTokenCount"].as_u64().unwrap_or(0) as u32,
    )
    .with_reasoning(usage_metadata["thoughtsTokenCount"].as_u64().unwrap_or(0) as u32);

    let mut content = Vec::new();
    if let Some(candidate) = candidates.get(0) {
        if let Some(parts) = candidate["content"]["parts"].as_array() {
            for (idx, part) in parts.iter().enumerate() {
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
                        // ⚠ F-4. The part index is part of the id.
                        //
                        // P2-36 added it on the STREAMING path and left this
                        // one at `gemini-{name}`, so two calls to the same tool
                        // in one unary response shared an id — and the engine
                        // correlates results to calls by id, so one result
                        // would answer both calls and the other would go
                        // unanswered. "Read these two files" is the everyday
                        // shape that triggers it.
                        id: format!("gemini-{}-{}", name, idx),
                        name,
                        input: args,
                        // Must be echoed back verbatim on the next request or
                        // Gemini rejects the turn — see the streaming path.
                        opaque: part
                            .get("thoughtSignature")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                    });
                }
            }
        }
    }

    Ok(CompletionResponse {
        message_id: "".to_string(),
        content,
        stop_reason,
        usage,
        model: "gemini".to_string(),
    })
}

#[cfg(test)]
#[path = "google_tests.rs"]
mod google_tests;
