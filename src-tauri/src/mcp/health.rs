// health_monitor.rs — Ruflo Feature 6. Periodic liveness pings on connected
// external MCP servers. When a connection drops, the UI shows an alert and
// automatic reconnection is attempted. Internal AutoDev services (memory-mcp,
// core-orchestrator, autodev-mcp-server) have their own watchdog — this file
// handles user-configured *external* MCP servers only (e.g. GitHub MCP).
//
// Transport variants:
//   HTTP   — GET {endpoint}/health with 5s timeout; returns latency
//   Stdio  — Cannot be pinged without a live process handle; registered for
//             display purposes only, state stays at Connected (assumed).
//
// See: RUFLO_IMPL_PLAN.md §6 for full specification.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// ─── Connection state ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum McpConnectionState {
    Connected,
    Degraded, // responded slowly or with errors
    Disconnected,
    Reconnecting,
    NotMonitored, // stdio transport — cannot ping
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerHealth {
    pub server_name: String,
    pub endpoint: String,  // URL for HTTP; "stdio" marker for stdio
    pub transport: String, // "http" | "stdio"
    pub state: McpConnectionState,
    pub last_ping_at: Option<String>,
    pub last_success_at: Option<String>,
    pub consecutive_failures: u32,
    pub avg_latency_ms: f64,
    pub error_message: Option<String>,
}

// ─── Ping internals ──────────────────────────────────────────────────────────

struct PingResult {
    latency_ms: u64,
}

#[derive(Debug)]
enum PingError {
    Timeout,
    ConnectionRefused,
    InvalidResponse(String),
}

// ─── Health monitor ───────────────────────────────────────────────────────────

pub struct McpHealthMonitor {
    connections: Arc<RwLock<HashMap<String, McpServerHealth>>>,
    /// How often to run a ping cycle (default: 30s)
    pub ping_interval_secs: u64,
    /// How many consecutive failures before transitioning to Disconnected (default: 3)
    pub failure_threshold: u32,
    /// Seconds to wait before first reconnect attempt (default: 10s)
    pub reconnect_delay_secs: u64,
    /// HTTP client shared across pings (one connection pool)
    http_client: reqwest::Client,
}

