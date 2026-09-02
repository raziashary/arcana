// Cross-adapter conformance (P2-67, P2-68).
//
// Spec `02 §11`, gate `G-CONFORM`: per provider × model — streaming, tools,
// parallel tools, system prompt, usage completeness, truncation detection,
// error classification.
//
// Every defect asserted here was live at the start of P2 and survived because
// **1,733 lines of adapter code carried no test of any kind.** These run in CI
// with no credentials; the live smoke that catches vendor wire drift is P2-69.
//
// The tests are written against the **request body each adapter builds** and the
// **chunks each parser emits** — the two places a conformance defect can hide.

use super::anthropic::AnthropicProvider;
use super::google::GoogleProvider;
use super::openai::OpenAIProvider;
use super::types::{CompletionRequest, MessageContent, RequestMessage, ToolDefinition};

fn req() -> CompletionRequest {
    CompletionRequest {
        model: "test-model".to_string(),
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
    }
}

/// A message carrying a tool result **and** commentary, which is the shape
/// `P2-39` showed OpenAI silently truncating.
fn message_with_result_and_text() -> RequestMessage {
    RequestMessage {
        role: "user".to_string(),
        content: vec![
            MessageContent::ToolResult {
                tool_use_id: "call_1".to_string(),
                name: "read_file".to_string(),
                content: "file contents".to_string(),
                is_error: false,
            },
            MessageContent::Text {
                text: "here is what I found".to_string(),
            },
        ],
    }
}

// ── max_tokens · P2-08, P2-45 ────────────────────────────────────────────────

#[test]
fn every_adapter_sends_an_output_ceiling() {
    // It was read only by Anthropic. OpenAI and Google never sent it, so the
    // engine's ceiling applied to one provider of three, and truncation could
    // neither be requested nor detected on the other two.
    let a = AnthropicProvider::new(String::new()).build_body_for_test(&req(), false);
    assert_eq!(a["max_tokens"], 4096, "anthropic");

    let o = OpenAIProvider::new(String::new()).build_body_for_test(&req(), false);
    assert_eq!(
        o["max_completion_tokens"], 4096,
        "openai — `max_completion_tokens`, not the deprecated `max_tokens`, \
         which the reasoning models reject outright"
    );

    let g = GoogleProvider::new(String::new()).build_body_for_test(&req(), false);
    assert_eq!(
        g["generationConfig"]["maxOutputTokens"], 4096,
        "google — `maxOutputTokens` appeared nowhere in the tree"
    );
}

// ── usage on a stream · P2-42 ────────────────────────────────────────────────

#[test]
fn openai_asks_for_usage_when_streaming() {
    // Without `stream_options.include_usage`, OpenAI sends no usage object on a
    // stream at all — so the parser's usage branch was a guard that could never
    // fire, and both token counts AND the stop reason were structurally zero
    // for every streaming call.
    let streaming = OpenAIProvider::new(String::new()).build_body_for_test(&req(), true);
    assert_eq!(streaming["stream"], true);
    assert_eq!(
        streaming["stream_options"]["include_usage"], true,
        "usage must be requested, or it never arrives"
    );

    // Not requested on the non-streaming path, where usage always comes back.
    let unary = OpenAIProvider::new(String::new()).build_body_for_test(&req(), false);
    assert!(unary["stream_options"].is_null());
}

// ── tool results · P2-38, P2-39 ──────────────────────────────────────────────

#[test]
fn openai_keeps_text_that_shares_a_message_with_tool_results() {
    // P2-39. The branch was `if tool_results { .. } else { text }`, so the
    // model's own commentary vanished whenever it accompanied a result.
    let mut r = req();
    r.messages = vec![message_with_result_and_text()];
    let body = OpenAIProvider::new(String::new()).build_body_for_test(&r, false);
    let msgs = body["messages"].as_array().expect("messages");

    let has_tool = msgs.iter().any(|m| m["role"] == "tool");
    let has_text = msgs
        .iter()
        .any(|m| m["content"].as_str().unwrap_or("").contains("here is what I found"));
    assert!(has_tool, "the tool result must be sent");
    assert!(has_text, "and the text alongside it must not be dropped");
}

#[test]
fn a_failed_tool_is_distinguishable_from_a_successful_one() {
    // P2-38. Neither OpenAI's `tool` message nor Gemini's `functionResponse`
    // carries an error flag, and both adapters destructured `is_error: _`. A
    // failed tool looked identical to a successful one on two of three
    // providers, and the model then builds on a result that never happened.
    let mut r = req();
    r.messages = vec![RequestMessage {
        role: "user".to_string(),
        content: vec![MessageContent::ToolResult {
            tool_use_id: "call_1".to_string(),
            name: "read_file".to_string(),
            content: "no such file".to_string(),
            is_error: true,
        }],
    }];

    let o = OpenAIProvider::new(String::new()).build_body_for_test(&r, false);
    let openai_body = serde_json::to_string(&o["messages"]).unwrap();
    assert!(
        openai_body.contains("ERROR"),
        "openai must mark the failure in the payload: {openai_body}"
    );

    let g = GoogleProvider::new(String::new()).build_body_for_test(&r, false);
    let google_body = serde_json::to_string(&g["contents"]).unwrap();
    assert!(
        google_body.contains("\"error\""),
        "google must mark the failure in the response object: {google_body}"
    );
}

