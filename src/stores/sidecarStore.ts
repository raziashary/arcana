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

interface StartupProgressPayload {
  phase: string;
  message?: string;
}

interface SidecarState {
  status: SidecarStatusStr;
  isConnected: boolean;

  // Actions
  loadStatus: () => Promise<void>;
  checkConnectivity: () => Promise<boolean>;
  startSidecar: (path?: string) => Promise<string>;
  stopSidecar: () => Promise<void>;
  initEventListeners: () => Promise<void>;
}

// Module-scoped guard to prevent duplicate listener registration (React StrictMode double-mount)
let _listenersInstalled = false;
const _unlistenFns: UnlistenFn[] = [];

export const useSidecarStore = create<SidecarState>((set, get) => ({
  status: "stopped",
  isConnected: false,

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
    } catch (e) {
      console.error("[sidecarStore] Failed to register startup:progress listener:", e);
      _listenersInstalled = false; // allow retry
    }
  },
}));
