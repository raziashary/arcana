use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Returns the file content with line numbers. \
         Use start_line and end_line to read specific portions of large files."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "Absolute or working-directory-relative path to the file."
                },
                "start_line": {
                    "type":        "integer",
                    "description": "Optional: 1-indexed line to start reading from."
                },
                "end_line": {
                    "type":        "integer",
                    "description": "Optional: 1-indexed line to stop reading at (inclusive)."
                }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input["path"].as_str() {
            Some(p) => p.to_string(),
            None    => return ToolResult::error("Missing required field: path"),
        };

        let path = resolve_path(&path_str, &ctx.working_dir);

        // Fire PreFileRead hook
        let hook_ctx = HookContext {
            hook_type: HookType::PreFileRead.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some(ctx.session_id.clone()),
            agent_mode: None,
            tool_call: Some(ToolCallContext {
                id: ctx.call_id.clone(),
                name: "read_file".to_string(),
                input: input.clone(),
                result: None,
                is_error: false,
            }),
            file_path: Some(path.to_string_lossy().to_string()),
            task_id: None,
            worker_id: None,
            extra: serde_json::json!({
                "start_line": input["start_line"].as_u64(),
                "end_line": input["end_line"].as_u64(),
            }),
        };
        let should_continue = HookRegistry::fire(&HookType::PreFileRead, &hook_ctx).await;
        if !should_continue {
            return ToolResult::error("File read blocked by hook");
        }

        // Guard: reject files larger than 256 KB (~64K tokens) when no line range is given.
        // Reading a 5 MB lock file or binary would fill the entire context window.
        // Agents that need specific content from large files must use start_line/end_line.
        let has_range = input["start_line"].as_u64().is_some() || input["end_line"].as_u64().is_some();
        if !has_range {
            if let Ok(meta) = std::fs::metadata(&path) {
                const MAX_BYTES: u64 = 256_000;
                if meta.len() > MAX_BYTES {
                    return ToolResult::error(format!(
                        "File '{}' is {} KB — too large to read in full (limit {} KB). \
                         Use start_line and end_line to read specific sections, \
                         or use grep to search for content within the file.",
                        path.display(),
                        meta.len() / 1024,
                        MAX_BYTES / 1024,
                    ));
                }
            }
        }

        match std::fs::read_to_string(&path) {
            Err(e) => ToolResult::error(format!("Cannot read '{}': {e}", path.display())),
            Ok(content) => {
                let start = input["start_line"].as_u64().map(|n| n as usize);
                let end   = input["end_line"].as_u64().map(|n| n as usize);

                let lines: Vec<&str> = content.lines().collect();
                let from = start.map(|n| n.saturating_sub(1)).unwrap_or(0);
                let to   = end.unwrap_or(lines.len()).min(lines.len());
                
                let selected_lines = &lines[from..to];
                let start_line_num = from + 1;
                
                let output = selected_lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>4} | {}", start_line_num + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");

                ToolResult::success(output)
            }
        }
    }
}

fn resolve_path(path: &str, working_dir: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { Path::new(working_dir).join(path) }
}
