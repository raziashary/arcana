// server.rs — Streamable HTTP transport for the AutoDev MCP Server.
//
// Binds to 127.0.0.1:8767 (preferred) and exposes a single /mcp endpoint that
// handles both POST (client → server) and GET (server → client SSE stream)
// per the MCP 2025-03-26 Streamable HTTP specification.
//
// The server is started as an in-process Tokio task inside the Tauri app,
// sharing the same binary and ToolRegistry as the PA-mode AgentEngine.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU16, Ordering};

use rmcp::transport::streamable_http_server::{
    StreamableHttpService,
    session::local::LocalSessionManager,
};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::TcpListener;
use tracing::{info, warn, error};

use crate::agent::ToolRegistry;
use super::tool_adapter::AutodevMcpServer;

/// Preferred port the AutoDev MCP Server listens on. Adjacent to
/// core-orchestrator (4000) and memory-mcp-server (8000). May be bumped at
/// runtime to one of the alternates below if 8767 is held by a previous
/// instance in Windows TIME_WAIT — see `bind_with_fallback`.
pub const MCP_SERVER_PORT: u16 = 8767;

/// Alternate ports tried in order when 8767 cannot be bound even with
/// SO_REUSEADDR. Keep this short — the goal is to recover from the rare
/// case where another process truly owns 8767, not to scan a wide range.
const MCP_PORT_FALLBACKS: &[u16] = &[8768, 8769, 8770];

/// Holds the actual port the MCP server bound to once `start_mcp_server`
/// finishes its bind dance. 0 means "not bound yet". Readers (the
/// OrchestratorClient health probe and registration call) consult this
/// instead of hard-coding 8767 so they follow port fallback automatically.
static BOUND_PORT: AtomicU16 = AtomicU16::new(0);

/// Set once the server is fully listening — used by code paths that want
/// to wait for readiness rather than poll the port.
static READY: OnceLock<()> = OnceLock::new();

/// Returns the actual port the MCP server is bound to, or the preferred
/// default `MCP_SERVER_PORT` when bind has not finished yet. Callers that
/// need the real port for HTTP requests should always go through this.
pub fn current_mcp_port() -> u16 {
    let p = BOUND_PORT.load(Ordering::Acquire);
    if p == 0 { MCP_SERVER_PORT } else { p }
}

/// True once the server is listening (regardless of which port it picked).
pub fn is_ready() -> bool {
    READY.get().is_some()
}

// ---------------------------------------------------------------------------
// Server startup — no auth; localhost-only, in-process transport.
// ---------------------------------------------------------------------------

/// Build a TCP listener with SO_REUSEADDR set BEFORE bind so we survive
/// the Windows TIME_WAIT window after a previous instance closed. Without
/// this flag, Windows holds the port unavailable for up to 2 minutes after
/// the previous process exits, which stalls the entire fleet on every
/// hot-reload / restart. Returns the bound listener or an io::Error.
fn bind_reusable(addr: SocketAddr) -> std::io::Result<TcpListener> {
    let domain = if addr.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_nonblocking(true)?;
    // SO_REUSEADDR semantics differ by platform:
    //   - Linux/macOS: allow rebind while old sockets linger in TIME_WAIT.
    //   - Windows:     allow rebind to a port held in TIME_WAIT.
    // On Windows we deliberately do NOT also set SO_EXCLUSIVEADDRUSE; the
    // socket is bound to 127.0.0.1 only, so cross-user hijack is moot.
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    // 128 is plenty — this is an in-process server, no public traffic.
    socket.listen(128)?;
    TcpListener::from_std(socket.into())
}

/// Try the preferred port first, then walk the fallback list. The first
/// port that binds wins. If everything fails we return the final error so
/// the caller can drop into a slow-retry loop.
fn bind_with_fallback() -> std::io::Result<TcpListener> {
    let mut last_err: Option<std::io::Error> = None;
    let ports = std::iter::once(MCP_SERVER_PORT).chain(MCP_PORT_FALLBACKS.iter().copied());
    for port in ports {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        match bind_reusable(addr) {
            Ok(listener) => {
                BOUND_PORT.store(port, Ordering::Release);
                if port != MCP_SERVER_PORT {
                    warn!(
                        "[autodev-mcp-server] Preferred port {} unavailable; bound to fallback {} instead",
                        MCP_SERVER_PORT, port
                    );
                }
                return Ok(listener);
            }
            Err(e) => {
                warn!(
                    "[autodev-mcp-server] Failed to bind to 127.0.0.1:{}: {} — trying next",
                    port, e
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::new(
        std::io::ErrorKind::AddrInUse,
        "no preferred or fallback ports available",
    )))
}

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

        // Bind with SO_REUSEADDR + port fallback. If the preferred port AND
        // every fallback are unavailable (rare — usually means another app
        // owns them), drop into a slow-retry loop so the server eventually
        // recovers once the OS releases at least one of them.
        let listener = match bind_with_fallback() {
            Ok(l) => l,
            Err(e) => {
                warn!(
                    "[autodev-mcp-server] All preferred and fallback ports unavailable \
                     (last error: {}). Entering slow-retry loop (30s interval) — fleet \
                     workers will idle until a port becomes available.",
                    e
                );
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    match bind_with_fallback() {
                        Ok(l) => {
                            info!(
                                "[autodev-mcp-server] Recovered — bound to port {}",
                                current_mcp_port()
                            );
                            break l;
                        }
                        Err(e) => {
                            warn!(
                                "[autodev-mcp-server] Still unable to bind: {} — retry in 30s",
                                e
                            );
                        }
                    }
                }
            }
        };

        let port = current_mcp_port();
        let bind_addr = format!("127.0.0.1:{}", port);

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
        let _ = READY.set(());

        // If we ended up on a fallback port, capture the conflict and tell
        // the UI exactly what's holding the preferred port — including
        // whether it's a ghost socket from a previous crashed instance.
        // Emitted via the app handle the events module stashes globally so
        // we don't need a Tauri State here.
        let status = super::port_status::snapshot();
        if status.fallback_active {
            warn!(
                "[autodev-mcp-server] Port conflict on {}: {}",
                status.preferred_port, status.message
            );
            if let Some(app) = crate::events::app_handle() {
                use tauri::Emitter;
                let _ = app.emit(crate::events::EVT_MCP_PORT_CONFLICT, &status);
            }
        }

        if let Err(e) = axum::serve(listener, router).await {
            error!("[autodev-mcp-server] Server error: {}", e);
        }
    })
}
