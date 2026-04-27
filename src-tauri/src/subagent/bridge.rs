// SubagentBridge — translates SubagentConfig + prompt into the create-invoke
// (stream)-delete lifecycle against core-orchestrator, parses the SSE event
// frames, and produces a SubagentResult.
//
// The bridge does NOT track handles or enforce concurrency limits — that is
// the SubagentManager / SubagentPool's job. The bridge is a stateless function
// over (config, prompt) → (result, event stream).
//
// SSE protocol notes:
//   core-orchestrator's `/api/agents/{id}/invoke/stream` emits frames in
//   `text/event-stream` format. Each frame typically looks like:
//
//     event: token
//     data: {"text": "Hello"}
//
//     event: tool_start
//     data: {"tool": "read_file", "input": {...}}
//
//   Different deployments may also use plain `data:` lines without `event:`,
//   carrying a JSON object whose `type` field is the discriminator. We accept
//   both shapes — see `parse_event_frame`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::errors::{AutoDevError, Result};
use crate::sidecar::client::OrchestratorClient;

use super::types::{
    SubagentConfig, SubagentEvent, SubagentResult, SubagentStatus,
};

/// Execute the full create → invoke(stream) → delete lifecycle.
///
/// `events_tx` receives every parsed SubagentEvent in real time so the
/// SubagentManager can rebroadcast them to the frontend (`subagent:event`).
/// On any error, the agent is still deleted (best effort) before returning.
///
/// Cancellation: if the caller's tokio task is dropped, the in-flight HTTP
/// future is dropped too. The `manager.rs` ensures cleanup runs anyway via
/// `terminate_subagent` or the orphan sweep.
pub async fn run(
    client:    Arc<OrchestratorClient>,
    subagent_id: String,
    config:    SubagentConfig,
    prompt:    String,
    events_tx: mpsc::UnboundedSender<SubagentEvent>,
) -> Result<SubagentResult> {
    let started_at = Instant::now();

    // ── 1. Create ephemeral agent ────────────────────────────────────────────
    emit(&events_tx, SubagentEvent::Status {
        subagent_id: subagent_id.clone(),
        status: SubagentStatus::Spawning,
        message: Some("Creating ephemeral agent in core-orchestrator".into()),
    });

    let mcp_servers = if config.mcp_servers.is_empty() {
        vec![SubagentConfig::DEFAULT_MCP_SERVER.to_string()]
    } else {
        config.mcp_servers.clone()
    };
    let capabilities = if config.capabilities.is_empty() {
        vec!["subagent_ephemeral".to_string()]
    } else {
        config.capabilities.clone()
    };

    let create_res = client.create_ephemeral_agent(
        &subagent_id,
        &config.system_prompt,
        &config.model,
        &mcp_servers,
        &capabilities,
    ).await;

    let agent_id = match create_res {
        Ok(id) => id,
        Err(e) => {
            emit(&events_tx, SubagentEvent::Status {
                subagent_id: subagent_id.clone(),
                status: SubagentStatus::Failed,
                message: Some(format!("create failed: {e}")),
            });
            return Err(e);
        }
    };

    // From here on, we MUST attempt cleanup before returning.
    let outcome = run_invoke_phase(
        client.clone(),
        &agent_id,
        &subagent_id,
        &config,
        &prompt,
        &events_tx,
    ).await;

    // ── 3. Delete (always attempted) ────────────────────────────────────────
    if let Err(e) = client.delete_agent(&agent_id).await {
        warn!("[SubagentBridge] cleanup DELETE failed for {agent_id}: {e}");
    } else {
        debug!("[SubagentBridge] cleaned up ephemeral subagent {agent_id}");
    }

    let duration_secs = started_at.elapsed().as_secs_f64();

    match outcome {
        Ok((summary, cost_usd, tokens_in, tokens_out)) => {
            let result = SubagentResult {
                subagent_id: subagent_id.clone(),
                status: SubagentStatus::Completed,
                result_summary: summary,
                files_modified: Vec::new(),
                cost_usd, tokens_in, tokens_out,
                duration_secs,
                error: None,
            };
            emit(&events_tx, SubagentEvent::Status {
                subagent_id,
                status: SubagentStatus::Completed,
                message: None,
            });
            Ok(result)
        }
        Err(e) => {
            let msg = e.to_string();
            emit(&events_tx, SubagentEvent::Status {
                subagent_id: subagent_id.clone(),
                status: SubagentStatus::Failed,
                message: Some(msg.clone()),
            });
            Ok(SubagentResult {
                subagent_id,
                status: SubagentStatus::Failed,
                result_summary: String::new(),
                files_modified: Vec::new(),
                cost_usd: 0.0,
                tokens_in:  0,
                tokens_out: 0,
                duration_secs,
                error: Some(msg),
            })
        }
    }
}

/// Inner phase: invoke streaming endpoint and consume SSE frames until done.
///
/// Returns (final_text, cost_usd, tokens_in, tokens_out).
async fn run_invoke_phase(
    client:      Arc<OrchestratorClient>,
    agent_id:    &str,
    subagent_id: &str,
    config:      &SubagentConfig,
    prompt:      &str,
    events_tx:   &mpsc::UnboundedSender<SubagentEvent>,
) -> Result<(String, f64, u32, u32)> {
    emit(events_tx, SubagentEvent::Status {
        subagent_id: subagent_id.to_string(),
        status: SubagentStatus::Running,
        message: None,
    });

    let timeout_dur = Duration::from_secs(config.timeout_secs.max(1) as u64);

    let stream_fut = consume_stream(
        client,
        agent_id,
        subagent_id,
        prompt,
        events_tx,
    );

    match timeout(timeout_dur, stream_fut).await {
        Ok(res) => res,
        Err(_) => Err(AutoDevError::Agent(format!(
            "subagent {subagent_id} timed out after {}s",
            config.timeout_secs
        ))),
    }
}

