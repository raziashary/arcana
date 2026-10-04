// append_file.rs — Appends content to a file without reading/rewriting the whole file.
// Prevents accidental overwrites during requirements.txt, package.json, or log updates.
// Available in all non-delegate PA modes and to MCP workers.

use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, SeekFrom};

use super::path_jail::validate_workspace_path;
use super::path_lock;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

async fn append_all<W>(writer: &mut W, bytes: &[u8]) -> Result<(), ToolResult>
where
    W: AsyncWrite + Unpin,
{
    writer.write_all(bytes).await.map_err(|_| {
        ToolResult::categorized_error(
            "append_outcome_unknown",
            "side_effect_outcome_unknown",
            "The append write failed after it started; some or all bytes may have been written.",
            "Reconcile the file contents against the task checkpoint before retrying this append.",
        )
        .with_unknown_side_effect_outcome()
    })
}

pub struct AppendFileTool;

#[async_trait]
impl Tool for AppendFileTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Write
    }

    fn name(&self) -> &str {
        "append_file"
    }

    fn description(&self) -> &str {
        "Append content to the end of a file. Creates the file if it does not exist. \
         Safer than read+write for adding lines to requirements.txt, package.json, logs, etc. \
         Use newline=true (default) to ensure content starts on its own line."
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type":        "string",
                    "description": "File path to append to. Absolute or relative to working directory."
                },
                "content": {
                    "type":        "string",
                    "description": "Content to append to the file."
                },
                "newline": {
                    "type":        "boolean",
                    "description": "If true (default), prepend a newline before content when the file already exists and does not end with a newline."
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let path_str = match input["path"].as_str() {
            Some(p) if !p.trim().is_empty() => p,
            _ => return ToolResult::error("Missing required parameter 'path'."),
        };

        let content = match input["content"].as_str() {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter 'content'."),
        };

        let newline = input["newline"].as_bool().unwrap_or(true);

        // Resolve path against working directory
        let raw = if Path::new(path_str).is_absolute() {
            Path::new(path_str).to_path_buf()
        } else {
            Path::new(&ctx.working_dir).join(path_str)
        };

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        // Same rationale as write_file: refuse appends whose canonical
        // destination escapes the configured workspace.
        let resolved = match validate_workspace_path(&raw, &ctx.working_dir) {
            Ok(canonical) => canonical,
            Err(msg) => return ToolResult::error(msg),
        };

        // Cross-tool, cross-worker per-path lock. Same key/registry used by
        // write_file and edit_file — a concurrent overwrite or edit cannot
        // truncate the file mid-append, and a concurrent append from another
        // worker is strictly ordered after ours rather than racing the
        // trailing-newline probe.
        let _lock_guard = path_lock::acquire_for(&resolved, &ctx.working_dir).await;

        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        if let Err(e) = crate::git::checkpoint::auto_checkpoint_before_write(
            &ctx.session_id,
            ctx.task_id,
            &resolved.to_string_lossy(),
        ) {
            return ToolResult::error(format!(
                "Append refused because its recovery checkpoint failed: {e}"
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return ToolResult::error(format!(
                    "Cannot create parent directory for '{}': {e}",
                    resolved.display()
                ));
            }
        }

        // Open once in read + append mode. Doing the trailing-newline probe
        // through the same file handle narrows the previous probe-then-open
        // race window (where another writer could extend the file between the
        // standalone `metadata` call and our `OpenOptions::open`). The window
        // is not fully closed — another concurrent appender can still extend
        // the file between `seek/read` and our `write_all` — but the worst
        // observable outcome is a duplicated or missing newline, which is
        // cosmetic. Documented limitation.
        let mut file = match OpenOptions::new()
            .read(true)
            .create(true)
            .append(true)
            .open(&resolved)
            .await
        {
            Ok(f) => f,
            Err(e) => {
                return ToolResult::error(format!(
                    "Cannot open '{}' for append: {e}",
                    resolved.display()
                ))
            }
        };

        // Probe last byte through the open handle.
        let needs_leading_nl = if newline {
            match file.metadata().await {
                Ok(meta) if meta.len() > 0 => {
                    let mut buf = [0u8; 1];
                    let seek_ok = file.seek(SeekFrom::End(-1)).await.is_ok();
                    let read_ok = seek_ok && file.read_exact(&mut buf).await.is_ok();
                    read_ok && buf[0] != b'\n'
                }
                _ => false,
            }
        } else {
            false
        };

        // Append mode auto-seeks to end on each write, so the read seek above
        // does not affect the write position.
        let to_write = if needs_leading_nl {
            format!("\n{}", content)
        } else {
            content.to_string()
        };
        let bytes_to_append = to_write.len();

        if let Err(result) = append_all(&mut file, to_write.as_bytes()).await {
            return result;
        }

        ToolResult::success(format!(
            "Appended {} bytes to '{}'",
            bytes_to_append,
            resolved.display()
        ))
    }
}

#[cfg(test)]
mod checkpoint_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::AsyncWrite;

    struct PartialThenFailWriter {
        written: Vec<u8>,
    }

    impl AsyncWrite for PartialThenFailWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buffer: &[u8],
        ) -> Poll<io::Result<usize>> {
            if self.written.is_empty() {
                let count = buffer.len().min(2);
                self.written.extend_from_slice(&buffer[..count]);
                Poll::Ready(Ok(count))
            } else {
                Poll::Ready(Err(io::Error::other("injected partial write failure")))
            }
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn partial_append_failure_is_marked_as_unknown_side_effect() {
        let mut writer = PartialThenFailWriter {
            written: Vec::new(),
        };
        let result = append_all(&mut writer, b"append payload")
            .await
            .expect_err("injected writer must fail after its first partial write");

        assert_eq!(writer.written, b"ap");
        let error: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(error["error"]["code"], "append_outcome_unknown");
        assert_eq!(error["error"]["outcome_unknown"], true);
        assert_eq!(error["error"]["retry_safe"], false);
        assert!(error["error"]["next_step"]
            .as_str()
            .unwrap()
            .contains("Reconcile the file contents"));
    }

    #[tokio::test]
    async fn append_uses_task_checkpoint_and_restores_prior_absence() {
        let home = tempfile::tempdir().expect("isolated Arcana data root");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("workspace");
        let db = crate::db::tasks::TasksDb::open().expect("open tasks db");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "append checkpoint task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "append is task-attributed".into(),
                created_by: Some("append-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        let context = ToolContext {
            session_id: "append-checkpoint-test".into(),
            working_dir: workspace.path().to_string_lossy().into_owned(),
            call_id: "append-checkpoint-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = AppendFileTool
            .execute(
                json!({"path":"new/file.txt","content":"task output"}),
                &context,
            )
            .await;
        assert!(!result.is_error, "append succeeded: {}", result.output);

        let file = workspace.path().join("new/file.txt");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "task output");
        let checkpoints =
            crate::git::checkpoint::list(None, None, Some(task.id)).expect("read task checkpoint");
        assert_eq!(checkpoints.len(), 1);
        crate::git::checkpoint::restore(&checkpoints[0].id).expect("restore file's prior absence");
        assert!(!file.exists());
    }
}
