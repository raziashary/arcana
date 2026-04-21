// startupStore.ts — Drives the startup loading overlay.
//
// Subscribes to the Rust-side `startup:progress` Tauri event and advances a
// forward-only state machine that mirrors `src-tauri/src/sidecar/startup_progress.rs`.
//
// Phase IDs MUST stay in sync with the `StartupPhase::id()` values in Rust.
// See: autodev/docs/development/startup_sequence_and_agent_persistence_analysis.md

import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// ── Phases ────────────────────────────────────────────────────────────────────
// Order here is the canonical ordering; `index` is used for progress-bar math.

export type StartupPhaseId =
  | "init"
  | "skills"
  | "memory_starting"
  | "vector_db"
  | "memory_ready"
  | "orch_starting"
  | "routers"
  | "agents"
  | "living_agents"
  | "orch_ready"
  | "workers"
  | "ready";

export interface StartupPhase {
  id: StartupPhaseId;
  label: string;
  status: "pending" | "active" | "complete";
}

export interface StartupProgressPayload {
  phase: StartupPhaseId;
  index: number;
  total: number;
  message: string;
}

// UI label for each phase. The Rust side also ships a `message` string in the
// event payload; we prefer the Rust text if it is present so wording stays
// consistent between tracing logs and the UI.
const PHASE_DEFS: Array<{ id: StartupPhaseId; label: string }> = [
  { id: "init",             label: "Initializing AutoDev…" },
  { id: "skills",           label: "Loading skills…" },
  { id: "memory_starting",  label: "Starting Memory MCP Server…" },
  { id: "vector_db",        label: "Connecting to vector database…" },
  { id: "memory_ready",     label: "Memory MCP Server ready" },
  { id: "orch_starting",    label: "Starting Core Orchestrator…" },
  { id: "routers",          label: "Loading API routers…" },
  { id: "agents",           label: "Initializing agent registry…" },
  { id: "living_agents",    label: "Creating LivingAgents…" },
  { id: "orch_ready",       label: "Core Orchestrator ready" },
  { id: "workers",          label: "Starting worker fleet…" },
  { id: "ready",            label: "AutoDev is ready" },
];

const PHASE_INDEX: Record<StartupPhaseId, number> = PHASE_DEFS.reduce(
  (acc, p, i) => ({ ...acc, [p.id]: i }),
  {} as Record<StartupPhaseId, number>,
);

function initialPhases(): StartupPhase[] {
  return PHASE_DEFS.map((p) => ({ ...p, status: "pending" }));
}

// ── Store ─────────────────────────────────────────────────────────────────────

interface StartupState {
  phases: StartupPhase[];
  currentIndex: number;          // -1 until first event arrives
  currentMessage: string | null; // rust-supplied message for the active phase
  isComplete: boolean;
  /// True once the app has decided the overlay is no longer needed.
  /// Flipped when phase reaches "ready" OR when the watchdog fires (sidecars
  /// remained healthy for long enough that we assume nothing is coming).
  dismissed: boolean;

  // Actions
  subscribe: () => Promise<UnlistenFn>;
  dismiss: () => void;
  /// Development helper — reset to pending. Not called in production.
  _reset: () => void;
}

export const useStartupStore = create<StartupState>((set, get) => ({
  phases: initialPhases(),
  currentIndex: -1,
  currentMessage: null,
  isComplete: false,
  dismissed: false,

  subscribe: async () => {
    // Apply a phase payload to the store. Used by both the live event
    // listener and the on-mount `get_startup_phase` snapshot.
    const apply = (payload: StartupProgressPayload) => {
      const { phase, message } = payload;
      const idx = PHASE_INDEX[phase];
      if (idx === undefined) return;

      set((state) => {
        // Monotonic: never roll the UI backwards even if Rust (shouldn't)
        // emits out of order.
        if (idx < state.currentIndex) return state;

        const phases = state.phases.map((p, i) => {
          if (i < idx)  return { ...p, status: "complete" as const };
          if (i === idx) return { ...p, status: "active"   as const };
          return { ...p, status: "pending" as const };
        });

        const isComplete = phase === "ready";
        if (isComplete) {
          phases[idx] = { ...phases[idx], status: "complete" };
        }

        return {
          phases,
          currentIndex:   idx,
          currentMessage: message,
          isComplete,
        };
      });

      // Auto-dismiss a beat after we reach "ready" so the user gets to see
      // the green check before the overlay disappears.
      if (phase === "ready") {
        setTimeout(() => get().dismiss(), 600);
      }
    };

    // 1) Register the live listener FIRST so we don't miss any phases that
    //    fire between this point and the snapshot fetch below.
    const unlisten = await listen<StartupProgressPayload>(
      "startup:progress",
      (event) => apply(event.payload),
    );

    // 2) Pull the current phase from Rust. This recovers any events that
    //    were emitted before this listener was registered (Tauri does NOT
    //    replay missed events). The store's monotonic guard means the
    //    snapshot is a no-op if the live listener already advanced past it.
    try {
      const snap = await invoke<StartupProgressPayload>("get_startup_phase");
      apply(snap);
    } catch (err) {
      // Non-fatal: the live listener will still drive the UI for any future
      // transitions. Log to aid debugging if the overlay ever sticks.
      console.warn("[startupStore] get_startup_phase snapshot failed:", err);
    }

    return unlisten;
  },

  dismiss: () => set({ dismissed: true }),

  _reset: () =>
    set({
      phases: initialPhases(),
      currentIndex: -1,
      currentMessage: null,
      isComplete: false,
      dismissed: false,
    }),
}));
