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

use crate::sidecar::client::OrchestratorClient;
use crate::sidecar::{resolve_memory_path, resolve_sidecar_path, SidecarManager};

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
    let mem_ok  = client.is_memory_healthy().await;
    let orch_ok = client.is_healthy().await;

    if mem_ok && !sidecar.memory_is_running() {
        sidecar.mark_memory_running();
    }
    if orch_ok && !sidecar.is_running() {
        sidecar.mark_running();
    }

    Ok(mem_ok && orch_ok)
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

    let orch_cwd = resolve_sidecar_path(path.as_deref()).map_err(|e| e.to_string())?;
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