impl McpHealthMonitor {
    pub fn new(ping_interval_secs: u64, failure_threshold: u32, reconnect_delay_secs: u64) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            ping_interval_secs,
            failure_threshold,
            reconnect_delay_secs,
            http_client,
        }
    }

    /// Register an external MCP server for health monitoring.
    /// If endpoint starts with "http" it will be polled; otherwise it's treated as stdio.
    pub async fn register_server(&self, name: &str, endpoint: &str) {
        let transport = if endpoint.starts_with("http") {
            "http"
        } else {
            "stdio"
        };
        let state = if transport == "http" {
            McpConnectionState::Disconnected
        } else {
            McpConnectionState::NotMonitored
        };
        let health = McpServerHealth {
            server_name: name.to_string(),
            endpoint: endpoint.to_string(),
            transport: transport.to_string(),
            state,
            last_ping_at: None,
            last_success_at: None,
            consecutive_failures: 0,
            avg_latency_ms: 0.0,
            error_message: None,
        };
        self.connections
            .write()
            .await
            .insert(name.to_string(), health);
        info!(
            "[mcp-health] registered '{}' ({} {})",
            name, transport, endpoint
        );
    }

    /// Remove a server from the registry (e.g. when user removes it from Settings).
    pub async fn unregister_server(&self, name: &str) {
        self.connections.write().await.remove(name);
        info!("[mcp-health] unregistered '{}'", name);
    }

    /// Return the current health snapshot of all registered servers.
    pub async fn get_all_health(&self) -> Vec<McpServerHealth> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Trigger a single reconnect attempt for the named server.
    /// Called by the `force_reconnect_mcp` Tauri command.
    pub async fn force_reconnect(&self, name: &str, app: &AppHandle) {
        let endpoint = {
            let guard = self.connections.read().await;
            guard.get(name).map(|h| h.endpoint.clone())
        };
        if let Some(ep) = endpoint {
            // Reset failure count so the next ping won't immediately re-disconnect
            {
                let mut guard = self.connections.write().await;
                if let Some(h) = guard.get_mut(name) {
                    h.state = McpConnectionState::Reconnecting;
                    h.consecutive_failures = 0;
                }
            }
            self.attempt_reconnect(name, &ep, app).await;
        }
    }

    // ── Background task ──────────────────────────────────────────────────────

    /// Spawn the monitoring background loop. Call once after setup.
    pub fn start_monitoring(self: Arc<Self>, app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            Self::monitoring_loop(self, app).await;
        });
    }

    async fn monitoring_loop(monitor: Arc<Self>, app: AppHandle) {
        let mut interval = tokio::time::interval(Duration::from_secs(monitor.ping_interval_secs));
        loop {
            interval.tick().await;

            // Snapshot names to avoid holding lock during I/O
            let server_names: Vec<String> =
                monitor.connections.read().await.keys().cloned().collect();

            for name in &server_names {
                let result = monitor.ping_server(name).await;
                monitor.process_ping_result(name, result, &app).await;
            }

            // Emit a full health-update snapshot after each ping cycle
            let health = monitor.get_all_health().await;
            if let Some(app_h) = crate::events::app_handle() {
                let _ = app_h.emit(crate::events::EVT_MCP_HEALTH_UPDATE, &health);
            }
            let _ = app; // keep alive
        }
    }

    // ── Ping ────────────────────────────────────────────────────────────────────

    async fn ping_server(&self, name: &str) -> Result<PingResult, PingError> {
        let (endpoint, transport) = {
            let guard = self.connections.read().await;
            match guard.get(name) {
                Some(h) => (h.endpoint.clone(), h.transport.clone()),
                None => return Err(PingError::InvalidResponse("server not found".into())),
            }
        };

        if transport == "stdio" {
            // Cannot ping stdio — consider healthy by assumption
            return Ok(PingResult { latency_ms: 0 });
        }

        // HTTP ping — try /health then fall back to root
        let url = if endpoint.ends_with('/') {
            format!("{}health", endpoint)
        } else {
            format!("{}/health", endpoint)
        };

        let start = Instant::now();
        match self.http_client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 404 => {
                // 404 on /health = server is up but has no health endpoint (fine)
                Ok(PingResult {
                    latency_ms: start.elapsed().as_millis() as u64,
                })
            }
            Ok(resp) => Err(PingError::InvalidResponse(format!(
                "HTTP {}",
                resp.status()
            ))),
            Err(e) if e.is_timeout() => Err(PingError::Timeout),
            Err(e) if e.is_connect() => Err(PingError::ConnectionRefused),
            Err(e) => Err(PingError::InvalidResponse(e.to_string())),
        }
    }

    // ── Process ping result ──────────────────────────────────────────────────────

    async fn process_ping_result(
        &self,
        name: &str,
        result: Result<PingResult, PingError>,
        app: &AppHandle,
    ) {
        let now = chrono::Utc::now().to_rfc3339();

        let mut guard = self.connections.write().await;
        let health = match guard.get_mut(name) {
            Some(h) => h,
            None => return,
        };

        // Stdio: just mark last_ping_at and stay NotMonitored
        if health.transport == "stdio" {
            health.last_ping_at = Some(now);
            return;
        }

        match result {
            Ok(ping) => {
                let prev_state = health.state.clone();
                health.last_ping_at = Some(now.clone());
                health.last_success_at = Some(now);
                health.consecutive_failures = 0;
                health.error_message = None;

                // Update rolling average latency (EMA with α=0.3)
                health.avg_latency_ms = if health.avg_latency_ms == 0.0 {
                    ping.latency_ms as f64
                } else {
                    health.avg_latency_ms * 0.7 + ping.latency_ms as f64 * 0.3
                };

                // Classify based on latency
                let new_state = if ping.latency_ms > 2000 {
                    McpConnectionState::Degraded
                } else {
                    McpConnectionState::Connected
                };

                if new_state == McpConnectionState::Degraded
                    && prev_state != McpConnectionState::Degraded
                {
                    // Transition into Degraded BEFORE dropping the guard.
                    // Earlier this assignment happened after an early return,
                    // so the state field stayed on the previous value and
                    // every subsequent ping kept re-emitting the degraded
                    // event because prev_state never advanced.
                    health.state = McpConnectionState::Degraded;
                    drop(guard);
                    let _ = app.emit(
                        crate::events::EVT_MCP_CONNECTION_DEGRADED,
                        &serde_json::json!({
                            "server_name": name,
                            "latency_ms":  ping.latency_ms,
                        }),
                    );
                    return;
                }

                if prev_state == McpConnectionState::Reconnecting
                    || prev_state == McpConnectionState::Disconnected
                {
                    health.state = McpConnectionState::Connected;
                    drop(guard);
                    let _ = app.emit(
                        crate::events::EVT_MCP_RECONNECTED,
                        &serde_json::json!({
                            "server_name": name,
                        }),
                    );
                    return;
                }

                health.state = new_state;
            }

            Err(e) => {
                let err_str = match &e {
                    PingError::Timeout => "timeout".to_string(),
                    PingError::ConnectionRefused => "connection refused".to_string(),
                    PingError::InvalidResponse(s) => s.clone(),
                };
                debug!("[mcp-health] ping '{}' failed: {}", name, err_str);

                health.last_ping_at = Some(now);
                health.error_message = Some(err_str.clone());
                health.consecutive_failures += 1;

                let threshold = self.failure_threshold;
                let prev_state = health.state.clone();
                let name_owned = name.to_string();
                let endpoint = health.endpoint.clone();
                let delay = self.reconnect_delay_secs;

                if health.consecutive_failures >= threshold
                    && prev_state == McpConnectionState::Connected
                {
                    health.state = McpConnectionState::Disconnected;
                    drop(guard);
                    warn!(
                        "[mcp-health] '{}' disconnected after {} failures",
                        name_owned, threshold
                    );
                    let _ = app.emit(
                        crate::events::EVT_MCP_CONNECTION_LOST,
                        &serde_json::json!({
                            "server_name":           name_owned,
                            "endpoint":              endpoint,
                            "consecutive_failures":  threshold,
                        }),
                    );
                    return;
                }

                if prev_state == McpConnectionState::Disconnected {
                    health.state = McpConnectionState::Reconnecting;
                    drop(guard);
                    // Spawn reconnect attempt in background — don't block the ping loop
                    let self_connections = self.connections.clone();
                    let http_client = self.http_client.clone();
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(Duration::from_secs(delay)).await;
                        Self::attempt_reconnect_inner(
                            &self_connections,
                            &http_client,
                            &name_owned,
                            &endpoint,
                            &app_clone,
                            delay,
                        )
                        .await;
                    });
                    return;
                }

                // Still within failure window — stay in current state
            }
        }
    }

    // ── Auto-reconnect ────────────────────────────────────────────────────────────

    async fn attempt_reconnect(&self, name: &str, endpoint: &str, app: &AppHandle) {
        Self::attempt_reconnect_inner(
            &self.connections,
            &self.http_client,
            name,
            endpoint,
            app,
            self.reconnect_delay_secs,
        )
        .await;
    }

    async fn attempt_reconnect_inner(
        connections: &Arc<RwLock<HashMap<String, McpServerHealth>>>,
        http_client: &reqwest::Client,
        name: &str,
        endpoint: &str,
        app: &AppHandle,
        _reconnect_delay: u64,
    ) {
        // For stdio servers, we can't reconnect automatically
        let is_stdio = {
            let guard = connections.read().await;
            guard.get(name).map_or(false, |h| h.transport == "stdio")
        };
        if is_stdio {
            let _ = app.emit(
                crate::events::EVT_MCP_REQUIRES_RESTART,
                &serde_json::json!({
                    "server_name": name,
                }),
            );
            return;
        }

        // HTTP: attempt a single ping
        let url = if endpoint.ends_with('/') {
            format!("{}health", endpoint)
        } else {
            format!("{}/health", endpoint)
        };

        let start = Instant::now();
        let ok = match http_client.get(&url).send().await {
            Ok(resp) => resp.status().is_success() || resp.status().as_u16() == 404,
            Err(_) => false,
        };

        if ok {
            let latency = start.elapsed().as_millis() as u64;
            let mut guard = connections.write().await;
            if let Some(h) = guard.get_mut(name) {
                h.state = McpConnectionState::Connected;
                h.consecutive_failures = 0;
                h.error_message = None;
                h.avg_latency_ms = latency as f64;
                h.last_success_at = Some(chrono::Utc::now().to_rfc3339());
            }
            drop(guard);
            let _ = app.emit(
                crate::events::EVT_MCP_RECONNECTED,
                &serde_json::json!({
                    "server_name": name,
                }),
            );
        } else {
            let _ = app.emit(
                crate::events::EVT_MCP_RECONNECT_FAILED,
                &serde_json::json!({
                    "server_name": name,
                    "attempt":     1,
                }),
            );
        }
    }
}
