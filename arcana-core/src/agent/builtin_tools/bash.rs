use async_trait::async_trait;
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

use super::path_jail::validate_workspace_path;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::platform::process_sandbox::{self, ProcessSpec};

/// The floor and ceiling for a model-supplied `timeout_secs` (`T-6`).
///
/// A floor because `0` made every command report a timeout it never ran
/// long enough to earn; a ceiling because an unbounded value silently
/// removes the guard `T-5` added.
const MIN_TIMEOUT_SECS: u64 = 1;
const MAX_TIMEOUT_SECS: u64 = 1800;

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_BYTES: usize = 50_000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::FreeformShell
    }

    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        #[cfg(target_os = "windows")]
        return "Execute a shell command in the working directory. Returns stdout and stderr. Use for running tests, linting, builds, and other shell operations.\n\nIMPORTANT: Do NOT prefix commands with `cd <path> &&` or `Set-Location <path>;` — the working directory is already set to the project root. Write commands directly, e.g. `npm install` not `cd C:\\path && npm install`.\n\nPlatform note: The shell is Windows CMD (`cmd /C`), NOT PowerShell. PowerShell cmdlets such as `Select-Object` and `Select-String` do NOT exist here and will fail.\n\nIf a pipe fails with 'is not recognized', run the command WITHOUT the pipe — this tool already returns the full stdout and stderr, so `| head` and `| tail` are rarely needed. Where you must filter, `findstr` and `more` are the cmd-native equivalents of `grep` and a pager. Unix utilities (`head`, `tail`, `grep`) work only if Git's `usr/bin` happens to be on PATH, which you cannot rely on.\n\n⚠ A `cmd` pipeline whose RIGHT half cannot be resolved does not run its LEFT half either, so a failed pipe means the command never executed at all.";
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
        if contains_git_index_or_commit_mutation(&command) {
            return ToolResult::error(
                "Direct Git staging, commit, and ref-mutation commands are blocked in Bash. Use `git_commit` with explicit changed file paths and a Conventional Commit subject, blank line, and descriptive body.",
            );
        }
        // T-6 (P8-33) - clamp the model-supplied timeout into a usable range.
        //
        // This was a bare `unwrap_or(DEFAULT)`, so `timeout_secs: 0` was
        // accepted and became `Duration::from_secs(0)` - every command timed
        // out instantly, and the model saw "timed out" for a command that
        // never ran. A huge value was accepted just as readily and removed the
        // timeout altogether, which is the guard `T-5` exists to keep.
        //
        // Clamped rather than refused: a caller asking for 0 or 10^9 has made
        // a units mistake, and the useful response is to run the command
        // inside sane bounds, not to hand back an error it cannot act on.
        let timeout_secs = input["timeout_secs"]
            .as_u64()
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS);

        // The shared resolver requires an existing absolute workspace and
        // canonicalizes it before the shell starts. The redirect scan below is
        // only a heuristic; free-form shell code can still write elsewhere, so
        // permission policy and OS-level isolation remain separate controls.
        let cwd_path = PathBuf::from(&ctx.working_dir);
        if let Err(msg) = validate_workspace_path(&cwd_path, &ctx.working_dir) {
            // A working directory that cannot canonicalise is a configuration
            // failure — surface it rather than running blindly in an unknown
            // location.
            return ToolResult::error(msg);
        }

        // 2. Best-effort scan for `>` / `>>` / `tee` redirect targets that
        //    point outside the workspace. Shell redirection has many forms
        //    (here-docs, process substitution, eval, dynamic vars) we cannot
        //    statically extract, so this only catches the easy cases.
        if let Some(err) = check_redirect_targets(&command, &ctx.working_dir) {
            return ToolResult::error(err);
        }

        let command = if cfg!(target_os = "windows") {
            let stripped = strip_redundant_cd_prefix(&command, &ctx.working_dir);
            if stripped != command {
                tracing::debug!(
                    "[BashTool] stripped redundant cd prefix: {} -> {}",
                    command,
                    stripped
                );
            }
            stripped
        } else {
            command
        };

        let (shell_program, shell_args) = if cfg!(target_os = "windows") {
            ("cmd", vec![OsString::from("/C"), OsString::from(&command)])
        } else {
            ("sh", vec![OsString::from("-c"), OsString::from(&command)])
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
        let env_remove = std::env::vars()
            .filter_map(|(key, _)| {
                crate::security::env_scrub::looks_like_a_credential(&key)
                    .then(|| OsString::from(key))
            })
            .collect();
        let shell_spec = ProcessSpec {
            program: OsString::from(shell_program),
            args: shell_args,
            raw_arg: None,
            cwd: cwd_path.clone(),
            env_remove,
            env_set: Vec::new(),
            #[cfg(windows)]
            read_only_paths: Vec::new(),
        };

        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        let result = timeout(
            Duration::from_secs(timeout_secs),
            run_command(shell_spec, &ctx.working_dir),
        )
        .await;

        match result {
            Err(_) => ToolResult::categorized_error(
                "operation_timed_out",
                "timeout",
                format!("Shell operation timed out after {timeout_secs} seconds; command text is omitted from the failure record."),
                "Inspect the workspace and process state before retrying; a timed-out command may have completed side effects.",
            )
            .with_unknown_side_effect_outcome(),
            Ok(Err(_)) => ToolResult::categorized_error(
                "process_spawn_failed",
                "infrastructure",
                "Arcana could not start the shell operation.",
                "Verify the workspace and available shell/toolchain, then retry only after checking task state.",
            ),
            Ok(Ok((stdout, stderr, exit_code))) => {
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
                    // 🔴 This used to suggest `Select-Object -First N`, which is a
                    // PowerShell cmdlet and cannot run under `cmd /C` -- the same
                    // syntax `rewrite_for_windows` was substituting, so following
                    // the advice reproduced the failure exactly. `P8-26`.
                    return ToolResult::categorized_error(
                        "process_exit_nonzero",
                        "command_exit",
                        "Windows CMD could not resolve a pipeline command; command output is omitted from the failure record.",
                        "Use CMD-compatible tools such as findstr or more, or use project_command for supported build/test operations.",
                    );
                }
                if is_error {
                    ToolResult::categorized_error(
                        "process_exit_nonzero",
                        "command_exit",
                        format!("Shell operation exited with code {exit_code}; command text and output are omitted from the failure record."),
                        "Use project_command for supported build/test operations or run a narrower diagnostic.",
                    )
                } else {
                    ToolResult::success(output)
                }
            }
        }
    }
}

