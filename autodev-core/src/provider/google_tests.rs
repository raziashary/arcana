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
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[
            {"functionCall":{"name":"read_file","args":{"path":"a.rs"}}},
            {"functionCall":{"name":"read_file","args":{"path":"b.rs"}}},
            {"functionCall":{"name":"grep","args":{"q":"fn main"}}}
        ]}}]}"#,
    ))
    .expect("parse");

    let starts: Vec<_> = chunks
        .iter()
        .filter_map(|c| match c {
            StreamChunk::ToolUseStart { id, block_index, .. } => Some((id.clone(), *block_index)),
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
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"content":{"parts":[
            {"text":"Let me look."},
            {"functionCall":{"name":"read_file","args":{"path":"x.rs"}}}
        ]}}]}"#,
    ))
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
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"finishReason":"STOP"}],
            "usageMetadata":{"promptTokenCount":1000,"candidatesTokenCount":50,
                             "cachedContentTokenCount":900,"thoughtsTokenCount":7}}"#,
    ))
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
    for (reason, payload) in [
        ("MAX_TOKENS", "MAX_TOKENS"),
        ("STOP", "STOP"),
    ] {
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
    let chunks = parse_google_sse_line(&sse(
        r#"{"candidates":[{"finishReason":"SAFETY"}]}"#,
    ))
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
