// Provider data types shared across all LLM provider implementations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id:                String,
    pub name:              String,
    pub context_window:    u32,
    pub supports_vision:   bool,
    pub supports_thinking: bool,   // Extended thinking / reasoning mode available
    pub cost_in_per_1m:    f64,    // USD per 1M input tokens
    pub cost_out_per_1m:   f64,    // USD per 1M output tokens
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderStatus {
    Connected,
    Disconnected,
    Testing,
    Error(String),
}

// ── Request / Response ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text    { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMessage {
    pub role:    String,   // "user" | "assistant"
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name:         String,
    pub description:  String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model:                  String,
    pub messages:               Vec<RequestMessage>,
    pub system:                 Option<String>,
    pub tools:                  Vec<ToolDefinition>,
    pub max_tokens:             u32,
    pub temperature:            Option<f32>,
    pub top_p:                  Option<f32>,    // nucleus sampling (0..1)
    pub reasoning_effort:       Option<String>, // "low" | "medium" | "high" (OpenAI reasoning models)
    pub thinking_budget_tokens: Option<u32>,    // Anthropic extended thinking budget
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message_id:  String,
    pub content:     Vec<MessageContent>,
    pub stop_reason: String,    // "end_turn" | "tool_use" | "max_tokens"
    pub tokens_in:   u32,
    pub tokens_out:  u32,
    pub model:       String,
}

// ── Streaming ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    TextDelta     { text: String },
    ThinkingDelta { text: String },
    ToolUseStart  { id: String, name: String, block_index: u64 },
    ToolInputDelta { block_index: u64, partial_json: String },
    MessageDelta  { stop_reason: String, tokens_in: u32, tokens_out: u32 },
    Done,
}
