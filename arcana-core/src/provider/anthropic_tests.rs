// Adapter tests for the Anthropic SSE parser (P2-67, P2-68).
//
// Anthropic's streaming path had no test of any kind, and it is the PRIMARY
// path: `complete()` is used for short one-shot calls, but every agent turn
// goes through `stream()`. The unary response parser was covered indirectly by
// the conformance suite; this covers the event stream.
//
// Anthropic's stream is event-typed rather than delta-typed, which makes it the
// odd one of the three: usage arrives split across TWO frames (`message_start`
// carries the input side, `message_delta` the output side), and tool arguments
// arrive as a `partial_json` string that must be concatenated. Both are places
// where "it looks right" and "it works" come apart.
//
// Fixtures are the shapes Anthropic actually emits, held as literals so this
// runs in CI with no credentials (§0.5).

use super::streaming::parse_sse_line;
use super::types::{MessageContent, StreamChunk};

fn sse(payload: &str) -> String {
    format!("data: {payload}")
}

#[test]
fn unsigned_thinking_is_rejected_before_an_outbound_tool_follow_up() {
    use super::anthropic::AnthropicProvider;
    use super::types::{CompletionRequest, RequestMessage};

    let mut request = CompletionRequest {
        model: "claude-haiku-4-5".into(),
        messages: vec![RequestMessage {
            role: "assistant".into(),
            content: vec![
                MessageContent::Thinking {
                    text: "reason".into(),
                    signature: None,
                },
                MessageContent::ToolUse {
                    id: "toolu_1".into(),
                    name: "read_file".into(),
                    input: serde_json::json!({"path":"package.json"}),
                    opaque: None,
                },
            ],
        }],
        system: None,
        tools: vec![],
        max_tokens: 64,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: Some(32),
        cache: Default::default(),
        clear_tool_uses: None,
    };
    let provider = AnthropicProvider::new("unused".into());
    assert!(provider
        .build_checked_body_for_test(&request, true)
        .is_err());
    if let MessageContent::Thinking { signature, .. } = &mut request.messages[0].content[0] {
        *signature = Some(String::new());
    }
    assert!(provider
        .build_checked_body_for_test(&request, true)
        .is_err());
}

#[test]
fn unary_thinking_block_keeps_its_signature() {
    let response = super::anthropic::parse_completion_response(serde_json::json!({
        "id": "msg_1", "model": "claude-haiku-4-5", "stop_reason": "tool_use",
        "usage": {"input_tokens": 1, "output_tokens": 1},
        "content": [
            {"type":"thinking","thinking":"reason","signature":"opaque-sig"},
            {"type":"tool_use","id":"toolu_1","name":"read_file","input":{}},
        ],
    }))
    .expect("unary response");
    assert!(
        matches!(&response.content[0], MessageContent::Thinking { text, signature }
        if text == "reason" && signature.as_deref() == Some("opaque-sig"))
    );
}

#[test]
fn message_start_carries_the_entire_input_side_of_usage() {
    // 🔴 P2-43. This frame was falling through to `_ => Ok(None)`. It is the
    // ONLY frame carrying `input_tokens`, `cache_read_input_tokens` and
    // `cache_creation_input_tokens` — `message_delta` reports output only.
    //
    // Swallowing it meant every streaming call summed ZERO input tokens: cost
    // under-reported, a zero input column written to analytics, and the cache
    // alarm left with no instrument at all, since cached_read arrives here and
    // nowhere else.
    let chunk = parse_sse_line(&sse(
        r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":1200,"cache_read_input_tokens":8000,"cache_creation_input_tokens":150,"output_tokens":0}}}"#,
    ))
    .expect("parse")
    .expect("message_start must produce a chunk");

    let StreamChunk::MessageDelta { usage, stop_reason } = chunk else {
        panic!("expected MessageDelta, got {chunk:?}");
    };
    assert_eq!(usage.input, 1200);
    assert_eq!(
        usage.cached_read, 8000,
        "the cache alarm reads this and nothing else"
    );
    assert_eq!(usage.cache_write, 150);
    assert_eq!(usage.output, 0, "output is not known on this frame");
    assert!(
        stop_reason.is_empty(),
        "message_start opens the message — a stop reason here would end the turn early"
    );
}

