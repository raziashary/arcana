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
// Moves to `arcana-core/platform/` in P1-05.

use std::path::{Path, PathBuf};

/// What the caller wants done, independent of how any toolchain spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandIntent {
    Install,
    Build,
    Test,
    Lint,
    Run,
}

impl CommandIntent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Build => "build",
            Self::Test => "test",
            Self::Lint => "lint",
            Self::Run => "run",
        }
    }
}

/// The toolchains Arcana knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toolchain {
    Npm,
    Pnpm,
    Yarn,
    Bun,
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
            Self::Bun => "bun",
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
        // Dependency installation is an explicit intent. It is never inferred
        // from a missing node_modules/ or other dependency sentinel.
        (Toolchain::Npm, CommandIntent::Install) => &["install"],
        (Toolchain::Pnpm, CommandIntent::Install) => &["install"],
        (Toolchain::Yarn, CommandIntent::Install) => &["install"],
        (Toolchain::Bun, CommandIntent::Install) => &["install"],
        (Toolchain::Cargo, CommandIntent::Install) => &["fetch"],
        (Toolchain::Go, CommandIntent::Install) => &["mod", "download"],
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

        (Toolchain::Bun, CommandIntent::Build) => &["run", "build"],
        (Toolchain::Bun, CommandIntent::Lint) => &["run", "lint"],
        (Toolchain::Bun, CommandIntent::Run) => &["run", "start"],
        (Toolchain::Bun, CommandIntent::Test) => &["test"],

        (Toolchain::Cargo, CommandIntent::Build) => &["build"],
        (Toolchain::Cargo, CommandIntent::Test) => &["test"],
        (Toolchain::Cargo, CommandIntent::Lint) => &["clippy", "--all-targets"],
        (Toolchain::Cargo, CommandIntent::Run) => &["run"],

        (Toolchain::Go, CommandIntent::Build) => &["build", "./..."],
        (Toolchain::Go, CommandIntent::Test) => &["test", "./..."],
        (Toolchain::Go, CommandIntent::Lint) => &["vet", "./..."],
        (Toolchain::Go, CommandIntent::Run) => &["run", "."],

        (Toolchain::Python, CommandIntent::Build) => &[
            "-S",
            "-m",
            "compileall",
            "-q",
            "-x",
            r"(^|[\\/])(\.git|\.venv|venv|node_modules|__pycache__|\.arcana-cache)([\\/]|$)",
            ".",
        ],
        (Toolchain::Python, CommandIntent::Test) => &["-m", "pytest"],
        (Toolchain::Python, CommandIntent::Lint) => &["-m", "ruff", "check"],
        // Python's deterministic build intent is standard-library syntax
        // compilation; test and lint remain independent verification intents.
        (Toolchain::Python | Toolchain::Make, CommandIntent::Install) => return None,
        (Toolchain::Python, CommandIntent::Run) => return None,

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
/// than defaulting to npm and producing a confusing failure.
pub fn detect_toolchain(project_dir: &Path) -> Option<Toolchain> {
    detect_toolchains(project_dir, project_dir)
        .ok()?
        .into_iter()
        .next()
        .map(|p| p.toolchain)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedProject {
    pub toolchain: Toolchain,
    pub project_dir: PathBuf,
}

/// Find the nearest project for each supported ecosystem while staying inside
/// the selected workspace. This lets a mixed Rust/Node repository validate
/// both projects independently and lets a nested crate inherit root projects
/// it does not itself define.
pub fn detect_toolchains(
    project_dir: &Path,
    workspace_root: &Path,
) -> Result<Vec<DetectedProject>, String> {
    let project_dir = project_dir.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve project directory {}: {error}",
            project_dir.display()
        )
    })?;
    let workspace_root = workspace_root.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve workspace {}: {error}",
            workspace_root.display()
        )
    })?;
    if !project_dir.starts_with(&workspace_root) {
        return Err("Project directory resolves outside the workspace".into());
    }

    let mut directories = Vec::new();
    let mut current = Some(project_dir.as_path());
    while let Some(dir) = current {
        directories.push(dir.to_path_buf());
        if dir == workspace_root {
            break;
        }
        current = dir
            .parent()
            .filter(|parent| parent.starts_with(&workspace_root));
    }

    directories.extend(discover_project_directories(&project_dir, &workspace_root)?);
    directories.sort();
    directories.dedup();

    let mut found = Vec::new();
    for dir in directories {
        if dir.join("Cargo.toml").is_file() {
            found.push(DetectedProject {
                toolchain: Toolchain::Cargo,
                project_dir: dir.clone(),
            });
        }
        if dir.join("go.mod").is_file() {
            found.push(DetectedProject {
                toolchain: Toolchain::Go,
                project_dir: dir.clone(),
            });
        }
        if dir.join("package.json").is_file() {
            found.push(DetectedProject {
                toolchain: detect_node_manager(&dir, &workspace_root)?,
                project_dir: dir.clone(),
            });
        }
        if dir.join("pyproject.toml").is_file()
            || dir.join("setup.py").is_file()
            || dir.join("requirements.txt").is_file()
        {
            found.push(DetectedProject {
                toolchain: Toolchain::Python,
                project_dir: dir.clone(),
            });
        }
        if dir.join("Makefile").is_file() {
            found.push(DetectedProject {
                toolchain: Toolchain::Make,
                project_dir: dir,
            });
        }
    }
    let cargo_candidates = found
        .iter()
        .filter(|project| project.toolchain == Toolchain::Cargo)
        .cloned()
        .collect::<Vec<_>>();
    let selected_candidates = found.clone();
    found.retain(|project| {
        let shadowed_ancestor = project.project_dir != project_dir
            && project_dir.starts_with(&project.project_dir)
            && selected_candidates.iter().any(|closer| {
                closer.toolchain == project.toolchain
                    && closer.project_dir != project.project_dir
                    && closer.project_dir.starts_with(&project.project_dir)
                    && project_dir.starts_with(&closer.project_dir)
            });
        !shadowed_ancestor
    });
    found.retain(|project| {
        project.toolchain != Toolchain::Cargo
            || !cargo_candidates.iter().any(|workspace| {
                workspace.toolchain == Toolchain::Cargo
                    && workspace.project_dir != project.project_dir
                    && project.project_dir.starts_with(&workspace.project_dir)
                    && cargo_workspace_contains(&workspace.project_dir, &project.project_dir)
            })
    });
    found.sort_by(|left, right| {
        toolchain_order(left.toolchain)
            .cmp(&toolchain_order(right.toolchain))
            .then_with(|| left.project_dir.cmp(&right.project_dir))
    });
    Ok(found)
}

