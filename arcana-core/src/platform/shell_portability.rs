//! Validation for shell guidance and saved shell hooks that must work across
//! the Windows, macOS, and Linux hosts Arcana supports.

const NON_PORTABLE_FRAGMENTS: &[&str] = &["/tmp/", "$?", "$(", "source .venv"];
const SHELL_FENCE_MARKERS: &[&str] = &["```bash", "```sh", "```shell"];
const SHELL_OPERATORS: &[&str] = &["&&", ";", "`", "&"];

/// Reject known POSIX-only paths/syntax in persisted agent guidance, checking
/// shell operators only in fenced shell examples and `bash('...')` examples.
pub fn validate_agent_guidance(text: &str) -> Result<(), String> {
    let mut in_shell_fence = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if SHELL_FENCE_MARKERS
            .iter()
            .any(|marker| trimmed.starts_with(marker))
        {
            in_shell_fence = true;
            continue;
        }
        if in_shell_fence && trimmed.starts_with("```") {
            in_shell_fence = false;
            continue;
        }

        if in_shell_fence {
            validate_shell_command(line)?;
        } else if let Some(command) = extract_bash_example(line) {
            validate_shell_command(command)?;
        }
    }
    Ok(())
}

/// Apply the stricter portability contract for guidance that Arcana will
/// persist and later hand to an agent. A POSIX temporary path can be described
/// in ordinary prose rather than a shell fence, so it must be rejected before
/// the fenced-command validator runs.
pub fn validate_persisted_agent_guidance(text: &str) -> Result<(), String> {
    if text.contains("/tmp/") {
        return Err("contains hardcoded POSIX temporary path '/tmp/'".into());
    }
    validate_agent_guidance(text)
}

/// Validate a persisted hook command before saving it and again before running
/// it, since older database rows may predate this check.
pub fn validate_shell_command(command: &str) -> Result<(), String> {
    validate_known_fragments(command)?;
    validate_no_tee_command(command)?;

    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in command.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        // Only double quotes protect operators on both POSIX shells and CMD.
        // Single quotes are ordinary characters to CMD, so treating their
        // contents as quoted would let a persisted hook hide `&&` on Windows.
        if ch == '"' {
            quote = Some(ch);
            continue;
        }
        for operator in SHELL_OPERATORS {
            if *operator == "&"
                && command[..index].ends_with('>')
                && command[index + 1..]
                    .chars()
                    .next()
                    .is_some_and(|target| target.is_ascii_digit() || target == '-')
            {
                // File-descriptor duplication such as `2>&1` is valid in both
                // POSIX shells and Windows CMD. It is not command chaining.
                continue;
            }
            if command[index..].starts_with(operator) {
                return Err(format!("contains non-portable shell operator {operator:?}"));
            }
        }
    }
    Ok(())
}

fn validate_no_tee_command(command: &str) -> Result<(), String> {
    for segment in command.split(['|', ';', '&']) {
        let mut words = segment.split_whitespace();
        let mut executable = next_shell_command(&mut words);
        if executable.is_some_and(|word| {
            word.trim_matches(['\'', '"'])
                .rsplit(['/', '\\'])
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("env"))
        }) {
            executable = next_shell_command(&mut words);
        }

        if executable.is_some_and(|word| {
            word.trim_matches(['\'', '"'])
                .rsplit(['/', '\\'])
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("tee"))
        }) {
            return Err("contains non-portable POSIX command `tee`".into());
        }
    }
    Ok(())
}

fn next_shell_command<'a>(words: &mut impl Iterator<Item = &'a str>) -> Option<&'a str> {
    words.find(|word| {
        !word.contains('=')
            || word.starts_with('=')
            || word.starts_with('\'')
            || word.starts_with('"')
    })
}

/// Validate a persisted hook working directory for known platform-specific
/// temporary paths without treating ordinary path punctuation as shell syntax.
pub fn validate_working_directory(path: &str) -> Result<(), String> {
    if path.contains("/tmp/") {
        return Err("contains hardcoded POSIX temporary path '/tmp/'".into());
    }
    Ok(())
}

fn validate_known_fragments(text: &str) -> Result<(), String> {
    for fragment in NON_PORTABLE_FRAGMENTS {
        if text.contains(fragment) {
            return Err(format!("contains non-portable shell fragment {fragment:?}"));
        }
    }
    Ok(())
}

fn extract_bash_example(line: &str) -> Option<&str> {
    let start = line.find("bash('")? + "bash('".len();
    let command = &line[start..];
    Some(command.split("')").next().unwrap_or(command))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guidance_checks_executable_examples_without_rejecting_explanatory_prose() {
        assert!(validate_agent_guidance("Use the project build command.").is_ok());
        assert!(validate_agent_guidance("Do not use `source .venv` or `2>&1` in prose.").is_ok());
        assert!(validate_agent_guidance("Temporary output belongs under /tmp/build").is_ok());
        assert!(validate_agent_guidance("```bash\nprintf ok > /tmp/build\n```").is_err());
        assert!(validate_agent_guidance("```bash\nnpm test && npm lint\n```").is_err());
        assert!(validate_agent_guidance("```bash\ntee build.log\n```").is_err());
        assert!(validate_agent_guidance("```bash\nnpm test 2>&1\n```").is_ok());
        assert!(validate_agent_guidance("bash('git status && git diff')").is_err());
        assert!(validate_agent_guidance("Run `bash('source .venv/bin/activate')`.").is_err());
        assert!(validate_agent_guidance("```powershell\nWrite-Output ok\n```").is_ok());
    }

    #[test]
    fn hook_commands_reject_shell_operators_outside_quotes_and_allow_quoted_literals() {
        assert!(validate_shell_command("npm test && npm lint").is_err());
        assert!(validate_shell_command("echo '$?' ").is_err());
        assert!(validate_shell_command("npm run build 2>&1").is_ok());
        assert!(validate_shell_command("echo 'a && b'").is_err());
        assert!(validate_shell_command("echo \"a && b\"").is_ok());
        assert!(validate_shell_command("tee build.log").is_err());
        assert!(validate_shell_command("PATH=tools tee build.log").is_err());
        assert!(validate_shell_command("env PATH=tools tee build.log").is_err());
        assert!(validate_shell_command("npm test | tee build.log").is_err());
        assert!(validate_shell_command("cargo test").is_ok());
    }
}
