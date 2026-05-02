// Plugin runner — executes plugin install / verify / uninstall / run commands.
// Uses std::process::Command so it works without any async executor.

use anyhow::{anyhow, Context, Result};
use std::process::{Command, Stdio};

use super::manifest::{DependencyCheck, PluginManifest, RunResult, VerifyResult};

// ─── Command execution ────────────────────────────────────────────────────────

/// Execute a shell command and return (exit_code, stdout, stderr).
pub fn exec_shell(cmd_str: &str, work_dir: Option<&std::path::Path>) -> RunResult {
    let result = shell_command(cmd_str, work_dir);
    match result {
        Ok((code, stdout, stderr)) => RunResult { success: code == 0, stdout, stderr, exit_code: code },
        Err(e) => RunResult {
            success:   false,
            stdout:    String::new(),
            stderr:    e.to_string(),
            exit_code: -1,
        },
    }
}

fn shell_command(
    cmd_str: &str,
    work_dir: Option<&std::path::Path>,
) -> Result<(i32, String, String)> {
    let mut cmd = make_shell_cmd(cmd_str);

    if let Some(dir) = work_dir {
        cmd.current_dir(dir);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let output = cmd.output().context("Failed to launch shell command")?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    Ok((code, stdout, stderr))
}

#[cfg(target_os = "windows")]
fn make_shell_cmd(cmd_str: &str) -> Command {
    let mut c = Command::new("cmd");
    c.args(["/C", cmd_str]);
    c
}

#[cfg(not(target_os = "windows"))]
fn make_shell_cmd(cmd_str: &str) -> Command {
    let mut c = Command::new("sh");
    c.args(["-c", cmd_str]);
    c
}

// ─── Install ──────────────────────────────────────────────────────────────────

/// Run the platform install command for a plugin.
/// First tries a script file (`install.sh` / `install.ps1`) in the plugin dir,
/// then falls back to the inline manifest command.
pub fn install_plugin(
    manifest: &PluginManifest,
    install_dir: &std::path::Path,
) -> RunResult {
    // Try a script file first
    #[cfg(target_os = "windows")]
    let script = install_dir.join("install.ps1");
    #[cfg(not(target_os = "windows"))]
    let script = install_dir.join("install.sh");

    if script.exists() {
        #[cfg(target_os = "windows")]
        let cmd = format!("powershell -ExecutionPolicy Bypass -File \"{}\"", script.display());
        #[cfg(not(target_os = "windows"))]
        let cmd = format!("bash \"{}\"", script.display());
        return exec_shell(&cmd, Some(install_dir));
    }

    // Fall back to manifest inline command
    let Some(os_cmds) = &manifest.commands.install else {
        return RunResult {
            success:   false,
            stdout:    String::new(),
            stderr:    "Plugin has no install command defined".into(),
            exit_code: 1,
        };
    };

    let Some(cmd) = os_cmds.for_current_os() else {
        return RunResult {
            success:   false,
            stdout:    String::new(),
            stderr:    format!("Plugin '{}' has no install command for this OS", manifest.name),
            exit_code: 1,
        };
    };

    exec_shell(cmd, None)
}

// ─── Verify ───────────────────────────────────────────────────────────────────

/// Check all dependencies declared in the manifest and return a VerifyResult.
pub fn verify_plugin(manifest: &PluginManifest) -> VerifyResult {
    let mut checks = Vec::new();
    let mut all_required_ok = true;

    for dep in &manifest.dependencies {
        let (present, version) = if let Some(check_cmd) = &dep.check_cmd {
            match shell_command(check_cmd, None) {
                Ok((code, stdout, _)) if code == 0 => {
                    let ver = stdout.lines().next().map(|l| l.trim().to_string());
                    (true, ver)
                }
                _ => (false, None),
            }
        } else {
            // No check command — assume present
            (true, None)
        };

        if !present && !dep.optional {
            all_required_ok = false;
        }

        checks.push(DependencyCheck {
            name:     dep.name.clone(),
            required: !dep.optional,
            present,
            version,
            message:  if present {
                "OK".into()
            } else if dep.optional {
                format!("Optional dependency '{}' not found", dep.name)
            } else {
                format!("Required dependency '{}' not found", dep.name)
            },
        });
    }

    // Also run the manifest-level verify command if present
    let verify_msg = if let Some(v_cmd) = &manifest.commands.verify {
        match shell_command(v_cmd, None) {
            Ok((0, stdout, _)) => format!("Verify OK: {}", stdout.lines().next().unwrap_or("").trim()),
            Ok((code, _, stderr)) => {
                all_required_ok = false;
                format!("Verify command exited {code}: {}", stderr.trim())
            }
            Err(e) => {
                all_required_ok = false;
                format!("Verify command error: {e}")
            }
        }
    } else {
        "No verify command defined".into()
    };

    VerifyResult {
        success: all_required_ok,
        plugin:  manifest.name.clone(),
        checks,
        message: verify_msg,
    }
}

// ─── Uninstall ────────────────────────────────────────────────────────────────

/// Run the uninstall command for a plugin.
pub fn uninstall_plugin(manifest: &PluginManifest, install_dir: &std::path::Path) -> RunResult {
    // Try script file first
    #[cfg(target_os = "windows")]
    let script = install_dir.join("uninstall.ps1");
    #[cfg(not(target_os = "windows"))]
    let script = install_dir.join("uninstall.sh");

    if script.exists() {
        #[cfg(target_os = "windows")]
        let cmd = format!("powershell -ExecutionPolicy Bypass -File \"{}\"", script.display());
        #[cfg(not(target_os = "windows"))]
        let cmd = format!("bash \"{}\"", script.display());
        return exec_shell(&cmd, Some(install_dir));
    }

    let Some(cmd) = &manifest.commands.uninstall else {
        // No uninstall command — succeed silently (dir cleanup handled by manager)
        return RunResult {
            success:   true,
            stdout:    "No uninstall command; directory will be removed.".into(),
            stderr:    String::new(),
            exit_code: 0,
        };
    };

    exec_shell(cmd, None)
}

// ─── Run ─────────────────────────────────────────────────────────────────────

/// Execute the plugin's `run` command with optional extra arguments.
pub fn run_plugin(
    manifest: &PluginManifest,
    install_dir: &std::path::Path,
    extra_args: &[String],
) -> Result<RunResult> {
    let base_cmd = manifest
        .commands
        .run
        .as_deref()
        .ok_or_else(|| anyhow!("Plugin '{}' has no run command", manifest.name))?;

    let cmd = if extra_args.is_empty() {
        base_cmd.to_string()
    } else {
        format!("{base_cmd} {}", extra_args.join(" "))
    };

    Ok(exec_shell(&cmd, Some(install_dir)))
}

// ─── Script file writer ───────────────────────────────────────────────────────

/// Write the inline manifest commands as executable scripts in the plugin directory.
/// Called during install so that later runs can use the script files.
pub fn write_plugin_scripts(
    manifest: &PluginManifest,
    install_dir: &std::path::Path,
) -> Result<()> {
    let Some(os_cmds) = &manifest.commands.install else {
        return Ok(());
    };
    let Some(cmd) = os_cmds.for_current_os() else {
        return Ok(());
    };

    #[cfg(target_os = "windows")]
    {
        let script = install_dir.join("install.ps1");
        std::fs::write(&script, format!("# Auto-generated install script\r\n{cmd}\r\n"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = install_dir.join("install.sh");
        std::fs::write(&script, format!("#!/usr/bin/env bash\nset -e\n{cmd}\n"))?;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    }

    if let Some(uninstall_cmd) = &manifest.commands.uninstall {
        #[cfg(target_os = "windows")]
        {
            let script = install_dir.join("uninstall.ps1");
            std::fs::write(&script, format!("# Auto-generated uninstall script\r\n{uninstall_cmd}\r\n"))?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = install_dir.join("uninstall.sh");
            std::fs::write(&script, format!("#!/usr/bin/env bash\nset -e\n{uninstall_cmd}\n"))?;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
        }
    }

    Ok(())
}
