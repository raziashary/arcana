// Tauri commands for Phase 4.9 — Subagent System.
//
// Exposes the SubagentManager to the frontend. Most commands are thin
// wrappers; the heavy lifting (HTTP, SSE parsing, pool enforcement) lives
// in `crate::subagent`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::subagent::{
    SubagentConfig, SubagentHandle, SubagentManager, SubagentResult, SubagentStatus,
    SubagentSummary,
};

/// Frontend-facing input for `spawn_subagent` Tauri command.
///
/// Most fields are optional and resolved server-side. The single required
/// field is `parent_slug` because it determines pool / depth accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnSubagentRequest {
    pub parent_slug: String,
    pub parent_session_id: String,
    pub task_description: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default)]
    pub depth: u32,
}

impl SpawnSubagentRequest {
    fn into_config(self) -> (SubagentConfig, String) {
        let cfg = SubagentConfig {
            parent_slug: self.parent_slug,
            role: "subagent".to_string(),
            // ⚠ G-5. A subagent spawned straight from the UI has a user
            // sitting in front of it — the same situation as the interactive
            // assistant, which runs `ManualApproval` deliberately
            // (`agent_commands.rs`). It is NOT part of the unattended fleet,
            // so it does not inherit the fleet's Autonomous.
            //
            // Deliberately not a field on the request: the mode is not the
            // caller's to choose. Letting the renderer name it would put the
            // policy back in reach of whatever can send this command.
            permission_mode: arcana_core::agent::permission::PermissionMode::ManualApproval,
            parent_session_id: self.parent_session_id,
            system_prompt: self.system_prompt.unwrap_or_default(),
            tools: self.tools,
            model: self.model.unwrap_or_default(),
            working_dir: self.working_dir.unwrap_or_default(),
            max_turns: self.max_turns.unwrap_or(0),
            timeout_secs: self.timeout_secs.unwrap_or(0),
            depth: self.depth,
        };
        (cfg, self.task_description)
    }
}

/// Spawn a subagent and return its handle immediately.
/// The actual SSE consumption runs in the background.
#[tauri::command]
pub async fn spawn_subagent(
    request: SpawnSubagentRequest,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<SubagentHandle, String> {
    let (config, prompt) = request.into_config();
    manager
        .spawn(config, prompt)
        .await
        .map_err(|e| e.to_string())
}

/// Block on a previously-spawned subagent's completion and return its result.
#[tauri::command]
pub async fn execute_subtask(
    subagent_id: String,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<SubagentResult, String> {
    manager
        .await_result(&subagent_id)
        .await
        .map_err(|e| e.to_string())
}

/// Snapshot the current status of a subagent.
#[tauri::command]
pub async fn get_subagent_status(
    subagent_id: String,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<SubagentStatus, String> {
    manager
        .get_status(&subagent_id)
        .await
        .map_err(|e| e.to_string())
}

/// Graceful termination + cleanup.
#[tauri::command]
pub async fn terminate_subagent(
    subagent_id: String,
    force: bool,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<(), String> {
    manager
        .terminate(&subagent_id, force)
        .await
        .map_err(|e| e.to_string())
}

/// Hard abort — alias for terminate(force=true).
#[tauri::command]
pub async fn abort_subagent(
    subagent_id: String,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<(), String> {
    manager.abort(&subagent_id).await.map_err(|e| e.to_string())
}

/// List active subagents, optionally filtered by parent session.
#[tauri::command]
pub async fn list_active_subagents(
    parent_session_id: Option<String>,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<Vec<SubagentSummary>, String> {
    Ok(manager.list(parent_session_id.as_deref()).await)
}

/// Doc 04 Phase 3.3 — live snapshot of QC reviewer concurrency.
///
/// Returns a tuple `(in_flight, max_concurrent)` so the Subagent Monitor /
/// Arcana Live Monitor can render "QC: 3 / 6 in flight" without re-walking
/// the registry on the frontend. `in_flight` is computed from the live
/// subagent list filtered to `parent_slug == "qc-dispatcher"`;
/// `max_concurrent` is the pool's `MAX_CONCURRENT_QC` constant.
#[tauri::command]
pub async fn get_qc_reviewer_stats(
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<serde_json::Value, String> {
    let summaries = manager.list(None).await;
    let in_flight = summaries
        .iter()
        .filter(|s| s.parent_slug == crate::subagent::pool::QC_DISPATCHER_SLUG)
        .filter(|s| !s.status.is_terminal())
        .count();
    let max = crate::subagent::pool::MAX_CONCURRENT_QC;
    Ok(serde_json::json!({
        "in_flight":     in_flight,
        "max_concurrent": max,
    }))
}
