use async_trait::async_trait;
use serde_json::json;
use std::path::Path;

use super::path_jail::validate_workspace_path;
use super::path_lock;
use super::validate;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Create a NEW file at the given path. Refuses by default if the file already \
         exists — use edit_file to modify an existing file. To intentionally overwrite \
         (e.g. regenerating a generated artifact), pass overwrite=true and the call \
         will replace the file's contents. Creates parent directories as needed."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "Absolute or working-directory-relative path to write."
                },
                "content": {
                    "type":        "string",
                    "description": "Full file content to write."
                },
                "overwrite": {
                    "type":        "boolean",
                    "description": "Optional. Default false. When false and the file already \
                                    exists, the call is refused — use edit_file instead. Set \
                                    to true only when the intent is to regenerate or wholly \
                                    replace the file."
                }
            },
            "required": ["path", "content"]
        })
    }


    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input["path"].as_str() {
            Some(p) => p,
            None => return ToolResult::error("Missing required field: path"),
        };
        let content = match input["content"].as_str() {
            Some(c) => c,
            None => return ToolResult::error("Missing required field: content"),
        };
        let overwrite = input["overwrite"].as_bool().unwrap_or(false);

        let resolved = resolve_path(path_str, &ctx.working_dir);

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        // Refuse any write whose canonical destination escapes the configured
        // workspace. Without this, an agent passing a relative path resolves
        // it against `std::env::current_dir()`, and files leak into
        // `src-tauri/` during `tauri dev`. The error message tells the model
        // exactly how to retry.
        let path = match validate_workspace_path(&resolved, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg) => return ToolResult::error(msg),
        };

        // Cross-tool, cross-worker per-path serialisation. Held for the
        // duration of this call so a concurrent edit_file / append_file /
        // write_file targeting the same path waits its turn rather than
        // racing the read-modify-write cycle.
        //
        // ⚠ P3-16. This locked the **pre-jail** `resolved` path while
        // `edit_file` and `append_file` lock the **post-jail canonical** one —
        // two different strings, two different mutex slots, for the same file.
        // The lost-update race that `edit_file`'s own comment documents was
        // therefore live between those tools.
        //
        // The comment here claimed *"lock_key normalises both forms to the same
        // slot"*. It did not: lexical normalisation cannot resolve symlinks or
        // case, which is precisely what separates the two forms.
        //
        // All three now lock the jailed path. `validate_workspace_path` handles
        // a non-existent tail by canonicalising the existing ancestor, so the
        // original objection — that a fresh file has no canonical path — does
        // not hold.
        let _lock_guard = path_lock::acquire_for(&path, &ctx.working_dir).await;

        // Refuse-on-exists. Workers create files with write_file and modify
        // them with edit_file; rewriting an existing file via write_file is
        // the canonical "I forgot the previous worker already created this"
        // bug, and is the path that produced the 14 May calendar run's
        // duplicate-scaffold problem. Require an explicit overwrite=true so
        // the agent has to acknowledge the intent.
        //
        // The check happens AFTER the path lock is acquired so two
        // concurrent first-time writers can't both observe "doesn't exist"
        // and both write the file.
        if path.exists() && !overwrite {
            return ToolResult::error(format!(
                "File '{}' already exists. write_file refuses to overwrite an existing \
                 file without explicit consent. To modify the file, call edit_file with \
                 the specific old_string→new_string change. If you really do want to \
                 replace the file wholesale (e.g. regenerating a generated artifact), \
                 retry with overwrite=true. This guard prevents one worker from silently \
                 clobbering another worker's freshly-created file.",
                path.display()
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!(
                    "Cannot create directories for '{}': {e}",
                    path.display()
                ));
            }
        }

        // Phase 3.3 — auto-checkpoint before overwrite so the edit is undoable.
        crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            &path.to_string_lossy(),
        );

        match std::fs::write(&path, content) {
            Ok(_) => {
                // Fire PostFileWrite hook after successful write
                let hook_ctx = HookContext {
                    hook_type: HookType::PostFileWrite.as_str().to_string(),
                    timestamp: chrono::Utc::now(),
                    session_id: Some(ctx.session_id.clone()),
                    agent_mode: None,
                    tool_call: Some(ToolCallContext {
                        id: ctx.call_id.clone(),
                        name: "write_file".to_string(),
                        input: input.clone(),
                        result: Some(serde_json::json!({"bytes_written": content.len()})),
                        is_error: false,
                    }),
                    file_path: Some(path.to_string_lossy().to_string()),
                    task_id: None,
                    worker_id: None,
                    extra: serde_json::json!({
                        "bytes_written": content.len(),
                    }),
                };
                tokio::spawn(async move {
                    HookRegistry::fire(&HookType::PostFileWrite, &hook_ctx).await;
                });

                // Option A — Tier 1 syntax validation (always-on, fast).
                // Appends lint feedback to the success message so the agent can
                // self-correct without a separate check_file call.
                let mut lint_parts: Vec<String> = Vec::new();
                if let Some(result) = validate::tier1_syntax(&path, &ctx.working_dir).await {
                    if !result.diagnostics.is_empty() {
                        lint_parts.push(result.format_inline());
                    }
                }
                // Doc 09 T1.1 — also run Tier 2 import cross-reference after
                // every write to .py/.js/.jsx/.ts/.tsx/.cjs/.mjs. The calendar
                // run shipped a `package.json` with zero frontend deps despite
                // ~12 `import` statements; tier2_imports catches that in-
                // process on the very write that introduced the mismatch.
                if let Some(result) = validate::tier2_imports(&path, &ctx.working_dir) {
                    if !result.is_clean() {
                        lint_parts.push(format!(
                            "{}\n  Hint: add missing packages to package.json / requirements.txt.",
                            result.format_inline()
                        ));
                    }
                }
                // Doc 09 T1.3 — Tier 2.5 export cross-reference. Catches
                // consumers importing names that the producing module never
                // exports (the `isDeleting`-not-returned bug class).
                if let Some(result) = validate::tier2_5_exports(&path, &ctx.working_dir) {
                    if !result.is_clean() {
                        lint_parts.push(format!(
                            "{}\n  Hint: the importing file references symbols not exported by the target module.",
                            result.format_inline()
                        ));
                    }
                }
                // Doc 09 T3.3 — Tier 2.6 API contract. When the backend has
                // already written INTERFACE.md (Doc 09 T3.1), every URL the
                // frontend uses must match. Skipped silently when no
                // INTERFACE.md exists yet — the file gates itself by the
                // backend worker's lifecycle, not by a config flag.
                if let Some(result) = validate::tier2_6_api_contract(&path, &ctx.working_dir) {
                    if !result.is_clean() {
                        lint_parts.push(format!(
                            "{}\n  Hint: the backend's INTERFACE.md does not declare these URLs. \
                             Either update INTERFACE.md (backend) or fix the URL (frontend).",
                            result.format_inline()
                        ));
                    }
                }
                let lint_note = if lint_parts.is_empty() {
                    String::new()
                } else {
                    format!("\n\n⚠ Lint feedback:\n{}", lint_parts.join("\n\n"))
                };

                ToolResult::success(format!(
                    "Wrote {} bytes to '{}'{lint_note}",
                    content.len(),
                    path.display()
                ))
            }
            Err(e) => ToolResult::error(format!("Cannot write '{}': {e}", path.display())),
        }
    }
}

fn resolve_path(path: &str, working_dir: &str) -> std::path::PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(working_dir).join(path)
    }
}
