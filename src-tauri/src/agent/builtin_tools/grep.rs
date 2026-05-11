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
    line_number:     usize,
    line_content:    String,
    match_positions: Vec<(usize, usize)>,
    context_before:  Vec<String>,  // K.5 — lines before the match
    context_after:   Vec<String>,  // K.5 — lines after the match
}

#[derive(Debug, Serialize)]
struct GrepFileResult {
    file:    String,
    matches: Vec<GrepMatch>,
}

#[derive(Debug, Serialize)]
struct GrepOutput {
    pattern:       String,
    total_matches: usize,
    files_matched: usize,
    truncated:     bool,           // K.5 — true when max_results was hit
    results:       Vec<GrepFileResult>,
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

fn default_context_lines() -> u32 { 1 }   // Reduced from 2 — saves ~10% tokens per result
fn default_max_results() -> u32 { 30 }    // Reduced from 100 — 100 matches × JSON = 15K tokens

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
                    "description": "Optional glob pattern to filter files by path (e.g., '*.rs', 'src/**/*.tsx')"
                },
                "context_lines": {
                    "type": "integer",
                    "description": "Number of context lines to show before and after each match. Default: 1",
                    "default": 1
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of matches to return. Default: 30. Increase for exhaustive searches.",
                    "default": 30
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

        // K.4 — Apply (?i) inline flag for case-insensitive matching instead of
        // lowercasing the line at match time. The previous to_lowercase() approach
        // caused match_positions to point into the lowercased string, and uppercase
        // patterns (e.g. "MyFunction") returned zero results because the regex was
        // compiled for its original case and matched against a lowercased copy.
        let pattern_str = if params.case_insensitive {
            format!("(?i){}", params.pattern)
        } else {
            params.pattern.clone()
        };
        let regex = match regex::Regex::new(&pattern_str) {
            Ok(r) => r,
            Err(e) => return ToolResult::error(format!("Invalid regex pattern: {e}")),
        };

        let mut file_results: Vec<GrepFileResult> = Vec::new();
        let mut total_matches = 0;
        let mut truncated = false;
        let skip_dirs = ["node_modules", ".git", "target", "dist", "build", ".next", ".cache"];

        // Walk directory tree
        'walk: for entry in WalkDir::new(&search_path)
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

            // K.6 — Compute relative_path early so the glob filter can match
            // against the full path (e.g. "src/**/*.tsx") rather than just the
            // filename. The previous filename-only match made ** path components
            // silently ineffective.
            let relative_path = entry.path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            if let Some(glob_pattern) = &params.glob {
                let file_name = entry.file_name().to_string_lossy();
                // Match against the full relative path first (respects src/**/*.tsx),
                // then fall back to filename-only for simple patterns like *.rs.
                let matched = glob::Pattern::new(glob_pattern)
                    .map(|p| p.matches(&relative_path) || p.matches(file_name.as_ref()))
                    .unwrap_or(false);
                if !matched {
                    continue;
                }
            }

            // Read file and search for pattern
            let content = match fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut file_matches: Vec<GrepMatch> = Vec::new();

            // K.5 — Pre-split into indexed lines so context windows are O(1).
            let lines: Vec<&str> = content.lines().collect();
            for (line_num, line) in lines.iter().enumerate() {
                // K.4 — Match against original line; regex already has (?i) if needed.
                // Use find_iter (not find) to capture ALL match positions on the line,
                // not just the first one. Previously grep("import") on a line with
                // "import os, import sys" would only report one match position.
                let all_matches: Vec<(usize, usize)> = regex.find_iter(line)
                    .map(|m| (m.start(), m.end()))
                    .collect();
                if !all_matches.is_empty() {
                    let match_positions = all_matches;

                    // K.5 — Collect context lines before and after the match.
                    let ctx_start = line_num.saturating_sub(params.context_lines as usize);
                    let ctx_end   = (line_num + params.context_lines as usize + 1).min(lines.len());
                    let context_before: Vec<String> = lines[ctx_start..line_num]
                        .iter().map(|s| s.to_string()).collect();
                    let context_after: Vec<String>  = lines[line_num + 1..ctx_end]
                        .iter().map(|s| s.to_string()).collect();

                    file_matches.push(GrepMatch {
                        line_number: line_num + 1,
                        line_content: line.to_string(),
                        match_positions,
                        context_before,
                        context_after,
                    });

                    total_matches += 1;

                    if total_matches >= params.max_results as usize {
                        truncated = true;
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

            if truncated {
                break 'walk;
            }
        }

        let output = GrepOutput {
            pattern: params.pattern.clone(),
            total_matches,
            files_matched: file_results.len(),
            truncated,
            results: file_results,
        };

        // Use compact JSON (not pretty-print) — pretty adds ~35% size overhead
        // and agents don't need human-readable indentation.
        let json_output = match serde_json::to_string(&output) {
            Ok(j) => j,
            Err(e) => return ToolResult::error(format!("Failed to serialize results: {e}")),
        };

        if total_matches == 0 {
            ToolResult::success(format!("No matches found for pattern '{}'", params.pattern))
        } else if total_matches >= params.max_results as usize {
            // Result hit the cap — signal the agent to narrow the search if needed
            ToolResult::success_with_hint(
                json_output,
                format!(
                    "Result capped at {} matches. Extract only what you need; use a more specific \
                     pattern, path filter, or start_line/end_line with read_file to narrow scope.",
                    params.max_results
                ),
            )
        } else {
            ToolResult::success(json_output)
        }
    }
}
