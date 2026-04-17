// HookConditionEvaluator — checks whether a HookConfig's conditions match
// the current HookContext. Phase 3.1

use regex::Regex;
use glob::Pattern;
use chrono::Timelike;

use super::types::{HookConditions, HookContext};

pub struct HookConditionEvaluator;

impl HookConditionEvaluator {
    /// Returns `true` if all conditions in `cond` are satisfied by `ctx`.
    /// An empty / None condition is always satisfied.
    pub fn matches(cond: &HookConditions, ctx: &HookContext) -> bool {
        // tool_name: exact match against ctx.tool_call.name
        if let Some(ref expected) = cond.tool_name {
            let actual = ctx.tool_call.as_ref().map(|t| t.name.as_str()).unwrap_or("");
            if actual != expected { return false; }
        }

        // file_pattern: glob against ctx.file_path
        if let Some(ref pattern) = cond.file_pattern {
            let file = ctx.file_path.as_deref().unwrap_or("");
            if let Ok(glob) = Pattern::new(pattern) {
                if !glob.matches(file) { return false; }
            }
        }

        // repo_path: glob prefix check
        if let Some(ref rpath) = cond.repo_path {
            let file = ctx.file_path.as_deref().unwrap_or("");
            if let Ok(glob) = Pattern::new(rpath) {
                if !glob.matches(file) { return false; }
            }
        }

        // command_pattern: regex match against tool_call.input["command"]
        if let Some(ref pattern) = cond.command_pattern {
            let cmd = ctx.tool_call.as_ref()
                .and_then(|t| t.input.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if let Ok(re) = Regex::new(pattern) {
                if !re.is_match(cmd) { return false; }
            }
        }

        // session_type: "pa" | "fleet"
        if let Some(ref expected) = cond.session_type {
            let actual = ctx.agent_mode.as_deref().unwrap_or("");
            if actual != expected { return false; }
        }

        // agent_mode: "fast_dev" | "review" | "delegate"
        if let Some(ref expected) = cond.agent_mode {
            let actual = ctx.agent_mode.as_deref().unwrap_or("");
            if actual != expected { return false; }
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
            if !in_range { return false; }

            if let Some(ref days) = tr.days {
                let day = now.format("%a").to_string().to_lowercase();
                let day3 = &day[..3];
                if !days.iter().any(|d| d.to_lowercase().starts_with(day3)) {
                    return false;
                }
            }
        }

        true
    }
}
