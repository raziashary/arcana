use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tracing::debug;
use walkdir::WalkDir;

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

        // Compile regex pattern
        let regex = match regex::Regex::new(&params.pattern) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Invalid regex pattern: {e}")),
        };

        let mut matches = Vec::new();
        let skip_dirs = ["node_modules", ".git", "target", "dist", "build", ".next", ".cache"];

        // Walk directory tree
        for entry in WalkDir::new(&search_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                if e.file_type().is_dir() {
                    !skip_dirs.contains(&e.file_name().to_string_lossy().as_ref())
                } else {
                    true
                }
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            if !entry.file_type().is_file() {
                continue;
            }

            // Check glob filter if provided
            if let Some(glob_pattern) = &params.glob {
                let file_name = entry.file_name().to_string_lossy();
                if !glob::Pattern::new(glob_pattern)
                    .ok()
                    .map(|p| p.matches(&file_name))
                    .unwrap_or(false)
                {
                    continue;
                }
            }

            // Read file and search for pattern
            let content = match fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let relative_path = entry.path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            for (line_num, line) in content.lines().enumerate() {
                let line_number = line_num + 1;
                let matches_pattern = if params.case_insensitive {
                    regex.is_match(&line.to_lowercase())
                } else {
                    regex.is_match(line)
                };

                if matches_pattern {
                    // Collect context lines
                    let lines: Vec<&str> = content.lines().collect();
                    let context_start = line_num.saturating_sub(params.context_lines as usize);
                    let context_end = (line_num + params.context_lines as usize + 1).min(lines.len());

                    let mut context_block = String::new();
                    for (i, ctx_line) in lines[context_start..context_end].iter().enumerate() {
                        let actual_line_num = context_start + i + 1;
                        let prefix = if actual_line_num == line_number { ">" } else { " " };
                        context_block.push_str(&format!("{} {:>5} | {}\n", prefix, actual_line_num, ctx_line));
                    }

                    matches.push(format!(
                        "{}:{}\n{}",
                        relative_path, line_number, context_block
                    ));

                    if matches.len() >= params.max_results as usize {
                        break;
                    }
                }
            }

            if matches.len() >= params.max_results as usize {
                break;
            }
        }

        if matches.is_empty() {
            ToolResult::success(format!("No matches found for pattern '{}'", params.pattern))
        } else {
            let truncated = if matches.len() >= params.max_results as usize {
                format!("\n\n... truncated ({} total matches, showing first {})", matches.len(), params.max_results)
            } else {
                String::new()
            };
            ToolResult::success(format!(
                "Found {} match{}:\n\n{}{}",
                matches.len(),
                if matches.len() == 1 { "" } else { "es" },
                matches.join("\n"),
                truncated
            ))
        }
    }
}