/// Keep common shell-authored Git index/ref mutations on the host-validated
/// `git_commit` path. This is a textual guard, not a process sandbox: encoded
/// programs and arbitrary subprocess code remain outside its guarantee.
fn contains_git_index_or_commit_mutation(command: &str) -> bool {
    command
        .split(['&', '|', ';', '\n'])
        .map(str::trim)
        .any(|segment| {
            let normalized = segment
                .replace(['(', ')', '[', ']', ','], " ")
                .replace(['"', '\'', '`'], "");
            let words = normalized.split_whitespace().collect::<Vec<_>>();
            let Some(executable) = words.first().copied() else {
                return false;
            };
            let executable = executable
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or_default()
                .to_ascii_lowercase();
            // Text-search commands routinely inspect examples containing these
            // words; they do not execute the quoted text as a Git command.
            if matches!(
                executable.as_str(),
                "rg" | "grep" | "egrep" | "fgrep" | "findstr"
            ) {
                return false;
            }

            // Shells can wrap a Git command (`cmd /c`, `sh -c`, PowerShell),
            // or a script can invoke it through a subprocess API. Inspect the
            // complete shell segment after token cleanup, not just argv[0].
            // This is a guard over shell-authored command text; executable
            // semantics remain arbitrary by design, so the registered
            // `git_commit` tool is still the only supported commit path.
            let words = words
                .iter()
                .map(|word| word.to_ascii_lowercase())
                .collect::<Vec<_>>();
            let is_git_mutation = |argument: &str| {
                matches!(
                    argument,
                    "add"
                        | "commit"
                        | "commit-tree"
                        | "update-index"
                        | "update-ref"
                        | "symbolic-ref"
                        | "read-tree"
                        | "write-tree"
                        | "fast-import"
                        | "apply"
                        | "am"
                        | "stash"
                        | "reset"
                        | "merge"
                        | "rebase"
                        | "cherry-pick"
                        | "revert"
                )
            };
            let has_mutation_argument =
                |arguments: &[String]| arguments.iter().any(|argument| is_git_mutation(argument));

            // An executable supplied through a shell variable can resolve to
            // Git even though the segment contains no literal `git` token:
            // `G=git; $G commit ...`. Refuse variable-selected executables
            // when followed by a Git index/ref mutation. This remains a
            // conservative textual guard; process confinement is the other
            // boundary, and host-side commits still use `git_commit`.
            let dynamic_executable = executable.starts_with('$');
            if dynamic_executable && has_mutation_argument(&words[1..]) {
                return true;
            }

            words.iter().enumerate().any(|(index, word)| {
                (word == "git"
                    || word == "git.exe"
                    || word
                        .split_once('=')
                        .is_some_and(|(_, value)| value == "git" || value == "git.exe"))
                    && has_mutation_argument(&words[index + 1..])
            })
        })
}

