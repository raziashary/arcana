// annotationStore — Zustand store for inline code annotations.
// Annotations are persisted in annotations.db via Tauri commands and rendered
// as gutter decorations in the IDE CodeViewer.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export interface Annotation {
  id: string
  filePath: string
  startLine: number
  endLine: number
  selectedText?: string
  prompt: string
  response?: string
  source: 'user' | 'qc_agent' | 'worker'
  severity: 'info' | 'warning' | 'blocking'
  status: 'open' | 'pending' | 'resolved'
  taskId?: string
  createdAt: string
  updatedAt: string
}

export interface CreateAnnotationInput {
  filePath: string
  startLine: number
  endLine: number
  selectedText?: string
  prompt: string
  source?: 'user' | 'qc_agent' | 'worker'
  severity?: 'info' | 'warning' | 'blocking'
  taskId?: string
}

interface AnnotationStore {
  // filePath → annotations for that file
  byFile: Record<string, Annotation[]>
  // which annotation is currently open in the panel
  activeId: string | null

  loadForFile:  (filePath: string) => Promise<void>
  create:       (input: CreateAnnotationInput) => Promise<Annotation>
  /** Write PA response text without resolving — annotation stays 'open'. */
  setResponse:  (id: string, response: string) => Promise<void>
  /** Link a fleet task_id to an annotation after Delegate dispatch. */
  setTaskId:    (id: string, taskId: string) => Promise<void>
  /** Mark as resolved (optionally with a resolution note). */
  resolve:      (id: string, response?: string) => Promise<void>
  remove:       (id: string) => Promise<void>
  setActive:    (id: string | null) => void
  getForFile:   (filePath: string) => Annotation[]
}

export const useAnnotationStore = create<AnnotationStore>((set, get) => ({
  byFile: {},
  activeId: null,

  loadForFile: async (filePath: string) => {
    try {
      const annotations = await invoke<Annotation[]>('list_annotations_for_file', { filePath })
      set((s) => ({ byFile: { ...s.byFile, [filePath]: annotations } }))
    } catch (e) {
      console.error('[annotationStore] loadForFile failed:', e)
    }
  },

  create: async (input: CreateAnnotationInput) => {
    const id = await invoke<string>('create_annotation', { input })
    const annotation: Annotation = {
      id,
      filePath:     input.filePath,
      startLine:    input.startLine,
      endLine:      input.endLine,
      selectedText: input.selectedText,
      prompt:       input.prompt,
      source:       input.source ?? 'user',
      severity:     input.severity ?? 'info',
      status:       'open',
      taskId:       input.taskId,
      createdAt:    new Date().toISOString(),
      updatedAt:    new Date().toISOString(),
    }
    set((s) => {
      const existing = s.byFile[input.filePath] ?? []
      return { byFile: { ...s.byFile, [input.filePath]: [...existing, annotation] } }
    })
    return annotation
  },

  setResponse: async (id: string, response: string) => {
    await invoke('update_annotation_response', { id, response })
    set((s) => {
      const updated: Record<string, Annotation[]> = {}
      for (const [path, list] of Object.entries(s.byFile)) {
        updated[path] = list.map((a) =>
          a.id === id ? { ...a, response } : a
        )
      }
      return { byFile: updated }
    })
  },

  setTaskId: async (id: string, taskId: string) => {
    await invoke('update_annotation_task_id', { id, taskId })
    set((s) => {
      const updated: Record<string, Annotation[]> = {}
      for (const [path, list] of Object.entries(s.byFile)) {
        updated[path] = list.map((a) =>
          a.id === id ? { ...a, taskId } : a
        )
      }
      return { byFile: updated }
    })
  },

  resolve: async (id: string, response?: string) => {
    await invoke('resolve_annotation', { id, response: response ?? null })
    set((s) => {
      const updated: Record<string, Annotation[]> = {}
      for (const [path, list] of Object.entries(s.byFile)) {
        updated[path] = list.map((a) =>
          a.id === id ? { ...a, status: 'resolved' as const, response, updatedAt: new Date().toISOString() } : a
        )
      }
      return { byFile: updated }
    })
  },

  remove: async (id: string) => {
    await invoke('delete_annotation', { id })
    set((s) => {
      const updated: Record<string, Annotation[]> = {}
      for (const [path, list] of Object.entries(s.byFile)) {
        updated[path] = list.filter((a) => a.id !== id)
      }
      return { byFile: updated, activeId: s.activeId === id ? null : s.activeId }
    })
  },

  setActive: (id) => set({ activeId: id }),

  getForFile: (filePath) => get().byFile[filePath] ?? [],
}))
