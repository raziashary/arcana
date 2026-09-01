// spawn_subagent — Phase 4.9 builtin tool.
//
// Lets a parent agent (PA, Worker, Supervisor) spawn an ephemeral subagent
// with isolated context, a custom system prompt, and (optionally) a
// restricted tool subset. The parent BLOCKS until the subagent completes,
// then receives a structured result.
//
// Pattern: matches Claude Code's Task tool and Goose's SubagentExecutionTool.
// See: docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::subagent::{self, SubagentConfig, SubagentResult, SubagentStatus};

pub struct SpawnSubagentTool;

#[async_trait]
impl Tool for SpawnSubagentTool {
    fn name(&self) -> &str {
        "spawn_subagent"
    }

    fn description(&self) -> &str {
        "Spawn a single isolated subagent to execute a focused subtask. \
         Returns the subagent's final result summary. Use for tasks that \
         (a) benefit from a fresh context window, (b) need a different \
         system prompt, or (c) should run with a restricted tool subset. \
         Parent blocks until subagent completes."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_description": {
                    "type": "string",
                    "description": "What the subagent should do. Be specific — the subagent has no parent context."
                },
                "system_prompt": {
                    "type": "string",
                    "description": "Custom system prompt for the subagent (optional). If empty, inherits a generic 'helpful subagent' prompt."
                },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Tool subset (e.g. [\"read_file\", \"grep\"]). Empty = MCP tools only."
                },
                "model": {
                    "type": "string",
                    "description": "LLM model registry key (optional). Falls back to default fleet model."
                },
                "task_context": {
                    "type": "string",
                    "description": "Optional: task title + acceptance criteria to orient the subagent. \
                                   When provided, the subagent receives a system prompt that includes \
                                   the working directory and this context, producing more targeted output."
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max tool turns. Default 20."
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds. Default 300."
                }
            },
            "required": ["task_description"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_description = match input.get("task_description").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'task_description' is required and must not be empty"),
        };

        // K.11 — Guard against unbounded recursion. If the caller is already
        // a subagent at the depth limit, refuse to spawn further.
        const MAX_SUBAGENT_DEPTH: u32 = 3;
        if ctx.depth >= MAX_SUBAGENT_DEPTH {
            return ToolResult::error(format!(
                "Subagent depth limit ({MAX_SUBAGENT_DEPTH}) reached — \
                 complete this work directly rather than spawning a subagent."
            ));
        }

        let mgr = match subagent::manager::global() {
            Some(m) => m,
            None => {
                return ToolResult::error(
                    "Subagent system unavailable: SubagentManager not yet initialised. \
                 Wait for sidecar startup to complete and try again.",
                )
            }
        };

        // K.12 — Build a richer system prompt when task_context is provided.
        // ALL branches inject working_dir — without it, subagents have no idea
        // which directory to resolve file operations against.
        let working_dir = &ctx.working_dir;
        let system_prompt = match (
            input.get("task_context").and_then(|v| v.as_str()),
            input.get("system_prompt").and_then(|v| v.as_str()),
        ) {
            (Some(task_ctx), _) => format!(
                "You are an AutoDev subagent. Working directory: {working_dir}.\n\
                 Task context: {task_ctx}\n\
                 Perform your assigned subtask and produce a structured summary."
            ),
            (None, Some(sp)) => format!(
                "[AUTODEV WORKSPACE]\nworking_dir: {working_dir}\n\
                 All file operations MUST be resolved relative to this absolute path \
                 unless an explicit absolute path is provided.\n\n{sp}"
            ),
            (None, None) => format!(
                "[AUTODEV WORKSPACE]\nworking_dir: {working_dir}\n\
                 All file operations MUST be resolved relative to this absolute path \
                 unless an explicit absolute path is provided.\n\n\
                 {DEFAULT_SUBAGENT_SYSTEM_PROMPT}"
            ),
        };

        let config = SubagentConfig {
            parent_slug: ctx.session_id.clone(),
            // A generic delegated worker.
            role: "subagent".to_string(),
            parent_session_id: ctx.session_id.clone(),
            system_prompt,
            tools: input
                .get("tools")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            model: input
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            working_dir: ctx.working_dir.clone(),
            max_turns: input
                .get("max_turns")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0),
            timeout_secs: input
                .get("timeout_secs")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0),
            depth: ctx.depth + 1, // K.11 — increment nesting depth
        };

        // Spawn (returns immediately) then await the bridge task to drive it
        // synchronously from the parent's perspective.
        let handle = match mgr.spawn(config, task_description).await {
            Ok(h) => h,
            Err(e) => return ToolResult::error(&format!("spawn_subagent failed: {e}")),
        };

        let result: SubagentResult = match mgr.await_result(&handle.id).await {
            Ok(r) => r,
            Err(e) => return ToolResult::error(&format!("subagent run failed: {e}")),
        };

        if matches!(
            result.status,
            SubagentStatus::Failed | SubagentStatus::Terminated
        ) {
            return ToolResult::error(&format!(
                "Subagent {} {}: {}",
                result.subagent_id,
                result.status.as_str(),
                result.error.as_deref().unwrap_or("(no error message)")
            ));
        }

        ToolResult::success(
            json!({
                "subagent_id":    result.subagent_id,
                "status":         result.status.as_str(),
                "result_summary": result.result_summary,
                "files_modified": result.files_modified,
                "cost_usd":       result.cost_usd,
                "tokens_in":      result.tokens_in,
                "tokens_out":     result.tokens_out,
                "duration_secs":  result.duration_secs,
            })
            .to_string(),
        )
    }
}

const DEFAULT_SUBAGENT_SYSTEM_PROMPT: &str =
    "You are an AutoDev subagent — a focused worker spawned by a parent agent \
     to handle a single, well-defined subtask. You have no memory of the \
     parent's conversation. Read the task carefully, do exactly what is \
     asked, and respond with a clear, structured summary that the parent \
     can act on.";
