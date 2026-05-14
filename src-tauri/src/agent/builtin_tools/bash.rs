use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use super::path_jail::validate_workspace_path;

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize   = 50_000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        #[cfg(target_os = "windows")]
        return "Execute a shell command in the working directory. Returns stdout and stderr. Use for running tests, linting, builds, and other shell operations.\n\nIMPORTANT: Do NOT prefix commands with `cd <path> &&` or `Set-Location <path>;` — the working directory is already set to the project root. Write commands directly, e.g. `npm install` not `cd C:\\path && npm install`.\n\nPlatform note: The shell is Windows CMD. Avoid Unix-only utilities (`head`, `tail`, `jq`, `grep` in pipes). Use PowerShell-compatible alternatives or run commands separately.";
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

        // Workspace path-jail (Phase 1 of docs/endtoendrun/14may2026/07).
        //
        // 1. The shell cwd MUST be inside the configured workspace. Without
        //    this check, a Mode 2 worker whose `ctx.working_dir` somehow
        //    drifted away from the configured workspace would run shell
        //    commands against the wrong tree.
        let cwd_path = PathBuf::from(&ctx.working_dir);
        if let Err(msg) = validate_workspace_path(&cwd_path, &ctx.working_dir) {
            // validate_workspace_path returns an error if the path doesn't
            // canonicalize. A workspace that can't canonicalize is itself a
            // configuration failure — surface to the agent so it asks the
            // user/supervisor rather than running blindly.
            return ToolResult::error(msg);
        }

        // 2. Best-effort scan for `>` / `>>` / `tee` redirect targets that
        //    point outside the workspace. This is heuristic — shell
        //    redirection has many forms (here-docs, process substitution,
        //    eval, dynamic vars) we cannot statically extract. The cwd jail
        //    above is the load-bearing check; this catches the easy cases.
        if let Some(err) = check_redirect_targets(&command, &ctx.working_dir) {
            return ToolResult::error(err);
        }

        let command = if cfg!(target_os = "windows") {
            let rewritten = rewrite_for_windows(&command, &ctx.working_dir);
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
                // K.1 — Label stdout and stderr separately so agents can
                // distinguish build warnings (stderr) from program output.
                let mut output = String::new();
                if !stdout.is_empty() {
                    output.push_str("=== STDOUT ===\n");
                    output.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !output.is_empty() { output.push('\n'); }
                    output.push_str("=== STDERR ===\n");
                    output.push_str(&stderr);
                }
                // Truncate very long output — keep the TAIL, not the head.
                // Build output starts with progress lines (noise) and ends with errors (signal).
                // Showing the tail means the agent sees the actual failure reason.
                if output.len() > MAX_OUTPUT_BYTES {
                    let tail_start = output.len() - MAX_OUTPUT_BYTES;
                    // Walk forward to a newline boundary to avoid cutting mid-line.
                    let boundary = output[tail_start..].find('\n')
                        .map(|i| tail_start + i + 1)
                        .unwrap_or(tail_start);
                    output = format!(
                        "[⚠ Output truncated — showing last {} bytes of {} bytes total]\n{}",
                        output.len() - boundary,
                        output.len(),
                        &output[boundary..]
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

    // K.2 — Pre-check existence so agents get a self-diagnosing error
    // instead of an opaque "Failed to spawn command" from the OS.
    if !normalized_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!(
                "working_dir does not exist: {}. \
                 Use list_directory to find the actual project root.",
                normalized_path.display()
            ),
        ));
    }

    cmd.current_dir(&normalized_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().await?;
    let stdout   = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr   = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((stdout, stderr, exit_code))
}

