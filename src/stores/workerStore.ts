// Worker store — Zustand store for the Worker Registry (workers.db).
// Phase 2.2: Dynamic worker management. The Agent Roster reads from here
// instead of the hardcoded AGENT_DEFS array.
// See: DEVELOPMENT_PLAN.md Phase 2.2 — Worker Management System

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

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
}))
