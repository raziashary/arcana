// sidecar/mod.rs — Fleet sidecar lifecycle management.
//
// Architecture: AutoDev fleet workers run in two modes (DEVELOPMENT_PLAN.md §4.1):
//   Mode 1: PA Agent         → Rust AgentEngine (direct LLM, low latency)
//   Mode 2: Worker Agent     → core-orchestrator LivingAgent (21 FSMs, mem0, A2A)
//
// Three managed processes must all run for Mode 2 to be active:
//   1. memory-mcp-server   (port 8000)  — long-term mem0 memory
//   2. core-orchestrator   (port 4000)  — LivingAgent gateway
//   3. autodev-mcp-server  (port 8767)  — canonical Rust builtin_tools via MCP
//
// Startup order: memory-mcp-server MUST start first because LivingAgent's
// MemoryCore initialises at agent creation time and connects to port 8000.
// autodev-mcp-server is an in-process Tokio task (no separate binary), so it
// starts immediately alongside the Tauri app.
//
// Log routing: stdout/stderr from both processes is piped to:
//   ~/.autodev/logs/core-orchestrator.log
//   ~/.autodev/logs/memory-mcp.log
// These files are readable by the AutoDev Logs tab and are also emitted
// through the Rust tracing subscriber (visible in the main app log).

pub mod client;
pub mod startup_progress;

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::errors::{AutoDevError, Result};
use startup_progress::{StartupPhase, StartupProgress};

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
//
// Phase 7.3 / 7.4 — sidecar log size limits.
//
// The 2026-04-29 run produced 133 MB (memory-mcp.log) + 4.8 MB (core-orch.log)
// in 24 minutes.  Root causes:
//   • mem0 logs the full 1000+ char agent system prompt on every memory store
//   • Redis connection errors emit a 12-line stack trace on every agent init
//   • No cap or rotation existed — logs grew without bound
//
// Fix strategy:
//   1. Rotate the sidecar log at process startup if it already exceeds 20 MB
//      (safe: file is not yet open, no Windows lock contention).
//   2. Apply a per-session line cap (50 000 lines) — if a tight loop or
//      runaway verbosity hits the cap, further writes are suppressed with a
//      single notice line.  Errors still flow through tracing even after cap.
//   3. Skip extremely long lines (> 2000 chars) — these are serialised
//      system-prompt or embedding-vector blobs that add bulk with no value.
//   4. Route Python log lines to the Rust tracing system at appropriate levels:
//        Python ERROR / Traceback / Exception → Rust error!   (→ main Rust log)
//        Python WARNING                        → Rust warn!    (→ main Rust log)
//        Everything else (INFO / DEBUG)        → Rust debug!   (NOT in main log)
//      This keeps the main autodev.YYYY-MM-DD log small — Python INFO chatter
//      goes only to the sidecar-specific file where it belongs.

/// Maximum size of a sidecar log file before it is rotated on next startup.
const MAX_SIDECAR_LOG_BYTES: u64  = 20 * 1_024 * 1_024; // 20 MB
/// Maximum lines written per session; further writes suppressed (errors still flow).
const MAX_SIDECAR_LOG_LINES: usize = 50_000;
/// Maximum character length of a single line; longer lines are dropped.
const MAX_SIDECAR_LINE_LEN:  usize = 2_000;

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

/// Phase 7.3 — rotate a sidecar log file if it exceeds MAX_SIDECAR_LOG_BYTES.
///
/// Must be called BEFORE opening the file for writing.  The rename is safe on
/// Windows (file is closed) and is a no-op when the file does not exist or is
/// below the threshold.
///
/// Rotation scheme: one numbered backup is kept.
///   memory-mcp.log  →  memory-mcp.log.1  (previous run, then overwritten)
///
/// If two log-router threads race on the same file (stdout + stderr share a
/// path), the second rename attempt just fails silently — the winner already
/// moved the file and the loser opens a fresh one.
fn rotate_sidecar_log_if_needed(log_file: &PathBuf) {
    if !log_file.exists() { return; }
    let meta = match fs::metadata(log_file) {
        Ok(m)  => m,
        Err(_) => return,
    };
    if meta.len() < MAX_SIDECAR_LOG_BYTES { return; }

    // Build rotated path: append ".1" to the full filename  (memory-mcp.log → memory-mcp.log.1)
    let mut rotated_os = log_file.as_os_str().to_owned();
    rotated_os.push(".1");
    let rotated = PathBuf::from(rotated_os);

    let _ = fs::remove_file(&rotated); // drop previous backup if present
    match fs::rename(log_file, &rotated) {
        Ok(_)  => info!(
            "[log_router] Rotated '{}' ({:.1} MB) — fresh file will be created",
            log_file.file_name().unwrap_or_default().to_string_lossy(),
            meta.len() as f64 / (1_024.0 * 1_024.0)
        ),
        Err(_) => {
            // Another thread already rotated it, or the file was deleted — fine.
        }
    }
}

