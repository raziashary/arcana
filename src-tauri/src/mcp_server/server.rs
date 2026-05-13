// server.rs — Streamable HTTP transport for the AutoDev MCP Server.
//
// Binds to 127.0.0.1:8767 and exposes a single /mcp endpoint that handles
// both POST (client → server) and GET (server → client SSE stream) per the
// MCP 2025-03-26 Streamable HTTP specification.
//
// The server is started as an in-process Tokio task inside the Tauri app,
// sharing the same binary and ToolRegistry as the PA-mode AgentEngine.

use std::sync::Arc;

use rmcp::transport::streamable_http_server::{
    StreamableHttpService,
    session::local::LocalSessionManager,
};
use tokio::net::TcpListener;
use tracing::{info, warn, error};

use crate::agent::ToolRegistry;
use super::tool_adapter::AutodevMcpServer;

/// Port the AutoDev MCP Server listens on. Adjacent to core-orchestrator (4000)
/// and memory-mcp-server (8000).
pub const MCP_SERVER_PORT: u16 = 8767;

// ---------------------------------------------------------------------------
// Server startup — no auth; localhost-only, in-process transport.
// ---------------------------------------------------------------------------

/// Start the AutoDev MCP Server as a background Tokio task.
///
/// Uses `tauri::async_runtime::spawn` (not `tokio::spawn`) so it is safe to
/// call from Tauri's synchronous `.setup()` callback, which runs on a thread
/// that does NOT have an active Tokio runtime context installed. `tokio::spawn`
/// would panic with "there is no reactor running" in that context.
///
/// The MCP server uses `register_for_mcp_worker` (L.4) — a fleet-scoped registry
/// that exposes every tool fleet workers need in Mode 2 (the primary execution mode):
///   • File/exec: read_file, write_file, edit_file, append_file, check_file,
///                bash, grep, glob, list_directory
///   • Task board: list_tasks, get_task, create_task, assign_task, update_task_status,
///                 claim_task, set_task_budget
///   • Communication: request_supervisor_action, suggest_worker
///   • QC/analysis: analyze_diff, annotate_file
///   • Epic/phase: get_epic_progress, get_ready_tasks_in_phase
///   • Delegation: spawn_subagent, spawn_parallel_subagents (pool hard caps + depth
///                 guard in SpawnSubagentTool prevent runaway fan-out)
///   • Planning:   planner (full DSL — workers use claimItem/createSubplan/submitForQC;
///                 Supervisor uses breakDownEpic)
///
/// NOT exposed (intentional):
///   • create_epic_tool / create_phased_task_tool — Supervisor/PA-only operations
///   • search_memory / remember_this — workers access memory via mcp-memory server directly
///   • validate — internal PA utility, not relevant to fleet task execution
///   • graph_tools / graph_prompts — use GraphStore API, not Tool trait; PA-only analysis
pub fn start_mcp_server(registry: Arc<ToolRegistry>) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        // L.4 — Build a worker-scoped registry; the passed-in PA registry is
        // intentionally not used here to enforce the restricted tool surface.
        let _ = registry;
        let mut mcp_registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mcp_worker(&mut mcp_registry);
        let mcp_registry = Arc::new(mcp_registry);

        let bind_addr = format!("127.0.0.1:{}", MCP_SERVER_PORT);

        // Retry binding to handle Windows TIME_WAIT (OS error 10048 / WSAEADDRINUSE)
        // that occurs when the app restarts quickly after a crash.
        //
        // Strategy — two phases:
        //   Phase 1 (fast): up to 5 attempts with exponential backoff (500ms→4s = 7.5s total).
        //                   Covers brief port-release delays after a clean shutdown.
        //   Phase 2 (slow): if fast retries are exhausted, retry every 30s indefinitely.
        //                   Windows TIME_WAIT can last 30–120s; this phase ensures the server
        //                   recovers without requiring an app restart.
        let mut listener_opt: Option<TcpListener> = None;

        // Phase 1 — fast retries
        'fast: for attempt in 1u32..=5 {
            match TcpListener::bind(&bind_addr).await {
                Ok(l) => {
                    if attempt > 1 {
                        info!(
                            "[autodev-mcp-server] Bound to {} on attempt {}",
                            bind_addr, attempt
                        );
                    }
                    listener_opt = Some(l);
                    break 'fast;
                }
                Err(e) if attempt < 5 => {
                    let delay_ms = 500u64 * (1 << (attempt - 1)); // 500, 1000, 2000, 4000 ms
                    warn!(
                        "[autodev-mcp-server] Failed to bind to {} (attempt {}/5): {}. \
                         Retrying in {}ms (port may be in TIME_WAIT after restart)…",
                        bind_addr, attempt, e, delay_ms
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                }
                Err(e) => {
                    warn!(
                        "[autodev-mcp-server] Fast retries exhausted for {} (last error: {}). \
                         Switching to slow-retry mode (30s interval) — will recover once the \
                         OS releases the port from TIME_WAIT. Fleet workers continue in Mode 1 \
                         until then.",
                        bind_addr, e
                    );
                }
            }
        }

        // Phase 2 — slow retry loop (runs only if Phase 1 exhausted without binding)
        if listener_opt.is_none() {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                match TcpListener::bind(&bind_addr).await {
                    Ok(l) => {
                        info!(
                            "[autodev-mcp-server] Recovered — bound to {} after TIME_WAIT cleared. \
                             Fleet workers can now use Mode 2 tooling.",
                            bind_addr
                        );
                        listener_opt = Some(l);
                        break;
                    }
                    Err(e) => {
                        warn!(
                            "[autodev-mcp-server] Port {} still in TIME_WAIT: {} — retry in 30s",
                            bind_addr, e
                        );
                    }
                }
            }
        }

        let listener = listener_opt.expect("loop guarantees Some or early return");

        let service = StreamableHttpService::new(
            move || Ok(AutodevMcpServer::new(mcp_registry.clone())),
            Arc::new(LocalSessionManager::default()),
            rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default(),
        );

        let router = axum::Router::new()
            .route_service("/mcp", service);
        // No auth middleware — localhost-only, in-process, zero external exposure.

        info!(
            "[autodev-mcp-server] Listening on http://{}/mcp — fleet workers can connect here",
            bind_addr
        );

        if let Err(e) = axum::serve(listener, router).await {
            error!("[autodev-mcp-server] Server error: {}", e);
        }
    })
}
