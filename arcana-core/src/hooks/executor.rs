// HookExecutor — runs a hook action with variable substitution in the context.
// Supports: shell, http, notification, log, block, emit_event.
// script and modify are stubbed (implemented in later phases).
// Phase 3.1

use std::time::Instant;
use tracing::{debug, warn};

use super::types::{HookAction, HookConfig, HookContext, HookExecutionLog, HookOutcome};

pub struct HookExecutor;

impl HookExecutor {
    /// Execute a hook and return a log entry describing the outcome.
    pub async fn execute(hook: &HookConfig, ctx: &HookContext) -> HookExecutionLog {
        let start = Instant::now();
        let (outcome, output, error) = Self::run_action(&hook.action, ctx).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        debug!(
            "[HookExecutor] hook='{}' type={} outcome={:?} duration={}ms",
            hook.name,
            hook.hook_type.as_str(),
            outcome,
            duration_ms
        );

        HookExecutionLog {
            id: 0,
            hook_id: hook.id.clone(),
            hook_name: hook.name.clone(),
            hook_type: hook.hook_type.as_str().to_string(),
            outcome,
            output,
            error,
            executed_at: crate::db::timestamps::now_rfc3339(),
            duration_ms,
        }
    }

    async fn run_action(
        action: &HookAction,
        ctx: &HookContext,
    ) -> (HookOutcome, Option<String>, Option<String>) {
        match action {
            HookAction::Block { message } => {
                let msg = message.as_deref().unwrap_or("Action blocked by hook");
                (HookOutcome::Blocked, Some(msg.to_string()), None)
            }

            HookAction::Shell {
                command,
                working_dir,
                timeout_secs,
            } => {
                let expanded_working_dir = working_dir.as_ref().map(|dir| substitute(dir, ctx));
                if let Some(working_dir) = &expanded_working_dir {
                    if let Err(error) =
                        crate::platform::shell_portability::validate_working_directory(working_dir)
                    {
                        return (
                            HookOutcome::Failed,
                            None,
                            Some(format!("Hook working directory refused: {error}")),
                        );
                    }
                }
                let cmd = substitute(command, ctx);
                if let Err(error) = crate::platform::shell_portability::validate_shell_command(&cmd)
                {
                    return (
                        HookOutcome::Failed,
                        None,
                        Some(format!("Expanded hook command refused: {error}")),
                    );
                }
                let timeout = u64::from(timeout_secs.unwrap_or(30).clamp(1, 1800));
                match run_sandboxed_shell_hook(&cmd, expanded_working_dir.as_deref(), ctx, timeout)
                    .await
                {
                    Ok((0, stdout, stderr)) => (
                        HookOutcome::Executed,
                        Some(stdout),
                        if stderr.is_empty() {
                            None
                        } else {
                            Some(stderr)
                        },
                    ),
                    Ok((_, stdout, stderr)) => (
                        HookOutcome::Failed,
                        Some(stdout),
                        Some(stderr_or_exit(stderr)),
                    ),
                    Err(error) => (HookOutcome::Failed, None, Some(error)),
                }
            }

            HookAction::Http {
                url,
                method,
                headers,
                body,
            } => {
                let url = substitute(url, ctx);
                let meth = method.to_uppercase();

                let client = reqwest::Client::new();
                let mut req = match meth.as_str() {
                    "GET" => client.get(&url),
                    "POST" => client.post(&url),
                    "PUT" => client.put(&url),
                    "DELETE" => client.delete(&url),
                    _ => client.post(&url),
                };

                if let Some(hdrs) = headers {
                    if let Some(obj) = hdrs.as_object() {
                        for (k, v) in obj {
                            if let Some(vs) = v.as_str() {
                                req = req.header(k.as_str(), vs);
                            }
                        }
                    }
                }

                if let Some(b) = body {
                    req = req.json(b);
                }

                match req.send().await {
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        let text = resp.text().await.unwrap_or_default();
                        if status < 400 {
                            (HookOutcome::Executed, Some(text), None)
                        } else {
                            (
                                HookOutcome::Failed,
                                None,
                                Some(format!("HTTP {status}: {text}")),
                            )
                        }
                    }
                    Err(e) => (HookOutcome::Failed, None, Some(e.to_string())),
                }
            }

            HookAction::Log { file, message } => {
                let msg = substitute(message, ctx);
                let path = std::path::Path::new(file);
                let line = format!("[{}] {}\n", crate::db::timestamps::now_rfc3339(), msg);
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    Ok(mut f) => {
                        use std::io::Write;
                        let _ = f.write_all(line.as_bytes());
                        (HookOutcome::Executed, Some("logged".into()), None)
                    }
                    Err(e) => (HookOutcome::Failed, None, Some(e.to_string())),
                }
            }

