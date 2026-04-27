// Tauri commands for Phase 4.9 — Subagent System.
//
// Exposes the SubagentManager to the frontend. Most commands are thin
// wrappers; the heavy lifting (HTTP, SSE parsing, pool enforcement) lives
// in `crate::subagent`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::subagent::{
    SubagentConfig, SubagentHandle, SubagentManager, SubagentResult,
    SubagentStatus, SubagentSummary,
};

/// Frontend-facing input for `spawn_subagent` Tauri command.
///
/// Most fields are optional and resolved server-side. The single required
/// field is `parent_slug` because it determines pool / depth accounting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnSubagentRequest {
    pub parent_slug:       String,
    pub parent_session_id: String,
    pub task_description:  String,
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
    pub mcp_servers: Vec<String>,
    #[serde(default)]
    pub depth: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

impl SpawnSubagentRequest {
    fn into_config(self) -> (SubagentConfig, String) {
        let cfg = SubagentConfig {
            parent_slug:       self.parent_slug,
            parent_session_id: self.parent_session_id,
            system_prompt:     self.system_prompt.unwrap_or_default(),
            tools:             self.tools,
            model:             self.model.unwrap_or_default(),
            working_dir:       self.working_dir.unwrap_or_default(),
            max_turns:         self.max_turns.unwrap_or(0),
            timeout_secs:      self.timeout_secs.unwrap_or(0),
            mcp_servers:       self.mcp_servers,
            capabilities:      self.capabilities,
            depth:             self.depth,
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
    manager.spawn(config, prompt).await.map_err(|e| e.to_string())
}

/// Block on a previously-spawned subagent's completion and return its result.
#[tauri::command]
pub async fn execute_subtask(
    subagent_id: String,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<SubagentResult, String> {
    manager.await_result(&subagent_id).await.map_err(|e| e.to_string())
}

/// Snapshot the current status of a subagent.
#[tauri::command]
pub async fn get_subagent_status(
    subagent_id: String,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<SubagentStatus, String> {
    manager.get_status(&subagent_id).await.map_err(|e| e.to_string())
}

/// Graceful termination + cleanup.
#[tauri::command]
pub async fn terminate_subagent(
    subagent_id: String,
    force: bool,
    manager: State<'_, Arc<SubagentManager>>,
) -> Result<(), String> {
    manager.terminate(&subagent_id, force).await.map_err(|e| e.to_string())
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
