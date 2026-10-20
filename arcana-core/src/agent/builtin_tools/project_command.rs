use async_trait::async_trait;
use serde_json::{json, Value};
#[cfg(windows)]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};

use super::build_evidence::BuildEvidenceKind;
use super::path_jail::validate_workspace_path;
use crate::agent::tool_trait::{Tool, ToolContext, ToolResult};
use crate::platform::command::{self, CommandIntent, Toolchain};
use crate::platform::process_sandbox::{self, ProcessSpec};

const DEFAULT_TIMEOUT_SECS: u64 = 3_600;
const MAX_TIMEOUT_SECS: u64 = 7_200;
const MAX_OUTPUT_BYTES: usize = 50_000;
const MAX_STREAM_BYTES: usize = MAX_OUTPUT_BYTES / 2;
const MAX_SAFE_DIAGNOSTICS: usize = 24;

pub struct ProjectCommandTool;

/// Result of a user-initiated, bounded IDE Run action. This path deliberately
/// does not produce task build evidence or impersonate a worker claim.
#[derive(Debug, serde::Serialize)]
pub struct IdeRunResult {
    pub toolchain: String,
    pub project_path: String,
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run the project containing an opened IDE file through the same fixed-argv
/// resolver and OS sandbox used by the worker's project command. Only an
/// explicit user action reaches this entry point; agent calls still require a
/// current task claim through `ProjectCommandTool`.
pub async fn run_ide_project(
    workspace: &Path,
    relative_file: &str,
    selector: Option<&str>,
) -> Result<IdeRunResult, String> {
    let workspace = workspace
        .canonicalize()
        .map_err(|error| format!("Cannot resolve selected workspace: {error}"))?;
    let supplied = Path::new(relative_file);
    if supplied.is_absolute() || relative_file.trim().is_empty() {
        return Err("IDE Run requires an opened workspace-relative file".into());
    }
    let file = validate_workspace_path(&workspace.join(supplied), &workspace.to_string_lossy())?
        .canonicalize()
        .map_err(|error| format!("Cannot resolve opened IDE file: {error}"))?;
    if !file.is_file() {
        return Err("IDE Run target is not an existing workspace file".into());
    }
    let directory = file
        .parent()
        .ok_or_else(|| "IDE Run target has no project directory".to_string())?;
    let detected = command::detect_toolchains(directory, &workspace)?;
    // Discovery also scans descendants. They are not projects of the opened
    // file, even when they happen to live beneath its containing directory.
    let mut candidates = detected
        .into_iter()
        .filter(|project| file.starts_with(&project.project_dir))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err("No supported project manifest contains the opened file".into());
    }
    if let Some(selector) = selector {
        let selected = command::parse_toolchain_selector(selector)
            .ok_or_else(|| format!("Unknown IDE Run toolchain selector '{selector}'"))?;
        candidates.retain(|project| project.toolchain == selected);
        if candidates.is_empty() {
            return Err(format!("No {selected:?} project contains the opened file"));
        }
    } else if let Some(preferred) = preferred_ide_toolchain(&file) {
        if candidates
            .iter()
            .any(|project| ide_toolchain_matches(preferred, project.toolchain))
        {
            candidates.retain(|project| ide_toolchain_matches(preferred, project.toolchain));
        }
    }
    candidates.sort_by(|left, right| {
        right
            .project_dir
            .components()
            .count()
            .cmp(&left.project_dir.components().count())
    });
    if candidates.len() > 1
        && candidates[0].project_dir.components().count()
            == candidates[1].project_dir.components().count()
    {
        return Err(format!(
            "Opened file belongs to multiple detected projects ({}). Select a toolchain to run.",
            format_detected_projects(&candidates)
        ));
    }
    let project = &candidates[0];
    let resolved = command::resolve(project.toolchain, CommandIntent::Run).ok_or_else(|| {
        format!(
            "No validated Run intent exists for {:?} at {}",
            project.toolchain,
            workspace_relative_path(&workspace, &project.project_dir)
        )
    })?;
    let _workspace_lease = process_sandbox::retain_workspace(&workspace);
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        run_resolved_command_inner(&resolved, &project.project_dir, &workspace, &[], None),
    )
    .await
    .map_err(|_| {
        "IDE Run reached its 120-second limit; the sandboxed process tree was stopped".to_string()
    })?
    .map_err(|error| format!("IDE Run could not launch in the workspace sandbox: {error}"))?;
    Ok(IdeRunResult {
        toolchain: format!("{:?}", project.toolchain).to_ascii_lowercase(),
        project_path: workspace_relative_path(&workspace, &project.project_dir),
        command: resolved.display(),
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
    })
}

fn preferred_ide_toolchain(file: &Path) -> Option<Toolchain> {
    match file.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "rs" => Some(Toolchain::Cargo),
        "go" => Some(Toolchain::Go),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" => Some(Toolchain::Npm),
        "py" => Some(Toolchain::Python),
        _ => None,
    }
}

/// A fixed-argv `.cmd` manager runs behind `cmd.exe`; the sandbox otherwise
/// sees only the system shell and misses the manager/runtime files. Grant the
/// selected installation directory only for an OS-managed installation.
#[cfg(windows)]
fn trusted_windows_shim_runtime(program: &OsStr) -> io::Result<PathBuf> {
    let shim = Path::new(program).canonicalize()?;
    if !shim.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected package-manager shim is not a file",
        ));
    }
    let parent = shim.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected shim has no runtime directory",
        )
    })?;
    let trusted = ["ProgramFiles", "ProgramFiles(x86)"]
        .into_iter()
        .filter_map(std::env::var_os)
        .filter_map(|path| PathBuf::from(path).canonicalize().ok())
        .any(|root| parent.starts_with(root));
    if !trusted {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "selected package-manager shim is outside an approved system runtime; stage it explicitly",
        ));
    }
    Ok(parent.to_path_buf())
}

fn ide_toolchain_matches(preferred: Toolchain, detected: Toolchain) -> bool {
    if preferred == Toolchain::Npm {
        matches!(
            detected,
            Toolchain::Npm | Toolchain::Pnpm | Toolchain::Yarn | Toolchain::Bun
        )
    } else {
        preferred == detected
    }
}

fn bounded_timeout_secs(requested: Option<u64>) -> u64 {
    requested
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS)
}

#[async_trait]
impl Tool for ProjectCommandTool {
    fn effect(&self) -> crate::agent::tool_effect::ToolEffect {
        crate::agent::tool_effect::ToolEffect::Execute
    }

    fn effect_for_input(&self, input: &Value) -> crate::agent::tool_effect::ToolEffect {
        if input["intent"].as_str() == Some("install") {
            crate::agent::tool_effect::ToolEffect::HostApproval
        } else {
            self.effect()
        }
    }

    fn name(&self) -> &str {
        "project_command"
    }

    fn description(&self) -> &str {
        "Run an explicit install, build, test, lint, or run intent. Rust discovers supported projects under the selected workspace, detects package managers, and constructs fixed argv. An optional toolchain selector runs every detected project for that ecosystem; without it, each detected project runs separately. Project dependency installs remain fail-closed until locked staging is available. The only current host-tool install is `intent=install`, `toolchain=python`, `package=ruff`, which requires per-call approval and stages a pinned Ruff binary in Arcana's runtime cache."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "intent": {
                    "type": "string",
                    "enum": ["install", "build", "test", "lint", "run"],
                    "description": "What to do with the detected project. Install is never run implicitly."
                },
                "toolchain": {
                    "type": "string",
                    "enum": ["cargo", "go", "npm", "pnpm", "yarn", "bun", "python", "make"],
                    "description": "Optional validated selector for an ecosystem. Every detected project for that ecosystem runs; without it, each detected project runs separately."
                },
                "package": {
                    "type": "string",
                    "enum": ["ruff"],
                    "description": "Only used with install + python to stage Arcana's pinned Ruff linter after per-call host approval."
                },
                "timeout_secs": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TIMEOUT_SECS,
                    "description": "Optional process timeout in seconds; defaults to 3600 and is capped at 7200."
                },
                "project_path": {
                    "type": "string",
                    "description": "Optional path relative to the worker workspace for a nested package. Absolute paths and paths escaping the workspace are rejected."
                }
            },
            "required": ["intent"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> ToolResult {
        let intent = match input["intent"].as_str() {
            Some("install") => CommandIntent::Install,
            Some("build") => CommandIntent::Build,
            Some("test") => CommandIntent::Test,
            Some("lint") => CommandIntent::Lint,
            Some("run") => CommandIntent::Run,
            Some(other) => {
                return ToolResult::error(format!(
                    "Unknown project intent '{other}'. Choose install, build, test, lint, or run."
                ))
            }
            None => return ToolResult::error("Missing required field: intent"),
        };
        // No intent may produce a host effect before the caller's claim is proven.
        // Only the pinned-Ruff branch below performs an effect at this point
        // (a network download plus a tool-cache write); the other install
        // requests are refused outright further down before any effect.
        if intent == CommandIntent::Install
            && input["toolchain"].as_str() == Some("python")
            && input["package"].as_str() == Some("ruff")
        {
            // This is the one install path that reaches the host: a network
            // download plus a write into Arcana's tool cache. It must prove the
            // caller's claim first; the plain install denial below is a pure
            // refusal and needs no claim.
            if let Err(message) = ctx.require_current_task_claim() {
                return ToolResult::error(message);
            }
            return install_pinned_ruff_for_host().await;
        }
        if intent == CommandIntent::Install {
            tracing::warn!(
                target: "arcana::telemetry",
                event_type = "project_command_denied",
                intent = intent.as_str(),
                tool_call_id = %ctx.call_id,
                task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                reason = "locked_dependency_staging_unavailable",
                "project dependency installation denied because locked host-controlled staging is not available"
            );
            return ToolResult::sandbox_violation(
                "network",
                "project_command install",
                false,
                "Arcana cannot yet stage locked project dependencies through a host-controlled path. Do not retry with Bash or grant build/test children network access; retry after locked dependency staging is implemented.",
            );
        }
        let timeout_secs = bounded_timeout_secs(input["timeout_secs"].as_u64());

        let workspace_root =
            match validate_workspace_path(Path::new(&ctx.working_dir), &ctx.working_dir) {
                Ok(path) => path,
                Err(error) => return ToolResult::error(error),
            };
        let project_dir =
            match resolve_project_directory(&workspace_root, input["project_path"].as_str()) {
                Ok(path) => path,
                Err(error) => return ToolResult::error(error),
            };
        let detected = match command::detect_toolchains(&project_dir, &workspace_root) {
            Ok(projects) => projects,
            Err(error) => return ToolResult::error(error),
        };
        if detected.is_empty() {
            return ToolResult::error(format!(
                "No supported project manifest found from {} through workspace root {}. Supported signals are Cargo.toml, go.mod, package.json, pyproject.toml, setup.py, and Makefile.",
                project_dir.display(), workspace_root.display()
            ));
        }
        let selector = match input["toolchain"].as_str() {
            Some(value) => match command::parse_toolchain_selector(value) {
                Some(toolchain) => Some(toolchain),
                None => return ToolResult::error(format!("Unknown toolchain '{value}'. Choose cargo, go, npm, pnpm, yarn, bun, python, or make.")),
            },
            None => None,
        };
        let projects = match select_projects(detected, selector) {
            Ok(projects) => projects,
            Err(error) => return ToolResult::error(error),
        };

        if let Err(message) = ctx.require_current_task_claim() {
            return ToolResult::error(message);
        }
        let mut consecutive_build_failures = if intent == CommandIntent::Build {
            let Some(task_id) = ctx.task_id else {
                return ToolResult::categorized_error(
                    "build_failure_state_unavailable",
                    "task_state",
                    "Arcana cannot verify the current task's observed build-failure count.",
                    "Do not start another build. Report the state-read failure to the Supervisor so it can decide how to proceed.",
                );
            };
            match crate::db::tasks::TasksDb::open().and_then(|db| db.get_retry_count(task_id)) {
                Ok(count) => count,
                Err(error) => {
                    tracing::error!(
                        target: "arcana::build_evidence",
                        task_id,
                        error = %error,
                        "could not read build-failure streak before project command"
                    );
                    return ToolResult::categorized_error(
                        "build_failure_state_unavailable",
                        "task_state",
                        "Arcana cannot verify the current task's observed build-failure count.",
                        "Do not start another build. Report the state-read failure to the Supervisor so it can decide how to proceed.",
                    );
                }
            }
        } else {
            0
        };
        if selector.is_some() {
            if let Some(project) = projects
                .iter()
                .find(|project| command::resolve(project.toolchain, intent).is_none())
            {
                return ToolResult::error(format!(
                    "No supported '{}' intent for selected {:?} project at {}.",
                    intent.as_str(),
                    project.toolchain,
                    workspace_relative_path(&workspace_root, &project.project_dir)
                ));
            }
        }
        let expected_build_count = if intent == CommandIntent::Build {
            projects
                .iter()
                .filter(|project| command::resolve(project.toolchain, intent).is_some())
                .count()
        } else {
            0
        };
        let initial_build_fingerprint = (intent == CommandIntent::Build)
            .then(|| crate::platform::workspace_fingerprint::fingerprint(&workspace_root).ok())
            .flatten();
        let build_batch = if intent == CommandIntent::Build && expected_build_count > 0 {
            match super::build_evidence::begin_project_build_batch_for_worker(
                &ctx.agent_role,
                &ctx.session_id,
                &workspace_root,
                expected_build_count,
                initial_build_fingerprint.as_deref(),
            ) {
                Ok(Some(batch)) => Some(batch),
                Ok(None) => {
                    return ToolResult::error(
                        "Arcana could not attribute the selected project build batch to one claimed task; no child process was started.",
                    )
                }
                Err(error) => {
                    if error.to_string().contains("build_diagnosis_required") {
                        return ToolResult::categorized_error(
                            "build_diagnosis_required",
                            "command_policy",
                            "Three consecutive observed build commands have failed for this task; no unused Supervisor approval exists.",
                            "Send a source-free diagnosis with workspace-relative locations using request_supervisor_action(request_type='build_retry_approval'), then wait. The Supervisor can authorize exactly one additional Rust-owned build batch.",
                        );
                    }
                    tracing::error!(
                        target: "arcana::build_evidence",
                        task_id = ?ctx.task_id,
                        session_id = %ctx.session_id,
                        error = %error,
                        "project build batch could not be persisted before process start"
                    );
                    return ToolResult::error(
                        "Arcana could not persist the project build batch before process start; no child process was started.",
                    );
                }
            }
        } else {
            None
        };
        let authorized_retry_batch = build_batch
            .as_ref()
            .is_some_and(|batch| batch.supervisor_retry_approved);
        let mut cancellation_guard = build_batch.as_ref().map(|batch| {
            super::build_evidence::ProjectBuildBatchCancellationGuard::new(
                batch.id,
                &ctx.agent_role,
                &ctx.session_id,
                &workspace_root,
            )
        });
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "project_command_start",
            intent = intent.as_str(),
            agent_role = %ctx.agent_role,
            task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
            project_count = projects.len(),
            "validated project command started"
        );

        let mut result = Vec::new();
        let mut failure_count = 0usize;
        let mut failure_codes = Vec::new();
        let mut safe_diagnostics = Vec::new();
        let mut build_attempts = Vec::new();
        let mut sandbox_denial = None;
        let mut timed_out = false;
        let mut last_failed_build = None;
        let mut run_count = 0usize;
        let mut diagnosis_cap_reached_in_batch = false;
        let build_workspace_fingerprint = initial_build_fingerprint;
        let mut build_workspace_fingerprint_unstable = false;
        for project in &projects {
            if intent == CommandIntent::Build
                && consecutive_build_failures >= 3
                && !authorized_retry_batch
            {
                break;
            }
            let Some(resolved) = command::resolve(project.toolchain, intent) else {
                let message = format!(
                    "Skipped {:?} at {}: no supported '{}' intent.",
                    project.toolchain,
                    project.project_dir.display(),
                    intent.as_str()
                );
                if selector.is_some() {
                    return ToolResult::error(message);
                }
                result.push(message);
                continue;
            };
            run_count += 1;
            let display = resolved.display();
            let workspace_fingerprint = if intent == CommandIntent::Build {
                let fingerprint_started = std::time::Instant::now();
                let fingerprint =
                    crate::platform::workspace_fingerprint::fingerprint(&workspace_root).ok();
                if fingerprint != build_workspace_fingerprint {
                    build_workspace_fingerprint_unstable = true;
                }
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "workspace_fingerprint",
                    intent = intent.as_str(),
                    task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                    session_id = %ctx.session_id,
                    tool_call_id = %ctx.call_id,
                    duration_ns = fingerprint_started.elapsed().as_nanos() as u64,
                    outcome = if fingerprint.is_some() { "success" } else { "failure" },
                    "Workspace build-input fingerprint computed"
                );
                fingerprint
            } else {
                None
            };
            let command_started = std::time::Instant::now();
            let outcome = tokio::time::timeout(
                Duration::from_secs(timeout_secs),
                run_resolved_command_for_call(
                    &resolved,
                    &project.project_dir,
                    &workspace_root,
                    ctx,
                    intent,
                    project.toolchain,
                ),
            )
            .await;
            if intent == CommandIntent::Build {
                let post_build_started = std::time::Instant::now();
                let post_build_fingerprint =
                    crate::platform::workspace_fingerprint::fingerprint(&workspace_root).ok();
                let matches_baseline = workspace_fingerprint_matches(
                    build_workspace_fingerprint.as_deref(),
                    post_build_fingerprint.as_deref(),
                );
                if !matches_baseline {
                    build_workspace_fingerprint_unstable = true;
                }
                tracing::info!(
                    target: "arcana::telemetry",
                    event_type = "workspace_fingerprint",
                    intent = intent.as_str(),
                    task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                    session_id = %ctx.session_id,
                    tool_call_id = %ctx.call_id,
                    duration_ns = post_build_started.elapsed().as_nanos() as u64,
                    outcome = if matches_baseline { "unchanged" } else { "changed_or_unavailable" },
                    "Workspace fingerprint checked after project build attempt"
                );
            }
            let (output, exit_code) = match outcome {
                Err(_) => {
                    timed_out = true;
                    tracing::info!(
                        target: "arcana::telemetry",
                        event_type = "project_command_result",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                        session_id = %ctx.session_id,
                        tool_call_id = %ctx.call_id,
                        duration_ns = command_started.elapsed().as_nanos() as u64,
                        outcome = "timeout",
                        "Validated project command timed out"
                    );
                    tracing::warn!(
                        target: "arcana::project_command",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = ?ctx.task_id,
                        tool_call_id = %ctx.call_id,
                        elapsed_ms = command_started.elapsed().as_millis() as u64,
                        "validated project command timed out"
                    );
                    failure_count += 1;
                    failure_codes.push("operation_timed_out");
                    result.push(format!(
                        "Project command for {:?} at {} exceeded its {timeout_secs}s timeout.",
                        project.toolchain,
                        workspace_relative_path(&workspace_root, &project.project_dir)
                    ));
                    continue;
                }
                Ok(Err(error)) => {
                    tracing::info!(
                        target: "arcana::telemetry",
                        event_type = "project_command_result",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                        session_id = %ctx.session_id,
                        tool_call_id = %ctx.call_id,
                        duration_ns = command_started.elapsed().as_nanos() as u64,
                        outcome = "spawn_error",
                        "Validated project command could not start"
                    );
                    tracing::error!(
                        target: "arcana::project_command",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = ?ctx.task_id,
                        elapsed_ms = command_started.elapsed().as_millis() as u64,
                        error = %error,
                        "validated project command failed to spawn"
                    );
                    if error.kind() == io::ErrorKind::PermissionDenied {
                        sandbox_denial = Some((
                            "sandbox_setup",
                            format!(
                                "project_command {:?} {}",
                                project.toolchain,
                                intent.as_str()
                            ),
                        ));
                    }
                    failure_count += 1;
                    failure_codes.push("command_spawn_failed");
                    result.push(format!(
                        "Could not start the {:?} command for project {}.",
                        project.toolchain,
                        workspace_relative_path(&workspace_root, &project.project_dir)
                    ));
                    continue;
                }
                Ok(Ok(output)) => {
                    let exit_code = output.exit_code;
                    tracing::info!(
                        target: "arcana::telemetry",
                        event_type = "project_command_result",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                        session_id = %ctx.session_id,
                        tool_call_id = %ctx.call_id,
                        exit_code,
                        stdout_bytes = output.stdout_total,
                        stderr_bytes = output.stderr_total,
                        runtime_prepare_ms = output.runtime_prepare.as_millis() as u64,
                        sandbox_prepare_ms = output.sandbox_prepare.as_millis() as u64,
                        process_execution_ms = output.process_execution.as_millis() as u64,
                        output_drain_ms = output.output_drain.as_millis() as u64,
                        duration_ns = command_started.elapsed().as_nanos() as u64,
                        outcome = if exit_code == 0 { "success" } else { "failed" },
                        "Validated project command returned"
                    );
                    tracing::info!(
                        target: "arcana::project_command",
                        intent = intent.as_str(),
                        toolchain = ?project.toolchain,
                        task_id = ?ctx.task_id,
                        tool_call_id = %ctx.call_id,
                        exit_code,
                        elapsed_ms = command_started.elapsed().as_millis() as u64,
                        stdout_bytes = output.stdout_total,
                        stderr_bytes = output.stderr_total,
                        runtime_prepare_ms = output.runtime_prepare.as_millis() as u64,
                        sandbox_prepare_ms = output.sandbox_prepare.as_millis() as u64,
                        process_execution_ms = output.process_execution.as_millis() as u64,
                        output_drain_ms = output.output_drain.as_millis() as u64,
                        "validated project command finished"
                    );
                    (output, exit_code)
                }
            };
            if intent == CommandIntent::Build {
                let attempt = (
                    display.clone(),
                    exit_code,
                    project.toolchain,
                    workspace_fingerprint,
                );
                if let Some(guard) = cancellation_guard.as_mut() {
                    guard.record_observed_exit(
                        &attempt.0,
                        attempt.1,
                        evidence_kind(attempt.2).expect("build evidence kind is supported"),
                        attempt.3.clone(),
                    );
                }
                build_attempts.push(attempt);
            }
            if exit_code != 0 {
                failure_count += 1;
                failure_codes.push("project_command_failed");
                safe_diagnostics.extend(source_free_diagnostics(
                    &output.stdout,
                    &output.stderr,
                    &project.project_dir,
                ));
                if intent == CommandIntent::Build {
                    last_failed_build = Some(build_attempts.len() - 1);
                    consecutive_build_failures += 1;
                    diagnosis_cap_reached_in_batch =
                        consecutive_build_failures >= 3 && !authorized_retry_batch;
                }
                result.push(format!(
                    "Project: {} ({:?})\nExit code: {exit_code}\n{} safe diagnostic record(s).",
                    workspace_relative_path(&workspace_root, &project.project_dir),
                    project.toolchain,
                    source_free_diagnostics(&output.stdout, &output.stderr, &project.project_dir,)
                        .len()
                ));
            } else {
                if intent == CommandIntent::Build {
                    consecutive_build_failures = 0;
                }
                let mut item = format!(
                    "Project: {} ({:?})\nCommand: {display}\nExit code: {exit_code}",
                    project.project_dir.display(),
                    project.toolchain
                );
                if !output.stdout.is_empty() {
                    item.push_str("\n=== STDOUT ===\n");
                    item.push_str(&output.stdout);
                }
                if !output.stderr.is_empty() {
                    item.push_str("\n=== STDERR ===\n");
                    item.push_str(&output.stderr);
                }
                if output.stdout_truncated || output.stderr_truncated {
                    item.push_str(&format!(
                        "\n[Output truncated; captured {} bytes on stdout and {} bytes on stderr.]",
                        output.stdout_total, output.stderr_total
                    ));
                }
                result.push(item);
            }
            if diagnosis_cap_reached_in_batch {
                break;
            }
        }
        if build_workspace_fingerprint_unstable {
            for (_, _, _, fingerprint) in &mut build_attempts {
                *fingerprint = None;
            }
            failure_count += 1;
            failure_codes.push("workspace_changed_during_build");
            result.push(
                "Workspace inputs changed or could not be verified during the build batch. Observed command exits were recorded, but they cannot authorize QC; rerun project_command(intent='build') after workspace writes stop.".into(),
            );
        }
        if run_count == 0 {
            return ToolResult::error(format!(
                "No detected project supports the '{}' intent. {}",
                intent.as_str(),
                result.join("\n\n")
            ));
        }
        let evidence_attempts = build_attempts
            .iter()
            .filter_map(|(command, exit_code, toolchain, fingerprint)| {
                evidence_kind(*toolchain)
                    .map(|kind| (command.clone(), *exit_code, kind, fingerprint.clone()))
            })
            .collect::<Vec<_>>();
        let aggregate_failure = if evidence_attempts.len() == build_attempts.len() {
            last_failed_build
        } else {
            None
        };
        let diagnostic_step = if let Some(batch) = build_batch {
            let evidence_started = std::time::Instant::now();
            let evidence_timing = ProjectCommandTimingContext {
                call_id: ctx.call_id.clone(),
                task_id: crate::observability::timing::correlation_task_id(ctx.task_id),
                session_id: ctx.session_id.clone(),
                intent: intent.as_str(),
                toolchain: "multi_project".into(),
            };
            let mut evidence_stage =
                ProjectCommandStageTimer::start("evidence_persistence", &evidence_timing);
            let all_children_completed = build_attempts.len() == expected_build_count
                && !timed_out
                && !failure_codes.contains(&"command_spawn_failed");
            let evidence_result = super::build_evidence::finish_project_build_batch_for_worker(
                batch.id,
                &ctx.agent_role,
                &ctx.session_id,
                &workspace_root,
                &evidence_attempts,
                aggregate_failure,
                all_children_completed,
            );
            if evidence_result.is_ok() {
                if let Some(guard) = cancellation_guard.as_mut() {
                    guard.disarm();
                }
            }
            tracing::info!(
                target: "arcana::telemetry",
                event_type = "build_evidence_persistence",
                task_id = %crate::observability::timing::correlation_task_id(ctx.task_id),
                session_id = %ctx.session_id,
                call_id = %ctx.call_id,
                duration_ns = evidence_started.elapsed().as_nanos() as u64,
                duration_ms = evidence_started.elapsed().as_millis() as u64,
                outcome = if evidence_result.is_ok() { "success" } else { "failure" },
                "Build evidence persistence finished"
            );
            if evidence_result.is_ok() {
                evidence_stage.succeed();
            }
            match evidence_result {
                Ok(Some(diagnostic_step)) => diagnostic_step,
                Ok(None) => {
                    return ToolResult::error(
                        "The project build ran, but Arcana could not finalize its durable batch record. QC remains blocked until a new complete build batch passes.",
                    )
                }
                Err(error) => {
                    tracing::error!(
                        target: "arcana::build_evidence",
                        task_id = ?ctx.task_id,
                        session_id = %ctx.session_id,
                        error = %error,
                        "observed build result could not be persisted"
                    );
                    return ToolResult::error(format!(
                        "The project build ran, but Arcana could not finalize its durable batch record: {error}. QC remains blocked until a new complete build batch passes."
                    ));
                }
            }
        } else {
            false
        };
        if let Some((capability, command_identity)) = sandbox_denial {
            return ToolResult::sandbox_violation(
                capability,
                &command_identity,
                false,
                "Arcana could not establish the required OS sandbox access for this selected toolchain. Do not retry automatically; inspect the workspace/runtime configuration or use the explicit host-controlled dependency setup path.",
            );
        }
        if failure_count == 0 {
            ToolResult::success(result.join("\n\n"))
        } else {
            safe_diagnostics.truncate(MAX_SAFE_DIAGNOSTICS);
            let code = if failure_codes.contains(&"operation_timed_out") {
                "operation_timed_out"
            } else if failure_codes.contains(&"command_spawn_failed") {
                "command_spawn_failed"
            } else if failure_codes.contains(&"workspace_changed_during_build") {
                "workspace_changed_during_build"
            } else {
                "project_command_failed"
            };
            let next_step = if diagnostic_step {
                "Three consecutive spawned build commands exited nonzero. Diagnose using only source-free diagnostic codes and workspace-relative locations, then stop and call request_supervisor_action(task_id=<current_task_id>, request_type='build_retry_approval', payload={diagnosis:<codes and workspace-relative locations>}). Do not issue another build until the Supervisor decides how to proceed; Arcana owns argv and the exit-code verdict."
            } else {
                "Use the source-free diagnostic codes and workspace-relative locations to inspect the failure. Check task and workspace state before retrying any command that may have completed side effects."
            };
            if timed_out {
                ToolResult::categorized_error(
                    "operation_timed_out",
                    "timeout",
                    "One or more selected project commands exceeded their bounded timeout.",
                    next_step,
                )
                .with_unknown_side_effect_outcome()
                .with_diagnostics(json!(safe_diagnostics))
            } else {
                ToolResult::categorized_error(
                    code,
                    "command",
                    format!("{failure_count} selected project command(s) did not complete successfully."),
                    next_step,
                )
                .with_diagnostics(json!(safe_diagnostics))
            }
        }
    }
}

