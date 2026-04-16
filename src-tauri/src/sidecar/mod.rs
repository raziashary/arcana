// sidecar/mod.rs — Fleet sidecar lifecycle management.
//
// Architecture: AutoDev fleet workers run in two modes (DEVELOPMENT_PLAN.md §4.1):
//   Mode 1: PA Agent         → Rust AgentEngine (direct LLM, low latency)
//   Mode 2: Worker Agent     → core-orchestrator LivingAgent (21 FSMs, mem0, A2A)
//
// Two Python sidecars must both run for Mode 2 to be active:
//   1. memory-mcp-server   (port 8000)  — long-term mem0 memory
//   2. core-orchestrator   (port 4000)  — LivingAgent gateway
//
// Startup order: memory-mcp-server MUST start first because LivingAgent's
// MemoryCore initialises at agent creation time and connects to port 8000.
//
// Log routing: stdout/stderr from both processes is piped to:
//   ~/.autodev/logs/core-orchestrator.log
//   ~/.autodev/logs/memory-mcp.log
// These files are readable by the AutoDev Logs tab and are also emitted
// through the Rust tracing subscriber (visible in the main app log).

pub mod client;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use tracing::{error, info, warn};

use crate::errors::{AutoDevError, Result};

// ─── SidecarStatus ────────────────────────────────────────────────────────────

/// Runtime status of a Python sidecar process.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarStatus {
    Stopped,
    Starting,
    Running,
    Error(String),
}

impl Default for SidecarStatus {
    fn default() -> Self {
        Self::Stopped
    }
}

impl std::fmt::Display for SidecarStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SidecarStatus::Stopped    => write!(f, "stopped"),
            SidecarStatus::Starting   => write!(f, "starting"),
            SidecarStatus::Running    => write!(f, "running"),
            SidecarStatus::Error(e)   => write!(f, "error: {e}"),
        }
    }
}

// ─── Log Routing ──────────────────────────────────────────────────────────────

/// Returns the ~/.autodev/logs/ directory, creating it if needed.
fn logs_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    let dir = home.join(".autodev").join("logs");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Spawns a background thread that reads lines from `reader`, writes each line
/// to `log_file` (appended), and also emits them via the tracing subscriber so
/// they appear in the AutoDev Logs tab alongside Rust log lines.
fn spawn_log_router<R: std::io::Read + Send + 'static>(
    reader: R,
    log_file: PathBuf,
    prefix: &'static str,
) {
    std::thread::spawn(move || {
        let mut file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            Ok(f) => f,
            Err(e) => {
                warn!("[log_router] Cannot open {:?}: {}", log_file, e);
                return;
            }
        };
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    info!("[{prefix}] {l}");
                    let _ = writeln!(file, "{l}");
                }
                Err(_) => break,
            }
        }
    });
}

// ─── SidecarManager ───────────────────────────────────────────────────────────

