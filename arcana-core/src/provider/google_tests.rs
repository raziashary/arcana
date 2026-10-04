// Adapter tests for the Google/Gemini parser (P2-67, P2-68).
//
// These are the first tests any adapter in this tree has ever had. Measured at
// the start of P2: **1,733 lines across four adapters, zero `#[test]`.** Every
// defect below survived to production because nothing could catch it.
//
// Fixtures are the payload shapes Gemini actually emits, held as literals so
// the suite runs in CI with no credentials — `§0.5`: a gate skipped in CI is
// not green. The live smoke that catches vendor wire drift is `P2-69`, behind a
// cargo feature.

use super::*;
use crate::provider::types::RequestMessage;

#[test]
fn count_tokens_error_summary_keeps_only_safe_status_and_field_paths() {
    let summary = safe_google_error_summary(
        br#"{"error":{"code":400,"status":"INVALID_ARGUMENT","message":"private prompt must never leak","details":[{"@type":"type.googleapis.com/google.rpc.BadRequest","fieldViolations":[{"field":"generateContentRequest.tools[0]","description":"private tool schema must never leak"},{"field":"unsafe\nfield"}]}]}}"#,
    );
    assert_eq!(
        summary,
        "INVALID_ARGUMENT fields=generateContentRequest.tools[0]"
    );
    assert!(!summary.contains("private"));
    assert!(!summary.contains("unsafe"));
}

#[tokio::test]
async fn count_tokens_uses_the_exact_generation_body_before_generation() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind countTokens fixture");
    let address = listener.local_addr().expect("fixture address");
    let server = tokio::task::spawn_blocking(move || {
        let (mut stream, _) = listener.accept().expect("accept countTokens request");
        let mut reader = BufReader::new(stream.try_clone().expect("clone fixture stream"));
        let mut request_line = String::new();
        reader
            .read_line(&mut request_line)
            .expect("read HTTP request line");
        let mut content_length = 0usize;
        let mut saw_api_key = false;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("read HTTP header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().expect("content length");
                }
                if name.eq_ignore_ascii_case("x-goog-api-key") && value.trim() == "test-secret" {
                    saw_api_key = true;
                }
            }
        }
        let mut body = vec![0; content_length];
        reader
            .read_exact(&mut body)
            .expect("read JSON request body");
        let status_body = r#"{"totalTokens":321}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_body.len(),
            status_body
        )
        .expect("write countTokens response");
        stream.flush().expect("flush fixture response");
        (request_line, saw_api_key, body)
    });

    let request = CompletionRequest {
        model: "gemini-3.8-flash".to_string(),
        messages: vec![RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "count this exact prompt".to_string(),
            }],
        }],
        system: Some("system contract".to_string()),
        tools: vec![crate::provider::types::ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a workspace file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {"path": {"type": "string"}}
            }),
        }],
        max_tokens: 2048,
        temperature: Some(0.2),
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: Some(512),
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    };
    let mut expected_generation_body =
        GoogleProvider::new("test-secret".to_string()).build_body_for_test(&request, false);
    expected_generation_body["model"] = serde_json::json!("models/gemini-3.8-flash");
    let provider = GoogleProvider::new_at("test-secret".to_string(), format!("http://{address}"));
    assert_eq!(
        provider
            .input_token_upper_bound(&request)
            .await
            .expect("countTokens response"),
        321
    );

    let (request_line, saw_api_key, body) = server.await.expect("fixture task");
    assert!(request_line.contains("/v1beta/models/gemini-3.8-flash:countTokens"));
    assert!(
        !request_line.contains("test-secret"),
        "API key stays out of URL"
    );
    assert!(saw_api_key, "API key uses the dedicated request header");
    let request_body: serde_json::Value = serde_json::from_slice(&body).expect("JSON body");
    assert_eq!(
        request_body["generateContentRequest"],
        expected_generation_body
    );
    assert_eq!(
        request_body["generateContentRequest"]["model"],
        "models/gemini-3.8-flash"
    );
}