// ── tool result identity · P2-37 ─────────────────────────────────────────────

#[test]
fn google_answers_with_the_function_name_not_the_call_id() {
    // Gemini keys `functionResponse` on the function NAME. Sending the id meant
    // telling it that a function called `gemini-read_file-0` had replied — not
    // a function it knows.
    let mut r = req();
    r.messages = vec![RequestMessage {
        role: "user".to_string(),
        content: vec![MessageContent::ToolResult {
            tool_use_id: "gemini-read_file-0".to_string(),
            name: "read_file".to_string(),
            content: "ok".to_string(),
            is_error: false,
        }],
    }];
    let g = GoogleProvider::new(String::new()).build_body_for_test(&r, false);
    let body = serde_json::to_string(&g["contents"]).unwrap();
    assert!(
        body.contains("\"name\":\"read_file\""),
        "must send the tool name: {body}"
    );
    assert!(
        !body.contains("\"name\":\"gemini-read_file-0\""),
        "must not send the call id as the name"
    );
}

// ── roles · P2-40 ────────────────────────────────────────────────────────────

#[test]
fn an_unrecognised_role_is_preserved_by_both_adapters() {
    // OpenAI had no `else` and the message vanished; Google coerced it to
    // `user`. Two adapters, two different silent behaviours for identical
    // input. Both now coerce — losing attribution is recoverable, losing the
    // content is not — and both log.
    let mut r = req();
    r.messages = vec![RequestMessage {
        role: "system-ish".to_string(),
        content: vec![MessageContent::Text {
            text: "important context".to_string(),
        }],
    }];

    let o = OpenAIProvider::new(String::new()).build_body_for_test(&r, false);
    assert!(
        serde_json::to_string(&o["messages"])
            .unwrap()
            .contains("important context"),
        "openai dropped a message with an unrecognised role"
    );

    let g = GoogleProvider::new(String::new()).build_body_for_test(&r, false);
    assert!(
        serde_json::to_string(&g["contents"])
            .unwrap()
            .contains("important context"),
        "google dropped a message with an unrecognised role"
    );
}

// ── system prompt · G-CONFORM ────────────────────────────────────────────────

#[test]
fn every_adapter_sends_the_system_prompt_somewhere() {
    let a = AnthropicProvider::new(String::new()).build_body_for_test(&req(), false);
    assert!(serde_json::to_string(&a).unwrap().contains("be brief"));

    let o = OpenAIProvider::new(String::new()).build_body_for_test(&req(), false);
    assert!(serde_json::to_string(&o).unwrap().contains("be brief"));

    let g = GoogleProvider::new(String::new()).build_body_for_test(&req(), false);
    assert!(serde_json::to_string(&g).unwrap().contains("be brief"));
}

#[test]
fn every_adapter_sends_the_tool_definitions() {
    for body in [
        AnthropicProvider::new(String::new()).build_body_for_test(&req(), false),
        OpenAIProvider::new(String::new()).build_body_for_test(&req(), false),
        GoogleProvider::new(String::new()).build_body_for_test(&req(), false),
    ] {
        assert!(
            serde_json::to_string(&body).unwrap().contains("read_file"),
            "a tool the agent registered never reached the provider"
        );
    }
}

// ── P2-09 · parallel tool results: one engine shape, three wire shapes ──────
//
// The engine collects every tool result for a turn into ONE user message
// (`engine.rs`, "Add tool results as user message"). Anthropic REQUIRES that —
// results split across several user messages are rejected. The other two want
// it flattened differently: OpenAI into one `tool` message per result, Google
// into one `functionResponse` part per result.
//
// So one engine shape has to become three wire shapes, and nothing asserted
// that it did. This is the PARALLEL case specifically: a single tool call is
// the shape that works by accident under all three encodings.

/// A three-call / three-result turn, as the engine actually assembles it.
fn parallel_tool_turn() -> CompletionRequest {
    let mut r = req();
    r.messages = vec![
        RequestMessage {
            role: "user".to_string(),
            content: vec![MessageContent::Text {
                text: "read three files".to_string(),
            }],
        },
        RequestMessage {
            role: "assistant".to_string(),
            content: vec![
                MessageContent::ToolUse {
                    id: "call_a".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "a.rs"}),
                },
                MessageContent::ToolUse {
                    id: "call_b".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({"path": "b.rs"}),
                },
                MessageContent::ToolUse {
                    id: "call_c".to_string(),
                    name: "grep".to_string(),
                    input: serde_json::json!({"pattern": "fn main"}),
                },
            ],
        },
        // All three results in ONE user message — the engine's shape.
        RequestMessage {
            role: "user".to_string(),
            content: vec![
                MessageContent::ToolResult {
                    tool_use_id: "call_a".to_string(),
                    name: "read_file".to_string(),
                    content: "contents of a".to_string(),
                    is_error: false,
                },
                MessageContent::ToolResult {
                    tool_use_id: "call_b".to_string(),
                    name: "read_file".to_string(),
                    content: "contents of b".to_string(),
                    is_error: false,
                },
                MessageContent::ToolResult {
                    tool_use_id: "call_c".to_string(),
                    name: "grep".to_string(),
                    content: "no matches".to_string(),
                    is_error: true,
                },
            ],
        },
    ];
    r
}

