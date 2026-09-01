// Platform-correct command construction.
//
// Rust owns the argv. Nothing here builds a shell string, and that is the whole
// point.
//
// BUG-COMPILE: `bash.rs` took a recipe's shell line, rewrote its pipes into
// PowerShell, and ran the result under `cmd /C`. The recipes' build step is
//
//     npm run build 2>&1 | tee build.log; echo "EXIT:$?"
//
// and in a pipeline the exit status is the LAST command's. `tee` failing — or
// simply the rewrite losing `$?` — made a *successful* build record as failed,
// which left `qc_review` unreachable on Windows because the build-evidence gate
// never saw a passing build. A build that works is reported as broken, and no
// amount of retrying helps.
//
// The fix is not a better quoting function. It is to stop producing shell
// strings: name the intent, get an argv, spawn it directly, and read the exit
// status of the process you actually launched.
//
// Moves to `autodev-core/platform/` in P1-05.

use std::path::{Path, PathBuf};

/// What the caller wants done, independent of how any toolchain spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandIntent {
    Build,
    Test,
    Lint,
    Run,
}

impl CommandIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Test => "test",
            Self::Lint => "lint",
            Self::Run => "run",
        }
    }
}

/// The toolchains AutoDev knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toolchain {
    Npm,
    Pnpm,
    Yarn,
    Cargo,
    Go,
    Python,
    Make,
}

impl Toolchain {
    /// The program name before any platform shim resolution.
    pub fn program(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "yarn",
            Self::Cargo => "cargo",
            Self::Go => "go",
            Self::Python => "python",
            Self::Make => "make",
        }
    }

    /// True when this program ships on Windows as a `.cmd`/`.bat` shim rather
    /// than a real executable.
    ///
    /// Rust 1.77 stopped implicitly appending `.cmd`/`.bat` when resolving a
    /// program name, for a documented argument-injection reason. So
    /// `Command::new("npm")` now fails outright on Windows with "program not
    /// found", even though `npm` works in every shell on that machine.
    pub fn is_windows_shim(self) -> bool {
        matches!(self, Self::Npm | Self::Pnpm | Self::Yarn)
    }
}

/// A command ready to spawn: a program and its arguments, never a shell line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCommand {
    pub program: String,
    pub args: Vec<String>,
}