#[test]
fn anthropic_excludes_cached_tokens_from_input_unlike_the_other_two() {
    // The normalisation that makes the six-field Usage comparable across
    // vendors (P2-01). Anthropic reports `input_tokens` ALREADY EXCLUDING
    // cached reads; OpenAI and Google include them. `total_input()` is what
    // both shapes agree on.
    let chunk = parse_sse_line(&sse(
        r#"{"type":"message_start","message":{"id":"m","usage":{"input_tokens":300,"cache_read_input_tokens":9000,"cache_creation_input_tokens":0,"output_tokens":0}}}"#,
    ))
    .expect("parse")
    .expect("chunk");
    let StreamChunk::MessageDelta { usage, .. } = chunk else {
        panic!("expected MessageDelta");
    };

    // Taken verbatim — no subtraction, because Anthropic already did it.
    assert_eq!(usage.input, 300);
    assert_eq!(usage.total_input(), 9_300, "300 fresh + 9,000 cached");
}

#[test]
fn tool_arguments_arrive_as_partial_json_and_must_concatenate() {
    // Anthropic streams tool input as a string split across arbitrary
    // boundaries — frequently mid-token, sometimes mid-UTF8-character. A parser
    // that tried to `serde_json::from_str` each delta would fail on every one
    // of them; the deltas have to be accumulated and parsed once at the end.
    let start = parse_sse_line(&sse(
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"read_file","input":{}}}"#,
    ))
    .expect("parse")
    .expect("a ToolUseStart");

    let StreamChunk::ToolUseStart {
        id,
        name,
        block_index,
        ..
    } = start
    else {
        panic!("expected ToolUseStart, got {start:?}");
    };
    assert_eq!(id, "toolu_01");
    assert_eq!(name, "read_file");
    assert_eq!(
        block_index, 1,
        "the wire index is what orders the assembled blocks"
    );

    let mut accumulated = String::new();
    for fragment in [r#"{\"pa"#, r#"th\": \"src/m"#, r#"ain.rs\"}"#] {
        let payload = format!(
            r#"{{"type":"content_block_delta","index":1,"delta":{{"type":"input_json_delta","partial_json":"{fragment}"}}}}"#
        );
        let chunk = parse_sse_line(&sse(&payload))
            .expect("parse")
            .expect("an input delta");
        let StreamChunk::ToolInputDelta { partial_json, .. } = chunk else {
            panic!("expected ToolInputDelta, got {chunk:?}");
        };
        accumulated.push_str(&partial_json);
    }

    // Only now is it valid JSON. This is the assertion that matters: each
    // fragment on its own is not parseable.
    let parsed: serde_json::Value =
        serde_json::from_str(&accumulated).expect("the concatenation must parse");
    assert_eq!(parsed["path"], serde_json::json!("src/main.rs"));
}

#[test]
fn a_truncated_turn_is_distinguishable_from_a_clean_one() {
    // P2-10/P2-46. `max_tokens` here means the assistant message is cut — and
    // possibly cut mid-`tool_use`, leaving arguments that will not parse.
    // Continuing the loop over it feeds a malformed message back to the model.
    for (wire, expect_truncated) in [
        ("max_tokens", true),
        ("end_turn", false),
        ("tool_use", false),
    ] {
        let chunk = parse_sse_line(&sse(&format!(
            r#"{{"type":"message_delta","delta":{{"stop_reason":"{wire}"}},"usage":{{"output_tokens":42}}}}"#
        )))
        .expect("parse")
        .expect("a MessageDelta");

        let StreamChunk::MessageDelta { stop_reason, usage } = chunk else {
            panic!("expected MessageDelta");
        };
        assert_eq!(
            stop_reason, wire,
            "the raw wire value is preserved verbatim"
        );
        assert_eq!(usage.output, 42);
        // D-9. The input side belongs to `message_start` alone.
        assert_eq!(usage.input, 0);
        assert_eq!(usage.cached_read, 0);
        assert_eq!(usage.cache_write, 0);
        assert_eq!(
            super::usage::StopReason::parse("anthropic", &stop_reason).is_truncated(),
            expect_truncated,
            "{wire} classified wrongly"
        );
    }
}

