use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }

    fn description(&self) -> &str {
        "List files and subdirectories at a path. Returns a structured text listing with file sizes and types."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "Directory path to list. Defaults to working directory."
                },
                "recursive": {
                    "type":        "boolean",
                    "description": "If true, list recursively (max depth 3). Default: false."
                }
            },
            "required": []
        })
    }

    fn is_read_only(&self) -> bool { true }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = input["path"].as_str().unwrap_or(&ctx.working_dir);
        let recursive = input["recursive"].as_bool().unwrap_or(false);

        let base = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };

        if !base.exists() {
            return ToolResult::error(format!("Path does not exist: {}", base.display()));
        }
        if !base.is_dir() {
            return ToolResult::error(format!("Not a directory: {}", base.display()));
        }

        let mut lines = vec![format!("{}/ (directory)", base.display())];
        list_dir_recursive(&base, &base, 0, if recursive { 3 } else { 1 }, &mut lines);

        if lines.len() == 1 {
            lines.push("  (empty)".to_string());
        }

        ToolResult::success(lines.join("\n"))
    }
}

fn list_dir_recursive(
    root:      &Path,
    path:      &Path,
    depth:     usize,
    max_depth: usize,
    out:       &mut Vec<String>,
) {
    if depth >= max_depth { return; }

    let Ok(entries) = std::fs::read_dir(path) else { return };
    let indent = "  ".repeat(depth + 1);

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        (!is_dir, e.file_name())
    });

    // Skip common noise directories
    const SKIP: &[&str] = &[
        ".git", "node_modules", "target", "__pycache__", ".venv", "venv",
        "dist", "build", ".airs",
    ];

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if SKIP.iter().any(|s| *s == name_str) { continue; }

        let entry_path = entry.path();
        if entry_path.is_dir() {
            out.push(format!("{indent}{name_str}/"));
            list_dir_recursive(root, &entry_path, depth + 1, max_depth, out);
        } else {
            let size = entry.metadata()
                .map(|m| format_size(m.len()))
                .unwrap_or_default();
            out.push(format!("{indent}{name_str}  {size}"));
        }
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1_024 {
        format!("({bytes}B)")
    } else if bytes < 1_024 * 1_024 {
        format!("({:.1}KB)", bytes as f64 / 1_024.0)
    } else {
        format!("({:.1}MB)", bytes as f64 / 1_024.0 / 1_024.0)
    }
}
