// runtimeConfigStore.ts — Mirror of Rust's `runtime_config.rs` on-disk JSON.
//
// Holds flags that influence app-launch behaviour and therefore can't live in
// localStorage (which is only readable from the webview, AFTER the webview
// has loaded — too late for the sidecar spawn task).
//
// It holds the default worker budgets the recipe runner reads without a
// round-trip to the webview.
//
// ⚠ It also carried `skip_sidecars` (Demo Mode) and a `setSkipSidecars`
// action that invoked a command deleted in `P8-53`. Both are gone (`P8-61`).

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface RuntimeConfig {
  // Ruflo Feature 4 — Default Worker Budget settings
  default_worker_budget_tokens: number;
  default_worker_budget_usd:    number;
  default_worker_budget_turns:  number;
  budget_warn_pct:              number;
}

interface RuntimeConfigState {
  config: RuntimeConfig | null;
  /// True when the flag in memory differs from what was last loaded from disk —
  /// used to render the "restart required" banner.
  dirty: boolean;
  isLoading: boolean;
  error: string | null;

  load:                   () => Promise<void>;
  setDefaultWorkerBudget: (maxTokens: number, maxUsd: number, maxTurns: number, warnPct: number) => Promise<void>;
}

export const useRuntimeConfigStore = create<RuntimeConfigState>((set) => ({
  config: null,
  dirty: false,
  isLoading: false,
  error: null,

  load: async () => {
    set({ isLoading: true, error: null });
    try {
      const cfg = await invoke<RuntimeConfig>("get_runtime_config");
      set({ config: cfg, dirty: false, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  setDefaultWorkerBudget: async (maxTokens, maxUsd, maxTurns, warnPct) => {
    try {
      const cfg = await invoke<RuntimeConfig>("set_default_worker_budget", {
        maxTokens, maxUsd, maxTurns, warnPct,
      });
      set({ config: cfg });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