#[test]
fn text_deltas_carry_their_block_index() {
    // Two text blocks in one message must stay distinguishable, or interleaved
    // deltas concatenate into the wrong block (`assemble_in_wire_order`).
    let chunk = parse_sse_line(&sse(
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#,
    ))
    .expect("parse")
    .expect("a text delta");

    let StreamChunk::TextDelta { text, block_index } = chunk else {
        panic!("expected TextDelta, got {chunk:?}");
    };
    assert_eq!(text, "Hello");
    assert_eq!(block_index, 0);
}

#[test]
fn sse_metadata_lines_are_ignored_rather_than_parsed() {
    // Anthropic sends `event:` lines before every `data:` line, plus keepalive
    // comments and blank separators. Each must yield None, not an error —
    // erroring on a keepalive would abort a working stream.
    for line in [
        "",
        "   ",
        ": keepalive",
        "event: content_block_delta",
        "event: message_stop",
        "id: 1",
    ] {
        assert!(
            parse_sse_line(line).expect("must not error").is_none(),
            "{line:?} should be ignored"
        );
    }
}

#[test]
fn a_malformed_data_payload_errors_instead_of_being_swallowed() {
    // The opposite failure: a `data:` line that is not JSON is a real problem
    // and must surface. Returning None here would silently drop content.
    let err = parse_sse_line(&sse("{\"private-prompt-marker\": not json"));
    assert!(
        err.is_err(),
        "a malformed data payload must not be silently dropped"
    );
    assert!(!err
        .unwrap_err()
        .to_string()
        .contains("private-prompt-marker"));
}

#[test]
fn the_done_sentinel_ends_the_stream() {
    let chunk = parse_sse_line("data: [DONE]")
        .expect("parse")
        .expect("a chunk");
    assert!(matches!(chunk, StreamChunk::Done));
}

#[test]
fn usage_absent_from_a_frame_reads_as_zero_rather_than_failing_the_turn() {
    // Not every frame carries usage. A missing field must not abort the stream
    // — but P2-52 means a turn that produced content and reported ZERO across
    // ALL frames is reported as an error at the engine level, which is where
    // the whole-turn view exists.
    let chunk = parse_sse_line(&sse(
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#,
    ))
    .expect("parse")
    .expect("a chunk");
    let StreamChunk::MessageDelta { usage, .. } = chunk else {
        panic!("expected MessageDelta");
    };
    assert_eq!(usage.output, 0);
    assert_eq!(usage.total(), 0);
}

#[test]
fn a_streamed_turn_does_not_double_count_its_input() {
    // 🔴 D-9, measured from the live wire on 2026-09-02. Anthropic's per-message
    // usage is CUMULATIVE, and it repeats the input side on both frames:
    //
    //   message_start  usage: {"input_tokens":9, "output_tokens":5, ...}
    //   message_delta  usage: {"input_tokens":9, "output_tokens":5, ...}
    //
    // The engine sums every MessageDelta, so reading the input side from both
    // reported 2× the real cost on every Anthropic streaming call — the primary
    // agent path. `complete()` was unaffected, which is why the unary
    // cost-reconciliation smoke passed the whole time.
    //
    // These are the exact bytes from that capture.
    let start = parse_sse_line(&sse(
        r#"{"type":"message_start","message":{"id":"msg_1","usage":{"input_tokens":9,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5}}}"#,
    ))
    .expect("parse")
    .expect("message_start");

    let delta = parse_sse_line(&sse(
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":9,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":5}}"#,
    ))
    .expect("parse")
    .expect("message_delta");

    let (StreamChunk::MessageDelta { usage: u1, .. }, StreamChunk::MessageDelta { usage: u2, .. }) =
        (start, delta)
    else {
        panic!("both frames must yield MessageDelta");
    };

    // What the engine actually does.
    let mut total = super::usage::Usage::default();
    total.add(&u1);
    total.add(&u2);

    assert_eq!(
        total.input, 9,
        "the turn used 9 input tokens; summing both frames reported {}",
        total.input
    );
    assert_eq!(total.output, 5, "output belongs to message_delta alone");
    assert_eq!(total.total_input(), 9);
}

