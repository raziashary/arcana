// append_file.rs — Appends content to a file without reading/rewriting the whole file.
// Prevents accidental overwrites during requirements.txt, package.json, or log updates.
// Available in all non-delegate PA modes and to MCP workers.

use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};

use super::path_jail::validate_workspace_path;
use super::path_lock;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct AppendFileTool;

#[async_trait]
impl Tool for AppendFileTool {
    fn name(&self) -> &str {
        "append_file"
    }

    fn description(&self) -> &str {
        "Append content to the end of a file. Creates the file if it does not exist. \
         Safer than read+write for adding lines to requirements.txt, package.json, logs, etc. \
         Use newline=true (default) to ensure content starts on its own line."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "File path to append to. Absolute or relative to working directory."
                },
                "content": {
                    "type":        "string",
                    "description": "Content to append to the file."
                },
                "newline": {
                    "type":        "boolean",
                    "description": "If true (default), prepend a newline before content when the file already exists and does not end with a newline."
                }
            },
            "required": ["path", "content"]
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input["path"].as_str() {
            Some(p) if !p.trim().is_empty() => p,
            _ => return ToolResult::error("Missing required parameter 'path'."),
        };

        let content = match input["content"].as_str() {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter 'content'."),
        };

        let newline = input["newline"].as_bool().unwrap_or(true);

        // Resolve path against working directory
        let raw = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        // Same rationale as write_file: refuse appends whose canonical
        // destination escapes the configured workspace.
        let resolved = match validate_workspace_path(&raw, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg) => return ToolResult::error(msg),
        };

        // Cross-tool, cross-worker per-path lock. Same key/registry used by
        // write_file and edit_file — a concurrent overwrite or edit cannot
        // truncate the file mid-append, and a concurrent append from another
        // worker is strictly ordered after ours rather than racing the
        // trailing-newline probe.
        let _lock_guard = path_lock::acquire_for(&resolved, &ctx.working_dir).await;

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::error(format!(
                    "Cannot create parent directory for '{}': {e}",
                    resolved.display()
                ));
            }
        }

        // Open once in read + append mode. Doing the trailing-newline probe
        // through the same file handle narrows the previous probe-then-open
        // race window (where another writer could extend the file between the
        // standalone `metadata` call and our `OpenOptions::open`). The window
        // is not fully closed — another concurrent appender can still extend
        // the file between `seek/read` and our `write_all` — but the worst
        // observable outcome is a duplicated or missing newline, which is
        // cosmetic. Documented limitation.
        let mut file = match OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(&resolved)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                return ToolResult::error(format!(
                    "Cannot open '{}' for append: {e}",
                    resolved.display()
                ))
            }
        };

        // Probe last byte through the open handle.
        let needs_leading_nl = if newline {
            match file.metadata().await {
                Ok(meta) if meta.len() > 0 => {
                    let mut buf = [0u8; 1];
                    let seek_ok = file.seek(SeekFrom::End(-1)).await.is_ok();
                    let read_ok = seek_ok && file.read_exact(&mut buf).await.is_ok();
                    read_ok && buf[0] != b'\n'
                }
                _ => false,
            }
        } else {
            false
        };

        // Append mode auto-seeks to end on each write, so the read seek above
        // does not affect the write position.
        let to_write = if needs_leading_nl {
            format!("\n{}", content)
        } else {
            content.to_string()
        };
        let bytes_to_append = to_write.len();

        if let Err(e) = file.write_all(to_write.as_bytes()).await {
            return ToolResult::error(format!("Write failed for '{}': {e}", resolved.display()));
        }

        ToolResult::success(format!(
            "Appended {} bytes to '{}'",
            bytes_to_append,
            resolved.display()
        ))
    }
}
