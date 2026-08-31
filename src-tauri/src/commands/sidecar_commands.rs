// sidecar_commands.rs — Tauri commands for the AutoDev fleet sidecar management.
//
// Exposed to frontend via invoke():
//   get_sidecar_status()         → "stopped" | "starting" | "running" | "error: ..."
//   check_sidecar_connectivity() → quick health probe (both sidecars) without starting anything
//   start_sidecar(path?)         → starts memory-mcp-server first, then core-orchestrator
//   stop_sidecar()               → kills both managed processes
//
// Startup order enforced here:
//   1. memory-mcp-server  (:8000) — starts first, polled for up to 20s
//   2. core-orchestrator  (:4000) — starts after memory is healthy, polled for up to 30s

use std::sync::Arc;
use tauri::State;
use tracing::info;

use crate::agent::ToolRegistry;
use crate::sidecar::client::OrchestratorClient;
use crate::sidecar::{resolve_memory_path, resolve_orchestrator_path, SidecarManager, SidecarStatus};

// ─── Status ───────────────────────────────────────────────────────────────────

/// Return the current core-orchestrator status as a plain string for the frontend.
/// "running" only when both memory-mcp-server AND core-orchestrator are healthy.
#[tauri::command]
pub fn get_sidecar_status(sidecar: State<'_, Arc<SidecarManager>>) -> String {
    if sidecar.fleet_is_ready() {
        "running".into()
    } else {
        sidecar.status().to_string()
    }
}

/// Perform lightweight health checks against both :8000 (memory) and :4000 (orchestrator).
/// Updates sidecar manager state and returns true only when both respond OK.
#[tauri::command]
pub async fn check_sidecar_connectivity(
    client:  State<'_, Arc<OrchestratorClient>>,
    sidecar: State<'_, Arc<SidecarManager>>,
) -> Result<bool, String> {
    let mem_ok   = client.is_memory_healthy().await;
    let orch_ok  = client.is_healthy().await;
    let redis_ok = client.is_redis_healthy().await;

    if mem_ok && !sidecar.memory_is_running() {
        sidecar.mark_memory_running();
    }
    if orch_ok && !sidecar.is_running() {
        sidecar.mark_running();
    }
    if redis_ok {
        sidecar.mark_redis_running();
    } else {
        // Phase 3.4b — Memurai auto-start:
        // On the first connectivity check where Redis is unreachable (status == Stopped,
        // meaning we have never yet attempted a start), try to bring up the Memurai
        // Windows service automatically. On subsequent failures (status == Error) we
        // skip the start attempt so we don't spam the log with repeated failed `net start`
        // calls on every polling interval.
        let is_first_attempt = matches!(sidecar.redis_status(), SidecarStatus::Stopped);
        if is_first_attempt {
            info!("[sidecar_commands] Redis not reachable — attempting Memurai auto-start");
            if sidecar.try_start_memurai_service().await {
                // Give the service 2 s to bind before re-probing
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if client.is_redis_healthy().await {
                    sidecar.mark_redis_running();
                } else {
                    sidecar.mark_redis_unavailable();
                }
            } else {
                sidecar.mark_redis_unavailable();
            }
        }
        // If status was already Error (tried before), leave it as-is — no re-attempt.
    }

    // Fleet readiness is still memory + orchestrator + in-process MCP. Redis
    // is tracked but optional (warm tier only).
    Ok(mem_ok && orch_ok)
}

// ─── Full sidecar status (all 4 services) ────────────────────────────────────

/// Rich status snapshot for the frontend SidecarPanel. Each entry includes a
/// human-readable service name, the runtime status, a note, and whether the
/// service is required for fleet readiness.
#[derive(serde::Serialize)]
pub struct ServiceStatus {
    pub id:         String,
    pub name:       String,
    pub status:     String,
    pub required:   bool,
    pub port:       u16,
    pub note:       String,
}

