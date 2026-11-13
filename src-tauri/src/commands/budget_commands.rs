// Budget Circuit Breaker — Tauri commands (Ruflo Feature 4, section 4.6).
//
// Thin command wrappers that delegate to db/tasks.rs and budget/mod.rs.
// Frontend calls these to set budgets, read live usage, and display daily summaries.

use tauri::command;

use crate::budget::{validate_budget, BudgetStatus, DailyBudgetSummary, TaskBudget};
use crate::db::tasks::TasksDb;
use crate::errors::Result;

// ── 4.6 set_task_budget ───────────────────────────────────────────────────────

/// Validate and persist budget limits for a task.
/// Called by the frontend when the user manually sets a budget, or by
/// internal code that bypasses the MCP tool layer.
#[command]
pub async fn cmd_set_task_budget(
    task_id: i64,
    max_tokens: Option<u64>,
    max_usd: Option<f64>,
    max_turns: Option<u32>,
) -> Result<TaskBudget> {
    let budget = validate_budget(max_tokens, max_usd, max_turns)
        .map_err(|code| crate::errors::ArcanaError::Agent(code.to_string()))?;

    let db = TasksDb::open()?;
    db.set_task_budget(task_id, budget.max_tokens, budget.max_usd, budget.max_turns)?;
    Ok(budget)
}

// ── 4.6 get_task_budget_status ────────────────────────────────────────────────

/// Returns the current token/cost/turn usage for a task.
/// Used by BudgetBar.tsx to render the live progress bar on a task card.
#[command]
pub async fn get_task_budget_status(task_id: i64) -> Result<BudgetStatus> {
    let db = TasksDb::open()?;
    db.get_task_budget_status(task_id)
}

// ── 4.6 get_daily_budget_summary ──────────────────────────────────────────────

/// Aggregates all tasks created today: allocated vs spent tokens/USD, exceeded count.
/// Used by the Dashboard footer and Settings budget overview card.
#[command]
pub async fn get_daily_budget_summary() -> Result<DailyBudgetSummary> {
    let db = TasksDb::open()?;
    db.get_daily_budget_summary()
}

// ── P2-50 · the daily spend ceiling, exposed to the UI ────────────────────────
//
// There were two budgets and only one of them did anything.
//
// `DailyCap` is the ceiling that actually enforces: `CappedProvider` reads it
// before every call, refuses the request once the day's spend reaches it, and
// pauses the fleet. It lives in app settings and is off by default.
//
// The frontend had a *different* number. `dashboardStore.ts` hardcoded
// `dailyBudget: 50` and `CostMeter.tsx` hardcoded 50 again, used only to raise a
// warning banner — and `setDailyBudget` existed with no caller, so the figure
// could not be changed at all.
//
// So the number the user saw and the number that stopped their work were
// unrelated, and neither was theirs: a user could watch a banner count toward
// "$50" while the enforcing cap was disabled entirely, or hit a real ceiling
// with no warning having appeared. These two commands make it one number.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DailyCapSettings {
    /// Whether the ceiling is enforced at all.
    ///
    /// Off by default and stays off until the user turns it on: a ceiling
    /// nobody chose, stopping work partway through a task, is worse than no
    /// ceiling (`§0.15` — a capability existing is not consent to use it).
    pub enabled: bool,
    pub limit_usd: f64,
}

/// Read the configured ceiling.
#[command]
pub fn get_daily_cap() -> DailyCapSettings {
    let cap = crate::budget::daily::DailyCap::load();
    DailyCapSettings {
        enabled: cap.enabled,
        limit_usd: cap.limit_usd,
    }
}

/// Set the ceiling.
///
/// Validation lives in `DailyCap::save`, which refuses a non-finite or
/// non-positive limit — a `NaN` cap compares false against every spend, so
/// storing one would silently disable the very ceiling it was meant to set. The
/// error is returned to the UI rather than swallowed, so the user learns the
/// value was rejected instead of believing a cap is in force.
#[command]
pub fn set_daily_cap(enabled: bool, limit_usd: f64) -> Result<DailyCapSettings> {
    let cap = crate::budget::daily::DailyCap { enabled, limit_usd };
    cap.save()?;
    Ok(DailyCapSettings {
        enabled: cap.enabled,
        limit_usd: cap.limit_usd,
    })
}

#[cfg(test)]
mod daily_cap_tests {
    use super::*;

    #[test]
    fn a_nan_or_non_positive_limit_is_refused_rather_than_stored() {
        // Why validation sits in `save` and this command returns its error:
        // NaN compares false against every spend, so a stored NaN reads as
        // "never exceeded" — the cap would appear set and enforce nothing.
        assert!(set_daily_cap(true, f64::NAN).is_err());
        assert!(set_daily_cap(true, f64::INFINITY).is_err());
        assert!(set_daily_cap(true, 0.0).is_err());
        assert!(set_daily_cap(true, -5.0).is_err());
    }
}
