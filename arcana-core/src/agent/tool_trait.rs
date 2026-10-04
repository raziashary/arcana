// The Tool trait — uniform interface for all built-in tools.
// Mirrors Claude Code's Tool interface and Goose's Tool trait.
// Each tool declares: name, description, JSON schema, read-only flag, execute().

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
    /// Optional guidance appended to the tool result content before sending to the LLM.
    /// Use this for large/noisy results to direct the agent on how to consume them,
    /// e.g. "Large result — extract only what you need before proceeding."
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl ToolResult {
    const DEFAULT_ERROR_NEXT_STEP: &'static str = "Use the cause to correct the request or check the required permission; do not repeat side-effecting actions without reconciling their result.";

    pub fn success(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            hint: None,
        }
    }

    /// Like `success` but attaches a hint string that is appended inline to the
    /// tool result sent to the LLM. Use for large outputs to guide summarisation.
    pub fn success_with_hint(output: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            is_error: false,
            hint: Some(hint.into()),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self::error_with_next_step(message, Self::DEFAULT_ERROR_NEXT_STEP)
    }

    /// Return a bounded structured error while preserving the caller's
    /// actionable recovery guidance outside the truncated cause field.
    pub fn error_with_next_step(message: impl Into<String>, next_step: impl Into<String>) -> Self {
        let message = message.into();
        let sanitize = |value: &str| {
            crate::security::redaction::redact_text(value)
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .take(512)
                .collect::<String>()
        };
        let cause = sanitize(&message);
        let next_step = sanitize(&next_step.into());
        let output = serde_json::json!({
            "schema_version": 1,
            "ok": false,
            "error": {
                "code": "tool_operation_failed",
                "message": cause.clone(),
                "cause": cause,
                "retry_safe": false,
                "next_step": next_step
            }
        })
        .to_string();
        Self {
            output,
            is_error: true,
            hint: None,
        }
    }

    pub fn categorized_error(
        code: &str,
        category: &str,
        message: impl Into<String>,
        next_step: impl Into<String>,
    ) -> Self {
        let mut result = Self::error_with_next_step(message, next_step);
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&result.output) {
            if let Some(error) = value.get_mut("error") {
                error["code"] = serde_json::json!(code);
                error["category"] = serde_json::json!(category);
            }
            result.output = value.to_string();
        }
        result
    }

    /// Attach source-free structured diagnostics to a command failure. The
    /// executor preserves only the bounded diagnostic fields understood by the
    /// shared failure contract.
    pub fn with_diagnostics(mut self, diagnostics: serde_json::Value) -> Self {
        if !self.is_error {
            return self;
        }
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&self.output) {
            if let Some(error) = value.get_mut("error") {
                error["diagnostics"] = diagnostics;
            }
            self.output = value.to_string();
        }
        self
    }

    /// Mark that an error was observed but the side effect may have completed.
    /// Run journals use this to preserve an unresolved outcome instead of
    /// recording a false, definite failure.
    pub fn with_unknown_side_effect_outcome(mut self) -> Self {
        if !self.is_error {
            return self;
        }
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&self.output) {
            if let Some(error) = value.get_mut("error") {
                error["outcome_unknown"] = serde_json::Value::Bool(true);
            }
            self.output = value.to_string();
        }
        self
    }

    /// Return an actionable, bounded denial when Arcana's OS sandbox refuses
    /// to establish or exercise a declared process capability. Command identity
    /// is supplied by the Rust-owned intent resolver; callers must not pass
    /// model-authored shell text or infer a blocked path from child output.
    pub fn sandbox_violation(
        capability: &str,
        command_identity: &str,
        retry_safe: bool,
        suggested_action: &str,
    ) -> Self {
        let sanitize = |value: &str| {
            crate::security::redaction::redact_text(value)
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .take(512)
                .collect::<String>()
        };
        let capability = sanitize(capability);
        let command_identity = sanitize(command_identity);
        let suggested_action = sanitize(suggested_action);
        let output = serde_json::json!({
            "schema_version": 1,
            "ok": false,
            "error": {
                "code": "sandbox_violation",
                "kind": "sandbox_violation",
                "capability": capability,
                "policy": "deny",
                "command": command_identity,
                "retry_safe": retry_safe,
                "suggested_action": suggested_action
            }
        })
        .to_string();
        Self {
            output,
            is_error: true,
            hint: None,
        }
    }

    /// Attach executor-owned identity and correlation metadata to every
    /// failure before it reaches the model, persistence, or UI. Tool handlers
    /// may provide a bounded JSON error with a stable code; plain-text legacy
    /// failures are wrapped here without trusting handler-authored IDs.
    pub fn with_execution_context(
        self,
        tool_name: &str,
        session_id: &str,
        agent_role: &str,
        task_id: Option<i64>,
        call_id: &str,
        run_id: Option<&str>,
    ) -> Self {
        self.with_execution_context_and_input(
            tool_name, session_id, agent_role, task_id, call_id, run_id, None,
        )
    }

    /// Attach executor-owned failure metadata while removing exact values from
    /// the untrusted tool arguments. Handlers often include paths or rejected
    /// values in their prose; those can be source text, prompts, or commands.
    pub fn with_execution_context_and_input(
        mut self,
        tool_name: &str,
        session_id: &str,
        agent_role: &str,
        task_id: Option<i64>,
        call_id: &str,
        run_id: Option<&str>,
        raw_input: Option<&serde_json::Value>,
    ) -> Self {
        if !self.is_error {
            return self;
        }

        let sanitize = |value: &str, limit: usize| {
            crate::security::redaction::redact_text(value)
                .chars()
                .map(|character| {
                    if character.is_control() {
                        ' '
                    } else {
                        character
                    }
                })
                .take(limit)
                .collect::<String>()
        };
        let parsed = serde_json::from_str::<serde_json::Value>(&self.output).ok();
        let existing_error = parsed
            .as_ref()
            .and_then(|value| value.get("error"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let code = existing_error
            .get("code")
            .and_then(serde_json::Value::as_str)
            .map(|value| sanitize(value, 80))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "tool_operation_failed".to_string());
        let cause = existing_error
            .get("cause")
            .or_else(|| existing_error.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(|value| sanitize_tool_error(value, 512, raw_input))
            .unwrap_or_else(|| sanitize_tool_error(&self.output, 512, raw_input));
        let retry_safe = existing_error
            .get("retry_safe")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let category = existing_error
            .get("category")
            .and_then(serde_json::Value::as_str)
            .map(|value| sanitize(value, 64))
            .unwrap_or_else(|| match code.as_str() {
                "sandbox_violation" => "sandbox".to_string(),
                "permission_denied" | "permission_unavailable" => "permission".to_string(),
                "operation_cancelled" => "cancellation".to_string(),
                "operation_timed_out" => "timeout".to_string(),
                _ => "tool_execution".to_string(),
            });
        let suggested_action = ["suggested_action", "next_step"]
            .into_iter()
            .find_map(|field| {
                existing_error
                    .get(field)
                    .and_then(serde_json::Value::as_str)
                    .map(|value| sanitize_tool_error(value, 512, raw_input))
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| Self::DEFAULT_ERROR_NEXT_STEP.to_string());
        let safe_run_id = run_id.map(|value| sanitize(value, 128));
        let safe_agent_id = sanitize(agent_role, 80);

        let mut error = serde_json::json!({
            "code": code,
            "error_code": code,
            "category": category,
            "operation": format!("tool.{}", sanitize(tool_name, 80)),
            "tool": sanitize(tool_name, 80),
            "message": cause,
            "cause": cause,
            "retryable": retry_safe,
            "retry_safe": retry_safe,
            "retry_after_ms": null,
            "suggested_action": suggested_action,
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "context": {
                "session_id": sanitize(session_id, 128),
                "agent_id": safe_agent_id,
                "task_id": task_id,
                "tool_call_id": sanitize(call_id, 128),
                "run_id": safe_run_id
            }
        });

        error["run_id"] = safe_run_id
            .clone()
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        error["agent_id"] = serde_json::Value::String(safe_agent_id);
        error["task_id"] = task_id
            .map(serde_json::Value::from)
            .unwrap_or(serde_json::Value::Null);

        for field in [
            "kind",
            "next_step",
            "capability",
            "policy",
            "command",
            "path",
        ] {
            if let Some(value) = existing_error.get(field) {
                if value.is_string() {
                    let value = value.as_str().unwrap_or_default();
                    error[field] = serde_json::Value::String(if field == "command" {
                        // Command identity is created by Arcana's intent
                        // resolver (for example `project_command install`),
                        // not copied from the model's shell/argument text.
                        sanitize(value, 512)
                    } else {
                        sanitize_tool_error(value, 512, raw_input)
                    });
                }
            }
        }
        if let Some(diagnostics) = existing_error
            .get("diagnostics")
            .and_then(serde_json::Value::as_array)
        {
            let bounded = diagnostics
                .iter()
                .take(24)
                .filter_map(|diagnostic| {
                    let code = diagnostic
                        .get("code")
                        .and_then(serde_json::Value::as_str)
                        .map(|value| sanitize(value, 32))
                        .filter(|value| !value.is_empty())?;
                    let severity = diagnostic
                        .get("severity")
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| matches!(*value, "error" | "warning" | "note"))
                        .unwrap_or("error");
                    let mut safe = serde_json::json!({"code": code, "severity": severity});
                    if let Some(path) = diagnostic
                        .get("file")
                        .and_then(serde_json::Value::as_str)
                        .map(|value| sanitize_tool_error(value, 160, raw_input))
                        .filter(|value| !value.is_empty())
                    {
                        safe["file"] = serde_json::json!(path);
                    }
                    for field in ["line", "column"] {
                        if let Some(value) =
                            diagnostic.get(field).and_then(serde_json::Value::as_u64)
                        {
                            safe[field] = serde_json::json!(value.min(1_000_000));
                        }
                    }
                    Some(safe)
                })
                .collect::<Vec<_>>();
            error["diagnostics"] = serde_json::Value::Array(bounded);
        }
        if let Some(retry_after_ms) = existing_error
            .get("retry_after_ms")
            .and_then(serde_json::Value::as_u64)
        {
            error["retry_after_ms"] = serde_json::json!(retry_after_ms.min(86_400_000));
        }

        self.output = serde_json::json!({
            "schema_version": 1,
            "ok": false,
            "error_code": error["error_code"].clone(),
            "category": error["category"].clone(),
            "message": error["message"].clone(),
            "operation": error["operation"].clone(),
            "tool": error["tool"].clone(),
            "retryable": error["retryable"].clone(),
            "retry_after_ms": error["retry_after_ms"].clone(),
            "cause": error["cause"].clone(),
            "suggested_action": error["suggested_action"].clone(),
            "context": error["context"].clone(),
            "run_id": error["run_id"].clone(),
            "agent_id": error["agent_id"].clone(),
            "task_id": error["task_id"].clone(),
            "timestamp": error["timestamp"].clone(),
            "error": error
        })
        .to_string();
        self
    }
}

fn sanitize_tool_error(value: &str, limit: usize, raw_input: Option<&serde_json::Value>) -> String {
    fn collect_strings(value: &serde_json::Value, output: &mut Vec<String>) {
        match value {
            serde_json::Value::String(value) if !value.is_empty() => output.push(value.clone()),
            serde_json::Value::Array(values) => {
                for value in values {
                    collect_strings(value, output);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    collect_strings(value, output);
                }
            }
            _ => {}
        }
    }

    let mut sanitized = crate::security::redaction::redact_text(value);
    if let Some(raw_input) = raw_input {
        let mut arguments = Vec::new();
        collect_strings(raw_input, &mut arguments);
        arguments.sort_by_key(|argument| std::cmp::Reverse(argument.len()));
        arguments.dedup();
        for argument in arguments {
            let matches = if argument.chars().count() < 12 {
                sanitized == argument
            } else {
                sanitized.contains(&argument)
            };
            if matches {
                sanitized = sanitized.replace(&argument, "[argument omitted]");
            }
        }
    }
    sanitized
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tool_result_error_tests {
    use super::ToolResult;
    use serde_json::Value;

    #[test]
    fn bounded_error_keeps_actionable_next_step_separate_from_long_cause() {
        let result = ToolResult::error_with_next_step(
            format!("{} compiler output", "x".repeat(900)),
            "Diagnose the captured build output and rerun verification with project_command.",
        );
        let value: Value = serde_json::from_str(&result.output).expect("valid structured error");
        assert_eq!(
            value["error"]["cause"].as_str().unwrap().chars().count(),
            512
        );
        assert!(value["error"]["next_step"]
            .as_str()
            .unwrap()
            .contains("Diagnose the captured build output"));
    }

    #[test]
    fn sandbox_violation_is_structured_and_does_not_claim_an_unobserved_path() {
        let result = ToolResult::sandbox_violation(
            "network",
            "project_command cargo test",
            false,
            "Use the explicit host-controlled dependency installation flow; do not retry this child with network access.",
        );
        let value: Value = serde_json::from_str(&result.output).expect("valid sandbox error");
        assert!(result.is_error);
        assert_eq!(value["error"]["code"], "sandbox_violation");
        assert_eq!(value["error"]["capability"], "network");
        assert_eq!(value["error"]["policy"], "deny");
        assert_eq!(value["error"]["command"], "project_command cargo test");
        assert_eq!(value["error"]["retry_safe"], false);
        assert!(value["error"]["suggested_action"]
            .as_str()
            .unwrap()
            .contains("host-controlled"));
        assert!(value["error"].get("path").is_none());
    }

    #[test]
    fn executor_context_enriches_failures_without_changing_successes() {
        let sandbox_action = "Request the required capability through the host.";
        let result = ToolResult::sandbox_violation(
            "network",
            "project_command cargo test",
            true,
            sandbox_action,
        )
        .with_execution_context(
            "project_command",
            "session-1",
            "worker-rust",
            Some(42),
            "call-7",
            Some("run-9"),
        );
        let value: Value = serde_json::from_str(&result.output).expect("valid error envelope");
        assert!(result.is_error);
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["error"]["category"], "sandbox");
        assert_eq!(value["error"]["operation"], "tool.project_command");
        assert_eq!(value["error"]["tool"], "project_command");
        assert_eq!(value["error"]["context"]["session_id"], "session-1");
        assert_eq!(value["error"]["context"]["agent_id"], "worker-rust");
        assert_eq!(value["error"]["context"]["task_id"], 42);
        assert_eq!(value["error"]["context"]["tool_call_id"], "call-7");
        assert_eq!(value["error"]["context"]["run_id"], "run-9");
        assert_eq!(value["error"]["retryable"], true);
        assert_eq!(value["error"]["capability"], "network");
        assert_eq!(value["error"]["suggested_action"], sandbox_action);
        assert!(value["error"]["timestamp"].as_str().is_some());

        let success = ToolResult::success("done").with_execution_context(
            "project_command",
            "session-1",
            "worker-rust",
            Some(42),
            "call-7",
            None,
        );
        assert_eq!(success.output, "done");
        assert!(!success.is_error);
    }

    #[test]
    fn executor_context_promotes_generic_next_step_to_suggested_action() {
        let action = "Inspect the current task state before retrying.";
        let result = ToolResult::error_with_next_step("The database operation failed.", action)
            .with_execution_context(
                "list_tasks",
                "session-1",
                "worker-rust",
                None,
                "call-2",
                None,
            );
        let value: Value = serde_json::from_str(&result.output).expect("valid error envelope");

        assert!(result.is_error);
        assert_eq!(value["error"]["next_step"], action);
        assert_eq!(value["error"]["suggested_action"], action);
    }

    #[test]
    fn executor_context_uses_next_step_when_handler_suggestion_is_empty() {
        let action = "Check the current task state before retrying.";
        let mut result = ToolResult::error_with_next_step("The database operation failed.", action);
        let mut envelope: Value = serde_json::from_str(&result.output).expect("valid error");
        envelope["error"]["suggested_action"] = Value::String(String::new());
        result.output = envelope.to_string();

        let result = result.with_execution_context(
            "list_tasks",
            "session-1",
            "worker-rust",
            None,
            "call-4",
            None,
        );
        let value: Value = serde_json::from_str(&result.output).expect("valid error envelope");

        assert!(result.is_error);
        assert_eq!(value["error"]["suggested_action"], action);
    }

    #[test]
    fn executor_context_adds_default_guidance_to_legacy_plain_text_failures() {
        let result = ToolResult {
            output: "The handler failed.".to_string(),
            is_error: true,
            hint: None,
        }
        .with_execution_context(
            "legacy_tool",
            "session-1",
            "worker-rust",
            None,
            "call-3",
            None,
        );
        let value: Value = serde_json::from_str(&result.output).expect("valid error envelope");

        assert!(result.is_error);
        assert_eq!(value["error"]["cause"], "The handler failed.");
        assert_eq!(
            value["error"]["suggested_action"],
            ToolResult::DEFAULT_ERROR_NEXT_STEP
        );
    }

    #[test]
    fn executor_context_preserves_only_bounded_source_free_diagnostic_fields() {
        let result = ToolResult::categorized_error(
            "project_command_failed",
            "command",
            "Project verification failed.",
            "Inspect the structured diagnostic location.",
        )
        .with_diagnostics(serde_json::json!([{
            "code": "E0425",
            "severity": "error",
            "file": "src/main.rs",
            "line": 8,
            "column": 3,
            "message": "must not be forwarded"
        }]))
        .with_execution_context(
            "project_command",
            "session",
            "worker-backend",
            Some(12),
            "call",
            Some("run"),
        );
        let value: Value = serde_json::from_str(&result.output).unwrap();

        assert!(result.is_error);
        assert_eq!(value["error"]["diagnostics"][0]["code"], "E0425");
        assert_eq!(value["error"]["diagnostics"][0]["file"], "src/main.rs");
        assert_eq!(value["error"]["diagnostics"][0]["line"], 8);
        assert!(value["error"]["diagnostics"][0].get("message").is_none());
    }

    #[test]
    fn executor_context_removes_echoed_command_prompt_source_and_path_arguments() {
        let command = "cargo test --message planted-command-argument";
        let prompt = "planted-private-prompt-value";
        let source = "fn planted_secret_source() { private_value(); }";
        let path = "C:\\Users\\Razi\\private\\workspace\\src\\secret.rs";
        let input = serde_json::json!({
            "command": command,
            "prompt": prompt,
            "content": source,
            "path": path,
        });
        let result = ToolResult::categorized_error(
            "handler_failed",
            "tool_execution",
            format!("Rejected `{command}` for {prompt}; source={source}; path={path}"),
            format!("Do not retry `{command}` with {prompt}; inspect {source} at {path}."),
        )
        .with_execution_context_and_input(
            "fixture_tool",
            "session-safe",
            "worker-backend",
            Some(12),
            "call-safe",
            Some("run-safe"),
            Some(&input),
        );

        let envelope: Value = serde_json::from_str(&result.output).expect("valid JSON envelope");
        assert!(result.is_error);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["error"]["code"], "handler_failed");
        assert_eq!(envelope["error"]["error_code"], "handler_failed");
        assert_eq!(envelope["error"]["retry_after_ms"], Value::Null);
        assert_eq!(envelope["error"]["run_id"], "run-safe");
        assert_eq!(envelope["error"]["agent_id"], "worker-backend");
        assert_eq!(envelope["error"]["task_id"], 12);
        assert_eq!(envelope["error_code"], "handler_failed");
        assert_eq!(envelope["run_id"], "run-safe");
        assert_eq!(envelope["agent_id"], "worker-backend");
        assert_eq!(envelope["task_id"], 12);
        assert_eq!(envelope["retry_after_ms"], Value::Null);
        for planted in [command, prompt, source, path] {
            assert!(
                !result.output.contains(planted),
                "tool argument leaked into failure JSON: {planted}"
            );
        }
        assert!(envelope["error"]["cause"]
            .as_str()
            .unwrap()
            .contains("[argument omitted]"));
        assert_eq!(envelope["error"]["context"]["tool_call_id"], "call-safe");
    }
}