async fn install_pinned_ruff_for_host() -> ToolResult {
    #[cfg(windows)]
    {
        return match crate::platform::python_runtime::install_ruff().await {
            Ok(()) => ToolResult::success(
                "Ruff 0.16.8 is installed in Arcana's verified Python tool cache. Python lint can now run offline through the process sandbox.",
            ),
            Err(error) => ToolResult::categorized_error(
                "python_lint_tool_install_failed",
                "dependency",
                format!("Arcana could not stage the pinned Ruff runtime: {error}"),
                "Check host network access and the signed Python installation, then retry this exact operation only after confirming the current stage was not partially installed.",
            ),
        };
    }

    #[cfg(not(windows))]
    {
        tracing::warn!(
            target: "arcana::telemetry",
            event_type = "project_command_denied",
            intent = "install",
            toolchain = "python",
            package = "ruff",
            reason = "host_tool_installer_unavailable",
            "the pinned Ruff installer is unavailable on this platform"
        );
        ToolResult::categorized_error(
            "python_lint_tool_install_unsupported",
            "unsupported_platform",
            "Arcana can stage its pinned Ruff runtime on Windows x64 only.",
            "Do not retry with a different package, installer, or Bash command. Complete a platform-specific host-approved Ruff staging path before using this exception on Linux or macOS.",
        )
    }
}

fn workspace_fingerprint_matches(baseline: Option<&str>, current: Option<&str>) -> bool {
    baseline
        .zip(current)
        .is_some_and(|(before, after)| before == after)
}

fn workspace_relative_path(workspace_root: &Path, project_dir: &Path) -> String {
    project_dir
        .strip_prefix(workspace_root)
        .ok()
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| ".".into())
}

/// Convert untrusted compiler output to bounded, source-free diagnostic
/// records. Only stable compiler error codes and locations inside the selected
/// project are retained; messages, source snippets, commands, and raw paths are
/// deliberately discarded.
fn source_free_diagnostics(stdout: &str, stderr: &str, project_dir: &Path) -> Vec<Value> {
    use std::sync::LazyLock;

    static RUST_CODE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^error\[(E[0-9]{4})\]:").expect("valid rustc diagnostic regex")
    });
    static TYPESCRIPT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^(.+?)\(([0-9]+),([0-9]+)\):\s*error\s+(TS[0-9]+)")
            .expect("valid TypeScript diagnostic regex")
    });
    static PYTHON_FILE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"^\s*File [\"'](.+?)[\"'], line ([0-9]+)"#)
            .expect("valid Python traceback regex")
    });
    static RUST_LOCATION: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^\s*-->\s+(.+?):([0-9]+):([0-9]+)\s*$")
            .expect("valid rustc location regex")
    });

    let lines = format!("{stderr}\n{stdout}")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut pending_python_location = None;
    for (index, line) in lines.iter().enumerate() {
        if let Some(captures) = TYPESCRIPT.captures(line) {
            diagnostics.push(diagnostic_record(
                captures.get(4).unwrap().as_str(),
                safe_diagnostic_path(captures.get(1).unwrap().as_str(), project_dir),
                captures
                    .get(2)
                    .and_then(|value| value.as_str().parse().ok()),
                captures
                    .get(3)
                    .and_then(|value| value.as_str().parse().ok()),
            ));
        } else if let Some(captures) = RUST_CODE.captures(line) {
            let location = lines[index + 1..]
                .iter()
                .take(8)
                .find_map(|next| RUST_LOCATION.captures(next));
            let (file, line, column) = location
                .map(|captures| {
                    (
                        safe_diagnostic_path(captures.get(1).unwrap().as_str(), project_dir),
                        captures
                            .get(2)
                            .and_then(|value| value.as_str().parse().ok()),
                        captures
                            .get(3)
                            .and_then(|value| value.as_str().parse().ok()),
                    )
                })
                .unwrap_or((None, None, None));
            diagnostics.push(diagnostic_record(
                captures.get(1).unwrap().as_str(),
                file,
                line,
                column,
            ));
        }
        if let Some(captures) = PYTHON_FILE.captures(line) {
            pending_python_location = Some((
                safe_diagnostic_path(captures.get(1).unwrap().as_str(), project_dir),
                captures
                    .get(2)
                    .and_then(|value| value.as_str().parse().ok()),
            ));
        }
        if line.trim_start().starts_with("SyntaxError:") {
            let (file, line) = pending_python_location.take().unwrap_or((None, None));
            diagnostics.push(diagnostic_record("PY_SYNTAX", file, line, None));
        }
        if diagnostics.len() >= MAX_SAFE_DIAGNOSTICS {
            break;
        }
    }
    if diagnostics.is_empty() {
        diagnostics.push(diagnostic_record("BUILD_FAILED", None, None, None));
    }
    diagnostics.truncate(MAX_SAFE_DIAGNOSTICS);
    diagnostics
}

fn diagnostic_record(
    code: &str,
    file: Option<String>,
    line: Option<u64>,
    column: Option<u64>,
) -> Value {
    let mut value = json!({ "code": code, "severity": "error" });
    if let Some(file) = file {
        value["file"] = json!(file);
    }
    if let Some(line) = line {
        value["line"] = json!(line.min(1_000_000));
    }
    if let Some(column) = column {
        value["column"] = json!(column.min(1_000_000));
    }
    value
}

