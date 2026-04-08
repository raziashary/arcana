// Permission system for tool execution.
// Three modes matching Claude Code's PermissionMode and Goose's GooseMode:
//   ManualApproval — user approves every non-read-only tool call
//   Autonomous     — all tools run without approval (worker agents)
//   SmartApprove   — auto-approve safe patterns, prompt on unknowns (Phase 2)

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    ManualApproval,
    Autonomous,
    SmartApprove,
}

impl Default for PermissionMode {
    fn default() -> Self { Self::ManualApproval }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub call_id:     String,
    pub tool_name:   String,
    pub input:       serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionResponse {
    Allow,
    Deny,
    AllowAlways,  // Auto-approve this tool for the rest of session
}

/// Pending permission requests awaiting user response.
/// Key = call_id, Value = oneshot sender to unblock the agent loop.
pub type PendingPermissions = Arc<Mutex<HashMap<String, oneshot::Sender<PermissionResponse>>>>;

pub fn new_pending_permissions() -> PendingPermissions {
    Arc::new(Mutex::new(HashMap::new()))
}