#[cfg(test)]
mod git_mutation_guard_tests {
    use super::contains_git_index_or_commit_mutation;

    #[test]
    fn direct_git_add_and_commit_are_blocked_but_inspection_is_allowed() {
        for command in [
            "git add src/main.rs",
            "git add -A",
            "git commit -m \"fix: change\"",
            "cargo test && git.exe commit -m \"fix: change\"",
            "git -C C:\\repo add .",
            "git -C . -c a=b -c c=d -c e=f commit -m bypass",
            "git --git-dir .git -c core.hooksPath=hooks -c user.name=worker commit -m bypass",
            "cmd /c \"git add -A\"",
            "sh -c 'git commit -m test'",
            "python -c \"subprocess.run(['git', 'add', '-A'])\"",
            "git update-index --add src/main.rs",
            "git update-ref refs/heads/main deadbeef",
            "git commit-tree HEAD^{tree} -m bypass",
            "G=git; $G commit -m bypass",
            "G=git; $G add -A",
            "git apply --cached patch.diff",
            "git reset --mixed HEAD~1",
        ] {
            assert!(contains_git_index_or_commit_mutation(command), "{command}");
        }
        for command in [
            "git status --short --branch",
            "git diff --cached",
            "git switch arcana/fleet",
            "rg \"git commit\" docs/",
        ] {
            assert!(!contains_git_index_or_commit_mutation(command), "{command}");
        }
    }
}

async fn run_command(
    spec: ProcessSpec,
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

    let mut child = process_sandbox::spawn(spec, &normalized_path).await?;
    let mut stdout = child.take_stdout().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "sandboxed child stdout was not captured",
        )
    })?;
    let mut stderr = child.take_stderr().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "sandboxed child stderr was not captured",
        )
    })?;
    let stdout_reader = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    });
    let stderr_reader = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).await?;
        Ok::<_, std::io::Error>(bytes)
    });
    let exit_code = child.wait().await?;
    let stdout = stdout_reader
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))??;
    let stderr = stderr_reader
        .await
        .map_err(|error| std::io::Error::other(error.to_string()))??;
    let stdout = String::from_utf8_lossy(&stdout).to_string();
    let stderr = String::from_utf8_lossy(&stderr).to_string();
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
                     variable or tilde expansion. Arcana cannot statically prove \
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

/// Strip a redundant `cd "<working_dir>" &&` / `Set-Location "<working_dir>"; `
/// prefix that agents generate unnecessarily.
///
/// The OS-level `current_dir` is already set to `working_dir` by `run_command`,
/// so these prefixes add nothing and cause Windows path-with-spaces failures.
/// Applied only on `cfg!(target_os = "windows")`.
///
/// 🔴 **This function used to also rewrite Unix utilities into PowerShell
/// cmdlets, and that half is DELETED** (`P8-26`, razi-ruled,
/// `ADR-LOG-RESTRUCT-0038`). It replaced `| tail -50` with
/// `| Select-Object -Last 50` and then ran the result under `cmd /C`, which
/// has no such cmdlet. Measured on Windows 11 with a passing control:
///
/// | command | exit |
/// |---|---|
/// | `shim.cmd` alone | 0 |
/// | `shim.cmd \| more` | 0 |
/// | `shim.cmd \| tail -50` | **0** — Git's `tail` is on PATH |
/// | `shim.cmd \| Select-Object -Last 50` | **255**, always |
///
/// So the rewrite converted a command that WORKED into one that cannot, and
/// when the right half of a `cmd` pipeline fails to resolve, `cmd` does not
/// run the left half either — the build never happened at all.
///
/// ⚠ It also closed a loop the model could not escape: line 29 of this file
/// tells it to avoid Unix utilities on Windows, and the exit-255 handler below
/// then suggests `Select-Object -First N` — the exact syntax the rewriter had
/// just substituted and that `cmd` cannot run. Letting the honest
/// `'tail' is not recognized` reach the model is what opens it.
fn strip_redundant_cd_prefix(cmd: &str, working_dir: &str) -> String {
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

    out
}