/// Context passed to every tool execution — working directory, session, etc.
#[derive(Debug, Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub working_dir: String,
    pub call_id: String,
    /// The owning worker execution run, when this tool call belongs to one.
    pub run_id: Option<String>,
    /// One atomic token ledger shared by this run and every child agent.
    pub run_token_budget: Option<Arc<crate::budget::run_tokens::RunTokenLedger>>,
    /// The claimed task associated with this worker run, if any.
    pub task_id: Option<i64>,
    /// The permission mode this call is being made under.
    ///
    /// 🔴 `G-5` (post-phase gap hunt). Without it, the spawn tools could not
    /// tell a subagent what policy its parent was running under — and
    /// `native_runner` hardcoded `PermissionMode::Autonomous` regardless. So a
    /// model held in `ManualApproval` escaped every prompt with a single
    /// `spawn_subagent` call: the child ran unattended, with `bash`, on the
    /// user's own key.
    ///
    /// ⚠ The justification comment at that site read *"a subagent has no user
    /// to prompt for permission"* — and two lines below it, `deps.pending` was
    /// passed straight in. The permission channel was there all along.
    pub permission_mode: crate::agent::permission::PermissionMode,
    /// K.11 — Subagent nesting depth (0 = top-level agent, n+1 = subagent
    /// spawned by an agent at depth n). Spawn tools read this to guard against
    /// unbounded recursion.
    pub depth: u32,
    /// Who is making this call: "supervisor", "worker-backend", "qc-agent", …
    ///
    /// NOT an Option. Every agent has an identity, and the three worker↔worker
    /// messaging tools hard-refuse without one — so an absent identity was not
    /// a neutral default, it silently disabled capabilities.
    pub agent_role: String,
}

