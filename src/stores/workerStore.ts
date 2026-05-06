// Worker store — Zustand store for the Worker Registry (workers.db).
// Phase 2.2: Dynamic worker management. The Agent Roster reads from here
// instead of the hardcoded AGENT_DEFS array.
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// ─── Types (mirror Rust Worker struct) ───────────────────────────────────────

export interface Worker {
  id:                number
  name:              string
  slug:              string
  worker_type:       string
  status:            WorkerStatus
  model:             string
  primary_repos:     string[]
  polling_interval:  number
  recipe_id:         string | null
  is_system_defined: boolean
  description:       string | null
  created_at:        string
  updated_at:        string
  // D.2/D.3 — tick timing fields populated by worker:tick-start/end events
  tick_started_at:    string | undefined
  tick_ended_at:      string | undefined
  tick_duration_ms:   number | undefined
  last_outcome:       string | undefined
  // D.4 — mode of the currently in-flight tick (cleared on tick-end).
  // 'mode2' while LivingAgent tick is running; 'mode1' during Rust fallback.
  // Takes precedence over last_outcome in the badge so mid-tick state is visible.
  current_tick_mode:  string | undefined
}

export type WorkerStatus =
  | 'created' | 'idle' | 'active' | 'working' | 'paused' | 'error' | 'deleted'

export interface CreateWorkerInput {
  name:             string
  worker_type:      string
  model:            string
  primary_repos:    string[]
  polling_interval?: number
  description?:     string
}

export interface CreateWorkerOutput {
  id:   number
  slug: string
}

// ─── Status helpers ───────────────────────────────────────────────────────────

export function workerStatusColor(status: WorkerStatus): string {
  switch (status) {
    case 'active':  return '#7FDBCA'   // aqua — live heartbeat
    case 'working': return '#FFCB8B'   // amber — in-progress
    case 'idle':    return '#82AAFF'   // periwinkle — waiting
    case 'paused':  return '#637777'   // muted — paused
    case 'error':   return '#FF6363'   // coral — error
    default:        return '#3D5A6E'   // dim — created / deleted
  }
}

export function workerStatusLabel(status: WorkerStatus): string {
  switch (status) {
    case 'created': return 'Created'
    case 'idle':    return 'Idle'
    case 'active':  return 'Active'
    case 'working': return 'Working'
    case 'paused':  return 'Paused'
    case 'error':   return 'Error'
    case 'deleted': return 'Deleted'
  }
}

// Worker type display labels
export const WORKER_TYPE_LABELS: Record<string, string> = {
  backend:    'Backend',
  frontend:   'Frontend',
  platform:   'Platform',
  mcp:        'MCP Builder',
  cleanup:    'Cleanup',
  supervisor: 'Supervisor',
  qc:         'QC Agent',
  custom:     'Custom',
}

// ─── Store ────────────────────────────────────────────────────────────────────

interface WorkerStoreState {
  workers:    Worker[]
  isLoading:  boolean
  error:      string | null

  // Actions
  loadWorkers:          () => Promise<void>
  createWorker:         (input: CreateWorkerInput) => Promise<CreateWorkerOutput>
  updateWorkerStatus:   (workerId: number, status: WorkerStatus) => Promise<void>
  updateWorkerModel:    (workerId: number, model: string) => Promise<void>
  updateWorkerRepos:    (workerId: number, repos: string[]) => Promise<void>
  bindWorkerRecipe:     (workerId: number, recipeId: string) => Promise<void>
  deleteWorker:         (workerId: number) => Promise<void>
  startWorker:          (workerId: number) => Promise<void>
  stopWorker:           (workerId: number) => Promise<void>

  // Derived selectors
  getWorkerBySlug:      (slug: string) => Worker | undefined
  activeWorkers:        () => Worker[]

  // Phase 3.0 R2 — push-channel subscription. Call once from App.tsx at
  // mount. Idempotent: guarded by the private flag below.
  initEventListeners:   () => Promise<void>
}

// Module-scoped guard so repeated calls to initEventListeners don't register
// duplicate handlers (React StrictMode double-mounts effects in development).
let _listenersInstalled = false
const _unlistenFns: UnlistenFn[] = []

// Phase 3.0 R2 event payload (matches Rust `WorkerStatusPayload`).
// Fix 9a: expanded with previous_status, reason, cooldown_seconds.
// D.2: expanded with tick_duration_ms, last_outcome.
interface WorkerStatusEvent {
  slug:               string
  status:             WorkerStatus
  previous_status:    string | null
  reason:             string | null
  cooldown_seconds:   number | null
  timestamp:          string
  tick_duration_ms:   number | null
  last_outcome:       string | null
}

// D.3 — tick boundary events (matches Rust WorkerTickStartPayload / WorkerTickEndPayload)
interface WorkerTickStartPayload {
  slug:      string
  mode:      string
  timestamp: string
}

interface WorkerTickEndPayload {
  slug:      string
  outcome:   string
  timestamp: string
}

// BUG-004 — QC decision event payload (matches Rust QcDecisionPayload).
interface QcDecisionPayload {
  task_id:          number
  task_title:       string
  decision:         string
  rejection_reason: string | null
  worker_slug:      string
}