            HookAction::Notification { title, message } => {
                let title = substitute(title, ctx);
                let msg = substitute(message, ctx);
                // Platform notification — best effort, no hard failure
                warn!(
                    "[HookExecutor] Notification (desktop notifications not yet integrated): {title} — {msg}"
                );
                (HookOutcome::Executed, Some(format!("{title}: {msg}")), None)
            }

            HookAction::EmitEvent {
                event_name,
                payload,
            } => {
                // Tauri event emission requires AppHandle; stubbed until wired in later phase
                let name = substitute(event_name, ctx);
                debug!("[HookExecutor] EmitEvent '{name}' (wiring pending)");
                let _ = payload;
                (HookOutcome::Executed, Some(format!("event:{name}")), None)
            }

            HookAction::Script { path, args } => {
                warn!("[HookExecutor] Script hooks not yet implemented — skipping");
                let _ = (path, args);
                (
                    HookOutcome::Skipped,
                    Some("script hooks pending".into()),
                    None,
                )
            }

            HookAction::Modify { transform } => {
                warn!("[HookExecutor] Modify hooks not yet implemented — skipping");
                let _ = transform;
                (
                    HookOutcome::Skipped,
                    Some("modify hooks pending".into()),
                    None,
                )
            }
        }
    }
}

#[cfg(windows)]
const MAX_HOOK_OUTPUT_BYTES: usize = 50_000;
#[cfg(windows)]
const HOOK_OUTPUT_READ_BYTES: usize = 8 * 1024;
async fn run_sandboxed_shell_hook(
    command: &str,
    action_working_dir: Option<&str>,
    ctx: &HookContext,
    timeout_secs: u64,
) -> Result<(i32, String, String), String> {
    let explicit_workspace = ctx
        .extra
        .get("working_dir")
        .and_then(serde_json::Value::as_str)
        .filter(|path| !path.trim().is_empty());
    let workspace = crate::platform::workspace::resolve_workspace_dir(explicit_workspace)
        .map_err(|error| format!("Shell hook selected workspace is unavailable: {error}"))?;

    let cwd = match action_working_dir {
        Some(path) => crate::platform::workspace::resolve_workspace_dir(Some(path))
            .map_err(|error| format!("Shell hook working directory is unavailable: {error}"))?,
        None => workspace.clone(),
    };
    if !cwd.starts_with(&workspace) {
        return Err("Shell hook working directory must stay inside the selected workspace.".into());
    }

    #[cfg(not(windows))]
    {
        let _ = (command, cwd, timeout_secs);
        return Err(
            "Shell hooks are unavailable on this platform because Arcana cannot provide the approved sandbox read/network policy and confirmed process-tree cleanup."
                .into(),
        );
    }

    #[cfg(windows)]
    {
        use std::ffi::OsString;

        use crate::platform::process_sandbox::ProcessSpec;

        let env_remove = std::env::vars()
            .filter_map(|(key, _)| {
                crate::security::env_scrub::looks_like_a_credential(&key)
                    .then(|| OsString::from(key))
            })
            .collect();
        let spec = ProcessSpec {
            program: OsString::from("cmd"),
            args: vec![OsString::from("/C"), OsString::from(command)],
            raw_arg: None,
            cwd,
            env_remove,
            env_set: Vec::new(),
            read_only_paths: Vec::new(),
        };

        match read_sandboxed_shell(spec, &workspace, timeout_secs).await {
            Ok(result) => Ok(result),
            Err(error) => Err(error.detail),
        }
    }
}

