// check_file — Option B standalone lint tool.
//
// Gives agents on-demand access to all three validation tiers:
//   Tier 1 — syntax check  (always fast, in-process or lightweight subprocess)
//   Tier 2 — import cross-reference (pure Rust, walks requirements.txt / package.json)
//   Tier 3 — type checker  (mypy / pyright / tsc / eslint, slow — opt-in)
//
// Typical use: after write_file/edit_file flags a syntax error, the agent
// calls check_file to get the full diagnostic picture before re-writing.

use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use super::validate;

pub struct CheckFileTool;

#[async_trait]
impl Tool for CheckFileTool {
    fn name(&self) -> &str { "check_file" }

    fn description(&self) -> &str {
        "Run lint / syntax / import checks on a file without executing it. \
         Three tiers are available:\n\
         • syntax  — fast in-process parse (JSON/YAML/TOML) or subprocess \
           (py_compile for .py, node --check for .js). Always runs.\n\
         • imports — cross-reference imports against requirements.txt / \
           package.json. Pure Rust, no subprocess. Default: included.\n\
         • types   — slow type checker (mypy/pyright for Python, tsc for \
           TypeScript, eslint for JavaScript). Opt-in only.\n\n\
         Use after write_file or edit_file reports a syntax error, or before \
         marking a task done to verify the code is clean."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or working-directory-relative path to the file to check."
                },
                "checks": {
                    "type": "array",
                    "items": {
                        "type": "string",
                        "enum": ["syntax", "imports", "types"]
                    },
                    "description": "Which checks to run. Default: [\"syntax\", \"imports\"]. \
                                    Add \"types\" to invoke mypy/pyright/tsc (slow).",
                    "default": ["syntax", "imports"]
                },
                "type_checker": {
                    "type": "string",
                    "enum": ["auto", "mypy", "pyright", "tsc", "eslint"],
                    "description": "Which type checker to use when 'types' is in checks. \
                                    Default: \"auto\" (pyright for Python if available, else mypy; \
                                    tsc for TypeScript; eslint for JavaScript).",
                    "default": "auto"
                }
            },
            "required": ["path"]
        })
    }

    fn is_read_only(&self) -> bool { true }
    fn requires_permission(&self) -> bool { false }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        // ── Resolve path ──────────────────────────────────────────────────────
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None    => return ToolResult::error("Missing required field: path"),
        };
        let path = if PathBuf::from(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            PathBuf::from(&ctx.working_dir).join(path_str)
        };

        if !path.exists() {
            return ToolResult::error(format!(
                "File not found: '{}'. Use glob or list_directory to find the correct path.",
                path.display()
            ));
        }

        // ── Parse checks parameter ────────────────────────────────────────────
        let checks: Vec<String> = if let Some(arr) = input["checks"].as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            vec!["syntax".into(), "imports".into()]
        };

        let run_syntax  = checks.iter().any(|c| c == "syntax");
        let run_imports = checks.iter().any(|c| c == "imports");
        let run_types   = checks.iter().any(|c| c == "types");

        let type_checker = input["type_checker"].as_str()
            .filter(|&s| s != "auto")
            .map(String::from);

        // ── Run requested tiers ───────────────────────────────────────────────
        let mut sections: Vec<String> = Vec::new();
        let mut has_errors = false;

        // Tier 1 — syntax
        if run_syntax {
            match validate::tier1_syntax(&path, &ctx.working_dir).await {
                None => {
                    let ext = path.extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    sections.push(format!(
                        "=== Syntax ===\n[unsupported] No syntax checker for .{ext} files.",
                    ));
                }
                Some(result) => {
                    let status = if result.is_clean() {
                        format!("[{}] ✓ No syntax errors.", result.checker)
                    } else {
                        has_errors = true;
                        result.format_inline()
                    };
                    sections.push(format!("=== Syntax ===\n{status}"));
                }
            }
        }

        // Tier 2 — imports
        if run_imports {
            match validate::tier2_imports(&path, &ctx.working_dir) {
                None => {
                    let ext = path.extension()
                        .map(|e| e.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    sections.push(format!(
                        "=== Imports ===\n[unsupported] No import checker for .{ext} files.",
                    ));
                }
                Some(result) => {
                    let status = if result.is_clean() {
                        format!("[{}] ✓ All imports resolvable.", result.checker)
                    } else {
                        has_errors = true;
                        format!(
                            "{}\n  Hint: add missing packages to requirements.txt or package.json.",
                            result.format_inline()
                        )
                    };
                    sections.push(format!("=== Imports ===\n{status}"));
                }
            }
        }

        // Tier 3 — types
        if run_types {
            let result = validate::tier3_types(
                &path,
                &ctx.working_dir,
                type_checker.as_deref(),
            ).await;
            let status = if result.is_clean() {
                format!("[{}] ✓ No type errors.", result.checker)
            } else {
                has_errors = true;
                result.format_inline()
            };
            sections.push(format!("=== Types ===\n{status}"));
        }

        // ── Build response ────────────────────────────────────────────────────
        let display_path = path.strip_prefix(&ctx.working_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        let header = format!(
            "check_file: {} (checks: {})",
            display_path,
            checks.join(", ")
        );
        let body = sections.join("\n\n");
        let output = format!("{header}\n\n{body}");

        if has_errors {
            ToolResult::error(output)
        } else {
            ToolResult::success(output)
        }
    }
}