/// Best-effort scan for `>`, `>>`, and `tee` redirect targets that resolve
/// outside the workspace. Heuristic only: a misbehaving model could still
/// escape via variable expansion, here-docs, or `eval`, so the cwd jail in
/// `execute` is the load-bearing check. This catches the obvious form
/// `something > /etc/passwd` or `tee C:\Windows\foo.txt`.
fn check_redirect_targets(command: &str, workspace: &str) -> Option<String> {
    use std::path::Path;
    // Tokenize on simple whitespace. We DON'T try to parse shell quoting —
    // if the model passes a quoted path with spaces we'll see it as one
    // token after the `>` and validate it as-is.
    let bytes = command.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Look for `>`, `>>`, or `tee` token starts.
        let (target_start, target_kind) = if bytes[i] == b'>' {
            // `>` or `>>` — skip the marker and any whitespace.
            let mut j = i + 1;
            if j < bytes.len() && bytes[j] == b'>' { j += 1; }
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') { j += 1; }
            (j, "redirect")
        } else if i + 3 <= bytes.len()
            && &bytes[i..i + 3] == b"tee"
            && (i == 0 || matches!(bytes[i.saturating_sub(1)], b' ' | b'\t' | b'|' | b'&' | b';'))
            && (i + 3 == bytes.len() || matches!(bytes[i + 3], b' ' | b'\t'))
        {
            // `tee` token — skip token + flags + whitespace to first non-flag arg.
            let mut j = i + 3;
            // Skip leading spaces and flags (`-a`, `--append`, etc.) until we
            // find a non-flag argument.
            loop {
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') { j += 1; }
                if j >= bytes.len() { break; }
                if bytes[j] == b'-' {
                    // Skip the flag token.
                    while j < bytes.len() && bytes[j] != b' ' && bytes[j] != b'\t' { j += 1; }
                    continue;
                }
                break;
            }
            (j, "tee")
        } else {
            i += 1;
            continue;
        };

        // Extract target token: up to next whitespace / shell metachar.
        let mut end = target_start;
        while end < bytes.len()
            && !matches!(bytes[end], b' ' | b'\t' | b'\n' | b'|' | b'&' | b';' | b'<' | b'>')
        {
            end += 1;
        }
        if end > target_start {
            let raw = &command[target_start..end];
            // Strip surrounding quotes if present.
            let stripped = raw.trim_matches('"').trim_matches('\'');
            // Skip non-path targets like file descriptors `&1`, `&2`.
            if !stripped.is_empty() && !stripped.starts_with('&') && !stripped.starts_with('/') {
                // Resolve relative redirect targets against workspace.
                let candidate = Path::new(stripped);
                let resolved = if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    Path::new(workspace).join(stripped)
                };
                if let Err(_msg) = validate_workspace_path(&resolved, workspace) {
                    return Some(format!(
                        "Refused: bash {target_kind} target '{stripped}' resolves outside the workspace '{workspace}'. \
                         Use a workspace-relative path. The command was not executed."
                    ));
                }
            } else if stripped.starts_with('/') {
                // Absolute Unix-style path — also validate.
                let resolved = Path::new(stripped).to_path_buf();
                if let Err(_msg) = validate_workspace_path(&resolved, workspace) {
                    return Some(format!(
                        "Refused: bash {target_kind} target '{stripped}' is outside the workspace '{workspace}'. \
                         Use a workspace-relative path. The command was not executed."
                    ));
                }
            }
        }
        i = end.max(i + 1);
    }
    None
}

/// Fix 10b: Unix → PowerShell command pre-processor for Windows.
///
/// Detects common Unix-only utilities in piped commands and rewrites them
/// to Windows equivalents before spawning the process. Applied only on
/// `cfg!(target_os = "windows")`.
///
/// Also strips any leading `cd "<working_dir>" &&` or `Set-Location "<working_dir>"; `
/// prefix that agents generate unnecessarily — the OS-level `current_dir` is already
/// set to `working_dir` by `run_command`, so these redundant prefixes only cause
/// Windows path-with-spaces failures.
fn rewrite_for_windows(cmd: &str, working_dir: &str) -> String {
    let mut out = cmd.to_string();

    // Strip redundant `cd "<working_dir>" &&` or `cd <working_dir> &&` prefix.
    // Agents frequently generate these even though current_dir is already set.
    let quoted_cd   = format!("cd \"{}\" && ", working_dir);
    let unquoted_cd = format!("cd {} && ", working_dir);
    let set_loc     = format!("Set-Location \"{}\" ; ", working_dir);
    let set_loc_sl  = format!("Set-Location {} ; ", working_dir);
    if out.starts_with(&quoted_cd) {
        out = out[quoted_cd.len()..].to_string();
    } else if out.starts_with(&unquoted_cd) {
        out = out[unquoted_cd.len()..].to_string();
    } else if out.starts_with(&set_loc) {
        out = out[set_loc.len()..].to_string();
    } else if out.starts_with(&set_loc_sl) {
        out = out[set_loc_sl.len()..].to_string();
    }

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
