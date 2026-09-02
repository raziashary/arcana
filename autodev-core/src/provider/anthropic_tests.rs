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
use super::types::StreamChunk;

fn sse(payload: &str) -> String {
    format!("data: {payload}")
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
    assert_eq!(usage.cached_read, 8000, "the cache alarm reads this and nothing else");
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

    let StreamChunk::ToolUseStart { id, name, block_index, .. } = start else {
        panic!("expected ToolUseStart, got {start:?}");
    };
    assert_eq!(id, "toolu_01");
    assert_eq!(name, "read_file");
    assert_eq!(block_index, 1, "the wire index is what orders the assembled blocks");

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
    for (wire, expect_truncated) in [("max_tokens", true), ("end_turn", false), ("tool_use", false)]
    {
        let chunk = parse_sse_line(&sse(&format!(
            r#"{{"type":"message_delta","delta":{{"stop_reason":"{wire}"}},"usage":{{"output_tokens":42}}}}"#
        )))
        .expect("parse")
        .expect("a MessageDelta");

        let StreamChunk::MessageDelta { stop_reason, usage } = chunk else {
            panic!("expected MessageDelta");
        };
        assert_eq!(stop_reason, wire, "the raw wire value is preserved verbatim");
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
    let err = parse_sse_line(&sse("{not json"));
    assert!(err.is_err(), "a malformed data payload must not be silently dropped");
}

#[test]
fn the_done_sentinel_ends_the_stream() {
    let chunk = parse_sse_line("data: [DONE]").expect("parse").expect("a chunk");
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
