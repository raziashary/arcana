// SSE frame parsing for core-orchestrator's streaming endpoints.
//
// These helpers used to live in `subagent/bridge.rs`, which drove subagents
// through the orchestrator over SSE. Subagents now run in-process on a nested
// AgentEngine (`subagent/native_runner.rs`) and that bridge is gone, but
// `sidecar::client::invoke_worker_streaming` still speaks this protocol — so
// the framing moved here, to the module that owns the orchestrator connection,
// rather than being deleted along with its former host.
//
// This module is deleted with the rest of the sidecar in P8.

use tracing::debug;

use crate::subagent::SubagentEvent;

/// Locate the byte offset *just before* the blank-line separator (`\n\n` or
/// `\r\n\r\n`). Returns None if the buffer does not yet contain a full frame.
pub fn find_frame_end(buf: &[u8]) -> Option<usize> {
    // Search for "\r\n\r\n" first (CRLF), then "\n\n" (LF). Prefer earliest.
    let mut earliest: Option<usize> = None;
    if let Some(i) = find_subsequence(buf, b"\r\n\r\n") {
        earliest = Some(i);
    }
    if let Some(i) = find_subsequence(buf, b"\n\n") {
        earliest = Some(match earliest {
            Some(e) => e.min(i),
            None => i,
        });
    }
    earliest
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Parse a single SSE frame into a SubagentEvent.
///
/// Accepts both shapes:
///
///   event: token
///   data: {"text": "Hi"}
///
///   data: {"type": "token", "text": "Hi"}
///
/// Returns None for unknown / unparseable frames (logged at debug level).
pub fn parse_event_frame(frame: &str, subagent_id: &str) -> Option<SubagentEvent> {
    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();

    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.trim_start());
        }
    }

    let data_str = data_lines.join("\n");
    let data: serde_json::Value = if data_str.is_empty() {
        serde_json::Value::Null
    } else {
        match serde_json::from_str(&data_str) {
            Ok(v) => v,
            Err(e) => {
                debug!("[OrchestratorSSE] non-JSON SSE data ignored: {e} :: {data_str}");
                return None;
            }
        }
    };

    let kind = event_name
        .or_else(|| {
            data.get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            data.get("event")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "token".to_string());

    let id = subagent_id.to_string();
    let text_field = || {
        data.get("text")
            .or_else(|| data.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let evt = match kind.as_str() {
        "token" | "text" | "delta" => SubagentEvent::Token {
            subagent_id: id,
            text: text_field(),
        },
        "thinking" | "thought" | "reasoning" => SubagentEvent::Thinking {
            subagent_id: id,
            text: text_field(),
        },
        "tool_start" | "tool_call" => SubagentEvent::ToolStart {
            subagent_id: id,
            tool: data
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            input: data
                .get("input")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        },
        "tool_end" | "tool_result" => SubagentEvent::ToolEnd {
            subagent_id: id,
            tool: data
                .get("tool")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            output: data
                .get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            is_error: data
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        "final" | "done" | "complete" | "completed" => SubagentEvent::Final {
            subagent_id: id,
            text: text_field(),
        },
        "cost" | "usage" => SubagentEvent::Cost {
            subagent_id: id,
            cost_usd: data.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
            tokens_in: data.get("tokens_in").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            tokens_out: data.get("tokens_out").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
        "error" => SubagentEvent::Error {
            subagent_id: id,
            message: data
                .get("message")
                .and_then(|v| v.as_str())
                .or_else(|| data.get("error").and_then(|v| v.as_str()))
                .unwrap_or("unknown error")
                .to_string(),
        },
        _ => {
            debug!("[OrchestratorSSE] unknown SSE event kind '{kind}' (forwarding as token)");
            SubagentEvent::Token {
                subagent_id: id,
                text: text_field(),
            }
        }
    };

    Some(evt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_event_frame() {
        let frame = "event: token\ndata: {\"text\": \"Hi\"}";
        let evt = parse_event_frame(frame, "subagent-x-1-1").expect("parsed");
        match evt {
            SubagentEvent::Token { text, .. } => assert_eq!(text, "Hi"),
            _ => panic!("expected Token"),
        }
    }

    #[test]
    fn parses_typed_data_only_frame() {
        let frame = "data: {\"type\": \"final\", \"text\": \"Done\"}";
        let evt = parse_event_frame(frame, "x").expect("parsed");
        match evt {
            SubagentEvent::Final { text, .. } => assert_eq!(text, "Done"),
            _ => panic!("expected Final"),
        }
    }

    #[test]
    fn finds_frame_end_lf() {
        assert!(find_frame_end(b"data: {\"text\": \"Hi\"}\n\n").is_some());
    }

    #[test]
    fn finds_frame_end_crlf() {
        assert!(find_frame_end(b"CRdata: {\"text\": \"Hi\"}\n\n").is_some());
    }

    #[test]
    fn ignores_invalid_data() {
        assert!(parse_event_frame("event: token\ndata: not-json", "x").is_none());
    }
}
