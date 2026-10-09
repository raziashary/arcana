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

#[cfg(test)]
mod p4_51_subagent_executor_matrix_tests {
    use crate::agent::permission::{new_pending_permissions, PermissionMode};
    use crate::agent::tool_executor::ToolExecutor;
    use crate::agent::tool_registry::ToolRegistry;
    use crate::agent::tool_trait::AgentIdentity;
    use serde_json::{json, Value};

    const SESSION_ID: &str = "p4-51-subagent-matrix-session";
    const AGENT_ID: &str = "worker-subagent-matrix";
    const TASK_ID: i64 = 51510;

    #[tokio::test]
    async fn registered_subagent_family_returns_correlated_private_failures_without_manager() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let workspace = tempfile::tempdir().expect("isolated subagent workspace");
        let _home_guard = crate::db::test_support::with_home(home.path());

        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        for name in [
            "spawn_subagent",
            "spawn_parallel_subagents",
            "spawn_qc_review",
        ] {
            assert!(
                registry.get(name).is_some(),
                "missing actual FastDevelopment registration: {name}"
            );
        }

        // ToolExecutor operation journaling is backed by the isolated home.
        // Starting this filtered test in a fresh test process leaves the
        // process-wide manager OnceLock empty, so the handlers fail before any
        // provider, DB claim, or network operation can begin.
        assert!(
            crate::subagent::manager::global().is_none(),
            "this exact-filter test requires an uninstalled process-local manager"
        );
        let run_id = crate::db::runs::RunsDb::open()
            .expect("open isolated RunsDb")
            .begin(crate::db::runs::kind::AGENT_TURN, Some(AGENT_ID))
            .expect("create isolated agent run");
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            SESSION_ID.to_string(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker(AGENT_ID),
        )
        .with_run_id(Some(run_id.clone()));
        executor.set_task_id(Some(TASK_ID));

        let planted = [
            "P4_51_PRIVATE_CREDENTIAL_CANARY",
            "P4_51_PRIVATE_PROMPT_CANARY",
            "P4_51_PRIVATE_SOURCE_CANARY",
            "P4_51_PRIVATE_ARGUMENT_CANARY",
            "P4_51_PRIVATE_PATH_CANARY",
        ];
        let cases = [
            (
                "spawn_subagent",
                json!({
                    "task_description": "delegate P4_51_PRIVATE_PROMPT_CANARY",
                    "system_prompt": "P4_51_PRIVATE_SOURCE_CANARY",
                    "api_key": "P4_51_PRIVATE_CREDENTIAL_CANARY",
                    "raw_argument": "P4_51_PRIVATE_ARGUMENT_CANARY",
                    "working_path": "P4_51_PRIVATE_PATH_CANARY",
                }),
                "spawn-subagent",
            ),
            (
                "spawn_parallel_subagents",
                json!({
                    "subtasks": [{
                        "task_description": "parallel task P4_51_PRIVATE_PROMPT_CANARY",
                        "system_prompt": "P4_51_PRIVATE_SOURCE_CANARY",
                    }],
                    "api_key": "P4_51_PRIVATE_CREDENTIAL_CANARY",
                    "raw_argument": "P4_51_PRIVATE_ARGUMENT_CANARY",
                    "working_path": "P4_51_PRIVATE_PATH_CANARY",
                }),
                "spawn-parallel-subagents",
            ),
            (
                "spawn_qc_review",
                json!({
                    "task_id": TASK_ID,
                    "repo_path": "P4_51_PRIVATE_PATH_CANARY",
                    "model": "P4_51_PRIVATE_ARGUMENT_CANARY",
                    "api_key": "P4_51_PRIVATE_CREDENTIAL_CANARY",
                    "prompt": "P4_51_PRIVATE_PROMPT_CANARY",
                    "source_text": "P4_51_PRIVATE_SOURCE_CANARY",
                }),
                "spawn-qc-review",
            ),
        ];

        for (name, input, call_id) in cases {
            let tool = registry
                .get(name)
                .unwrap_or_else(|| panic!("FastDevelopment registry omitted {name}"));
            let mut results = executor
                .execute_batch(vec![(tool, input, call_id.to_string())])
                .await
                .expect("production ToolExecutor should return a completed batch");
            assert_eq!(
                results.len(),
                1,
                "one registered call should yield one result"
            );
            let (returned_call_id, result) = results.pop().expect("one tool result");
            assert_eq!(returned_call_id, call_id, "model call ID must survive");
            assert!(
                result.is_error,
                "{name} reported success without a SubagentManager: {}",
                result.output
            );
            assert!(result.output.len() <= 8192, "{name} failure exceeded bound");
            for marker in planted {
                assert!(
                    !result.output.contains(marker),
                    "{name} reflected planted private value {marker:?}: {}",
                    result.output
                );
            }

            let envelope: Value = serde_json::from_str(&result.output)
                .unwrap_or_else(|error| panic!("{name} failure was not JSON: {error}"));
            assert_eq!(envelope["schema_version"], 1, "{name}: {envelope}");
            assert_eq!(envelope["ok"], false, "{name}: {envelope}");
            assert_eq!(envelope["tool"], name, "{name}: {envelope}");
            assert_eq!(envelope["operation"], format!("tool.{name}"));
            assert!(envelope["error_code"]
                .as_str()
                .is_some_and(|s| !s.is_empty()));
            assert!(envelope["category"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(envelope["message"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(envelope["cause"].as_str().is_some_and(|s| !s.is_empty()));
            assert!(envelope["retryable"].is_boolean());
            assert!(envelope["retry_after_ms"].is_null() || envelope["retry_after_ms"].is_u64());
            assert!(envelope["suggested_action"]
                .as_str()
                .is_some_and(|s| !s.is_empty()));
            for field in ["message", "cause", "suggested_action"] {
                assert!(
                    envelope[field].as_str().unwrap().len() <= 512,
                    "{name} {field} exceeded its bound"
                );
            }
            assert_eq!(envelope["run_id"], run_id, "{name}");
            assert_eq!(envelope["agent_id"], AGENT_ID, "{name}");
            assert_eq!(envelope["task_id"], TASK_ID, "{name}");
            assert_eq!(envelope["context"]["session_id"], SESSION_ID, "{name}");
            assert_eq!(envelope["context"]["run_id"], run_id, "{name}");
            assert_eq!(envelope["context"]["agent_id"], AGENT_ID, "{name}");
            assert_eq!(envelope["context"]["task_id"], TASK_ID, "{name}");
            assert_eq!(
                envelope["context"]["tool_call_id"],
                format!("{SESSION_ID}-{call_id}"),
                "{name}"
            );
            assert!(envelope["timestamp"]
                .as_str()
                .is_some_and(|s| !s.is_empty()));
            assert!(envelope["error"].is_object(), "{name}");
            for success_field in ["subagent_id", "results", "combined_summary", "claim_token"] {
                assert!(
                    envelope.get(success_field).is_none(),
                    "{name} failure carried success-shaped field {success_field}"
                );
            }
        }
    }
}
