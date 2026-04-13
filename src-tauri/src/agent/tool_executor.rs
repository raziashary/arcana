// Tool executor — handles permission checks + concurrent/serial execution.
// Read-only tools run concurrently (tokio tasks).
// Write tools run serially in ManualApproval mode with user confirmation.
// Pattern from Claude Code: read-only concurrent, writes serial.

use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::oneshot;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::errors::Result;
use crate::events::{
    emit_permission_request, emit_tool_end, emit_tool_start,
    PermissionPayload, ToolEndPayload, ToolStartPayload,
};
use super::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use super::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ToolExecutor {
    mode:        PermissionMode,
    pending:     PendingPermissions,
    app:         AppHandle,
    session_id:  String,
    working_dir: String,
    // Tools auto-approved for this session (AllowAlways responses)
    always_allow: std::collections::HashSet<String>,
}

impl ToolExecutor {
    pub fn new(
        mode:        PermissionMode,
        pending:     PendingPermissions,
        app:         AppHandle,
        session_id:  String,
        working_dir: String,
    ) -> Self {
        Self {
            mode,
            pending,
            app,
            session_id,
            working_dir,
            always_allow: std::collections::HashSet::new(),
        }
    }

    /// Execute a tool call with permission check.
    /// Returns the tool result payload.
    pub async fn execute(
        &mut self,
        tool: Arc<dyn Tool>,
        input: serde_json::Value,
    ) -> Result<ToolResult> {
        let call_id = Uuid::new_v4().to_string();
        let ctx = ToolContext {
            session_id:  self.session_id.clone(),
            working_dir: self.working_dir.clone(),
            call_id:     call_id.clone(),
        };

        // Emit tool start event for UI
        emit_tool_start(&self.app, ToolStartPayload {
            session_id: self.session_id.clone(),
            call_id:    call_id.clone(),
            tool_name:  tool.name().to_string(),
            input:      input.clone(),
        });

        // Permission check
        let approved = match self.mode {
            PermissionMode::Autonomous => true,
            PermissionMode::ManualApproval => {
                if tool.is_read_only() || self.always_allow.contains(tool.name()) {
                    true
                } else {
                    self.request_permission(&call_id, &tool, &input).await?
                }
            }
            PermissionMode::SmartApprove => {
                // Phase 2: smart approval logic
                // For now, auto-approve read-only, prompt otherwise
                tool.is_read_only() || self.always_allow.contains(tool.name())
                    || self.request_permission(&call_id, &tool, &input).await?
            }
        };

        let result = if approved {
            debug!("Executing tool: {}", tool.name());
            tool.execute(input, &ctx).await
        } else {
            warn!("Tool '{}' denied by user", tool.name());
            ToolResult::error(format!("Tool '{}' was denied by user", tool.name()))
        };

        // Emit tool end event for UI
        emit_tool_end(&self.app, ToolEndPayload {
            session_id: self.session_id.clone(),
            call_id:    call_id.clone(),
            tool_name:  tool.name().to_string(),
            output:     result.output.clone(),
            is_error:   result.is_error,
        });

        Ok(result)
    }

    async fn request_permission(
        &mut self,
        call_id:  &str,
        tool:     &Arc<dyn Tool>,
        input:    &serde_json::Value,
    ) -> Result<bool> {
        let (tx, rx) = oneshot::channel::<PermissionResponse>();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(call_id.to_string(), tx);
        }

        emit_permission_request(&self.app, PermissionPayload {
            session_id:  self.session_id.clone(),
            call_id:     call_id.to_string(),
            tool_name:   tool.name().to_string(),
            input:       input.clone(),
            description: tool.description().to_string(),
        });

        // Wait for the user to respond (frontend sends respond_to_permission)
        match rx.await {
            Ok(PermissionResponse::Allow) => Ok(true),
            Ok(PermissionResponse::AllowAlways) => {
                // Add this tool to the always-allow set for this session
                self.always_allow.insert(tool.name().to_string());
                debug!("Tool '{}' added to always-allow for session {}", tool.name(), self.session_id);
                Ok(true)
            }
            Ok(PermissionResponse::Deny) | Err(_) => Ok(false),
        }
    }
}