impl ToolContext {
    /// Refuse worker mutations unless this run still owns an in-progress task.
    /// Interactive Supervisor operations remain taskless by design.
    pub fn require_current_task_claim(&self) -> Result<(), String> {
        if self.agent_role.trim() == "supervisor" {
            return Ok(());
        }
        let Some(task_id) = self.task_id else {
            return Err("Worker mutations require a task claim; claim assigned work before editing or running commands".into());
        };
        let db = crate::db::tasks::TasksDb::open()
            .map_err(|_| "Arcana could not verify the current task claim".to_string())?;
        let task = db
            .get_by_id(task_id)
            .map_err(|_| "Arcana could not verify the current task claim".to_string())?
            .ok_or_else(|| "The claimed task no longer exists".to_string())?;
        if task.status.as_str() != "in_progress"
            || task.assigned_worker.as_deref() != Some(self.agent_role.trim())
        {
            return Err("This worker no longer owns the in-progress task; ask the Supervisor to reconcile or reassign it".into());
        }
        Ok(())
    }

    /// Atomically validate and hold a worker claim for a short synchronous
    /// host mutation. A supervisor operation has no task claim and receives no
    /// guard. The returned database connection retains `BEGIN IMMEDIATE`
    /// until the guard is dropped.
    pub(crate) fn lock_current_task_claim(&self) -> Result<Option<TaskClaimGuard>, String> {
        if self.agent_role.trim() == "supervisor" {
            return Ok(None);
        }
        let Some(task_id) = self.task_id else {
            return Err("Worker mutations require a task claim; claim assigned work before editing or running commands".into());
        };
        let db = crate::db::tasks::TasksDb::open()
            .map_err(|_| "Arcana could not verify the current task claim".to_string())?;
        match db.lock_current_worker_claim(task_id, self.agent_role.trim()) {
            Ok(true) => {}
            Ok(false) => {
                return Err("This worker no longer owns the in-progress task; ask the Supervisor to reconcile or reassign it".to_string());
            }
            Err(_) => {
                return Err("Arcana could not verify or lock the current task claim".to_string());
            }
        }
        Ok(Some(TaskClaimGuard { _db: db }))
    }
}

