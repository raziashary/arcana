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
                let timeout = timeout_secs.unwrap_or(30);

                let mut builder =
                    tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" });
                if cfg!(windows) {
                    builder.args(["/C", &cmd]);
                } else {
                    builder.args(["-c", &cmd]);
                }
                if let Some(wd) = expanded_working_dir {
                    builder.current_dir(wd);
                }

                match tokio::time::timeout(
                    std::time::Duration::from_secs(timeout as u64),
                    builder.output(),
                )
                .await
                {
                    Ok(Ok(out)) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        if out.status.success() {
                            (
                                HookOutcome::Executed,
                                Some(stdout),
                                if stderr.is_empty() {
                                    None
                                } else {
                                    Some(stderr)
                                },
                            )
                        } else {
                            (HookOutcome::Failed, Some(stdout), Some(stderr))
                        }
                    }
                    Ok(Err(e)) => (HookOutcome::Failed, None, Some(e.to_string())),
                    Err(_) => (
                        HookOutcome::Failed,
                        None,
                        Some(format!("Timed out after {timeout}s")),
                    ),
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
                warn!("[HookExecutor] Notification (desktop notifications not yet integrated): {title} — {msg}");
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
