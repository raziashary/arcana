// Subagent shared types — DTOs serialized to the frontend, payloads for the
// internal event stream, and the configuration surface used by Rust callers.
//
// See: docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md
//      docs/development/DEVELOPMENT_PLAN.md (Phase 4.9)
//
// Mirror these structures in src/stores/subagentStore.ts for type parity.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Request shape for spawning a subagent.
///
/// `parent_slug` is **required** because every subagent ID is namespaced under
/// its parent's slug (see `subagent::slug::generate`). This makes lineage
/// traceable in logs and the Subagent Monitor UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentConfig {
    /// Slug of the parent agent (e.g. "worker-backend", "supervisor", "pa").
    pub parent_slug: String,

    /// The permission mode the child runs under (`G-5`).
    ///
    /// Inherited from the parent, never chosen by the model. `native_runner`
    /// hardcoded `Autonomous` for every subagent regardless of its parent, so
    /// `spawn_subagent` was a one-call escape from `ManualApproval`.
    #[serde(default)]
    pub permission_mode: crate::agent::permission::PermissionMode,

    /// The child's ROLE — what kind of agent it is ("qc-agent", "subagent"),
    /// not which instance it is.
    ///
    /// These are different things and conflating them broke two consumers:
    /// `annotate_file_tool` matches the role to attribute an annotation, and
    /// the worker-messaging tools use it as an inbox address. When the role was
    /// set to the generated per-spawn id (`subagent-qc-dispatcher-{millis}-{n}`)
    /// every QC annotation was filed as `source = "user"` and no sender could
    /// address a subagent's inbox. Found by the P0 post-phase audit (F-04, F-05).
    #[serde(default = "default_subagent_role")]
    pub role: String,

    /// Parent's session ID — preserved for lineage but not propagated to the
    /// subagent, which always gets a fresh isolated session.
    pub parent_session_id: String,

    /// System prompt for the subagent. Inherits parent's prompt if empty.
    pub system_prompt: String,

    /// Tool subset the subagent is allowed to use (e.g. ["read_file", "grep"]).
    /// Empty vec = no `internal_tools` (subagent must rely on MCP only).
    pub tools: Vec<String>,

    /// LLM model name (registry key, e.g. "claude-4-5-haiku").
    /// Falls back to the default fleet model when empty.
    pub model: String,

    /// Working directory for the subagent (typically a fresh git worktree).
    /// Empty string = inherit parent's working_dir.
    pub working_dir: String,

    /// Hard cap on tool turns. Default 20.
    pub max_turns: u32,

    /// Hard cap on subagent runtime. Default 300s.
    pub timeout_secs: u32,

    /// Nesting depth (0 = spawned by a top-level agent, 1 = subagent of a
    /// subagent, etc.). Enforced by `SubagentPool` against `MAX_NESTING_DEPTH`.
    pub depth: u32,
}

impl SubagentConfig {
    /// Defaults applied by `SpawnSubagentTool` when the LLM omits fields.
    pub const DEFAULT_MAX_TURNS: u32 = 20;
    pub const DEFAULT_TIMEOUT_SECS: u32 = 300;
    /// Fallback model when the LLM omits the `model` field in a subtask.
    /// Matches the fleet default used by RecipeRunner.
    pub const DEFAULT_MODEL: &'static str = "gemini-3.8-flash";
}

/// Lifecycle state of a subagent. Drives UI status indicators and the
/// `SubagentManager`'s cleanup decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    /// Being registered; its task has not started yet.
    Spawning,
    /// Registered and awaiting its first turn.
    Idle,
    /// Its engine loop is running and emitting events.
    Running,
    /// Stream completed successfully; result accumulated.
    Completed,
    /// The run errored, timed out, or the engine reported failure.
    Failed,
    /// User or pool explicitly aborted; cleanup in progress or done.
    Terminated,
}

impl SubagentStatus {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Terminated)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Spawning => "spawning",
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Terminated => "terminated",
        }
    }
}

