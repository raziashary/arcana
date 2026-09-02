// Adapter tests for the OpenAI SSE parser (P2-67, P2-68).
//
// OpenAI's stream is delta-typed rather than event-typed, and it has two
// properties that make the streaming path fail differently from the other two:
//
//   1. Usage arrives on a **final frame with an EMPTY `choices` array**, and
//      only if `stream_options: {include_usage: true}` was sent. Both halves
//      were missing — the option was sent nowhere, and the parser discarded any
//      frame whose `choices` was empty. Two independent bugs, either of which
//      alone produced zero usage on every streaming call (P2-42).
//   2. Tool calls arrive as indexed fragments whose `id` and `name` appear only
//      on the FIRST fragment for that index; every later fragment carries
//      arguments alone. A parser keying on `id` being present drops the rest.
//
// Fixtures are the shapes OpenAI actually emits, held as literals so this runs
// in CI with no credentials (§0.5).

use super::openai::parse_openai_sse_line_for_test as parse;
use super::types::StreamChunk;

fn sse(payload: &str) -> String {
    format!("data: {payload}")
}

#[test]
fn usage_arrives_on_a_frame_whose_choices_array_is_empty() {
    // 🔴 P2-42, the second half. Even once `stream_options.include_usage` was
    // sent, the parser returned early on an empty `choices` array — which is
    // exactly the shape of the frame carrying the usage. The option and the
    // parser had to BOTH change; fixing either alone still yielded zero.
    let chunk = parse(&sse(
        r#"{"id":"c1","object":"chat.completion.chunk","choices":[],"usage":{"prompt_tokens":1500,"completion_tokens":75,"total_tokens":1575}}"#,
    ))
    .expect("parse")
    .expect("the usage frame must produce a chunk");

    let StreamChunk::MessageDelta { usage, .. } = chunk else {
        panic!("expected MessageDelta, got {chunk:?}");
    };
    assert_eq!(usage.output, 75);
    assert_eq!(usage.total_input(), 1500);
}

#[test]
fn openai_includes_cached_tokens_in_prompt_tokens_so_the_adapter_subtracts() {
    // The other side of P2-01's normalisation. OpenAI's `prompt_tokens`
    // INCLUDES cached reads; Anthropic's `input_tokens` excludes them. `input`
    // must mean "not cached" on both, or the two cannot be summed or compared.
    //
    // The live smoke measured exactly this on 2026-09-02: prompt_tokens 8,659
    // with cached 8,448, and the adapter reported input 211. 211 + 8,448 = 8,659.
    let chunk = parse(&sse(
        r#"{"id":"c1","choices":[],"usage":{"prompt_tokens":8659,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":8448}}}"#,
    ))
    .expect("parse")
    .expect("chunk");
    let StreamChunk::MessageDelta { usage, .. } = chunk else {
        panic!("expected MessageDelta");
    };

    assert_eq!(usage.cached_read, 8448);
    assert_eq!(usage.input, 211, "cached tokens must be subtracted out of `input`");
    assert_eq!(usage.total_input(), 8659, "and the total must match the wire");
}

#[test]
fn reasoning_tokens_are_reported_separately() {
    // Reasoning tokens bill at the OUTPUT rate but are not visible content.
    // Folding them into `output` would price correctly but make the six-field
    // Usage unable to explain a bill.
    let chunk = parse(&sse(
        r#"{"id":"c1","choices":[],"usage":{"prompt_tokens":100,"completion_tokens":500,"completion_tokens_details":{"reasoning_tokens":450}}}"#,
    ))
    .expect("parse")
    .expect("chunk");
    let StreamChunk::MessageDelta { usage, .. } = chunk else {
        panic!("expected MessageDelta");
    };
    assert_eq!(usage.reasoning, 450);
}