#[cfg(windows)]
async fn read_sandboxed_shell(
    spec: crate::platform::process_sandbox::ProcessSpec,
    workspace: &std::path::Path,
    timeout_secs: u64,
) -> Result<(i32, String, String), ShellHookRunError> {
    use std::time::Duration;

    let mut child = crate::platform::process_sandbox::spawn_for_shell_hook(spec, workspace)
        .await
        .map_err(|error| {
            ShellHookRunError::releasing(format!("process admission failed: {error}"))
        })?;
    let mut stdout = match child.take_stdout() {
        Some(stdout) => stdout,
        None => {
            return Err(stop_failed_hook_child(
                child,
                "sandboxed hook stdout was unavailable".into(),
            )
            .await);
        }
    };
    let mut stderr = match child.take_stderr() {
        Some(stderr) => stderr,
        None => {
            return Err(stop_failed_hook_child(
                child,
                "sandboxed hook stderr was unavailable".into(),
            )
            .await);
        }
    };
    let mut stdout_reader =
        tokio::spawn(async move { read_bounded_hook_output(&mut stdout).await });
    let mut stderr_reader =
        tokio::spawn(async move { read_bounded_hook_output(&mut stderr).await });

    let run = async {
        let exit_code = child
            .wait()
            .await
            .map_err(|error| format!("sandboxed shell wait failed: {error}"))?;
        let stdout = (&mut stdout_reader)
            .await
            .map_err(|error| format!("sandboxed hook stdout reader failed: {error}"))?
            .map_err(|error| format!("sandboxed hook stdout read failed: {error}"))?;
        let stderr = (&mut stderr_reader)
            .await
            .map_err(|error| format!("sandboxed hook stderr reader failed: {error}"))?
            .map_err(|error| format!("sandboxed hook stderr read failed: {error}"))?;
        Ok::<_, String>((exit_code, stdout, stderr))
    };

    match tokio::time::timeout(Duration::from_secs(timeout_secs), run).await {
        Ok(Ok(result)) => {
            if let Err(error) = child.terminate_and_confirm().await {
                return Err(ShellHookRunError {
                    detail: format!(
                        "Shell hook completed, but the sandbox could not confirm its process tree stopped ({error}); the workspace remains quarantined while cleanup retries."
                    ),
                });
            }
            Ok(result)
        }
        Ok(Err(detail)) => {
            abort_output_readers(stdout_reader, stderr_reader).await;
            Err(stop_failed_hook_child(child, detail).await)
        }
        Err(_) => {
            abort_output_readers(stdout_reader, stderr_reader).await;
            Err(stop_failed_hook_child(
                child,
                format!("Shell hook timed out after {timeout_secs} seconds; workspace effects may be partial."),
            )
            .await)
        }
    }
}

#[cfg(windows)]
struct ShellHookRunError {
    detail: String,
}

#[cfg(windows)]
impl ShellHookRunError {
    fn releasing(detail: String) -> Self {
        Self {
            detail: format!("Sandboxed shell hook failed: {detail}"),
        }
    }
}

#[cfg(windows)]
async fn stop_failed_hook_child(
    mut child: crate::platform::process_sandbox::HookSandboxChild,
    detail: String,
) -> ShellHookRunError {
    match child.terminate_and_confirm().await {
        Ok(()) => ShellHookRunError::releasing(detail),
        Err(error) => {
            // Dropping the owner transfers child, lease, and admission to the
            // cleanup worker; no new managed writer can enter this workspace.
            drop(child);
            ShellHookRunError {
                detail: format!(
                    "{detail}; process-tree cleanup is unconfirmed ({error}); the workspace is quarantined while cleanup retries."
                ),
            }
        }
    }
}