const MAX_DISCOVERED_DIRECTORIES: usize = 20_000;

fn discover_project_directories(
    start: &Path,
    workspace_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    const PRUNED: &[&str] = &[
        ".arcana",
        ".cache",
        ".git",
        ".gradle",
        ".hg",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        ".svn",
        ".next",
        ".turbo",
        ".venv",
        "__pycache__",
        "build",
        "coverage",
        "dist",
        // Checked-in project fixtures exercise discovery itself; they are
        // not production projects to build when scanning a repository root.
        "fixtures",
        "node_modules",
        "target",
        "vendor",
        "venv",
    ];

    let mut found = Vec::new();
    let mut pending = vec![start.to_path_buf()];
    let mut visited = std::collections::HashSet::new();
    while let Some(dir) = pending.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        if found.len() >= MAX_DISCOVERED_DIRECTORIES {
            return Err(format!(
                "Project discovery exceeded the {MAX_DISCOVERED_DIRECTORIES}-directory safety limit under {}",
                start.display()
            ));
        }
        found.push(dir.clone());
        let entries = std::fs::read_dir(&dir).map_err(|error| {
            format!(
                "Cannot inspect project directory {}: {error}",
                dir.display()
            )
        })?;
        let mut children = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "Cannot read an entry in project directory {}: {error}",
                    dir.display()
                )
            })?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("Cannot inspect {}: {error}", entry.path().display()))?;
            if !file_type.is_dir() || file_type.is_symlink() {
                continue;
            }
            if PRUNED.contains(&entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            let child = entry.path().canonicalize().map_err(|error| {
                format!(
                    "Cannot resolve project directory {}: {error}",
                    entry.path().display()
                )
            })?;
            if child.starts_with(workspace_root) {
                children.push(child);
            }
        }
        children.sort();
        pending.extend(children.into_iter().rev());
    }
    Ok(found)
}