#[tokio::test]
async fn count_tokens_rejects_invalid_response_without_exposing_body() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind countTokens fixture");
    let address = listener.local_addr().expect("fixture address");
    let server = tokio::task::spawn_blocking(move || {
        let (mut stream, _) = listener.accept().expect("accept countTokens request");
        let mut reader = BufReader::new(stream.try_clone().expect("clone fixture stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request line");
        loop {
            line.clear();
            reader.read_line(&mut line).expect("read header");
            if line == "\r\n" || line.is_empty() {
                break;
            }
        }
        let mut drain = [0u8; 2048];
        let _ = reader.read(&mut drain);
        let status_body = r#"{"provider_private_text":"do not leak"}"#;
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status_body.len(),
            status_body
        )
        .expect("write invalid response");
        stream.flush().expect("flush fixture response");
    });

    let mut request = CompletionRequest {
        model: "gemini-3.8-flash".to_string(),
        messages: vec![],
        system: None,
        tools: vec![],
        max_tokens: 128,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    };
    request.system = Some("private request".into());
    let provider = GoogleProvider::new_at("test-secret".to_string(), format!("http://{address}"));
    let error = provider
        .input_token_upper_bound(&request)
        .await
        .expect_err("missing totalTokens must fail closed");
    assert!(!error.to_string().contains("do not leak"));
    server.await.expect("fixture task");
}

#[tokio::test]
async fn count_tokens_stops_reading_an_oversized_response_at_the_bound() {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").expect("bind oversized response fixture");
    let address = listener.local_addr().expect("fixture address");
    let server = tokio::task::spawn_blocking(move || {
        let (mut stream, _) = listener.accept().expect("accept countTokens request");
        let mut reader = BufReader::new(stream.try_clone().expect("clone fixture stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request line");
        let mut content_length = 0usize;
        loop {
            line.clear();
            reader.read_line(&mut line).expect("read request headers");
            if line == "\r\n" || line.is_empty() {
                break;
            }
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("content-length") {
                    content_length = value.trim().parse().expect("content length");
                }
            }
        }
        let mut request_body = vec![0; content_length];
        reader
            .read_exact(&mut request_body)
            .expect("read request JSON");
        let body = "x".repeat(MAX_COUNT_TOKENS_RESPONSE_BYTES + 1);
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.flush();
    });

    let request = CompletionRequest {
        model: "gemini-3.8-flash".into(),
        messages: vec![],
        system: None,
        tools: vec![],
        max_tokens: 128,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    };
    let provider = GoogleProvider::new_at("test-secret".into(), format!("http://{address}"));
    let error = provider
        .input_token_upper_bound(&request)
        .await
        .expect_err("oversized response must be bounded");
    assert!(error.to_string().contains("safe response bound"));
    server.await.expect("fixture task");
}

#[test]
fn native_thought_parts_are_not_emitted_as_visible_text() {
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"text":"private thought","thought":true},{"text":"visible answer"}]}}]}"#,
    ))
    .expect("parse");
    assert!(matches!(
        &chunks[0],
        StreamChunk::ThinkingDelta { text, .. } if text == "private thought"
    ));
    assert!(matches!(
        &chunks[1],
        StreamChunk::TextDelta { text, .. } if text == "visible answer"
    ));
}

/// One SSE `data:` line, as Gemini sends it.
fn sse(payload: &str) -> String {
    format!("data: {payload}")
}

#[test]
fn a_tool_call_carries_its_arguments() {
    // 🔴 The defect this file exists for. `google.rs` computed the arguments
    // and dropped them (`let _args = ...`), and never emitted a
    // `ToolInputDelta`. The engine then assembled the call from an empty
    // accumulator, `from_str("")` failed, and `.unwrap_or` substituted `{}` —
    // so EVERY Gemini tool call ran with empty input, not merely parallel ones.
    // Gemini is R1's primary provider.
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read_file","args":{"path":"src/main.rs","limit":40}}}]}}]}"#,
    ))
    .expect("parse");

    let start = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::ToolUseStart { .. }))
        .expect("a ToolUseStart");
    let StreamChunk::ToolUseStart { name, .. } = start else {
        unreachable!()
    };
    assert_eq!(name, "read_file");

    let delta = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::ToolInputDelta { .. }))
        .expect("a ToolInputDelta must follow — this is the whole fix");
    let StreamChunk::ToolInputDelta { partial_json, .. } = delta else {
        unreachable!()
    };
    let parsed: serde_json::Value = serde_json::from_str(partial_json).expect("valid json");
    assert_eq!(parsed["path"], "src/main.rs");
    assert_eq!(parsed["limit"], 40);
}

