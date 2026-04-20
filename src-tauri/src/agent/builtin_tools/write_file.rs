use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }

    fn description(&self) -> &str {
        "Write content to a file at the given path. Creates the file (and any parent directories) if it doesn't exist. Overwrites existing content."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "Absolute or working-directory-relative path to write."
                },
                "content": {
                    "type":        "string",
                    "description": "Full file content to write."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn is_read_only(&self) -> bool { false }
    fn requires_permission(&self) -> bool { true }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None    => return ToolResult::error("Missing required field: path"),
        };
        let content = match input["content"].as_str() {
            Some(c) => c,
            None    => return ToolResult::error("Missing required field: content"),
        };

        let path = resolve_path(path_str, &ctx.working_dir);

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!("Cannot create directories for '{}': {e}", path.display()));
            }
        }

        // Phase 3.3 — auto-checkpoint before overwrite so the edit is undoable.
        crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            &path.to_string_lossy(),
        );

        match std::fs::write(&path, content) {
            Ok(_)  => {
                // Fire PostFileWrite hook after successful write
                let hook_ctx = HookContext {
                    hook_type: HookType::PostFileWrite.as_str().to_string(),
                    timestamp: chrono::Utc::now(),
                    session_id: Some(ctx.session_id.clone()),
                    agent_mode: None,
                    tool_call: Some(ToolCallContext {
                        id: ctx.call_id.clone(),
                        name: "write_file".to_string(),
                        input: input.clone(),
                        result: Some(serde_json::json!({"bytes_written": content.len()})),
                        is_error: false,
                    }),
                    file_path: Some(path.to_string_lossy().to_string()),
                    task_id: None,
                    worker_id: None,
                    extra: serde_json::json!({
                        "bytes_written": content.len(),
                    }),
                };
                tokio::spawn(async move {
                    HookRegistry::fire(&HookType::PostFileWrite, &hook_ctx).await;
                });
                ToolResult::success(format!("Wrote {} bytes to '{}'", content.len(), path.display()))
            }
            Err(e) => ToolResult::error(format!("Cannot write '{}': {e}", path.display())),
        }
    }
}

fn resolve_path(path: &str, working_dir: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { Path::new(working_dir).join(path) }
}