fn safe_diagnostic_path(path: &str, project_dir: &Path) -> Option<String> {
    let path = Path::new(path.trim().trim_matches(['"', '\'']));
    let relative = if path.is_absolute() {
        let canonical = path.canonicalize().ok()?;
        canonical
            .strip_prefix(project_dir.canonicalize().ok()?)
            .ok()?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn format_detected_projects(projects: &[command::DetectedProject]) -> String {
    projects
        .iter()
        .map(|project| {
            format!(
                "{:?} at {}",
                project.toolchain,
                project.project_dir.display()
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn select_projects(
    detected: Vec<command::DetectedProject>,
    selector: Option<Toolchain>,
) -> Result<Vec<command::DetectedProject>, String> {
    if let Some(selector) = selector {
        let selected = detected
            .iter()
            .filter(|project| project.toolchain == selector)
            .cloned()
            .collect::<Vec<_>>();
        return if selected.is_empty() {
            Err(format!(
                "Toolchain selector {selector:?} was not detected. Detected projects: {}",
                format_detected_projects(&detected)
            ))
        } else {
            Ok(selected)
        };
    }
    Ok(detected)
}

struct ProcessOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
    stdout_total: usize,
    stderr_total: usize,
    stdout_truncated: bool,
    stderr_truncated: bool,
    runtime_prepare: Duration,
    sandbox_prepare: Duration,
    process_execution: Duration,
    output_drain: Duration,
}

#[derive(Clone)]
struct ProjectCommandTimingContext {
    call_id: String,
    task_id: String,
    session_id: String,
    intent: &'static str,
    toolchain: String,
}

impl ProjectCommandTimingContext {
    fn from_call(ctx: &ToolContext, intent: CommandIntent, toolchain: Toolchain) -> Self {
        Self {
            call_id: ctx.call_id.clone(),
            task_id: crate::observability::timing::correlation_task_id(ctx.task_id),
            session_id: ctx.session_id.clone(),
            intent: intent.as_str(),
            toolchain: format!("{toolchain:?}"),
        }
    }
}

/// Emits its timing even when the command future is cancelled by the
/// production timeout. A stage only reports success after its awaited work
/// finishes; cancellation and errors remain visible as `incomplete`.
struct ProjectCommandStageTimer {
    stage: &'static str,
    started: std::time::Instant,
    context: ProjectCommandTimingContext,
    outcome: &'static str,
}

impl ProjectCommandStageTimer {
    fn start(stage: &'static str, context: &ProjectCommandTimingContext) -> Self {
        Self {
            stage,
            started: std::time::Instant::now(),
            context: context.clone(),
            outcome: "incomplete",
        }
    }

    fn succeed(&mut self) {
        self.outcome = "success";
    }
}

impl Drop for ProjectCommandStageTimer {
    fn drop(&mut self) {
        tracing::info!(
            target: "arcana::telemetry",
            event_type = "project_command_stage",
            stage = self.stage,
            call_id = %self.context.call_id,
            task_id = %self.context.task_id,
            session_id = %self.context.session_id,
            intent = self.context.intent,
            toolchain = %self.context.toolchain,
            duration_ns = self.started.elapsed().as_nanos() as u64,
            outcome = self.outcome,
            "Project command stage timing recorded"
        );
    }
}

#[cfg(test)]
async fn run_resolved_command(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
    workspace_root: &Path,
) -> io::Result<ProcessOutput> {
    run_resolved_command_inner(resolved, project_dir, workspace_root, &[], None).await
}

async fn run_resolved_command_for_call(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
    workspace_root: &Path,
    ctx: &ToolContext,
    intent: CommandIntent,
    toolchain: Toolchain,
) -> io::Result<ProcessOutput> {
    let timing = ProjectCommandTimingContext::from_call(ctx, intent, toolchain);
    run_resolved_command_inner(resolved, project_dir, workspace_root, &[], Some(&timing)).await
}

#[cfg(test)]
async fn run_resolved_command_with_env(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
    workspace_root: &Path,
    additional_env: &[(OsString, OsString)],
) -> io::Result<ProcessOutput> {
    run_resolved_command_inner(resolved, project_dir, workspace_root, additional_env, None).await
}

async fn run_resolved_command_inner(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
    workspace_root: &Path,
    additional_env: &[(OsString, OsString)],
    timing: Option<&ProjectCommandTimingContext>,
) -> io::Result<ProcessOutput> {
    #[cfg(windows)]
    let (staged_python, staged_ruff, runtime_prepare) = {
        let mut stage_timer =
            timing.map(|context| ProjectCommandStageTimer::start("runtime_prepare", context));
        let started = std::time::Instant::now();
        let is_ruff = resolved.program.eq_ignore_ascii_case("python")
            && resolved
                .args
                .iter()
                .map(String::as_str)
                .eq(["-m", "ruff", "check"]);
        let ruff = if is_ruff {
            Some(crate::platform::python_runtime::staged_ruff()?)
        } else {
            None
        };
        let runtime = if !is_ruff
            && (resolved.program.eq_ignore_ascii_case("python")
                || resolved.program.eq_ignore_ascii_case("python.exe"))
        {
            Some(crate::platform::python_runtime::stage()?)
        } else {
            None
        };
        if let Some(timer) = &mut stage_timer {
            timer.succeed();
        }
        (runtime, ruff, started.elapsed())
    };
    #[cfg(not(windows))]
    let runtime_prepare = {
        let mut stage_timer =
            timing.map(|context| ProjectCommandStageTimer::start("runtime_prepare", context));
        if let Some(timer) = &mut stage_timer {
            timer.succeed();
        }
        Duration::ZERO
    };
    #[cfg(windows)]
    let (mut program, mut args, raw_arg, toolchain_env, shim_runtime) = {
        let rustup_tools = resolve_rustup_cargo_tools(resolved, project_dir)?;
        let selected_program = rustup_tools
            .as_ref()
            .map(|tools| tools.cargo.as_os_str())
            .unwrap_or_else(|| OsStr::new(&resolved.program));
        let is_batch_shim = Path::new(selected_program)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
            });
        if is_batch_shim {
            let shim_runtime = trusted_windows_shim_runtime(selected_program)?;
            // Windows cannot launch .cmd/.bat shims with CreateProcessW
            // directly. These argv values are generated by the fixed intent
            // resolver (for example `npm run build`), never model shell text.
            // Preserve the quoted shim path through cmd's /S /C parsing.
            let invocation = format!(
                "\"\"{}\" {}\"",
                selected_program.to_string_lossy(),
                resolved.args.join(" ")
            );
            (
                OsString::from("cmd.exe"),
                vec![
                    OsString::from("/d"),
                    OsString::from("/s"),
                    OsString::from("/c"),
                ],
                Some(OsString::from(invocation)),
                vec![(
                    OsString::from("NODE_OPTIONS"),
                    OsString::from("--preserve-symlinks --preserve-symlinks-main"),
                )],
                Some(shim_runtime),
            )
        } else {
            (
                selected_program.to_os_string(),
                resolved.args.iter().map(OsString::from).collect(),
                None,
                rustup_tools
                    .map(|tools| {
                        let mut path = tools.toolchain_dir.as_os_str().to_os_string();
                        path.push(";");
                        if let Some(linker_dir) = &tools.msvc_linker_dir {
                            path.push(linker_dir.as_os_str());
                            path.push(";");
                        }
                        if let Some(existing) = std::env::var_os("PATH") {
                            path.push(existing);
                        }
                        let mut environment = vec![
                            (OsString::from("RUSTC"), tools.rustc.into_os_string()),
                            (OsString::from("RUSTDOC"), tools.rustdoc.into_os_string()),
                            (OsString::from("PATH"), path),
                        ];
                        if !tools.msvc_lib_dirs.is_empty() {
                            let mut lib = OsString::new();
                            for directory in &tools.msvc_lib_dirs {
                                if !lib.is_empty() {
                                    lib.push(";");
                                }
                                lib.push(directory.as_os_str());
                            }
                            if let Some(existing) = std::env::var_os("LIB") {
                                if !lib.is_empty() {
                                    lib.push(";");
                                }
                                lib.push(existing);
                            }
                            environment.push((OsString::from("LIB"), lib));
                        }
                        environment
                    })
                    .unwrap_or_default(),
                None,
            )
        }
    };
    #[cfg(not(windows))]
    let (program, args, raw_arg) = (
        OsString::from(&resolved.program),
        resolved.args.iter().map(OsString::from).collect(),
        None,
    );
    let env_remove = std::env::vars()
        .filter_map(|(key, _)| {
            crate::security::env_scrub::looks_like_a_credential(&key).then(|| OsString::from(key))
        })
        .collect::<Vec<_>>();
    #[cfg(windows)]
    let mut env_remove = env_remove;
    #[cfg(windows)]
    env_remove.extend(
        [
            "PYTHONHOME",
            "PYTHONPATH",
            "PYTHONUSERBASE",
            "PYTHONSTARTUP",
        ]
        .into_iter()
        .map(OsString::from),
    );
    let mut env_set = clippy_config_environment(resolved, project_dir, workspace_root)?;
    #[cfg(windows)]
    env_set.extend(toolchain_env);
    #[cfg(windows)]
    let mut read_only_paths = if let Some((executable, runtime_root)) = staged_ruff {
        program = executable.into_os_string();
        args = vec![OsString::from("check")];
        vec![runtime_root]
    } else if let Some(runtime) = staged_python {
        program = runtime.executable.clone().into_os_string();
        let mut isolated_args = vec![
            OsString::from("-B"),
            OsString::from("-I"),
            OsString::from("-S"),
        ];
        isolated_args.append(&mut args);
        args = isolated_args;
        env_set.push((
            OsString::from("PYTEST_DISABLE_PLUGIN_AUTOLOAD"),
            OsString::from("1"),
        ));
        vec![runtime.root]
    } else {
        Vec::new()
    };
    #[cfg(windows)]
    if let Some(root) = shim_runtime {
        read_only_paths.push(root);
    }
    env_set.extend(additional_env.iter().cloned());
    let process = ProcessSpec {
        program,
        args,
        raw_arg,
        cwd: project_dir.to_path_buf(),
        env_remove,
        env_set,
        #[cfg(windows)]
        read_only_paths,
    };
    let sandbox_started = std::time::Instant::now();
    let mut sandbox_timer =
        timing.map(|context| ProjectCommandStageTimer::start("sandbox_prepare", context));
    let mut child = process_sandbox::spawn(process, workspace_root).await?;
    let sandbox_prepare = sandbox_started.elapsed();
    if let Some(timer) = &mut sandbox_timer {
        timer.succeed();
    }
    drop(sandbox_timer);
    let stdout = child.take_stdout().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Other,
            "child process stdout was not captured",
        )
    })?;
    let stderr = child.take_stderr().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Other,
            "child process stderr was not captured",
        )
    })?;
    let stdout_reader = tokio::spawn(capture_tail(stdout, MAX_STREAM_BYTES));
    let stderr_reader = tokio::spawn(capture_tail(stderr, MAX_STREAM_BYTES));

    // The timeout wraps this function at the call site. Dropping the child
    // therefore kills the process, while each reader retains only a bounded
    // tail and drains the pipe so a verbose compiler cannot exhaust memory.
    let process_started = std::time::Instant::now();
    let mut process_timer =
        timing.map(|context| ProjectCommandStageTimer::start("process_wait", context));
    let exit_code = child.wait().await?;
    let process_execution = process_started.elapsed();
    if let Some(timer) = &mut process_timer {
        timer.succeed();
    }
    drop(process_timer);
    let drain_started = std::time::Instant::now();
    let mut drain_timer =
        timing.map(|context| ProjectCommandStageTimer::start("output_drain", context));
    let (stdout_result, stderr_result) =
        tokio::join!(join_reader(stdout_reader), join_reader(stderr_reader));
    let (stdout, stdout_total) = stdout_result?;
    let (stderr, stderr_total) = stderr_result?;
    let output_drain = drain_started.elapsed();
    if let Some(timer) = &mut drain_timer {
        timer.succeed();
    }
    drop(drain_timer);
    let stdout_truncated = stdout_total > stdout.len();
    let stderr_truncated = stderr_total > stderr.len();
    let stdout = String::from_utf8_lossy(&stdout).into_owned();
    let stderr = String::from_utf8_lossy(&stderr).into_owned();
    Ok(ProcessOutput {
        exit_code,
        stdout: crate::security::truncate::truncate_bytes_from_end(&stdout, MAX_STREAM_BYTES)
            .to_string(),
        stderr: crate::security::truncate::truncate_bytes_from_end(&stderr, MAX_STREAM_BYTES)
            .to_string(),
        stdout_total,
        stderr_total,
        stdout_truncated: stdout_truncated || stdout_total > MAX_STREAM_BYTES,
        stderr_truncated: stderr_truncated || stderr_total > MAX_STREAM_BYTES,
        runtime_prepare,
        sandbox_prepare,
        process_execution,
        output_drain,
    })
}

#[cfg(windows)]
struct RustupCargoTools {
    cargo: std::path::PathBuf,
    toolchain_dir: std::path::PathBuf,
    rustc: std::path::PathBuf,
    rustdoc: std::path::PathBuf,
    msvc_linker_dir: Option<std::path::PathBuf>,
    msvc_lib_dirs: Vec<std::path::PathBuf>,
}

/// Resolve a Rustup proxy to the selected installed toolchain before sandboxing.
/// Rustup itself attempts to initialize its home on each proxy invocation;
/// running the actual toolchain binaries keeps autonomous builds read-only with
/// respect to the user's global Rustup state.
#[cfg(windows)]
fn resolve_rustup_cargo_tools(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
) -> io::Result<Option<RustupCargoTools>> {
    let is_cargo = Path::new(&resolved.program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case("cargo"));
    if !is_cargo {
        return Ok(None);
    }

    let Some(path) = std::env::var_os("PATH") else {
        return Ok(None);
    };
    let rustup = std::env::split_paths(&path)
        .map(|directory| directory.join("rustup.exe"))
        .find(|candidate| candidate.is_file());
    let Some(rustup) = rustup else {
        return Ok(None);
    };

    let output = Command::new(&rustup)
        .args(["which", "cargo"])
        .current_dir(project_dir)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Rustup could not resolve Cargo for the selected project: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    let cargo_output = String::from_utf8_lossy(&output.stdout);
    let cargo = Path::new(cargo_output.trim()).canonicalize()?;
    let toolchain_dir = cargo
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Rustup Cargo path has no parent",
            )
        })?;
    let rustc = toolchain_dir.join("rustc.exe");
    let rustdoc = toolchain_dir.join("rustdoc.exe");
    if !rustc.is_file() || !rustdoc.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Rustup's selected toolchain is missing rustc.exe or rustdoc.exe",
        ));
    }

    let msvc_linker_dir = resolve_msvc_linker_directory();
    let msvc_lib_dirs = resolve_msvc_library_directories(msvc_linker_dir.as_deref());
    Ok(Some(RustupCargoTools {
        cargo,
        toolchain_dir,
        rustc,
        rustdoc,
        msvc_linker_dir,
        msvc_lib_dirs,
    }))
}

#[cfg(windows)]
fn resolve_msvc_linker_directory() -> Option<std::path::PathBuf> {
    for variable in ["ProgramFiles(x86)", "ProgramFiles"] {
        let Some(program_files) = std::env::var_os(variable) else {
            continue;
        };
        let vswhere = std::path::PathBuf::from(program_files)
            .join("Microsoft Visual Studio")
            .join("Installer")
            .join("vswhere.exe");
        if !vswhere.is_file() {
            continue;
        }
        let output = Command::new(vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-find",
                "VC\\Tools\\MSVC\\**\\bin\\Hostx64\\x64\\link.exe",
            ])
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        if let Some(directory) = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .find(|path| !path.is_empty())
            .and_then(|path| std::path::Path::new(path).parent())
        {
            return Some(directory.to_path_buf());
        }
    }
    None
}

#[cfg(windows)]
fn resolve_msvc_library_directories(linker_dir: Option<&Path>) -> Vec<std::path::PathBuf> {
    let mut directories = Vec::new();
    if let Some(version_root) = linker_dir.and_then(|directory| {
        directory.ancestors().find(|ancestor| {
            ancestor
                .parent()
                .and_then(Path::file_name)
                .is_some_and(|name| name.eq_ignore_ascii_case("MSVC"))
        })
    }) {
        let vc_lib = version_root.join("lib").join("x64");
        if vc_lib.is_dir() {
            directories.push(vc_lib);
        }
    }

    let Some(program_files) = std::env::var_os("ProgramFiles(x86)") else {
        return directories;
    };
    let sdk_lib_root = std::path::PathBuf::from(program_files)
        .join("Windows Kits")
        .join("10")
        .join("Lib");
    let Ok(entries) = std::fs::read_dir(sdk_lib_root) else {
        return directories;
    };
    let mut versions = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let parts = name
                .split('.')
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            Some((parts, entry.path()))
        })
        .collect::<Vec<_>>();
    versions.sort_by(|left, right| left.0.cmp(&right.0));
    if let Some((_, sdk_version)) = versions.pop() {
        for directory in [
            sdk_version.join("ucrt").join("x64"),
            sdk_version.join("um").join("x64"),
        ] {
            if directory.is_dir() {
                directories.push(directory);
            }
        }
    }
    directories
}

/// Keep Cargo Clippy's config discovery inside the selected workspace. Its
/// default ancestor walk leaves the workspace, which fails under the read-only
/// process sandbox and can unexpectedly consume another project's/user's
/// config. Honor the nearest checked-in workspace config; when none exists,
/// point Clippy at the selected workspace root and let it use built-in defaults.
fn clippy_config_environment(
    resolved: &command::ResolvedCommand,
    project_dir: &Path,
    workspace_root: &Path,
) -> io::Result<Vec<(OsString, OsString)>> {
    let is_cargo_clippy = Path::new(&resolved.program)
        .file_stem()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("cargo"))
        && resolved.args.iter().any(|argument| argument == "clippy");
    if !is_cargo_clippy {
        return Ok(Vec::new());
    }

    let mut directory = project_dir;
    loop {
        for name in [".clippy.toml", "clippy.toml"] {
            let config = directory.join(name);
            match std::fs::symlink_metadata(&config) {
                Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Clippy config must be a regular file inside the selected workspace",
                    ));
                }
                Ok(_) => {
                    let absolute = canonical_workspace_config_path(workspace_root, directory)?;
                    return Ok(vec![(
                        OsString::from("CLIPPY_CONF_DIR"),
                        absolute.into_os_string(),
                    )]);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        if directory == workspace_root {
            break;
        }
        directory = directory
            .parent()
            .filter(|parent| parent.starts_with(workspace_root))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "project is outside the selected workspace",
                )
            })?;
    }

    let absolute = workspace_root.canonicalize()?;
    Ok(vec![(
        OsString::from("CLIPPY_CONF_DIR"),
        absolute.into_os_string(),
    )])
}

fn canonical_workspace_config_path(workspace_root: &Path, target: &Path) -> io::Result<PathBuf> {
    let workspace_root = workspace_root.canonicalize()?;
    let target = target.canonicalize()?;
    if !target.starts_with(&workspace_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Clippy config is outside the selected workspace",
        ));
    }
    #[cfg(windows)]
    {
        let value = target.to_string_lossy();
        if let Some(unc_path) = value.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{unc_path}")));
        }
        if let Some(path) = value.strip_prefix(r"\\?\") {
            return Ok(PathBuf::from(path));
        }
    }
    Ok(target)
}

async fn capture_tail<R>(mut reader: R, max_bytes: usize) -> io::Result<(Vec<u8>, usize)>
where
    R: AsyncRead + Unpin,
{
    let mut tail = Vec::with_capacity(max_bytes);
    let mut chunk = [0u8; 8192];
    let mut total = 0usize;
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        total = total.saturating_add(read);
        let bytes = &chunk[..read];
        if bytes.len() >= max_bytes {
            tail.clear();
            tail.extend_from_slice(&bytes[bytes.len() - max_bytes..]);
            continue;
        }
        let overflow = tail
            .len()
            .saturating_add(bytes.len())
            .saturating_sub(max_bytes);
        if overflow > 0 {
            tail.drain(..overflow);
        }
        tail.extend_from_slice(bytes);
    }
    Ok((tail, total))
}

async fn join_reader(
    reader: tokio::task::JoinHandle<io::Result<(Vec<u8>, usize)>>,
) -> io::Result<(Vec<u8>, usize)> {
    reader
        .await
        .map_err(|error| io::Error::new(io::ErrorKind::Other, error.to_string()))?
}

#[cfg(test)]
async fn run_child_for_test(program: &str, args: &[&str], project_dir: &Path) -> ProcessOutput {
    let resolved = command::ResolvedCommand {
        program: program.into(),
        args: args.iter().map(|arg| (*arg).into()).collect(),
    };
    run_resolved_command(&resolved, project_dir, project_dir)
        .await
        .expect("test process should spawn")
}

fn evidence_kind(toolchain: Toolchain) -> Option<BuildEvidenceKind> {
    match toolchain {
        Toolchain::Npm | Toolchain::Pnpm | Toolchain::Yarn | Toolchain::Bun => {
            Some(BuildEvidenceKind::Node)
        }
        Toolchain::Cargo => Some(BuildEvidenceKind::Cargo),
        Toolchain::Go => Some(BuildEvidenceKind::Go),
        Toolchain::Python => Some(BuildEvidenceKind::Python),
        Toolchain::Make => Some(BuildEvidenceKind::Make),
    }
}

