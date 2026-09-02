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
    is_directory: bool,
    // size_bytes and modified_at omitted by default — agents rarely need them
    // and they add significant token cost at 200 results. Pass include_metadata=true
    // to get them.
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modified_at: Option<String>,
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
    #[serde(default)]
    include_metadata: bool, // If true, include size_bytes and modified_at
}

fn default_max_results() -> usize {
    200
}

#[async_trait]
impl Tool for GlobTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Read
    }

    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        // ⚠ P3-20. This claimed *"Automatically respects .gitignore"*. It does
        // not, and never did: there is no gitignore parsing anywhere in this
        // file — only the fixed `skip_dirs` list below. The claim mattered,
        // because a model told its results are gitignore-filtered will assume
        // ignored files (`.env`, credential dumps, build output) are absent
        // from what it gets back. Stating the real rule lets it check.
        "Find files matching a glob pattern. \
         Supports patterns like '*.rs', '**/*.py', 'src/**/test_*.rs'. \
         Returns list of matching file paths. \
         \
         Skips a fixed set of noise directories: node_modules, .git, target, \
         dist, build, .next, .cache. It does NOT read .gitignore — files your \
         VCS ignores WILL appear in these results unless they sit in one of \
         those directories."
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
                },
                "include_metadata": {
                    "type": "boolean",
                    "description": "When true, include size_bytes and modified_at in each result. Default: false.",
                    "default": false
                }
            },
            "required": ["pattern"]
        })
    }



    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let params: GlobInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        // P3-14: the resolved root is jailed below.
        let raw = if let Some(path) = &params.path {
            let p = PathBuf::from(path);
            if p.is_absolute() {
                p
            } else {
                PathBuf::from(&ctx.working_dir).join(path)
            }
        } else {
            PathBuf::from(&ctx.working_dir)
        };
        // ⚠ P3-14. This tool did not call the jail and was auto-approved.
        // Reading outside the workspace is an **exfiltration primitive**, not a
        // harmless read: the model names a path, the contents enter its
        // context, and nothing asked the user. Six tools bypassed the jail;
        // two of them acted on the path rather than merely reading it.
        let search_path = match super::path_jail::validate_workspace_path(
            &raw,
            &ctx.working_dir,
        ) {
            Ok(canonical) => canonical,
            Err(e) => return ToolResult::error(e),
        };

        debug!("Globbing pattern '{}' in {:?}", params.pattern, search_path);

        let pattern = match glob::Pattern::new(&params.pattern) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid glob pattern: {e}")),
        };

        let mut file_entries = Vec::new();
        let skip_dirs = [
            "node_modules",
            ".git",
            "target",
            "dist",
            "build",
            ".next",
            ".cache",
        ];

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

            let relative_path = entry
                .path()
                .strip_prefix(&search_path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");

            let is_match = pattern.matches(&relative_path)
                || pattern.matches(entry.file_name().to_string_lossy().as_ref());

            if is_match {
                let metadata = match fs::metadata(entry.path()) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let (size_bytes, modified_at) = if params.include_metadata {
                    let sz = Some(metadata.len());
                    let mt = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| {
                            chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                                .unwrap_or_else(|| "unknown".to_string())
                        });
                    (sz, mt)
                } else {
                    (None, None)
                };

                file_entries.push(GlobFileEntry {
                    path: relative_path,
                    is_directory: metadata.is_dir(),
                    size_bytes,
                    modified_at,
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

        // Compact JSON — pretty-print adds ~35% size overhead with no benefit to agents.
        let json_output = match serde_json::to_string(&output) {
            Ok(j) => j,
            Err(e) => return ToolResult::error(format!("Failed to serialize results: {e}")),
        };

        if output.total_files == 0 {
            ToolResult::success(format!(
                "No files found matching pattern '{}'",
                params.pattern
            ))
        } else if output.total_files >= params.max_results {
            // Hit the cap — guide the agent to narrow scope
            ToolResult::success_with_hint(
                json_output,
                format!(
                    "Result capped at {} files. If you need a specific file, use a more precise \
                     pattern or filter by subdirectory to narrow scope.",
                    params.max_results
                ),
            )
        } else {
            ToolResult::success(json_output)
        }
    }
}
