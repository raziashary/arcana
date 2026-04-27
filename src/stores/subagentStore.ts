// subagentStore — Phase 4.9 frontend state for the Subagent System.
//
// Mirrors src-tauri/src/subagent/types.rs. Listens to the global
// `subagent:event` Tauri event and updates per-subagent feeds.
//
// See: docs/development/PHASE_4_9_EPHEMERAL_VIA_LIFECYCLE.md

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ── Types (mirror Rust types.rs) ───────────────────────────────────────────

export type SubagentStatus =
  | 'spawning'
  | 'idle'
  | 'running'
  | 'completed'
  | 'failed'
  | 'terminated';

export interface SubagentConfig {
  parent_slug: string;
  parent_session_id: string;
  system_prompt: string;
  tools: string[];
  model: string;
  working_dir: string;
  max_turns: number;
  timeout_secs: number;
  mcp_servers: string[];
  capabilities: string[];
  depth: number;
}

export interface SubagentHandle {
  id: string;
  config: SubagentConfig;
  status: SubagentStatus;
  created_at: string;
}

export interface SubagentSummary {
  id: string;
  parent_slug: string;
  parent_session_id: string;
  status: SubagentStatus;
  model: string;
  created_at: string;
  depth: number;
  last_event: string | null;
}

export interface SubagentResult {
  subagent_id: string;
  status: SubagentStatus;
  result_summary: string;
  files_modified: string[];
  cost_usd: number;
  tokens_in: number;
  tokens_out: number;
  duration_secs: number;
  error: string | null;
}

// SubagentEvent uses serde tagged "kind" — match the Rust enum exactly.
export type SubagentEvent =
  | { kind: 'token';      subagent_id: string; text: string }
  | { kind: 'thinking';   subagent_id: string; text: string }
  | { kind: 'tool_start'; subagent_id: string; tool: string; input: unknown }
  | { kind: 'tool_end';   subagent_id: string; tool: string; output: string; is_error: boolean }
  | { kind: 'final';      subagent_id: string; text: string }
  | { kind: 'cost';       subagent_id: string; cost_usd: number; tokens_in: number; tokens_out: number }
  | { kind: 'status';     subagent_id: string; status: SubagentStatus; message: string | null }
  | { kind: 'error';      subagent_id: string; message: string };

interface SubagentEventPayload {
  subagent_id: string;
  event: SubagentEvent;
}

export interface SpawnSubagentRequest {
  parent_slug: string;
  parent_session_id: string;
  task_description: string;
  system_prompt?: string;
  tools?: string[];
  model?: string;
  working_dir?: string;
  max_turns?: number;
  timeout_secs?: number;
  mcp_servers?: string[];
  depth?: number;
  capabilities?: string[];
}

// ── Store ────────────────────────────────────────────────────────────────

interface SubagentState {
  // id → handle (live subagents only; cleared after `terminate_subagent`)
  active: Record<string, SubagentHandle>;
  // id → ordered list of events received so far (for the EventStream UI)
  events: Record<string, SubagentEvent[]>;
  // id → final result (set when `await_result` resolves)
  results: Record<string, SubagentResult>;
  // True while we have an active listener subscription.
  isListening: boolean;
  unlisten: UnlistenFn | null;

  // ── actions ─────────────────────────────────────────────────────
  /** Subscribe to the global subagent:event channel. Idempotent. */
  startListening: () => Promise<void>;
  stopListening: () => void;

  /** Spawn a subagent and immediately track its handle. */
  spawnSubagent: (req: SpawnSubagentRequest) => Promise<SubagentHandle>;

  /** Block on a subagent's completion and store its result. */
  executeSubtask: (subagentId: string) => Promise<SubagentResult>;

  /** Status snapshot (also refreshes the local handle). */
  getStatus: (subagentId: string) => Promise<SubagentStatus>;

  /** Graceful or forceful termination + cleanup. */
  terminateSubagent: (subagentId: string, force?: boolean) => Promise<void>;