#[cfg(test)]
mod p8_26_tests {
    //! `P8-26` — the Windows command pre-processor.
    //!
    //! 🔴 **`bash.rs` had no tests at all before this**, which is how a
    //! rewriter that converted working commands into broken ones survived. It
    //! is the one executor that runs model-authored commands.

    use super::*;

    const WD: &str = r"C:\projects\demo";

    // --- what the pre-processor MUST still do --------------------------------

    #[test]
    fn a_redundant_quoted_cd_prefix_is_stripped() {
        let out = strip_redundant_cd_prefix(&format!("cd \"{WD}\" && npm install"), WD);
        assert_eq!(out, "npm install");
    }

    #[test]
    fn a_redundant_unquoted_cd_prefix_is_stripped() {
        let out = strip_redundant_cd_prefix(&format!("cd {WD} && cargo build"), WD);
        assert_eq!(out, "cargo build");
    }

    #[test]
    fn a_set_location_prefix_is_stripped() {
        let out = strip_redundant_cd_prefix(&format!("Set-Location \"{WD}\" ; cargo test"), WD);
        assert_eq!(out, "cargo test");
    }

    #[test]
    fn a_cd_to_a_different_directory_is_left_alone() {
        // Only the REDUNDANT prefix goes. A deliberate `cd` elsewhere is the
        // caller meaning it.
        let cmd = r"cd C:\somewhere\else && npm test";
        assert_eq!(strip_redundant_cd_prefix(cmd, WD), cmd);
    }

    // --- what it must NEVER do again -----------------------------------------

    #[test]
    fn a_unix_pipe_is_passed_through_untouched() {
        // 🔴 The regression this phase exists to close. This used to become
        // `| Select-Object -Last 50`, a PowerShell cmdlet that `cmd /C` cannot
        // run -- exit 255, always. Measured on Windows 11: `| tail -50` exits
        // 0 when Git's usr/bin is on PATH, so the rewrite converted a WORKING
        // command into a broken one.
        for cmd in [
            "npm run build 2>&1 | tail -50",
            "cargo test | head -n 20",
            "git log | grep fix",
            "cat package.json | jq .",
        ] {
            assert_eq!(
                strip_redundant_cd_prefix(cmd, WD),
                cmd,
                "the pre-processor must not rewrite a pipe",
            );
        }
    }

    #[test]
    fn no_powershell_cmdlet_is_ever_substituted() {
        // Asserts the SECRET-IS-GONE shape (`L-C14`): name the thing that must
        // be absent, rather than checking that something changed.
        let out =
            strip_redundant_cd_prefix(&format!("cd \"{WD}\" && npm run build 2>&1 | tail -50"), WD);
        assert!(!out.contains("Select-Object"), "got: {out}");
        assert!(!out.contains("Select-String"), "got: {out}");
        assert_eq!(out, "npm run build 2>&1 | tail -50");
    }

    // --- the closed loop the rewriter created --------------------------------

    #[test]
    fn the_tool_description_does_not_advise_a_cmdlet_cmd_cannot_run() {
        // 🔴 `bash.rs`'s description and its exit-255 handler both used to
        // point the model at `Select-Object`, which is exactly what the
        // rewriter substituted and what `cmd` cannot execute -- advice the
        // model could not escape by following.
        let tool = BashTool;
        let d = tool.description();
        if cfg!(target_os = "windows") {
            assert!(
                !d.contains("PowerShell-compatible alternatives"),
                "the description still recommends PowerShell under cmd",
            );
            assert!(
                d.contains("NOT PowerShell"),
                "it must say which shell this is"
            );
            assert!(d.contains("findstr"), "it must name a cmd-native filter");
        }
    }

    // --- P8-27: the timed-out child must actually die -------------------------

