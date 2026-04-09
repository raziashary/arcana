use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct EditFileTool;

#[derive(Debug, Deserialize)]
struct EditFileInput {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a specific string with another. \
         Use this for targeted modifications like changing a function, fixing a line, or updating a value. \
         The old_string must match EXACTLY (including whitespace and indentation). \
         Fails if old_string is not found or appears multiple times (unless replace_all=true). \
         For large rewrites, prefer write_file instead."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to find and replace. Must match exactly including whitespace."
                },
                "new_string": {
                    "type": "string",
                    "description": "Text to replace old_string with. Can be empty to delete."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replaces all occurrences. If false (default), fails if there are multiple matches.",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    fn requires_permission(&self) -> bool {
        true
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let params: EditFileInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        let file_path = if PathBuf::from(&params.path).is_absolute() {
            PathBuf::from(&params.path)
        } else {
            PathBuf::from(&ctx.working_dir).join(&params.path)
        };

        debug!("Editing file: {:?}", file_path);

        let content = match fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to read file '{}': {}", file_path.display(), e)),
        };

        let match_count = content.matches(&params.old_string).count();

        if match_count == 0 {
            return ToolResult::error(format!(
                "old_string not found in file. Searched for:\n```\n{}\n```\n\nFile preview (first 500 chars):\n```\n{}\n```",
                params.old_string,
                &content.chars().take(500).collect::<String>()
            ));
        }

        if match_count > 1 && !params.replace_all {
            let mut positions = Vec::new();
            let mut start = 0;
            while let Some(pos) = content[start..].find(&params.old_string) {
                let line_num = content[..start + pos].lines().count() + 1;
                positions.push(line_num);
                start += pos + params.old_string.len();
            }

            return ToolResult::error(format!(
                "old_string found {} times at lines: {:?}. Use replace_all=true to replace all occurrences, or make old_string more specific.",
                match_count, positions
            ));
        }

        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            content.replacen(&params.old_string, &params.new_string, 1)
        };

        let old_lines = content.lines().count();
        let new_lines = new_content.lines().count();

        if let Err(e) = fs::write(&file_path, &new_content).await {
            return ToolResult::error(format!("Failed to write file '{}': {}", file_path.display(), e));
        }

        let occurrences = if params.replace_all { match_count } else { 1 };
        ToolResult::success(format!(
            "✓ Edited {} ({} occurrence{} replaced, {} lines → {} lines)",
            params.path,
            occurrences,
            if occurrences == 1 { "" } else { "s" },
            old_lines,
            new_lines
        ))
    }
}