pub(crate) struct TaskClaimGuard {
    _db: crate::db::tasks::TasksDb,
}

/// Who an agent is, and how deeply nested it is.
///
/// Both fields are authored in Rust and threaded from the spawn site. They were
/// previously carried over the MCP boundary: `subagent/bridge.rs` injected
/// "Include `__depth: {n}` in every tool call" into the child's *prompt* and
/// `mcp_server/tool_adapter.rs` read it back off the arguments. That made the
/// recursion guard depend on the model choosing to report its own nesting depth
/// honestly, and the in-process path — which never went through the adapter —
/// hardcoded `depth: 0` and `agent_role: None` at all three construction sites.
///
/// See ADR-LOG-RESTRUCT-0001.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentIdentity {
    pub role: String,
    pub depth: u32,
}

impl AgentIdentity {
    /// The interactive assistant. "supervisor" matches the role the shipped
    /// recipes already use for it (`recipes/defaults.rs:217`).
    pub fn supervisor() -> Self {
        Self {
            role: "supervisor".to_string(),
            depth: 0,
        }
    }

    /// A top-level fleet worker, identified by its recipe slug.
    pub fn worker(slug: impl Into<String>) -> Self {
        Self {
            role: slug.into(),
            depth: 0,
        }
    }

    /// A subagent spawned by this agent — one level deeper, by construction.
    pub fn child(&self, role: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            depth: self.depth + 1,
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;

    /// What this tool does to the world — **the only prompting key** (`P3-11`).
    ///
    /// ⚠ **Deliberately has no default.** A new tool must state its effect or
    /// the crate does not compile.
    ///
    /// A permissive default is exactly how this codebase got here: prompting
    /// was keyed on `is_read_only()`, which defaults to `false`… and then two
    /// tools *overrode it to `true`* while uploading the user's repository to a
    /// third party and executing binaries found inside the workspace under
    /// inspection. A default that means "no prompt" is a default that silently
    /// grants permission, and nothing in review looks wrong.
    ///
    /// See `agent::tool_effect::ToolEffect`.
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect;

    /// Resolve a call-specific effect when a typed tool has intents with
    /// different consequences. The executor uses this for prompting,
    /// standing-allow policy, hooks, and scheduling. Most tools have one fixed
    /// effect and inherit this default.
    fn effect_for_input(
        &self,
        _input: &serde_json::Value,
    ) -> crate::agent::tool_effect::ToolEffect {
        self.effect()
    }

    /// Whether this tool may run **concurrently** with others.
    ///
    /// ⚠ This is a **concurrency hint for the path lock and nothing else**. It
    /// was written for that (`"Pattern from Claude Code's isConcurrencySafe()"`)
    /// and was then used as the auto-approval key, which made it a security
    /// control by accident — `P3-11`, `G-11`. It gates nothing now, and
    /// `TG-P3-11` asserts it stays that way.
    ///
    /// Derived from the effect by default, so the two cannot disagree: only a
    /// `Read` is concurrency-safe. A tool may still override where it knows
    /// better about its own contention.
    fn is_read_only(&self) -> bool {
        self.effect().is_concurrency_safe()
    }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult;
}

#[cfg(test)]
mod identity_tests {
    use super::AgentIdentity;