#[test]
fn a_cached_streamed_turn_counts_its_cache_read_once() {
    // The same defect on the field the cache alarm reads. Double-counting
    // `cached_read` would also make a cached turn look twice as expensive as it
    // was — inverting the saving caching exists to produce.
    let start = parse_sse_line(&sse(
        r#"{"type":"message_start","message":{"id":"m","usage":{"input_tokens":15,"cache_read_input_tokens":11923,"cache_creation_input_tokens":0,"output_tokens":0}}}"#,
    ))
    .expect("parse")
    .expect("message_start");

    let delta = parse_sse_line(&sse(
        r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":15,"cache_read_input_tokens":11923,"cache_creation_input_tokens":0,"output_tokens":4}}"#,
    ))
    .expect("parse")
    .expect("message_delta");

    let (StreamChunk::MessageDelta { usage: u1, .. }, StreamChunk::MessageDelta { usage: u2, .. }) =
        (start, delta)
    else {
        panic!("both frames must yield MessageDelta");
    };

    let mut total = super::usage::Usage::default();
    total.add(&u1);
    total.add(&u2);

    assert_eq!(total.cached_read, 11_923, "counted once, not twice");
    assert_eq!(total.input, 15);
    assert_eq!(total.output, 4);
}

// ── P3-25 · the clear layer reaches the wire in the shape the API accepts ──

#[cfg(test)]
mod p3_25_wire_tests {
    use super::super::anthropic::AnthropicProvider;
    use crate::provider::types::{
        ClearToolUses, CompletionRequest, MessageContent, RequestMessage,
    };

    fn base() -> CompletionRequest {
        CompletionRequest {
            model: "claude-haiku-4-5-20251001".into(),
            messages: vec![RequestMessage {
                role: "user".into(),
                content: vec![MessageContent::Text { text: "hi".into() }],
            }],
            system: None,
            tools: vec![],
            max_tokens: 16,
            temperature: None,
            top_p: None,
            reasoning_effort: None,
            thinking_budget_tokens: None,
            cache: Default::default(),
            clear_tool_uses: None,
        }
    }

    #[test]
    fn the_body_matches_what_the_live_api_accepted() {
        // ⚠ These exact strings were verified against `api.anthropic.com` on
        // 2026-09-02 (`L-22` — ask the wire, not the docs). The same body with
        // the beta header returns 200; without it, 400
        // "context_management: Extra inputs are not permitted".
        let p = AnthropicProvider::new("test-key".into());
        let mut req = base();
        req.clear_tool_uses = Some(ClearToolUses {
            trigger_input_tokens: 120_000,
            keep_tool_uses: 3,
            clear_at_least_input_tokens: 10_000,
        });

        let body = p.build_body_for_test(&req, false);
        let edit = &body["context_management"]["edits"][0];

        assert_eq!(edit["type"], "clear_tool_uses_20250919");
        assert_eq!(edit["trigger"]["type"], "input_tokens");
        assert_eq!(edit["trigger"]["value"], 120_000);
        assert_eq!(edit["keep"]["type"], "tool_uses");
        assert_eq!(edit["keep"]["value"], 3);
        assert_eq!(edit["clear_at_least"]["type"], "input_tokens");
        assert_eq!(edit["clear_at_least"]["value"], 10_000);
    }

    #[test]
    fn a_request_without_a_clear_policy_sends_no_context_management() {
        // The no-fire half, and it is not cosmetic: the field is REFUSED
        // outright on any request that does not carry the beta header, so
        // emitting it unconditionally would 400 every call.
        let p = AnthropicProvider::new("test-key".into());
        let body = p.build_body_for_test(&base(), false);

        assert!(
            body.get("context_management").is_none(),
            "context_management must be absent when no policy is set — the API \
             rejects the field without the beta header, so an unconditional \
             emit would fail every request"
        );
    }

