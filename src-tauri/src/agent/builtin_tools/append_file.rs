// append_file.rs — Appends content to a file without reading/rewriting the whole file.
// Prevents accidental overwrites during requirements.txt, package.json, or log updates.
// Available in all non-delegate PA modes and to MCP workers.

use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct AppendFileTool;

#[async_trait]
impl Tool for AppendFileTool {
    fn name(&self) -> &str { "append_file" }

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

    fn is_read_only(&self) -> bool { false }

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
        let resolved = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::error(format!(
                    "Cannot create parent directory for '{}': {e}",
                    resolved.display()
                ));
            }
        }

        // Determine whether to inject a leading newline
        let prefix = if newline && resolved.exists() {
            // Read only the last byte to check if file ends with newline
            match std::fs::metadata(&resolved) {
                Ok(meta) if meta.len() > 0 => {
                    // Read last byte
                    let last_byte = read_last_byte(&resolved);
                    if last_byte != b'\n' { "\n" } else { "" }
                }
                _ => "",
            }
        } else {
            ""
        };

        let to_write = format!("{}{}", prefix, content);
        let bytes_to_append = to_write.len();

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&resolved)
            .await
        {
            Ok(f) => f,
            Err(e) => return ToolResult::error(format!(
                "Cannot open '{}' for append: {e}",
                resolved.display()
            )),
        };

        if let Err(e) = file.write_all(to_write.as_bytes()).await {
            return ToolResult::error(format!(
                "Write failed for '{}': {e}",
                resolved.display()
            ));
        }

        ToolResult::success(format!(
            "Appended {} bytes to '{}'",
            bytes_to_append,
            resolved.display()
        ))
    }
}

// Read only the final byte of a file to check for trailing newline.
fn read_last_byte(path: &Path) -> u8 {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = std::fs::File::open(path) else { return b'\n' };
    let Ok(meta) = f.metadata() else { return b'\n' };
    if meta.len() == 0 { return b'\n'; }
    if f.seek(SeekFrom::End(-1)).is_err() { return b'\n'; }
    let mut buf = [0u8; 1];
    if f.read_exact(&mut buf).is_err() { return b'\n'; }
    buf[0]
}
