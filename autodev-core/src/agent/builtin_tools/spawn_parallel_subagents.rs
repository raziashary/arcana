// spawn_parallel_subagents — Phase 4.9 builtin tool.
//
// Spawns multiple subagents concurrently with a shared concurrency cap and
// returns aggregated results. Use cases:
//   - Reviewing N files in parallel (1 subagent per file)
//   - Trying M strategies in parallel (1 subagent per strategy)
//   - Sharding analysis across a large codebase
//
// Concurrency is limited by:
//   - The tool-level `max_concurrent` parameter (cosmetic batching)
//   - SubagentPool's MAX_GLOBAL_CONCURRENT and MAX_PER_PARENT (hard limits)
//
// All subagents are awaited even if some fail; the result list mirrors
// the input ordering so the parent can correlate subtasks by index.

use async_trait::async_trait;
use futures::future;
use serde_json::{json, Value};

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::subagent::{self, SubagentConfig};

pub struct SpawnParallelSubagentsTool;

#[async_trait]
impl Tool for SpawnParallelSubagentsTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Network
    }

    fn name(&self) -> &str {
        "spawn_parallel_subagents"
    }

    fn description(&self) -> &str {
        "Spawn 2-10 subagents in parallel and aggregate their results. \
         Each subtask runs in its own isolated context. Use for fan-out \
         workloads (parallel file review, multi-strategy exploration, \
         codebase sharding). Returns one result per subtask in the same \
         order as the input."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subtasks": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": 10,
                    "description": "Array of subtask specs. Each spec is an object with at minimum 'task_description'.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "task_description": { "type": "string" },
                            "system_prompt":    { "type": "string" },
                            "tools":            { "type": "array", "items": { "type": "string" } },
                            "model":            { "type": "string" },
                            "max_turns":        { "type": "integer" },
                            "timeout_secs":     { "type": "integer" }
                        },
                        "required": ["task_description"]
                    }
                },
                "max_concurrent": {
                    "type": "integer",
                    "description": "Soft cap on simultaneous in-flight subagents (default 5). Hard caps come from SubagentPool."
                },
                "aggregate_results": {
                    "type": "boolean",
                    "description": "When true (default), include a combined summary in the response."
                }
            },
            "required": ["subtasks"]
        })
    }


    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let subtasks = match input.get("subtasks").and_then(|v| v.as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => return ToolResult::error("'subtasks' must be a non-empty array"),
        };

        if subtasks.len() > 10 {
            return ToolResult::error("'subtasks' must contain at most 10 items");
        }

        // K.11 — Depth guard (same as spawn_subagent).
        const MAX_SUBAGENT_DEPTH: u32 = 3;
        if ctx.depth >= MAX_SUBAGENT_DEPTH {
            return ToolResult::error(format!(
                "Subagent depth limit ({MAX_SUBAGENT_DEPTH}) reached — \
                 complete this work directly rather than spawning parallel subagents."
            ));
        }

        let aggregate = input
            .get("aggregate_results")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Soft cap on how many subagents run simultaneously. Subtasks are
        // processed in windows of this size: spawn a batch, await it fully,
        // then move to the next batch. The pool's hard caps (MAX_GLOBAL_CONCURRENT,
        // MAX_PER_PARENT) still apply within each batch.
        let max_concurrent: usize = input
            .get("max_concurrent")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5)
            .max(1);

        let mgr = match subagent::manager::global() {
            Some(m) => m,
            None => {
                return ToolResult::error(
                    "Subagent system unavailable: SubagentManager not yet initialised.",
                )
            }
        };

        // Convert each subtask into a (config, prompt) pair.
        let mut spawn_inputs = Vec::with_capacity(subtasks.len());
        for (i, st) in subtasks.iter().enumerate() {
            let task_description = match st.get("task_description").and_then(|v| v.as_str()) {
                Some(s) if !s.trim().is_empty() => s.trim().to_string(),
                _ => {
                    return ToolResult::error(&format!(
                        "subtasks[{i}].task_description is required and must not be empty"
                    ))
                }
            };
            // Always inject working_dir into the system prompt — parallel subagents have
            // no parent context and would otherwise resolve paths against the wrong root.
            let base_prompt = st
                .get("system_prompt")
                .and_then(|v| v.as_str())
                .unwrap_or(DEFAULT_SUBAGENT_SYSTEM_PROMPT);
            let effective_prompt = format!(
                "[AUTODEV WORKSPACE]\nworking_dir: {}\n\
                 All file operations MUST be resolved relative to this absolute path \
                 unless an explicit absolute path is provided.\n\n{}",
                ctx.working_dir, base_prompt
            );
            let config = SubagentConfig {
                parent_slug: ctx.session_id.clone(),
                // A generic delegated worker.
                role: "subagent".to_string(),
                parent_session_id: ctx.session_id.clone(),
                system_prompt: effective_prompt,
                tools: st
                    .get("tools")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                model: st
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                working_dir: ctx.working_dir.clone(),
                max_turns: st
                    .get("max_turns")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0),
                timeout_secs: st
                    .get("timeout_secs")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(0),
                depth: ctx.depth + 1, // K.11 — increment nesting depth
            };
            spawn_inputs.push((config, task_description));
        }

        // Process subtasks in chunks of max_concurrent. Each chunk is fully
        // spawned and awaited before the next chunk begins, enforcing the soft cap.
        let mut results: Vec<Result<crate::subagent::SubagentResult, String>> =
            Vec::with_capacity(spawn_inputs.len());

        for chunk in spawn_inputs.chunks(max_concurrent) {
            // Spawn every subagent in this chunk (sequential — pool handles back-pressure).
            let mut chunk_ids: Vec<Result<String, String>> = Vec::with_capacity(chunk.len());
            for (config, prompt) in chunk {
                match mgr.spawn(config.clone(), prompt.clone()).await {
                    Ok(h) => chunk_ids.push(Ok(h.id)),
                    Err(e) => chunk_ids.push(Err(e.to_string())),
                }
            }

            // Await all subagents in this chunk in parallel.
            let chunk_futs = chunk_ids.into_iter().map(|id_res| {
                let mgr = mgr.clone();
                async move {
                    match id_res {
                        Err(e) => Err(e),
                        Ok(id) => mgr.await_result(&id).await.map_err(|e| e.to_string()),
                    }
                }
            });
            let chunk_results = future::join_all(chunk_futs).await;
            results.extend(chunk_results);
        }

        // Build the response, mirroring input ordering.
        let mut total_cost: f64 = 0.0;
        let mut total_in: u32 = 0;
        let mut total_out: u32 = 0;
        let mut total_dur: f64 = 0.0;
        let mut summaries = Vec::with_capacity(results.len());

        let response_results: Vec<Value> = results
            .into_iter()
            .enumerate()
            .map(|(i, res)| {
                match res {
                    Ok(r) => {
                        total_cost += r.cost_usd;
                        total_in += r.tokens_in;
                        total_out += r.tokens_out;
                        if r.duration_secs > total_dur {
                            total_dur = r.duration_secs;
                        } // wall time = max
                        summaries.push(format!("[{i}] {}", r.result_summary));
                        json!({
                            "index":          i,
                            "subagent_id":    r.subagent_id,
                            "status":         r.status.as_str(),
                            "result_summary": r.result_summary,
                            "files_modified": r.files_modified,
                            "cost_usd":       r.cost_usd,
                            "tokens_in":      r.tokens_in,
                            "tokens_out":     r.tokens_out,
                            "duration_secs":  r.duration_secs,
                            "error":          r.error,
                        })
                    }
                    Err(msg) => {
                        summaries.push(format!("[{i}] FAILED: {msg}"));
                        json!({
                            "index":  i,
                            "status": "failed",
                            "error":  msg,
                        })
                    }
                }
            })
            .collect();

        let combined = if aggregate {
            summaries.join("\n\n---\n\n")
        } else {
            String::new()
        };

        ToolResult::success(
            json!({
                "results":             response_results,
                "combined_summary":    combined,
                "total_cost_usd":      total_cost,
                "total_tokens_in":     total_in,
                "total_tokens_out":    total_out,
                "total_duration_secs": total_dur,
            })
            .to_string(),
        )
    }
}

const DEFAULT_SUBAGENT_SYSTEM_PROMPT: &str =
    "You are an AutoDev subagent — a focused worker spawned by a parent agent \
     as part of a parallel batch. Each batch member runs in isolation. \
     Read your task carefully and produce a structured summary the parent \
     can aggregate with the other batch members.";
