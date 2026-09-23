// Streaming response parser utilities shared by provider implementations.
// Handles Server-Sent Events (SSE) line parsing and JSON extraction.

use super::types::StreamChunk;
use crate::errors::{ArcanaError, Result};
use super::usage::Usage;

/// Parse a single SSE data line into a StreamChunk.
/// Returns None for comment lines, ping events, or empty data.
pub fn parse_sse_line(line: &str) -> Result<Option<StreamChunk>> {
    let line = line.trim();
    // Skip empty lines, comments, and event: lines (SSE metadata)
    if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
        return Ok(None);
    }
    // Only parse data: lines
    let data = match line.strip_prefix("data: ") {
        Some(d) => d,
        None => return Ok(None), // Ignore lines without "data: " prefix
    };
    if data == "[DONE]" {
        return Ok(Some(StreamChunk::Done));
    }
    match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => parse_anthropic_event(&v),
        Err(e) => Err(ArcanaError::Provider(format!(
            "SSE parse error: {e} — data: {data}"
        ))),
    }
}

fn parse_anthropic_event(v: &serde_json::Value) -> Result<Option<StreamChunk>> {
    let event_type = v["type"].as_str().unwrap_or("");
    match event_type {
        // P2-43. `message_start` was falling through `_ => Ok(None)`, and it is
        // the frame that carries Anthropic's INPUT side: `input_tokens`,
        // `cache_read_input_tokens` and `cache_creation_input_tokens`.
        // `message_delta` carries only the output side.
        //
        // Swallowing it meant `total_tokens_in` summed zeros for every
        // streaming call — under-reporting cost, writing a zero input column to
        // analytics, and leaving the cache-hit alarm (P2-12) with no instrument
        // at all, since `cache_read_input_tokens` arrives here and nowhere else.
        "message_start" => {
            let u = &v["message"]["usage"];
            Ok(Some(StreamChunk::MessageDelta {
                // Not a stop: this frame opens the message. The engine
                // accumulates usage and ignores a `stop_reason` of "".
                stop_reason: String::new(),
                usage: Usage {
                    input: u["input_tokens"].as_u64().unwrap_or(0) as u32,
                    cached_read: u["cache_read_input_tokens"].as_u64().unwrap_or(0) as u32,
                    cache_write: u["cache_creation_input_tokens"].as_u64().unwrap_or(0) as u32,
                    // Output is not known yet; `message_delta` reports it.
                    output: 0,
                    reasoning: 0,
                    tool: 0,
                },
            }))
        }
        "content_block_delta" => {
            let delta = &v["delta"];
            match delta["type"].as_str().unwrap_or("") {
                "text_delta" => {
                    let text = delta["text"].as_str().unwrap_or("").to_string();
                    Ok(Some(StreamChunk::TextDelta {
                        text,
                        block_index: v["index"].as_u64().unwrap_or(0),
                    }))
                }
                "thinking_delta" => {
                    let text = delta["thinking"].as_str().unwrap_or("").to_string();
                    Ok(Some(StreamChunk::ThinkingDelta { text }))
                }
                "input_json_delta" => {
                    let partial = delta["partial_json"].as_str().unwrap_or("").to_string();
                    let block_index = v["index"].as_u64().unwrap_or(0);
                    Ok(Some(StreamChunk::ToolInputDelta {
                        block_index,
                        partial_json: partial,
                    }))
                }
                _ => Ok(None),
            }
        }
        "content_block_start" => {
            let block = &v["content_block"];
            let block_index = v["index"].as_u64().unwrap_or(0);
            if block["type"].as_str() == Some("tool_use") {
                let id = block["id"].as_str().unwrap_or("").to_string();
                let name = block["name"].as_str().unwrap_or("").to_string();
                Ok(Some(StreamChunk::ToolUseStart {
                    id,
                    name,
                    block_index,
                    // Anthropic issues no echo token (see MessageContent::ToolUse).
                    opaque: None,
                }))
            } else {
                Ok(None)
            }
        }
        "message_delta" => {
            let usage = &v["usage"];
            let stop_r = v["delta"]["stop_reason"]
                .as_str()
                .unwrap_or("end_turn")
                .to_string();
            // ⚠ D-9 (P2 post-phase audit). The input side is DELIBERATELY ZERO
            // here, and that is a correction, not an omission.
            //
            // The previous comment said "the vendor may report it in either
            // frame; the engine accumulates, so a field absent from one frame
            // contributes zero". Anthropic reports it in **both**, always —
            // captured from the live wire on 2026-09-02:
            //
            //   message_start  usage: {"input_tokens":9, "output_tokens":5, ...}
            //   message_delta  usage: {"input_tokens":9, "output_tokens":5, ...}
            //
            // Anthropic's per-message usage is **cumulative**, not incremental.
            // The engine sums every `MessageDelta`, so reading the input side
            // here counted it twice: every Anthropic STREAMING call reported
            // exactly 2× its real input cost, burned the daily ceiling at twice
            // the rate, and wrote a doubled figure to analytics. `complete()`
            // was unaffected, which is why the cost-reconciliation smoke — which
            // uses the unary path — passed throughout.
            //
            // The input side is fixed for a message and fully known at
            // `message_start`, so it is taken there and only there. `output` is
            // taken here and only here, because `message_start` reports a
            // partial count and this frame carries the final total —
            // `message_start` correspondingly emits `output: 0`.
            Ok(Some(StreamChunk::MessageDelta {
                stop_reason: stop_r,
                usage: Usage {
                    input: 0,
                    cached_read: 0,
                    cache_write: 0,
                    output: usage["output_tokens"].as_u64().unwrap_or(0) as u32,
                    reasoning: 0,
                    tool: 0,
                },
            }))
        }
        "message_stop" => Ok(Some(StreamChunk::Done)),
        _ => Ok(None),
    }
}
