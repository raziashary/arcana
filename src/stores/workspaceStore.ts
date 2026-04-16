// Workspace directory store — holds the root folder for all AutoDev agents.
// The workspace dir is used as:
//   - working_dir for all PA chat sessions (persists per session)
//   - working_dir for all worker recipe runs (RecipeRunner)
//   - base for @-mention path resolution in chat input
//
// Stored in ~/.autodev/config.db on the Rust side.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export interface WorkspaceStore {
  workspaceDir: string
  isLoading:    boolean
  isSaving:     boolean
  error:        string | null

  loadWorkspaceDir:  () => Promise<void>
  setWorkspaceDir:   (path: string) => Promise<void>
  pickWorkspaceDir:  () => Promise<void>
  clearWorkspaceDir: () => Promise<void>
}

export const useWorkspaceStore = create<WorkspaceStore>((set, get) => ({
  workspaceDir: '',
  isLoading:    false,
  isSaving:     false,
  error:        null,

  loadWorkspaceDir: async () => {
    set({ isLoading: true, error: null })
    try {
      const dir = await invoke<string>('get_workspace_dir')
      set({ workspaceDir: dir, isLoading: false })
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  setWorkspaceDir: async (path: string) => {
    set({ isSaving: true, error: null })
    try {
      await invoke('set_workspace_dir', { path })
      set({ workspaceDir: path, isSaving: false })
    } catch (e) {
      set({ error: String(e), isSaving: false })
    }
  },

  pickWorkspaceDir: async () => {
    set({ error: null })
    try {
      const picked = await invoke<string | null>('pick_workspace_dir')
      if (picked) {
        await get().setWorkspaceDir(picked)
      }
    } catch (e) {
      set({ error: String(e) })
    }
  },

  clearWorkspaceDir: async () => {
    await get().setWorkspaceDir('')
  },
}))
