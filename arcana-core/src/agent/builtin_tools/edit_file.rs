use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio::fs;
use tracing::debug;

use super::path_jail::validate_workspace_path;
use super::path_lock;
use super::validate;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::hooks::{HookContext, HookRegistry, HookType, ToolCallContext};

pub struct EditFileTool;

#[derive(Debug, Deserialize)]
struct EditFileInput {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditFileTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a specific string with another. \
         Use this for targeted modifications like changing a function, fixing a line, or updating a value. \
         The old_string must match EXACTLY (including whitespace and indentation). \
         Fails if old_string is not found or appears multiple times (unless replace_all=true). \
         For large rewrites, prefer write_file instead."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Must not be empty. Exact text to find and replace. Must match exactly including whitespace."
                },
                "new_string": {
                    "type": "string",
                    "description": "Text to replace old_string with. Can be empty to delete."
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replaces all occurrences. If false (default), fails if there are multiple matches.",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }



    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        // Clone before moving into from_value so the hook context below can reference it.
        let input_for_hook = input.clone();
        let params: EditFileInput = match serde_json::from_value(input) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid input: {e}")),
        };

        let resolved = if PathBuf::from(&params.path).is_absolute() {
            PathBuf::from(&params.path)
        } else {
            PathBuf::from(&ctx.working_dir).join(&params.path)
        };

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        // Same rationale as write_file: refuse edits whose canonical
        // destination escapes the configured workspace.
        let file_path = match validate_workspace_path(&resolved, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg) => return ToolResult::error(msg),
        };

        // Cross-tool, cross-worker per-path lock. Held across the entire
        // read → string-search → write sequence so a concurrent writer (any
        // other worker via write_file / edit_file / append_file, or the same
        // worker through a second invocation) cannot extend or rewrite the
        // file between our `fs::read_to_string` and `fs::write`. Without
        // this, the classic edit_file race produces a silent lost update:
        // worker A reads, worker B reads + writes, worker A writes — A
        // overwrites B's change with stale data.
        //
        // One registry for every writer, so an assistant edit, a fleet
        // worker edit and an ephemeral subagent edit all contend on a
        // single key per file. Different files run in parallel as before.
        let _lock_guard = path_lock::acquire_for(&file_path, &ctx.working_dir).await;

        debug!("Editing file: {:?}", file_path);

        let content = match fs::read_to_string(&file_path).await {
            Ok(c) => c,
            Err(e) => {
                // If the file doesn't exist, point the agent at `write_file` instead.
                // Workers bootstrapping a project commonly try edit_file first; failing
                // with a bare I/O error encourages a retry loop on a doomed path.
                if e.kind() == std::io::ErrorKind::NotFound {
                    return ToolResult::error(format!(
                        "File '{}' does not exist. To create a new file, call \
                         `write_file(path='{}', content=...)` instead — edit_file only \
                         modifies existing files.",
                        file_path.display(),
                        params.path
                    ));
                }
                return ToolResult::error(format!(
                    "Failed to read file '{}': {}",
                    file_path.display(),
                    e
                ));
            }
        };

        // 🔴 An empty `old_string` is not an edit, and both branches below
        // handle it catastrophically.
        //
        // `"abc".matches("")` yields one match per char boundary, so
        // `match_count` is `len + 1` and the code takes the "found many times"
        // path. There, `content[start..].find("")` returns `Some(0)` for ever
        // and `start += pos + 0` never advances — an **infinite loop** that
        // also grows `positions` without bound, while holding this path's lock
        // so no other tool can touch the file either.
        //
        // With `replace_all = true` it does not loop; it is worse.
        // `content.replace("", new)` inserts `new` at **every** character
        // boundary, so a 10 KB source file becomes ten thousand copies of the
        // replacement interleaved with the original — and that is what gets
        // written to disk. The user's file is destroyed, and the tool reports
        // success.
        //
        // The schema marks `old_string` required but never says non-empty, and
        // a model that means "insert at the top of the file" reaches for `""`
        // as the obvious way to say it. So this is not a hostile input, it is
        // an ordinary one.
        if params.old_string.is_empty() {
            return ToolResult::error(format!(
                "'old_string' must not be empty. An empty match has no location \
                 in '{}' — it matches at every position, so there is nothing to \
                 replace. To insert text, pass the surrounding line as \
                 'old_string' and include it in 'new_string'; to create or \
                 overwrite the whole file, use write_file.",
                params.path
            ));
        }

        let match_count = content.matches(&params.old_string).count();

        if match_count == 0 {
            // Do NOT include a file preview here — it re-injects up to 500 chars of file
            // content into the agent context on every failed edit attempt, compounding
            // token cost across retries. The agent already has the file if it read it.
            // Instead, give the line count so the agent can target read_file(start/end).
            let line_count = content.lines().count();
            return ToolResult::error(format!(
                "old_string not found in '{}' ({line_count} lines total). \
                 The text must match exactly including whitespace and indentation. \
                 Use read_file(path, start_line=X, end_line=Y) to inspect the target area \
                 before retrying.",
                params.path
            ));
        }

        if match_count > 1 && !params.replace_all {
            let mut positions = Vec::new();
            let mut start = 0;
            while let Some(pos) = content[start..].find(&params.old_string) {
                let line_num = content[..start + pos].lines().count() + 1;
                positions.push(line_num);
                start += pos + params.old_string.len();
            }

            return ToolResult::error(format!(
                "old_string found {} times at lines: {:?}. Use replace_all=true to replace all occurrences, or make old_string more specific.",
                match_count, positions
            ));
        }

        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            content.replacen(&params.old_string, &params.new_string, 1)
        };

        let old_lines = content.lines().count();
        let new_lines = new_content.lines().count();

        // Phase 3.3 — snapshot current contents before overwriting.
        crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            &file_path.to_string_lossy(),
        );

        if let Err(e) = fs::write(&file_path, &new_content).await {
            return ToolResult::error(format!(
                "Failed to write file '{}': {}",
                file_path.display(),
                e
            ));
        }

        // K.3 — Fire PostFileWrite hook (same as write_file.rs) so the graph
        // engine, telemetry, and hook consumers observe agent edits.
        let file_path_str = file_path.to_string_lossy().to_string();
        let hook_ctx = HookContext {
            hook_type: HookType::PostFileWrite.as_str().to_string(),
            timestamp: chrono::Utc::now(),
            session_id: Some(ctx.session_id.clone()),
            agent_mode: None,
            tool_call: Some(ToolCallContext {
                id: ctx.call_id.clone(),
                name: "edit_file".to_string(),
                input: input_for_hook,
                result: Some(json!({"bytes_written": new_content.len()})),
                is_error: false,
            }),
            file_path: Some(file_path_str),
            task_id: None,
            worker_id: None,
            extra: json!({
                "bytes_written": new_content.len(),
                "occurrences_replaced": if params.replace_all { match_count } else { 1 },
            }),
        };
        tokio::spawn(async move {
            HookRegistry::fire(&HookType::PostFileWrite, &hook_ctx).await;
        });

        let occurrences = if params.replace_all { match_count } else { 1 };

        // Option A — Tier 1 syntax validation (always-on, fast).
        // Appended to the success response so the agent knows immediately if
        // its edit introduced a syntax error, without needing a check_file call.
        let lint_note = match validate::tier1_syntax(&file_path, &ctx.working_dir).await {
            Some(result) if !result.diagnostics.is_empty() => {
                format!("\n\n⚠ Lint feedback:\n{}", result.format_inline())
            }
            _ => String::new(),
        };

        ToolResult::success(format!(
            "✓ Edited {} ({} occurrence{} replaced, {} lines → {} lines){lint_note}",
            params.path,
            occurrences,
            if occurrences == 1 { "" } else { "s" },
            old_lines,
            new_lines
        ))
    }
}

