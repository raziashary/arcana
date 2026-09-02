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

use super::validate;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

pub struct CheckFileTool;

#[async_trait]
impl Tool for CheckFileTool {
    // ⚠ `Execute`, not `Read`. It runs external checkers, including one it
    // resolves from **inside the workspace under inspection** (`P3-19`), so a
    // cloned repository can supply the binary. It declared `is_read_only = true`.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Execute
    }

    fn name(&self) -> &str {
        "check_file"
    }

    fn description(&self) -> &str {
        "Run lint / syntax / import checks on a file without executing it. \
         Three tiers are available:\n\
         • syntax  — fast in-process parse (JSON/YAML/TOML) or subprocess \
           (py_compile for .py, node --check for .js/.mjs/.cjs). Always runs.\n\
         • imports — cross-reference imports against requirements.txt / \
           package.json. Covers .py, .js, .ts, .tsx, .jsx. Default: included.\n\
         • types   — slow type checker (mypy/pyright for .py; tsc for .ts/.tsx; \
           eslint for .js/.jsx). Opt-in only.\n\n\
         Tiers that have no checker for the given file extension are silently \
         skipped — only actionable results are returned.\n\n\
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
                        "enum": ["syntax", "imports", "exports", "contract", "types"]
                    },
                    "description": "Which checks to run. Default: \
                                    [\"syntax\", \"imports\", \"exports\", \"contract\"]. \
                                    Add \"types\" to invoke mypy/pyright/tsc (slow).",
                    "default": ["syntax", "imports", "exports", "contract"]
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


    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        // ── Resolve path ──────────────────────────────────────────────────────
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None => return ToolResult::error("Missing required field: path"),
        };
        let raw = if PathBuf::from(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            PathBuf::from(&ctx.working_dir).join(path_str)
        };
        // ⚠ P3-14, and this is one of the two that **act** on the path rather
        // than reading it: `check_file` spawns external checkers against it,
        // including one it resolves from inside the workspace (`P3-19`). An
        // unjailed path here is not a leak, it is an execution target.
        let path = match super::path_jail::validate_workspace_path(&raw, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(e) => return ToolResult::error(e),
        };

        if !path.exists() {
            return ToolResult::error(format!(
                "File not found: '{}'. Use glob or list_directory to find the correct path.",
                path.display()
            ));
        }

        // ── Parse checks parameter ────────────────────────────────────────────
        // Doc 09 T1.4 — defaults now include the two passes that were the
        // most expensive misses in the 14 May calendar run: tier2_5_exports
        // (consumer/producer drift) and tier2_6_api_contract (URL mismatches
        // against INTERFACE.md). `types` stays opt-in because Tier 3 is
        // genuinely slow (multi-second subprocesses).
        let checks: Vec<String> = if let Some(arr) = input["checks"].as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            vec![
                "syntax".into(),
                "imports".into(),
                "exports".into(),
                "contract".into(),
            ]
        };

        let run_syntax = checks.iter().any(|c| c == "syntax");
        let run_imports = checks.iter().any(|c| c == "imports");
        let run_exports = checks.iter().any(|c| c == "exports");
        let run_contract = checks.iter().any(|c| c == "contract");
        let run_types = checks.iter().any(|c| c == "types");

        let type_checker = input["type_checker"]
            .as_str()
            .filter(|&s| s != "auto")
            .map(String::from);

        // ── Run requested tiers ───────────────────────────────────────────────
        let mut sections: Vec<String> = Vec::new();
        let mut has_errors = false;

        // Tier 1 — syntax
        // None means the extension has no checker — skip silently (no noise for .rs, .css, etc.)
        if run_syntax {
            if let Some(result) = validate::tier1_syntax(&path, &ctx.working_dir).await {
                let status = if result.is_clean() {
                    format!("[{}] ✓ No syntax errors.", result.checker)
                } else if !result.ran() {
                    // ⚠ P3-18. A checker that could not run is NOT an error —
                    // razi-ruled 2026-09-02. The defect being fixed is the false
                    // *claim*, not the absent tool: an uninstalled `tsc` must
                    // never render as "✓ No syntax errors", but neither should
                    // it fail a tool call the model cannot possibly repair by
                    // retrying. `format_inline` renders "NOT CHECKED".
                    result.format_inline()
                } else {
                    has_errors = true;
                    result.format_inline()
                };
                sections.push(format!("=== Syntax ===\n{status}"));
            }
        }

        // Tier 2 — imports
        // None means unsupported extension or no manifest found — skip silently.
        if run_imports {
            if let Some(result) = validate::tier2_imports(&path, &ctx.working_dir) {
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

        // Tier 2.5 — exports (Doc 09 T1.3)
        if run_exports {
            if let Some(result) = validate::tier2_5_exports(&path, &ctx.working_dir) {
                let status = if result.is_clean() {
                    format!(
                        "[{}] ✓ All named imports resolve to actual exports.",
                        result.checker
                    )
                } else {
                    has_errors = true;
                    format!(
                        "{}\n  Hint: the producing module is missing one or more of these names.",
                        result.format_inline()
                    )
                };
                sections.push(format!("=== Exports ===\n{status}"));
            }
        }

        // Tier 2.6 — API contract (Doc 09 T3.3)
        if run_contract {
            if let Some(result) = validate::tier2_6_api_contract(&path, &ctx.working_dir) {
                let status = if result.is_clean() {
                    format!(
                        "[{}] ✓ All API URLs documented in INTERFACE.md.",
                        result.checker
                    )
                } else {
                    has_errors = true;
                    format!(
                        "{}\n  Hint: fix either INTERFACE.md (backend) or the URL (frontend).",
                        result.format_inline()
                    )
                };
                sections.push(format!("=== API contract ===\n{status}"));
            }
        }

        // Tier 3 — types
        if run_types {
            let result =
                validate::tier3_types(&path, &ctx.working_dir, type_checker.as_deref()).await;
            let status = if result.is_clean() {
                format!("[{}] ✓ No type errors.", result.checker)
            } else if !result.ran() {
                // ⚠ P3-18, as above: no type checker installed (or none
                // configured for this extension) is a gap in coverage, not a
                // defect in the file.
                result.format_inline()
            } else {
                has_errors = true;
                result.format_inline()
            };
            sections.push(format!("=== Types ===\n{status}"));
        }

        // ── Build response ────────────────────────────────────────────────────
        let display_path = path
            .strip_prefix(&ctx.working_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");

        // If no tier produced output (all checkers returned None for this extension),
        // surface a concise note rather than returning an empty body.
        if sections.is_empty() {
            let ext = path
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
                .unwrap_or_else(|| "(no extension)".into());
            return ToolResult::success(format!(
                "check_file: {display_path}\n\
                 No checkers available for {ext} files in the requested tiers ({}).\n\
                 Supported: syntax → .json/.yaml/.toml/.py/.js/.ts/.tsx; \
                 imports → .py/.js/.ts/.tsx/.jsx; \
                 exports → .js/.ts/.tsx/.jsx; \
                 contract → .js/.ts/.tsx (when INTERFACE.md present); \
                 types → .py/.ts/.tsx/.js/.jsx",
                checks.join(", ")
            ));
        }

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
