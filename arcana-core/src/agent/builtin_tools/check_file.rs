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
        // ⚠ P3-20, three corrections in one description:
        //   1. "Always runs" — false. `run_syntax` is `checks.contains("syntax")`;
        //      the tier is in the DEFAULT set, but a caller passing
        //      `checks: ["types"]` skips it entirely.
        //   2. The syntax tier's coverage omitted .ts/.tsx/.jsx, which it has
        //      handled via `tsc` since Doc 09 T1.2 (`validate.rs`, "ts"|"tsx"|"jsx").
        //   3. "silently skipped" is no longer true after P3-18: a tier with no
        //      checker now reports NOT CHECKED rather than vanishing, precisely
        //      so absence of a check stops reading as a pass.
        "Run lint / syntax / import checks on a file without executing it. \
         Three tiers are available:\n\
         • syntax  — fast in-process parse (JSON/YAML/TOML) or subprocess \
           (py_compile for .py, node --check for .js/.mjs/.cjs, tsc for \
           .ts/.tsx/.jsx). Included by default; omitted if you pass a \
           `checks` list without \"syntax\".\n\
         • imports — cross-reference imports against requirements.txt / \
           package.json. Covers .py, .js, .ts, .tsx, .jsx. Default: included.\n\
         • types   — slow type checker (mypy/pyright for .py; tsc for .ts/.tsx; \
           eslint for .js/.jsx). Opt-in only.\n\n\
         A tier with no checker for the file's extension is skipped without \
         output. A tier whose checker exists but could NOT RUN (binary not \
         installed, or it exceeded its 30s ceiling) reports \"NOT CHECKED\" — \
         that is not a pass, and the file remains unverified.\n\n\
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

        // 🔴 T-28 — refuse a request that would run NOTHING.
        //
        // `checks` is matched by exact string, so `["lint"]`, `["Syntax"]` or
        // a typo selected no check at all — and the tool then returned
        // **SUCCESS**, having executed nothing. A model asking "is this file
        // valid?" was told yes by a function that never looked. That is the
        // worst shape a false green can take, because the answer is
        // indistinguishable from a real pass.
        if !run_syntax && !run_imports {
            return ToolResult::error(&format!(
                "no runnable check was selected. `checks` was {checks:?}; the \
                 recognised values are \"syntax\" and \"imports\". Nothing was \
                 run, so nothing is being reported as passing."
            ));
        }
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

#[cfg(test)]
mod p8_33_t28_tests {
    //! `T-28` — `check_file` returned SUCCESS having run nothing.
    //!
    //! `checks` is matched by exact string, so `["lint"]`, `["Syntax"]` or any
    //! typo selected no check — and the tool answered **success**. A model
    //! asking *"is this file valid?"* was told yes by a function that never
    //! looked, and the answer was byte-identical to a real pass.

    use super::*;

    // ⚠ One sandbox PER TEST, keyed by tag. All three shared a single
    // directory and each called `remove_dir_all` on entry, so under cargo's
    // default parallelism one test deleted the file another was reading. They
    // passed run alone and failed in the full suite -- `L-30`: my own tests are
    // the first concurrent users of anything process-global.
    fn tmp_py(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("arcana-p8-33-t28-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let f = dir.join("ok.py");
        std::fs::write(&f, "x = 1\n").expect("write");
        f
    }

    fn ctx(dir: &std::path::Path) -> ToolContext {
        // Built field-by-field, matching `edit_file.rs`'s own test helper --
        // there is no `for_tests` constructor and inventing one was this
        // test's first mistake (`L-9`: verify the API, do not assume it).
        ToolContext {
            session_id: "s1".into(),
            working_dir: dir.to_string_lossy().to_string(),
            call_id: "c1".into(),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),
        }
    }

    fn run(path: &std::path::Path, checks: serde_json::Value) -> ToolResult {
        let tool = CheckFileTool;
        let dir = path.parent().expect("parent").to_path_buf();
        tokio::runtime::Runtime::new()
            .expect("rt")
            .block_on(tool.execute(
                serde_json::json!({ "path": path.to_string_lossy(), "checks": checks }),
                &ctx(&dir),
            ))
    }

    /// 🔴 The false green.
    #[test]
    fn t28_an_unrecognised_check_is_refused_rather_than_passed() {
        let f = tmp_py("unrecognised");
        let out = run(&f, serde_json::json!(["lint"]));
        assert!(
            out.is_error,
            "an unrecognised check reported SUCCESS having run nothing: {:?}",
            out.output
        );
        assert!(
            out.output.contains("syntax"),
            "the message must name what IS recognised: {}",
            out.output
        );
    }

    #[test]
    fn t28_an_empty_check_list_is_refused() {
        let f = tmp_py("empty");
        assert!(run(&f, serde_json::json!([])).is_error);
    }

    /// The falsification: a real check must still run, or "refuse everything"
    /// would satisfy the two tests above while breaking the tool.
    #[test]
    fn t28_a_recognised_check_still_runs() {
        let f = tmp_py("syntax");
        let out = run(&f, serde_json::json!(["syntax"]));
        assert!(!out.is_error, "a legal check was refused: {}", out.output);
    }
}
