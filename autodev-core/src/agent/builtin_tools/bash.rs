use async_trait::async_trait;
use serde_json::json;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use super::path_jail::validate_workspace_path;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize = 50_000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Execute
    }

    fn name(&self) -> &str {
        "bash"
    }

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


    async fn execute(&self, input: serde_json::Value, ctx: &ToolContext) -> ToolResult {
        let command = match input["command"].as_str() {
            Some(c) => c.to_string(),
            None => return ToolResult::error("Missing required field: command"),
        };
        let timeout_secs = input["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        // ⚠ G-4 (post-phase gap hunt). This block was commented as the
        // **workspace path-jail** — "the shell cwd MUST be inside the
        // configured workspace", guarding against a `ctx.working_dir` that had
        // "drifted away from the configured workspace".
        //
        // 🔴 It is not a jail and never was. Both arguments below are the SAME
        // VALUE, so the comparison is trivially satisfied; the only way it can
        // fail is if the path does not canonicalise. It is an **existence and
        // canonicalisation check** wearing a jail's description — and both
        // `check_redirect_targets`'s docstring and `builtin_tools/mod.rs`
        // deferred to it as the load-bearing defence.
        //
        // It cannot be made into a jail here: there is no separately
        // configured workspace root anywhere in `AppState` to compare against,
        // so the drift it claims to detect is undetectable from this position.
        // Introducing one is an architectural decision, flagged rather than
        // invented (`ADR-LOG-RESTRUCT-0029`).
        //
        // **What actually bounds `bash`:** it declares `ToolEffect::Execute`,
        // so it is prompted in every mode but `Autonomous` — consent, not
        // containment. `check_redirect_targets` is a secondary heuristic over
        // `>` / `>>` / `tee` tokens and does not see `copy`, `curl -o`, or
        // `python -c "open(...,'w')"`. A shell tool is bounded by the user
        // saying yes, and saying otherwise in a comment is how a reader comes
        // to believe otherwise.
        let cwd_path = PathBuf::from(&ctx.working_dir);
        if let Err(msg) = validate_workspace_path(&cwd_path, &ctx.working_dir) {
            // A working directory that cannot canonicalise is a configuration
            // failure — surface it rather than running blindly in an unknown
            // location.
            return ToolResult::error(msg);
        }

        // 2. Best-effort scan for `>` / `>>` / `tee` redirect targets that
        //    point outside the workspace. This is heuristic — shell
        //    redirection has many forms (here-docs, process substitution,
        //    eval, dynamic vars) we cannot statically extract. The cwd jail
        //    above is NOT a load-bearing check (G-4: it compares the working
        //    directory against itself). Consent is what bounds this tool;
        //    this scanner catches the easy cases on top of that.
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

        let mut shell_cmd = if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", &command]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", &command]);
            cmd
        };

        // ⚠ F-8, defence in depth — applied to BOTH shells, once.
        //
        // `main.rs` scrubs credentials from the process environment at startup,
        // and this strips them again on the way into the child. Two independent
        // guards, deliberately, because this tool runs commands the **model**
        // chose: a single missed variable here is a readable secret, not a
        // degraded feature, and `echo $ANTHROPIC_API_KEY` needs no
        // vulnerability beyond an inherited environment.
        //
        // The first version of this landed in the Windows branch only, which is
        // its own small lesson about two-branch fixes.
        for (key, _) in std::env::vars() {
            if crate::security::env_scrub::looks_like_a_credential(&key) {
                shell_cmd.env_remove(&key);
            }
        }

        let result = timeout(
            Duration::from_secs(timeout_secs),
            run_command(shell_cmd, &ctx.working_dir),
        )
        .await;

        match result {
            Err(_) => ToolResult::error(format!(
                "Command timed out after {timeout_secs}s: {command}"
            )),
            Ok(Err(e)) => ToolResult::error(format!("Failed to spawn command: {e}")),
            Ok(Ok((stdout, stderr, exit_code))) => {
                // Doc 09 T2.4 — record build-like invocations into the
                // module-level evidence tracker so update_task_status can
                // gate qc_review transitions. Side-effect only — no impact
                // on the tool's return value or output.
                super::build_evidence::record(&ctx.session_id, &command, exit_code);

                // K.1 — Label stdout and stderr separately so agents can
                // distinguish build warnings (stderr) from program output.
                let mut output = String::new();
                if !stdout.is_empty() {
                    output.push_str("=== STDOUT ===\n");
                    output.push_str(&stdout);
                }
                if !stderr.is_empty() {
                    if !output.is_empty() {
                        output.push('\n');
                    }
                    output.push_str("=== STDERR ===\n");
                    output.push_str(&stderr);
                }
                // Truncate very long output — keep the TAIL, not the head.
                // Build output starts with progress lines (noise) and ends with errors (signal).
                // Showing the tail means the agent sees the actual failure reason.
                if output.len() > MAX_OUTPUT_BYTES {
                    // ⚠ P3-04. `output[tail_start..]` panicked here, and this is
                    // the site most likely to fire: `tail_start` is an arbitrary
                    // byte offset, and rustc and cargo emit box-drawing
                    // characters (`┌ │ └`) in ordinary diagnostics — so any
                    // command that compiles something could produce a
                    // multi-byte character straddling the cut.
                    //
                    // The panic happened AT THE SLICE, before the newline search
                    // that follows it ever ran. That is why a fix templated on
                    // `&x[..N]` would have missed this site entirely: it slices
                    // a computed *start*, not a length.
                    let total = output.len();
                    let tail = crate::security::truncate::truncate_bytes_from_end(
                        &output,
                        MAX_OUTPUT_BYTES,
                    );
                    // Now trim to a line boundary *within* the safe tail, so a
                    // partial first line is dropped rather than shown.
                    let shown = match tail.find('\n') {
                        Some(i) => &tail[i + 1..],
                        None => tail,
                    };
                    output = format!(
                        "[⚠ Output truncated — showing last {} bytes of {} bytes total]\n{}",
                        shown.len(),
                        total,
                        shown
                    );
                }
                let is_error = exit_code != 0;
                if is_error && output.is_empty() {
                    output = format!("Command exited with code {exit_code}");
                }
                // Fix 10c: structured suggestion on exit 255 + "not recognized" (Windows)
                if is_error
                    && exit_code == 255
                    && cfg!(target_os = "windows")
                    && output.contains("is not recognized")
                {
                    let suggestion = format!(
                        "{output}\n\n[SUGGESTION] This command uses a Unix utility not available on Windows. \
                        Try rewriting it with PowerShell-compatible syntax (e.g. `Select-Object -First N` instead of `head -n`, \
                        `Out-File` instead of `>` redirection in pipes, and avoid `grep`, `jq`, `tail` in piped commands)."
                    );
                    return ToolResult::error(suggestion);
                }
                if is_error {
                    ToolResult::error(output)
                } else {
                    ToolResult::success(output)
                }
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
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);
    Ok((stdout, stderr, exit_code))
}

