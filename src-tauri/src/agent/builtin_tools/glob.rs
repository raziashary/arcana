use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tracing::debug;
use walkdir::WalkDir;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct GlobTool;

#[derive(Debug, Deserialize)]
struct GlobInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize { 200 }

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern. \
         Supports patterns like '*.rs', '**/*.py', 'src/**/test_*.rs'. \
         Returns list of matching file paths. \
         Automatically respects .gitignore and skips common noise directories."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g., '*.rs', '**/*.json', 'src/**/test_*.py')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in. Defaults to current working directory."
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Default: 200",
                    "default": 200
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
        let params: GlobInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        let search_path = if let Some(path) = &params.path {
            let p = PathBuf::from(path);
            if p.is_absolute() { p } else { PathBuf::from(&ctx.working_dir).join(path) }
        } else {
            PathBuf::from(&ctx.working_dir)
        };

        debug!("Globbing pattern '{}' in {:?}", params.pattern, search_path);

        let pattern = match glob::Pattern::new(&params.pattern) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid glob pattern: {e}")),
        };

        let mut matches = Vec::new();
        let skip_dirs = ["node_modules", ".git", "target", "dist", "build", ".next", ".cache"];

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

            let relative_path = entry.path().strip_prefix(&search_path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            if pattern.matches(&relative_path) || pattern.matches(entry.file_name().to_string_lossy().as_ref()) {
                matches.push(relative_path.to_string());
                if matches.len() >= params.max_results {
                    break;
                }
            }
        }

        matches.sort();

        if matches.is_empty() {
            ToolResult::success(format!("No files found matching pattern '{}'", params.pattern))
        } else {
            let truncated = if matches.len() >= params.max_results {
                format!("\n\n(Truncated at {} results)", params.max_results)
            } else {
                String::new()
            };
            ToolResult::success(format!(
                "Found {} file{}:\n\n{}{}",
                matches.len(),
                if matches.len() == 1 { "" } else { "s" },
                matches.join("\n"),
                truncated
            ))
        }
    }
}
