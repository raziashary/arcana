use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

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
