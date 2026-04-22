use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize   = 50_000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        #[cfg(target_os = "windows")]
        return "Execute a shell command in the working directory. Returns stdout and stderr. Use for running tests, linting, builds, and other shell operations.\n\nPlatform note: The shell is Windows CMD. Avoid Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes). Use PowerShell-compatible alternatives or run commands separately.";
        #[cfg(not(target_os = "windows"))]
        return "Execute a shell command in the working directory. Returns stdout and stderr. Use for running tests, linting, builds, and other shell operations.";
    }

    fn input_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type":        "string",
                    "description": "Shell command to execute."
                },
                "timeout_secs": {
                    "type":        "integer",
                    "description": "Optional: max execution time in seconds. Default: 120."
                }
            },
            "required": ["command"]
        })
    }

    fn is_read_only(&self) -> bool { false }
    fn requires_permission(&self) -> bool { true }

    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let command = match input["command"].as_str() {
            Some(c) => c.to_string(),
            None    => return ToolResult::error("Missing required field: command"),
        };
        let timeout_secs = input["timeout_secs"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        let command = if cfg!(target_os = "windows") {
            let rewritten = rewrite_for_windows(&command);
            if rewritten != command {
                tracing::debug!("[BashTool] rewritten command: {} -> {}", command, rewritten);
            }
            rewritten
        } else {
            command
        };

        let shell_cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &command]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &command]);
            cmd
        };

        let result = timeout(
            Duration::from_secs(timeout_secs),
            run_command(shell_cmd, &ctx.working_dir),
        ).await;

        match result {
            Err(_) => ToolResult::error(format!(
                "Command timed out after {timeout_secs}s: {command}"
            )),
            Ok(Err(e)) => ToolResult::error(format!("Failed to spawn command: {e}")),
            Ok(Ok((stdout, stderr, exit_code))) => {
                let mut output = String::new();
                if !stdout.is_empty() {
                    output.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !output.is_empty() { output.push('\n'); }
                    output.push_str(&stderr);
                }
                // Truncate very long output
                if output.len() > MAX_OUTPUT_BYTES {
                    output = format!(
                        "{}...\n[Output truncated — {} bytes total]",
                        &output[..MAX_OUTPUT_BYTES],
                        output.len()
                    );
                }
                let is_error = exit_code != 0;
                if is_error && output.is_empty() {
                    output = format!("Command exited with code {exit_code}");
                }
                // Fix 10c: structured suggestion on exit 255 + "not recognized" (Windows)
                if is_error && exit_code == 255 && cfg!(target_os = "windows") && output.contains("is not recognized") {
                    let suggestion = format!(
                        "{output}\n\n[SUGGESTION] This command uses a Unix utility not available on Windows. \
                        Try rewriting it with PowerShell-compatible syntax (e.g. `Select-Object -First N` instead of `head -n`, \
                        `Out-File` instead of `>` redirection in pipes, and avoid `grep`, `jq`, `tail` in piped commands)."
                    );
                    return ToolResult::error(suggestion);
                }
                if is_error { ToolResult::error(output) } else { ToolResult::success(output) }
            }
        }
    }
}

async fn run_command(
    mut cmd: Command,
    working_dir: &str,
) -> std::io::Result<(String, String, i32)> {
    // Normalize the path to handle spaces and special characters on Windows
    let normalized_path = PathBuf::from(working_dir);

    cmd.current_dir(&normalized_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await?;
    let stdout   = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr   = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((stdout, stderr, exit_code))
}

/// Fix 10b: Unix → PowerShell command pre-processor for Windows.
///
/// Detects common Unix-only utilities in piped commands and rewrites them
/// to Windows equivalents before spawning the process. Applied only on
/// `cfg!(target_os = "windows")`.
fn rewrite_for_windows(cmd: &str) -> String {
    let mut out = cmd.to_string();
    // `head -N` → `Select-Object -First N` (PowerShell)
    // Handle both `head -20` and `head -n 20` forms.
    out = out.replace(" | head -n ", " | Select-Object -First ");
    out = out.replace(" | head -", " | Select-Object -First ");
    // `tail -N` → `Select-Object -Last N`
    out = out.replace(" | tail -n ", " | Select-Object -Last ");
    out = out.replace(" | tail -", " | Select-Object -Last ");
    // `grep PATTERN` in a pipe → `Select-String -Pattern PATTERN`
    // This is a best-effort rewrite; complex grep flags are not handled.
    out = out.replace(" | grep ", " | Select-String ");
    out = out.replace(" | grep -i ", " | Select-String -Pattern ");
    // `jq '.'` → no direct equivalent; leave as-is so it fails with exit 255
    // and the structured error handler (Fix 10c) guides the LLM to self-correct.
    out
}
