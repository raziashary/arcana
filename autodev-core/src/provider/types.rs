use super::usage::{CachePolicy, Usage};
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
        /// An opaque provider-issued token that must be echoed back verbatim.
        ///
        /// ── Why this field exists ────────────────────────────────────────────
        ///
        /// Gemini attaches a `thoughtSignature` to each `functionCall` it emits,
        /// and **rejects the turn with a 400 if that signature is missing when
        /// the call is replayed**:
        ///
        /// > Function call is missing a thought_signature in functionCall parts.
        /// > This is required for tools to work correctly.
        ///
        /// We parsed the call and dropped the signature, so every multi-turn
        /// Gemini tool conversation failed on the second request — and Gemini is
        /// the primary provider for rollout 1. No fixture could have caught it:
        /// the captured payloads predate the requirement, and they go on passing
        /// forever. The live smoke (`P2-69`) found it on its second run.
        ///
        /// Deliberately opaque and deliberately generic. It is not named
        /// `thought_signature` because the other two providers may well
        /// introduce their own must-echo token, and the correct handling is
        /// identical: carry it, echo it, never interpret it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opaque: Option<String>,
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
    /// What caching to request (`P2-16`).
    ///
    /// A *policy shape*, not a provider name: `Explicit` means we place the
    /// breakpoints (Anthropic), `Automatic` means the vendor caches on its own
    /// and we only steer affinity (OpenAI, Gemini). An adapter that cannot
    /// honour the shape it is given ignores it rather than approximating —
    /// approximating a cache produces a request that looks cached and is not.
    ///
    /// Defaults to `Off`. A default that cached would send `cache_control` on
    /// behalf of callers who never asked, and a cache WRITE is billed above the
    /// input rate — so the wrong default costs money rather than saving it.
    #[serde(default)]
    pub cache: CachePolicy,

    /// Server-side clearing of stale tool results (`P3-25`).
    ///
    /// `None` means "do not ask for it", which is what every non-Anthropic
    /// adapter sees and ignores. Kept as a *policy shape* rather than a raw
    /// body fragment for the same reason `CachePolicy` is: an adapter that
    /// cannot honour it must ignore it, not approximate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clear_tool_uses: Option<ClearToolUses>,
}

/// Ask the provider to drop old tool results from the request server-side.
///
/// ⚠ **Verified against the live API, not against documentation** (`L-22`):
/// `POST /v1/messages` with `anthropic-beta: context-management-2025-06-27`
/// and this body shape returns **200**; the identical body **without** the beta
/// header returns **400 `context_management: Extra inputs are not permitted`**.
/// The header is load-bearing, and that is a measurement rather than a belief.
///
/// The response echoes `context_management.applied_edits`, which is how a
/// caller learns whether anything was actually cleared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClearToolUses {
    /// Clear once the request's input tokens exceed this.
    pub trigger_input_tokens: u32,
    /// Number of most-recent tool uses to keep intact.
    pub keep_tool_uses: u32,
    /// Do not bother clearing unless it would free at least this many tokens.
    pub clear_at_least_input_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub message_id: String,
    pub content: Vec<MessageContent>,
    /// The **raw** wire value, kept verbatim per provider.
    ///
    /// ⚠ This is NOT a normalised vocabulary. It said
    /// `"end_turn" | "tool_use" | "max_tokens"` until 2026-09-02 — Anthropic's
    /// three values, written as though every provider used them. OpenAI sends
    /// `stop` / `tool_calls` / `length`; Google sends `STOP` / `MAX_TOKENS` and
    /// several safety values. A caller that believed the old comment read an
    /// OpenAI truncation as a clean finish, and one did (`engine.rs`, P2-46).
    ///
    /// Classify with `StopReason::parse` rather than comparing to a literal.
    pub stop_reason: String,
    /// Six-field provider-reported usage. Replaced `tokens_in`/`tokens_out`
    /// in P2-01: two fields cannot price a cached call, and cache_read is the
    /// only instrument that can tell whether caching works at all.
    pub usage: Usage,
    pub model: String,
}

impl CompletionResponse {
    /// Record this response'"'"'s usage into `out` and return the response.
    ///
    /// A small convenience with a specific purpose (`P2-66`): it makes it hard
    /// to consume a response for its content while silently dropping what it
    /// cost, which is exactly what compaction did.
    pub fn usage_recorded(self, out: &mut crate::provider::usage::Usage) -> Self {
        *out = self.usage.clone();
        self
    }
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
        /// The opaque echo token, when the provider issues one on the streaming
        /// path. See `MessageContent::ToolUse::opaque` — Gemini rejects a
        /// replayed call whose `thoughtSignature` is missing, and streaming is
        /// the primary agent path, so it has to survive here as well as in the
        /// unary response.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opaque: Option<String>,
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