#[test]
fn a_tool_calls_id_and_name_arrive_only_on_its_first_fragment() {
    // OpenAI sends the id and name once, then argument fragments keyed only by
    // `index`. A parser that required `id` on every fragment would emit one
    // ToolUseStart and drop every argument after it.
    let first = parse(&sse(
        r#"{"id":"c1","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#,
    ))
    .expect("parse")
    .expect("a ToolUseStart");

    let StreamChunk::ToolUseStart { id, name, block_index, .. } = first else {
        panic!("expected ToolUseStart, got {first:?}");
    };
    assert_eq!(id, "call_abc");
    assert_eq!(name, "read_file");
    assert_eq!(block_index, 0);

    // Subsequent fragments: arguments only, no id, no name.
    let mut accumulated = String::new();
    for fragment in [r#"{\"path\":"#, r#" \"src/lib"#, r#".rs\"}"#] {
        let payload = format!(
            r#"{{"id":"c1","choices":[{{"index":0,"delta":{{"tool_calls":[{{"index":0,"function":{{"arguments":"{fragment}"}}}}]}}}}]}}"#
        );
        let chunk = parse(&sse(&payload)).expect("parse").expect("an input delta");
        let StreamChunk::ToolInputDelta { partial_json, block_index } = chunk else {
            panic!("expected ToolInputDelta, got {chunk:?}");
        };
        assert_eq!(block_index, 0, "the index is the only thing tying this to its call");
        accumulated.push_str(&partial_json);
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&accumulated).expect("the concatenation must parse");
    assert_eq!(parsed["path"], serde_json::json!("src/lib.rs"));
}

#[test]
fn parallel_tool_calls_keep_separate_indices() {
    // Two calls in one turn arrive interleaved and are distinguished ONLY by
    // `index`. Collapsing them concatenates one call's arguments into the
    // other's, producing JSON that either fails to parse or — worse — parses
    // into the wrong arguments.
    let a = parse(&sse(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","type":"function","function":{"name":"read_file","arguments":""}}]}}]}"#,
    ))
    .expect("parse")
    .expect("chunk");
    let b = parse(&sse(
        r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","type":"function","function":{"name":"grep","arguments":""}}]}}]}"#,
    ))
    .expect("parse")
    .expect("chunk");

    let (StreamChunk::ToolUseStart { block_index: ia, id: id_a, .. }, StreamChunk::ToolUseStart { block_index: ib, id: id_b, .. }) = (a, b) else {
        panic!("expected two ToolUseStarts");
    };
    assert_ne!(ia, ib, "parallel calls must not share a block index");
    assert_eq!(id_a, "call_a");
    assert_eq!(id_b, "call_b");
}

#[test]
fn a_truncated_turn_is_distinguishable_from_a_clean_one() {
    // P2-41. The old code compared `finish_reason != "null"` — against the
    // four-character STRING. JSON `null` yields `None` from `as_str()` and
    // never reached that comparison, so the check was inert.
    //
    // P2-10: OpenAI spells truncation `length`, not `max_tokens`.
    for (wire, truncated) in [("length", true), ("stop", false), ("tool_calls", false)] {
        let chunk = parse(&sse(&format!(
            r#"{{"choices":[{{"index":0,"delta":{{}},"finish_reason":"{wire}"}}]}}"#
        )))
        .expect("parse")
        .expect("a MessageDelta");

        let StreamChunk::MessageDelta { stop_reason, .. } = chunk else {
            panic!("expected MessageDelta for {wire}, got {chunk:?}");
        };
        assert_eq!(stop_reason, wire);
        assert_eq!(
            super::usage::StopReason::parse("openai", &stop_reason).is_truncated(),
            truncated,
            "{wire} classified wrongly"
        );
    }
}

#[test]
fn a_null_finish_reason_is_not_a_stop() {
    // Every content frame carries `"finish_reason": null`. Treating that as a
    // stop would end the turn on the first token.
    let chunk = parse(&sse(
        r#"{"choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#,
    ))
    .expect("parse")
    .expect("a text delta");
    assert!(
        matches!(chunk, StreamChunk::TextDelta { .. }),
        "a null finish_reason must not produce a stop, got {chunk:?}"
    );
}

#[test]
fn sse_metadata_and_the_done_sentinel_are_handled() {
    for line in ["", "   ", ": keepalive", "event: ping"] {
        assert!(
            parse(line).expect("must not error").is_none(),
            "{line:?} should be ignored"
        );
    }
    let done = parse("data: [DONE]").expect("parse").expect("a chunk");
    assert!(matches!(done, StreamChunk::Done));
}

#[test]
fn a_malformed_data_payload_errors_instead_of_being_swallowed() {
    assert!(
        parse(&sse("{not json")).is_err(),
        "a malformed data payload must not be silently dropped"
    );
}
