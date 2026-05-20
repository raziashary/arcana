use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookRegistry, HookType, HookContext, ToolCallContext};

pub struct ReadFileTool;

/// Soft cap: files with more than this many lines REQUIRE an explicit
/// start_line / end_line range. The error message instructs the agent to
/// narrow the read.
const READ_FILE_REQUIRE_RANGE_OVER_LINES: usize = 800;

/// Hard cap: a single read_file call returns at most this many lines, even
/// when the agent explicitly asked for a wider range. Larger reads must be
/// performed as multiple paged calls.
const READ_FILE_MAX_LINES_PER_CALL: usize = 2000;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path. Returns the file content with \
         line numbers. \
         \
         REQUIRED for large files: pass start_line and end_line (1-indexed, inclusive) \
         to read a specific section. Files longer than 800 lines refuse a no-range \
         read; the error tells you the total line count so you can page through \
         in 2,000-line windows. Even with an explicit range, a single call returns \
         at most 2,000 lines — wider spans must be split across multiple calls."
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
                    "minimum":     1,
                    "description": "1-indexed line to start reading from. REQUIRED for files \
                                    longer than 800 lines."
                },
                "end_line": {
                    "type":        "integer",
                    "minimum":     1,
                    "description": "1-indexed line to stop reading at (inclusive). Combined \
                                    with start_line, the call returns at most 2,000 lines."
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
                let total_lines = lines.len();

                // Soft cap: refuse a no-range read of a long file. The
                // agent learns the total line count from the error and
                // can page through in 2000-line windows.
                if !has_range && total_lines > READ_FILE_REQUIRE_RANGE_OVER_LINES {
                    return ToolResult::error(format!(
                        "File '{}' has {} lines, which exceeds the {}-line no-range read \
                         limit. Re-read with start_line and end_line to page through it \
                         (at most {} lines per call). For example:\n  \
                         read_file(path='{}', start_line=1, end_line={})",
                        path.display(),
                        total_lines,
                        READ_FILE_REQUIRE_RANGE_OVER_LINES,
                        READ_FILE_MAX_LINES_PER_CALL,
                        path_str,
                        READ_FILE_MAX_LINES_PER_CALL.min(total_lines),
                    ));
                }

                let from = start.map(|n| n.saturating_sub(1)).unwrap_or(0);
                let requested_to = end.unwrap_or(total_lines).min(total_lines);

                // Hard cap: even with an explicit range, never emit more
                // than READ_FILE_MAX_LINES_PER_CALL lines in one response.
                // Wider spans must be paged across multiple calls.
                let to = requested_to.min(from + READ_FILE_MAX_LINES_PER_CALL);
                let truncated_by_hard_cap = to < requested_to;

                if from >= total_lines {
                    return ToolResult::error(format!(
                        "start_line={} is past end of file '{}' (file has {} lines).",
                        from + 1, path.display(), total_lines,
                    ));
                }
                if to <= from {
                    return ToolResult::error(format!(
                        "Empty line range: start_line={}, end_line={}. \
                         end_line must be ≥ start_line.",
                        from + 1, requested_to,
                    ));
                }

                let selected_lines = &lines[from..to];
                let start_line_num = from + 1;

                let mut output = selected_lines
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>4} | {}", start_line_num + i, line))
                    .collect::<Vec<_>>()
                    .join("\n");

                if truncated_by_hard_cap {
                    output.push_str(&format!(
                        "\n\n[⚠ Output truncated to {max} lines — file has {total} lines, \
                         range requested ended at line {req}. Continue with \
                         read_file(path='{path}', start_line={next}, end_line={next_end}).]",
                        max      = READ_FILE_MAX_LINES_PER_CALL,
                        total    = total_lines,
                        req      = requested_to,
                        path     = path_str,
                        next     = to + 1,
                        next_end = (to + READ_FILE_MAX_LINES_PER_CALL).min(total_lines),
                    ));
                }

                ToolResult::success(output)
            }
        }
    }
}

fn resolve_path(path: &str, working_dir: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() { p.to_path_buf() } else { Path::new(working_dir).join(path) }
}