/// Returns a snapshot of all 4 managed services. Non-blocking — uses cached
/// status only; call `check_sidecar_connectivity` first to refresh liveness.
#[tauri::command]
pub fn get_full_sidecar_status(
    sidecar: State<'_, Arc<SidecarManager>>,
) -> Vec<ServiceStatus> {
    vec![
        ServiceStatus {
            id:       "memory-mcp-server".into(),
            name:     "Memory MCP Server".into(),
            status:   sidecar.memory_status().to_string(),
            required: true,
            port:     8000,
            note:     "mem0 long-term memory — required for Fleet Mode 2".into(),
        },
        ServiceStatus {
            id:       "core-orchestrator".into(),
            name:     "Core Orchestrator".into(),
            status:   sidecar.status().to_string(),
            required: true,
            port:     4000,
            note:     "LivingAgent gateway (21 FSMs, A2A, litellm)".into(),
        },
        ServiceStatus {
            id:       "redis".into(),
            name:     "Redis / Memurai".into(),
            status:   sidecar.redis_status().to_string(),
            required: false,
            port:     6379,
            note:     "Warm memory tier + agent card store (optional — core-orch falls back to cold tier if missing)".into(),
        },
    ]
}

/// Snapshot of the last stderr lines captured from a sidecar's subprocess.
/// Used to render the actual Python traceback in a UI banner when a sidecar
/// crashes, instead of leaving the user staring at "Mode 1 fallback" with no
/// explanation. `id` accepts "core-orchestrator" or "memory-mcp-server".
#[tauri::command]
pub fn get_sidecar_stderr_tail(
    id:      String,
    sidecar: State<'_, Arc<SidecarManager>>,
) -> Vec<String> {
    sidecar.stderr_tail(&id)
}

// ─── Start ────────────────────────────────────────────────────────────────────

/// Start the full sidecar fleet: memory-mcp-server first, then core-orchestrator.
/// If `path` is None, both are auto-detected from AUTODEV_* env vars or relative dirs.
/// Returns "running" when both sidecars are healthy.
#[tauri::command]
pub async fn start_sidecar(
    path:    Option<String>,
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> Result<String, String> {
    // ── Phase 1: memory-mcp-server (:8000) ──────────────────────────────────
    if client.is_memory_healthy().await {
        sidecar.mark_memory_running();
        info!("[sidecar_commands] memory-mcp-server already reachable on :8000");
    } else {
        let mem_cwd = resolve_memory_path(None).map_err(|e| e.to_string())?;
        sidecar.start_memory(&mem_cwd).map_err(|e| e.to_string())?;

        // Poll memory health: up to 20 × 1s attempts
        let mut mem_ok = false;
        for attempt in 1..=20u32 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if client.is_memory_healthy().await {
                sidecar.mark_memory_running();
                info!("[sidecar_commands] memory-mcp-server ready after {attempt}s");
                mem_ok = true;
                break;
            }
        }
        if !mem_ok {
            sidecar.mark_memory_error("health check timed out after 20 s");
            return Err("memory-mcp-server did not become healthy within 20 seconds".into());
        }
    }

    // ── Phase 2: core-orchestrator (:4000) ──────────────────────────────────
    if client.is_healthy().await {
        sidecar.mark_running();
        client.clear_cache();
        info!("[sidecar_commands] core-orchestrator already reachable on :4000");
        return Ok("running".into());
    }

    let orch_cwd = resolve_orchestrator_path(path.as_deref()).map_err(|e| e.to_string())?;
    sidecar.start(&orch_cwd).map_err(|e| e.to_string())?;

    // Poll orchestrator health: up to 30 × 1s attempts
    for attempt in 1..=30u32 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.is_healthy().await {
            sidecar.mark_running();
            client.clear_cache();
            info!("[sidecar_commands] core-orchestrator ready after {attempt}s");
            return Ok("running".into());
        }
    }

    sidecar.mark_error("health check timed out after 30 s");
    Err("core-orchestrator did not become healthy within 30 seconds".into())
}

// ─── Stop ─────────────────────────────────────────────────────────────────────

