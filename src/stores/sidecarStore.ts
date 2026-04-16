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
import { create } from "zustand";

export type SidecarStatusStr =
  | "stopped"
  | "starting"
  | "running"
  | `error: ${string}`;

interface SidecarState {
  status: SidecarStatusStr;
  isConnected: boolean;

  // Actions
  loadStatus: () => Promise<void>;
  checkConnectivity: () => Promise<boolean>;
  startSidecar: (path?: string) => Promise<string>;
  stopSidecar: () => Promise<void>;
}

export const useSidecarStore = create<SidecarState>((set) => ({
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
}));
