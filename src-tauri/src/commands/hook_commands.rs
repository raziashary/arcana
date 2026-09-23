// Tauri commands — Hooks system. Phase 3.1
// list_hooks, enable_hook, disable_hook, add_hook, remove_hook,
// test_hook, get_hook_logs

use crate::errors::Result;
use crate::hooks::db::HooksDb;
use crate::hooks::types::{AddHookInput, HookConfig, HookContext, HookExecutionLog};

// ── list_hooks ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_hooks() -> Result<Vec<HookConfig>> {
    HooksDb::open()?.list_all()
}

// ── enable_hook / disable_hook ────────────────────────────────────────────────

#[tauri::command]
pub fn enable_hook(hook_id: String) -> Result<()> {
    HooksDb::open()?.set_enabled(&hook_id, true)
}

#[tauri::command]
pub fn disable_hook(hook_id: String) -> Result<()> {
    HooksDb::open()?.set_enabled(&hook_id, false)
}

// ── add_hook ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn add_hook(config: AddHookInput) -> Result<HookConfig> {
    HooksDb::open()?.insert(&config)
}

// ── remove_hook ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn remove_hook(hook_id: String) -> Result<()> {
    HooksDb::open()?.delete(&hook_id)
}

// ── test_hook ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn test_hook(hook_id: String, context: HookContext) -> Result<Vec<HookExecutionLog>> {
    let db = HooksDb::open()?;
    let hook = db.get(&hook_id)?.ok_or_else(|| {
        crate::errors::ArcanaError::Agent(format!("Hook '{}' not found", hook_id))
    })?;

    use crate::hooks::executor::HookExecutor;
    let log = HookExecutor::execute(&hook, &context).await;
    Ok(vec![log])
}

// ── get_hook_logs ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_hook_logs(
    hook_id: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<HookExecutionLog>> {
    HooksDb::open()?.get_logs(hook_id.as_deref(), limit.unwrap_or(100))
}