/// Manages both Python sidecar processes for the AutoDev fleet:
///
/// ┌──────────────────────────────────────────────────────────────┐
/// │  AutoDev (Rust + Tauri)                                      │
/// │    └── SidecarManager                                        │
/// │          ├── memory-mcp-server  :8000  (start first)        │
/// │          │     └── stdout/stderr → ~/.autodev/logs/          │
/// │          └── core-orchestrator  :4000  (start after memory)  │
/// │                └── stdout/stderr → ~/.autodev/logs/          │
/// └──────────────────────────────────────────────────────────────┘
///
/// See DEVELOPMENT_PLAN.md Phase 2.6 for full connection diagram.
pub struct SidecarManager {
    // core-orchestrator (port 4000)
    orch_process:   Mutex<Option<Child>>,
    orch_status:    Mutex<SidecarStatus>,
    // memory-mcp-server (port 8000) — must be running before core-orchestrator
    memory_process: Mutex<Option<Child>>,
    memory_status:  Mutex<SidecarStatus>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            orch_process:   Mutex::new(None),
            orch_status:    Mutex::new(SidecarStatus::Stopped),
            memory_process: Mutex::new(None),
            memory_status:  Mutex::new(SidecarStatus::Stopped),
        }
    }

    // ─── memory-mcp-server lifecycle ───────────────────────────────────────────

    /// Spawn memory-mcp-server from `cwd`.
    /// Command: `uv run python main.py --mode dev`
    /// Must be called (and healthy) before `start()` for core-orchestrator.
    pub fn start_memory(&self, cwd: &PathBuf) -> Result<()> {
        {
            let s = self.memory_status.lock().unwrap();
            if matches!(*s, SidecarStatus::Running | SidecarStatus::Starting) {
                return Ok(());
            }
        }
        *self.memory_status.lock().unwrap() = SidecarStatus::Starting;
        info!("[SidecarManager] Spawning memory-mcp-server from: {}", cwd.display());

        let mut child = Command::new("uv")
            .args(["run", "python", "main.py", "--mode", "dev"])
            .current_dir(cwd)
            .env("PYTHONIOENCODING", "utf-8:replace")
            .env("PYTHONUTF8", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AutoDevError::Agent(
                format!("Failed to spawn memory-mcp-server (is `uv` in PATH?): {e}")
            ))?;

        let log_dir = logs_dir();
        if let Some(stdout) = child.stdout.take() {
            spawn_log_router(stdout, log_dir.join("memory-mcp.log"), "memory-mcp");
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_router(stderr, log_dir.join("memory-mcp.log"), "memory-mcp ERR");
        }

        *self.memory_process.lock().unwrap() = Some(child);
        info!("[SidecarManager] memory-mcp-server spawned; awaiting health check on :8000");
        Ok(())
    }

    /// Terminate memory-mcp-server.
    pub fn stop_memory(&self) {
        if let Some(mut child) = self.memory_process.lock().unwrap().take() {
            let _ = child.kill();
            info!("[SidecarManager] memory-mcp-server terminated");
        }
        *self.memory_status.lock().unwrap() = SidecarStatus::Stopped;
    }

    /// Mark memory-mcp-server as healthy (call after /health probe passes).
    pub fn mark_memory_running(&self) {
        *self.memory_status.lock().unwrap() = SidecarStatus::Running;
        info!("[SidecarManager] memory-mcp-server is ready on :8000");
    }

    /// Mark memory-mcp-server as errored.
    pub fn mark_memory_error(&self, msg: &str) {
        warn!("[SidecarManager] memory-mcp-server error: {msg}");
        *self.memory_status.lock().unwrap() = SidecarStatus::Error(msg.to_string());
    }

    pub fn memory_status(&self) -> SidecarStatus {
        self.memory_status.lock().unwrap().clone()
    }

    pub fn memory_is_running(&self) -> bool {
        matches!(self.memory_status.lock().unwrap().clone(), SidecarStatus::Running)
    }

    // ─── core-orchestrator lifecycle ───────────────────────────────────────────

    /// Spawn core-orchestrator from `cwd`.
    /// Command: `uv run uvicorn api_gateway.api.main:app --host 127.0.0.1 --port 4000`
    /// Call this after memory-mcp-server health check passes.
    pub fn start(&self, cwd: &PathBuf) -> Result<()> {
        {
            let s = self.orch_status.lock().unwrap();
            if matches!(*s, SidecarStatus::Running | SidecarStatus::Starting) {
                return Ok(());
            }
        }
        *self.orch_status.lock().unwrap() = SidecarStatus::Starting;
        info!("[SidecarManager] Spawning core-orchestrator from: {}", cwd.display());

        let mut child = Command::new("uv")
            .args([
                "run", "uvicorn", "api_gateway.api.main:app",
                "--host", "127.0.0.1",
                "--port", "4000",
                "--log-level", "info",
            ])
            .current_dir(cwd)
            .env("PYTHONIOENCODING", "utf-8:replace")
            .env("PYTHONUTF8", "1")
            .env("LITELLM_LOG", "WARNING")  // T7.3 — suppress LiteLLM debug noise
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AutoDevError::Agent(
                format!("Failed to spawn core-orchestrator (is `uv` in PATH?): {e}")
            ))?;

        let log_dir = logs_dir();
        if let Some(stdout) = child.stdout.take() {
            spawn_log_router(stdout, log_dir.join("core-orchestrator.log"), "core-orch");
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_router(stderr, log_dir.join("core-orchestrator.log"), "core-orch ERR");
        }

        *self.orch_process.lock().unwrap() = Some(child);
        info!("[SidecarManager] core-orchestrator spawned; awaiting health check on :4000");
        Ok(())
    }

    /// Terminate core-orchestrator.
    pub fn stop(&self) {
        if let Some(mut child) = self.orch_process.lock().unwrap().take() {
            let _ = child.kill();
            info!("[SidecarManager] core-orchestrator terminated");
        }
        *self.orch_status.lock().unwrap() = SidecarStatus::Stopped;
    }

    /// Mark core-orchestrator as healthy (call after /api/system/health passes).
    pub fn mark_running(&self) {
        *self.orch_status.lock().unwrap() = SidecarStatus::Running;
        info!("[SidecarManager] core-orchestrator is ready and accepting requests on :4000");
    }

    /// Mark core-orchestrator as errored.
    pub fn mark_error(&self, msg: &str) {
        warn!("[SidecarManager] core-orchestrator error: {msg}");
        *self.orch_status.lock().unwrap() = SidecarStatus::Error(msg.to_string());
    }

    // ─── Combined accessors ────────────────────────────────────────────────────

    /// Status of core-orchestrator (the primary gateway the UI cares about).
    pub fn status(&self) -> SidecarStatus {
        self.orch_status.lock().unwrap().clone()
    }

    /// True only when core-orchestrator is Running.
    pub fn is_running(&self) -> bool {
        matches!(self.orch_status.lock().unwrap().clone(), SidecarStatus::Running)
    }

    /// True only when BOTH memory-mcp-server AND core-orchestrator are Running.
    pub fn fleet_is_ready(&self) -> bool {
        self.memory_is_running() && self.is_running()
    }

    /// Terminate both sidecars (call on app shutdown).
    pub fn stop_all(&self) {
        self.stop();
        self.stop_memory();
    }
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Path Resolution ──────────────────────────────────────────────────────────

