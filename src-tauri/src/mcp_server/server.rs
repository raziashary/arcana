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
use tracing::{info, error};

use crate::agent::ToolRegistry;
use super::tool_adapter::AutodevMcpServer;

/// Port the AutoDev MCP Server listens on. Adjacent to core-orchestrator (4000)
/// and memory-mcp-server (8000).
pub const MCP_SERVER_PORT: u16 = 8767;

/// Start the AutoDev MCP Server as a background Tokio task.
///
/// Returns a `tokio::task::JoinHandle` so the caller can await or abort it.
/// In practice this is spawned from `main.rs` during app startup (after the
/// ToolRegistry is fully populated) and dropped on app shutdown.
pub fn start_mcp_server(registry: Arc<ToolRegistry>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let bind_addr = format!("127.0.0.1:{}", MCP_SERVER_PORT);
        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("[autodev-mcp-server] Failed to bind to {}: {}", bind_addr, e);
                return;
            }
        };

        let service = StreamableHttpService::new(
            move || Ok(AutodevMcpServer::new(registry.clone())),
            Arc::new(LocalSessionManager::default()),
            rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default(),
        );

        let router = axum::Router::new()
            .route_service("/mcp", service);

        info!(
            "[autodev-mcp-server] Listening on http://{}/mcp — fleet workers can connect here",
            bind_addr
        );

        if let Err(e) = axum::serve(listener, router).await {
            error!("[autodev-mcp-server] Server error: {}", e);
        }
    })
}
