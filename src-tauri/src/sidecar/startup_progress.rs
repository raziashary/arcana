// sidecar/startup_progress.rs — Startup phase state machine + log-pattern scanner.
//
// Purpose
// -------
// The AutoDev fleet takes ~15–20 seconds to fully boot (memory-mcp-server,
// core-orchestrator, A2A servers, agent registry). Without visible feedback
// the app appears frozen. This module drives a forward-only state machine
// whose transitions are emitted as Tauri events (`startup:progress`) so the
// frontend can render a multi-step loading overlay that mirrors what the
// backend is actually doing.
//
// Two event sources drive the machine:
//   1. Direct emit from main.rs / SidecarManager at deterministic boundaries
//      (app init, sidecar spawn, health OK, fleet ready).
//   2. Log-line scanning on sidecar stdout/stderr — fills in the intermediate
//      phases ("vector_db", "routers", "agents", "living_agents") whose
//      completion is only observable as a log message from Python.
//
// Transitions are monotonic: `advance()` ignores any phase that would move
// the state machine backwards. Re-emitting the current phase is also a no-op,
// which keeps repeated log matches cheap.
//
// See: autodev/docs/development/startup_sequence_and_agent_persistence_analysis.md

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info};
use autodev_core::state::lock::LockTolerant;

/// Tauri event name the frontend listens for.
pub const EVT_STARTUP_PROGRESS: &str = "startup:progress";

/// Ordered startup phases. Integer ordering is the state machine's total order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupPhase {
    /// App process started, logging + vault + state ready.
    Init = 0,
    /// Built-in skills loaded from disk.
    Skills = 1,
    /// `uv run python main.py` for memory-mcp-server has been spawned.
    MemoryStarting = 2,
    /// memory-mcp-server is loading mem0 / Qdrant.
    VectorDb = 3,
    /// memory-mcp-server `/health` returned 200.
    MemoryReady = 4,
    /// `uv run uvicorn api_gateway.api.main:app` has been spawned.
    OrchStarting = 5,
    /// core-orchestrator FastAPI routers are being registered.
    Routers = 6,
    /// core-orchestrator is loading agent YAML configs from `config/agents/`.
    Agents = 7,
    /// LivingAgent instances are being created with memory cores wired up.
    LivingAgents = 8,
    /// core-orchestrator `/api/system/health` returned 200.
    OrchReady = 9,
    /// RecipeScheduler has started the worker fleet.
    Workers = 10,
    /// All 7 fleet agents verified registered in core-orchestrator.
    FleetReady = 11,
    /// Both sidecars healthy, fleet warm — app is usable.
    Ready = 12,
}

impl StartupPhase {
    /// Short identifier shipped to the frontend. Must be kept in sync with the
    /// `id` values in `src/stores/startupStore.ts`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Skills => "skills",
            Self::MemoryStarting => "memory_starting",
            Self::VectorDb => "vector_db",
            Self::MemoryReady => "memory_ready",
            Self::OrchStarting => "orch_starting",
            Self::Routers => "routers",
            Self::Agents => "agents",
            Self::LivingAgents => "living_agents",
            Self::OrchReady => "orch_ready",
            Self::Workers => "workers",
            Self::FleetReady => "fleet_ready",
            Self::Ready => "ready",
        }
    }

    /// Human-readable message shown alongside the spinner.
    pub fn message(self) -> &'static str {
        match self {
            Self::Init => "Initializing AutoDev…",
            Self::Skills => "Loading skills…",
            Self::MemoryStarting => "Starting Memory MCP Server…",
            Self::VectorDb => "Connecting to vector database…",
            Self::MemoryReady => "Memory MCP Server ready",
            Self::OrchStarting => "Starting Core Orchestrator…",
            Self::Routers => "Loading API routers…",
            Self::Agents => "Initializing agent registry…",
            Self::LivingAgents => "Creating LivingAgents…",
            Self::OrchReady => "Core Orchestrator ready",
            Self::Workers => "Starting worker fleet…",
            Self::FleetReady => "Verifying fleet agents…",
            Self::Ready => "AutoDev is ready",
        }
    }
}

