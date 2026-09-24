// Streaming response parser utilities shared by provider implementations.
// Handles Server-Sent Events (SSE) line parsing and JSON extraction.

use super::types::StreamChunk;
use super::usage::Usage;
use crate::errors::{ArcanaError, Result};

/// Incremental parser for legacy model output that wraps private reasoning in
/// `<thinking>...</thinking>`. It retains only a partial delimiter between
/// chunks; complete reasoning is emitted as a segment and never buffered here.
#[derive(Debug, Default)]
pub struct XmlThinkingDetector {
    in_thinking: bool,
    pending: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThinkingSegment {
    Content(String),
    Thinking(String),
}

impl XmlThinkingDetector {
    pub fn push(&mut self, chunk: &str) -> Vec<ThinkingSegment> {
        self.pending.push_str(chunk);
        let mut segments = Vec::new();
        loop {
            let delimiter = if self.in_thinking {
                "</thinking>"
            } else {
                "<thinking>"
            };
            if let Some(index) = self.pending.find(delimiter) {
                let before = self.pending[..index].to_string();
                push_segment(&mut segments, self.in_thinking, before);
                self.pending.drain(..index + delimiter.len());
                self.in_thinking = !self.in_thinking;
                continue;
            }

            let keep = (1..=self.pending.len().min(delimiter.len() - 1))
                .rev()
                .find(|&len| self.pending.ends_with(&delimiter[..len]))
                .unwrap_or(0);
            let emit_len = self.pending.len() - keep;
            if emit_len > 0 {
                let rest = self.pending.drain(..emit_len).collect();
                push_segment(&mut segments, self.in_thinking, rest);
            }
            break;
        }
        segments
    }

    /// Flush an incomplete delimiter as literal text/reasoning at stream end.
    pub fn finish(&mut self) -> Option<ThinkingSegment> {
        if self.pending.is_empty() {
            return None;
        }
        let text = std::mem::take(&mut self.pending);
        Some(if self.in_thinking {
            ThinkingSegment::Thinking(text)
        } else {
            ThinkingSegment::Content(text)
        })
    }
}

fn push_segment(segments: &mut Vec<ThinkingSegment>, thinking: bool, text: String) {
    if text.is_empty() {
        return;
    }
    match (thinking, segments.last_mut()) {
        (true, Some(ThinkingSegment::Thinking(previous))) => previous.push_str(&text),
        (false, Some(ThinkingSegment::Content(previous))) => previous.push_str(&text),
        (true, _) => segments.push(ThinkingSegment::Thinking(text)),
        (false, _) => segments.push(ThinkingSegment::Content(text)),
    }
}

#[cfg(test)]
mod xml_thinking_tests {
    use super::{ThinkingSegment, XmlThinkingDetector};

    #[test]
    fn delimiters_split_across_chunks_keep_thought_private_and_content_visible() {
        let mut detector = XmlThinkingDetector::default();
        let mut output = Vec::new();
        for chunk in [
            "Before <thi",
            "nking>secret",
            " thoughts</think",
            "ing> after",
        ] {
            output.extend(detector.push(chunk));
        }
        if let Some(last) = detector.finish() {
            output.push(last);
        }
        assert_eq!(
            output,
            vec![
                ThinkingSegment::Content("Before ".into()),
                ThinkingSegment::Thinking("secret".into()),
                ThinkingSegment::Thinking(" thoughts".into()),
                ThinkingSegment::Content(" after".into()),
            ]
        );
    }

    #[test]
    fn unmatched_partial_opening_tag_is_returned_as_content() {
        let mut detector = XmlThinkingDetector::default();
        let mut segments = detector.push("ordinary <think");
        assert_eq!(segments, vec![ThinkingSegment::Content("ordinary ".into())]);
        segments.push(detector.finish().unwrap());
        assert_eq!(
            segments,
            vec![
                ThinkingSegment::Content("ordinary ".into()),
                ThinkingSegment::Content("<think".into())
            ]
        );
    }

    #[test]
    fn trailing_open_thinking_delimiter_flushes_to_private_channel() {
        let mut detector = XmlThinkingDetector::default();
        assert_eq!(
            detector.push("<thinking>private"),
            vec![ThinkingSegment::Thinking("private".into())]
        );
        assert_eq!(
            detector.finish(),
            None,
            "complete thought content was emitted incrementally, not buffered"
        );
    }
}

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
                    Ok(Some(StreamChunk::ThinkingDelta {
                        text,
                        block_index: v["index"].as_u64().unwrap_or(0),
                    }))
                }
                "signature_delta" => Ok(Some(StreamChunk::ThinkingSignature {
                    signature: delta["signature"].as_str().unwrap_or("").to_string(),
                    block_index: v["index"].as_u64().unwrap_or(0),
                })),
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
            } else if block["type"].as_str() == Some("thinking") {
                Ok(Some(StreamChunk::ThinkingDelta {
                    text: block["thinking"].as_str().unwrap_or("").to_string(),
                    block_index,
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
