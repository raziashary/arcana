// Adapter tests for the Groq wire path (P2-67/P2-68 discipline).
//
// Groq serves the OpenAI-compatible stream, so the parser tests exercise the
// same shared code the OpenAI fixtures test — but the fixtures here are the
// shapes **Groq's** documentation actually describes, including the ones that
// differ from OpenAI's real payloads:
//
//   * the end-of-stream usage chunk carries its statistics under
//     `x_groq.usage` (Groq SSE spec), not only top-level `usage`;
//   * Qwen thinking models deliver reasoning as a `reasoning` field when
//     `reasoning_format` is `parsed`, not `reasoning_content`.
//
// Fixtures are literals so this runs in CI with no credentials (§0.5). The
// live drift check for this adapter is a models-list smoke (free); running a
// paid completion against Groq is a separate owner-authorized act.

use super::{parse_groq_sse_line_for_test as parse, GroqProvider};
use crate::provider::{
    types::{CompletionRequest, MessageContent, RequestMessage, StreamChunk, ToolDefinition},
    Provider,
};

fn sse(payload: &str) -> String {
    format!("data: {payload}")
}

fn req() -> CompletionRequest {
    CompletionRequest {
        model: "qwen/qwen3.8-27b".to_string(),
        messages: vec![RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "hello".to_string(),
            }],
        }],
        system: Some("be brief".to_string()),
        tools: vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "read a file".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        max_tokens: 4096,
        temperature: None,
        top_p: None,
        reasoning_effort: None,
        thinking_budget_tokens: None,
        cache: crate::provider::usage::CachePolicy::Off,
        clear_tool_uses: None,
    }
}

// ── Request body ─────────────────────────────────────────────────────────────

#[test]
fn the_body_is_openai_compatible_without_openai_only_fields() {
    // `prompt_cache_key` is an OpenAI affinity hint. Groq's model page
    // documents no such field, and sending an undocumented field to an
    // OpenAI-compatible vendor risks a 400 for zero benefit. Even when the
    // engine asks for automatic-cache steering, Groq must not relay it.
    let mut request = req();
    request.cache = crate::provider::usage::CachePolicy::Automatic {
        affinity_key: "affinity".to_string(),
    };

    let body = GroqProvider::new(String::new()).build_body_for_test(&request, false);
    assert_eq!(body["model"], "qwen/qwen3.8-27b");
    assert_eq!(body["max_completion_tokens"], 4096);
    assert!(
        body.get("prompt_cache_key").is_none(),
        "the OpenAI affinity hint must not be sent to Groq"
    );
}

#[test]
fn a_streaming_body_asks_for_usage_the_openai_compatible_way() {
    // `stream_options.include_usage` IS in Groq's API reference and makes the
    // final empty-choices usage frame arrive. Without it the stream carries
    // no usage object at all — the same trap P2-42 documented for OpenAI.
    let body = GroqProvider::new(String::new()).build_body_for_test(&req(), true);
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"]["include_usage"], true);
}

#[test]
fn reasoning_effort_passes_through_unchanged() {
    // Groq accepts none/low/medium/high for this model and rejects the rest
    // with a 400, so the engine's value reaches the wire exactly as given —
    // any silent translation here would be a claim about the vendor's
    // taxonomy made without a measurement.
    let mut request = req();
    request.reasoning_effort = Some("high".to_string());
    request.tools.clear();

    let body = GroqProvider::new(String::new()).build_body_for_test(&request, false);
    assert_eq!(body["reasoning_effort"], "high");
}

// ── Stream fixtures · Groq's documented shapes ──────────────────────────────

#[test]
fn text_deltas_arrive_in_delta_content() {
    let chunks = parse(&sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[{"index":0,"delta":{"content":"Arcana answer"},"finish_reason":null}]}"#,
    ))
    .expect("parse");
    assert!(matches!(
        &chunks[0],
        StreamChunk::TextDelta { text, .. } if text == "Arcana answer"
    ));
}

#[test]
fn qwen_reasoning_spelled_reasoning_becomes_a_thinking_delta() {
    // Groq's Qwen thinking models return reasoning under `reasoning` when
    // `reasoning_format` is `parsed` — a DIFFERENT key from the
    // `reasoning_content` OpenAI's reasoning models use. The shared parser
    // accepts both spellings.
    let chunks = parse(&sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[{"index":0,"delta":{"reasoning":"private thought","content":"visible answer"},"finish_reason":null}]}"#,
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

#[test]
fn tool_call_fragments_keyed_by_index_are_reassembled() {
    // Same OpenAI fragment shape: id and name once, then argument-only
    // fragments keyed by index. Every fragment must survive the parse.
    let chunks = parse(&sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_tool_0","type":"function","function":{"name":"read_file","arguments":"{\"path\":"}}]},"finish_reason":null}]}"#,
    ))
    .expect("parse");
    assert!(matches!(
        &chunks[0],
        StreamChunk::ToolUseStart { name, .. } if name == "read_file"
    ));
    assert!(matches!(
        &chunks[1],
        StreamChunk::ToolInputDelta { partial_json, .. } if partial_json == "{\"path\":"
    ));
}

