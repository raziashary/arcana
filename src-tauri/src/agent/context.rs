// Context builder — constructs the system prompt for PA mode.
// Injects CLAUDE.md content from ~/.airs/CLAUDE.md and {cwd}/CLAUDE.md.
// Pattern from Claude Code's context injection system.

use std::path::Path;
use tracing::debug;

pub struct ContextBuilder {
    working_dir: String,
}

impl ContextBuilder {
    pub fn new(working_dir: impl Into<String>) -> Self {
        Self { working_dir: working_dir.into() }
    }

    /// Build the full system prompt including CLAUDE.md contents.
    /// If override is provided, it replaces BASE_SYSTEM_PROMPT entirely.
    pub fn build_system_prompt(&self, override_prompt: Option<&str>) -> String {
        let base = override_prompt.unwrap_or(BASE_SYSTEM_PROMPT);
        let mut parts = vec![base.to_string()];

        // ~/.airs/CLAUDE.md — user global instructions
        if let Some(home) = home_dir() {
            let global_claude = home.join(".airs").join("CLAUDE.md");
            if let Ok(content) = std::fs::read_to_string(&global_claude) {
                if !content.trim().is_empty() {
                    debug!("Injecting global CLAUDE.md from {:?}", global_claude);
                    parts.push(format!("## Global Instructions (CLAUDE.md)\n\n{content}"));
                }
            }
        }

        // {cwd}/CLAUDE.md — project-specific instructions
        let cwd_claude = Path::new(&self.working_dir).join("CLAUDE.md");
        if let Ok(content) = std::fs::read_to_string(&cwd_claude) {
            if !content.trim().is_empty() {
                debug!("Injecting project CLAUDE.md from {:?}", cwd_claude);
                parts.push(format!("## Project Instructions (CLAUDE.md)\n\n{content}"));
            }
        }

        // Also check AGENTS.md (Goose-style)
        let agents_md = Path::new(&self.working_dir).join("AGENTS.md");
        if let Ok(content) = std::fs::read_to_string(&agents_md) {
            if !content.trim().is_empty() {
                debug!("Injecting AGENTS.md from {:?}", agents_md);
                parts.push(format!("## Agent Instructions (AGENTS.md)\n\n{content}"));
            }
        }

        parts.join("\n\n---\n\n")
    }

    /// Returns the default base system prompt (for display in settings).
    pub fn default_system_prompt() -> &'static str {
        BASE_SYSTEM_PROMPT
    }
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(std::path::PathBuf::from)
}

const BASE_SYSTEM_PROMPT: &str = r#"You are AutoDev, an AI software engineering assistant built by AIRS Foundry.

You help developers by writing code, reading files, running commands, and managing projects. You are precise, thorough, and write production-quality code.

## Guidelines
- Always read files before modifying them to understand context
- Prefer minimal, focused changes over large rewrites
- Run tests after code changes when possible
- Report errors clearly and suggest fixes
- Ask for clarification when intent is ambiguous

## Available Tools

You have access to 7 built-in tools for file operations, code search, and command execution:

### File Reading & Search
- **read_file** — Read file contents with line numbers. Supports line ranges for large files.
- **grep** — Search for patterns in files using ripgrep. Returns matching lines with context.
- **glob** — Find files matching a glob pattern (e.g., `*.rs`, `**/*.py`).
- **list_directory** — List files and directories recursively. Good for exploring project structure.

### File Modification  
- **edit_file** — Edit files by replacing specific strings. PREFERRED for targeted changes (single function, line fix, etc.). Safer than write_file.
- **write_file** — Write entire file contents. Use only when creating new files or complete rewrites are needed.

### Command Execution
- **bash** — Execute shell commands. Use for running tests, linting, git operations, package installs, etc.

## Tool Usage Best Practices
1. **Always read before editing** — Use `read_file` to understand context first
2. **Prefer edit_file over write_file** — For modifications, use `edit_file` with exact string matching
3. **Use grep to find code** — Before editing, search for the code location with `grep`
4. **Explore with list_directory and glob** — Understand project structure before making changes
5. **Test changes with bash** — Run tests/lints after code modifications"#;
