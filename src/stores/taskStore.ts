// Task store — Internal AutoDev Task Board state.
// Reads tasks from Tauri (SQLite ~/.autodev/tasks.db) and provides kanban-ready columns.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export type TaskPriority = 'p0' | 'p1' | 'p2' | 'p3'
export type TaskType     = 'feature' | 'bug' | 'refactor' | 'docs' | 'chore'
export type TaskStatus   = 'ready' | 'assigned' | 'in_progress' | 'qc_review' | 'done' | 'failed' | 'build_failed' | 'cancelled' | 'rejected'

export interface Task {
  id:                  number
  title:               string
  description:         string | null
  priority:            TaskPriority
  task_type:           TaskType
  status:              TaskStatus
  acceptance_criteria: string
  assigned_worker:     string | null
  rejection_reason:    string | null
  retry_count:         number
  created_by:          string
  created_at:          string
  updated_at:          string
  completed_at:        string | null
}

// ─── Worker Request Types ─────────────────────────────────────────────────────

export type WorkerRequestStatus = 'pending' | 'acknowledged' | 'completed' | 'rejected'

export interface WorkerRequest {
  id:                number
  worker_slug:       string
  task_id:           number | null
  request_type:      string
  payload:           string
  status:            WorkerRequestStatus
  resolution_note:   string | null
  created_at:        string
  resolved_at:       string | null
}

// Kanban columns ordered for display
export const KANBAN_COLUMNS: { status: TaskStatus; label: string }[] = [
  { status: 'ready',       label: 'Ready'       },
  { status: 'assigned',    label: 'Assigned'    },
  { status: 'in_progress', label: 'In Progress' },
  { status: 'qc_review',   label: 'QC Review'   },
  { status: 'done',        label: 'Done'        },
  { status: 'failed',      label: 'Failed'      },
]

const PRIORITY_COLORS: Record<TaskPriority, string> = {
  p0: 'var(--error)',
  p1: 'var(--warning)',
  p2: 'var(--accent)',
  p3: 'var(--text-4)',
}

export const priorityColor = (p: TaskPriority) => PRIORITY_COLORS[p] ?? 'var(--text-4)'
export const priorityLabel = (p: TaskPriority) => ({ p0: 'P0', p1: 'P1', p2: 'P2', p3: 'P3' }[p])

interface TaskState {
  tasks:        Task[]
  qcQueue:      { task: Task; ageSeconds: number }[]
  workerRequests:   WorkerRequest[]
  isLoading:    boolean
  error:        string | null

  loadTasks:          ()                                                    => Promise<void>
  loadQcQueue:        ()                                                    => Promise<void>
  loadWorkerRequests: (status?: WorkerRequestStatus | null)                   => Promise<void>
  resolveWorkerRequest: (id: number, action: WorkerRequestStatus, note?: string) => Promise<void>
  updateTaskStatus:   (id: number, status: TaskStatus, worker?: string)    => Promise<void>
  deleteTask:         (id: number)                                          => Promise<void>
  tasksByStatus:      (status: TaskStatus)                                  => Task[]

  // Phase 3.0 R1 — live task-board push channel. Call once from App.tsx.
  initEventListeners: () => Promise<void>
}

// Module-level guard so StrictMode double-mounts don't double-register.
let _taskListenersInstalled = false
const _taskUnlistenFns: UnlistenFn[] = []

export const useTaskStore = create<TaskState>((set, get) => ({
  tasks:     [],
  qcQueue:   [],
  workerRequests: [],
  isLoading: false,
  error:     null,

  loadTasks: async () => {
    set({ isLoading: true, error: null })
    try {
      const tasks = await invoke<Task[]>('list_tasks', { status: null })
      set({ tasks })
    } catch (e) {
      set({ error: String(e) })
      console.error('Failed to load tasks:', e)
    } finally {
      set({ isLoading: false })
    }
  },

  loadQcQueue: async () => {
    try {
      const queue = await invoke<{ task: Task; ageSeconds: number }[]>('get_qc_queue')
      set({ qcQueue: queue })
    } catch (e) {
      console.error('Failed to load QC queue:', e)
    }
  },

  updateTaskStatus: async (id, status, worker) => {
    try {
      await invoke('update_task_status', { id, status, assignedWorker: worker ?? null })
      await get().loadTasks()
    } catch (e) {
      console.error('Failed to update task status:', e)
    }
  },

  deleteTask: async (id) => {
    try {
      await invoke('delete_task', { id })
      set(s => ({ tasks: s.tasks.filter(t => t.id !== id) }))
    } catch (e) {
      console.error('Failed to delete task:', e)
    }
  },

  loadWorkerRequests: async (status) => {
    try {
      const reqs = await invoke<WorkerRequest[]>('list_worker_requests', { status: status ?? null })
      set({ workerRequests: reqs })
    } catch (e) {
      console.error('Failed to load worker requests:', e)
    }
  },

  resolveWorkerRequest: async (id, action, note) => {
    try {
      await invoke('resolve_worker_request', { id, action, note: note ?? null })
      await get().loadWorkerRequests('pending')
    } catch (e) {
      console.error('Failed to resolve worker request:', e)
    }
  },

  tasksByStatus: (status) => get().tasks.filter(t => t.status === status),

  initEventListeners: async () => {
    // Phase 3.0 R1 — subscribe to the board-push channel. Every task write
    // (create / assign / status update / delete / clear_all) emits
    // `task:changed` from Rust; we react by re-fetching. Simple and
    // correct — the list is small (bounded by how many tasks the user has
    // open). Fine-grained patching can replace this if the board ever grows.
    if (_taskListenersInstalled) return
    _taskListenersInstalled = true

    try {
      const unlisten = await listen('task:changed', () => {
        // Fire-and-forget refresh. Keeps the kanban in sync without the UI
        // having to poll.
        void get().loadTasks()
      })
      _taskUnlistenFns.push(unlisten)

      // Worker request push channel — refresh pending requests live
      const unlistenReq = await listen('worker:request', () => {
        void get().loadWorkerRequests('pending')
      })
      _taskUnlistenFns.push(unlistenReq)
    } catch (e) {
      console.error('[taskStore] Failed to register task:changed listener:', e)
      _taskListenersInstalled = false
    }
  },
}))
