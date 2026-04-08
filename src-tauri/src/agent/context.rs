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

## Tool Usage
- Use read_file to understand existing code before changes
- Use list_directory to explore project structure
- Use bash for running tests, linting, and build commands
- Use write_file only when you have full context of what to write"#;
