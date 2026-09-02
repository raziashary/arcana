// Budget Circuit Breaker — Tauri commands (Ruflo Feature 4, section 4.6).
//
// Thin command wrappers that delegate to db/tasks.rs and budget/mod.rs.
// Frontend calls these to set budgets, read live usage, and display daily summaries.

use tauri::command;

use crate::budget::{validate_budget, BudgetStatus, DailyBudgetSummary, TaskBudget};
use crate::db::tasks::TasksDb;
use crate::errors::Result;
use crate::runtime_config;

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
        .map_err(|code| crate::errors::AutoDevError::Agent(code.to_string()))?;

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

// ── 4.8 set_default_worker_budget ────────────────────────────────────────────

/// Persist default budget settings so the Rust recipe runner can read them
/// when assigning tasks without an explicit budget override.
/// Stored in `~/.autodev/runtime_config.json` alongside skip_sidecars.
#[command]
pub async fn set_default_worker_budget(
    max_tokens: u64,
    max_usd: f64,
    max_turns: u32,
    warn_pct: u8,
) -> Result<runtime_config::RuntimeConfig> {
    // Validate using the same ceiling rules as validate_budget
    if let Err(code) = validate_budget(Some(max_tokens), Some(max_usd), Some(max_turns)) {
        return Err(crate::errors::AutoDevError::Agent(code.to_string()));
    }
    if warn_pct > 100 {
        return Err(crate::errors::AutoDevError::Agent(
            "warn_pct must be 0-100".into(),
        ));
    }
    let mut cfg = runtime_config::read();
    cfg.default_worker_budget_tokens = max_tokens;
    cfg.default_worker_budget_usd = max_usd;
    cfg.default_worker_budget_turns = max_turns;
    cfg.budget_warn_pct = warn_pct;
    runtime_config::write(&cfg)
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Config write failed: {e}")))?;
    Ok(cfg)
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