    #[test]
    fn top_level_agents_start_at_depth_zero() {
        assert_eq!(AgentIdentity::supervisor().depth, 0);
        assert_eq!(AgentIdentity::supervisor().role, "supervisor");
        assert_eq!(AgentIdentity::worker("worker-backend").depth, 0);
        assert_eq!(
            AgentIdentity::worker("worker-backend").role,
            "worker-backend"
        );
    }

    #[test]
    fn each_generation_is_exactly_one_level_deeper() {
        let parent = AgentIdentity::worker("worker-backend");
        let child = parent.child("subagent-a");
        let grandchild = child.child("subagent-b");

        assert_eq!(child.depth, 1);
        assert_eq!(grandchild.depth, 2);
        assert_eq!(grandchild.role, "subagent-b");
        // The parent is untouched — depth is not shared mutable state.
        assert_eq!(parent.depth, 0);
    }

    /// The regression this whole change exists for.
    ///
    /// All three `ToolContext` construction sites in `tool_executor.rs` used to
    /// hardcode `depth: 0` and `agent_role: None`. That silently disabled the
    /// three worker-messaging tools, made `claim_task` attribute work to
    /// "unknown", and left the subagent recursion guard unable to fire on the
    /// in-process path — while the only site that set a real value lived in the
    /// MCP adapter that P0 deletes.
    ///
    /// Reading the source is the only way to assert this: the values are struct
    /// literals, so there is no runtime surface to interrogate.
    #[test]
    fn no_tool_context_site_hardcodes_identity() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/agent/tool_executor.rs");
        // Line endings normalised: git's CRLF conversion silently changed the
        // meaning of three other gates that parse source (`L-C15`).
        let source = std::fs::read_to_string(&path)
            .expect("read tool_executor.rs")
            .replace(
                "
", "
",
            );

