// Hooks system types — Phase 3.1
// Event-driven extensibility at key lifecycle points throughout AutoDev.
// Hooks are persisted in ~/.autodev/config.db and executed by HookExecutor.

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// ── Hook Event Types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    // Agent lifecycle
    SessionStart,
    SessionEnd,
    AgentLoopStart,
    AgentLoopEnd,
    OnError,

    // Tool execution
    PreToolUse,
    PostToolUse,
    OnToolError,
    OnPermissionRequest,

    // File system
    PreFileRead,
    PostFileWrite,
    PreFileDelete,
    OnCheckpointCreate,
    OnCheckpointRestore,

    // Git
    PreCommit,
    PostCommit,
    PrePush,
    OnBranchSwitch,
    OnMergeConflict,

    // Plan management
    PrePlanStep,
    PostPlanStep,
    OnPlanActivate,
    OnPlanComplete,
    OnPlanAbandon,

    // Fleet
    OnTaskClaim,
    OnTaskComplete,
    OnTaskFail,
    OnQcReviewStart,
    OnQcApprove,
    OnQcFail,
}

impl HookType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart        => "session_start",
            Self::SessionEnd          => "session_end",
            Self::AgentLoopStart      => "agent_loop_start",
            Self::AgentLoopEnd        => "agent_loop_end",
            Self::OnError             => "on_error",
            Self::PreToolUse          => "pre_tool_use",
            Self::PostToolUse         => "post_tool_use",
            Self::OnToolError         => "on_tool_error",
            Self::OnPermissionRequest => "on_permission_request",
            Self::PreFileRead         => "pre_file_read",
            Self::PostFileWrite       => "post_file_write",
            Self::PreFileDelete       => "pre_file_delete",
            Self::OnCheckpointCreate  => "on_checkpoint_create",
            Self::OnCheckpointRestore => "on_checkpoint_restore",
            Self::PreCommit           => "pre_commit",
            Self::PostCommit          => "post_commit",
            Self::PrePush             => "pre_push",
            Self::OnBranchSwitch      => "on_branch_switch",
            Self::OnMergeConflict     => "on_merge_conflict",
            Self::PrePlanStep         => "pre_plan_step",
            Self::PostPlanStep        => "post_plan_step",
            Self::OnPlanActivate      => "on_plan_activate",
            Self::OnPlanComplete      => "on_plan_complete",
            Self::OnPlanAbandon       => "on_plan_abandon",
            Self::OnTaskClaim         => "on_task_claim",
            Self::OnTaskComplete      => "on_task_complete",
            Self::OnTaskFail          => "on_task_fail",
            Self::OnQcReviewStart     => "on_qc_review_start",
            Self::OnQcApprove         => "on_qc_approve",
            Self::OnQcFail            => "on_qc_fail",
        }
    }
}

// ── Hook Action Types ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookAction {
    Shell {
        command:     String,
        working_dir: Option<String>,
        timeout_secs: Option<u32>,
    },
    Script {
        path:    String,
        args:    Option<Vec<String>>,
    },
    Http {
        url:     String,
        method:  String,
        headers: Option<serde_json::Value>,
        body:    Option<serde_json::Value>,
    },
    Notification {
        title:   String,
        message: String,
    },
    Log {
        file:    String,
        message: String,
    },
    Block {
        message: Option<String>,
    },
    Modify {
        transform: String,          // JSON path expression for mutation
    },
    EmitEvent {
        event_name: String,
        payload:    Option<serde_json::Value>,
    },
}

// ── Hook Conditions ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HookConditions {
    pub tool_name:        Option<String>,
    pub file_pattern:     Option<String>,
    pub repo_path:        Option<String>,
    pub command_pattern:  Option<String>,
    pub session_type:     Option<String>,   // "pa" | "fleet"
    pub agent_mode:       Option<String>,   // "fast_dev" | "review" | "delegate"
    pub time_range:       Option<TimeRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_hour: u8,    // 0-23
    pub end_hour:   u8,
    pub days:       Option<Vec<String>>,   // ["mon","tue","wed","thu","fri"]
}

// ── Hook Config (stored in DB) ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    pub id:         String,
    pub name:       String,
    pub hook_type:  HookType,
    pub enabled:    bool,
    pub priority:   i32,                   // Lower = higher priority
    pub conditions: HookConditions,
    pub action:     HookAction,
    pub created_at: String,
    pub updated_at: String,
}

// ── Hook Execution Context (passed to action at runtime) ─────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub hook_type:   String,
    pub timestamp:   DateTime<Utc>,
    pub session_id:  Option<String>,
    pub agent_mode:  Option<String>,
    pub tool_call:   Option<ToolCallContext>,
    pub file_path:   Option<String>,
    pub task_id:     Option<String>,
    pub worker_id:   Option<String>,
    pub extra:       serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallContext {
    pub id:        String,
    pub name:      String,
    pub input:     serde_json::Value,
    pub result:    Option<serde_json::Value>,
    pub is_error:  bool,
}

// ── Hook Execution Result ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HookOutcome {
    Executed,
    Blocked,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookExecutionLog {
    pub id:          i64,
    pub hook_id:     String,
    pub hook_name:   String,
    pub hook_type:   String,
    pub outcome:     HookOutcome,
    pub output:      Option<String>,
    pub error:       Option<String>,
    pub executed_at: String,
    pub duration_ms: u64,
}

// ── Add Hook Input ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AddHookInput {
    pub name:       String,
    pub hook_type:  HookType,
    pub enabled:    Option<bool>,
    pub priority:   Option<i32>,
    pub conditions: Option<HookConditions>,
    pub action:     HookAction,
}