#[cfg(test)]
mod empty_old_string_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;

    fn ctx(dir: &std::path::Path) -> ToolContext {
        ToolContext {
            session_id: "s1".into(),
            working_dir: dir.to_string_lossy().to_string(),
            call_id: "c1".into(),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),
        }
    }

    fn sandbox(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("arcana-editfile-{tag}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// 🔴 An empty `old_string` had two catastrophic outcomes and no guard.
    ///
    /// ⚠ Only the `replace_all = true` half is exercised by *running* it. The
    /// other half is an **infinite loop**, so a test that drove it would hang
    /// the suite rather than fail it — it is proven by reading
    /// (`content[start..].find("")` is always `Some(0)` and `start` advances by
    /// `pos + 0`), and it is closed by the same guard this asserts.
    #[tokio::test]
    async fn an_empty_old_string_is_refused_rather_than_shredding_the_file() {
        let dir = sandbox("shred");
        let file = dir.join("src.rs");
        let original = "fn main() {\n    println!(\"hello\");\n}\n";
        std::fs::write(&file, original).unwrap();

        let out = EditFileTool
            .execute(
                json!({
                    "path": file.to_string_lossy(),
                    "old_string": "",
                    "new_string": "X",
                    "replace_all": true
                }),
                &ctx(&dir),
            )
            .await;

        assert!(
            out.is_error,
            "an empty old_string was accepted — `content.replace(\"\", \"X\")` \
             inserts at EVERY character boundary, so this call rewrites the \
             file as shredded output and reports success"
        );

        // The file is the real assertion: a refusal that still wrote is not a
        // refusal.
        let after = std::fs::read_to_string(&file).unwrap();
        assert_eq!(
            after, original,
            "the file was modified despite the call being refused"
        );
    }

    /// The no-fire half: a real edit still works, or the guard above is just
    /// breaking the tool.
    #[tokio::test]
    async fn an_ordinary_edit_still_applies() {
        let dir = sandbox("ok");
        let file = dir.join("src.rs");
        std::fs::write(&file, "let x = 1;\n").unwrap();

        let out = EditFileTool
            .execute(
                json!({
                    "path": file.to_string_lossy(),
                    "old_string": "let x = 1;",
                    "new_string": "let x = 2;"
                }),
                &ctx(&dir),
            )
            .await;

        assert!(!out.is_error, "a valid edit was refused: {}", out.output);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "let x = 2;\n");
    }

    /// The schema tells the model before it tries, so the refusal above is a
    /// backstop rather than the only signal.
    #[test]
    fn the_schema_declares_old_string_non_empty() {
        let schema = EditFileTool.input_schema();
        let old = &schema["properties"]["old_string"];
        assert_eq!(
            old["minLength"], 1,
            "the schema permits an empty old_string, so a well-behaved model \
             has no way to know it is invalid"
        );
    }
}
