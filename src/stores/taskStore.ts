// Task store — Internal AutoDev Task Board state.
// Reads tasks from Tauri (SQLite ~/.autodev/tasks.db) and provides kanban-ready columns.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export type TaskPriority = 'p0' | 'p1' | 'p2' | 'p3'
export type TaskType     = 'feature' | 'bug' | 'refactor' | 'docs' | 'chore'
export type TaskStatus   = 'ready' | 'assigned' | 'in_progress' | 'qc_review' | 'done' | 'failed' | 'cancelled'

export interface Task {
  id:                  number
  title:               string
  description:         string | null
  priority:            TaskPriority
  task_type:           TaskType
  status:              TaskStatus
  acceptance_criteria: string
  assigned_worker:     string | null
  created_by:          string
  created_at:          string
  updated_at:          string
  completed_at:        string | null
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
  isLoading:    boolean
  error:        string | null

  loadTasks:          ()                                                    => Promise<void>
  updateTaskStatus:   (id: number, status: TaskStatus, worker?: string)    => Promise<void>
  deleteTask:         (id: number)                                          => Promise<void>
  tasksByStatus:      (status: TaskStatus)                                  => Task[]
}

export const useTaskStore = create<TaskState>((set, get) => ({
  tasks:     [],
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

  tasksByStatus: (status) => get().tasks.filter(t => t.status === status),
}))