// ── Payload ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct StartupProgressPayload {
    pub phase: &'static str,
    pub index: u8,
    pub total: u8,
    pub message: &'static str,
}

// ── State machine ────────────────────────────────────────────────────────────

/// Monotonic startup phase tracker with event emission.
pub struct StartupProgress {
    app: AppHandle,
    current: Mutex<StartupPhase>,
}

impl StartupProgress {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            current: Mutex::new(StartupPhase::Init),
        }
    }

    /// Total number of phases (used by the UI for progress bar denominator).
    pub const fn total() -> u8 {
        (StartupPhase::Ready as u8) + 1
    }

    /// Advance the state machine to `target` if it would move strictly forward.
    /// Idempotent: calling with a ≤ current phase is a no-op.
    pub fn advance(&self, target: StartupPhase) {
        let mut cur = self.current.lock_tolerant();
        if target <= *cur && target != StartupPhase::Init {
            // Allow Init to fire once as the initial event even though it's
            // the stored default.
            debug!(
                "[StartupProgress] skip advance {:?} → already at {:?}",
                target, *cur
            );
            return;
        }
        *cur = target;
        drop(cur);

        let payload = StartupProgressPayload {
            phase: target.id(),
            index: target as u8,
            total: Self::total(),
            message: target.message(),
        };
        info!(
            "[StartupProgress] {} ({}/{}) — {}",
            payload.phase, payload.index, payload.total, payload.message
        );
        let _ = self.app.emit(EVT_STARTUP_PROGRESS, payload);
    }

    /// Current phase (mostly useful for tests and debugging).
    pub fn current(&self) -> StartupPhase {
        *self.current.lock_tolerant()
    }

    /// Snapshot of the current phase as a frontend-ready payload.
    ///
    /// Tauri's event bus does NOT replay events emitted before a webview's
    /// `listen()` handler is registered. The setup hook fires `Init` and
    /// `Skills` very early — likely before the React tree has even mounted —
    /// so those events are silently dropped.
    ///
    /// To recover, the frontend calls `get_startup_phase` on mount (right
    /// after subscribing). Whatever is returned here represents the latest
    /// phase the FSM has reached, and the React store advances to it
    /// monotonically. From then on, live event delivery takes over.
    pub fn snapshot(&self) -> StartupProgressPayload {
        let phase = self.current();
        StartupProgressPayload {
            phase: phase.id(),
            index: phase as u8,
            total: Self::total(),
            message: phase.message(),
        }
    }

    /// Inspect a single log line from a sidecar and advance the phase if the
    /// line matches a known milestone. Called once per line by the log router.
    ///
    /// Patterns are substring checks for robustness against Python log format
    /// changes. Order does not matter — `advance()` enforces monotonicity.
    pub fn scan_line(&self, prefix: &str, line: &str) {
        match prefix {
            "memory-mcp" | "memory-mcp ERR" => {
                // memory-mcp-server → Mem0 + Qdrant initialisation indicators.
                if line.contains("Using Memory(") || line.contains("Qdrant") {
                    self.advance(StartupPhase::VectorDb);
                }
            }
            "core-orch" | "core-orch ERR" => {
                // core-orchestrator → routers → agents → LivingAgents boot order.
                if line.contains("routers") && line.contains("Registered") {
                    self.advance(StartupPhase::Routers);
                } else if line.contains("Loading agent config from") {
                    self.advance(StartupPhase::Agents);
                } else if line.contains("Created instance for non-system agent")
                    || line.contains("born with memory consciousness")
                    || line.contains("LivingAgent")
                {
                    self.advance(StartupPhase::LivingAgents);
                }
            }
            _ => {}
        }
    }
}