fn cargo_workspace_contains(workspace_dir: &Path, candidate_dir: &Path) -> bool {
    let manifest_path = workspace_dir.join("Cargo.toml");
    let Ok(contents) = std::fs::read_to_string(manifest_path) else {
        return false;
    };
    let Ok(manifest) = contents.parse::<toml::Value>() else {
        return false;
    };
    let Some(members) = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("members"))
        .and_then(toml::Value::as_array)
    else {
        return false;
    };
    let Ok(relative) = candidate_dir.strip_prefix(workspace_dir) else {
        return false;
    };
    let relative = relative.to_string_lossy().replace('\\', "/");
    let excluded = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .is_some_and(|patterns| {
            patterns
                .iter()
                .filter_map(toml::Value::as_str)
                .any(|pattern| {
                    glob::Pattern::new(pattern).is_ok_and(|pattern| pattern.matches(&relative))
                })
        });
    !excluded
        && members
            .iter()
            .filter_map(toml::Value::as_str)
            .any(|pattern| {
                glob::Pattern::new(pattern).is_ok_and(|pattern| pattern.matches(&relative))
            })
}

pub fn parse_toolchain_selector(selector: &str) -> Option<Toolchain> {
    match selector.trim().to_ascii_lowercase().as_str() {
        "npm" => Some(Toolchain::Npm),
        "pnpm" => Some(Toolchain::Pnpm),
        "yarn" => Some(Toolchain::Yarn),
        "bun" => Some(Toolchain::Bun),
        "cargo" | "rust" => Some(Toolchain::Cargo),
        "go" | "golang" => Some(Toolchain::Go),
        "python" | "py" => Some(Toolchain::Python),
        "make" => Some(Toolchain::Make),
        _ => None,
    }
}

fn toolchain_order(toolchain: Toolchain) -> u8 {
    match toolchain {
        Toolchain::Cargo => 0,
        Toolchain::Go => 1,
        Toolchain::Npm => 2,
        Toolchain::Pnpm => 3,
        Toolchain::Yarn => 4,
        Toolchain::Bun => 5,
        Toolchain::Python => 6,
        Toolchain::Make => 7,
    }
}

