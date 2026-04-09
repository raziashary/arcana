use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct GrepTool;

#[derive(Debug, Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    case_insensitive: bool,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default = "default_context_lines")]
    context_lines: u32,
    #[serde(default = "default_max_results")]
    max_results: u32,
}

fn default_context_lines() -> u32 { 2 }
fn default_max_results() -> u32 { 100 }

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files using ripgrep. \
         Returns matching lines with surrounding context. \
         Searches recursively from the specified path (or current directory). \
         Automatically skips binary files, .git, node_modules, etc."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Pattern to search for (supports regex)"
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file to search in. Defaults to current working directory."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "If true, search is case-insensitive. Default: false",
                    "default": false
                },
                "glob": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., '*.rs', '**/*.py')"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to show before and after each match. Default: 2",
                    "default": 2
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return. Default: 100",
                    "default": 100
                }
            },
            "required": ["pattern"]
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn requires_permission(&self) -> bool {
        false
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let params: GrepInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        let search_path = if let Some(path) = &params.path {
            let p = PathBuf::from(path);
            if p.is_absolute() { p } else { PathBuf::from(&ctx.working_dir).join(path) }
        } else {
            PathBuf::from(&ctx.working_dir)
        };

        debug!("Grepping pattern '{}' in {:?}", params.pattern, search_path);

        let mut cmd = Command::new("rg");
        cmd.arg(&params.pattern);
        cmd.arg(&search_path);
        
        cmd.arg("--context").arg(params.context_lines.to_string());
        cmd.arg("--max-count").arg(params.max_results.to_string());
        cmd.arg("--line-number");
        cmd.arg("--color").arg("never");
        cmd.arg("--no-heading");
        cmd.arg("--with-filename");

        if params.case_insensitive {
            cmd.arg("--ignore-case");
        }

        if let Some(glob) = &params.glob {
            cmd.arg("--glob").arg(glob);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = match cmd.output().await {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Failed to execute ripgrep. Is 'rg' installed? Error: {e}")),
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No matches found") || output.status.code() == Some(1) {
                return ToolResult::success(format!("No matches found for pattern '{}'", params.pattern));
            }
            return ToolResult::error(format!("ripgrep error: {stderr}"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = if stdout.is_empty() {
            format!("No matches found for pattern '{}'", params.pattern)
        } else {
            let lines: Vec<&str> = stdout.lines().collect();
            let truncated = if lines.len() > params.max_results as usize {
                format!("\n\n... truncated ({} total matches, showing first {})", lines.len(), params.max_results)
            } else {
                String::new()
            };
            format!("Found {} matches:\n\n{}{}", lines.len(), stdout, truncated)
        };

        ToolResult::success(result)
    }
}