#[test]
fn anthropic_keeps_parallel_results_in_one_user_message() {
    let body = AnthropicProvider::new(String::new()).build_body_for_test(&parallel_tool_turn(), false);
    let msgs = body["messages"].as_array().expect("messages");

    // Anthropic rejects results split across messages, so the count of user
    // messages CARRYING results is the invariant, not just the result count.
    let carrying = msgs
        .iter()
        .filter(|m| {
            m["role"] == "user"
                && m["content"]
                    .as_array()
                    .is_some_and(|cs| cs.iter().any(|c| c["type"] == "tool_result"))
        })
        .count();
    assert_eq!(carrying, 1, "anthropic requires every result for a turn in ONE user message");

    let results: Vec<serde_json::Value> = msgs
        .iter()
        .filter(|m| m["role"] == "user")
        .flat_map(|m| m["content"].as_array().cloned().unwrap_or_default())
        .filter(|c| c["type"] == "tool_result")
        .collect();
    assert_eq!(results.len(), 3, "anthropic dropped a parallel tool result");
    for (i, id) in ["call_a", "call_b", "call_c"].iter().enumerate() {
        assert_eq!(results[i]["tool_use_id"], serde_json::json!(id), "results reordered");
    }
    assert_eq!(
        results[2]["is_error"],
        serde_json::json!(true),
        "the failing tool lost its error flag"
    );
}

#[test]
fn openai_flattens_parallel_results_into_one_tool_message_each_in_call_order() {
    let body = OpenAIProvider::new(String::new()).build_body_for_test(&parallel_tool_turn(), false);
    let msgs = body["messages"].as_array().expect("messages");

    let tool_msgs: Vec<&serde_json::Value> =
        msgs.iter().filter(|m| m["role"] == "tool").collect();
    assert_eq!(tool_msgs.len(), 3, "openai dropped a parallel tool result");
    for (i, id) in ["call_a", "call_b", "call_c"].iter().enumerate() {
        assert_eq!(
            tool_msgs[i]["tool_call_id"],
            serde_json::json!(id),
            "openai pairs results with calls BY POSITION — a reorder mispairs them (P2-61)"
        );
    }

    // The assistant turn must carry all three calls, or those ids dangle and
    // the request is rejected.
    let calls = msgs
        .iter()
        .find(|m| m["role"] == "assistant" && m["tool_calls"].is_array())
        .and_then(|m| m["tool_calls"].as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    assert_eq!(calls, 3, "openai lost a parallel tool CALL");
}

#[test]
fn google_emits_one_function_response_part_per_parallel_result() {
    let body = GoogleProvider::new(String::new()).build_body_for_test(&parallel_tool_turn(), false);
    let contents = body["contents"].as_array().expect("contents");

    let parts: Vec<serde_json::Value> = contents
        .iter()
        .flat_map(|c| c["parts"].as_array().cloned().unwrap_or_default())
        .collect();

    let responses: Vec<&serde_json::Value> = parts
        .iter()
        .filter(|p| p.get("functionResponse").is_some())
        .collect();
    assert_eq!(responses.len(), 3, "google dropped a parallel tool result");

    // P2-37: the NAME, not the id. Two of these deliberately share a name —
    // that is the case that makes a name-keyed protocol ambiguous, and it is
    // still what the wire format requires.
    let names: Vec<&str> = responses
        .iter()
        .filter_map(|p| p["functionResponse"]["name"].as_str())
        .collect();
    assert_eq!(names, vec!["read_file", "read_file", "grep"]);

    let calls: Vec<&serde_json::Value> = parts
        .iter()
        .filter(|p| p.get("functionCall").is_some())
        .collect();
    assert_eq!(calls.len(), 3, "google lost a parallel tool CALL");
    // Arguments survive — P2-06 was the same defect on the response side.
    assert_eq!(calls[0]["functionCall"]["args"]["path"], serde_json::json!("a.rs"));
    assert_eq!(calls[2]["functionCall"]["args"]["pattern"], serde_json::json!("fn main"));
}

#[test]
fn every_shipped_model_that_claims_parallel_tools_actually_gets_them() {
    // `capabilities()` is only worth having if it answers for the models we
    // ship. Before the catalogue landed it answered for none of them: the
    // adapters listed a previous generation, so a lookup by id found nothing
    // and every model fell to the conservative default — which reports
    // `parallel_tools: false`. The agent would have serialised calls the model
    // was perfectly capable of running at once.
    for e in crate::provider::catalogue::CATALOGUE {
        let caps = crate::provider::catalogue::capabilities_for(e.id);
        assert!(caps.parallel_tools, "{} does not report parallel tool support", e.id);
        assert!(caps.tools && caps.streaming, "{} lost a base capability", e.id);
    }
}