/// Spawns a background thread that reads lines from `reader`, writes each line
/// to `log_file` (appended), and routes genuine errors/warnings to the Rust
/// tracing subscriber.
///
/// Phase 7.3/7.4 improvements vs the original:
///   • Rotates `log_file` at open time if it already exceeds 20 MB.
///   • Drops lines longer than 2 000 chars (serialised system-prompt blobs).
///   • Enforces a 50 000-line cap per session; a notice is written then writes stop.
///   • Python INFO/DEBUG  → Rust debug! only (NOT in main autodev log at info filter).
///   • Python WARNING     → Rust warn!  (visible in main log).
///   • Python ERROR/Trace → Rust error! (visible in main log).
///   This keeps the main autodev.YYYY-MM-DD log small; full detail stays in the
///   sidecar-specific file (core-orchestrator.log / memory-mcp.log).
///
/// If `progress` is provided each line is fed through the startup-phase scanner
/// so the loading UI advances as Python sidecars hit their milestones.
fn spawn_log_router<R: std::io::Read + Send + 'static>(
    reader: R,
    log_file: PathBuf,
    prefix: &'static str,
    progress: Option<Arc<StartupProgress>>,
) {
    std::thread::spawn(move || {
        // Phase 7.3 — rotate on open if file is already over the size limit.
        // File is not yet held open here so the rename is safe on Windows.
        rotate_sidecar_log_if_needed(&log_file);

        let mut file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            Ok(f)  => f,
            Err(e) => {
                warn!("[log_router] Cannot open {:?}: {}", log_file, e);
                return;
            }
        };

        let reader = BufReader::new(reader);
        let mut lines_written: usize = 0;
        let mut cap_reached = false;

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    // Phase 7.4 — skip serialised system-prompt / embedding blobs.
                    // mem0 logs the full agent system prompt (~1 000+ chars) on every
                    // memory store operation; dropping these lines reduces memory-mcp.log
                    // by ~85 % with no loss of useful diagnostic information.
                    if l.len() > MAX_SIDECAR_LINE_LEN {
                        continue;
                    }

                    // Always feed the startup-progress scanner so the loading overlay
                    // advances correctly — even after the line-cap is hit.
                    if let Some(p) = progress.as_ref() {
                        p.scan_line(prefix, &l);
                    }

                    // Phase 7.4 — per-session line cap.
                    // If a tight loop or runaway verbosity hits the limit, further
                    // writes to the file are suppressed.  Genuine errors still flow
                    // to the Rust tracing subscriber even after the cap so nothing
                    // critical is silently dropped.
                    lines_written += 1;
                    if lines_written > MAX_SIDECAR_LOG_LINES {
                        if !cap_reached {
                            let _ = writeln!(
                                file,
                                "[log-router] Session line cap ({MAX_SIDECAR_LOG_LINES}) \
                                 reached for '{prefix}' — suppressing further file output \
                                 (ERROR/WARNING lines still emitted via Rust tracing)"
                            );
                            cap_reached = true;
                        }
                        // After cap: only route genuine errors to the Rust log.
                        if classify_python_log_level(&l) == "error" {
                            error!("[{prefix}] {l}");
                        }
                        continue;
                    }

                    // Phase 3.0 R4 — Windows asyncio teardown chatter.
                    // Benign keep-alive reap messages are downgraded to debug regardless
                    // of Python log level so they don't pollute the Rust error log.
                    if is_windows_asyncio_noise(&l) {
                        debug!("[{prefix}] {l}");
                        let _ = writeln!(file, "{l}");
                        continue;
                    }

                    // Phase 7.3/7.4 — level-aware routing to Rust tracing.
                    //
                    // Previous behaviour: every sidecar line was emitted as info!()
                    // which caused ALL Python stdout to appear in the main autodev Rust
                    // log, inflating it to 142 MB in 24 minutes.
                    //
                    // New behaviour:
                    //   Python ERROR / Traceback / Exception → Rust error!
                    //       → appears in main autodev log (important to see)
                    //   Python WARNING                        → Rust warn!
                    //       → appears in main autodev log
                    //   Python INFO / DEBUG / everything else → Rust debug!
                    //       → NOT written to main autodev log at default info filter
                    //       → still written to the sidecar-specific log file below
                    //
                    // This keeps the main autodev.YYYY-MM-DD log to Rust-only events
                    // plus Python errors — a fraction of the previous volume.
                    match classify_python_log_level(&l) {
                        "error" => error!("[{prefix}] {l}"),
                        "warn"  => warn!("[{prefix}] {l}"),
                        "info"  => info!("[{prefix}] {l}"),
                        _       => debug!("[{prefix}] {l}"),
                    }

                    // Always write every (non-oversized, non-capped) line to the
                    // sidecar-specific log file so full detail is available for
                    // post-run forensics via the Logs tab or direct file inspection.
                    let _ = writeln!(file, "{l}");
                }
                Err(_) => break,
            }
        }
    });
}