/// Best-effort scan for `>`, `>>`, `tee` redirect targets, plus heredoc tails
/// of the form `<< EOF ... > /etc/foo` that resolve outside the workspace.
/// Heuristic only: a misbehaving model could still escape via variable
/// expansion (`> "$VAR"`), process substitution (`> >(cmd)`), or `eval`, so
/// ⚠ G-4: this docstring said "the cwd jail in `execute` is the load-bearing
/// check". There is no cwd jail — that call compares `ctx.working_dir` with
/// itself. What bounds `bash` is the permission prompt its `Execute` effect
/// forces; this scanner is a heuristic on top, and it does not see `copy`,
/// `curl -o`, or `python -c "open(...,'w')"`.
///
/// Bug C — the detector now also:
///   * skips bytes inside an identified heredoc body (so a `>` *inside* a
///     heredoc doesn't trigger a false positive on harmless text), and
///   * still scans the segment AFTER a heredoc's closing delimiter, because
///     a chained `cat << EOF ... EOF > /etc/passwd` is the actual attack
///     surface.
///   * refuses any redirect target that contains a bash variable expansion
///     (`$VAR`, `${...}`, `~`) — without expanding the variable we cannot
///     prove the target lands inside the workspace, so we err on the side
///     of refusal with a clear message telling the agent to pass an
///     already-expanded literal path.
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
            if j < bytes.len() && bytes[j] == b'>' {
                j += 1;
            }
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            (j, "redirect")
        } else if i + 3 <= bytes.len()
            && &bytes[i..i + 3] == b"tee"
            && (i == 0
                || matches!(
                    bytes[i.saturating_sub(1)],
                    b' ' | b'\t' | b'|' | b'&' | b';'
                ))
            && (i + 3 == bytes.len() || matches!(bytes[i + 3], b' ' | b'\t'))
        {
            // `tee` token — skip token + flags + whitespace to first non-flag arg.
            let mut j = i + 3;
            // Skip leading spaces and flags (`-a`, `--append`, etc.) until we
            // find a non-flag argument.
            loop {
                while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
                if j >= bytes.len() {
                    break;
                }
                if bytes[j] == b'-' {
                    // Skip the flag token.
                    while j < bytes.len() && bytes[j] != b' ' && bytes[j] != b'\t' {
                        j += 1;
                    }
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
            && !matches!(
                bytes[end],
                b' ' | b'\t' | b'\n' | b'|' | b'&' | b';' | b'<' | b'>'
            )
        {
            end += 1;
        }
        if end > target_start {
            let raw = &command[target_start..end];
            // Strip surrounding quotes if present.
            let stripped = raw.trim_matches('"').trim_matches('\'');

            // Bug C — refuse variable-expanded or tilde-expanded targets that
            // we cannot statically resolve. The cwd jail in `execute()` is
            // the load-bearing check, but for explicit redirect tokens we
            // prefer to err on the side of refusal so the agent retries with
            // a literal path.
            if stripped.contains('$') || stripped.starts_with('~') {
                return Some(format!(
                    "Refused: bash {target_kind} target '{stripped}' contains a shell \
                     variable or tilde expansion. AutoDev cannot statically prove \
                     this resolves inside the workspace '{workspace}'. Pass an \
                     already-expanded literal path instead. The command was not \
                     executed."
                ));
            }

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

    // Bug C — heredoc tail scan. After scanning the standard tokens above,
    // walk the command looking for `<<` / `<<-` markers, find each heredoc's
    // delimiter, skip its body, and re-scan the trailing tokens for redirects.
    // We don't try to enforce balanced delimiters or nested heredocs — those
    // are exotic enough that the cwd jail remains the load-bearing defence.
    if let Some(after_heredoc) = strip_heredoc_bodies(command) {
        if after_heredoc != command {
            if let Some(err) = check_redirect_targets(&after_heredoc, workspace) {
                return Some(err);
            }
        }
    }

    None
}

/// Best-effort: return the command with heredoc bodies removed so the outer
/// scanner can match `> /etc/passwd` AFTER a heredoc closes. Returns None
/// when no heredoc marker is present. This is a textual approximation; a
/// proper shell parser would handle quoted delimiters, indented `<<-`
/// tab-stripping, and nested heredocs.
fn strip_heredoc_bodies(command: &str) -> Option<String> {
    let bytes = command.as_bytes();
    let mut out = String::with_capacity(command.len());
    let mut i = 0;
    let mut found = false;
    while i < bytes.len() {
        // Look for `<<` not preceded by another `<`.
        if i + 2 <= bytes.len() && &bytes[i..i + 2] == b"<<" && (i == 0 || bytes[i - 1] != b'<') {
            found = true;
            // Skip the `<<` (and an optional `-` for `<<-`)
            let mut j = i + 2;
            if j < bytes.len() && bytes[j] == b'-' {
                j += 1;
            }
            // Skip whitespace before the delimiter token.
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            // Capture delimiter (strip optional quotes).
            let delim_start = j;
            while j < bytes.len()
                && !matches!(
                    bytes[j],
                    b' ' | b'\t' | b'\n' | b'|' | b'&' | b';' | b'<' | b'>'
                )
            {
                j += 1;
            }
            let delim_raw = &command[delim_start..j];
            let delim = delim_raw.trim_matches('"').trim_matches('\'').to_string();
            if delim.is_empty() {
                // Malformed; bail out, return whole command minus the marker
                // so the outer scanner still gets a shot at later tokens.
                // P3-04. `i` comes from a byte-wise scan, so it is not
                // guaranteed to sit on a character boundary — a heredoc
                // delimiter containing any non-ASCII byte reaches here with a
                // mid-character index.
                out.push_str(crate::security::truncate::truncate_bytes_from_end(
                    &command[..],
                    command.len().saturating_sub(i),
                ));
                break;
            }
            // Find the closing delimiter on its own line.
            let mut body_end = j;
            let tail = &command[j..];
            // Walk line by line until we find a line containing only `delim`.
            let mut search_pos = 0usize;
            while search_pos < tail.len() {
                let line_end = tail[search_pos..]
                    .find('\n')
                    .map(|p| search_pos + p)
                    .unwrap_or(tail.len());
                let line = tail[search_pos..line_end].trim();
                if line == delim {
                    body_end = j + line_end;
                    break;
                }
                if line_end == tail.len() {
                    body_end = j + tail.len();
                    break;
                }
                search_pos = line_end + 1;
            }
            // Skip everything up to and including the closing delim line.
            i = body_end;
            // Consume the newline after the closing delim if present.
            if i < bytes.len() && bytes[i] == b'\n' {
                i += 1;
            }
            // Insert a space so the outer scanner doesn't glue tokens.
            out.push(' ');
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    if found {
        Some(out)
    } else {
        None
    }
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
    let quoted_cd = format!("cd \"{}\" && ", working_dir);
    let unquoted_cd = format!("cd {} && ", working_dir);
    let set_loc = format!("Set-Location \"{}\" ; ", working_dir);
    let set_loc_sl = format!("Set-Location {} ; ", working_dir);
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
