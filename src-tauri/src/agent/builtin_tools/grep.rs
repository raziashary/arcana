use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tracing::debug;
use walkdir::WalkDir;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

#[derive(Debug, Serialize)]
struct GrepMatch {
    line_number: usize,
    line_content: String,
    match_positions: Vec<(usize, usize)>,
}

#[derive(Debug, Serialize)]
struct GrepFileResult {
    file: String,
    matches: Vec<GrepMatch>,
}

#[derive(Debug, Serialize)]
struct GrepOutput {
    pattern: String,
    total_matches: usize,
    files_matched: usize,
    results: Vec<GrepFileResult>,
}

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

        let mut file_results: Vec<GrepFileResult> = Vec::new();
        let mut total_matches = 0;
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

            let mut file_matches: Vec<GrepMatch> = Vec::new();

            for (line_num, line) in content.lines().enumerate() {
                let line_number = line_num + 1;
                let search_line = if params.case_insensitive {
                    line.to_lowercase()
                } else {
                    line.to_string()
                };

                if let Some(mat) = regex.find(&search_line) {
                    let match_positions = vec![(mat.start(), mat.end())];
                    
                    file_matches.push(GrepMatch {
                        line_number,
                        line_content: line.to_string(),
                        match_positions,
                    });

                    total_matches += 1;

                    if total_matches >= params.max_results as usize {
                        break;
                    }
                }
            }

            if !file_matches.is_empty() {
                file_results.push(GrepFileResult {
                    file: relative_path,
                    matches: file_matches,
                });
            }

            if total_matches >= params.max_results as usize {
                break;
            }
        }

        let output = GrepOutput {
            pattern: params.pattern.clone(),
            total_matches,
            files_matched: file_results.len(),
            results: file_results,
        };

        let json_output = match serde_json::to_string_pretty(&output) {
            Ok(j) => j,
            Err(e) => return ToolResult::error(format!("Failed to serialize results: {e}")),
        };

        if total_matches == 0 {
            ToolResult::success(format!("No matches found for pattern '{}'", params.pattern))
        } else {
            ToolResult::success(json_output)
        }
    }
}