    #[tokio::test]
    async fn provider_error_body_is_not_forwarded_to_anthropic_consumers() {
        use super::super::anthropic::AnthropicProvider;
        use super::super::traits::Provider;
        use super::super::usage::{
            test_completion_request, test_error_server, test_response_server,
        };

        let body = r#"{"error":{"type":"rate_limit_error","message":"private rate-limit marker"}}"#;
        let (api_base, server) = test_error_server(body.to_string());
        let provider = AnthropicProvider::new_at("test-key".into(), api_base);
        let error = provider
            .complete(test_completion_request())
            .await
            .expect_err("provider HTTP error")
            .to_string();
        assert!(error.contains("[Retryable]"), "{error}");
        assert!(error.contains("provider_error_response"), "{error}");
        assert!(!error.contains("private rate-limit marker"), "{error}");
        server.await.expect("Anthropic fixture");

        let (api_base, server) = test_error_server(body.to_string());
        let provider = AnthropicProvider::new_at("test-key".into(), api_base);
        let error = provider
            .stream(test_completion_request())
            .await
            .err()
            .expect("provider stream HTTP error")
            .to_string();
        assert!(error.contains("[Retryable]"), "{error}");
        assert!(error.contains("provider_error_response"), "{error}");
        assert!(!error.contains("private rate-limit marker"), "{error}");
        server.await.expect("Anthropic stream fixture");

        let (api_base, server) =
            test_response_server("private-decode-marker".to_string(), 200, "OK");
        let provider = AnthropicProvider::new_at("test-key".into(), api_base);
        let error = provider
            .complete(test_completion_request())
            .await
            .expect_err("malformed provider response")
            .to_string();
        assert!(error.contains("provider_response_invalid"), "{error}");
        assert!(!error.contains("private-decode-marker"), "{error}");
        server.await.expect("Anthropic malformed-response fixture");

        let provider = AnthropicProvider::new_at(
            "test-key".into(),
            "http://127.0.0.1:9/private-transport-marker".into(),
        );
        let error = provider
            .complete(test_completion_request())
            .await
            .expect_err("unavailable provider")
            .to_string();
        assert!(error.contains("provider_transport_failure"), "{error}");
        assert!(error.contains("[Retryable]"), "{error}");
        assert!(!error.contains("private-transport-marker"), "{error}");
    }

    #[tokio::test]
    async fn adapter_http_failures_keep_class_and_fixed_summary() {
        use super::super::anthropic::AnthropicProvider;
        use super::super::traits::Provider;
        use super::super::usage::{test_completion_request, test_response_server};

        let cases = [
            (
                429,
                "Too Many Requests",
                r#"{"error":{"type":"rate_limit_error","message":"Per-minute request limit reached; retry shortly. private-rate-marker"}}"#,
                "Retryable",
            ),
            (
                429,
                "Too Many Requests",
                r#"{"error":{"type":"invalid_request_error","message":"Credit balance is too low. private-quota-marker"}}"#,
                "QuotaExhausted",
            ),
            (
                503,
                "Service Unavailable",
                r#"{"error":{"type":"api_error","message":"private-503-marker"}}"#,
                "Overloaded",
            ),
            (
                529,
                "Overloaded",
                r#"{"error":{"type":"overloaded_error","message":"private-529-marker"}}"#,
                "Overloaded",
            ),
            (
                500,
                "Internal Server Error",
                r#"{"error":{"type":"api_error","message":"private-500-marker"}}"#,
                "Retryable",
            ),
            (
                502,
                "Bad Gateway",
                r#"{"error":{"type":"api_error","message":"private-502-marker"}}"#,
                "Retryable",
            ),
            (
                504,
                "Gateway Timeout",
                r#"{"error":{"type":"api_error","message":"private-504-marker"}}"#,
                "Retryable",
            ),
        ];

        for (status, reason, body, class) in cases {
            for stream in [false, true] {
                let (api_base, server) = test_response_server(body.to_string(), status, reason);
                let provider = AnthropicProvider::new_at("test-key".into(), api_base);
                let error = if stream {
                    provider
                        .stream(test_completion_request())
                        .await
                        .err()
                        .expect("stream HTTP failure response")
                        .to_string()
                } else {
                    provider
                        .complete(test_completion_request())
                        .await
                        .err()
                        .expect("completion HTTP failure response")
                        .to_string()
                };

                let operation = if stream {
                    "Anthropic stream"
                } else {
                    "Anthropic"
                };
                assert!(
                    error.starts_with(&format!("Provider error: {operation} {status} ")),
                    "{error}"
                );
                assert!(
                    error.ends_with(&format!("[{class}]: provider_error_response")),
                    "{error}"
                );
                assert!(!error.contains("private-"), "{error}");
                server.await.expect("Anthropic HTTP error fixture");
            }
        }
    }
}