async fn consume_stream(
    client:      Arc<OrchestratorClient>,
    agent_id:    &str,
    subagent_id: &str,
    prompt:      &str,
    events_tx:   &mpsc::UnboundedSender<SubagentEvent>,
) -> Result<(String, f64, u32, u32)> {
    let resp = client.invoke_agent_stream_raw(agent_id, prompt).await?;

    let mut byte_stream = resp.bytes_stream();
    let mut buf = Vec::<u8>::new();

    let mut accumulated_text = String::new();
    let mut last_final_text  = String::new();
    let mut cost_usd:   f64 = 0.0;
    let mut tokens_in:  u32 = 0;
    let mut tokens_out: u32 = 0;

    while let Some(chunk_res) = byte_stream.next().await {
        let chunk = chunk_res
            .map_err(|e| AutoDevError::Agent(format!("SSE chunk read: {e}")))?;
        buf.extend_from_slice(&chunk);

        // SSE frames are separated by a blank line ("\n\n" or "\r\n\r\n").
        loop {
            let Some(end) = find_frame_end(&buf) else { break };
            let frame_bytes: Vec<u8> = buf.drain(..end).collect();
            // Drop the trailing blank line we matched on (2 or 4 bytes)
            let trailer_len = if buf.starts_with(b"\r\n\r\n") || buf.starts_with(b"\n\n") {
                if buf.starts_with(b"\r\n\r\n") { 4 } else { 2 }
            } else { 0 };
            buf.drain(..trailer_len);

            let frame = String::from_utf8_lossy(&frame_bytes).to_string();
            if frame.trim().is_empty() { continue; }

            if let Some(evt) = parse_event_frame(&frame, subagent_id) {
                // Side-effects: accumulate text + usage before forwarding.
                match &evt {
                    SubagentEvent::Token { text, .. } => {
                        accumulated_text.push_str(text);
                    }
                    SubagentEvent::Final { text, .. } => {
                        last_final_text = text.clone();
                    }
                    SubagentEvent::Cost { cost_usd: c, tokens_in: ti, tokens_out: to_, .. } => {
                        cost_usd  = *c;
                        tokens_in = *ti;
                        tokens_out = *to_;
                    }
                    _ => {}
                }
                emit(events_tx, evt);
            }
        }
    }

    let summary = if !last_final_text.is_empty() { last_final_text } else { accumulated_text };
    Ok((summary, cost_usd, tokens_in, tokens_out))
}

/// Locate the byte offset *just before* the blank-line separator (`\n\n` or
/// `\r\n\r\n`). Returns None if the buffer does not yet contain a full frame.
fn find_frame_end(buf: &[u8]) -> Option<usize> {
    // Search for "\r\n\r\n" first (CRLF), then "\n\n" (LF). Prefer earliest.
    let mut earliest: Option<usize> = None;
    if let Some(i) = find_subsequence(buf, b"\r\n\r\n") {
        earliest = Some(i);
    }
    if let Some(i) = find_subsequence(buf, b"\n\n") {
        earliest = Some(match earliest {
            Some(e) => e.min(i),
            None    => i,
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
fn parse_event_frame(frame: &str, subagent_id: &str) -> Option<SubagentEvent> {
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
                debug!("[SubagentBridge] non-JSON SSE data ignored: {e} :: {data_str}");
                return None;
            }
        }
    };

    let kind = event_name
        .or_else(|| data.get("type").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .or_else(|| data.get("event").and_then(|v| v.as_str()).map(|s| s.to_string()))
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
            subagent_id: id, text: text_field(),
        },
        "thinking" | "thought" | "reasoning" => SubagentEvent::Thinking {
            subagent_id: id, text: text_field(),
        },
        "tool_start" | "tool_call" => SubagentEvent::ToolStart {
            subagent_id: id,
            tool: data.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            input: data.get("input").cloned().unwrap_or(serde_json::Value::Null),
        },
        "tool_end" | "tool_result" => SubagentEvent::ToolEnd {
            subagent_id: id,
            tool: data.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            output: data.get("output").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            is_error: data.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "final" | "done" | "complete" | "completed" => SubagentEvent::Final {
            subagent_id: id, text: text_field(),
        },
        "cost" | "usage" => SubagentEvent::Cost {
            subagent_id: id,
            cost_usd:    data.get("cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0),
            tokens_in:   data.get("tokens_in").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            tokens_out:  data.get("tokens_out").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
        "error" => SubagentEvent::Error {
            subagent_id: id,
            message: data.get("message").and_then(|v| v.as_str())
                .or_else(|| data.get("error").and_then(|v| v.as_str()))
                .unwrap_or("unknown error")
                .to_string(),
        },
        _ => {
            debug!("[SubagentBridge] unknown SSE event kind '{kind}' (forwarding as token)");
            SubagentEvent::Token { subagent_id: id, text: text_field() }
        }
    };

    Some(evt)
}

fn emit(tx: &mpsc::UnboundedSender<SubagentEvent>, evt: SubagentEvent) {
    let _ = tx.send(evt);
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
        let buf = b"data: {\"text\": \"Hi\"}\n\n";
        assert!(find_frame_end(buf).is_some());
    }

    #[test]
    fn finds_frame_end_crlf() {
        let buf = b"data: {\"text\": \"Hi\"}\r\n\r\n";
        assert!(find_frame_end(buf).is_some());
    }

    #[test]
    fn ignores_invalid_data() {
        let frame = "event: token\ndata: not-json";
        assert!(parse_event_frame(frame, "x").is_none());
    }
}