/// Live handle returned to callers immediately after `spawn_subagent`.
/// Tracked in `SubagentManager` until the lifecycle terminates.
///
/// The actual SSE event stream is held inside `SubagentManager` (not here)
/// so the handle stays cheaply cloneable for IPC across the Tauri boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentHandle {
    /// The unique slug generated for this subagent, e.g.
    /// "subagent-worker-backend-1745740800123-1".
    pub id: String,
    pub config: SubagentConfig,
    pub status: SubagentStatus,
    pub created_at: DateTime<Utc>,
}

/// Compact summary used by `list_active_subagents` for the Monitor UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentSummary {
    pub id: String,
    pub parent_slug: String,
    pub parent_session_id: String,
    pub status: SubagentStatus,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub depth: u32,
    /// Most recent event/message text for at-a-glance progress display.
    pub last_event: Option<String>,
}

/// Final outcome of a subagent's run. Returned to the parent's tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub subagent_id: String,
    pub status: SubagentStatus,
    /// Final accumulated text content from the subagent.
    pub result_summary: String,
    /// Files the subagent modified (best-effort; populated when worktree
    /// integration is enabled and a diff is available).
    pub files_modified: Vec<String>,
    /// Total cost in USD.
    pub cost_usd: f64,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub duration_secs: f64,
    /// Failure message when `status == Failed` or `Terminated`.
    pub error: Option<String>,
}

/// Streaming events forwarded from a subagent's engine loop to the Arcana
/// event bus.
///
/// The kind discriminator (`"token"`, `"tool_start"`, etc.) lets the frontend
/// reuse rendering logic from the existing chat event stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentEvent {
    /// Streamed token from the LLM.
    Token { subagent_id: String, text: String },
    /// Visible "thinking" content (chain-of-thought blocks).
    Thinking { subagent_id: String, text: String },
    /// Tool invocation started.
    ToolStart {
        subagent_id: String,
        tool: String,
        input: serde_json::Value,
    },
    /// Tool invocation completed.
    ToolEnd {
        subagent_id: String,
        tool: String,
        output: String,
        is_error: bool,
    },
    /// Final summary message; the SSE stream is about to close.
    Final { subagent_id: String, text: String },
    /// Cost / usage update (may arrive multiple times).
    Cost {
        subagent_id: String,
        cost_usd: f64,
        tokens_in: u32,
        tokens_out: u32,
    },
    /// Status transition broadcast (Spawning → Running → Completed, etc.).
    Status {
        subagent_id: String,
        status: SubagentStatus,
        message: Option<String>,
    },
    /// Free-form error event (non-fatal; stream continues unless followed
    /// by a `Status::Failed`).
    Error {
        subagent_id: String,
        message: String,
    },
}

impl SubagentEvent {
    pub fn subagent_id(&self) -> &str {
        match self {
            SubagentEvent::Token { subagent_id, .. }
            | SubagentEvent::Thinking { subagent_id, .. }
            | SubagentEvent::ToolStart { subagent_id, .. }
            | SubagentEvent::ToolEnd { subagent_id, .. }
            | SubagentEvent::Final { subagent_id, .. }
            | SubagentEvent::Cost { subagent_id, .. }
            | SubagentEvent::Status { subagent_id, .. }
            | SubagentEvent::Error { subagent_id, .. } => subagent_id,
        }
    }
}

/// Input shape for the `spawn_subagent` builtin tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnSubagentInput {
    pub task_description: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
}

/// Input shape for the `spawn_parallel_subagents` builtin tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnParallelSubagentsInput {
    pub subtasks: Vec<SpawnSubagentInput>,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: u32,
    #[serde(default = "default_aggregate")]
    pub aggregate_results: bool,
}

fn default_max_concurrent() -> u32 {
    5
}
fn default_aggregate() -> bool {
    true
}

fn default_subagent_role() -> String {
    "subagent".to_string()
}
