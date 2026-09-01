// HookConditionEvaluator — checks whether a HookConfig's conditions match
// the current HookContext. Phase 3.1
//
// custom_condition uses the Rhai scripting engine for expression evaluation.
// Available variables in scope:
//   tool_name, file_path, agent_mode, command, hook_type,
//   session_id, worker_id, task_id, is_error
// Example: `tool_name == "bash" && command.contains("rm -rf")`

use chrono::Timelike;
use glob::Pattern;
use regex::Regex;
use rhai::{Engine, Scope};

use super::types::{HookConditions, HookContext};

/// Build a sandboxed Rhai engine for condition evaluation.
/// No file system, no process, no networking — read-only expression evaluation.
fn build_engine() -> Engine {
    let mut engine = Engine::new();
    // Restrict to safe operations: no file/env/process access
    engine.set_max_expr_depths(32, 16);
    engine.set_max_operations(10_000);
    engine.set_max_string_size(4096);
    engine
}

pub struct HookConditionEvaluator;

impl HookConditionEvaluator {
    /// Returns `true` if all conditions in `cond` are satisfied by `ctx`.
    /// An empty / None condition is always satisfied.
    pub fn matches(cond: &HookConditions, ctx: &HookContext) -> bool {
        // tool_name: exact match against ctx.tool_call.name
        if let Some(ref expected) = cond.tool_name {
            let actual = ctx
                .tool_call
                .as_ref()
                .map(|t| t.name.as_str())
                .unwrap_or("");
            if actual != expected {
                return false;
            }
        }

        // file_pattern: glob against ctx.file_path
        if let Some(ref pattern) = cond.file_pattern {
            let file = ctx.file_path.as_deref().unwrap_or("");
            if let Ok(glob) = Pattern::new(pattern) {
                if !glob.matches(file) {
                    return false;
                }
            }
        }

        // repo_path: glob prefix check
        if let Some(ref rpath) = cond.repo_path {
            let file = ctx.file_path.as_deref().unwrap_or("");
            if let Ok(glob) = Pattern::new(rpath) {
                if !glob.matches(file) {
                    return false;
                }
            }
        }

        // command_pattern: regex match against tool_call.input["command"]
        if let Some(ref pattern) = cond.command_pattern {
            let cmd = ctx
                .tool_call
                .as_ref()
                .and_then(|t| t.input.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Ok(re) = Regex::new(pattern) {
                if !re.is_match(cmd) {
                    return false;
                }
            }
        }

        // session_type: "pa" | "fleet"
        if let Some(ref expected) = cond.session_type {
            let actual = ctx.agent_mode.as_deref().unwrap_or("");
            if actual != expected {
                return false;
            }
        }

        // agent_mode: "fast_dev" | "review" | "delegate"
        if let Some(ref expected) = cond.agent_mode {
            let actual = ctx.agent_mode.as_deref().unwrap_or("");
            if actual != expected {
                return false;
            }
        }

        // time_range: hour window
        if let Some(ref tr) = cond.time_range {
            let now = chrono::Utc::now();
            let hour = now.hour() as u8;
            let in_range = if tr.start_hour <= tr.end_hour {
                hour >= tr.start_hour && hour < tr.end_hour
            } else {
                // Wraps midnight
                hour >= tr.start_hour || hour < tr.end_hour
            };
            if !in_range {
                return false;
            }

            if let Some(ref days) = tr.days {
                let day = now.format("%a").to_string().to_lowercase();
                let day3 = &day[..3];
                if !days.iter().any(|d| d.to_lowercase().starts_with(day3)) {
                    return false;
                }
            }
        }

        // custom_condition: Rhai expression evaluated with context variables in scope.
        // Errors (syntax, type, timeout) are treated as `false` (fail-safe).
        if let Some(ref expr) = cond.custom_condition {
            if !expr.trim().is_empty() {
                if !Self::eval_custom(expr, ctx) {
                    return false;
                }
            }
        }

        true
    }

    /// Evaluate a Rhai expression with hook context variables populated in scope.
    /// Returns `false` on any error — expressions should never crash the hook system.
    fn eval_custom(expr: &str, ctx: &HookContext) -> bool {
        let engine = build_engine();
        let mut scope = Scope::new();

        // Populate scope with every useful context field as typed variables
        scope.push("hook_type", ctx.hook_type.clone());
        scope.push("session_id", ctx.session_id.clone().unwrap_or_default());
        scope.push("agent_mode", ctx.agent_mode.clone().unwrap_or_default());
        scope.push("file_path", ctx.file_path.clone().unwrap_or_default());
        scope.push("task_id", ctx.task_id.clone().unwrap_or_default());
        scope.push("worker_id", ctx.worker_id.clone().unwrap_or_default());

        // Tool call fields
        let (tool_name, command, is_error) = if let Some(ref tc) = ctx.tool_call {
            let cmd = tc
                .input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            (tc.name.clone(), cmd, tc.is_error)
        } else {
            (String::new(), String::new(), false)
        };

        scope.push("tool_name", tool_name);
        scope.push("command", command);
        scope.push("is_error", is_error);

        // Evaluate — treat any error as false
        match engine.eval_expression_with_scope::<bool>(&mut scope, expr) {
            Ok(result) => result,
            Err(e) => {
                tracing::warn!("[hooks] custom_condition eval error: {e} (expr: {expr:?})");
                false
            }
        }
    }
}