/// Kill both managed sidecar processes (memory-mcp-server + core-orchestrator).
#[tauri::command]
pub fn stop_sidecar(
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> String {
    sidecar.stop_all();
    client.clear_cache();
    info!("[sidecar_commands] fleet stopped by user (both sidecars terminated)");
    "stopped".into()
}

// ─── Restart ─────────────────────────────────────────────────────────────────

/// Stop the full fleet and restart it in the correct order
/// (memory-mcp-server first, then core-orchestrator).
/// Waits for health probes on both services before returning.
#[tauri::command]
pub async fn restart_sidecar(
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> Result<String, String> {
    info!("[sidecar_commands] restart_sidecar: stopping fleet");
    sidecar.stop_all();
    client.clear_cache();

    // Brief pause to let ports be released by the OS
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // ── memory-mcp-server ────────────────────────────────────────────────────
    info!("[sidecar_commands] restart_sidecar: starting memory-mcp-server");
    let mem_cwd = resolve_memory_path(None).map_err(|e| e.to_string())?;
    sidecar.start_memory(&mem_cwd).map_err(|e| e.to_string())?;

    let mut mem_ok = false;
    for attempt in 1..=20u32 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.is_memory_healthy().await {
            sidecar.mark_memory_running();
            info!("[sidecar_commands] restart: memory-mcp-server ready after {attempt}s");
            mem_ok = true;
            break;
        }
    }
    if !mem_ok {
        sidecar.mark_memory_error("restart: health check timed out after 20 s");
        return Err("memory-mcp-server did not become healthy within 20 seconds on restart".into());
    }

    // ── core-orchestrator ────────────────────────────────────────────────────
    info!("[sidecar_commands] restart_sidecar: starting core-orchestrator");
    let orch_cwd = resolve_orchestrator_path(None).map_err(|e| e.to_string())?;
    sidecar.start(&orch_cwd).map_err(|e| e.to_string())?;

    for attempt in 1..=30u32 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.is_healthy().await {
            sidecar.mark_running();
            client.clear_cache();
            info!("[sidecar_commands] restart: core-orchestrator ready after {attempt}s");
            return Ok("running".into());
        }
    }

    sidecar.mark_error("restart: health check timed out after 30 s");
    Err("core-orchestrator did not become healthy within 30 seconds on restart".into())
}

// ─── MCP tool browser (Phase 3.4b) ───────────────────────────────────────────

