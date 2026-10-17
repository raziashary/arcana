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
                "recovery_intent_cursor": {
                    "type": "integer", "minimum": 1,
                    "description": "Only for an explicitly Supervisor-authorized retry: original journal intent cursor. Arcana revalidates and consumes its task/operation-bound grant once before running."
                },
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

        // Workspace path-jail. Relative file names are joined to the agent's
        // resolved workspace, while the shared resolver rejects an invalid or
        // relative workspace instead of letting the process CWD choose a root.
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

        // Phase 3.3 — auto-checkpoint before overwrite so the edit is undoable.
        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        if let Err(e) = crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            ctx.task_id,
            &path.to_string_lossy(),
        ) {
            return ToolResult::error(format!(
                "Write refused because its recovery checkpoint failed: {e}"
            ));
        }

        // Checkpoint first so a failed recovery record cannot leave even newly
        // created parent directories behind.
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return ToolResult::error(format!(
                    "Cannot create directories for '{}': {e}",
                    path.display()
                ));
            }
        }

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
                        "working_dir": ctx.working_dir,
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

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_trait::AgentIdentity;

    fn context(working_dir: String) -> ToolContext {
        ToolContext {
            session_id: "workspace-test-session".into(),
            working_dir,
            call_id: "workspace-test-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: AgentIdentity::supervisor().role,

            run_token_budget: None,
        }
    }

    #[tokio::test]
    async fn write_file_refuses_a_relative_workspace_without_touching_process_cwd() {
        let name = format!("arcana-workspace-refusal-{}.txt", uuid::Uuid::new_v4());
        let result = WriteFileTool
            .execute(
                json!({"path": name, "content": "must not be written"}),
                &context(".".into()),
            )
            .await;
        assert!(result.is_error, "invalid workspace should stop the write");
        assert!(
            !Path::new(&name).exists(),
            "no CWD-relative file may appear"
        );
    }

    #[tokio::test]
    async fn write_file_creates_a_workspace_relative_file_under_the_canonical_root() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("workspace");
        let mut tool_context = context(workspace.path().to_string_lossy().into_owned());
        tool_context.task_id = Some(99);
        let result = WriteFileTool
            .execute(
                json!({"path": "src/main.rs", "content": "fn main() {}\n"}),
                &tool_context,
            )
            .await;
        assert!(
            !result.is_error,
            "valid workspace write failed: {}",
            result.output
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("src/main.rs")).unwrap(),
            "fn main() {}\n"
        );
        let destination = std::fs::canonicalize(workspace.path().join("src/main.rs"))
            .expect("canonicalize written destination");
        let checkpoints = crate::git::checkpoint::list(None, None, Some(99))
            .expect("read task checkpoint through production store");
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(
            std::fs::canonicalize(&checkpoints[0].file_path).unwrap(),
            destination,
            "the checkpoint must store the canonical destination"
        );
    }

    #[tokio::test]
    async fn failed_checkpoint_blocks_file_and_parent_directory_creation() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let arcana_dir = home.path().join(".arcana");
        std::fs::create_dir_all(arcana_dir.join("config.db"))
            .expect("make config.db path unusable");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("workspace");
        let destination = workspace.path().join("new/nested/main.rs");

        let result = WriteFileTool
            .execute(
                json!({"path": "new/nested/main.rs", "content": "fn main() {}"}),
                &context(workspace.path().to_string_lossy().into_owned()),
            )
            .await;

        assert!(result.is_error);
        assert!(!destination.exists());
        assert!(!destination.parent().unwrap().exists());
    }

    #[tokio::test]
    async fn production_tool_executor_refuses_worker_writes_without_a_current_claim() {
        use crate::agent::permission::{new_pending_permissions, PermissionMode};
        use crate::agent::tool_executor::ToolExecutor;
        use crate::agent::tool_registry::ToolRegistry;
        use std::sync::Arc;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let foreign_task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "unclaimed write boundary fixture".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a write without the claim must not run".into(),
                created_by: Some("test".into()),
            })
            .expect("create isolated fixture task");
        drop(tasks);

        let mut registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        let tool = registry
            .get("write_file")
            .expect("FastDevelopment registers write_file");

        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
            "unclaimed-write-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker("worker-unclaimed-write"),
        );
        // A worker on no task at all.
        let unclaimed = executor
            .execute_batch(vec![(
                tool.clone(),
                serde_json::json!({
                    "path": "claimed/unclaimed.rs",
                    "content": "must not be written"
                }),
                "unclaimed-write-call".into(),
            )])
            .await
            .expect("executor returns a structured result");
        let refused = unclaimed.into_iter().next().expect("one result").1;
        assert!(refused.is_error, "unclaimed write must be refused");
        let envelope: serde_json::Value =
            serde_json::from_str(&refused.output).expect("versioned JSON failure envelope");
        assert_eq!(envelope["schema_version"], 1);
        assert_eq!(
            envelope["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Worker mutations require a task claim"),
            true,
            "refusal names the missing task claim: {}",
            refused.output
        );
        assert!(
            !workspace.path().join("claimed").exists(),
            "no parent directory may be created before the claim gate"
        );

        // A worker whose task context names a task not assigned to it.
        executor.set_task_id(Some(foreign_task.id));
        let foreign = executor
            .execute_batch(vec![(
                tool,
                serde_json::json!({
                    "path": "claimed/foreign.rs",
                    "content": "must not be written"
                }),
                "foreign-write-call".into(),
            )])
            .await
            .expect("executor returns a structured result");
        let foreign_refused = foreign.into_iter().next().expect("one result").1;
        assert!(foreign_refused.is_error, "foreign claim must be refused");
        let foreign_envelope: serde_json::Value =
            serde_json::from_str(&foreign_refused.output).expect("versioned JSON failure envelope");
        assert_eq!(foreign_envelope["schema_version"], 1);
        assert!(
            foreign_envelope["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("no longer owns the in-progress task"),
            "refusal names the stale ownership: {}",
            foreign_refused.output
        );
        assert!(
            !workspace.path().join("claimed/foreign.rs").exists(),
            "no file may be written under a foreign task context"
        );
    }

    #[tokio::test]
    async fn production_tool_executor_claim_identity_blocks_write_after_checkpoint_failure() {
        use crate::agent::permission::{new_pending_permissions, PermissionMode};
        use crate::agent::tool_executor::ToolExecutor;
        use crate::agent::tool_trait::AgentIdentity;
        use std::sync::Arc;

        let home = tempfile::tempdir().expect("isolated Arcana home");
        // Prepare failure injection before publishing process-global HOME.
        // Concurrent tests that persist counters must not win the path first.
        let arcana_dir = home.path().join(".arcana");
        std::fs::create_dir_all(arcana_dir.join("config.db"))
            .expect("make checkpoint database path unusable");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        let worker_slug = "worker-checkpoint-test";
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "checkpoint-gated file write".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "failed checkpoint blocks mutation".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        tasks
            .update_status(task.id, "assigned", Some(worker_slug))
            .expect("assign task to worker");
        tasks
            .claim_task_with_pid(task.id, worker_slug, std::process::id())
            .expect("claim task through production claim API");
        drop(tasks);

        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            new_pending_permissions(),
            "checkpoint-executor-session".into(),
            workspace.path().to_string_lossy().into_owned(),
            AgentIdentity::worker(worker_slug),
        );
        executor.set_task_id(Some(task.id));
        let result = executor
            .execute_batch(vec![(
                Arc::new(WriteFileTool),
                json!({"path":"src/nested/main.rs","content":"fn main() {}"}),
                "checkpoint-write-call".into(),
            )])
            .await
            .expect("execute production ToolExecutor batch");

        assert!(
            result[0].1.is_error,
            "failed task checkpoint must deny write"
        );
        assert!(
            result[0].1.output.contains("recovery checkpoint failed"),
            "ToolExecutor must carry the claimed task identity into the failed checkpoint path: {}",
            result[0].1.output
        );
        let destination = workspace.path().join("src/nested/main.rs");
        assert!(!destination.exists(), "destination must remain absent");
        assert!(
            !workspace.path().join("src").exists(),
            "parent directories must remain absent"
        );
    }
}