#[cfg(windows)]
async fn abort_output_readers(
    stdout_reader: tokio::task::JoinHandle<std::io::Result<String>>,
    stderr_reader: tokio::task::JoinHandle<std::io::Result<String>>,
) {
    stdout_reader.abort();
    stderr_reader.abort();
    let _ = stdout_reader.await;
    let _ = stderr_reader.await;
}

#[cfg(windows)]
async fn read_bounded_hook_output(
    reader: &mut (dyn tokio::io::AsyncRead + Unpin + Send),
) -> std::io::Result<String> {
    use tokio::io::AsyncReadExt;

    let mut output = Vec::with_capacity(MAX_HOOK_OUTPUT_BYTES);
    let mut buffer = [0u8; HOOK_OUTPUT_READ_BYTES];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        output.extend_from_slice(&buffer[..read]);
        if output.len() > MAX_HOOK_OUTPUT_BYTES {
            let excess = output.len() - MAX_HOOK_OUTPUT_BYTES;
            output.drain(..excess);
            truncated = true;
        }
    }

    let text = String::from_utf8_lossy(&output).into_owned();
    Ok(if truncated {
        format!("[hook output truncated; retaining final {MAX_HOOK_OUTPUT_BYTES} bytes]\n{text}")
    } else {
        text
    })
}

fn stderr_or_exit(stderr: String) -> String {
    if stderr.is_empty() {
        "Shell hook exited unsuccessfully.".into()
    } else {
        stderr
    }
}

