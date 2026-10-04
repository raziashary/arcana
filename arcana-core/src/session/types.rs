// Core session and message data types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub role: MessageRole,
    pub content: String,
    // For tool messages: metadata about the tool execution
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub tool_call_id: Option<String>, // Links tool result to original call
    pub is_error: bool,               // For tool messages: indicates error
    // Token/cost for this message (assistant messages only)
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub cost_usd: f64,
    pub created_at: DateTime<Utc>,
}

impl Message {
    pub fn new_user(session_id: &str, content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            tool_name: None,
            tool_input: None,
            tool_call_id: None,
            is_error: false,
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            created_at: Utc::now(),
        }
    }

    pub fn new_assistant(session_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            tool_name: None,
            tool_input: None,
            tool_call_id: None,
            is_error: false,
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            created_at: Utc::now(),
        }
    }

    /// Create a new tool result message (separate bubble in chat)
    pub fn new_tool(
        session_id: &str,
        call_id: &str,
        tool_name: &str,
        input: serde_json::Value,
        output: &str,
        is_error: bool,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            role: MessageRole::Tool,
            content: output.to_string(),
            tool_name: Some(tool_name.to_string()),
            tool_input: Some(input),
            tool_call_id: Some(call_id.to_string()),
            is_error,
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub working_dir: String,
    pub provider: String,
    pub model: String,
    // Phase 3.0 R3 — persisted PA mode. Previously only lived in the
    // frontend Zustand store + localStorage, so a session reload or a fresh
    // engine build snapped back to the default `fast_development` even when
    // the user had explicitly selected Delegate. Stored per-session so each
    // chat remembers its own mode.
    pub pa_mode: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

/// Valid values for `Session.pa_mode`. Kept in one place so the Rust side
/// and the Tauri command layer can validate uniformly.
pub const PA_MODES: &[&str] = &["fast_development", "review", "delegate"];

impl Session {
    pub fn new(working_dir: &str, provider: &str, model: &str) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4().to_string(),
            title: "New Session".to_string(),
            // SessionManager passes the canonical workspace resolved by the
            // shared platform resolver. Keep this data constructor free of
            // process-CWD fallbacks.
            working_dir: working_dir.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            pa_mode: "fast_development".to_string(),
            created_at: now,
            updated_at: now,
            messages: vec![],
        }
    }
}
