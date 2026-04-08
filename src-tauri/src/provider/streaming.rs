// Streaming response parser utilities shared by provider implementations.
// Handles Server-Sent Events (SSE) line parsing and JSON extraction.

use crate::errors::{AutoDevError, Result};
use super::types::StreamChunk;

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
        Ok(v)  => parse_anthropic_event(&v),
        Err(e) => Err(AutoDevError::Provider(format!("SSE parse error: {e} — data: {data}"))),
    }
}

fn parse_anthropic_event(v: &serde_json::Value) -> Result<Option<StreamChunk>> {
    let event_type = v["type"].as_str().unwrap_or("");
    match event_type {
        "content_block_delta" => {
            let delta = &v["delta"];
            match delta["type"].as_str().unwrap_or("") {
                "text_delta" => {
                    let text = delta["text"].as_str().unwrap_or("").to_string();
                    Ok(Some(StreamChunk::TextDelta { text }))
                }
                "thinking_delta" => {
                    let text = delta["thinking"].as_str().unwrap_or("").to_string();
                    Ok(Some(StreamChunk::ThinkingDelta { text }))
                }
                "input_json_delta" => {
                    let partial      = delta["partial_json"].as_str().unwrap_or("").to_string();
                    let block_index  = v["index"].as_u64().unwrap_or(0);
                    Ok(Some(StreamChunk::ToolInputDelta { block_index, partial_json: partial }))
                }
                _ => Ok(None),
            }
        }
        "content_block_start" => {
            let block       = &v["content_block"];
            let block_index = v["index"].as_u64().unwrap_or(0);
            if block["type"].as_str() == Some("tool_use") {
                let id   = block["id"].as_str().unwrap_or("").to_string();
                let name = block["name"].as_str().unwrap_or("").to_string();
                Ok(Some(StreamChunk::ToolUseStart { id, name, block_index }))
            } else {
                Ok(None)
            }
        }
        "message_delta" => {
            let usage    = &v["usage"];
            let stop_r   = v["delta"]["stop_reason"].as_str().unwrap_or("end_turn").to_string();
            let tok_in   = usage["input_tokens"].as_u64().unwrap_or(0) as u32;
            let tok_out  = usage["output_tokens"].as_u64().unwrap_or(0) as u32;
            Ok(Some(StreamChunk::MessageDelta {
                stop_reason: stop_r,
                tokens_in:   tok_in,
                tokens_out:  tok_out,
            }))
        }
        "message_stop" => Ok(Some(StreamChunk::Done)),
        _ => Ok(None),
    }
}