#[test]
fn the_end_of_stream_usage_frame_is_read_under_x_groq() {
    // Groq's documented usage chunk: empty `choices`, statistics under
    // `x_groq.usage`. The OpenAI spelling is also accepted (see the next
    // test); only one of the two need arrive.
    let chunk_line = sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[],"x_groq":{"usage":{"prompt_tokens":1500,"completion_tokens":75,"total_tokens":1575}}}"#,
    );
    let chunks = parse(&chunk_line).expect("parse");
    let StreamChunk::MessageDelta { usage, .. } = &chunks[0] else {
        panic!("expected MessageDelta, got {chunks:?}");
    };
    assert_eq!(usage.output, 75);
    assert_eq!(usage.total_input(), 1500);
}

#[test]
fn the_end_of_stream_usage_frame_is_also_read_top_level() {
    // The OpenAI spelling on the same shape: a top-level `usage` object with
    // empty choices. Groq's schema allows `usage` null on intermediate chunks
    // and an object elsewhere, so both must parse.
    let chunks = parse(&sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[],"usage":{"prompt_tokens":8659,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":8448}}}"#,
    ))
    .expect("parse");
    let StreamChunk::MessageDelta { usage, .. } = &chunks[0] else {
        panic!("expected MessageDelta, got {chunks:?}");
    };
    // INCLUSIVE vendor: prompt_tokens contains the cached reads, and the
    // adapter must report the uncached remainder as `input`.
    assert_eq!(usage.cached_read, 8448);
    assert_eq!(usage.input, 211);
}

#[test]
fn completion_tokens_reasoning_are_reported_separately() {
    // Reasoning tokens bill at the output rate but are not visible content
    // (the catalogue prices `qwen/qwen3.8-27b` reasoning at the output rate).
    let chunks = parse(&sse(
        r#"{"id":"chatcmpl-x","object":"chat.completion.chunk","model":"qwen/qwen3.8-27b","choices":[],"x_groq":{"usage":{"prompt_tokens":100,"completion_tokens":500,"completion_tokens_details":{"reasoning_tokens":450}}}}"#,
    ))
    .expect("parse");
    let StreamChunk::MessageDelta { usage, .. } = &chunks[0] else {
        panic!("expected MessageDelta, got {chunks:?}");
    };
    assert_eq!(usage.reasoning, 450);
}

#[test]
fn the_done_sentinel_closes_the_stream() {
    let chunks = parse("data: [DONE]").expect("parse");
    assert!(matches!(chunks[0], StreamChunk::Done));
}

// ── Unary response fixture ───────────────────────────────────────────────────

#[test]
fn a_unary_response_parses_text_tools_and_reasoning() {
    // Shape per Groq's chat.completion schema: reasoning as `reasoning`
    // (Qwen thinking, reasoning_format parsed), tool call with an arguments
    // STRING, finish_reason, and usage with a cached-read breakdown.
    let data: serde_json::Value = serde_json::from_str(
        r#"{
            "id": "chatcmpl-1",
            "model": "qwen/qwen3.8-27b",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "reasoning": "I should read the file first",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_tool_0",
                        "type": "function",
                        "function": {"name": "read_file", "arguments": "{\"path\":\"src/lib.rs\"}"}
                    }]
                }
            }],
            "usage": {"prompt_tokens": 1420, "completion_tokens": 88, "total_tokens": 1508}
        }"#,
    )
    .expect("fixture");

    let response =
        crate::provider::openai::parse_completion_response(data).expect("parse unary response");

    assert_eq!(response.stop_reason, "tool_calls");
    assert_eq!(response.model, "qwen/qwen3.8-27b");
    assert!(
        matches!(&response.content[0], MessageContent::Thinking { text, .. }
            if text == "I should read the file first"
        ),
        "the `reasoning` spelling must surface as a thinking block, got {:?}",
        response.content[0]
    );
    assert!(
        matches!(&response.content[1], MessageContent::ToolUse { name, input, .. }
            if name == "read_file" && input["path"] == "src/lib.rs"
        )
    );

    assert_eq!(response.usage.total_input(), 1420);
    assert_eq!(response.usage.output, 88);
}

#[test]
fn the_catalogued_default_model_serves_from_the_shared_table() {
    let p = GroqProvider::new(String::new());
    assert_eq!(p.id(), "groq");
    let models = p.models();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "qwen/qwen3.8-27b");
    assert_eq!(models[0].context_window, 131_072);
}
