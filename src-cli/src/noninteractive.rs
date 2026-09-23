//! Non-interactive mode utilities for CI/CD scripting.
//!
//! Exit codes:
//!   0 — success
//!   1 — error (unrecoverable failure)
//!   2 — partial success (some items failed, some succeeded)

/// Detect whether stdin/stdout are a real TTY (vs a pipe or CI environment).
pub fn is_tty() -> bool {
    // Check common CI environment variables first
    if std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("GITLAB_CI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
        || std::env::var("BUILDKITE").is_ok()
        || std::env::var("CIRCLECI").is_ok()
    {
        return false;
    }

    // On Windows, check for a console handle via winapi-style check:
    // If TERM is "dumb" or not set and there's no COLORTERM, assume no TTY.
    #[cfg(windows)]
    {
        // Rough heuristic: PowerShell/cmd sets SESSIONNAME; subprocess pipes don't.
        std::env::var("SESSIONNAME").is_ok()
    }
    #[cfg(not(windows))]
    {
        // On Unix: use std::io::IsTerminal (stable since Rust 1.70 — no libc dep needed).
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    }
}

/// Standard exit codes for arcanactl.
pub mod exit_code {
    pub const SUCCESS: i32 = 0;
    pub const ERROR: i32 = 1;
    pub const PARTIAL_SUCCESS: i32 = 2;
}

/// Exit the process with the appropriate code and, if non-zero,
/// print the error message to stderr (JSON format when not a TTY).
pub fn exit_with(code: i32, message: Option<&str>) -> ! {
    if let Some(msg) = message {
        if is_tty() {
            eprintln!("Error: {msg}");
        } else {
            // Machine-readable: print JSON to stderr for piping
            eprintln!("{{\"status\":\"error\",\"message\":{}}}", json_string(msg));
        }
    }
    std::process::exit(code);
}

/// Emit a partial-success notice (exit code 2) with a summary.
pub fn partial_success(succeeded: usize, failed: usize) -> ! {
    let msg = format!("{succeeded} succeeded, {failed} failed");
    if is_tty() {
        eprintln!("Partial success: {msg}");
    } else {
        eprintln!("{{\"status\":\"partial\",\"succeeded\":{succeeded},\"failed\":{failed}}}");
    }
    std::process::exit(exit_code::PARTIAL_SUCCESS);
}

/// Minimal JSON string escaping (no external deps needed here).
fn json_string(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// Resolve effective non-interactive mode:
/// CLI flag `--non-interactive` OR not a TTY OR CI env var set.
pub fn is_non_interactive(flag: bool) -> bool {
    flag || !is_tty()
}

/// Read a prompt/task from a file path, stripping leading/trailing whitespace.
pub fn read_prompt_file(path: &str) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("Cannot read prompt file '{}': {}", path, e))?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!("Prompt file '{}' is empty", path));
    }
    Ok(trimmed.to_string())
}
