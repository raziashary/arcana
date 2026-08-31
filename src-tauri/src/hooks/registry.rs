// HookRegistry — loads hooks from SQLite, dispatches execution.
// Priority: lower number = runs first (1000 = highest priority).
// Phase 3.1

use tracing::{debug, warn};

use super::conditions::HookConditionEvaluator;
use super::db::HooksDb;
use super::executor::HookExecutor;
use super::types::{HookContext, HookExecutionLog, HookOutcome, HookType};

pub struct HookRegistry;

impl HookRegistry {
    /// Fire all enabled hooks matching `hook_type` and `ctx`.
    /// Returns true if execution should continue (false only if a `Block` hook fires).
    pub async fn fire(hook_type: &HookType, ctx: &HookContext) -> bool {
        let db = match HooksDb::open() {
            Ok(d) => d,
            Err(e) => {
                warn!("[HookRegistry] Cannot open hooks DB: {e}");
                return true;
            }
        };

        let hooks = match db.list_by_type(hook_type) {
            Ok(h) => h,
            Err(e) => {
                warn!("[HookRegistry] list_by_type error: {e}");
                return true;
            }
        };

        if hooks.is_empty() {
            return true;
        }

        // Sort by priority ascending (lower = first)
        let mut sorted = hooks;
        sorted.sort_by_key(|h| h.priority);

        for hook in &sorted {
            if !hook.enabled {
                continue;
            }
            if !HookConditionEvaluator::matches(&hook.conditions, ctx) {
                continue;
            }

            debug!(
                "[HookRegistry] firing hook '{}' for {:?}",
                hook.name, hook_type
            );
            let log = HookExecutor::execute(hook, ctx).await;

            // Persist log
            if let Err(e) = db.insert_log(&log) {
                warn!("[HookRegistry] Failed to persist hook log: {e}");
            }

            // Block hooks stop execution immediately
            if log.outcome == HookOutcome::Blocked {
                warn!("[HookRegistry] Hook '{}' blocked the action", hook.name);
                return false;
            }
        }

        true
    }

    /// Fire hooks and return all execution logs (for debugging / testing).
    pub async fn fire_with_logs(hook_type: &HookType, ctx: &HookContext) -> Vec<HookExecutionLog> {
        let db = match HooksDb::open() {
            Ok(d) => d,
            Err(e) => {
                warn!("[HookRegistry] Cannot open hooks DB: {e}");
                return vec![];
            }
        };

        let hooks = match db.list_by_type(hook_type) {
            Ok(h) => h,
            Err(e) => {
                warn!("[HookRegistry] list_by_type error: {e}");
                return vec![];
            }
        };

        let mut logs = Vec::new();
        let mut sorted = hooks;
        sorted.sort_by_key(|h| h.priority);

        for hook in &sorted {
            if !hook.enabled {
                continue;
            }
            if !HookConditionEvaluator::matches(&hook.conditions, ctx) {
                continue;
            }

            let log = HookExecutor::execute(hook, ctx).await;
            let _ = db.insert_log(&log);
            let blocked = log.outcome == HookOutcome::Blocked;
            logs.push(log);
            if blocked {
                break;
            }
        }

        logs
    }
}
