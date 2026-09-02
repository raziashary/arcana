use super::usage::Usage;
// Provider data types shared across all LLM provider implementations.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub supports_vision: bool,
    pub supports_thinking: bool, // Extended thinking / reasoning mode available
    pub cost_in_per_1m: f64,     // USD per 1M input tokens
    pub cost_out_per_1m: f64,    // USD per 1M output tokens
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
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        /// The tool this result came from.
        ///
        /// Added in P2-37. Google's `functionResponse` keys on the function
        /// **name**, and the adapter was sending the tool-use **id** — so with
        /// ids of the form `gemini-read_file-0` the round trip told Gemini a
        /// function called `gemini-read_file-0` had replied, which is not a
        /// function it knows. Recovering the name by parsing the id back would
        /// work only for ids this adapter minted; carrying it is correct for
        /// history from any provider.
        name: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMessage {
    pub role: String, // "user" | "assistant"
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<RequestMessage>,
    pub system: Option<String>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,                  // nucleus sampling (0..1)
    pub reasoning_effort: Option<String>,    // "low" | "medium" | "high" (OpenAI reasoning models)
    pub thinking_budget_tokens: Option<u32>, // Anthropic extended thinking budget
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message_id: String,
    pub content: Vec<MessageContent>,
    pub stop_reason: String, // "end_turn" | "tool_use" | "max_tokens"
    /// Six-field provider-reported usage. Replaced `tokens_in`/`tokens_out`
    /// in P2-01: two fields cannot price a cached call, and cache_read is the
    /// only instrument that can tell whether caching works at all.
    pub usage: Usage,
    pub model: String,
}

// ── Streaming ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamChunk {
    TextDelta {
        text: String,
        /// Which content block this text belongs to.
        ///
        /// Added in P2-60. The engine used to buffer all text into one string
        /// and push it **before** every tool call, so a model that emitted
        /// `tool_use` and then commentary had the two swapped in the transcript
        /// — and that reordered message is replayed on every later turn.
        /// Anthropic sends this index on `content_block_delta` and it was being
        /// discarded.
        block_index: u64,
    },
    ThinkingDelta {
        text: String,
    },
    ToolUseStart {
        id: String,
        name: String,
        block_index: u64,
    },
    ToolInputDelta {
        block_index: u64,
        partial_json: String,
    },
    MessageDelta {
        stop_reason: String,
        usage: Usage,
    },
    Done,
}