/// Phase 3.0 R4 — predicate for the Windows asyncio keep-alive teardown log
/// pattern. Matched on both stdout and stderr routers. Kept as a free
/// function so unit tests can exercise it without spawning a subprocess.
fn is_windows_asyncio_noise(line: &str) -> bool {
    // The two anchor substrings are distinctive enough that substring match
    // is sufficient — no regex dependency needed.
    line.contains("ConnectionResetError") && line.contains("WinError 10054")
        || line.contains("_ProactorBasePipeTransport._call_connection_lost")
}

/// J.1 — Map a Python sidecar log line to the Rust tracing level it should
/// be emitted at. Known transient networking / reconnect patterns are
/// downgraded from ERROR to DEBUG so they don't drown genuine failures in
/// the main autodev log. Returns a static string matched in the call site.
fn classify_python_log_level(line: &str) -> &'static str {
    // Transient Windows asyncio / trio networking noise — not real errors.
    if line.contains("ClosedResourceError")
        || line.contains("WinError 10054")
        || line.contains("ConnectionResetError")
        || line.contains("_call_connection_lost")
        || line.contains("send_nowait")
    {
        return "debug";
    }
    // Memory MCP reconnect success — informational, not an error.
    if line.contains("Memory MCP reconnected successfully") {
        return "info";
    }
    // Standard Python log level markers.
    if line.contains(" ERROR ") || line.contains("Traceback")
        || line.contains("Exception") || line.contains(" CRITICAL ")
    {
        return "error";
    }
    if line.contains(" WARNING ") || line.contains(" WARN ") {
        return "warn";
    }
    "debug"
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
    // autodev-mcp-server (port 8767) — in-process Tokio task, tracked for fleet readiness
    mcp_status:     Mutex<SidecarStatus>,
    // Redis / Memurai (port 6379) — externally managed (Windows service or
    // system Redis). AutoDev only probes liveness; it does NOT spawn or stop
    // Redis. Redis is OPTIONAL — its absence disables the warm memory tier
    // and agent card store but does not block fleet startup.
    redis_status:   Mutex<SidecarStatus>,
    // Startup phase tracker — set once the Tauri AppHandle is available
    // (see main.rs setup hook). Log router + status transitions feed this.
    progress:       Mutex<Option<Arc<StartupProgress>>>,
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            orch_process:   Mutex::new(None),
            orch_status:    Mutex::new(SidecarStatus::Stopped),
            memory_process: Mutex::new(None),
            memory_status:  Mutex::new(SidecarStatus::Stopped),
            mcp_status:     Mutex::new(SidecarStatus::Running), // Phase 3.4 — MCP is in-process, starts immediately
            redis_status:   Mutex::new(SidecarStatus::Stopped), // Phase 3.4b — polled, not spawned
            progress:       Mutex::new(None),
        }
    }

    /// Attach the startup-progress tracker. Called once from main.rs after the
    /// Tauri AppHandle is available. Safe to call multiple times — replaces the
    /// previous tracker.
    pub fn set_progress(&self, progress: Arc<StartupProgress>) {
        *self.progress.lock().unwrap() = Some(progress);
    }

    fn progress(&self) -> Option<Arc<StartupProgress>> {
        self.progress.lock().unwrap().clone()
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
        if let Some(p) = self.progress() {
            p.advance(StartupPhase::MemoryStarting);
        }

        let mut child = Command::new("uv")
            .args(["run", "python", "main.py", "--mode", "dev"])
            .current_dir(cwd)
            .env("PYTHONIOENCODING", "utf-8:replace")
            .env("PYTHONUTF8", "1")
            // Phase 7.4 — suppress mem0 INFO/DEBUG chatter.
            // mem0 logs the full agent system prompt (~1 000 chars) at INFO level on
            // every memory store, producing 133 MB of logs in 24 minutes.
            // Setting LOG_LEVEL=WARNING drops INFO/DEBUG from the root Python logger.
            // Side-effect: the startup-progress scanner will not see the "Using Memory("
            // / "Qdrant" INFO lines and therefore won't advance the VectorDb phase — the
            // loading overlay skips that cosmetic step but startup still completes via
            // the health-check poll in main.rs.
            .env("LOG_LEVEL",     "WARNING")
            .env("MEM0_LOG_LEVEL","WARNING")   // mem0-specific override if read by the app
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AutoDevError::Agent(
                format!("Failed to spawn memory-mcp-server (is `uv` in PATH?): {e}")
            ))?;

        let log_dir = logs_dir();
        let progress = self.progress();
        if let Some(stdout) = child.stdout.take() {
            spawn_log_router(stdout, log_dir.join("memory-mcp.log"), "memory-mcp", progress.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_router(stderr, log_dir.join("memory-mcp.log"), "memory-mcp ERR", progress);
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
        if let Some(p) = self.progress() {
            p.advance(StartupPhase::MemoryReady);
        }
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

    /// Returns the OS PID of memory-mcp-server child process, if alive.
    pub fn memory_pid(&self) -> Option<u32> {
        self.memory_process.lock().unwrap().as_ref().map(|c| c.id())
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
        if let Some(p) = self.progress() {
            p.advance(StartupPhase::OrchStarting);
        }

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
        let progress = self.progress();
        if let Some(stdout) = child.stdout.take() {
            spawn_log_router(stdout, log_dir.join("core-orchestrator.log"), "core-orch", progress.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_log_router(stderr, log_dir.join("core-orchestrator.log"), "core-orch ERR", progress);
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
        if let Some(p) = self.progress() {
            p.advance(StartupPhase::OrchReady);
            // Fleet is ready only when BOTH sidecars are healthy. The Workers
            // transition is squeezed in between OrchReady and Ready so the
            // loading UI shows a final "Starting worker fleet…" step instead
            // of jumping straight to ready.
            if self.fleet_is_ready() {
                p.advance(StartupPhase::Workers);
                p.advance(StartupPhase::Ready);
            }
        }
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

    /// Returns the OS PID of core-orchestrator child process, if alive.
    pub fn orch_pid(&self) -> Option<u32> {
        self.orch_process.lock().unwrap().as_ref().map(|c| c.id())
    }

    /// True only when memory-mcp-server, core-orchestrator, AND autodev-mcp-server are Running.
    pub fn fleet_is_ready(&self) -> bool {
        self.memory_is_running() && self.is_running() && self.mcp_is_running()
    }

    // ─── autodev-mcp-server status ────────────────────────────────────────────

    /// Mark autodev-mcp-server as healthy.
    pub fn mark_mcp_running(&self) {
        *self.mcp_status.lock().unwrap() = SidecarStatus::Running;
        info!("[SidecarManager] autodev-mcp-server is ready on :8767");
    }

    /// Mark autodev-mcp-server as errored.
    pub fn mark_mcp_error(&self, msg: &str) {
        warn!("[SidecarManager] autodev-mcp-server error: {msg}");
        *self.mcp_status.lock().unwrap() = SidecarStatus::Error(msg.to_string());
    }

    pub fn mcp_status(&self) -> SidecarStatus {
        self.mcp_status.lock().unwrap().clone()
    }

    pub fn mcp_is_running(&self) -> bool {
        matches!(self.mcp_status.lock().unwrap().clone(), SidecarStatus::Running)
    }

    // ─── redis / memurai status (Phase 3.4b) ──────────────────────────────────
    //
    // Redis is externally managed (Memurai service on Windows, system redis on
    // Linux/macOS). AutoDev only polls liveness — it never spawns redis.
    // Redis is OPTIONAL: core-orchestrator runs without it (warm tier disabled)
    // so `redis_is_running()` is NOT part of `fleet_is_ready()`.

    /// Mark Redis as healthy (call after PING returns +PONG).
    pub fn mark_redis_running(&self) {
        let mut s = self.redis_status.lock().unwrap();
        if !matches!(*s, SidecarStatus::Running) {
            info!("[SidecarManager] Redis/Memurai is reachable on :6379");
        }
        *s = SidecarStatus::Running;
    }

    /// Mark Redis as unreachable. This is not an error for AutoDev — it just
    /// means the warm memory tier is unavailable and core-orchestrator will
    /// fall back to cold tier only.
    pub fn mark_redis_unavailable(&self) {
        let mut s = self.redis_status.lock().unwrap();
        if !matches!(*s, SidecarStatus::Error(_)) {
            warn!(
                "[SidecarManager] Redis/Memurai not reachable on :6379 — \
                 warm memory tier disabled (core-orchestrator will use cold tier only)"
            );
        }
        *s = SidecarStatus::Error("Redis not reachable on 127.0.0.1:6379".into());
    }

    pub fn redis_status(&self) -> SidecarStatus {
        self.redis_status.lock().unwrap().clone()
    }

    pub fn redis_is_running(&self) -> bool {
        matches!(self.redis_status.lock().unwrap().clone(), SidecarStatus::Running)
    }

    /// Terminate managed sidecars (call on app shutdown).
    /// NOTE: Redis is NOT stopped — it is externally managed by the OS.
    pub fn stop_all(&self) {
        self.stop();
        self.stop_memory();
    }

    // ─── Process-exit detection (for auto-restart watchdog) ───────────────────

    /// Non-blocking check: returns true if the core-orchestrator Child has exited.
    /// Returns false when no process has been started.
    pub fn orch_exited(&self) -> bool {
        if let Some(child) = self.orch_process.lock().unwrap().as_mut() {
            matches!(child.try_wait(), Ok(Some(_)))
        } else {
            false
        }
    }

    /// Non-blocking check: returns true if memory-mcp-server Child has exited.
    /// Returns false when no process has been started.
    pub fn memory_exited(&self) -> bool {
        if let Some(child) = self.memory_process.lock().unwrap().as_mut() {
            matches!(child.try_wait(), Ok(Some(_)))
        } else {
            false
        }
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

// ─── Auto-restart Watchdog (Phase 3.4B) ──────────────────────────────────────
//
// Monitors both sidecar processes and restarts them on failure using
// exponential backoff.  Tolerates transient health-check failures up to
// WATCHDOG_UNHEALTHY_THRESHOLD consecutive misses before triggering a restart.
//
// Backoff sequence: 2s → 4s → 8s → 16s → 32s → 64s → 120s (capped).
// The backoff resets to 2s after a successful recovery.

const WATCHDOG_POLL_SECS:        u64  = 10;
const WATCHDOG_INITIAL_DELAY:    u64  = 90;  // Let startup complete first
const WATCHDOG_MAX_BACKOFF:      u64  = 120;
const WATCHDOG_UNHEALTHY_STRIKES: u32 = 3;   // 3 × 10s = 30s of failure

/// Spawn background watchdog tasks that monitor both sidecars and restart on
/// failure.  Should be called from main.rs after the initial startup sequence
/// completes.  The tasks run for the lifetime of the Tokio runtime.
pub fn start_watchdog(
    sidecar_mgr: Arc<SidecarManager>,
    client:      Arc<client::OrchestratorClient>,
) {
    // Use tauri::async_runtime::spawn instead of tokio::spawn.
    // start_watchdog is called from Tauri's synchronous .setup() callback where
    // no Tokio reactor is running on the current thread yet.  Tauri's spawn
    // stores a handle to the runtime and works correctly from any thread.
    let mem_mgr    = sidecar_mgr.clone();
    let mem_client = client.clone();
    tauri::async_runtime::spawn(async move {
        memory_watchdog(mem_mgr, mem_client).await;
    });

    let orch_mgr    = sidecar_mgr;
    let orch_client = client;
    tauri::async_runtime::spawn(async move {
        orch_watchdog(orch_mgr, orch_client).await;
    });
}

async fn orch_watchdog(
    mgr:    Arc<SidecarManager>,
    client: Arc<client::OrchestratorClient>,
) {
    // Wait for the initial fleet startup to complete before monitoring.
    tokio::time::sleep(Duration::from_secs(WATCHDOG_INITIAL_DELAY)).await;
    info!("[watchdog] core-orchestrator watchdog active (poll every {}s)", WATCHDOG_POLL_SECS);

    let mut backoff_secs  = 2u64;
    let mut unhealthy_run = 0u32;

    loop {
        tokio::time::sleep(Duration::from_secs(WATCHDOG_POLL_SECS)).await;

        // Only watch when the orchestrator was intentionally started.
        if !matches!(mgr.status(), SidecarStatus::Running | SidecarStatus::Starting) {
            unhealthy_run = 0;
            backoff_secs  = 2;
            continue;
        }

        // Two-stage failure detection: process exit OR health-probe failure.
        let exited    = mgr.orch_exited();
        let unhealthy = !client.is_healthy().await;

        if !exited && !unhealthy {
            unhealthy_run = 0;
            backoff_secs  = 2;
            continue;
        }

        unhealthy_run += 1;
        if !exited && unhealthy_run < WATCHDOG_UNHEALTHY_STRIKES {
            // Tolerate transient failures
            debug!("[watchdog] orch unhealthy probe #{unhealthy_run}/{WATCHDOG_UNHEALTHY_STRIKES}");
            continue;
        }

        warn!(
            "[watchdog] core-orchestrator failure after {} misses \
             (exited={exited}, unhealthy={unhealthy}) — restarting in {backoff_secs}s",
            unhealthy_run
        );
        unhealthy_run = 0;
        mgr.stop();
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

        match resolve_sidecar_path(None) {
            Ok(cwd) => {
                if let Err(e) = mgr.start(&cwd) {
                    warn!("[watchdog] orch restart failed: {e}");
                    backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
                    continue;
                }
                let mut recovered = false;
                for attempt in 1..=30u32 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if client.is_healthy().await {
                        mgr.mark_running();
                        client.clear_cache();
                        info!("[watchdog] core-orchestrator recovered after {attempt}s");
                        backoff_secs = 2;
                        recovered    = true;
                        break;
                    }
                }
                if !recovered {
                    warn!("[watchdog] orch recovery timed out — will retry after {}s", backoff_secs * 2);
                    mgr.mark_error("watchdog: restart health-check timed out");
                    backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
                }
            }
            Err(e) => {
                warn!("[watchdog] orch path resolution failed: {e}");
                backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
            }
        }
    }
}

async fn memory_watchdog(
    mgr:    Arc<SidecarManager>,
    client: Arc<client::OrchestratorClient>,
) {
    tokio::time::sleep(Duration::from_secs(WATCHDOG_INITIAL_DELAY)).await;
    info!("[watchdog] memory-mcp-server watchdog active (poll every {}s)", WATCHDOG_POLL_SECS);

    let mut backoff_secs  = 2u64;
    let mut unhealthy_run = 0u32;

    loop {
        tokio::time::sleep(Duration::from_secs(WATCHDOG_POLL_SECS)).await;

        if !matches!(mgr.memory_status(), SidecarStatus::Running | SidecarStatus::Starting) {
            unhealthy_run = 0;
            backoff_secs  = 2;
            continue;
        }

        let exited    = mgr.memory_exited();
        let unhealthy = !client.is_memory_healthy().await;

        if !exited && !unhealthy {
            unhealthy_run = 0;
            backoff_secs  = 2;
            continue;
        }

        unhealthy_run += 1;
        if !exited && unhealthy_run < WATCHDOG_UNHEALTHY_STRIKES {
            debug!("[watchdog] memory unhealthy probe #{unhealthy_run}/{WATCHDOG_UNHEALTHY_STRIKES}");
            continue;
        }

        warn!(
            "[watchdog] memory-mcp-server failure after {} misses \
             (exited={exited}, unhealthy={unhealthy}) — restarting in {backoff_secs}s",
            unhealthy_run
        );
        unhealthy_run = 0;
        mgr.stop_memory();
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

        match resolve_memory_path(None) {
            Ok(cwd) => {
                if let Err(e) = mgr.start_memory(&cwd) {
                    warn!("[watchdog] memory restart failed: {e}");
                    backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
                    continue;
                }
                let mut recovered = false;
                for attempt in 1..=20u32 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    if client.is_memory_healthy().await {
                        mgr.mark_memory_running();
                        info!("[watchdog] memory-mcp-server recovered after {attempt}s");
                        backoff_secs = 2;
                        recovered    = true;
                        break;
                    }
                }
                if !recovered {
                    warn!("[watchdog] memory recovery timed out — will retry after {}s", backoff_secs * 2);
                    mgr.mark_memory_error("watchdog: restart health-check timed out");
                    backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
                }
            }
            Err(e) => {
                warn!("[watchdog] memory path resolution failed: {e}");
                backoff_secs = (backoff_secs * 2).min(WATCHDOG_MAX_BACKOFF);
            }
        }
    }
}
