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
    task_id:    i64,
    max_tokens: Option<u64>,
    max_usd:    Option<f64>,
    max_turns:  Option<u32>,
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
    max_usd:    f64,
    max_turns:  u32,
    warn_pct:   u8,
) -> Result<runtime_config::RuntimeConfig> {
    // Validate using the same ceiling rules as validate_budget
    if let Err(code) = validate_budget(Some(max_tokens), Some(max_usd), Some(max_turns)) {
        return Err(crate::errors::AutoDevError::Agent(code.to_string()));
    }
    if warn_pct > 100 {
        return Err(crate::errors::AutoDevError::Agent("warn_pct must be 0-100".into()));
    }
    let mut cfg = runtime_config::read();
    cfg.default_worker_budget_tokens = max_tokens;
    cfg.default_worker_budget_usd    = max_usd;
    cfg.default_worker_budget_turns  = max_turns;
    cfg.budget_warn_pct              = warn_pct;
    runtime_config::write(&cfg)
        .map_err(|e| crate::errors::AutoDevError::Agent(format!("Config write failed: {e}")))?;
    Ok(cfg)
}