impl ResolvedCommand {
    fn new(program: impl Into<String>, args: &[&str]) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
        }
    }

    /// Display form for logs and UI. **Not** for execution — reconstructing a
    /// shell string from argv is how quoting bugs get reintroduced.
    pub fn display(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Build the argv for an intent on a toolchain.
///
/// Returns `None` when the combination has no meaning (`cargo lint` is not a
/// thing; the caller should use clippy, or nothing).
pub fn resolve(toolchain: Toolchain, intent: CommandIntent) -> Option<ResolvedCommand> {
    let program = resolve_program(toolchain);

    let args: &[&str] = match (toolchain, intent) {
        // npm-family: `run` is required for user scripts, `test` is a built-in.
        (Toolchain::Npm, CommandIntent::Build) => &["run", "build"],
        (Toolchain::Npm, CommandIntent::Lint) => &["run", "lint"],
        (Toolchain::Npm, CommandIntent::Run) => &["start"],
        (Toolchain::Npm, CommandIntent::Test) => &["test"],

        (Toolchain::Pnpm, CommandIntent::Build) => &["run", "build"],
        (Toolchain::Pnpm, CommandIntent::Lint) => &["run", "lint"],
        (Toolchain::Pnpm, CommandIntent::Run) => &["start"],
        (Toolchain::Pnpm, CommandIntent::Test) => &["test"],

        (Toolchain::Yarn, CommandIntent::Build) => &["build"],
        (Toolchain::Yarn, CommandIntent::Lint) => &["lint"],
        (Toolchain::Yarn, CommandIntent::Run) => &["start"],
        (Toolchain::Yarn, CommandIntent::Test) => &["test"],

        (Toolchain::Cargo, CommandIntent::Build) => &["build"],
        (Toolchain::Cargo, CommandIntent::Test) => &["test"],
        (Toolchain::Cargo, CommandIntent::Lint) => &["clippy", "--all-targets"],
        (Toolchain::Cargo, CommandIntent::Run) => &["run"],

        (Toolchain::Go, CommandIntent::Build) => &["build", "./..."],
        (Toolchain::Go, CommandIntent::Test) => &["test", "./..."],
        (Toolchain::Go, CommandIntent::Lint) => &["vet", "./..."],
        (Toolchain::Go, CommandIntent::Run) => &["run", "."],

        (Toolchain::Python, CommandIntent::Test) => &["-m", "pytest"],
        (Toolchain::Python, CommandIntent::Lint) => &["-m", "ruff", "check"],
        // Python has no universal build or run step worth guessing at.
        (Toolchain::Python, _) => return None,

        (Toolchain::Make, CommandIntent::Build) => &["build"],
        (Toolchain::Make, CommandIntent::Test) => &["test"],
        (Toolchain::Make, CommandIntent::Lint) => &["lint"],
        (Toolchain::Make, CommandIntent::Run) => &["run"],
    };

    Some(ResolvedCommand::new(program, args))
}

/// Resolve a toolchain's program name to something spawnable on this platform.
///
/// On Windows the npm-family tools are `.cmd` shims. Since Rust 1.77 the
/// standard library no longer appends those extensions when resolving a bare
/// name, so `Command::new("npm")` fails on a machine where `npm` plainly works.
/// Search PATH for the shim and return its full path; fall back to the bare
/// name so the caller gets the OS's own "not found" error rather than ours.
pub fn resolve_program(toolchain: Toolchain) -> String {
    let base = toolchain.program();

    #[cfg(windows)]
    {
        if toolchain.is_windows_shim() {
            if let Some(path) = find_on_path(base, &["cmd", "bat", "exe"]) {
                return path.to_string_lossy().into_owned();
            }
        }
    }

    base.to_string()
}

/// Find `name.<ext>` on PATH, trying each extension in order.
#[cfg_attr(not(windows), allow(dead_code))]
fn find_on_path(name: &str, extensions: &[&str]) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        for ext in extensions {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Guess the toolchain for a project directory from its manifest files.
///
/// Deliberately narrow: it reports what it can see and `None` otherwise, rather
/// than defaulting to npm and producing a confusing failure. Full ecosystem
/// detection is `P4-11`.
pub fn detect_toolchain(project_dir: &Path) -> Option<Toolchain> {
    if project_dir.join("Cargo.toml").is_file() {
        return Some(Toolchain::Cargo);
    }
    if project_dir.join("go.mod").is_file() {
        return Some(Toolchain::Go);
    }
    if project_dir.join("package.json").is_file() {
        // Lockfile decides which npm-family driver actually manages this tree.
        if project_dir.join("pnpm-lock.yaml").is_file() {
            return Some(Toolchain::Pnpm);
        }
        if project_dir.join("yarn.lock").is_file() {
            return Some(Toolchain::Yarn);
        }
        return Some(Toolchain::Npm);
    }
    if project_dir.join("pyproject.toml").is_file() || project_dir.join("setup.py").is_file() {
        return Some(Toolchain::Python);
    }
    if project_dir.join("Makefile").is_file() {
        return Some(Toolchain::Make);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_build_is_run_build_not_a_shell_line() {
        let cmd = resolve(Toolchain::Npm, CommandIntent::Build).expect("npm build");
        assert_eq!(cmd.args, vec!["run", "build"]);

        // The regression this module exists for: no argument may carry a pipe,
        // a redirect, a command separator, or an exit-status expansion. If one
        // does, we are building a shell line again and BUG-COMPILE is back.
        for arg in &cmd.args {
            for forbidden in ['|', '>', '<', ';', '&', '$'] {
                assert!(
                    !arg.contains(forbidden),
                    "argv must never contain shell metacharacters, found {forbidden:?} in {arg:?}"
                );
            }
        }
    }

    #[test]
    fn no_resolved_command_anywhere_contains_shell_metacharacters() {
        let toolchains = [
            Toolchain::Npm,
            Toolchain::Pnpm,
            Toolchain::Yarn,
            Toolchain::Cargo,
            Toolchain::Go,
            Toolchain::Python,
            Toolchain::Make,
        ];
        let intents = [
            CommandIntent::Build,
            CommandIntent::Test,
            CommandIntent::Lint,
            CommandIntent::Run,
        ];

        for tc in toolchains {
            for intent in intents {
                let Some(cmd) = resolve(tc, intent) else {
                    continue;
                };
                for arg in &cmd.args {
                    assert!(
                        !arg.contains(['|', '>', '<', ';', '&', '$', '`']),
                        "{tc:?}/{intent:?} produced a shell-looking argument: {arg:?}"
                    );
                }
                assert!(!cmd.program.is_empty());
            }
        }
    }

    #[test]
    fn npm_test_does_not_take_a_run_prefix() {
        // `npm run test` only works when a "test" script exists; `npm test` is
        // the built-in and also runs the script when present.
        let cmd = resolve(Toolchain::Npm, CommandIntent::Test).expect("npm test");
        assert_eq!(cmd.args, vec!["test"]);
    }

    #[test]
    fn cargo_lint_is_clippy() {
        let cmd = resolve(Toolchain::Cargo, CommandIntent::Lint).expect("cargo lint");
        assert_eq!(cmd.args, vec!["clippy", "--all-targets"]);
    }

    #[test]
    fn python_has_no_invented_build_step() {
        assert!(resolve(Toolchain::Python, CommandIntent::Build).is_none());
        assert!(resolve(Toolchain::Python, CommandIntent::Run).is_none());
        assert!(resolve(Toolchain::Python, CommandIntent::Test).is_some());
    }

    #[test]
    fn only_the_npm_family_needs_a_windows_shim() {
        assert!(Toolchain::Npm.is_windows_shim());
        assert!(Toolchain::Pnpm.is_windows_shim());
        assert!(Toolchain::Yarn.is_windows_shim());
        assert!(!Toolchain::Cargo.is_windows_shim());
        assert!(!Toolchain::Go.is_windows_shim());
        assert!(!Toolchain::Make.is_windows_shim());
    }

    /// On Windows this must not return the bare name for an npm-family tool
    /// when a shim is actually on PATH — `Command::new("npm")` fails there.
    #[test]
    fn windows_resolves_the_npm_shim_when_one_exists() {
        let resolved = resolve_program(Toolchain::Npm);

        if cfg!(windows) && find_on_path("npm", &["cmd", "bat", "exe"]).is_some() {
            assert_ne!(
                resolved, "npm",
                "on Windows npm must resolve to its .cmd shim; the bare name is not spawnable"
            );
            assert!(
                resolved.to_lowercase().ends_with(".cmd")
                    || resolved.to_lowercase().ends_with(".bat")
                    || resolved.to_lowercase().ends_with(".exe")
            );
        } else {
            assert_eq!(resolved, "npm");
        }
    }

    #[test]
    fn non_shim_tools_are_never_rewritten() {
        assert_eq!(resolve_program(Toolchain::Cargo), "cargo");
        assert_eq!(resolve_program(Toolchain::Go), "go");
    }

    #[test]
    fn detection_prefers_the_lockfile_that_is_present() {
        let dir = std::env::temp_dir().join("autodev-platform-detect-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");

        assert_eq!(detect_toolchain(&dir), None, "an empty dir must not guess");

        std::fs::write(dir.join("package.json"), "{}").unwrap();
        assert_eq!(detect_toolchain(&dir), Some(Toolchain::Npm));

        std::fs::write(dir.join("yarn.lock"), "").unwrap();
        assert_eq!(detect_toolchain(&dir), Some(Toolchain::Yarn));

        std::fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(detect_toolchain(&dir), Some(Toolchain::Pnpm));

        // Cargo wins over a co-located package.json — a Tauri app is a Rust
        // project with a frontend, and `cargo build` is the build that matters.
        std::fs::write(dir.join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_toolchain(&dir), Some(Toolchain::Cargo));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn display_is_for_humans_only() {
        let cmd = resolve(Toolchain::Cargo, CommandIntent::Lint).unwrap();
        assert_eq!(cmd.display(), "cargo clippy --all-targets");
    }
}