        let sites = source.matches("ToolContext {").count();

        // ⚠ P3-67. This asserted `sites >= 3`, which encoded how many
        // construction sites the executor happened to have — so deleting the
        // dead `execute()` path in `P3-10` failed it **by succeeding**.
        //
        // The invariant is *every site takes its depth from the threaded
        // identity*, asserted below. The floor exists only so the gate cannot
        // pass by scanning a file with no sites at all. (`L-C6`.)
        assert!(
            sites >= 1,
            "found no ToolContext construction sites — this gate is scanning the wrong              file, so its green result means nothing"
        );

        // NOT `!contains("agent_role: None")`: `agent_role` is a `String` now, so
        // that assertion is type-impossible and can never fail — it looks like a
        // guard and is worth nothing. Found by the P0 post-phase audit (F-09).
        //
        // The property that must hold is positive: EVERY construction site takes
        // both fields from the threaded identity.
        //
        // 🔴 **This counted `identity.role` across the WHOLE FILE and
        // asserted the total equalled the number of construction sites.** That
        // is a proxy, and it held only while the executor mentioned the
        // identity nowhere else. `P8-66` added event payloads that take
        // `agent_id` from the same threaded identity — doing exactly what this
        // gate exists to encourage — and the count went from 2 to 10, so the
        // gate went red for **compliance**.
        //
        // ⚠ The gate's own comment already records `P3-67` fixing the previous
        // version, which encoded how many sites the file happened to have. Same
        // class, one level up: `L-C6`, a check written from a countable stand-in
        // rather than from the invariant. It now reads each construction block.
        let mut checked = 0usize;
        for seg in source.split("ToolContext {").skip(1) {
            let block = seg.split('}').next().unwrap_or_default();
            assert!(
                block.contains("identity.role"),
                "a ToolContext site does not take agent_role from the threaded \
                 AgentIdentity, which silently disables the ACP tools and the \
                 qc_review gate for that path:\n{block}"
            );
            assert!(
                block.contains("identity.depth"),
                "a ToolContext site does not take depth from the threaded \
                 AgentIdentity, so the subagent recursion guard cannot fire on \
                 that path:\n{block}"
            );
            checked += 1;
        }
        assert_eq!(
            checked, sites,
            "{checked} of {sites} ToolContext construction blocks were examined              — the split is not finding them all, so some site went unchecked"
        );

        // A literal depth defeats the guard whatever value it is given.
        for forbidden in ["depth: 0", "depth: 1"] {
            assert!(
                !source.contains(forbidden),
                "a ToolContext is built with a literal `{forbidden}` instead of the \
                 threaded depth"
            );
        }
    }

    #[test]
    fn tool_errors_are_bounded_structured_and_redacted_json() {
        let result = super::ToolResult::error(format!(
            "provider failed api_key=AIzaSyD{secret}",
            secret = "x".repeat(800)
        ));
        let value: serde_json::Value =
            serde_json::from_str(&result.output).expect("tool error is JSON");
        assert!(result.is_error);
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "tool_operation_failed");
        assert_eq!(value["error"]["retry_safe"], false);
        assert!(value["error"]["next_step"]
            .as_str()
            .unwrap()
            .contains("permission"));
        assert!(!result.output.contains("AIzaSyD"));
        assert!(result.output.len() < 1000);
    }
}