/// Replace `{{field}}` placeholders in a string from the context's JSON representation.
fn substitute(template: &str, ctx: &HookContext) -> String {
    let json = serde_json::to_value(ctx).unwrap_or_default();
    let mut result = template.to_string();

    if let Some(obj) = json.as_object() {
        for (k, v) in obj {
            let placeholder = format!("{{{{{k}}}}}");
            let replacement = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }

    // Also expose tool_call sub-fields
    if let Some(tc) = &ctx.tool_call {
        result = result.replace("{{tool_name}}", &tc.name);
        result = result.replace("{{tool_id}}", &tc.id);
    }

    result
}

#[cfg(test)]
mod portability_tests {
    use super::*;
    use crate::hooks::types::{HookType, ToolCallContext};

    #[tokio::test]
    async fn expanded_saved_hook_command_is_refused_before_shell_process_starts() {
        let temp = tempfile::tempdir().unwrap();
        let sentinel = temp.path().join("hook-ran.txt");
        let hook = HookConfig {
            id: "legacy-hook".into(),
            name: "Legacy hook".into(),
            hook_type: HookType::PreToolUse,
            enabled: true,
            priority: 100,
            conditions: Default::default(),
            action: HookAction::Shell {
                command: "echo {{session_id}}".into(),
                working_dir: None,
                timeout_secs: Some(2),
            },
            created_at: String::new(),
            updated_at: String::new(),
        };
        let context = HookContext {
            hook_type: "pre_tool_use".into(),
            timestamp: chrono::Utc::now(),
            session_id: Some(format!(
                "ok && echo should-not-run > {}",
                sentinel.display()
            )),
            agent_mode: None,
            tool_call: Some(ToolCallContext {
                id: "test".into(),
                name: "test".into(),
                input: serde_json::Value::Null,
                result: None,
                is_error: false,
            }),
            file_path: None,
            task_id: None,
            worker_id: None,
            extra: serde_json::Value::Null,
        };
        let log = HookExecutor::execute(&hook, &context).await;
        assert_eq!(log.outcome, HookOutcome::Failed);
        assert!(log.error.unwrap().contains("command refused"));
        assert!(
            !sentinel.exists(),
            "refused hook must not spawn its command"
        );
    }
}

#[cfg(test)]
mod shell_boundary_tests {
    use super::*;
    use crate::hooks::types::{HookType, ToolCallContext};
    use serde_json::json;

    fn hook(command: String, working_dir: Option<String>, timeout_secs: Option<u32>) -> HookConfig {
        HookConfig {
            id: "shell-boundary-test".into(),
            name: "Shell boundary test".into(),
            hook_type: HookType::PostFileWrite,
            enabled: true,
            priority: 100,
            conditions: Default::default(),
            action: HookAction::Shell {
                command,
                working_dir,
                timeout_secs,
            },
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn context(workspace: Option<&std::path::Path>) -> HookContext {
        HookContext {
            hook_type: HookType::PostFileWrite.as_str().into(),
            timestamp: chrono::Utc::now(),
            session_id: Some("shell-boundary-test".into()),
            agent_mode: None,
            tool_call: Some(ToolCallContext {
                id: "shell-boundary-test".into(),
                name: "write_file".into(),
                input: serde_json::Value::Null,
                result: None,
                is_error: false,
            }),
            file_path: None,
            task_id: None,
            worker_id: None,
            extra: workspace.map_or(
                serde_json::Value::Null,
                |path| json!({"working_dir": path.to_string_lossy()}),
            ),
        }
    }

    #[tokio::test]
    async fn shell_hook_requires_selected_workspace_before_spawn() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let sentinel = home.path().join("shell-hook-ran.txt");
        let log = HookExecutor::execute(
            &hook(format!("echo ran > {}", sentinel.display()), None, Some(1)),
            &context(None),
        )
        .await;

        assert_eq!(log.outcome, HookOutcome::Failed);
        assert!(log
            .error
            .as_deref()
            .is_some_and(|error| error.contains("selected workspace is unavailable")));
        assert!(
            !sentinel.exists(),
            "a missing workspace must not spawn a shell"
        );
    }

    #[tokio::test]
    async fn shell_hook_rejects_action_directory_outside_selected_workspace() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        let outside = home.path().join("outside");
        std::fs::create_dir(&workspace).expect("selected workspace");
        std::fs::create_dir(&outside).expect("outside directory");
        let sentinel = outside.join("shell-hook-ran.txt");
        let log = HookExecutor::execute(
            &hook(
                format!("echo ran > {}", sentinel.display()),
                Some(outside.to_string_lossy().into_owned()),
                Some(1),
            ),
            &context(Some(&workspace)),
        )
        .await;

        assert_eq!(log.outcome, HookOutcome::Failed);
        assert!(log
            .error
            .as_deref()
            .is_some_and(|error| { error.contains("must stay inside the selected workspace") }));
        assert!(
            !sentinel.exists(),
            "an escaped working directory must not spawn a shell"
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn shell_hook_runs_only_inside_the_selected_sandboxed_workspace() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir(&workspace).expect("selected workspace");
        let sentinel = workspace.join("shell-hook-ran.txt");
        let log = HookExecutor::execute(
            &hook("echo ran > shell-hook-ran.txt".into(), None, Some(10)),
            &context(Some(&workspace)),
        )
        .await;

        assert_eq!(log.outcome, HookOutcome::Executed, "{:?}", log.error);
        assert_eq!(std::fs::read_to_string(&sentinel).unwrap().trim(), "ran");
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn shell_hook_fails_closed_when_platform_policy_is_unavailable() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir(&workspace).expect("selected workspace");
        let sentinel = workspace.join("shell-hook-ran.txt");
        let log = HookExecutor::execute(
            &hook(format!("echo ran > {}", sentinel.display()), None, Some(1)),
            &context(Some(&workspace)),
        )
        .await;

        assert_eq!(log.outcome, HookOutcome::Failed);
        assert!(log
            .error
            .as_deref()
            .is_some_and(|error| { error.contains("unavailable on this platform") }));
        assert!(
            !sentinel.exists(),
            "unsupported platforms must not spawn a shell"
        );
    }
}
