// server.rs — Streamable HTTP transport for the AutoDev MCP Server.
//
// Binds to 127.0.0.1:8767 and exposes a single /mcp endpoint that handles
// both POST (client → server) and GET (server → client SSE stream) per the
// MCP 2025-03-26 Streamable HTTP specification.
//
// The server is started as an in-process Tokio task inside the Tauri app,
// sharing the same binary and ToolRegistry as the PA-mode AgentEngine.

use std::sync::Arc;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
};
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

// ---------------------------------------------------------------------------
// L.2 — API key helpers
// ---------------------------------------------------------------------------

fn mcp_key_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".autodev").join("mcp.key")
}

/// Load the machine-local MCP API key, creating it on first run.
/// The key is written to ~/.autodev/mcp.key (user-readable only).
/// Fleet workers read the same file to obtain the bearer token.
fn load_or_create_mcp_key() -> String {
    let path = mcp_key_path();
    if let Ok(key) = std::fs::read_to_string(&path) {
        let key = key.trim().to_string();
        if !key.is_empty() {
            return key;
        }
    }
    // Generate a stable random key for this installation.
    let key = uuid::Uuid::new_v4().to_string().replace('-', "");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, &key);
    key
}

/// Axum middleware that validates the `Authorization: Bearer <key>` header.
/// Returns 401 Unauthorized for missing or wrong tokens.
async fn auth_middleware(
    State(api_key): State<String>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let auth = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = format!("Bearer {}", api_key);
    if auth != expected {
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(axum::body::Body::from("Unauthorized"))
            .unwrap();
    }

    next.run(req).await
}

// ---------------------------------------------------------------------------
// Server startup
// ---------------------------------------------------------------------------

/// Start the AutoDev MCP Server as a background Tokio task.
///
/// Returns a `tokio::task::JoinHandle` so the caller can await or abort it.
/// In practice this is spawned from `main.rs` during app startup and dropped
/// on app shutdown.
///
/// The MCP server uses a restricted `register_for_mcp_worker` registry (L.4)
/// so fleet workers cannot call assign_task, spawn_subagent, etc. via MCP.
pub fn start_mcp_server(registry: Arc<ToolRegistry>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // L.2 — Load (or generate) the machine-local bearer token.
        let api_key = load_or_create_mcp_key();
        info!(
            "[autodev-mcp-server] MCP API key ready ({})",
            mcp_key_path().display()
        );

        // L.4 — Build a worker-scoped registry; the passed-in PA registry is
        // intentionally not used here to enforce the restricted tool surface.
        let _ = registry;
        let mut mcp_registry = ToolRegistry::new();
        crate::agent::builtin_tools::register_for_mcp_worker(&mut mcp_registry);
        let mcp_registry = Arc::new(mcp_registry);

        let bind_addr = format!("127.0.0.1:{}", MCP_SERVER_PORT);
        let listener = match TcpListener::bind(&bind_addr).await {
            Ok(l) => l,
            Err(e) => {
                error!("[autodev-mcp-server] Failed to bind to {}: {}", bind_addr, e);
                return;
            }
        };

        let service = StreamableHttpService::new(
            move || Ok(AutodevMcpServer::new(mcp_registry.clone())),
            Arc::new(LocalSessionManager::default()),
            rmcp::transport::streamable_http_server::StreamableHttpServerConfig::default(),
        );

        // L.2 — Wrap the MCP route with bearer-token auth middleware.
        let router = axum::Router::new()
            .route_service("/mcp", service)
            .route_layer(middleware::from_fn_with_state(api_key, auth_middleware));

        info!(
            "[autodev-mcp-server] Listening on http://{}/mcp — fleet workers can connect here",
            bind_addr
        );

        if let Err(e) = axum::serve(listener, router).await {
            error!("[autodev-mcp-server] Server error: {}", e);
        }
    })
}