export const useWorkerStore = create<WorkerStoreState>((set, get) => ({
  workers:   [],
  isLoading: false,
  error:     null,

  loadWorkers: async () => {
    set({ isLoading: true, error: null })
    try {
      const workers = await invoke<Worker[]>('list_workers')
      set({ workers, isLoading: false })
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      set({ error: msg, isLoading: false })
      console.error('[workerStore] loadWorkers failed:', e)
    }
  },

  createWorker: async (input) => {
    const result = await invoke<CreateWorkerOutput>('create_worker', { input })
    await get().loadWorkers()
    return result
  },

  updateWorkerStatus: async (workerId, status) => {
    await invoke('update_worker_status', { workerId, status })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, status } : w
      ),
    }))
  },

  updateWorkerModel: async (workerId, model) => {
    await invoke('update_worker_model', { workerId, model })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, model } : w
      ),
    }))
  },

  updateWorkerRepos: async (workerId, repos) => {
    await invoke('update_worker_repos', { workerId, primaryRepos: repos })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, primary_repos: repos } : w
      ),
    }))
  },

  bindWorkerRecipe: async (workerId, recipeId) => {
    await invoke('bind_worker_recipe', { workerId, recipeId })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, recipe_id: recipeId } : w
      ),
    }))
  },

  deleteWorker: async (workerId) => {
    await invoke('delete_worker', { workerId })
    set(state => ({
      workers: state.workers.filter(w => w.id !== workerId),
    }))
  },

  startWorker: async (workerId) => {
    await invoke('start_worker_recipe', { workerId })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, status: 'active' as WorkerStatus } : w
      ),
    }))
  },

  stopWorker: async (workerId) => {
    await invoke('stop_worker_recipe', { workerId })
    set(state => ({
      workers: state.workers.map(w =>
        w.id === workerId ? { ...w, status: 'paused' as WorkerStatus } : w
      ),
    }))
  },

  getWorkerBySlug: (slug) => get().workers.find(w => w.slug === slug),

  activeWorkers: () =>
    get().workers.filter(w => w.status === 'active' || w.status === 'working'),

  initEventListeners: async () => {
    // Phase 3.0 R2 — register the `worker:status` push listener exactly
    // once per app lifetime. Replaces the old pattern of deriving status
    // from task-board staleness, which was the root of the `idle → error
    // → idle` flicker observed in the Agent Roster (see
    // docs/development/runtime-fixes/phase3/fleet-flow-analysis.md §2.6).
    if (_listenersInstalled) return
    _listenersInstalled = true

    // Fix 9b: dedup map — prevent toast spam on repeated error transitions.
    const _lastToastTs: Record<string, number> = {}
    const TOAST_DEDUP_MS = 5000

    try {
      const unlisten = await listen<WorkerStatusEvent>('worker:status', (event) => {
        const { slug, status, reason, cooldown_seconds, tick_duration_ms, last_outcome } = event.payload
        set(state => ({
          workers: state.workers.map(w =>
            w.slug === slug
              ? {
                  ...w,
                  status,
                  // D.2 — propagate timing info when present in the status event
                  ...(tick_duration_ms != null ? { tick_duration_ms } : {}),
                  ...(last_outcome    != null ? { last_outcome }    : {}),
                }
              : w
          ),
        }))
        // Fix 9b: emit toast on error or cooldown transitions
        const now = Date.now()
        if (status === 'error') {
          const last = _lastToastTs[slug] ?? 0
          if (now - last > TOAST_DEDUP_MS) {
            _lastToastTs[slug] = now
            const { addToast } = require('../stores/toastStore')
            addToast({
              title: `Worker ${slug} errored`,
              message: reason ?? 'Invoke failed — see logs for details',
              type: 'error',
              workerSlug: slug,
              durationMs: cooldown_seconds ? cooldown_seconds * 1000 : 4000,
            })
          }
        }
      })
      _unlistenFns.push(unlisten)

      // D.3 — subscribe to tick boundary events for granular per-agent state
      const unlistenTickStart = await listen<WorkerTickStartPayload>('worker:tick-start', (event) => {
        const { slug, timestamp, mode } = event.payload
        set(state => ({
          workers: state.workers.map(w =>
            w.slug === slug
              ? { ...w, tick_started_at: timestamp, current_tick_mode: mode }
              : w
          ),
        }))
      })
      _unlistenFns.push(unlistenTickStart)

      const unlistenTickEnd = await listen<WorkerTickEndPayload>('worker:tick-end', (event) => {
        const { slug, outcome, timestamp } = event.payload
        set(state => ({
          workers: state.workers.map(w =>
            w.slug === slug
              ? { ...w, last_outcome: outcome, tick_ended_at: timestamp, current_tick_mode: undefined }
              : w
          ),
        }))
      })
      _unlistenFns.push(unlistenTickEnd)

      // BUG-004 — QC decision toast event.
      const unlistenQc = await listen<QcDecisionPayload>('qc:decision', (event) => {
        const { task_id, task_title, decision, rejection_reason } = event.payload
        const { addToast } = require('../stores/toastStore')
        if (decision === 'approved') {
          addToast({
            title: `✅ QC passed — Task #${task_id} done`,
            message: task_title,
            type: 'success',
            durationMs: 5000,
          })
        } else {
          addToast({
            title: `⛔ QC rejected — Task #${task_id}`,
            message: rejection_reason?.split('\n')[0] ?? 'See board for details',
            type: 'error',
            durationMs: 10000,
          })
        }
      })
      _unlistenFns.push(unlistenQc)
    } catch (e) {
      console.error('[workerStore] Failed to register worker:status listener:', e)
      _listenersInstalled = false  // allow retry
    }
  },
}))
