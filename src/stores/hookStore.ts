// Hook store — Phase 3.1
// Manages hook configurations and execution logs.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

// ── Types ──────────────────────────────────────────────────────────────────────

export type HookType =
  | 'session_start' | 'session_end' | 'agent_loop_start' | 'agent_loop_end' | 'on_error'
  | 'pre_tool_use' | 'post_tool_use' | 'on_tool_error' | 'on_permission_request'
  | 'pre_file_read' | 'post_file_write' | 'pre_file_delete' | 'on_checkpoint_create' | 'on_checkpoint_restore'
  | 'pre_commit' | 'post_commit' | 'pre_push' | 'on_branch_switch' | 'on_merge_conflict'
  | 'pre_plan_step' | 'post_plan_step' | 'on_plan_activate' | 'on_plan_complete' | 'on_plan_abandon'
  | 'on_task_claim' | 'on_task_complete' | 'on_task_fail' | 'on_qc_review_start' | 'on_qc_approve' | 'on_qc_fail'

export type HookOutcome = 'executed' | 'blocked' | 'skipped' | 'failed'

export interface HookConditions {
  tool_name?:        string
  file_pattern?:     string
  repo_path?:        string
  command_pattern?:  string
  session_type?:     string
  agent_mode?:       string
  time_range?:       { start_hour: number; end_hour: number; days?: string[] }
  /** Rhai expression evaluated with context variables.
   *  Variables: tool_name, file_path, agent_mode, command, hook_type, session_id, worker_id, task_id, is_error
   *  Example: `tool_name == "bash" && command.contains("rm -rf")` */
  custom_condition?: string
}

export type HookAction =
  | { type: 'shell';        command: string; working_dir?: string; timeout_secs?: number }
  | { type: 'http';         url: string; method: string; headers?: Record<string, string>; body?: unknown }
  | { type: 'notification'; title: string; message: string }
  | { type: 'log';          file: string; message: string }
  | { type: 'block';        message?: string }
  | { type: 'emit_event';   event_name: string; payload?: unknown }

export interface HookConfig {
  id:         string
  name:       string
  hook_type:  HookType
  enabled:    boolean
  priority:   number
  conditions: HookConditions
  action:     HookAction
  created_at: string
  updated_at: string
}

export interface HookExecutionLog {
  id:          number
  hook_id:     string
  hook_name:   string
  hook_type:   string
  outcome:     HookOutcome
  output?:     string
  error?:      string
  executed_at: string
  duration_ms: number
}

export interface AddHookInput {
  name:       string
  hook_type:  HookType
  enabled?:   boolean
  priority?:  number
  conditions?: HookConditions
  action:     HookAction
}

/** A pre-built hook template shown in the template gallery. */
export interface HookTemplate {
  id:          string
  name:        string
  description: string
  category:    'safety' | 'automation' | 'notifications' | 'logging'
  hook_type:   HookType
  conditions:  HookConditions
  action:      HookAction
}

// ── Store ──────────────────────────────────────────────────────────────────────

interface HookState {
  hooks:     HookConfig[]
  logs:      HookExecutionLog[]
  isLoading: boolean
  error:     string | null

  loadHooks:    () => Promise<void>
  enableHook:   (id: string) => Promise<void>
  disableHook:  (id: string) => Promise<void>
  addHook:      (input: AddHookInput) => Promise<HookConfig>
  removeHook:   (id: string) => Promise<void>
  loadLogs:     (hookId?: string, limit?: number) => Promise<void>
  testHook:     (hookId: string, context: Record<string, unknown>) => Promise<HookExecutionLog[]>
}

export const useHookStore = create<HookState>((set) => ({
  hooks:     [],
  logs:      [],
  isLoading: false,
  error:     null,

  loadHooks: async () => {
    set({ isLoading: true, error: null })
    try {
      const hooks = await invoke<HookConfig[]>('list_hooks')
      set({ hooks, isLoading: false })
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  enableHook: async (id: string) => {
    await invoke('enable_hook', { hookId: id })
    set(s => ({
      hooks: s.hooks.map(h => h.id === id ? { ...h, enabled: true } : h)
    }))
  },

  disableHook: async (id: string) => {
    await invoke('disable_hook', { hookId: id })
    set(s => ({
      hooks: s.hooks.map(h => h.id === id ? { ...h, enabled: false } : h)
    }))
  },

  addHook: async (input: AddHookInput) => {
    const hook = await invoke<HookConfig>('add_hook', { config: input })
    set(s => ({ hooks: [...s.hooks, hook] }))
    return hook
  },

  removeHook: async (id: string) => {
    await invoke('remove_hook', { hookId: id })
    set(s => ({ hooks: s.hooks.filter(h => h.id !== id) }))
  },

  loadLogs: async (hookId?: string, limit?: number) => {
    try {
      const logs = await invoke<HookExecutionLog[]>('get_hook_logs', {
        hookId: hookId ?? null,
        limit:  limit ?? 100,
      })
      set({ logs })
    } catch (e) {
      set({ error: String(e) })
    }
  },

  testHook: async (hookId: string, context: Record<string, unknown>) => {
    return invoke<HookExecutionLog[]>('test_hook', { hookId, context })
  },
}))
