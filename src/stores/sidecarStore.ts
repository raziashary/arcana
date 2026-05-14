// sidecarStore.ts — Zustand store for core-orchestrator sidecar status.
//
// Tracks whether the Python sidecar is reachable and exposes actions to
// start/stop it from the UI. The store polls status on mount and after
// any start/stop action.
//
// Sidecar statuses (string from Rust):
//   "stopped"   — not running, fleet uses Rust AgentEngine (Mode 1)
//   "starting"  — process spawned, awaiting health check
//   "running"   — healthy, fleet uses LivingAgent (Mode 2)
//   "error: …"  — spawn/health failure

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { create } from "zustand";

export type SidecarStatusStr =
  | "stopped"
  | "starting"
  | "running"
  | `error: ${string}`;

// ─── Ruflo Feature 6 — MCP Server Health ─────────────────────────────────────

export type McpConnectionState =
  | 'connected'
  | 'degraded'
  | 'disconnected'
  | 'reconnecting'
  | 'not_monitored'

export interface McpServerHealth {
  server_name:           string
  endpoint:              string
  transport:             string       // "http" | "stdio"
  state:                 McpConnectionState
  last_ping_at:          string | null
  last_success_at:       string | null
  consecutive_failures:  number
  avg_latency_ms:        number
  error_message:         string | null
}

// In-process AutoDev MCP server port binding status. Refreshed via
// get_mcp_port_status on mount and via the mcp:port-conflict event when
// startup falls back to an alternate port.
export interface McpPortStatus {
  preferred_port:   number
  bound_port:       number
  fallback_active:  boolean
  conflict_pid:     number | null
  is_ghost:         boolean
  message:          string
}

interface StartupProgressPayload {
  phase: string;
  message?: string;
}

interface SidecarState {
  status: SidecarStatusStr;
  isConnected: boolean;
  // Ruflo Feature 6 — external MCP server health
  mcpServerHealth:   McpServerHealth[];
  mcpDisconnected:   string[];           // server names currently disconnected
  // In-process AutoDev MCP server port — surfaced as a live indicator in
  // Settings so the user knows when a Windows ghost socket forced a fallback.
  mcpPortStatus:     McpPortStatus | null;

  // Actions
  loadStatus: () => Promise<void>;
  checkConnectivity: () => Promise<boolean>;
  startSidecar: (path?: string) => Promise<string>;
  stopSidecar: () => Promise<void>;
  loadMcpHealth: () => Promise<void>;
  loadMcpPortStatus: () => Promise<void>;
  initEventListeners: () => Promise<void>;
}

// Module-scoped guard to prevent duplicate listener registration (React StrictMode double-mount)
let _listenersInstalled = false;
const _unlistenFns: UnlistenFn[] = [];

