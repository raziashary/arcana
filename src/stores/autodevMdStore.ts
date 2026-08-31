// AUTODEV.md store — manages per-scope (global / repo) instruction content.
// Storage backend is SQLite via Tauri commands (not filesystem).
// Two scopes: 'global' applies to all sessions, 'repo' applies to active cwd.
// See: DEVELOPMENT_PLAN.md Phase 2.1.2 — AUTODEV.md System

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export type AutodevScope = 'global' | 'repo'

const MAX_CHARS = 40_000

interface AutodevMdState {
  globalContent:   string
  repoContents:    Record<string, string>   // repoPath → content
  charCounts:      Record<string, number>   // 'global' | repoPath → count
  modifiedScopes:  Set<string>              // 'global' | repoPath
  isLoading:       boolean
  isSaving:        boolean
  error:           string | null

  loadGlobal:        ()                                          => Promise<void>
  loadRepo:          (repoPath: string)                          => Promise<void>
  saveGlobal:        (content: string)                          => Promise<void>
  saveRepo:          (repoPath: string, content: string)        => Promise<void>
  deleteRepo:        (repoPath: string)                         => Promise<void>
  setModified:       (scope: string, isModified: boolean)       => void
  getContent:        (scope: AutodevScope, repoPath?: string)   => string
  getCharCount:      (scope: AutodevScope, repoPath?: string)   => number
  isOverLimit:       (scope: AutodevScope, repoPath?: string)   => boolean
}

export const useAutodevMdStore = create<AutodevMdState>((set, get) => ({
  globalContent:  '',
  repoContents:   {},
  charCounts:     {},
  modifiedScopes: new Set(),
  isLoading:      false,
  isSaving:       false,
  error:          null,

  loadGlobal: async () => {
    set({ isLoading: true, error: null })
    try {
      const content = await invoke<string>('get_autodev_instructions', {
        scope:    'global',
        repoPath: null,
      })
      const count = content.length
      set(s => ({
        globalContent: content,
        charCounts: { ...s.charCounts, global: count },
      }))
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isLoading: false })
    }
  },

  loadRepo: async (repoPath) => {
    set({ isLoading: true, error: null })
    try {
      const content = await invoke<string>('get_autodev_instructions', {
        scope:    'repo',
        repoPath,
      })
      const count = content.length
      set(s => ({
        repoContents: { ...s.repoContents, [repoPath]: content },
        charCounts:   { ...s.charCounts, [repoPath]: count },
      }))
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isLoading: false })
    }
  },

  saveGlobal: async (content) => {
    if (content.length > MAX_CHARS) {
      set({ error: `Content exceeds ${MAX_CHARS.toLocaleString()} character limit` })
      return
    }
    set({ isSaving: true, error: null })
    try {
      await invoke('set_autodev_instructions', {
        scope:    'global',
        repoPath: null,
        content,
      })
      set(s => ({
        globalContent:  content,
        charCounts:     { ...s.charCounts, global: content.length },
        modifiedScopes: new Set([...s.modifiedScopes].filter(x => x !== 'global')),
      }))
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isSaving: false })
    }
  },

  saveRepo: async (repoPath, content) => {
    if (content.length > MAX_CHARS) {
      set({ error: `Content exceeds ${MAX_CHARS.toLocaleString()} character limit` })
      return
    }
    set({ isSaving: true, error: null })
    try {
      await invoke('set_autodev_instructions', {
        scope: 'repo',
        repoPath,
        content,
      })
      set(s => ({
        repoContents:   { ...s.repoContents, [repoPath]: content },
        charCounts:     { ...s.charCounts, [repoPath]: content.length },
        modifiedScopes: new Set([...s.modifiedScopes].filter(x => x !== repoPath)),
      }))
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isSaving: false })
    }
  },

  deleteRepo: async (repoPath) => {
    set({ isSaving: true, error: null })
    try {
      await invoke('delete_autodev_instructions', { repoPath })
      set(s => {
        const next = { ...s.repoContents }
        delete next[repoPath]
        const counts = { ...s.charCounts }
        delete counts[repoPath]
        return { repoContents: next, charCounts: counts }
      })
    } catch (e) {
      set({ error: String(e) })
    } finally {
      set({ isSaving: false })
    }
  },

  setModified: (scope, isModified) => {
    set(s => {
      const next = new Set(s.modifiedScopes)
      if (isModified) { next.add(scope) } else { next.delete(scope) }
      return { modifiedScopes: next }
    })
  },

  getContent: (scope, repoPath) => {
    const s = get()
    if (scope === 'global') return s.globalContent
    return repoPath ? (s.repoContents[repoPath] ?? '') : ''
  },

  getCharCount: (scope, repoPath) => {
    const s = get()
    const key = scope === 'global' ? 'global' : (repoPath ?? '')
    return s.charCounts[key] ?? 0
  },

  isOverLimit: (scope, repoPath) => {
    return get().getCharCount(scope, repoPath) > MAX_CHARS
  },
}))

export const MAX_AUTODEV_CHARS = MAX_CHARS