#[test]
fn parallel_tool_calls_all_survive() {
    // Gemini emits parallel calls as several `functionCall` parts of ONE
    // candidate. The old parser `return`ed on the first matching part, so
    // every call after the first was silently dropped.
    let chunks = parse_google_sse_line(&sse(r#"{"candidates":[{"content":{"parts":[
            {"functionCall":{"name":"read_file","args":{"path":"a.rs"}}},
            {"functionCall":{"name":"read_file","args":{"path":"b.rs"}}},
            {"functionCall":{"name":"grep","args":{"q":"fn main"}}}
        ]}}]}"#))
    .expect("parse");

    let starts: Vec<_> = chunks
        .iter()
        .filter_map(|c| match c {
            StreamChunk::ToolUseStart {
                id, block_index, ..
            } => Some((id.clone(), *block_index)),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 3, "all three calls must survive");

    // Each needs a distinct block_index or they collide in the engine's
    // correlation map, and a distinct id or the same tool called twice
    // overwrites itself.
    let indices: std::collections::HashSet<u64> = starts.iter().map(|(_, i)| *i).collect();
    assert_eq!(indices.len(), 3, "block_index was hardcoded to 0");

    let ids: std::collections::HashSet<String> = starts.iter().map(|(i, _)| i.clone()).collect();
    assert_eq!(
        ids.len(),
        3,
        "two calls to the same tool produced the same id"
    );

    // And every one keeps its own arguments.
    let deltas: Vec<_> = chunks
        .iter()
        .filter_map(|c| match c {
            StreamChunk::ToolInputDelta { partial_json, .. } => Some(partial_json.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas.len(), 3);
    assert!(deltas.iter().any(|d| d.contains("a.rs")));
    assert!(deltas.iter().any(|d| d.contains("b.rs")));
    assert!(deltas.iter().any(|d| d.contains("fn main")));
}

#[test]
fn text_and_a_tool_call_in_one_chunk_both_survive() {
    // The old loop returned on the first text part, so a tool call sharing the
    // chunk was lost entirely.
    let chunks = parse_google_sse_line(&sse(r#"{"candidates":[{"content":{"parts":[
            {"text":"Let me look."},
            {"functionCall":{"name":"read_file","args":{"path":"x.rs"}}}
        ]}}]}"#))
    .expect("parse");

    assert!(chunks
        .iter()
        .any(|c| matches!(c, StreamChunk::TextDelta { .. })));
    assert!(chunks
        .iter()
        .any(|c| matches!(c, StreamChunk::ToolUseStart { .. })));
}

#[test]
fn usage_arrives_on_a_frame_that_has_no_parts() {
    // The usage handler was unreachable by two routes. This is the first: a
    // pure finish/usage frame has no `parts`, and the old parser returned
    // `None` before ever reaching the `finishReason` check thirty lines below.
    let chunks = parse_google_sse_line(&sse(r#"{"candidates":[{"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":1000,"candidatesTokenCount":50,
                             "cachedContentTokenCount":900,"thoughtsTokenCount":7}}"#))
    .expect("parse");

    let delta = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::MessageDelta { .. }))
        .expect("usage must reach the engine");
    let StreamChunk::MessageDelta { usage, stop_reason } = delta else {
        unreachable!()
    };
    assert_eq!(stop_reason, "STOP");
    // Google reports a prompt total that INCLUDES the cached tokens, so the
    // non-cached remainder is 100 and the total reconstructs to 1000.
    assert_eq!(usage.input, 100);
    assert_eq!(usage.cached_read, 900);
    assert_eq!(usage.total_input(), 1000);
    assert_eq!(usage.output, 50);
    // Billed, and never read before this change.
    assert_eq!(usage.reasoning, 7);
}

#[test]
fn usage_also_arrives_when_the_frame_carries_text() {
    // The second blocked route: a chunk with both text and a finishReason.
    // The old loop returned on the text part and the usage was never read.
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"text":"done"}]},"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":2}}"#,
    ))
    .expect("parse");

    assert!(chunks
        .iter()
        .any(|c| matches!(c, StreamChunk::TextDelta { .. })));
    assert!(
        chunks
            .iter()
            .any(|c| matches!(c, StreamChunk::MessageDelta { .. })),
        "a chunk carrying text AND a finish reason must still report usage"
    );
}

#[test]
fn a_truncated_turn_is_distinguishable_from_a_clean_one() {
    // P2-46. With `stop_reason` discarded, a MAX_TOKENS truncation and a clean
    // STOP were the same code path, so silent truncation was invisible.
    for (reason, payload) in [("MAX_TOKENS", "MAX_TOKENS"), ("STOP", "STOP")] {
        let chunks = parse_google_sse_line(&sse(&format!(
            r#"{{"candidates":[{{"finishReason":"{payload}"}}],
                 "usageMetadata":{{"promptTokenCount":5,"candidatesTokenCount":1}}}}"#
        )))
        .expect("parse");
        let StreamChunk::MessageDelta { stop_reason, .. } = chunks
            .iter()
            .find(|c| matches!(c, StreamChunk::MessageDelta { .. }))
            .expect("a MessageDelta")
        else {
            unreachable!()
        };
        assert_eq!(stop_reason, reason);
    }
}

