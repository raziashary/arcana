use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};
use super::path_jail::validate_workspace_path;
use super::validate;

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
        // Clone before moving into from_value so the hook context below can reference it.
        let input_for_hook = input.clone();
        let params: EditFileInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        let resolved = if PathBuf::from(&params.path).is_absolute() {
            PathBuf::from(&params.path)
        } else {
            PathBuf::from(&ctx.working_dir).join(&params.path)
        };

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        // Same rationale as write_file: refuse edits whose canonical
        // destination escapes the configured workspace.
        let file_path = match validate_workspace_path(&resolved, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg)      => return ToolResult::error(msg),
        };

        debug!("Editing file: {:?}", file_path);

        let content = match fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                // If the file doesn't exist, point the agent at `write_file` instead.
                // Workers bootstrapping a project commonly try edit_file first; failing
                // with a bare I/O error encourages a retry loop on a doomed path.
                if e.kind() == std::io::ErrorKind::NotFound {
                    return ToolResult::error(format!(
                        "File '{}' does not exist. To create a new file, call \
                         `write_file(path='{}', content=...)` instead — edit_file only \
                         modifies existing files.",
                        file_path.display(), params.path
                    ));
                }
                return ToolResult::error(format!(
                    "Failed to read file '{}': {}", file_path.display(), e
                ));
            }
        };

        let match_count = content.matches(&params.old_string).count();

        if match_count == 0 {
            // Do NOT include a file preview here — it re-injects up to 500 chars of file
            // content into the agent context on every failed edit attempt, compounding
            // token cost across retries. The agent already has the file if it read it.
            // Instead, give the line count so the agent can target read_file(start/end).
            let line_count = content.lines().count();
            return ToolResult::error(format!(
                "old_string not found in '{}' ({line_count} lines total). \
                 The text must match exactly including whitespace and indentation. \
                 Use read_file(path, start_line=X, end_line=Y) to inspect the target area \
                 before retrying.",
                params.path
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

        // Phase 3.3 — snapshot current contents before overwriting.
        crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            &file_path.to_string_lossy(),
        );

        if let Err(e) = fs::write(&file_path, &new_content).await {
            return ToolResult::error(format!("Failed to write file '{}': {}", file_path.display(), e));
        }

        // K.3 — Fire PostFileWrite hook (same as write_file.rs) so the graph
        // engine, telemetry, and hook consumers observe agent edits.
        let file_path_str = file_path.to_string_lossy().to_string();
        let hook_ctx = HookContext {
            hook_type:  HookType::PostFileWrite.as_str().to_string(),
            timestamp:  chrono::Utc::now(),
            session_id: Some(ctx.session_id.clone()),
            agent_mode: None,
            tool_call: Some(ToolCallContext {
                id:       ctx.call_id.clone(),
                name:     "edit_file".to_string(),
                input:    input_for_hook,
                result:   Some(json!({"bytes_written": new_content.len()})),
                is_error: false,
            }),
            file_path:  Some(file_path_str),
            task_id:    None,
            worker_id:  None,
            extra: json!({
                "bytes_written": new_content.len(),
                "occurrences_replaced": if params.replace_all { match_count } else { 1 },
            }),
        };
        tokio::spawn(async move {
            HookRegistry::fire(&HookType::PostFileWrite, &hook_ctx).await;
        });

        let occurrences = if params.replace_all { match_count } else { 1 };

        // Option A — Tier 1 syntax validation (always-on, fast).
        // Appended to the success response so the agent knows immediately if
        // its edit introduced a syntax error, without needing a check_file call.
        let lint_note = match validate::tier1_syntax(&file_path, &ctx.working_dir).await {
            Some(result) if !result.diagnostics.is_empty() => {
                format!("\n\n⚠ Lint feedback:\n{}", result.format_inline())
            }
            _ => String::new(),
        };

        ToolResult::success(format!(
            "✓ Edited {} ({} occurrence{} replaced, {} lines → {} lines){lint_note}",
            params.path,
            occurrences,
            if occurrences == 1 { "" } else { "s" },
            old_lines,
            new_lines
        ))
    }
}
