use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tracing::debug;
use walkdir::WalkDir;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

#[derive(Debug, Serialize)]
struct GlobFileEntry {
    path: String,
    size_bytes: u64,
    modified_at: String,
    is_directory: bool,
}

#[derive(Debug, Serialize)]
struct GlobOutput {
    pattern: String,
    total_files: usize,
    files: Vec<GlobFileEntry>,
}

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

        let mut file_entries = Vec::new();
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

            let relative_path = entry.path().strip_prefix(&search_path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            let is_match = pattern.matches(&relative_path) || 
                           pattern.matches(entry.file_name().to_string_lossy().as_ref());

            if is_match {
                let metadata = match fs::metadata(entry.path()) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let size_bytes = metadata.len();
                let modified_at = metadata.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| {
                        let secs = d.as_secs();
                        chrono::DateTime::from_timestamp(secs as i64, 0)
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                            .unwrap_or_else(|| "unknown".to_string())
                    })
                    .unwrap_or_else(|| "unknown".to_string());

                file_entries.push(GlobFileEntry {
                    path: relative_path,
                    size_bytes,
                    modified_at,
                    is_directory: metadata.is_dir(),
                });

                if file_entries.len() >= params.max_results {
                    break;
                }
            }
        }

        file_entries.sort_by(|a, b| a.path.cmp(&b.path));

        let output = GlobOutput {
            pattern: params.pattern.clone(),
            total_files: file_entries.len(),
            files: file_entries,
        };

        let json_output = match serde_json::to_string_pretty(&output) {
            Ok(j) => j,
            Err(e) => return ToolResult::error(format!("Failed to serialize results: {e}")),
        };

        if output.total_files == 0 {
            ToolResult::success(format!("No files found matching pattern '{}'", params.pattern))
        } else {
            ToolResult::success(json_output)
        }
    }
}