#[test]
fn usage_is_reported_even_when_every_count_is_absent() {
    // The old code gated MessageDelta on a non-zero token count, so a finish
    // frame without usage threw the stop reason away with it.
    let chunks = parse_google_sse_line(&sse(r#"{"candidates":[{"finishReason":"SAFETY"}]}"#))
        .expect("parse");
    let StreamChunk::MessageDelta { stop_reason, usage } = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::MessageDelta { .. }))
        .expect("the stop reason must survive a missing usage object")
    else {
        unreachable!()
    };
    assert_eq!(stop_reason, "SAFETY");
    assert_eq!(usage.total(), 0);
}

#[test]
fn non_data_lines_and_malformed_payloads_are_handled() {
    assert!(parse_google_sse_line("").expect("empty").is_empty());
    assert!(parse_google_sse_line(": keep-alive")
        .expect("comment")
        .is_empty());
    assert!(parse_google_sse_line("event: message")
        .expect("non-data")
        .is_empty());
    // A malformed payload is an error, not a silent empty — a parser that
    // swallows garbage cannot report a vendor changing its wire format.
    assert!(parse_google_sse_line(&sse("{not json")).is_err());
}

#[test]
fn an_empty_candidate_list_yields_nothing_rather_than_panicking() {
    assert!(parse_google_sse_line(&sse(r#"{"candidates":[]}"#))
        .expect("parse")
        .is_empty());
    assert!(parse_google_sse_line(&sse(r#"{}"#))
        .expect("parse")
        .is_empty());
}

#[test]
fn a_tool_calls_thought_signature_survives_the_round_trip() {
    // 🔴 Found live on 2026-09-02, and unfindable from fixtures alone.
    //
    // The signature is attached to the part, not the functionCall object.
    // Gemini requires it on the first call in a tool-call step:
    //
    //   "Function call is missing a thought_signature in functionCall parts.
    //    This is required for tools to work correctly."
    //
    // We parsed the call and dropped the signature, so every multi-turn Gemini
    // tool conversation failed on its SECOND request — and Gemini is the
    // primary provider for rollout 1. The captured payloads in this file
    // predate the requirement, so they went on passing forever; only the live
    // smoke could see it.
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read_file","args":{"path":"a.rs"}},"thoughtSignature":"CtEBAVKm-sig-xyz"}]}}]}"#,
    ))
    .expect("parse");

    let start = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::ToolUseStart { .. }))
        .expect("a ToolUseStart");
    let StreamChunk::ToolUseStart { opaque, .. } = start else {
        unreachable!()
    };
    assert_eq!(
        opaque.as_deref(),
        Some("CtEBAVKm-sig-xyz"),
        "the signature must be captured, or the next request is rejected outright"
    );
}

#[test]
fn snake_case_thought_signature_is_normalized_and_replayed_on_the_part() {
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"read_file","args":{"path":"a.rs"}},"thought_signature":"opaque-token"}]}}]}"#,
    ))
    .expect("parse");
    let StreamChunk::ToolUseStart { opaque, .. } = chunks
        .iter()
        .find(|chunk| matches!(chunk, StreamChunk::ToolUseStart { .. }))
        .expect("tool call")
    else {
        unreachable!()
    };
    assert_eq!(opaque.as_deref(), Some("opaque-token"));

    let request = CompletionRequest {
        model: "test-model".to_string(),
        messages: vec![RequestMessage {
            role: "assistant".to_string(),
            content: vec![MessageContent::ToolUse {
                id: "call-1".to_string(),
                name: "read_file".to_string(),
                input: serde_json::json!({"path":"a.rs"}),
                opaque: Some("opaque-token".to_string()),
            }],
        }],
        system: None,
        tools: vec![],
        max_tokens: 1024,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    };
    let body = GoogleProvider::new(String::new()).build_body_for_test(&request, false);
    let replayed_part = &body["contents"][0]["parts"][0];
    assert_eq!(replayed_part["thoughtSignature"], "opaque-token");
    assert!(replayed_part["functionCall"]["thoughtSignature"].is_null());
}

#[test]
fn a_call_without_a_signature_is_still_parsed() {
    // Not every functionCall carries one, and a missing signature must not
    // drop the call — the model asked for a tool either way.
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"grep","args":{"pattern":"fn"}}}]}}]}"#,
    ))
    .expect("parse");

    let start = chunks
        .iter()
        .find(|c| matches!(c, StreamChunk::ToolUseStart { .. }))
        .expect("the call must still be parsed");
    let StreamChunk::ToolUseStart { opaque, name, .. } = start else {
        unreachable!()
    };
    assert_eq!(name, "grep");
    assert!(opaque.is_none(), "absent is not the same as empty");
}
