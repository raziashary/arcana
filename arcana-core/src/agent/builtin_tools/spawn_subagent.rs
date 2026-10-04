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
    // `Network`: a subagent makes provider calls with the user's key and
    // then acts autonomously. Spawning one is a larger commitment than any
    // single file write.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Network
    }

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
                    "description": "Tool subset the subagent may use, e.g. [\"read_file\", \"grep\"]. OMIT OR LEAVE EMPTY TO INHERIT every tool you currently hold -- an empty list is NOT a restriction. Narrowing can only remove tools you already have; it can never widen access."
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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let task_description = match input.get("task_description").and_then(|v| v.as_str()) {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => return ToolResult::error("'task_description' is required and must not be empty"),
        };

        // K.11 — Guard against unbounded recursion. If the caller is already
        // a subagent at the depth limit, refuse to spawn further.
        // ⚠ P3-58, corrected by the post-phase claims audit (F-21).
        //
        // A local depth guard stood here. The task said it was "strictly
        // tighter" than the pool's and fired a generation earlier; my fix
        // rewrote it from `ctx.depth >= MAX` to `ctx.depth + 1 > MAX` and
        // claimed the boundary had moved.
        //
        // 🔴 **Both were wrong.** For `u32`, `d >= 3` and `d + 1 > 3` are the
        // SAME predicate — so the original premise was false and my fix was
        // arithmetically a no-op that changed nothing while claiming to. The
        // pool's depth branch stayed exactly as unreachable as before.
        //
        // The guard is gone instead. `pool.acquire` is the FIRST thing
        // `manager::spawn` does and it already rejects `child_depth > MAX`
        // with an actionable message, which every caller here surfaces. One
        // boundary, decided in one place — which is what the task asked for
        // and what neither version delivered.

        let mgr = match subagent::manager::global() {
            Some(m) => m,
            None => {
                // 🔴 `P8-58`. This used to end *"Wait for sidecar startup
                // to complete and try again."* There is no sidecar, and there
                // is nothing to wait for: `main.rs:193` calls `install_global`
                // unconditionally during desktop startup, so `global()` returns
                // `None` only in a process that never runs that line — the
                // `arcanactl` CLI, or a test binary. **Retrying cannot succeed**,
                // and the model was being told to retry.
                return ToolResult::error(
                    "Subagent system unavailable: this process has no \
                     SubagentManager, so no subagent can be spawned here. \
                     Retrying will not change that — the manager is installed \
                     once at desktop-app startup, and a process without it \
                     (the CLI, or a test harness) never gains one. Do the work \
                     directly instead, or report that delegation is not \
                     available in this context.",
                );
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
            // ── P2-64 · per-run facts go LAST, never first ────────────────
            //
            // `working_dir` used to sit at **offset 0** of every one of these,
            // and it differs per run. A prefix cache matches on exact bytes
            // from the start, so a differing first line makes everything after
            // it uncacheable — including `DEFAULT_SUBAGENT_SYSTEM_PROMPT`,
            // which is otherwise identical for every subagent ever spawned.
            //
            // A cache WRITE costs more than a normal input token, so requesting
            // caching over a prefix that can never be read back does not merely
            // fail to save — it adds cost to every call, silently.
            (Some(task_ctx), _) => format!(
                "You are an Arcana subagent. Perform your assigned subtask and \
                 produce a structured summary.\n\n\
                 [ARCANA WORKSPACE]\nworking_dir: {working_dir}\n\
                 Task context: {task_ctx}"
            ),
            (None, Some(sp)) => format!(
                "{sp}\n\n\
                 [ARCANA WORKSPACE]\nworking_dir: {working_dir}\n\
                 All file operations MUST be resolved relative to this absolute path \
                 unless an explicit absolute path is provided."
            ),
            (None, None) => format!(
                "{DEFAULT_SUBAGENT_SYSTEM_PROMPT}\n\n\
                 [ARCANA WORKSPACE]\nworking_dir: {working_dir}\n\
                 All file operations MUST be resolved relative to this absolute path \
                 unless an explicit absolute path is provided."
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
            // G-5: a child never outranks its parent.
            permission_mode: ctx.permission_mode.clone(),
        };

        // Spawn (returns immediately) then await the bridge task to drive it
        // synchronously from the parent's perspective.
        let handle = match mgr
            .spawn_with_run_token_budget(config, task_description, ctx.run_token_budget.clone())
            .await
        {
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
    "You are an Arcana subagent — a focused worker spawned by a parent agent \
     to handle a single, well-defined subtask. You have no memory of the \
     parent's conversation. Read the task carefully, do exactly what is \
     asked, and respond with a clear, structured summary that the parent \
     can act on.";
