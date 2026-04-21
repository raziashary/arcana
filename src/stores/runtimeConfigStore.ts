// runtimeConfigStore.ts — Mirror of Rust's `runtime_config.rs` on-disk JSON.
//
// Holds flags that influence app-launch behaviour and therefore can't live in
// localStorage (which is only readable from the webview, AFTER the webview
// has loaded — too late for the sidecar spawn task).
//
// Today the only flag is `skip_sidecars` (Demo Mode). Changing the flag does
// NOT take effect until the next app launch, because the sidecar auto-start
// task only runs once during the Tauri `setup()` hook. The Settings UI shows
// an inline "restart required" notice when the value is changed.

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface RuntimeConfig {
  skip_sidecars: boolean;
}

interface RuntimeConfigState {
  config: RuntimeConfig | null;
  /// True when the flag in memory differs from what was last loaded from disk —
  /// used to render the "restart required" banner.
  dirty: boolean;
  isLoading: boolean;
  error: string | null;

  load:             () => Promise<void>;
  setSkipSidecars:  (skip: boolean) => Promise<void>;
}

export const useRuntimeConfigStore = create<RuntimeConfigState>((set, get) => ({
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

  setSkipSidecars: async (skip: boolean) => {
    const prev = get().config;
    try {
      const cfg = await invoke<RuntimeConfig>("set_skip_sidecars", { skip });
      // `dirty` flips on whenever the user has changed the value during this
      // session. Even if they toggle it back to its original value we still
      // keep dirty=true, because the on-disk file has been rewritten and a
      // restart is the safest way to reach a known-good state.
      set({ config: cfg, dirty: prev?.skip_sidecars !== skip || get().dirty });
    } catch (err) {
      set({ error: String(err) });
    }
  },
}));
