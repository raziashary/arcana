import { create } from 'zustand'

export type ToastType = 'info' | 'warning' | 'error' | 'success'

export interface Toast {
  id:        string
  title:     string
  message:   string
  type:      ToastType
  workerSlug?: string
  durationMs: number
  createdAt: number
}

interface ToastStoreState {
  toasts: Toast[]
  addToast:     (t: Omit<Toast, 'id' | 'createdAt'>) => void
  removeToast:  (id: string) => void
  clearAll:     () => void
}

let _idCounter = 0
function nextId(): string {
  return `toast-${++_idCounter}-${Date.now()}`
}

export const useToastStore = create<ToastStoreState>((set) => ({
  toasts: [],

  addToast: (partial) => {
    const toast: Toast = {
      id: nextId(),
      createdAt: Date.now(),
      ...partial,
    }
    set((state) => ({ toasts: [...state.toasts, toast] }))
  },

  removeToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }))
  },

  clearAll: () => set({ toasts: [] }),
}))