    #[tokio::test]
    async fn a_timed_out_command_does_not_outlive_its_timeout() {
        // 🔴 Asserts the BEHAVIOUR, not the flag. `assert!(source.contains(
        // "kill_on_drop"))` would pass against a `kill_on_drop` on the wrong
        // Command, and this repo has already shipped a guard that was present
        // and unreachable more than once (`L-C12`).
        //
        // The shape: run something that writes a marker AFTER a delay, time it
        // out well before that, then wait past the delay. If the child
        // survived the drop, the marker appears.
        use std::time::Duration;
        use tokio::time::{sleep, timeout};

        let dir =
            std::env::temp_dir().join(format!("arcana-p8-27-killtest-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).expect("create unique isolated directory");
        let marker = dir.join("survived.txt");

        let script = if cfg!(target_os = "windows") {
            // Avoid `ping`: the AppContainer intentionally denies network
            // access, so it exits immediately instead of exercising timeout.
            "for /l %i in (1,1,100000000) do @rem & echo survived > survived.txt"
        } else {
            "sleep 4; echo survived > survived.txt"
        };

        let (program, args) = if cfg!(target_os = "windows") {
            ("cmd", vec![OsString::from("/C"), OsString::from(script)])
        } else {
            ("sh", vec![OsString::from("-c"), OsString::from(script)])
        };
        let spec = ProcessSpec {
            program: OsString::from(program),
            args,
            raw_arg: None,
            cwd: dir.clone(),
            env_remove: Vec::new(),
            env_set: Vec::new(),
            #[cfg(windows)]
            read_only_paths: Vec::new(),
        };

        // Time out long before the child would write its marker.
        let outcome = timeout(
            Duration::from_millis(400),
            run_command(spec, dir.to_str().expect("utf-8 temp path")),
        )
        .await;
        assert!(outcome.is_err(), "the fixture must actually time out");

        // Wait past the point where a surviving child would have written.
        sleep(Duration::from_secs(6)).await;

        assert!(
            !marker.exists(),
            "the child outlived its timeout and wrote {}: kill_on_drop is not \
             in effect, so a timed-out `cargo build` would keep holding \
             target/.cargo-lock",
            marker.display()
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod process_sandbox_tests {
    use super::*;
    use crate::agent::permission::PermissionMode;
    use crate::agent::tool_trait::{Tool, ToolContext};

    #[tokio::test]
    async fn nonzero_shell_exit_is_a_structured_failure_without_command_output() {
        let workspace = tempfile::tempdir().expect("workspace");
        let context = ToolContext {
            session_id: "bash-structured-error-test".into(),
            working_dir: workspace.path().to_string_lossy().into_owned(),
            call_id: "bash-structured-error-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "supervisor".into(),

            run_token_budget: None,
        };
        let command = if cfg!(windows) {
            "echo PRIVATE_COMMAND_OUTPUT & exit /b 17"
        } else {
            "printf PRIVATE_COMMAND_OUTPUT; exit 17"
        };
        let result = BashTool
            .execute(serde_json::json!({"command": command}), &context)
            .await;
        let envelope: serde_json::Value =
            serde_json::from_str(&result.output).expect("shell failure has valid JSON");

        assert!(result.is_error);
        assert_eq!(envelope["schema_version"], 1);
        assert_eq!(envelope["ok"], false);
        assert_eq!(envelope["error"]["code"], "process_exit_nonzero");
        assert_eq!(envelope["error"]["category"], "command_exit");
        assert!(!result.output.contains("PRIVATE_COMMAND_OUTPUT"));
        assert!(!result.output.contains(command));
    }

    #[tokio::test]
    async fn worker_process_is_refused_before_a_task_claim() {
        let workspace = tempfile::tempdir().expect("workspace");
        let context = ToolContext {
            session_id: "bash-unclaimed-test".into(),
            working_dir: workspace.path().to_string_lossy().into_owned(),
            call_id: "bash-unclaimed-call".into(),
            run_id: None,
            task_id: None,
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = BashTool
            .execute(
                serde_json::json!({"command": "mkdir must-not-run"}),
                &context,
            )
            .await;

        assert!(result.is_error);
        assert!(!workspace.path().join("must-not-run").exists());
    }

    #[tokio::test]
    async fn bash_tool_process_can_write_inside_but_not_to_a_sibling_directory() {
        let parent = tempfile::tempdir().expect("temporary parent");
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = parent.path().join("workspace");
        let outside = parent.path().join("outside");
        std::fs::create_dir(&workspace).expect("workspace");
        let db = crate::db::tasks::TasksDb::open().expect("open task database");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "sandboxed shell task".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "shell writes stay in the workspace".into(),
                created_by: Some("sandbox-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        let command = if cfg!(windows) {
            "mkdir inside-dir & mkdir ..\\outside"
        } else {
            "mkdir inside-dir; mkdir ../outside"
        };
        let context = ToolContext {
            session_id: "bash-sandbox-test".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "bash-sandbox-write-boundary".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };

        let result = BashTool
            .execute(serde_json::json!({"command": command}), &context)
            .await;

        assert!(workspace.join("inside-dir").is_dir(), "{result:?}");
        assert!(
            !outside.exists(),
            "Bash process escaped its workspace boundary: {result:?}"
        );
    }

    // --- P4-19 / TG4-23 · process-writer coverage ------------------------------
    //
    // P4-19 closed for the four file tools but left its process writers open.
    // A shell command is not a file write the host can checkpoint ahead of
    // time, so its attributable recovery boundary is the executor's journalled
    // intent plus task-attributed `RecoverySnapshot` (workspace fingerprint,
    // Git HEAD/branch/index digest, the task's checkpoint ids) written BEFORE
    // the child spawns. These tests hold that boundary to the same rule the
    // file tools already obey: if the boundary cannot be persisted, the write
    // does not happen. What they deliberately do NOT claim: per-file content
    // reversal of a file a shell rewrote — that stays with the repository
    // snapshot breadth owned by P4-53/TG4-27 (ADR-LOG-RESTRUCT-0403).

    #[cfg(any(windows, target_os = "linux"))]
    fn claim_process_writer_task(worker: &str) -> i64 {
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "process-writer recovery boundary".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a shell write needs an attributable recovery boundary".into(),
                created_by: Some("p4-19-process-writer-test".into()),
            })
            .expect("create process-writer task");
        tasks
            .update_status(task.id, "assigned", Some(worker))
            .expect("assign process-writer task");
        tasks
            .claim_task_with_pid(task.id, worker, std::process::id())
            .expect("claim process-writer task");
        task.id
    }

    /// Drive the registered production `bash` tool through `ToolExecutor`,
    /// including the per-call host approval every freeform shell requires in
    /// Autonomous mode, and return the model-facing result.
    #[cfg(any(windows, target_os = "linux"))]
    async fn run_registered_bash(
        workspace: &std::path::Path,
        worker: &str,
        session_id: &str,
        run_id: Option<String>,
        task_id: Option<i64>,
        command: &str,
    ) -> ToolResult {
        use crate::agent::permission::{
            new_pending_permissions, PermissionMode, PermissionResponse,
        };
        use crate::agent::tool_executor::ToolExecutor;
        use crate::agent::tool_trait::AgentIdentity;
        use std::sync::Arc;
        use std::time::Duration;

        let pending = new_pending_permissions();
        let responder_map = Arc::clone(&pending);
        let mut executor = ToolExecutor::new(
            PermissionMode::Autonomous,
            pending,
            session_id.into(),
            workspace.to_string_lossy().into_owned(),
            AgentIdentity::worker(worker),
        )
        .with_run_id(run_id);
        executor.set_task_id(task_id);
        let tool: Arc<dyn Tool> = Arc::new(BashTool);
        let input = serde_json::json!({ "command": command });
        let execution = tokio::spawn(async move {
            executor
                .execute_batch(vec![(tool, input, "process-boundary-call".into())])
                .await
        });

        // The executor keys the one-use approval on `<session>-<tool_use_id>`.
        let permission_id = format!("{session_id}-process-boundary-call");
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if responder_map
                    .lock()
                    .expect("pending permission mutex")
                    .contains_key(&permission_id)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("registered Bash must request per-call host approval");
        responder_map
            .lock()
            .expect("pending permission mutex")
            .remove(&permission_id)
            .expect("one-use pending responder")
            .send(PermissionResponse::Allow)
            .expect("permission waiter still active");
        let results = execution
            .await
            .expect("registered Bash executor task")
            .expect("registered Bash batch");
        results.into_iter().next().expect("one batch result").1
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn shell_mutation_without_a_persistable_recovery_boundary_never_runs() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        let task_id = claim_process_writer_task("worker-backend");
        let runs = crate::db::runs::RunsDb::open().expect("open isolated RunsDb");
        let run_id = runs
            .begin_worker_tick("worker-backend", "process-boundary-session")
            .expect("begin durable worker run");
        runs.conn
            .execute_batch(
                "CREATE TRIGGER reject_operation_intent
                 BEFORE INSERT ON operation_journal
                 WHEN NEW.record_type = 'intent'
                 BEGIN SELECT RAISE(ABORT, 'injected intent-store failure'); END;",
            )
            .expect("inject an intent-store failure");

        let result = run_registered_bash(
            workspace.path(),
            "worker-backend",
            "process-boundary-session",
            Some(run_id.clone()),
            Some(task_id),
            "echo written> boundary-sentinel.txt",
        )
        .await;

        assert!(
            result.is_error,
            "a shell without a persisted recovery boundary must be refused: {}",
            result.output
        );
        assert!(
            !workspace.path().join("boundary-sentinel.txt").exists(),
            "the child must never spawn when its task-attributed boundary is unpersistable"
        );
        let envelope: serde_json::Value =
            serde_json::from_str(&result.output).expect("structured boundary refusal");
        assert_eq!(
            envelope["error"]["code"], "operation_intent_unavailable",
            "the refusal must name the missing recovery evidence: {}",
            result.output
        );
        assert_eq!(
            runs.operation_journal_page(&run_id, 0, 10)
                .expect("read the failed intent page")
                .entries
                .len(),
            0,
            "a refused shell leaves no intent that could later read as a duplicate effect"
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn unresolved_prior_shell_effect_blocks_a_new_shell_mutation() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        let task_id = claim_process_writer_task("worker-backend");
        let runs = crate::db::runs::RunsDb::open().expect("open isolated RunsDb");
        let interrupted = runs
            .begin_worker_tick("worker-backend", "process-interrupted-session")
            .expect("begin interrupted worker run");
        runs.record_operation_intent(
            &interrupted,
            Some(task_id),
            "uncertain-shell-intent",
            "bash",
            "FreeformShell",
            &[],
        )
        .expect("persist the prior uncertain shell effect");
        runs.finish_worker_tick(&interrupted, "interrupted", "interrupted", None)
            .expect("close the interrupted run");
        let replacement = runs
            .begin_worker_tick("worker-backend", "process-replacement-session")
            .expect("begin replacement worker run");

        let result = run_registered_bash(
            workspace.path(),
            "worker-backend",
            "process-replacement-session",
            Some(replacement),
            Some(task_id),
            "echo written> unresolved-sentinel.txt",
        )
        .await;

        assert!(result.is_error, "{}", result.output);
        assert!(
            !workspace.path().join("unresolved-sentinel.txt").exists(),
            "a replacement worker may not run a shell while its task effect is unresolved"
        );
        let envelope: serde_json::Value =
            serde_json::from_str(&result.output).expect("structured reconciliation refusal");
        assert_eq!(
            envelope["error"]["code"], "operation_reconciliation_required",
            "{}",
            result.output
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn shell_mutation_records_a_task_attributed_recovery_boundary() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        let task_id = claim_process_writer_task("worker-backend");
        let runs = crate::db::runs::RunsDb::open().expect("open isolated RunsDb");
        let run_id = runs
            .begin_worker_tick("worker-backend", "process-record-session")
            .expect("begin durable worker run");

        let result = run_registered_bash(
            workspace.path(),
            "worker-backend",
            "process-record-session",
            Some(run_id.clone()),
            Some(task_id),
            "echo written> process-write.txt",
        )
        .await;
        assert!(!result.is_error, "{}", result.output);
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("process-write.txt"))
                .expect("shell wrote inside its workspace")
                .trim(),
            "written"
        );

        let page = runs
            .operation_journal_page(&run_id, 0, 10)
            .expect("read the process-write journal");
        assert_eq!(page.entries.len(), 1);
        let entry = &page.entries[0];
        assert_eq!(
            entry.task_id,
            Some(task_id),
            "a process write is attributed to the claimed task, not its session"
        );
        assert_eq!(entry.operation, "bash");
        assert_eq!(entry.effect, "FreeformShell");
        let before = entry
            .before_snapshot
            .as_ref()
            .expect("the shell must carry a pre-spawn recovery snapshot");
        assert_eq!(
            before.task_id(),
            task_id,
            "the recovery snapshot belongs to the same claimed task"
        );
    }
}