fn resolve_project_directory(
    workspace_root: &Path,
    relative: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    let root = workspace_root.canonicalize().map_err(|error| {
        format!(
            "Cannot resolve workspace {}: {error}",
            workspace_root.display()
        )
    })?;
    let Some(relative) = relative.filter(|path| !path.trim().is_empty()) else {
        return Ok(root);
    };
    let supplied = Path::new(relative);
    if supplied.is_absolute() {
        return Err("project_path must be relative to the workspace".into());
    }
    let candidate = root.join(supplied).canonicalize().map_err(|error| {
        format!("Cannot resolve project_path '{relative}' inside the workspace: {error}")
    })?;
    if !candidate.starts_with(&root) {
        return Err("project_path resolves outside the workspace".into());
    }
    if !candidate.is_dir() {
        return Err(format!("project_path is not a directory: {relative}"));
    }
    Ok(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn ide_run_uses_opened_file_project_and_sandbox_without_a_task_claim() {
        let root = tempfile::tempdir().expect("isolated IDE workspace");
        let src = root.path().join("src");
        std::fs::create_dir(&src).expect("create source directory");
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"ide_run_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo manifest");
        std::fs::write(
            root.path().join("package.json"),
            r#"{"name":"wrong-project","version":"1.0.0","scripts":{"start":"exit 99"}}"#,
        )
        .expect("write competing Node manifest");
        std::fs::write(
            src.join("main.rs"),
            "fn main() { println!(\"ARCANA_IDE_RUN_OK\"); }\n",
        )
        .expect("write runnable artifact");

        let result = run_ide_project(root.path(), "src/main.rs", None)
            .await
            .expect("run selected Rust project without a task claim");
        assert_eq!(result.toolchain, "cargo");
        assert_eq!(result.project_path, ".");
        assert_eq!(result.command, "cargo run");
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert!(result.stdout.contains("ARCANA_IDE_RUN_OK"));
        assert!(run_ide_project(root.path(), "../outside.rs", None)
            .await
            .is_err());
        assert!(run_ide_project(root.path(), "src/main.rs", Some("python"))
            .await
            .is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn ide_run_executes_selected_system_node_shim_inside_windows_sandbox() {
        let fixture = tempfile::tempdir().expect("isolated Node fixture");
        let root = fixture.path().join("workspace");
        std::fs::create_dir(&root).expect("create Node workspace");
        let sibling = fixture.path().join("private.txt");
        std::fs::write(&sibling, "NODE_SIBLING_SECRET").expect("write sibling fixture");
        std::fs::write(
            root.join("package.json"),
            r#"{"name":"ide-node-fixture","version":"1.0.0","scripts":{"start":"node index.js"}}"#,
        )
        .expect("write Node manifest");
        std::fs::write(
            root.join("index.js"),
            format!(
                "const fs = require('node:fs');\ntry {{ fs.readFileSync({}); console.log('SIBLING_LEAK'); }} catch {{ console.log('SIBLING_DENIED'); }}\nconsole.log('ARCANA_IDE_NODE_RUN_OK');\n",
                serde_json::to_string(&sibling.to_string_lossy()).expect("quote sibling path")
            ),
        )
        .expect("write Node program");

        let result = run_ide_project(&root, "index.js", None)
            .await
            .expect("run selected npm project without a task claim");
        assert_eq!(result.toolchain, "npm");
        assert_eq!(result.exit_code, 0, "{}", result.stderr);
        assert!(result.stdout.contains("ARCANA_IDE_NODE_RUN_OK"));
        assert!(result.stdout.contains("SIBLING_DENIED"));
        assert!(!result.stdout.contains("SIBLING_LEAK"));
        assert!(!result.stdout.contains("NODE_SIBLING_SECRET"));
    }

    #[cfg(windows)]
    #[test]
    fn user_owned_package_manager_shim_requires_explicit_staging() {
        let root = tempfile::tempdir().expect("user-owned shim fixture");
        let shim = root.path().join("npm.cmd");
        std::fs::write(&shim, "@echo off\r\n").expect("write shim");
        let error = trusted_windows_shim_runtime(shim.as_os_str())
            .expect_err("a user-owned shim directory must not be granted to project code");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn pinned_ruff_install_fails_closed_with_structured_unsupported_platform_result() {
        let result = install_pinned_ruff_for_host().await;
        assert!(result.is_error);
        let error: Value = serde_json::from_str(&result.output).expect("structured tool error");
        assert_eq!(
            error["error"]["code"],
            "python_lint_tool_install_unsupported"
        );
        assert_eq!(error["error"]["category"], "unsupported_platform");
        assert!(error["error"]["next_step"]
            .as_str()
            .expect("actionable unsupported guidance")
            .contains("platform-specific"));
        assert!(!result.output.contains("http"));
    }

    #[test]
    fn incomplete_command_stage_keeps_tool_call_correlation() {
        let context = ProjectCommandTimingContext {
            call_id: "call-test".into(),
            task_id: "42".into(),
            session_id: "session-test".into(),
            intent: "build",
            toolchain: "Cargo".into(),
        };
        let (_, events) = crate::observability::timing::capture_json_events(|| {
            drop(ProjectCommandStageTimer::start("process_wait", &context));
        });
        let event = events
            .iter()
            .find(|event| event["fields"]["event_type"] == "project_command_stage")
            .expect("command-stage event is emitted on drop");
        assert_eq!(event["fields"]["stage"], "process_wait");
        assert_eq!(event["fields"]["call_id"], "call-test");
        assert_eq!(event["fields"]["task_id"], "42");
        assert_eq!(event["fields"]["session_id"], "session-test");
        assert_eq!(event["fields"]["outcome"], "incomplete");
        assert!(event["fields"]["duration_ns"].as_u64().is_some());
    }

    #[test]
    fn evidence_persistence_stage_uses_logs_call_correlation() {
        let context = ProjectCommandTimingContext {
            call_id: "build-call".into(),
            task_id: "42".into(),
            session_id: "session-build".into(),
            intent: "build",
            toolchain: "multi_project".into(),
        };
        let (_, events) = crate::observability::timing::capture_json_events(|| {
            let mut stage = ProjectCommandStageTimer::start("evidence_persistence", &context);
            stage.succeed();
        });
        let event = events
            .iter()
            .find(|event| {
                event["fields"]["event_type"] == "project_command_stage"
                    && event["fields"]["stage"] == "evidence_persistence"
            })
            .expect("evidence stage is observable by the Logs trace reader");
        assert_eq!(event["fields"]["call_id"], "build-call");
        assert_eq!(event["fields"]["task_id"], "42");
        assert_eq!(event["fields"]["outcome"], "success");
        assert!(event["fields"]["duration_ns"].as_u64().is_some());
    }

    #[test]
    fn dropped_project_build_batch_guard_persists_incomplete_observed_results() {
        let root = tempfile::tempdir().expect("isolated profile and workspace");
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&home).expect("create isolated profile");
        std::fs::create_dir_all(&workspace).expect("create selected workspace");
        let _home = crate::db::test_support::with_home(&home);

        let db = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "cancelled project batch".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "cancelled batches are never QC evidence".into(),
                created_by: Some("p4-14-cancellation-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign worker");
        db.claim_task_with_pid(task.id, "worker-backend", 992)
            .expect("claim task");
        let batch = db
            .begin_project_build_batch_for_worker(
                "worker-backend",
                "cancelled-session",
                2,
                &workspace.to_string_lossy(),
                None,
            )
            .expect("persist batch before child start")
            .expect("a claimed task owns this batch");
        drop(db);

        let mut guard = super::super::build_evidence::ProjectBuildBatchCancellationGuard::new(
            batch.id,
            "worker-backend",
            "cancelled-session",
            &workspace,
        );
        guard.record_observed_exit(
            "cargo build",
            17,
            super::super::build_evidence::BuildEvidenceKind::Cargo,
            None,
        );
        drop(guard);

        let db = crate::db::tasks::TasksDb::open().expect("reopen isolated TasksDb");
        let (outcome, expected, observed, passing, ..) = db
            .latest_project_build_batch_status(task.id)
            .expect("read finalized batch")
            .expect("batch remains durable");
        assert_eq!(outcome, "failed");
        assert_eq!(expected, 2);
        assert_eq!(observed, 1);
        assert_eq!(passing, 0);
        assert_eq!(db.get_retry_count(task.id).expect("read retry streak"), 1);
    }

    #[tokio::test]
    async fn cancelling_production_project_command_stops_descendant_and_closes_batch() {
        let root = tempfile::tempdir().expect("isolated profile and workspace");
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&home).expect("create isolated profile");
        let passing_project = workspace.join("a-passing");
        let cancelled_project = workspace.join("z-cancelled");
        std::fs::create_dir_all(passing_project.join("src")).expect("create first Rust project");
        std::fs::create_dir_all(cancelled_project.join("src")).expect("create second Rust project");
        let _home = crate::db::test_support::with_home(&home);
        std::fs::write(
            passing_project.join("Cargo.toml"),
            "[package]\nname = \"passing-build-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write first fixture manifest");
        std::fs::write(passing_project.join("src/main.rs"), "fn main() {}\n")
            .expect("write first fixture source");
        std::fs::write(
            cancelled_project.join("Cargo.toml"),
            "[package]\nname = \"cancelled-build-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n",
        )
        .expect("write second fixture manifest");
        std::fs::write(cancelled_project.join("src/main.rs"), "fn main() {}\n")
            .expect("write second fixture source");
        std::fs::write(
            cancelled_project.join("build.rs"),
            r#"
use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    process::Command,
    thread,
    time::Duration,
};

fn main() {
    let heartbeat = concat!(env!("CARGO_MANIFEST_DIR"), "/cancel-heartbeat");
    let child_started = concat!(env!("CARGO_MANIFEST_DIR"), "/cancel-child-started");
    if env::var_os("ARCANA_CANCEL_DESCENDANT").is_some() {
        fs::write(child_started, "started").unwrap();
        loop {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(heartbeat)
                .unwrap()
                .write_all(b"x")
                .unwrap();
            thread::sleep(Duration::from_millis(10));
        }
    }
    let executable = env::current_exe().unwrap();
    Command::new(executable)
        .env("ARCANA_CANCEL_DESCENDANT", "1")
        .spawn()
        .unwrap();
    fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/cancel-parent-started"), "started")
        .unwrap();
    thread::sleep(Duration::from_secs(120));
}
"#,
        )
        .expect("write long-running build script");

        let db = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "cancel project command safely".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "cancellation stops descendants and blocks QC".into(),
                created_by: Some("p4-14-cancellation-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        drop(db);

        let context = ToolContext {
            session_id: "cancelled-project-command-session".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "cancelled-project-command-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),
            run_token_budget: None,
        };
        let command = tokio::spawn(async move {
            ProjectCommandTool
                .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
                .await
        });
        let child_started = cancelled_project.join("cancel-child-started");
        tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                if child_started.is_file() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("production batch must finish the first crate and start the second");

        command.abort();
        assert!(
            command.await.is_err(),
            "the production tool task was aborted"
        );

        let db = crate::db::tasks::TasksDb::open().expect("reopen isolated TasksDb");
        let (outcome, expected, observed, passing, ..) = db
            .latest_project_build_batch_status(task.id)
            .expect("read cancelled batch")
            .expect("batch persisted before child spawn");
        assert_eq!(outcome, "incomplete");
        assert_eq!(expected, 2);
        assert_eq!(observed, 1, "the first project's observed pass is retained");
        assert_eq!(passing, 1);
        assert_eq!(db.get_retry_count(task.id).expect("read retry streak"), 0);
        let first_heartbeat = std::fs::read(cancelled_project.join("cancel-heartbeat"))
            .expect("descendant wrote its initial heartbeat");
        tokio::time::sleep(Duration::from_millis(100)).await;
        let stopped_heartbeat = std::fs::read(cancelled_project.join("cancel-heartbeat"))
            .expect("read descendant heartbeat after cancellation");
        assert_eq!(
            first_heartbeat, stopped_heartbeat,
            "the sandbox process tree must stop before cancellation returns"
        );
        assert!(
            crate::agent::builtin_tools::build_evidence::qc_gate_error_in(&db, task.id)
                .expect("QC gate reads cancelled batch")
                .is_some(),
            "an incomplete batch must never authorize QC"
        );
        assert!(
            db.update_status_with_workspace_and_run_id(
                task.id,
                "qc_review",
                Some("worker-backend"),
                None,
                None,
                &workspace,
            )
            .is_err(),
            "the shared TasksDb transition must reject cancellation after a passing sibling"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    rusqlite::params![task.id]
                )
                .is_err(),
            "SQLite must reject direct SQL QC over a partial multi-project batch"
        );
    }

    #[tokio::test]
    async fn production_multi_project_spawn_failure_cannot_be_hidden_by_passing_sibling() {
        let root = tempfile::tempdir().expect("isolated profile and workspace");
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        let passing_project = workspace.join("a-passing");
        let unavailable_project = workspace.join("z-unavailable");
        std::fs::create_dir_all(&home).expect("create isolated profile");
        std::fs::create_dir_all(passing_project.join("src")).expect("create first Rust project");
        std::fs::create_dir_all(unavailable_project.join("src"))
            .expect("create second Rust project");
        let _home = crate::db::test_support::with_home(&home);
        std::fs::write(
            passing_project.join("Cargo.toml"),
            "[package]\nname = \"spawn-barrier-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n",
        )
        .expect("write first fixture manifest");
        std::fs::write(passing_project.join("src/main.rs"), "fn main() {}\n")
            .expect("write first fixture source");
        std::fs::write(
            passing_project.join("build.rs"),
            r#"
use std::{env, fs, path::PathBuf, thread, time::Duration};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("ready"), "ready").unwrap();
    while !out.join("continue").is_file() {
        thread::sleep(Duration::from_millis(10));
    }
}
"#,
        )
        .expect("write first fixture synchronization script");
        std::fs::write(
            unavailable_project.join("Cargo.toml"),
            "[package]\nname = \"unavailable-build-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write second fixture manifest");
        std::fs::write(unavailable_project.join("src/main.rs"), "fn main() {}\n")
            .expect("write second fixture source");

        let db = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "partial build spawn failure blocks QC".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a passing sibling cannot hide a spawn failure".into(),
                created_by: Some("p4-14-spawn-failure-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        drop(db);

        let context = ToolContext {
            session_id: "multi-spawn-failure-session".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "multi-spawn-failure-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),
            run_token_budget: None,
        };
        let command = tokio::spawn(async move {
            ProjectCommandTool
                .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
                .await
        });
        let out_dir = tokio::time::timeout(Duration::from_secs(120), async {
            loop {
                let build_dir = workspace.join(".arcana-cache/cargo-target/debug/build");
                if let Ok(entries) = std::fs::read_dir(build_dir) {
                    if let Some(out_dir) = entries
                        .flatten()
                        .map(|entry| entry.path().join("out"))
                        .find(|candidate| candidate.join("ready").is_file())
                    {
                        break out_dir;
                    }
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("first production project reaches its controlled build barrier");

        let moved_project = workspace.join("z-unavailable-moved");
        std::fs::rename(&unavailable_project, &moved_project)
            .expect("remove the second selected project directory before its spawn");
        std::fs::write(out_dir.join("continue"), "continue")
            .expect("release the first project build barrier");
        let result = tokio::time::timeout(Duration::from_secs(120), command)
            .await
            .expect("multi-project operation finishes after the second spawn failure")
            .expect("project command task completes");
        assert!(
            result.is_error,
            "a selected spawn failure must fail the batch"
        );
        let output: Value = serde_json::from_str(&result.output).expect("structured tool error");
        assert_eq!(output["error"]["code"], "command_spawn_failed");

        let db = crate::db::tasks::TasksDb::open().expect("reopen isolated TasksDb");
        let (outcome, expected, observed, passing, ..) = db
            .latest_project_build_batch_status(task.id)
            .expect("read persisted batch")
            .expect("batch survives database reopen");
        assert_eq!(outcome, "incomplete");
        assert_eq!(expected, 2);
        assert_eq!(observed, 1, "the spawn failure has no fabricated exit");
        assert_eq!(passing, 1, "the first observed pass remains attributable");
        assert_eq!(db.get_retry_count(task.id).expect("read retry streak"), 0);
        assert!(
            db.update_status_with_workspace_and_run_id(
                task.id,
                "qc_review",
                Some("worker-backend"),
                None,
                None,
                &workspace,
            )
            .is_err(),
            "the shared QC transition must reject a passing sibling plus spawn failure"
        );
        assert!(
            db.conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    rusqlite::params![task.id]
                )
                .is_err(),
            "SQLite must reject direct SQL QC over a partial spawn-failed batch"
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn production_mixed_batch_timeout_cannot_be_hidden_by_passing_sibling() {
        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        #[cfg(windows)]
        crate::platform::python_runtime::stage()
            .expect("pre-stage the signed Python runtime outside the command timeout");
        #[cfg(windows)]
        let fixture_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("p4-acl-tests");
        #[cfg(windows)]
        std::fs::create_dir_all(&fixture_parent).expect("create isolated fixture parent");
        #[cfg(windows)]
        let workspace = tempfile::Builder::new()
            .prefix("project-command-timeout-batch-")
            .tempdir_in(&fixture_parent)
            .expect("create selected workspace");
        #[cfg(target_os = "linux")]
        let workspace = tempfile::tempdir().expect("create native Linux workspace");
        std::fs::create_dir_all(workspace.path().join("src"))
            .expect("create Rust source directory");
        // The workspace lives under `arcana-core/target/`, i.e. inside the
        // Arcana cargo workspace, so the manifest must declare its own empty
        // `[workspace]` table. Without it cargo refuses the build outright
        // ("current package believes it's in a workspace when it's not"),
        // which made the child exit non-zero immediately instead of timing
        // out. The checked-in `tests/fixtures/fx-repo-rust` fixture carries
        // the same table for the same reason.
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"timeout-batch-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\nbuild = \"build.rs\"\n\n[workspace]\n",
        )
        .expect("write Cargo manifest");
        std::fs::write(workspace.path().join("src/main.rs"), "fn main() {}\n")
            .expect("write Rust source");
        std::fs::write(
            workspace.path().join("build.rs"),
            "fn main() { std::thread::sleep(std::time::Duration::from_secs(60)); }\n",
        )
        .expect("write deliberately slow build script");
        let python = workspace.path().join("python");
        std::fs::create_dir_all(&python).expect("create nested Python project");
        std::fs::write(
            python.join("pyproject.toml"),
            "[project]\nname = \"arcana-timeout-batch-fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Python project marker");
        std::fs::write(python.join("valid.py"), "def valid():\n    return True\n")
            .expect("write valid Python source");
        let _workspace_lease = process_sandbox::retain_workspace(workspace.path());

        let detected = command::detect_toolchains(workspace.path(), workspace.path())
            .expect("detect mixed Cargo and Python projects");
        assert_eq!(detected.len(), 2, "{detected:#?}");
        assert_eq!(detected[0].toolchain, command::Toolchain::Cargo);
        assert_eq!(detected[1].toolchain, command::Toolchain::Python);

        let db = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "mixed batch timeout blocks QC".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "a passing sibling cannot hide a timeout".into(),
                created_by: Some("p4-14-timeout-test".into()),
            })
            .expect("create task");
        db.update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        drop(db);

        let context = ToolContext {
            session_id: "mixed-timeout-session".into(),
            working_dir: workspace.path().to_string_lossy().into_owned(),
            call_id: "mixed-timeout-call".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),
            run_token_budget: None,
        };
        let result = ProjectCommandTool
            .execute(json!({"intent": "build", "timeout_secs": 15}), &context)
            .await;
        assert!(result.is_error, "a timed-out child must fail the batch");

        let db = crate::db::tasks::TasksDb::open().expect("reopen isolated TasksDb");
        let (outcome, expected, observed, passing, ..) = db
            .latest_project_build_batch_status(task.id)
            .expect("read timeout batch")
            .expect("persisted timeout batch");
        assert_eq!(outcome, "incomplete");
        assert_eq!(expected, 2);
        assert_eq!(observed, 1, "the timed-out Cargo child has no exit result");
        assert_eq!(passing, 1, "the selected Python sibling passed");
        assert_eq!(db.get_retry_count(task.id).expect("read retry streak"), 0);
        let status_tool =
            crate::agent::builtin_tools::update_task_status_tool::UpdateTaskStatusTool;
        let qc = status_tool
            .execute(
                json!({"id": task.id, "status": "qc_review", "assigned_worker": "worker-backend"}),
                &context,
            )
            .await;
        assert!(
            qc.is_error,
            "the passing sibling cannot authorize QC after timeout"
        );
    }

    #[test]
    fn workspace_fingerprint_comparison_rejects_changed_or_unavailable_samples() {
        let root = std::env::temp_dir().join(format!(
            "arcana-project-command-fingerprint-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create isolated fingerprint fixture");
        let input = root.join("input.txt");
        std::fs::write(&input, "before").expect("write initial input");
        let baseline = crate::platform::workspace_fingerprint::fingerprint(&root)
            .expect("fingerprint initial workspace");
        assert!(workspace_fingerprint_matches(
            Some(&baseline),
            Some(&baseline)
        ));

        std::fs::write(&input, "changed during build").expect("mutate workspace input");
        let after = crate::platform::workspace_fingerprint::fingerprint(&root)
            .expect("fingerprint changed workspace");
        assert!(!workspace_fingerprint_matches(
            Some(&baseline),
            Some(&after)
        ));
        assert!(!workspace_fingerprint_matches(Some(&baseline), None));
        assert!(!workspace_fingerprint_matches(None, Some(&after)));
        std::fs::remove_dir_all(root).expect("remove isolated fingerprint fixture");
    }

    #[test]
    fn project_command_timeout_defaults_to_one_hour_and_caps_at_two() {
        assert_eq!(bounded_timeout_secs(None), 3_600);
        assert_eq!(bounded_timeout_secs(Some(1)), 1);
        assert_eq!(bounded_timeout_secs(Some(7_200)), 7_200);
        assert_eq!(bounded_timeout_secs(Some(u64::MAX)), 7_200);
        let schema = ProjectCommandTool.input_schema();
        assert_eq!(schema["properties"]["timeout_secs"]["maximum"], 7_200);
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn production_command_timeout_stops_nested_process_writes_and_records_stage_time() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime");
        let (_, events) = crate::observability::timing::capture_json_events(|| {
            runtime.block_on(async {
                let root = tempfile::tempdir().expect("isolated workspace");
                let workspace = root.path();
                let descendant_started = workspace.join("descendant-started.txt");
                let heartbeat = workspace.join("descendant-heartbeat.txt");
                let late = workspace.join("descendant-late.txt");
                #[cfg(windows)]
                let command = {
                    std::fs::write(
                        workspace.join("descendant.cmd"),
                        "@echo off\r\necho started>descendant-started.txt\r\nfor /l %%i in (1,1,100000000) do @echo alive>>descendant-heartbeat.txt\r\necho late>descendant-late.txt\r\n",
                    )
                    .expect("write descendant command");
                    std::fs::write(
                        workspace.join("parent.cmd"),
                        "@echo off\r\ncmd.exe /d /c descendant.cmd\r\n",
                    )
                    .expect("write parent command");
                    command::ResolvedCommand {
                        program: "cmd.exe".into(),
                        args: ["/d", "/c", "parent.cmd"]
                            .into_iter()
                            .map(str::to_owned)
                            .collect(),
                    }
                };
                #[cfg(target_os = "linux")]
                let command = {
                    std::fs::write(
                        workspace.join("descendant.py"),
                        "import pathlib, time\npathlib.Path('descendant-started.txt').write_text('started')\nfor _ in range(10000):\n    with open('descendant-heartbeat.txt', 'a') as output:\n        output.write('alive\\n')\n    time.sleep(0.03)\npathlib.Path('descendant-late.txt').write_text('late')\n",
                    )
                    .expect("write descendant script");
                    std::fs::write(
                        workspace.join("parent.py"),
                        "import subprocess, sys\nsubprocess.run([sys.executable, 'descendant.py'], check=True)\n",
                    )
                    .expect("write parent script");
                    command::ResolvedCommand {
                        program: "python3".into(),
                        args: vec!["parent.py".into()],
                    }
                };
                let context = ToolContext {
                    session_id: "timeout-stage-session".into(),
                    working_dir: workspace.display().to_string(),
                    call_id: "timeout-stage-call".into(),
                    run_id: None,
                    task_id: Some(42),
                    permission_mode: crate::agent::permission::PermissionMode::Autonomous,
                    depth: 0,
                    agent_role: "worker-backend".into(),

                    run_token_budget: None,};

                let mut command_future = Box::pin(run_resolved_command_for_call(
                    &command,
                    workspace,
                    workspace,
                    &context,
                    CommandIntent::Run,
                    Toolchain::Make,
                ));
                tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if descendant_started.is_file() {
                            break;
                        }
                        tokio::select! {
                            _output = &mut command_future => {
                                panic!("nested command exited before its descendant started")
                            },
                            _ = tokio::time::sleep(Duration::from_millis(10)) => {}
                        }
                    }
                })
                .await
                .expect("sandbox preparation and descendant startup must finish within ten seconds");
                let timed_out =
                    tokio::time::timeout(Duration::from_millis(300), &mut command_future).await;
                assert!(timed_out.is_err(), "production command should time out");
                drop(command_future);
                assert!(descendant_started.is_file());
                assert!(heartbeat.is_file());

                tokio::time::sleep(Duration::from_millis(200)).await;
                let heartbeat_size = std::fs::metadata(&heartbeat)
                    .expect("descendant heartbeat")
                    .len();
                tokio::time::sleep(Duration::from_millis(250)).await;
                assert_eq!(
                    std::fs::metadata(&heartbeat)
                        .expect("descendant heartbeat remains readable")
                        .len(),
                    heartbeat_size,
                    "project_command timeout left a nested process running"
                );
                assert!(
                    !late.exists(),
                    "descendant reached its post-timeout side effect"
                );
            });
        });
        let process_wait = events
            .iter()
            .find(|event| {
                event["fields"]["event_type"] == "project_command_stage"
                    && event["fields"]["stage"] == "process_wait"
            })
            .expect("timeout emits process-wait timing");
        assert_eq!(process_wait["fields"]["call_id"], "timeout-stage-call");
        assert_eq!(process_wait["fields"]["task_id"], "42");
        assert_eq!(process_wait["fields"]["outcome"], "incomplete");
        assert!(process_wait["fields"]["duration_ns"].as_u64().is_some());
        let sandbox_prepare = events
            .iter()
            .find(|event| {
                event["fields"]["event_type"] == "project_command_stage"
                    && event["fields"]["stage"] == "sandbox_prepare"
            })
            .expect("completed sandbox preparation is timed before cancellation");
        assert_eq!(sandbox_prepare["fields"]["call_id"], "timeout-stage-call");
        assert_eq!(sandbox_prepare["fields"]["outcome"], "success");
        let stage_position = |stage| {
            events
                .iter()
                .position(|event| {
                    event["fields"]["event_type"] == "project_command_stage"
                        && event["fields"]["stage"] == stage
                })
                .expect("stage event exists")
        };
        assert!(stage_position("sandbox_prepare") < stage_position("process_wait"),
            "completed preparation must be reported before process wait ends, including cancellation");
    }

    #[test]
    fn dependency_install_is_a_host_approval_call_and_cannot_be_standing_allowed() {
        let install = json!({"intent": "install"});
        let build = json!({"intent": "build"});
        let install_effect = ProjectCommandTool.effect_for_input(&install);
        assert_eq!(
            install_effect,
            crate::agent::tool_effect::ToolEffect::HostApproval
        );
        assert!(!install_effect.supports_standing_allow());
        assert_eq!(
            ProjectCommandTool.effect_for_input(&json!({
                "intent": "install",
                "toolchain": "python",
                "package": "ruff"
            })),
            crate::agent::tool_effect::ToolEffect::HostApproval
        );
        assert_eq!(
            ProjectCommandTool.effect_for_input(&build),
            crate::agent::tool_effect::ToolEffect::Execute
        );
    }

    #[tokio::test]
    async fn dependency_install_fails_closed_until_locked_staging_exists() {
        let context = ToolContext {
            session_id: "install-denial-session".into(),
            working_dir: ".".into(),
            call_id: "install-denial-call".into(),
            run_id: None,
            task_id: Some(7),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };

        let result = ProjectCommandTool
            .execute(json!({"intent": "install"}), &context)
            .await;
        let output: Value = serde_json::from_str(&result.output).expect("structured denial");

        assert!(result.is_error);
        assert_eq!(output["error"]["kind"], "sandbox_violation");
        assert_eq!(output["error"]["capability"], "network");
        assert_eq!(output["error"]["policy"], "deny");
        assert_eq!(output["error"]["command"], "project_command install");
        assert_eq!(output["error"]["retry_safe"], false);
        assert!(output["error"]["suggested_action"]
            .as_str()
            .unwrap()
            .contains("host-controlled path"));
    }

    #[tokio::test]
    async fn project_install_cannot_select_an_arbitrary_python_package() {
        let context = ToolContext {
            session_id: "install-denial-session".into(),
            working_dir: ".".into(),
            call_id: "install-denial-call".into(),
            run_id: None,
            task_id: Some(7),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = ProjectCommandTool
            .execute(
                json!({"intent": "install", "toolchain": "python", "package": "malicious"}),
                &context,
            )
            .await;
        let output: Value = serde_json::from_str(&result.output).expect("structured denial");
        assert!(result.is_error);
        assert_eq!(output["error"]["kind"], "sandbox_violation");
        assert_eq!(output["error"]["policy"], "deny");
        assert_eq!(output["error"]["command"], "project_command install");
    }

    #[test]
    fn evidence_kind_matches_detected_ecosystem() {
        assert_eq!(
            evidence_kind(Toolchain::Cargo),
            Some(BuildEvidenceKind::Cargo)
        );
        assert_eq!(
            evidence_kind(Toolchain::Python),
            Some(BuildEvidenceKind::Python)
        );
        assert_eq!(
            evidence_kind(Toolchain::Make),
            Some(BuildEvidenceKind::Make)
        );
    }

    #[cfg(windows)]
    #[test]
    fn cargo_prefers_the_installed_visual_studio_linker_over_ambient_path() {
        let Some(linker_dir) = resolve_msvc_linker_directory() else {
            return;
        };
        assert!(linker_dir.join("link.exe").is_file());
        assert!(linker_dir
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains("microsoft visual studio"));
    }

    #[test]
    fn arcana_workspace_detects_frontend_and_rust_candidates_separately() {
        let core_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = core_dir.parent().expect("Arcana workspace root");
        let projects = command::detect_toolchains(workspace_root, workspace_root)
            .expect("detect projects from the Arcana workspace");

        assert!(projects
            .iter()
            .any(|project| project.toolchain == Toolchain::Cargo));
        assert!(projects
            .iter()
            .any(|project| project.toolchain == Toolchain::Npm));
        assert_eq!(
            select_projects(projects.clone(), None).unwrap().len(),
            projects.len(),
            "without a selector, all detected projects are run separately"
        );
        assert_eq!(
            select_projects(projects.clone(), Some(Toolchain::Npm))
                .unwrap()
                .len(),
            1
        );
        assert!(select_projects(projects, Some(Toolchain::Go)).is_err());
    }

    #[test]
    fn a_toolchain_selector_keeps_each_matching_nested_project() {
        let projects = vec![
            command::DetectedProject {
                toolchain: Toolchain::Npm,
                project_dir: Path::new("workspace/web").to_path_buf(),
            },
            command::DetectedProject {
                toolchain: Toolchain::Cargo,
                project_dir: Path::new("workspace").to_path_buf(),
            },
            command::DetectedProject {
                toolchain: Toolchain::Npm,
                project_dir: Path::new("workspace/docs").to_path_buf(),
            },
        ];
        let selected = select_projects(projects, Some(Toolchain::Npm)).unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].project_dir, Path::new("workspace/web"));
        assert_eq!(selected[1].project_dir, Path::new("workspace/docs"));
    }

    #[test]
    fn project_path_must_resolve_inside_the_workspace() {
        let root =
            std::env::temp_dir().join(format!("arcana-project-root-{}", uuid::Uuid::new_v4()));
        let nested = root.join("frontend");
        std::fs::create_dir_all(&nested).unwrap();
        assert_eq!(
            resolve_project_directory(&root, Some("frontend")).unwrap(),
            nested.canonicalize().unwrap()
        );
        assert!(resolve_project_directory(&root, Some("..")).is_err());
        assert!(resolve_project_directory(&root, Some("C:\\outside")).is_err());
        assert!(resolve_project_directory(&root, Some("missing")).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn failed_compiler_output_becomes_source_free_structured_diagnostics() {
        let project = tempfile::tempdir().expect("project fixture");
        let source = project.path().join("src");
        std::fs::create_dir(&source).expect("source directory");
        std::fs::write(source.join("main.rs"), "fn main() { private_source(); }\n")
            .expect("source fixture");
        let stderr = format!(
            "error[E0425]: cannot find value `private_source`\n  --> {}:1:12\n  |\n1 | fn main() {{ let api_key = \"sk-abcdefghijklmnopqrstuvwxyz012345\"; }}\n",
            source.join("main.rs").display()
        );

        let diagnostics = source_free_diagnostics("", &stderr, project.path());
        let serialized = serde_json::to_string(&diagnostics).unwrap();
        let parsed: Value = serde_json::from_str(&serialized).unwrap();

        assert_eq!(parsed[0]["code"], "E0425");
        assert_eq!(parsed[0]["severity"], "error");
        assert_eq!(parsed[0]["file"], "src/main.rs");
        assert_eq!(parsed[0]["line"], 1);
        assert_eq!(parsed[0]["column"], 12);
        assert!(!serialized.contains("private_source"));
        assert!(!serialized.contains("api_key"));
        assert!(!serialized.contains("sk-abcdefghijklmnopqrstuvwxyz"));
        assert!(!serialized.contains(project.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn diagnostic_paths_outside_the_project_are_omitted() {
        let root = tempfile::tempdir().expect("workspace fixture");
        let project = root.path().join("project");
        let sibling = root.path().join("sibling.rs");
        std::fs::create_dir(&project).expect("project directory");
        std::fs::write(&sibling, "fn private() {}\n").expect("sibling source");
        let stderr = format!(
            "error[E0425]: missing item\n  --> {}:4:9\n",
            sibling.display()
        );

        let diagnostics = source_free_diagnostics("", &stderr, &project);
        assert_eq!(diagnostics[0]["code"], "E0425");
        assert!(diagnostics[0].get("file").is_none());
        assert_eq!(diagnostics[0]["line"], 4);
    }

    #[tokio::test]
    async fn registered_compiler_failure_returns_only_bounded_safe_diagnostics() {
        let root = tempfile::tempdir().expect("isolated Arcana profile");
        let home = root.path().join("home");
        std::fs::create_dir_all(&home).expect("create isolated Arcana home");
        #[cfg(windows)]
        let workspace_parent = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("p4-51-compiler-privacy-workspaces");
        #[cfg(not(windows))]
        let workspace_parent =
            std::env::temp_dir().join("arcana-p4-51-compiler-privacy-workspaces");
        std::fs::create_dir_all(&workspace_parent).expect("create sandbox-compatible parent");
        let workspace_temp = tempfile::Builder::new()
            .prefix("compiler-privacy-")
            .tempdir_in(&workspace_parent)
            .expect("create isolated compiler workspace");
        let workspace = workspace_temp
            .path()
            .canonicalize()
            .expect("canonicalize sandbox-compatible workspace");
        std::fs::create_dir_all(workspace.join("src")).expect("create compiler source tree");
        let _home = crate::db::test_support::with_home(&home);
        // The workspace lives under `arcana-core/target/`, i.e. inside the
        // Arcana cargo workspace, so the manifest must declare its own empty
        // `[workspace]` table. Without it cargo refuses the build outright
        // ("current package believes it's in a workspace when it's not") and
        // the diagnostics parser sees no `error[E####]` line, falling back to
        // BUILD_FAILED instead of the planted E0425.
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"registered-compiler-privacy\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .expect("write dependency-free Cargo manifest");
        std::fs::write(
            workspace.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"registered-compiler-privacy\"\nversion = \"0.1.0\"\n",
        )
        .expect("stage the dependency-free lockfile before workspace fingerprinting");
        let long_private_source = "P4_PRIVATE_LONG_COMPILER_SOURCE_CANARY_".repeat(128);
        let source = format!(
            "fn main() {{ let _ = \"P4_PRIVATE_SOURCE_CANARY_{long_private_source}\"; let _private_api_key = \"sk-abcdefghijklmnopqrstuvwxyz012345\"; P4_PRIVATE_RAW_COMMAND_CANARY(); }}\n"
        );
        std::fs::write(workspace.join("src/main.rs"), &source)
            .expect("write intentionally failing private source");

        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "verify registered compiler diagnostic privacy".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "compiler failures expose only safe diagnostics".into(),
                created_by: Some("p4-51-compiler-privacy-test".into()),
            })
            .expect("create worker task");
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign worker");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim worker task");

        let session_id = uuid::Uuid::new_v4().to_string();
        let run_id = crate::db::runs::RunsDb::open()
            .expect("open isolated RunsDb")
            .begin_worker_tick("worker-backend", &session_id)
            .expect("persist real worker run before registered tool execution");
        let provider_call_id = format!("compiler-privacy-{}", uuid::Uuid::new_v4());
        let expected_raw_call_id = format!("{session_id}-{provider_call_id}");
        let workspace_path = crate::platform::workspace::resolve_workspace_dir(Some(
            workspace.to_str().expect("compiler workspace path"),
        ))
        .expect("use the production selected-workspace path representation")
        .to_string_lossy()
        .into_owned();
        let mut registry = crate::agent::tool_registry::ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mode(
            &mut registry,
            crate::agent::PAMode::FastDevelopment,
        );
        let mut executor = crate::agent::tool_executor::ToolExecutor::new(
            crate::agent::permission::PermissionMode::Autonomous,
            crate::agent::permission::new_pending_permissions(),
            session_id.clone(),
            workspace_path.clone(),
            crate::agent::tool_trait::AgentIdentity::worker("worker-backend"),
        )
        .with_run_id(Some(run_id.clone()));
        executor.set_task_id(Some(task.id));
        let mut results = executor
            .execute_batch(vec![(
                registry
                    .get("project_command")
                    .expect("FastDevelopment project_command registration"),
                json!({"intent": "build", "toolchain": "cargo"}),
                provider_call_id.clone(),
            )])
            .await
            .expect("ToolExecutor returns the failed compiler result");
        assert_eq!(results.len(), 1);
        let (returned_call_id, result) = results.pop().expect("one registered result");
        assert_eq!(returned_call_id, provider_call_id);
        assert!(
            result.is_error,
            "compiler failure must remain a failed tool result"
        );
        assert!(result.output.len() <= 8_192, "failure envelope is bounded");

        let envelope: Value = serde_json::from_str(&result.output)
            .expect("registered compiler failure is structured JSON");
        assert_eq!(envelope["schema_version"], 1);
        assert_eq!(envelope["ok"], false);
        assert_ne!(envelope["error_code"], "operation_intent_unavailable");
        assert_eq!(envelope["error_code"], "project_command_failed");
        assert_eq!(envelope["tool"], "project_command");
        assert_eq!(envelope["operation"], "tool.project_command");
        assert_eq!(envelope["run_id"], run_id);
        assert_eq!(envelope["context"]["run_id"], run_id);
        assert_eq!(envelope["task_id"], task.id);
        assert_eq!(envelope["context"]["tool_call_id"], expected_raw_call_id);
        let diagnostic = &envelope["error"]["diagnostics"][0];
        assert_eq!(diagnostic["code"], "E0425");
        assert_eq!(diagnostic["severity"], "error");
        assert_eq!(diagnostic["file"], "src/main.rs");
        assert_eq!(diagnostic["line"], 1);

        for private_text in [
            "P4_PRIVATE_RAW_COMMAND_CANARY",
            "P4_PRIVATE_SOURCE_CANARY",
            "P4_PRIVATE_LONG_COMPILER_SOURCE_CANARY",
            "sk-abcdefghijklmnopqrstuvwxyz012345",
            source.as_str(),
            workspace_path.as_str(),
        ] {
            assert!(
                !result.output.contains(private_text),
                "private compiler text was reflected: {private_text}"
            );
        }

        let journal = crate::db::runs::RunsDb::open()
            .expect("reopen isolated RunsDb")
            .operation_journal_page(&run_id, 0, 10)
            .expect("read registered project-command journal");
        assert_eq!(
            journal.entries.len(),
            1,
            "one registered call was journaled"
        );
        assert_eq!(journal.entries[0].run_id, run_id);
        assert_eq!(journal.entries[0].operation, "project_command");
        assert_eq!(journal.entries[0].task_id, Some(task.id));
        assert_eq!(
            journal.entries[0].outcome,
            crate::db::runs::OperationOutcome::Error
        );

        let observed = tasks
            .latest_build_evidence(task.id)
            .expect("read actual failed build evidence")
            .expect("compiler failure was observed and persisted");
        assert_eq!(observed.session_id, session_id);
        assert_ne!(
            observed.exit_code, 0,
            "the recorded compiler exited unsuccessfully"
        );
        assert!(observed.exit_code_attributable);
        assert_eq!(observed.kind, "cargo");
        let expected_workspace_identity =
            crate::platform::workspace::workspace_identity(Path::new(&workspace_path))
                .expect("normalize selected workspace identity");
        assert_eq!(
            crate::platform::workspace::workspace_identity(Path::new(
                observed
                    .workspace_path
                    .as_deref()
                    .expect("observed workspace"),
            ))
            .expect("normalize persisted evidence workspace"),
            expected_workspace_identity,
        );
        assert!(!result.output.contains(
            observed
                .workspace_path
                .as_deref()
                .expect("observed workspace"),
        ));
        assert!(observed.workspace_fingerprint.is_some());

        let batch = tasks
            .latest_project_build_batch_status(task.id)
            .expect("read finalized project build batch")
            .expect("failed build batch was persisted");
        assert_eq!(
            (batch.0.as_str(), batch.1, batch.2, batch.3),
            ("failed", 1, 1, 0)
        );
        assert_eq!(
            batch.6, 1,
            "one actual compiler result belongs to the batch"
        );
        assert_eq!(
            batch.7, 1,
            "the nonzero compiler result is classified invalid for QC"
        );
        assert_eq!(
            crate::platform::workspace::workspace_identity(Path::new(&batch.4))
                .expect("normalize persisted batch workspace"),
            expected_workspace_identity,
        );
        assert_eq!(batch.5, observed.workspace_fingerprint);

        crate::db::runs::RunsDb::open()
            .expect("reopen isolated RunsDb to close fixture run")
            .finish_worker_tick(
                &run_id,
                "failed",
                crate::db::runs::outcome::FAILED,
                Some("project_command_failed"),
            )
            .expect("finish the actual worker run");
    }

    fn copy_fixture_tree(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).expect("create fixture directory");
        for entry in std::fs::read_dir(source).expect("read checked-in fixture") {
            let entry = entry.expect("fixture entry");
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());
            if source_path.is_dir() {
                copy_fixture_tree(&source_path, &destination_path);
            } else {
                std::fs::copy(source_path, destination_path).expect("copy fixture file");
            }
        }
    }

    #[tokio::test]
    async fn checked_in_ecosystem_corpora_run_real_intents_through_the_production_launcher() {
        use command::{CommandIntent, Toolchain};

        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        // Python staging is a process-global cache and must not happen inside a
        // command timeout: prime it here exactly as the mixed-batch test does,
        // so this test does not depend on another test in the same process
        // having staged the runtime first (it failed on a fresh process with
        // "runtime files were not found").
        #[cfg(windows)]
        crate::platform::python_runtime::stage()
            .expect("pre-stage the signed Python runtime outside the command timeouts");
        let fixtures = [
            (
                "fx-repo-rust",
                Toolchain::Cargo,
                &[
                    CommandIntent::Build,
                    CommandIntent::Test,
                    CommandIntent::Lint,
                    CommandIntent::Run,
                ][..],
            ),
            (
                "fx-repo-node",
                Toolchain::Npm,
                &[
                    CommandIntent::Build,
                    CommandIntent::Test,
                    CommandIntent::Lint,
                    CommandIntent::Run,
                ][..],
            ),
            (
                "fx-repo-python",
                Toolchain::Python,
                // The portable corpus proves the standard-library build
                // intent. Pytest and Ruff are dependencies: their production
                // paths are covered by the Windows staged-runtime test and
                // must not be installed implicitly on the Linux host.
                &[CommandIntent::Build][..],
            ),
        ];

        for (fixture_name, expected_toolchain, intents) in fixtures {
            let source = manifest_dir.join("tests/fixtures").join(fixture_name);
            let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
            std::fs::create_dir_all(&fixture_parent)
                .expect("create repository-local ACL fixture parent");
            let sandbox = tempfile::Builder::new()
                .prefix("project-command-corpus-")
                .tempdir_in(&fixture_parent)
                .expect("create isolated toolchain corpus");
            let project = sandbox.path().join(fixture_name);
            copy_fixture_tree(&source, &project);
            let _workspace_lease = process_sandbox::retain_workspace(&project);

            let detected =
                command::detect_toolchains(&project, &project).expect("detect fixture toolchain");
            assert_eq!(detected.len(), 1, "fixture {fixture_name}");
            assert_eq!(
                detected[0].toolchain, expected_toolchain,
                "fixture {fixture_name}"
            );

            for intent in intents {
                let resolved = command::resolve(expected_toolchain, *intent)
                    .unwrap_or_else(|| panic!("{expected_toolchain:?} must support {intent:?}"));
                let output = run_resolved_command(&resolved, &project, &project)
                    .await
                    .unwrap_or_else(|error| panic!("{fixture_name} {intent:?}: {error}"));
                assert_eq!(
                    output.exit_code, 0,
                    "{fixture_name} {intent:?} failed\nstdout: {}\nstderr: {}",
                    output.stdout, output.stderr
                );
            }
        }

        assert!(command::resolve(Toolchain::Python, CommandIntent::Build).is_some());
        assert!(command::resolve(Toolchain::Python, CommandIntent::Run).is_none());
        assert!(command::resolve(Toolchain::Python, CommandIntent::Install).is_none());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_build_and_test_intents_run_for_all_checked_in_ecosystems() {
        use command::{CommandIntent, Toolchain};

        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        std::fs::create_dir_all(&fixture_parent).expect("create repository-local ACL parent");

        for (fixture_name, toolchain) in [
            ("fx-repo-rust", Toolchain::Cargo),
            ("fx-repo-node", Toolchain::Npm),
            ("fx-repo-python", Toolchain::Python),
        ] {
            let sandbox = tempfile::Builder::new()
                .prefix("project-command-build-test-")
                .tempdir_in(&fixture_parent)
                .expect("create isolated checked-in project");
            let project = sandbox.path().join(fixture_name);
            copy_fixture_tree(
                &manifest_dir.join("tests/fixtures").join(fixture_name),
                &project,
            );
            let _workspace_lease = process_sandbox::retain_workspace(&project);
            let detected = command::detect_toolchains(&project, &project)
                .expect("detect the checked-in project");
            assert_eq!(detected.len(), 1, "{fixture_name}");
            assert_eq!(detected[0].toolchain, toolchain, "{fixture_name}");

            for intent in [CommandIntent::Build, CommandIntent::Test] {
                let resolved = command::resolve(toolchain, intent)
                    .unwrap_or_else(|| panic!("{fixture_name} must support {intent:?}"));
                let output = run_resolved_command(&resolved, &project, &project)
                    .await
                    .unwrap_or_else(|error| panic!("{fixture_name} {intent:?}: {error}"));
                assert_eq!(
                    output.exit_code, 0,
                    "{fixture_name} {intent:?} failed\nstdout: {}\nstderr: {}",
                    output.stdout, output.stderr
                );
            }
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_rust_run_and_node_lint_run_use_production_sandbox() {
        use command::{CommandIntent, Toolchain};

        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        std::fs::create_dir_all(&fixture_parent).expect("create repository-local ACL parent");

        for (fixture_name, toolchain, intents) in [
            ("fx-repo-rust", Toolchain::Cargo, &[CommandIntent::Run][..]),
            (
                "fx-repo-node",
                Toolchain::Npm,
                &[CommandIntent::Lint, CommandIntent::Run][..],
            ),
        ] {
            let sandbox = tempfile::Builder::new()
                .prefix("project-command-lint-run-")
                .tempdir_in(&fixture_parent)
                .expect("create isolated checked-in project");
            let project = sandbox.path().join(fixture_name);
            copy_fixture_tree(
                &manifest_dir.join("tests/fixtures").join(fixture_name),
                &project,
            );
            let _workspace_lease = process_sandbox::retain_workspace(&project);

            for intent in intents {
                let resolved = command::resolve(toolchain, *intent)
                    .unwrap_or_else(|| panic!("{fixture_name} must support {intent:?}"));
                let output = run_resolved_command(&resolved, &project, &project)
                    .await
                    .unwrap_or_else(|error| panic!("{fixture_name} {intent:?}: {error}"));
                assert_eq!(
                    output.exit_code, 0,
                    "{fixture_name} {intent:?} failed\nstdout: {}\nstderr: {}",
                    output.stdout, output.stderr
                );
            }
        }
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn production_python_build_and_tests_use_the_staged_runtime() {
        use command::{CommandIntent, Toolchain};

        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = manifest_dir.join("tests/fixtures").join("fx-repo-python");
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        std::fs::create_dir_all(&fixture_parent)
            .expect("create repository-local ACL fixture parent");
        let sandbox = tempfile::Builder::new()
            .prefix("project-command-python-")
            .tempdir_in(&fixture_parent)
            .expect("create Python fixture workspace");
        copy_fixture_tree(&source, sandbox.path());
        let _workspace_lease = process_sandbox::retain_workspace(sandbox.path());

        for intent in [CommandIntent::Build, CommandIntent::Test] {
            let resolved = command::resolve(Toolchain::Python, intent).expect("Python intent");
            let output = run_resolved_command(&resolved, sandbox.path(), sandbox.path())
                .await
                .unwrap_or_else(|error| panic!("Python {intent:?} launch: {error}"));
            assert_eq!(
                output.exit_code, 0,
                "Python {intent:?} failed\nstdout: {}\nstderr: {}",
                output.stdout, output.stderr
            );
            if intent == CommandIntent::Test {
                assert!(output.stdout.contains("1 passed"), "{}", output.stdout);
            }
        }
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn production_mixed_project_batch_requires_every_child_to_pass_before_qc() {
        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let rust_fixture = manifest_dir.join("tests/fixtures/fx-repo-rust");
        #[cfg(windows)]
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        #[cfg(windows)]
        std::fs::create_dir_all(&fixture_parent)
            .expect("create repository-local ACL fixture parent");
        #[cfg(windows)]
        let sandbox = tempfile::Builder::new()
            .prefix("project-command-mixed-batch-")
            .tempdir_in(&fixture_parent)
            .expect("create mixed-project workspace");
        #[cfg(target_os = "linux")]
        let sandbox = tempfile::tempdir().expect("create native Linux workspace");
        copy_fixture_tree(&rust_fixture, sandbox.path());
        let python = sandbox.path().join("python");
        std::fs::create_dir_all(&python).expect("create nested Python project");
        std::fs::write(
            python.join("pyproject.toml"),
            "[project]\nname = \"arcana-p4-mixed-fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Python project marker");
        let source = python.join("broken.py");
        std::fs::write(&source, "def broken(:\n    pass\n")
            .expect("write intentionally invalid Python source");
        let _workspace_lease = process_sandbox::retain_workspace(sandbox.path());

        let detected = command::detect_toolchains(sandbox.path(), sandbox.path())
            .expect("detect mixed Rust and Python projects");
        assert_eq!(detected.len(), 2, "{detected:#?}");
        assert_eq!(detected[0].toolchain, command::Toolchain::Cargo);
        assert_eq!(detected[1].toolchain, command::Toolchain::Python);

        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated task DB");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "mixed-project batch needs every child to pass".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "all detected projects pass before QC".into(),
                created_by: Some("test".into()),
            })
            .expect("create task");
        assert!(crate::db::workers::WorkersDb::open()
            .expect("open isolated worker DB")
            .registered_slugs()
            .expect("read registered workers")
            .iter()
            .any(|slug| slug == "worker-backend"));
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign task");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        drop(tasks);

        let ctx = ToolContext {
            session_id: "session-mixed-build-proof".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "project-command-mixed-build-proof".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let status_tool =
            crate::agent::builtin_tools::update_task_status_tool::UpdateTaskStatusTool;
        let qc_input = json!({
            "id": task.id,
            "status": "qc_review",
            "assigned_worker": "worker-backend"
        });

        let tool = ProjectCommandTool;
        let failed_batch = tool.execute(json!({"intent": "build"}), &ctx).await;
        assert!(failed_batch.is_error, "{}", failed_batch.output);
        let db = crate::db::tasks::TasksDb::open().expect("read failed mixed batch");
        let batch = db
            .latest_project_build_batch_status(task.id)
            .expect("read batch status")
            .expect("persisted mixed build batch");
        assert_eq!(
            batch.0, "failed",
            "a passing Rust child cannot hide Python failure"
        );
        assert_eq!(batch.1, 2, "both detected projects belong to the batch");
        assert_eq!(batch.2, 2, "both real command exits are observed");
        assert_eq!(batch.3, 1, "only the Rust child passed");
        assert!(status_tool.execute(qc_input.clone(), &ctx).await.is_error);
        assert_eq!(
            db.get_by_id(task.id).unwrap().unwrap().status,
            crate::db::tasks::TaskStatus::InProgress,
            "incomplete success across a mixed workspace must not enter QC"
        );

        std::fs::write(&source, "def repaired():\n    return True\n")
            .expect("repair Python fixture after failed mixed batch");
        let passing_batch = tool.execute(json!({"intent": "build"}), &ctx).await;
        assert!(
            !passing_batch.is_error,
            "all-project batch should pass after repair: {}",
            passing_batch.output
        );
        let db = crate::db::tasks::TasksDb::open().expect("read completed mixed batch");
        let batch = db
            .latest_project_build_batch_status(task.id)
            .expect("read completed batch status")
            .expect("passing mixed build batch");
        assert_eq!(batch.0, "passed");
        assert_eq!(batch.1, 2);
        assert_eq!(batch.2, 2);
        assert_eq!(batch.3, 2);
        let qc = status_tool.execute(qc_input, &ctx).await;
        assert!(
            !qc.is_error,
            "complete passing batch should authorize QC: {}",
            qc.output
        );
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[tokio::test]
    async fn production_mixed_batch_stops_after_third_observed_failure() {
        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let rust_fixture = manifest_dir.join("tests/fixtures/fx-repo-rust");
        #[cfg(windows)]
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        #[cfg(windows)]
        std::fs::create_dir_all(&fixture_parent)
            .expect("create repository-local ACL fixture parent");
        #[cfg(windows)]
        let sandbox = tempfile::Builder::new()
            .prefix("project-command-diagnosis-cap-")
            .tempdir_in(&fixture_parent)
            .expect("create mixed-project workspace");
        #[cfg(target_os = "linux")]
        let sandbox = tempfile::tempdir().expect("create native Linux workspace");
        copy_fixture_tree(&rust_fixture, sandbox.path());
        std::fs::write(sandbox.path().join("src/main.rs"), "fn main( {\n")
            .expect("break Rust fixture to produce an observed failure");
        let python = sandbox.path().join("python");
        std::fs::create_dir_all(&python).expect("create nested Python project");
        std::fs::write(
            python.join("pyproject.toml"),
            "[project]\nname = \"arcana-p4-cap-fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write Python project marker");
        let _workspace_lease = process_sandbox::retain_workspace(sandbox.path());

        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated task DB");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "stop a multi-project batch at the diagnosis threshold".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "no child launches after the third observed failure".into(),
                created_by: Some("p4-15-test".into()),
            })
            .expect("create task");
        tasks
            .increment_retry(task.id)
            .expect("seed prior failure one");
        tasks
            .increment_retry(task.id)
            .expect("seed prior failure two");
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign worker");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", std::process::id())
            .expect("claim task");
        drop(tasks);

        let ctx = ToolContext {
            session_id: "session-diagnosis-cap".into(),
            working_dir: sandbox.path().to_string_lossy().into_owned(),
            call_id: "project-command-diagnosis-cap".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let result = ProjectCommandTool
            .execute(json!({"intent": "build"}), &ctx)
            .await;
        assert!(result.is_error, "the broken Rust child must fail");
        let db = crate::db::tasks::TasksDb::open().expect("read completed build batch");
        let batch = db
            .latest_project_build_batch_status(task.id)
            .expect("read build batch status")
            .expect("persisted build batch");
        assert_eq!(
            batch.0, "failed",
            "project command result: {}",
            result.output
        );
        assert_eq!(batch.1, 2, "both ecosystems were selected");
        assert_eq!(
            batch.2, 1,
            "Python must not launch after Rust reaches three"
        );
        assert_eq!(batch.3, 0);
        assert_eq!(db.get_retry_count(task.id).unwrap(), 3);
        let status_tool =
            crate::agent::builtin_tools::update_task_status_tool::UpdateTaskStatusTool;
        assert!(
            status_tool
                .execute(
                    json!({
                        "id": task.id,
                        "status": "qc_review",
                        "assigned_worker": "worker-backend"
                    }),
                    &ctx
                )
                .await
                .is_error,
            "a cutoff batch cannot authorize QC"
        );

        let request_result =
            crate::agent::builtin_tools::request_supervisor_action::RequestSupervisorActionTool
                .execute(
                    json!({
                        "task_id": task.id,
                        "request_type": "build_retry_approval",
                        "payload": {
                            "diagnosis": [{"code": "RUST_COMPILER_ERROR", "location": "src/main.rs:1"}]
                        }
                    }),
                    &ctx,
                )
                .await;
        assert!(
            !request_result.is_error,
            "worker must persist its source-free diagnosis: {}",
            request_result.output
        );
        let request: Value = serde_json::from_str(&request_result.output).unwrap();
        let mut supervisor_context = ctx.clone();
        supervisor_context.agent_role = "supervisor".into();
        supervisor_context.task_id = None;
        let approval =
            crate::agent::builtin_tools::resolve_worker_request_tool::ResolveWorkerRequestTool
                .execute(
                    json!({
                        "id": request["request_id"],
                        "action": "approve_build_retry",
                        "note": "The Rust syntax diagnosis is specific; one mixed verification batch is justified."
                    }),
                    &supervisor_context,
                )
                .await;
        assert!(
            !approval.is_error,
            "Supervisor should authorize one bounded batch: {}",
            approval.output
        );

        std::fs::copy(
            rust_fixture.join("src/main.rs"),
            sandbox.path().join("src/main.rs"),
        )
        .expect("repair Rust fixture using its known-good source");
        let approved_result = ProjectCommandTool
            .execute(json!({"intent": "build"}), &ctx)
            .await;
        assert!(
            !approved_result.is_error,
            "the approved mixed-project verification should pass: {}",
            approved_result.output
        );
        let db = crate::db::tasks::TasksDb::open().expect("read approved mixed batch");
        let approved_batch = db
            .latest_project_build_batch_status(task.id)
            .expect("read approved mixed build status")
            .expect("approved retry batch must be persisted");
        assert_eq!(approved_batch.0, "passed");
        assert_eq!(approved_batch.1, 2, "Cargo and Python remain selected");
        assert_eq!(
            approved_batch.2, 2,
            "both approved children must be observed"
        );
        assert_eq!(approved_batch.3, 2, "both approved child builds must pass");
        let qc = status_tool
            .execute(
                json!({
                    "id": task.id,
                    "status": "qc_review",
                    "assigned_worker": "worker-backend"
                }),
                &ctx,
            )
            .await;
        assert!(
            !qc.is_error,
            "the complete approved mixed batch should authorize QC: {}",
            qc.output
        );
    }

    fn assert_task_build_timing_sample(
        db: &crate::db::tasks::TasksDb,
        task_id: i64,
        workspace: &Path,
        call_id: &str,
        events: &[serde_json::Value],
        wall_ns: u128,
    ) -> std::collections::BTreeMap<&'static str, u64> {
        let batch = db
            .latest_project_build_batch_status(task_id)
            .expect("read persisted ProjectCommand build batch")
            .expect("ProjectCommand must persist a build batch");
        assert_eq!(batch.0, "passed");
        assert_eq!((batch.1, batch.2, batch.3), (1, 1, 1));
        assert_eq!(batch.6, 1, "one successful build evidence row must persist");
        assert_eq!(batch.7, 0, "persisted build evidence must be valid");
        let workspace_identity = crate::platform::workspace::workspace_identity(workspace)
            .expect("resolve the measured workspace identity");
        assert_eq!(
            crate::platform::workspace::workspace_identity(Path::new(&batch.4))
                .expect("resolve the persisted batch workspace identity"),
            workspace_identity
        );
        let evidence = db
            .latest_build_evidence(task_id)
            .expect("read persisted ProjectCommand build evidence")
            .expect("passing batch must retain its build evidence");
        assert_eq!(evidence.exit_code, 0);
        assert!(evidence.exit_code_attributable);
        assert_eq!(
            crate::platform::workspace::workspace_identity(Path::new(
                evidence
                    .workspace_path
                    .as_deref()
                    .expect("persisted evidence workspace path"),
            ))
            .expect("resolve the persisted evidence workspace identity"),
            workspace_identity
        );
        assert_eq!(
            evidence.workspace_fingerprint.as_deref(),
            batch.5.as_deref()
        );

        let mut stages = std::collections::BTreeMap::new();
        for stage in [
            "runtime_prepare",
            "sandbox_prepare",
            "process_wait",
            "output_drain",
            "evidence_persistence",
        ] {
            let matching = events
                .iter()
                .filter(|event| {
                    event["fields"]["event_type"] == "project_command_stage"
                        && event["fields"]["call_id"] == call_id
                        && event["fields"]["stage"] == stage
                })
                .collect::<Vec<_>>();
            assert_eq!(
                matching.len(),
                1,
                "one {stage} event per selected command batch"
            );
            assert_eq!(matching[0]["fields"]["task_id"], task_id.to_string());
            assert_eq!(matching[0]["fields"]["outcome"], "success");
            stages.insert(
                stage,
                matching[0]["fields"]["duration_ns"]
                    .as_u64()
                    .expect("stage duration in nanoseconds"),
            );
        }
        assert!(
            stages
                .values()
                .map(|duration| u128::from(*duration))
                .sum::<u128>()
                <= wall_ns,
            "stage durations must not exceed ProjectCommandTool wall time"
        );
        stages
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "measurement-only P4-62 task-attributed cold/warm timing and evidence"]
    fn p4_62_measure_linux_task_build_stages_with_evidence() {
        const PROFILES: usize = 3;
        const WARM_RUNS: usize = 5;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("measurement runtime");
        let home = tempfile::tempdir().expect("owned measurement profile");
        let _home = crate::db::test_support::with_home(home.path());
        let db = crate::db::tasks::TasksDb::open().expect("measurement TasksDb");
        let mut samples = Vec::new();
        for profile in 0..PROFILES {
            let workspace = tempfile::tempdir().expect("fresh cold workspace/cache");
            std::fs::write(
                workspace.path().join("pyproject.toml"),
                "[project]\nname = \"arcana-timing-fixture\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
            std::fs::write(
                workspace.path().join("valid.py"),
                "def valid():\n    return True\n",
            )
            .unwrap();
            let task = db
                .create(crate::db::tasks::CreateTaskInput {
                    title: format!("P4-62 matched build profile {profile}"),
                    description: None,
                    priority: "p2".into(),
                    task_type: "feature".into(),
                    acceptance_criteria: "observed build plus correlated persistence timings"
                        .into(),
                    created_by: Some("p4-62-measurement".into()),
                })
                .unwrap();
            db.update_status(task.id, "assigned", Some("worker-backend"))
                .unwrap();
            db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
                .unwrap();
            for sample in 0..=WARM_RUNS {
                let context = ToolContext {
                    session_id: format!("timing-profile-{profile}"),
                    working_dir: workspace.path().display().to_string(),
                    call_id: format!("timing-profile-{profile}-sample-{sample}"),
                    run_id: None,
                    task_id: Some(task.id),
                    permission_mode: crate::agent::permission::PermissionMode::Autonomous,
                    depth: 0,
                    agent_role: "worker-backend".into(),
                    run_token_budget: None,
                };
                let started = std::time::Instant::now();
                let (result, events) = crate::observability::timing::capture_json_events(|| {
                    runtime.block_on(
                        ProjectCommandTool
                            .execute(json!({"intent": "build", "toolchain": "python"}), &context),
                    )
                });
                assert!(!result.is_error, "build failed: {}", result.output);
                let wall_ns = started.elapsed().as_nanos();
                let stages = assert_task_build_timing_sample(
                    &db,
                    task.id,
                    workspace.path(),
                    &context.call_id,
                    &events,
                    wall_ns,
                );
                let row = json!({"profile": profile, "sample": sample, "cold": sample == 0,
                    "wall_ns": wall_ns, "stages_ns": stages});
                eprintln!("P4-62 observed sample {row}");
                samples.push(row);
            }
            db.update_status(task.id, "done", None)
                .expect("release the completed measurement task before the next profile");
        }
        assert_eq!(samples.len(), PROFILES * (WARM_RUNS + 1));
        // These are measurements, not a universal latency or speedup guarantee.
        // Sources stay identical within each profile; warm caches are retained.
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "measurement-only P4-62 Windows task-attributed cold/warm stage timing and evidence"]
    fn p4_62_measure_windows_task_build_stages_with_evidence() {
        const PROFILES: usize = 3;
        const WARM_RUNS: usize = 5;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("measurement runtime");
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        std::fs::create_dir_all(&fixture_parent)
            .expect("create repository-local ACL measurement parent");
        let mut samples = Vec::with_capacity(PROFILES * (WARM_RUNS + 1));

        for profile_index in 0..PROFILES {
            let profile = tempfile::tempdir().expect("owned isolated Arcana profile");
            let _profile_guard = crate::db::test_support::with_home(profile.path());
            let db = crate::db::tasks::TasksDb::open().expect("measurement TasksDb");
            let workspace = tempfile::Builder::new()
                .prefix("p4-62-windows-task-build-")
                .tempdir_in(&fixture_parent)
                .expect("create fresh repository-local Windows workspace/cache");
            std::fs::write(
                workspace.path().join("pyproject.toml"),
                "[project]\nname = \"arcana-timing-fixture\"\nversion = \"0.1.0\"\n",
            )
            .expect("write identical Python project manifest");
            std::fs::write(
                workspace.path().join("valid.py"),
                "def valid():\n    return True\n",
            )
            .expect("write identical Python source");
            let _workspace_lease = process_sandbox::retain_workspace(workspace.path());
            let task = db
                .create(crate::db::tasks::CreateTaskInput {
                    title: format!("P4-62 Windows matched build profile {profile_index}"),
                    description: None,
                    priority: "p2".into(),
                    task_type: "feature".into(),
                    acceptance_criteria:
                        "successful Python Build with correlated stage timings and persisted evidence"
                            .into(),
                    created_by: Some("p4-62-measurement".into()),
                })
                .expect("create measurement task");
            db.update_status(task.id, "assigned", Some("worker-backend"))
                .expect("assign measurement task");
            db.claim_task_with_pid(task.id, "worker-backend", std::process::id())
                .expect("atomically claim measurement task with current PID");

            // Each profile gets fresh home/workspace/application caches. Cold is
            // relative to those Arcana-owned caches only; the OS cache is intact.
            crate::platform::python_runtime::reset_process_stage_cache_for_test();
            for sample_index in 0..=WARM_RUNS {
                let context = ToolContext {
                    session_id: format!("p4-62-windows-profile-{profile_index}"),
                    working_dir: workspace.path().display().to_string(),
                    call_id: format!("p4-62-windows-profile-{profile_index}-sample-{sample_index}"),
                    run_id: None,
                    task_id: Some(task.id),
                    permission_mode: crate::agent::permission::PermissionMode::Autonomous,
                    depth: 0,
                    agent_role: "worker-backend".into(),
                    run_token_budget: None,
                };
                let started = std::time::Instant::now();
                let (result, events) = crate::observability::timing::capture_json_events(|| {
                    runtime.block_on(
                        ProjectCommandTool
                            .execute(json!({"intent": "build", "toolchain": "python"}), &context),
                    )
                });
                assert!(
                    !result.is_error,
                    "Windows Python Build profile {profile_index} sample {sample_index} failed: {}",
                    result.output
                );
                let wall_ns = started.elapsed().as_nanos();
                let stages = assert_task_build_timing_sample(
                    &db,
                    task.id,
                    workspace.path(),
                    &context.call_id,
                    &events,
                    wall_ns,
                );
                let row = json!({
                    "profile": profile_index,
                    "sample": sample_index,
                    "cold": sample_index == 0,
                    "wall_ns": wall_ns,
                    "stages_ns": stages,
                });
                eprintln!("P4-62 Windows observed sample {row}");
                samples.push((profile_index, sample_index, wall_ns, stages));
            }
            db.update_status_for_worker_claim(task.id, "worker-backend", "done", None, None)
                .expect("complete task and release its real worker claim");
        }

        fn median_and_range(values: impl Iterator<Item = u128>) -> (u128, u128, u128) {
            let mut values = values.collect::<Vec<_>>();
            assert!(!values.is_empty());
            values.sort_unstable();
            (
                values[values.len() / 2],
                values[0],
                values[values.len() - 1],
            )
        }

        assert_eq!(samples.len(), PROFILES * (WARM_RUNS + 1));
        for stage in [
            "wall",
            "runtime_prepare",
            "sandbox_prepare",
            "process_wait",
            "output_drain",
            "evidence_persistence",
        ] {
            let value = |sample: &(
                usize,
                usize,
                u128,
                std::collections::BTreeMap<&'static str, u64>,
            )| {
                if stage == "wall" {
                    sample.2
                } else {
                    u128::from(*sample.3.get(stage).expect("all five stage samples"))
                }
            };
            let (cold_median, cold_min, cold_max) =
                median_and_range(samples.iter().filter(|sample| sample.1 == 0).map(value));
            let (warm_median, warm_min, warm_max) =
                median_and_range(samples.iter().filter(|sample| sample.1 > 0).map(value));
            eprintln!(
                "P4-62 Windows {stage} ns: cold_median={cold_median} cold_range={cold_min}..{cold_max}; warm_median={warm_median} warm_range={warm_min}..{warm_max}"
            );
        }
        // These are observed command-stage timings, not compiler time or a
        // general speedup claim. OS caches are intentionally left untouched.
    }

    #[cfg(windows)]
    #[ignore = "legacy launcher-only cold/warm comparison; use task-attributed stage fixture"]
    #[tokio::test]
    async fn p4_62_measure_matching_python_build_cold_and_warm() {
        use command::{CommandIntent, Toolchain};

        const WARM_RUNS: usize = 5;
        let profile = tempfile::tempdir().expect("isolated Arcana profile");
        let _profile_guard = crate::db::test_support::with_home(profile.path());
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = manifest_dir.join("tests/fixtures/fx-repo-python");
        let fixture_parent = manifest_dir.join("target").join("p4-acl-tests");
        std::fs::create_dir_all(&fixture_parent)
            .expect("create repository-local ACL fixture parent");
        let sandbox = tempfile::Builder::new()
            .prefix("p4-62-python-build-")
            .tempdir_in(&fixture_parent)
            .expect("create matched Python build workspace");
        copy_fixture_tree(&source, sandbox.path());
        let bytecode_cache = sandbox.path().join("tests").join("__pycache__");
        if bytecode_cache.exists() {
            std::fs::remove_dir_all(&bytecode_cache)
                .expect("remove copied fixture bytecode before the cold sample");
        }
        let _workspace_lease = process_sandbox::retain_workspace(sandbox.path());
        let resolved =
            command::resolve(Toolchain::Python, CommandIntent::Build).expect("Python Build intent");

        // Keep the same profile and workspace for every sample. The cold run
        // stages the trusted runtime from disk; warm runs reuse its verified
        // process cache. Remove only compileall output created by this fixture
        // so every invocation starts from the same project contents.
        crate::platform::python_runtime::reset_process_stage_cache_for_test();
        let mut samples = Vec::with_capacity(WARM_RUNS + 1);
        for sample_index in 0..=WARM_RUNS {
            let output = run_resolved_command(&resolved, sandbox.path(), sandbox.path())
                .await
                .unwrap_or_else(|error| panic!("Python Build sample {sample_index}: {error}"));
            assert_eq!(
                output.exit_code, 0,
                "Python Build sample {sample_index} failed\nstdout: {}\nstderr: {}",
                output.stdout, output.stderr
            );
            samples.push((
                output.runtime_prepare,
                output.sandbox_prepare,
                output.process_execution,
                output.output_drain,
            ));
            if bytecode_cache.exists() {
                std::fs::remove_dir_all(&bytecode_cache)
                    .expect("remove fixture-owned compileall bytecode cache");
            }
        }

        fn median_ms(values: impl Iterator<Item = Duration>) -> u128 {
            let mut values = values
                .map(|duration| duration.as_millis())
                .collect::<Vec<_>>();
            values.sort_unstable();
            values[values.len() / 2]
        }
        let cold = samples[0];
        let warm = &samples[1..];
        eprintln!(
            "P4-62 matched Python Build timings (same staged runtime profile, same workspace, one cold + {WARM_RUNS} warm): cold_ms={{runtime_prepare:{},sandbox_prepare:{},process_execution:{},output_drain:{}}}; warm_median_ms={{runtime_prepare:{},sandbox_prepare:{},process_execution:{},output_drain:{}}}; warm_range_ms={{total:{}..{}}}",
            cold.0.as_millis(),
            cold.1.as_millis(),
            cold.2.as_millis(),
            cold.3.as_millis(),
            median_ms(warm.iter().map(|sample| sample.0)),
            median_ms(warm.iter().map(|sample| sample.1)),
            median_ms(warm.iter().map(|sample| sample.2)),
            median_ms(warm.iter().map(|sample| sample.3)),
            warm.iter()
                .map(|sample| sample.0 + sample.1 + sample.2 + sample.3)
                .map(|duration| duration.as_millis())
                .min()
                .unwrap_or_default(),
            warm.iter()
                .map(|sample| sample.0 + sample.1 + sample.2 + sample.3)
                .map(|duration| duration.as_millis())
                .max()
                .unwrap_or_default(),
        );
    }

    #[test]
    fn clippy_config_respects_workspace_config_and_never_walks_above_workspace() {
        let sandbox = tempfile::tempdir().expect("workspace fixture");
        let workspace = sandbox.path().join("workspace");
        let project = workspace.join("nested");
        std::fs::create_dir_all(&project).unwrap();
        let command = command::resolve(Toolchain::Cargo, CommandIntent::Lint).unwrap();

        let environment = clippy_config_environment(&command, &project, &workspace).unwrap();
        assert_eq!(environment[0].0, OsString::from("CLIPPY_CONF_DIR"));
        let default_config_dir = PathBuf::from(&environment[0].1);
        assert!(default_config_dir.is_absolute());
        assert_eq!(default_config_dir, workspace.canonicalize().unwrap());
        assert!(!default_config_dir.join(".clippy.toml").exists());

        std::fs::write(
            workspace.join("clippy.toml"),
            "too-many-arguments-threshold = 8\n",
        )
        .unwrap();
        let environment = clippy_config_environment(&command, &project, &workspace).unwrap();
        assert_eq!(
            PathBuf::from(&environment[0].1).canonicalize().unwrap(),
            workspace.canonicalize().unwrap()
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn production_clippy_environment_is_visible_and_readable_in_workspace_sandbox() {
        // Mirror the approved PSEC staging location: a private, OS-temporary
        // root outside the repository path. This checks whether relocating a
        // selected workspace alone addresses the host's canonicalization
        // denial before adding production copy-back behavior.
        let root = tempfile::Builder::new()
            .prefix("production-clippy-stage-")
            .tempdir()
            .expect("staged workspace fixture");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let _lease = process_sandbox::retain_workspace(&workspace);
        // Keep the probe manifest at the selected workspace root so Cargo's
        // generated target tree inherits the active workspace ACL lease.
        let project = workspace.clone();
        std::fs::write(
            project.join("Cargo.toml"),
            "[package]\nname = \"clippy_probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write minimal Rust manifest");
        std::fs::create_dir(project.join("src")).expect("create Rust source directory");
        std::fs::write(
            project.join("src/lib.rs"),
            "pub fn probe() -> bool { true }\n",
        )
        .expect("write minimal Rust source");
        let lint = command::resolve(Toolchain::Cargo, CommandIntent::Lint).unwrap();
        let env_set = clippy_config_environment(&lint, &project, &workspace)
            .expect("resolve the production Clippy config environment");
        let config_dir = project.join(PathBuf::from(&env_set[0].1));
        let expected_config = config_dir.join(".clippy.toml");
        let config_contents = "too-many-arguments-threshold = 8";
        std::fs::write(&expected_config, config_contents).expect("write workspace config");

        let wrapper_source = workspace.join("clippy_env_probe.rs");
        let wrapper_path = workspace.join("clippy_env_probe.exe");
        let wrapper_log = workspace.join("clippy_env_probe.log");
        std::fs::write(
            &wrapper_source,
            r#"
use std::io::Write;
use std::process::Command;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let compiler = args.next().expect("Cargo supplies the compiler as argument one");
    let compiler_args = args.collect::<Vec<_>>();
    let log_path = std::env::var_os("ARCANA_TEST_CLIPPY_LOG").expect("probe log path");
    let config_dir = std::env::var_os("CLIPPY_CONF_DIR").expect("Clippy config directory");
    let config_dir = std::path::PathBuf::from(config_dir);
    let config_file = config_dir.join(".clippy.toml");
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(log_path).unwrap();
    writeln!(log, "config_dir_path={:?}", config_dir).unwrap();
    writeln!(log, "config_dir={:?}", config_dir.canonicalize()).unwrap();
    writeln!(log, "config_dir_metadata={:?}", std::fs::metadata(&config_dir)).unwrap();
    writeln!(log, "config_file={:?}", config_file.canonicalize()).unwrap();
    writeln!(log, "config_file_metadata={:?}", std::fs::metadata(&config_file)).unwrap();
    writeln!(log, "config_file_read={:?}", std::fs::read(&config_file)).unwrap();
    let status = Command::new(compiler).args(compiler_args).status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
"#,
        )
        .expect("write Rust compiler environment probe");
        let tools = resolve_rustup_cargo_tools(&lint, &project)
            .expect("resolve selected Rust toolchain")
            .expect("Rustup toolchain is installed");
        let compile_probe = Command::new(&tools.rustc)
            .arg("--edition=2021")
            .arg(&wrapper_source)
            .arg("-o")
            .arg(&wrapper_path)
            .status()
            .expect("compile the Rust compiler environment probe");
        assert!(compile_probe.success(), "compile Rust environment probe");
        let additional_env = [
            (
                OsString::from("RUSTC_WRAPPER"),
                wrapper_path.as_os_str().to_os_string(),
            ),
            (
                OsString::from("ARCANA_TEST_CLIPPY_LOG"),
                wrapper_log.as_os_str().to_os_string(),
            ),
        ];
        let lint_output =
            run_resolved_command_with_env(&lint, &project, &workspace, &additional_env)
                .await
                .expect("run the real Cargo Clippy intent through the production sandbox");
        let probe_log = std::fs::read_to_string(&wrapper_log).expect("read Rust environment probe");
        let acl = Command::new("icacls.exe")
            .arg(&config_dir)
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
            .unwrap_or_else(|error| format!("icacls unavailable: {error}"));
        assert_eq!(
            lint_output.exit_code, 0,
            "Clippy failed after sandbox config probe; rustc environment: {probe_log}\nACL: {acl}\nstdout: {}\nstderr: {}",
            lint_output.stdout, lint_output.stderr
        );
        assert!(probe_log.contains("config_dir=Ok("), "{probe_log}");
        assert!(probe_log.contains("config_file=Ok("), "{probe_log}");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn selected_msvc_toolchain_builds_through_the_production_sandbox() {
        if resolve_msvc_linker_directory().is_none() {
            return;
        }
        let workspace = tempfile::tempdir().expect("isolated Cargo workspace");
        let _lease = process_sandbox::retain_workspace(workspace.path());
        std::fs::create_dir(workspace.path().join("src")).expect("create source directory");
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname = \"sandbox-linker-probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write Cargo manifest");
        std::fs::write(workspace.path().join("src/main.rs"), "fn main() {}\n")
            .expect("write probe source");
        let build = command::resolve(Toolchain::Cargo, CommandIntent::Build).unwrap();
        let output = run_resolved_command(&build, workspace.path(), workspace.path())
            .await
            .expect("launch the selected MSVC toolchain through the production sandbox");
        assert_eq!(
            output.exit_code, 0,
            "sandboxed Cargo build failed\nstdout: {}\nstderr: {}",
            output.stdout, output.stderr
        );
    }

    #[tokio::test]
    async fn spawned_process_exit_status_is_the_status_that_is_returned() {
        let fixture = tempfile::tempdir().expect("isolated shell workspace");
        let project = fixture.path();
        #[cfg(windows)]
        let cases: &[(&str, &[&str], i32)] = &[
            ("cmd.exe", &["/d", "/c", "exit /b 0"], 0),
            ("cmd.exe", &["/d", "/c", "exit /b 17"], 17),
            (
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", "exit 0"],
                0,
            ),
            (
                "powershell.exe",
                &["-NoProfile", "-NonInteractive", "-Command", "exit 17"],
                17,
            ),
        ];
        #[cfg(not(windows))]
        let cases: &[(&str, &[&str], i32)] =
            &[("sh", &["-c", "exit 0"], 0), ("sh", &["-c", "exit 17"], 17)];

        for (program, args, expected) in cases {
            let actual = run_child_for_test(program, args, project).await;
            assert_eq!(actual.exit_code, *expected, "{program} {args:?}");
        }
    }

    #[tokio::test]
    async fn third_observed_build_failure_exposes_diagnostic_guidance_on_the_tool_path() {
        let root = tempfile::tempdir().expect("isolated fixture root");
        let home = root.path().join("home");
        let workspace = root.path().join("workspace");
        std::fs::create_dir_all(&home).expect("create isolated Arcana home");
        std::fs::create_dir_all(workspace.join("src")).expect("create broken project");
        let _home = crate::db::test_support::with_home(&home);
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"diagnostic-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write fixture manifest");
        std::fs::write(workspace.join("src/main.rs"), "fn main( {\n")
            .expect("write intentionally invalid Rust source");

        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated task DB");
        let task = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "diagnose repeated compiler failure".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "third observed build failure asks for diagnosis".into(),
                created_by: Some("p4-15-test".into()),
            })
            .expect("create task");
        tasks
            .update_status(task.id, "assigned", Some("worker-backend"))
            .expect("assign worker");
        tasks
            .claim_task_with_pid(task.id, "worker-backend", 993)
            .expect("claim task");

        let context = ToolContext {
            session_id: "diagnostic-session".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "diagnostic-build".into(),
            run_id: None,
            task_id: Some(task.id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        for failure in 1..=3 {
            let result = ProjectCommandTool
                .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
                .await;
            assert!(result.is_error, "broken fixture build {failure} must fail");
            if failure < 3 {
                assert!(
                    !result
                        .output
                        .contains("Three consecutive spawned build commands"),
                    "diagnosis guidance must wait until the threshold"
                );
            } else {
                let structured: Value =
                    serde_json::from_str(&result.output).expect("tool errors are JSON");
                let next_step = structured["error"]["next_step"]
                    .as_str()
                    .expect("structured error includes next-step guidance");
                assert!(
                    next_step.contains("Three consecutive spawned build commands exited nonzero")
                        && next_step.contains("source-free diagnostic codes")
                        && next_step.contains("request_supervisor_action(task_id=<current_task_id>")
                        && next_step.contains("request_type='build_retry_approval'")
                        && next_step.contains("Do not issue another build"),
                    "third observed failure must stop the worker and request a Supervisor decision: {}",
                    result.output
                );
            }
        }
        let fourth_attempt = ProjectCommandTool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
            .await;
        assert!(fourth_attempt.is_error, "the fourth build must be stopped");
        let stopped: Value =
            serde_json::from_str(&fourth_attempt.output).expect("tool errors are JSON");
        assert_eq!(
            stopped["error"]["code"], "build_diagnosis_required",
            "the supervisor decision boundary must be machine-readable"
        );
        assert!(
            stopped["error"]["next_step"]
                .as_str()
                .expect("next step")
                .contains("request_supervisor_action(request_type='build_retry_approval')"),
            "the worker must stop and escalate rather than retry: {}",
            fourth_attempt.output
        );
        let request_result = super::super::request_supervisor_action::RequestSupervisorActionTool
            .execute(
                json!({
                    "task_id": task.id,
                    "request_type": "build_retry_approval",
                    "payload": {"diagnosis": "syntax error isolated to src/main.rs"}
                }),
                &context,
            )
            .await;
        assert!(
            !request_result.is_error,
            "worker submits its diagnosis request"
        );
        let request_json: Value = serde_json::from_str(&request_result.output).unwrap();
        let request_id = request_json["request_id"].as_i64().unwrap();
        assert!(
            tasks
                .approve_build_retry_request(request_id, "worker-backend", "self approval")
                .is_err(),
            "worker cannot grant its own retry"
        );
        let mut supervisor_context = context.clone();
        supervisor_context.agent_role = "supervisor".into();
        supervisor_context.task_id = None;
        let approval_result = super::super::resolve_worker_request_tool::ResolveWorkerRequestTool
            .execute(
                json!({
                    "id": request_id,
                    "action": "approve_build_retry",
                    "note": "source fix is narrow and one verification batch is justified"
                }),
                &supervisor_context,
            )
            .await;
        assert!(
            !approval_result.is_error,
            "Supervisor approves one additional batch: {}",
            approval_result.output
        );
        let approved_attempt = ProjectCommandTool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
            .await;
        assert!(
            approved_attempt.is_error,
            "the one authorized batch must run and report the still-broken project: {}",
            approved_attempt.output
        );
        let reused_attempt = ProjectCommandTool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &context)
            .await;
        assert!(
            reused_attempt.is_error,
            "the consumed grant cannot be reused"
        );
        let reused: Value = serde_json::from_str(&reused_attempt.output).unwrap();
        assert_eq!(reused["error"]["code"], "build_diagnosis_required");
        assert_eq!(
            tasks.get_by_id(task.id).unwrap().unwrap().retry_count,
            4,
            "one approved failing batch records its observed exit"
        );
    }

    /// Supersedes `windows_batch_package_manager_shim_runs_with_fixed_argv`,
    /// which asserted pre-guard behaviour: a user-owned `.cmd` shim used to be
    /// granted to project code and executed. The guard added later refuses
    /// that, so this test pins the refusal *through the production launcher*
    /// (the sibling `user_owned_package_manager_shim_requires_explicit_staging`
    /// pins the guard helper itself). The fixed-argv batch path is still
    /// covered by the checked-in node corpus, whose `npm.cmd` lives under
    /// `Program Files` and therefore takes the same `cmd.exe /d /s /c` branch.
    #[cfg(windows)]
    #[tokio::test]
    async fn user_owned_package_manager_shim_is_refused_by_the_production_launcher() {
        let root = tempfile::Builder::new()
            .prefix("arcana cmd shim ")
            .tempdir()
            .expect("create a temporary directory whose path includes spaces");
        let shim = root.path().join("npm.cmd");
        std::fs::write(&shim, "@echo off\r\nexit /b 0\r\n")
            .expect("write local package-manager shim");
        let resolved = command::ResolvedCommand {
            program: shim.to_string_lossy().into_owned(),
            args: vec!["run".into(), "build".into()],
        };

        let error = match run_resolved_command(&resolved, root.path(), root.path()).await {
            Ok(output) => panic!(
                "a user-owned shim must not be granted to project code (exit {})",
                output.exit_code
            ),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error.to_string().contains("stage it explicitly"),
            "refusal must name the explicit staging path: {error}"
        );
    }

    #[tokio::test]
    async fn child_output_is_drained_but_captured_with_a_bounded_tail() {
        let project = std::env::current_dir().expect("current directory");
        #[cfg(windows)]
        let output = run_child_for_test(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::Out.Write(('x' * 80000)); [Console]::Error.Write(('y' * 80000))",
            ],
            &project,
        )
        .await;
        #[cfg(not(windows))]
        let output = run_child_for_test(
            "sh",
            &[
                "-c",
                "head -c 80000 /dev/zero | tr '\\0' x; head -c 80000 /dev/zero | tr '\\0' y >&2",
            ],
            &project,
        )
        .await;

        assert_eq!(output.exit_code, 0);
        assert!(output.stdout.len() <= MAX_STREAM_BYTES);
        assert!(output.stderr.len() <= MAX_STREAM_BYTES);
        assert!(output.stdout_truncated);
        assert!(output.stderr_truncated);
    }

    #[tokio::test]
    async fn shipped_project_command_persists_spawned_build_results_and_qc_reads_them() {
        let root = std::env::temp_dir().join(format!(
            "arcana-project-command-build-{}",
            uuid::Uuid::new_v4()
        ));
        let home = root.with_extension("home");
        let source = root.join("src");
        let _workspace_lease = process_sandbox::retain_workspace(&root);
        std::fs::create_dir_all(&home).expect("create isolated home");
        std::fs::create_dir_all(&source).expect("create fixture source");
        let _home = crate::db::test_support::with_home(&home);
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"project-command-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write fixture manifest");
        std::fs::write(
            root.join("Cargo.lock"),
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"project-command-fixture\"\nversion = \"0.1.0\"\n",
        )
        .expect("write stable fixture lockfile");
        let source_file = source.join("main.rs");
        std::fs::write(&source_file, "fn main() {}\n").expect("write valid source");

        let db = crate::db::tasks::TasksDb::open().expect("open isolated Arcana task DB");
        let task_id = db
            .create(crate::db::tasks::CreateTaskInput {
                title: "shipped project command build".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria:
                    "the shipped project_command records the actual cargo build result".into(),
                created_by: Some("test".into()),
            })
            .expect("create task")
            .id;
        assert!(crate::db::workers::WorkersDb::open()
            .expect("open isolated worker DB")
            .registered_slugs()
            .expect("read default workers")
            .iter()
            .any(|slug| slug == "worker-backend"));
        db.update_status(task_id, "assigned", Some("worker-backend"))
            .expect("assign task");
        db.claim_task_with_pid(task_id, "worker-backend", std::process::id())
            .expect("claim task");

        drop(db);
        let ctx = ToolContext {
            session_id: "session-build-proof".into(),
            working_dir: root.to_string_lossy().into_owned(),
            call_id: "project-command-build-proof".into(),
            run_id: None,
            task_id: Some(task_id),
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        };
        let status_tool =
            crate::agent::builtin_tools::update_task_status_tool::UpdateTaskStatusTool;
        let status_input = json!({
            "id": task_id,
            "status": "qc_review",
            "assigned_worker": "worker-backend"
        });
        let missing_evidence = status_tool.execute(status_input.clone(), &ctx).await;
        assert!(
            missing_evidence.is_error,
            "QC must refuse missing build evidence"
        );
        assert_eq!(
            crate::db::tasks::TasksDb::open()
                .unwrap()
                .get_by_id(task_id)
                .unwrap()
                .unwrap()
                .status,
            crate::db::tasks::TaskStatus::InProgress,
            "a refused QC transition must leave the task in progress"
        );

        let tool = ProjectCommandTool;
        std::fs::write(&source_file, "fn main( {\n").expect("write invalid source");
        let failed_result = tool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &ctx)
            .await;
        assert!(failed_result.is_error, "{}", failed_result.output);
        let failure: Value =
            serde_json::from_str(&failed_result.output).expect("project failure is valid JSON");
        assert_eq!(failure["schema_version"], 1);
        assert_eq!(failure["ok"], false);
        assert_eq!(failure["error"]["code"], "project_command_failed");
        let db = crate::db::tasks::TasksDb::open().expect("reopen isolated task DB");
        let latest = super::super::build_evidence::most_recent_build_in(&db, task_id)
            .expect("read failed build evidence")
            .unwrap_or_else(|| {
                panic!(
                    "failing build result is persisted; tool result was: {}",
                    failed_result.output
                )
            });
        assert_ne!(
            latest.exit_code, 0,
            "failing cargo process must be recorded"
        );
        assert!(
            super::super::build_evidence::qc_gate_error_in_for_workspace(
                &db,
                task_id,
                Some(std::path::Path::new(&ctx.working_dir)),
            )
            .expect("QC reads latest failed build")
            .is_some()
        );
        let failed_qc = status_tool.execute(status_input.clone(), &ctx).await;
        assert!(failed_qc.is_error, "QC must refuse the failed build result");
        assert_eq!(
            crate::db::tasks::TasksDb::open()
                .unwrap()
                .get_by_id(task_id)
                .unwrap()
                .unwrap()
                .status,
            crate::db::tasks::TaskStatus::InProgress,
            "a failed build must not advance the task to QC"
        );

        std::fs::write(&source_file, "fn main() {}\n").expect("restore valid source");

        let build_result = tool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &ctx)
            .await;
        assert!(!build_result.is_error, "{}", build_result.output);
        assert!(build_result.output.contains("Exit code: 0"));

        std::fs::write(
            root.join("build.rs"),
            "fn main() { std::thread::sleep(std::time::Duration::from_secs(5)); }\n",
        )
        .expect("add a deliberately slow build script after the passing build");
        let timeout_result = tool
            .execute(
                json!({"intent": "build", "toolchain": "cargo", "timeout_secs": 1}),
                &ctx,
            )
            .await;
        assert!(timeout_result.is_error, "the slow build must time out");
        let timeout_error: Value =
            serde_json::from_str(&timeout_result.output).expect("timeout result is structured");
        assert_eq!(timeout_error["error"]["code"], "operation_timed_out");
        let latest_after_timeout = super::super::build_evidence::most_recent_build(task_id)
            .expect("read build evidence after timeout")
            .expect("the real earlier pass remains the latest observed exit");
        assert_eq!(
            latest_after_timeout.exit_code, 0,
            "a timeout must not fabricate a process exit result"
        );

        let stale_qc = status_tool
            .execute(
                json!({
                    "id": task_id,
                    "status": "qc_review",
                    "assigned_worker": "worker-backend"
                }),
                &ctx,
            )
            .await;
        assert!(
            stale_qc.is_error,
            "workspace changes made before a later timeout must invalidate old QC evidence"
        );
        assert!(
            crate::db::tasks::TasksDb::open()
                .unwrap()
                .conn
                .execute(
                    "UPDATE tasks SET status='qc_review' WHERE id=?1",
                    rusqlite::params![task_id]
                )
                .is_err(),
            "the direct-SQL trigger must reject a stale workspace fingerprint"
        );
        std::fs::remove_file(root.join("build.rs")).expect("remove slow build script");
        let final_build = tool
            .execute(json!({"intent": "build", "toolchain": "cargo"}), &ctx)
            .await;
        assert!(
            !final_build.is_error,
            "a fresh complete batch must recover from the earlier timeout: {}",
            final_build.output
        );
        let selected_workspace =
            crate::platform::workspace::resolve_workspace_dir(Some(&ctx.working_dir))
                .expect("canonical selected workspace");
        let evidence_db = crate::db::tasks::TasksDb::open().expect("open QC evidence DB");
        let latest_batch = evidence_db
            .latest_project_build_batch_status(task_id)
            .expect("read latest batch")
            .expect("final passing batch");
        assert_eq!(
            crate::platform::workspace::workspace_identity(std::path::Path::new(&latest_batch.4))
                .expect("normalize evidence workspace"),
            crate::platform::workspace::workspace_identity(&selected_workspace)
                .expect("normalize selected workspace"),
            "persisted evidence workspace must equal the current selected workspace"
        );
        assert_eq!(latest_batch.0, "passed");
        assert!(
            super::super::build_evidence::qc_gate_error_in_for_workspace(
                &evidence_db,
                task_id,
                Some(&selected_workspace),
            )
            .expect("evaluate task scoped QC gate")
            .is_none()
        );
        let passing_qc = status_tool.execute(status_input, &ctx).await;
        assert!(
            !passing_qc.is_error,
            "the real status tool should accept persisted passing evidence: {}",
            passing_qc.output
        );
        assert_eq!(
            crate::db::tasks::TasksDb::open()
                .unwrap()
                .get_by_id(task_id)
                .unwrap()
                .unwrap()
                .status,
            crate::db::tasks::TaskStatus::QcReview,
            "a passing build should permit the real status transition"
        );
        drop(db);

        let reopened =
            crate::db::tasks::TasksDb::open().expect("reopen task DB after build process");
        let latest = super::super::build_evidence::most_recent_build_in(&reopened, task_id)
            .expect("read build evidence after reopen")
            .expect("latest build result exists");
        assert_eq!(latest.exit_code, 0, "latest passing build must win");
        assert_eq!(
            super::super::build_evidence::qc_gate_error_in_for_workspace(
                &reopened,
                task_id,
                Some(std::path::Path::new(&ctx.working_dir)),
            )
            .expect("QC reads latest passing build"),
            None
        );

        drop(reopened);
        let _ = std::fs::remove_dir_all(&home);
        let _ = std::fs::remove_dir_all(root);
    }

    // --- P4-19 · the second process writer under the same claim boundary ------
    //
    // `bash` already proves its claim refusal. `project_command` is the other
    // production process writer, and its claim check sits after project
    // detection but before any batch row, sandbox admission or child spawn —
    // so a worker that does not own an in-progress task can neither build nor
    // leave build evidence behind.

    fn write_claim_boundary_cargo_fixture(workspace: &std::path::Path) {
        std::fs::create_dir_all(workspace.join("src")).expect("create cargo source tree");
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"claim-boundary\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .expect("write dependency-free Cargo manifest");
        std::fs::write(workspace.join("src/lib.rs"), "pub fn value() -> u8 { 1 }\n")
            .expect("write cargo source");
    }

    fn claim_boundary_context(workspace: &std::path::Path, task_id: Option<i64>) -> ToolContext {
        ToolContext {
            session_id: "project-claim-boundary-session".into(),
            working_dir: workspace.to_string_lossy().into_owned(),
            call_id: "project-claim-boundary-call".into(),
            run_id: None,
            task_id,
            permission_mode: crate::agent::permission::PermissionMode::Autonomous,
            depth: 0,
            agent_role: "worker-backend".into(),

            run_token_budget: None,
        }
    }

    #[tokio::test]
    async fn project_command_process_is_refused_before_a_task_claim() {
        let home = tempfile::tempdir().expect("isolated Arcana home");
        let _home = crate::db::test_support::with_home(home.path());
        let workspace = tempfile::tempdir().expect("isolated selected workspace");
        write_claim_boundary_cargo_fixture(workspace.path());

        let unclaimed = ProjectCommandTool
            .execute(
                json!({"intent": "build"}),
                &claim_boundary_context(workspace.path(), None),
            )
            .await;
        let unclaimed_envelope: Value =
            serde_json::from_str(&unclaimed.output).expect("structured claim refusal");
        assert!(unclaimed.is_error);
        assert!(
            unclaimed_envelope["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Worker mutations require a task claim"),
            "an unclaimed worker may not build: {}",
            unclaimed.output
        );

        // The pinned-Ruff install is a network download plus a write into
        // Arcana's tool cache, so it must hit the same claim boundary as the
        // sandboxed build — and hit it *before* the installer can start.
        let unclaimed_install = ProjectCommandTool
            .execute(
                json!({"intent": "install", "toolchain": "python", "package": "ruff"}),
                &claim_boundary_context(workspace.path(), None),
            )
            .await;
        assert!(unclaimed_install.is_error);
        let install_envelope: Value =
            serde_json::from_str(&unclaimed_install.output).expect("structured install refusal");
        assert!(
            install_envelope["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Worker mutations require a task claim"),
            "an unclaimed worker may not install the pinned Ruff runtime: {}",
            unclaimed_install.output
        );

        // A task that exists but is owned by another worker is the same refusal
        // shape the file tools already prove, and it must leave no build batch.
        let tasks = crate::db::tasks::TasksDb::open().expect("open isolated TasksDb");
        let foreign = tasks
            .create(crate::db::tasks::CreateTaskInput {
                title: "foreign build claim".into(),
                description: None,
                priority: "p2".into(),
                task_type: "feature".into(),
                acceptance_criteria: "only the owning worker may build".into(),
                created_by: Some("p4-19-process-writer-test".into()),
            })
            .expect("create foreign task");
        tasks
            .update_status(foreign.id, "assigned", Some("worker-frontend"))
            .expect("assign the task to a different worker");
        tasks
            .claim_task_with_pid(foreign.id, "worker-frontend", std::process::id())
            .expect("claim the task for the other worker");
        drop(tasks);

        let foreign_attempt = ProjectCommandTool
            .execute(
                json!({"intent": "build"}),
                &claim_boundary_context(workspace.path(), Some(foreign.id)),
            )
            .await;
        let foreign_envelope: Value =
            serde_json::from_str(&foreign_attempt.output).expect("structured ownership refusal");
        assert!(foreign_attempt.is_error);
        assert!(
            foreign_envelope["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("no longer owns the in-progress task"),
            "a foreign claim may not build: {}",
            foreign_attempt.output
        );

        let tasks = crate::db::tasks::TasksDb::open().expect("reopen isolated TasksDb");
        assert_eq!(
            tasks
                .latest_project_build_batch_status(foreign.id)
                .expect("read build batches for the foreign task"),
            None,
            "a refused process must not persist a build batch"
        );
    }
}