fn detect_node_manager(package_dir: &Path, workspace_root: &Path) -> Result<Toolchain, String> {
    let manifest_path = package_dir.join("package.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest_path)
            .map_err(|error| format!("Cannot read {}: {error}", manifest_path.display()))?,
    )
    .map_err(|error| format!("Invalid {}: {error}", manifest_path.display()))?;
    if let Some(package_manager) = manifest.get("packageManager").and_then(|v| v.as_str()) {
        let manager = package_manager.split('@').next().unwrap_or_default();
        return parse_toolchain_selector(manager).filter(|toolchain| {
            matches!(toolchain, Toolchain::Npm | Toolchain::Pnpm | Toolchain::Yarn | Toolchain::Bun)
        }).ok_or_else(|| format!("Unsupported packageManager '{package_manager}' in {}. Supported managers are npm, pnpm, yarn, and bun.", manifest_path.display()));
    }

    let mut lockfiles = [false; 4];
    let mut current = Some(package_dir);
    while let Some(dir) = current {
        lockfiles[0] |= dir.join("pnpm-lock.yaml").is_file();
        lockfiles[1] |= dir.join("yarn.lock").is_file();
        lockfiles[2] |=
            dir.join("package-lock.json").is_file() || dir.join("npm-shrinkwrap.json").is_file();
        lockfiles[3] |= dir.join("bun.lock").is_file() || dir.join("bun.lockb").is_file();
        if dir == workspace_root {
            break;
        }
        current = dir
            .parent()
            .filter(|parent| parent.starts_with(workspace_root));
    }
    Ok(if lockfiles[0] {
        Toolchain::Pnpm
    } else if lockfiles[1] {
        Toolchain::Yarn
    } else if lockfiles[2] {
        Toolchain::Npm
    } else if lockfiles[3] {
        Toolchain::Bun
    } else {
        Toolchain::Npm
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npm_build_is_run_build_not_a_shell_line() {
        let cmd = resolve(Toolchain::Npm, CommandIntent::Build).expect("npm build");
        assert_eq!(cmd.args, vec!["run", "build"]);

        // The regression this module exists for: no argument may carry a pipe,
        // a redirect, or a command separator. `$` and other punctuation can be
        // ordinary argv data (for example a Python path-filter regex); they do
        // not become shell syntax because these commands are launched directly.
        for arg in &cmd.args {
            for forbidden in ['|', '>', '<', ';', '&'] {
                assert!(
                    !arg.contains(forbidden),
                    "argv must never contain shell metacharacters, found {forbidden:?} in {arg:?}"
                );
            }
        }
    }

    #[test]
    fn resolved_commands_are_program_argv_not_shell_invocations() {
        let toolchains = [
            Toolchain::Npm,
            Toolchain::Pnpm,
            Toolchain::Yarn,
            Toolchain::Bun,
            Toolchain::Cargo,
            Toolchain::Go,
            Toolchain::Python,
            Toolchain::Make,
        ];
        let intents = [
            CommandIntent::Build,
            CommandIntent::Install,
            CommandIntent::Test,
            CommandIntent::Lint,
            CommandIntent::Run,
        ];

        for tc in toolchains {
            for intent in intents {
                let Some(cmd) = resolve(tc, intent) else {
                    continue;
                };
                assert!(!cmd.program.is_empty());
                let executable = Path::new(&cmd.program)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&cmd.program)
                    .to_ascii_lowercase();
                assert!(
                    !matches!(
                        executable.as_str(),
                        "cmd"
                            | "cmd.exe"
                            | "powershell"
                            | "powershell.exe"
                            | "pwsh"
                            | "pwsh.exe"
                            | "sh"
                            | "bash"
                    ),
                    "{tc:?}/{intent:?} resolved to a shell executable: {cmd:?}"
                );
                assert!(
                    !cmd.args
                        .iter()
                        .any(|arg| { matches!(arg.as_str(), "/c" | "/C" | "-c" | "-Command") }),
                    "{tc:?}/{intent:?} resolved to a shell command switch: {cmd:?}"
                );
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
    fn python_build_is_standard_library_compilation_and_keeps_tests_separate() {
        let build = resolve(Toolchain::Python, CommandIntent::Build).expect("python build");
        assert_eq!(
            build.args,
            vec![
                "-S",
                "-m",
                "compileall",
                "-q",
                "-x",
                r"(^|[\\/])(\.git|\.venv|venv|node_modules|__pycache__|\.arcana-cache)([\\/]|$)",
                "."
            ]
        );
        assert!(resolve(Toolchain::Python, CommandIntent::Run).is_none());
        assert_eq!(
            resolve(Toolchain::Python, CommandIntent::Test)
                .expect("python tests")
                .args,
            vec!["-m", "pytest"]
        );
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
        let dir =
            std::env::temp_dir().join(format!("arcana-platform-detect-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("mkdir");

        assert!(
            detect_toolchains(&dir, &dir).unwrap().is_empty(),
            "an empty dir must not guess"
        );

        std::fs::write(dir.join("package.json"), "{}").unwrap();
        std::fs::write(dir.join("yarn.lock"), "").unwrap();
        std::fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(dir.join("package-lock.json"), "{}").unwrap();
        assert_eq!(
            detect_toolchains(&dir, &dir).unwrap()[0].toolchain,
            Toolchain::Pnpm
        );

        std::fs::write(
            dir.join("package.json"),
            r#"{"packageManager":"npm@11.0.0"}"#,
        )
        .unwrap();
        assert_eq!(
            detect_toolchains(&dir, &dir).unwrap()[0].toolchain,
            Toolchain::Npm
        );

        std::fs::write(
            dir.join("package.json"),
            r#"{"packageManager":"mystery@1.0.0"}"#,
        )
        .unwrap();
        assert!(detect_toolchains(&dir, &dir)
            .unwrap_err()
            .contains("Unsupported packageManager"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_nested_project_falls_back_to_ancestor_candidates_and_runs_each_ecosystem() {
        let root =
            std::env::temp_dir().join(format!("arcana-platform-mixed-{}", uuid::Uuid::new_v4()));
        let nested = root.join("src-tauri");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("package.json"), "{}").unwrap();
        std::fs::write(root.join("package-lock.json"), "{}").unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        std::fs::write(
            nested.join("Cargo.toml"),
            "[package]\nname='nested'\nversion='0.1.0'\n",
        )
        .unwrap();

        let projects = detect_toolchains(&nested, &root).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(
            projects[0],
            DetectedProject {
                toolchain: Toolchain::Cargo,
                project_dir: nested.canonicalize().unwrap()
            }
        );
        assert_eq!(
            projects[1],
            DetectedProject {
                toolchain: Toolchain::Npm,
                project_dir: root.canonicalize().unwrap()
            }
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn workspace_discovery_finds_nested_packages_and_prunes_generated_and_fixture_trees() {
        let root = std::env::temp_dir().join(format!(
            "arcana-platform-discovery-{}",
            uuid::Uuid::new_v4()
        ));
        let frontend = root.join("frontend");
        let docs = root.join("docs");
        let generated = root.join("node_modules").join("ignored-package");
        let fixture = root.join("tests").join("fixtures").join("fx-repo-node");
        std::fs::create_dir_all(&frontend).unwrap();
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::create_dir_all(&generated).unwrap();
        std::fs::create_dir_all(&fixture).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        let cargo_member = root.join("crates").join("arcana-core");
        std::fs::create_dir_all(&cargo_member).unwrap();
        std::fs::write(
            cargo_member.join("Cargo.toml"),
            "[package]\nname='arcana-core'\nversion='0.1.0'\n",
        )
        .unwrap();
        std::fs::write(frontend.join("package.json"), "{}\n").unwrap();
        std::fs::write(frontend.join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(
            docs.join("package.json"),
            r#"{"packageManager":"yarn@4.0.0"}"#,
        )
        .unwrap();
        std::fs::write(docs.join("yarn.lock"), "").unwrap();
        std::fs::write(generated.join("package.json"), "{}\n").unwrap();
        std::fs::write(fixture.join("package.json"), "{}\n").unwrap();

        let projects = detect_toolchains(&root, &root).expect("discover workspace projects");
        assert_eq!(
            projects,
            vec![
                DetectedProject {
                    toolchain: Toolchain::Cargo,
                    project_dir: root.canonicalize().unwrap(),
                },
                DetectedProject {
                    toolchain: Toolchain::Pnpm,
                    project_dir: frontend.canonicalize().unwrap(),
                },
                DetectedProject {
                    toolchain: Toolchain::Yarn,
                    project_dir: docs.canonicalize().unwrap(),
                },
            ],
            "workspace members collapse to their Cargo root, separate Node packages are detected, and generated dependencies/test fixtures are ignored"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn selector_is_validated_and_explicit_installs_have_fixed_argv() {
        assert_eq!(parse_toolchain_selector("rust"), Some(Toolchain::Cargo));
        assert_eq!(parse_toolchain_selector("pnpm"), Some(Toolchain::Pnpm));
        assert_eq!(parse_toolchain_selector("unknown"), None);
        assert_eq!(
            resolve(Toolchain::Pnpm, CommandIntent::Install)
                .unwrap()
                .args,
            vec!["install"]
        );
        assert_eq!(
            resolve(Toolchain::Cargo, CommandIntent::Install)
                .unwrap()
                .args,
            vec!["fetch"]
        );
        assert_eq!(
            resolve(Toolchain::Go, CommandIntent::Install).unwrap().args,
            vec!["mod", "download"]
        );
        assert!(resolve(Toolchain::Python, CommandIntent::Install).is_none());
    }

    #[test]
    fn display_is_for_humans_only() {
        let cmd = resolve(Toolchain::Cargo, CommandIntent::Lint).unwrap();
        assert_eq!(cmd.display(), "cargo clippy --all-targets");
    }
}