/// Tool metadata returned to the frontend MCP tool browser. Mirrors the shape
/// that `autodev-mcp-server` advertises via MCP `tools/list` but queried
/// directly from the in-process ToolRegistry to avoid a JSON-RPC round-trip.
#[derive(serde::Serialize)]
pub struct McpToolInfo {
    pub name:        String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Returns every tool exposed by `autodev-mcp-server`. The frontend renders
/// this list in the MCP tool browser so developers can inspect available
/// capabilities without connecting an external MCP client.
#[tauri::command]
pub fn list_mcp_tools(
    registry: State<'_, Arc<ToolRegistry>>,
) -> Vec<McpToolInfo> {
    let mut tools: Vec<McpToolInfo> = registry
        .all()
        .into_iter()
        .map(|t| McpToolInfo {
            name:         t.name().to_string(),
            description:  t.description().to_string(),
            input_schema: t.input_schema(),
        })
        .collect();
    tools.sort_by(|a, b| a.name.cmp(&b.name));
    tools
}

// ─── Per-service lifecycle (Phase 3.4b SidecarPanel) ─────────────────────────

/// Start **only** memory-mcp-server. Orchestrator is left untouched.
/// Auto-detects cwd from AUTODEV_MEMORY_PATH or relative candidates.
/// Returns "running" when health probe passes, or error string.
#[tauri::command]
pub async fn start_memory_mcp(
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> Result<String, String> {
    if client.is_memory_healthy().await {
        sidecar.mark_memory_running();
        return Ok("running".into());
    }
    let mem_cwd = resolve_memory_path(None).map_err(|e| e.to_string())?;
    sidecar.start_memory(&mem_cwd).map_err(|e| e.to_string())?;

    for attempt in 1..=20u32 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.is_memory_healthy().await {
            sidecar.mark_memory_running();
            info!("[sidecar_commands] memory-mcp-server ready after {attempt}s");
            return Ok("running".into());
        }
    }
    sidecar.mark_memory_error("health check timed out after 20 s");
    Err("memory-mcp-server did not become healthy within 20 seconds".into())
}

/// Stop **only** memory-mcp-server.
#[tauri::command]
pub fn stop_memory_mcp(sidecar: State<'_, Arc<SidecarManager>>) -> String {
    sidecar.stop_memory();
    info!("[sidecar_commands] memory-mcp-server stopped by user");
    "stopped".into()
}

/// Start **only** core-orchestrator. Does NOT start memory-mcp-server.
/// Auto-detects cwd from AUTODEV_SIDECAR_PATH or relative candidates.
/// Returns "running" when health probe passes, or error string.
#[tauri::command]
pub async fn start_orchestrator(
    path:    Option<String>,
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> Result<String, String> {
    if client.is_healthy().await {
        sidecar.mark_running();
        client.clear_cache();
        return Ok("running".into());
    }
    let orch_cwd = resolve_orchestrator_path(path.as_deref()).map_err(|e| e.to_string())?;
    sidecar.start(&orch_cwd).map_err(|e| e.to_string())?;

    for attempt in 1..=30u32 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        if client.is_healthy().await {
            sidecar.mark_running();
            client.clear_cache();
            info!("[sidecar_commands] core-orchestrator ready after {attempt}s");
            return Ok("running".into());
        }
    }
    sidecar.mark_error("health check timed out after 30 s");
    Err("core-orchestrator did not become healthy within 30 seconds".into())
}

/// Stop **only** core-orchestrator.
#[tauri::command]
pub fn stop_orchestrator(
    sidecar: State<'_, Arc<SidecarManager>>,
    client:  State<'_, Arc<OrchestratorClient>>,
) -> String {
    sidecar.stop();
    client.clear_cache();
    info!("[sidecar_commands] core-orchestrator stopped by user");
    "stopped".into()
}

// ─── Resource usage (Phase 3.4b) ─────────────────────────────────────────────

/// CPU% and resident memory for a managed sidecar process.
/// `service_id` must be `"memory-mcp-server"` or `"core-orchestrator"`.
/// Requires the process to be currently running (PID known).
#[derive(serde::Serialize)]
pub struct ResourceSnapshot {
    pub cpu_percent:  f32,
    pub memory_bytes: u64,
    pub pid:          u32,
}

#[tauri::command]
pub fn get_resource_usage(service_id: String, sidecar: State<'_, Arc<SidecarManager>>)
    -> Result<ResourceSnapshot, String>
{
    use sysinfo::System;

    let pid = match service_id.as_str() {
        "memory-mcp-server" => sidecar.memory_pid(),
        "core-orchestrator" => sidecar.orch_pid(),
        other => return Err(format!("Resource monitoring not available for '{other}'")),
    };

    let pid = pid.ok_or_else(|| format!("{service_id} is not running (no PID)"))?;

    let mut sys = System::new_all();
    sys.refresh_all();

    // CPU usage in sysinfo requires two refreshes separated by the update interval
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_cpu_usage();

    let process = sys.process(sysinfo::Pid::from(pid as usize))
        .ok_or_else(|| format!("Process {pid} not found in system table"))?;

    Ok(ResourceSnapshot {
        cpu_percent:  process.cpu_usage(),
        memory_bytes: process.memory(),
        pid,
    })
}

// ─── Sidecar log tail (Phase 3.4b) ─────────────────────────────────────────────

/// Read the tail of a sidecar-specific log file (not the date-stamped tracing
/// logs used by the Logs tab). Returns empty string if the log file does not
/// exist yet.
///
/// Supported `service_id` values:
///   - `"memory-mcp-server"`  → `~/.autodev/logs/memory-mcp.log`
///   - `"core-orchestrator"`  → `~/.autodev/logs/core-orchestrator.log`
#[tauri::command]
pub fn read_sidecar_log(service_id: String, tail_lines: Option<usize>) -> Result<String, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Cannot resolve home directory".to_string())?;
    let log_dir = std::path::PathBuf::from(home).join(".autodev").join("logs");

    let file_name = match service_id.as_str() {
        "memory-mcp-server"  => "memory-mcp.log",
        "core-orchestrator"  => "core-orchestrator.log",
        other => return Err(format!("No log file mapped for service '{other}'")),
    };

    let path = log_dir.join(file_name);
    if !path.exists() {
        return Ok(String::new());
    }

    let limit = tail_lines.unwrap_or(500);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read log file: {e}"))?;

    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let visible = if total > limit {
        lines.into_iter().rev().take(limit).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>()
    } else {
        lines
    };

    Ok(visible.join("\n"))
}

// ─── Submodule path status (Phase 5.3 — Installer bundling) ─────────────────

/// Status of a submodule (core-orchestrator or memory-mcp-server).
#[derive(serde::Serialize)]
pub struct SubmoduleStatus {
    pub id:          String,
    pub name:        String,
    pub path:        String,
    pub exists:      bool,
    pub source:      String,  // "installed" | "env_var" | "relative" | "not_found"
    pub required:    bool,
}

/// Check if both submodules exist and return their resolved paths.
/// The frontend uses this to display submodule status in Settings.
#[tauri::command]
pub fn get_submodule_paths() -> Vec<SubmoduleStatus> {
    let orch_result = crate::sidecar::resolve_orchestrator_path(None);
    let memory_result = crate::sidecar::resolve_memory_path(None);

    let orch_status = match &orch_result {
        Ok(path) => {
            let path_str = path.display().to_string();
            // Priority mirrors resolve_orchestrator_path: env var wins, then
            // check for installed-copy markers, then classify as relative/bundled.
            let source = if std::env::var("AUTODEV_SIDECAR_PATH").is_ok() {
                "env_var".to_string()
            } else if path_str.contains("AutoDev") && path_str.contains("sidecars") {
                "installed".to_string()
            } else if path_str.contains("Programs") {
                "installed".to_string()
            } else {
                "relative".to_string()
            };
            SubmoduleStatus {
                id:       "core-orchestrator".into(),
                name:     "Core Orchestrator".into(),
                path:     path_str,
                exists:   true,
                source,
                required: true,
            }
        }
        Err(_) => SubmoduleStatus {
            id:       "core-orchestrator".into(),
            name:     "Core Orchestrator".into(),
            path:     "not found".into(),
            exists:   false,
            source:   "not_found".into(),
            required: true,
        },
    };

    let memory_status = match &memory_result {
        Ok(path) => {
            let path_str = path.display().to_string();
            let source = if std::env::var("AUTODEV_MEMORY_PATH").is_ok() {
                "env_var".to_string()
            } else if path_str.contains("AutoDev") && path_str.contains("sidecars") {
                "installed".to_string()
            } else if path_str.contains("Programs") {
                "installed".to_string()
            } else {
                "relative".to_string()
            };
            SubmoduleStatus {
                id:       "memory-mcp-server".into(),
                name:     "Memory MCP Server".into(),
                path:     path_str,
                exists:   true,
                source,
                required: true,
            }
        }
        Err(_) => SubmoduleStatus {
            id:       "memory-mcp-server".into(),
            name:     "Memory MCP Server".into(),
            path:     "not found".into(),
            exists:   false,
            source:   "not_found".into(),
            required: true,
        },
    };

    vec![orch_status, memory_status]
}

// ─── MCP Connection Health — Ruflo Feature 6 ─────────────────────────────────

/// Return the current health snapshot for all registered external MCP servers.
/// Called by the Sidecar Panel and status bar to show connection state badges.
#[tauri::command]
pub async fn get_mcp_health(
    monitor: State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
) -> Result<Vec<crate::mcp::health::McpServerHealth>, String> {
    Ok(monitor.get_all_health().await)
}

/// Manually trigger a reconnect attempt for a named external MCP server.
/// Called when the user clicks the "Reconnect" button in the Sidecar Panel.
#[tauri::command]
pub async fn force_reconnect_mcp(
    server_name: String,
    monitor:     State<'_, Arc<crate::mcp::health::McpHealthMonitor>>,
    app:         tauri::AppHandle,
) -> Result<(), String> {
    monitor.force_reconnect(&server_name, &app).await;
    Ok(())
}
