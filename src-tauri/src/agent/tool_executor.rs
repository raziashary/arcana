// Tool executor — handles permission checks + concurrent/serial execution.
// Read-only tools run concurrently (tokio tasks, max 10).
// Write tools run serially with user confirmation.
// Pattern from Claude Code: read-only concurrent, writes serial.

use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{oneshot, Semaphore};
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::errors::Result;
use crate::events::{
    emit_permission_request, emit_tool_end, emit_tool_start,
    PermissionPayload, ToolEndPayload, ToolStartPayload,
};
use super::permission::{PendingPermissions, PermissionMode, PermissionResponse};
use super::tool_trait::{Tool, ToolContext, ToolResult};

// Max concurrent read-only tools (Claude Code uses 10)
const MAX_CONCURRENT_READ_TOOLS: usize = 10;

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

    /// Execute a batch of tools with partitioned concurrency.
    /// Read-only tools run concurrently (max 10), write tools run serially.
    /// Returns results in the same order as input.
    /// Pattern from Claude Code's partitionToolCalls() + runTools().
    pub async fn execute_batch(
        &mut self,
        tools: Vec<(Arc<dyn Tool>, serde_json::Value, String)>, // (tool, input, tool_use_id)
    ) -> Result<Vec<(String, ToolResult)>> { // (tool_use_id, result)
        if tools.is_empty() {
            return Ok(Vec::new());
        }

        // Partition: read-only vs write tools
        let mut read_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, usize)> = Vec::new();
        let mut write_tools: Vec<(Arc<dyn Tool>, serde_json::Value, String, usize)> = Vec::new();

        for (idx, (tool, input, tool_use_id)) in tools.into_iter().enumerate() {
            if tool.is_read_only() {
                read_tools.push((tool, input, tool_use_id, idx));
            } else {
                write_tools.push((tool, input, tool_use_id, idx));
            }
        }

        let mut results: Vec<(String, ToolResult)> = Vec::with_capacity(read_tools.len() + write_tools.len());
        results.resize(read_tools.len() + write_tools.len(), (String::new(), ToolResult::error("placeholder")));

        // Execute read-only tools concurrently with semaphore
        if !read_tools.is_empty() {
            debug!(
                "Executing {} read-only tools concurrently (max {})",
                read_tools.len(),
                MAX_CONCURRENT_READ_TOOLS
            );

            let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_READ_TOOLS));
            let mut read_handles: Vec<JoinHandle<(usize, String, ToolResult)>> = Vec::new();

            for (tool, input, tool_use_id, original_idx) in read_tools {
                let sem_clone = Arc::clone(&semaphore);
                let app = self.app.clone();
                let session_id = self.session_id.clone();
                let working_dir = self.working_dir.clone();
                let mode = self.mode.clone();
                let always_allow = self.always_allow.clone();
                let pending = self.pending.clone();

                let handle: JoinHandle<(usize, String, ToolResult)> = tokio::spawn(async move {
                    let _permit = sem_clone.acquire().await.unwrap();

                    let call_id = Uuid::new_v4().to_string();
                    let ctx = ToolContext {
                        session_id: session_id.clone(),
                        working_dir: working_dir.clone(),
                        call_id: call_id.clone(),
                    };

                    // Emit start event
                    emit_tool_start(&app, ToolStartPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        input: input.clone(),
                    });

                    // Execute (read-only always approved)
                    let result = tool.execute(input, &ctx).await;

                    // Emit end event
                    emit_tool_end(&app, ToolEndPayload {
                        session_id: session_id.clone(),
                        call_id: call_id.clone(),
                        tool_name: tool.name().to_string(),
                        output: result.output.clone(),
                        is_error: result.is_error,
                    });

                    (original_idx, tool_use_id, result)
                });

                read_handles.push(handle);
            }

            // Collect concurrent results
            for handle in read_handles {
                match handle.await {
                    Ok((idx, tool_use_id, result)) => {
                        results[idx] = (tool_use_id, result);
                    }
                    Err(e) => {
                        error!("Read-only tool task panicked: {}", e);
                        // Placeholder will be replaced; we need to find which one failed
                    }
                }
            }
        }

        // Execute write tools serially (preserve order, respect permissions)
        if !write_tools.is_empty() {
            debug!("Executing {} write tools serially", write_tools.len());

            for (tool, input, tool_use_id, original_idx) in write_tools {
                let call_id = Uuid::new_v4().to_string();
                let ctx = ToolContext {
                    session_id: self.session_id.clone(),
                    working_dir: self.working_dir.clone(),
                    call_id: call_id.clone(),
                };

                // Emit start event
                emit_tool_start(&self.app, ToolStartPayload {
                    session_id: self.session_id.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    input: input.clone(),
                });

                // Permission check (may prompt user)
                let approved = match self.mode {
                    PermissionMode::Autonomous => true,
                    PermissionMode::ManualApproval => {
                        if self.always_allow.contains(tool.name()) {
                            true
                        } else {
                            self.request_permission(&call_id, &tool, &input).await?
                        }
                    }
                    PermissionMode::SmartApprove => {
                        self.always_allow.contains(tool.name())
                            || self.request_permission(&call_id, &tool, &input).await?
                    }
                };

                let result = if approved {
                    debug!("Executing write tool: {}", tool.name());
                    tool.execute(input, &ctx).await
                } else {
                    warn!("Write tool '{}' denied by user", tool.name());
                    ToolResult::error(format!("Tool '{}' was denied by user", tool.name()))
                };

                // Emit end event
                emit_tool_end(&self.app, ToolEndPayload {
                    session_id: self.session_id.clone(),
                    call_id: call_id.clone(),
                    tool_name: tool.name().to_string(),
                    output: result.output.clone(),
                    is_error: result.is_error,
                });

                results[original_idx] = (tool_use_id, result);
            }
        }

        Ok(results)
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