/// Resolve the core-orchestrator working directory from:
///   1. Explicit path string
///   2. AUTODEV_SIDECAR_PATH environment variable
///   3. Relative candidates from the current working directory
pub fn resolve_sidecar_path(explicit: Option<&str>) -> std::result::Result<PathBuf, String> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(format!("Sidecar path does not exist: {}", pb.display()));
    }

    if let Ok(p) = std::env::var("AUTODEV_SIDECAR_PATH") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return Ok(pb);
        }
        warn!("[SidecarManager] AUTODEV_SIDECAR_PATH={p} does not exist");
    }

    for rel in &[
        "submodules/core-orchestrator",
        "../submodules/core-orchestrator",
        "../../submodules/core-orchestrator",
        "../../../submodules/core-orchestrator",
    ] {
        let pb = PathBuf::from(rel);
        if pb.exists() {
            return Ok(pb.canonicalize().unwrap_or(pb));
        }
    }

    error!("[SidecarManager] core-orchestrator path not found; set AUTODEV_SIDECAR_PATH");
    Err("core-orchestrator path not found. Set AUTODEV_SIDECAR_PATH env var or pass an explicit path.".into())
}

/// Resolve the memory-mcp-server working directory from:
///   1. Explicit path string
///   2. AUTODEV_MEMORY_PATH environment variable
///   3. Relative candidates from the current working directory
pub fn resolve_memory_path(explicit: Option<&str>) -> std::result::Result<PathBuf, String> {
    if let Some(p) = explicit {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(format!("Memory MCP path does not exist: {}", pb.display()));
    }

    if let Ok(p) = std::env::var("AUTODEV_MEMORY_PATH") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return Ok(pb);
        }
        warn!("[SidecarManager] AUTODEV_MEMORY_PATH={p} does not exist");
    }

    for rel in &[
        "submodules/memory-mcp-server",
        "../submodules/memory-mcp-server",
        "../../submodules/memory-mcp-server",
        "../../../submodules/memory-mcp-server",
    ] {
        let pb = PathBuf::from(rel);
        if pb.exists() {
            return Ok(pb.canonicalize().unwrap_or(pb));
        }
    }

    error!("[SidecarManager] memory-mcp-server path not found; set AUTODEV_MEMORY_PATH");
    Err("memory-mcp-server path not found. Set AUTODEV_MEMORY_PATH env var or pass an explicit path.".into())
}