export const useSidecarStore = create<SidecarState>((set, get) => ({
  status: "stopped",
  isConnected: false,
  mcpServerHealth:  [],
  mcpDisconnected:  [],
  mcpPortStatus:    null,

  // ── Loaders ──────────────────────────────────────────────────────────────────

  loadStatus: async () => {
    try {
      const status = await invoke<string>("get_sidecar_status");
      const isConnected = status === "running";
      set({ status: status as SidecarStatusStr, isConnected });
    } catch {
      set({ status: "stopped", isConnected: false });
    }
  },

  checkConnectivity: async () => {
    try {
      const healthy = await invoke<boolean>("check_sidecar_connectivity");
      const status: SidecarStatusStr = healthy ? "running" : "stopped";
      set({ status, isConnected: healthy });
      return healthy;
    } catch {
      set({ status: "stopped", isConnected: false });
      return false;
    }
  },

  // ── Lifecycle ─────────────────────────────────────────────────────────────────

  startSidecar: async (path?: string) => {
    set({ status: "starting" });
    try {
      const result = await invoke<string>("start_sidecar", { path });
      const isConnected = result === "running";
      set({ status: result as SidecarStatusStr, isConnected });
      return result;
    } catch (err) {
      const msg = `error: ${String(err)}`;
      set({ status: msg as SidecarStatusStr, isConnected: false });
      throw err;
    }
  },

  stopSidecar: async () => {
    try {
      await invoke<string>("stop_sidecar");
    } finally {
      set({ status: "stopped", isConnected: false });
    }
  },

  // ── Ruflo Feature 6 — MCP health ─────────────────────────────────────────────

  loadMcpHealth: async () => {
    try {
      const health = await invoke<McpServerHealth[]>("get_mcp_health")
      const disconnected = health
        .filter(h => h.state === 'disconnected' || h.state === 'reconnecting')
        .map(h => h.server_name)
      set({ mcpServerHealth: health, mcpDisconnected: disconnected })
    } catch (e) {
      console.error("[sidecarStore] Failed to load MCP health:", e)
    }
  },

  loadMcpPortStatus: async () => {
    try {
      const status = await invoke<McpPortStatus>("get_mcp_port_status")
      set({ mcpPortStatus: status })
    } catch (e) {
      console.error("[sidecarStore] Failed to load MCP port status:", e)
    }
  },

  // ── Event Listeners ──────────────────────────────────────────────────────
  // Task A1: Wire sidecarStore to startup:progress events.
  // Subscribes to Rust startup:progress event and transitions to "running" when
  // orchestrator reaches orch_ready, fleet_ready, or ready phase.
  // Also starts a 5-second polling interval until isConnected === true.
  initEventListeners: async () => {
    if (_listenersInstalled) return;
    _listenersInstalled = true;

    try {
      // Register listener for startup:progress events
      const unlisten = await listen<StartupProgressPayload>(
        "startup:progress",
        (event) => {
          const { phase } = event.payload;
          // When orchestrator reaches these phases, mark sidecar as running
          if (phase === "orch_ready" || phase === "fleet_ready" || phase === "ready") {
            set({ status: "running", isConnected: true });
          }
        }
      );
      _unlistenFns.push(unlisten);

      // Start a 5-second polling interval as fallback
      // (in case startup:progress events are missed or delayed)
      let pollAttempts = 0;
      const maxAttempts = 12; // 5s * 12 = 60s max
      const pollInterval = setInterval(async () => {
        pollAttempts++;
        const healthy = await get().checkConnectivity();
        if (healthy) {
          clearInterval(pollInterval);
        } else if (pollAttempts >= maxAttempts) {
          clearInterval(pollInterval);
        }
      }, 5000);

      // Ruflo Feature 6 — MCP health event subscriptions

      // Full snapshot from each ping cycle — replace entire health list
      const unlistenMcpUpdate = await listen<McpServerHealth[]>('mcp:health-update', e => {
        const disconnected = e.payload
          .filter(h => h.state === 'disconnected' || h.state === 'reconnecting')
          .map(h => h.server_name)
        set({ mcpServerHealth: e.payload, mcpDisconnected: disconnected })
      })
      _unlistenFns.push(unlistenMcpUpdate)

      // Connection lost — add to disconnected list and update that server's entry
      const unlistenLost = await listen<{ server_name: string }>('mcp:connection-lost', e => {
        set(s => ({
          mcpDisconnected: s.mcpDisconnected.includes(e.payload.server_name)
            ? s.mcpDisconnected
            : [...s.mcpDisconnected, e.payload.server_name],
          mcpServerHealth: s.mcpServerHealth.map(h =>
            h.server_name === e.payload.server_name
              ? { ...h, state: 'disconnected' as McpConnectionState }
              : h
          ),
        }))
      })
      _unlistenFns.push(unlistenLost)

      // Reconnected — remove from disconnected list
      const unlistenReconn = await listen<{ server_name: string }>('mcp:reconnected', e => {
        set(s => ({
          mcpDisconnected: s.mcpDisconnected.filter(n => n !== e.payload.server_name),
          mcpServerHealth: s.mcpServerHealth.map(h =>
            h.server_name === e.payload.server_name
              ? { ...h, state: 'connected' as McpConnectionState, consecutive_failures: 0 }
              : h
          ),
        }))
      })
      _unlistenFns.push(unlistenReconn)

      // Degraded — server responded but slowly (>2s avg latency). Patch the
      // server's state so the McpHealthPanel badge flips to yellow without
      // waiting for the next snapshot. Not added to mcpDisconnected because
      // the server is still functional.
      const unlistenDegraded = await listen<{ server_name: string, latency_ms: number }>(
        'mcp:connection-degraded',
        e => {
          set(s => ({
            mcpServerHealth: s.mcpServerHealth.map(h =>
              h.server_name === e.payload.server_name
                ? { ...h, state: 'degraded' as McpConnectionState, avg_latency_ms: e.payload.latency_ms }
                : h
            ),
          }))
        }
      )
      _unlistenFns.push(unlistenDegraded)

      // Reconnect attempt failed — keep the server in mcpDisconnected and
      // surface the failure on the health record so the UI Reconnect button
      // shows the latest error.
      const unlistenReconnFailed = await listen<{ server_name: string, attempt?: number }>(
        'mcp:reconnect-failed',
        e => {
          set(s => ({
            mcpDisconnected: s.mcpDisconnected.includes(e.payload.server_name)
              ? s.mcpDisconnected
              : [...s.mcpDisconnected, e.payload.server_name],
            mcpServerHealth: s.mcpServerHealth.map(h =>
              h.server_name === e.payload.server_name
                ? {
                    ...h,
                    state: 'reconnecting' as McpConnectionState,
                    error_message: `reconnect failed${e.payload.attempt ? ` (attempt ${e.payload.attempt})` : ''}`,
                  }
                : h
            ),
          }))
        }
      )
      _unlistenFns.push(unlistenReconnFailed)

      // Stdio MCP server needs manual restart — the monitor can't relaunch
      // it without knowing the original spawn command. Mark as disconnected
      // and stamp a clear message so the UI prompts the user to restart.
      const unlistenRequiresRestart = await listen<{ server_name: string }>(
        'mcp:requires-restart',
        e => {
          set(s => ({
            mcpDisconnected: s.mcpDisconnected.includes(e.payload.server_name)
              ? s.mcpDisconnected
              : [...s.mcpDisconnected, e.payload.server_name],
            mcpServerHealth: s.mcpServerHealth.map(h =>
              h.server_name === e.payload.server_name
                ? {
                    ...h,
                    state: 'disconnected' as McpConnectionState,
                    error_message: 'stdio server crashed — manual restart required',
                  }
                : h
            ),
          }))
        }
      )
      _unlistenFns.push(unlistenRequiresRestart)

      // In-process MCP server port conflict — emitted once at startup when
      // the preferred port (8767) couldn't be bound and we fell back to
      // 8768/8769/8770. Carries the holding PID and a ghost-socket flag.
      const unlistenPortConflict = await listen<McpPortStatus>(
        'mcp:port-conflict',
        e => { set({ mcpPortStatus: e.payload }) }
      )
      _unlistenFns.push(unlistenPortConflict)
    } catch (e) {
      console.error("[sidecarStore] Failed to register startup:progress listener:", e);
      _listenersInstalled = false; // allow retry
    }
  },
}));