  /** Hard abort — alias for terminate(force=true). */
  abortSubagent: (subagentId: string) => Promise<void>;

  /** Refresh the active list from the backend. */
  refreshActive: (parentSessionId?: string) => Promise<void>;

  /** Clear local state for a subagent (after the user dismisses its card). */
  forget: (subagentId: string) => void;
}

export const useSubagentStore = create<SubagentState>((set, get) => ({
  active: {},
  events: {},
  results: {},
  isListening: false,
  unlisten: null,

  startListening: async () => {
    if (get().isListening) return;
    const unlisten = await listen<SubagentEventPayload>('subagent:event', (e) => {
      const { subagent_id, event } = e.payload;
      set((state) => {
        // Append to event log
        const prior = state.events[subagent_id] ?? [];
        const events = { ...state.events, [subagent_id]: [...prior, event] };

        // Track status transitions on the active handle
        let active = state.active;
        if (event.kind === 'status' && active[subagent_id]) {
          active = {
            ...active,
            [subagent_id]: { ...active[subagent_id], status: event.status },
          };
        }

        return { ...state, events, active };
      });
    });
    set({ isListening: true, unlisten });
  },

  stopListening: () => {
    const { unlisten } = get();
    if (unlisten) unlisten();
    set({ isListening: false, unlisten: null });
  },

  spawnSubagent: async (req) => {
    const handle = await invoke<SubagentHandle>('spawn_subagent', { request: req });
    set((s) => ({ active: { ...s.active, [handle.id]: handle } }));
    return handle;
  },

  executeSubtask: async (subagentId) => {
    const result = await invoke<SubagentResult>('execute_subtask', { subagentId });
    set((s) => ({
      results: { ...s.results, [subagentId]: result },
      active: dropKey(s.active, subagentId), // result lifecycle done; remove handle
    }));
    return result;
  },

  getStatus: async (subagentId) => {
    const status = await invoke<SubagentStatus>('get_subagent_status', { subagentId });
    set((s) => {
      const handle = s.active[subagentId];
      if (!handle) return s;
      return { active: { ...s.active, [subagentId]: { ...handle, status } } };
    });
    return status;
  },

  terminateSubagent: async (subagentId, force = false) => {
    await invoke('terminate_subagent', { subagentId, force });
    set((s) => ({ active: dropKey(s.active, subagentId) }));
  },

  abortSubagent: async (subagentId) => {
    await invoke('abort_subagent', { subagentId });
    set((s) => ({ active: dropKey(s.active, subagentId) }));
  },

  refreshActive: async (parentSessionId) => {
    const list = await invoke<SubagentSummary[]>('list_active_subagents', {
      parentSessionId: parentSessionId ?? null,
    });
    // Convert SubagentSummary[] back to handle-like map (config is unknown
    // server-side from this endpoint; we keep just status + metadata).
    set((s) => {
      const active = { ...s.active };
      for (const summary of list) {
        const existing = active[summary.id];
        active[summary.id] = existing
          ? { ...existing, status: summary.status }
          : {
              id: summary.id,
              status: summary.status,
              created_at: summary.created_at,
              config: {
                parent_slug:       summary.parent_slug,
                parent_session_id: summary.parent_session_id,
                system_prompt:     '',
                tools:             [],
                model:             summary.model,
                working_dir:       '',
                max_turns:         0,
                timeout_secs:      0,
                mcp_servers:       [],
                capabilities:      [],
                depth:             summary.depth,
              },
            };
      }
      return { active };
    });
  },

  forget: (subagentId) => {
    set((s) => ({
      active:  dropKey(s.active, subagentId),
      events:  dropKey(s.events, subagentId),
      results: dropKey(s.results, subagentId),
    }));
  },
}));

function dropKey<T extends Record<string, unknown>>(obj: T, key: string): T {
  if (!(key in obj)) return obj;
  const next = { ...obj };
  delete (next as Record<string, unknown>)[key];
  return next as T;
}
