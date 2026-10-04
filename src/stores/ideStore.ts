// IDE store — open tabs, active tab, file tree cache, expansion state.
//
// State machine (see DEVELOPMENT_PLAN.md §3.2):
//   mounted → load workspace state → fetch root tree
//   openFile(path) → fetch content → push tab → persist
//   closeTab(path) → splice → persist
//   switchTab(path) → set active → persist
//
// All mutations that affect persistence call `persist()`; the backend
// `set_ide_workspace_state` command is debounced by relying on React's
// batched state updates plus an explicit save on relevant actions.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

// ─── Backend types (mirror src-tauri/src/ide/types.rs) ────────────────────────

export type GitStatus =
  | 'staged'
  | 'modified'
  | 'untracked'
  | 'deleted'
  | 'ignored'
  | 'conflicted'
  | 'clean'

export interface FileTreeNode {
  name:           string
  relative_path:  string
  absolute_path:  string
  is_directory:   boolean
  size_bytes:     number | null
  modified_ms:    number | null
  git_status:     GitStatus | null
  children:       FileTreeNode[]
  has_more:       boolean
}

export interface FileContent {
  absolute_path: string
  relative_path: string
  content:       string
  encoding:      string
  is_binary:     boolean
  line_count:    number
  size_bytes:    number
  start_line:    number | null
  end_line:      number | null
  language:      string
}

export interface FileMetadata {
  absolute_path: string
  relative_path: string
  size_bytes:    number
  modified_ms:   number | null
  is_directory:  boolean
  is_readonly:   boolean
  git_status:    GitStatus | null
}

export interface FileSearchResult {
  name:          string
  relative_path: string
  absolute_path: string
  is_directory:  boolean
  score:         number
}

export interface ContentMatch {
  line_number: number
  line_text:   string
  match_start: number
  match_end:   number
}

export interface ContentSearchResult {
  relative_path: string
  absolute_path: string
  matches:       ContentMatch[]
}

export type SymbolKind =
  | 'function' | 'method' | 'class' | 'struct' | 'enum'
  | 'interface' | 'trait' | 'module' | 'constant' | 'variable' | 'type'

export interface Symbol {
  name:     string
  kind:     SymbolKind
  line:     number
  column:   number
  children: Symbol[]
}

export interface SymbolTree {
  relative_path: string
  language:      string
  symbols:       Symbol[]
}

interface IdeWorkspaceState {
  open_tabs:     string[]
  active_tab:    string | null
  expanded_dirs: string[]
}

// ─── Store ────────────────────────────────────────────────────────────────────

export interface OpenTab {
  relativePath: string
  content:      FileContent | null
  loading:      boolean
  error:        string | null
  /// Monaco sometimes requests a jump to a specific line after opening.
  gotoLine:     number | null
}

export interface IdeStore {
  // Tree
  rootTree:        FileTreeNode | null
  expandedDirs:    Set<string>
  treeLoading:     boolean
  treeError:       string | null

  // Tabs
  openTabs:        OpenTab[]
  activeTab:       string | null

  // Search overlay
  searchOpen:      boolean

  // Recent files (most-recent first, capped at 20)
  recentFiles:     string[]

  // Actions
  init:               () => Promise<void>
  refreshTree:        () => Promise<void>
  loadDirChildren:    (relativePath: string) => Promise<void>
  toggleDir:          (relativePath: string) => Promise<void>
  openFile:           (relativePath: string, gotoLine?: number) => Promise<void>
  revealFile:         (relativePath: string, gotoLine?: number) => Promise<void>
  closeTab:           (relativePath: string) => void
  closeOtherTabs:     (relativePath: string) => void
  closeAllTabs:       () => void
  switchTab:          (relativePath: string) => void
  setSearchOpen:      (open: boolean) => void
  /** Reorder tabs by moving the tab at `fromIdx` to `toIdx`. */
  reorderTabs:        (fromIdx: number, toIdx: number) => void
  // File mutations — each refreshes the tree after success
  createFile:         (relativePath: string, content?: string) => Promise<void>
  createDirectory:    (relativePath: string) => Promise<void>
  deleteFile:         (relativePath: string) => Promise<void>
  renameFile:         (oldPath: string, newPath: string) => Promise<void>
  moveFile:           (fromPath: string, toDir: string) => Promise<void>
}

const RECENT_LIMIT = 20

export const useIdeStore = create<IdeStore>((set, get) => ({
  rootTree:      null,
  expandedDirs:  new Set<string>(),
  treeLoading:   false,
  treeError:     null,

  openTabs:      [],
  activeTab:     null,

  searchOpen:    false,
  recentFiles:   [],

  init: async () => {
    await get().refreshTree()

    try {
      const st = await invoke<IdeWorkspaceState>('get_ide_workspace_state')
      set({
        expandedDirs: new Set(st.expanded_dirs ?? []),
      })

      for (const relPath of st.open_tabs ?? []) {
        await get().openFile(relPath)
      }
      if (st.active_tab) {
        set({ activeTab: st.active_tab })
      }

      // Lazy-load any directories that were expanded at restore time.
      for (const dir of st.expanded_dirs ?? []) {
        await get().loadDirChildren(dir)
      }
    } catch (e) {
      console.error('ide init: failed to restore workspace state', e)
    }
  },

  refreshTree: async () => {
    set({ treeLoading: true, treeError: null })
    try {
      const root = await invoke<FileTreeNode>('read_file_tree', {
        path:  null,
        depth: 1,
      })
      set({ rootTree: root, treeLoading: false })
    } catch (e) {
      set({ treeError: String(e), treeLoading: false })
    }
  },

  loadDirChildren: async (relativePath: string) => {
    try {
      const node = await invoke<FileTreeNode>('read_file_tree', {
        path:  relativePath,
        depth: 1,
      })
      set((state) => ({
        rootTree: state.rootTree
          ? replaceNode(state.rootTree, relativePath, node)
          : state.rootTree,
      }))
    } catch (e) {
      console.error(`loadDirChildren(${relativePath})`, e)
    }
  },

  toggleDir: async (relativePath: string) => {
    const s = get()
    const next = new Set(s.expandedDirs)
    if (next.has(relativePath)) {
      next.delete(relativePath)
    } else {
      next.add(relativePath)
      await get().loadDirChildren(relativePath)
    }
    set({ expandedDirs: next })
    void persist(get())
  },

  openFile: async (relativePath: string, gotoLine) => {
    const s = get()
    const existing = s.openTabs.find((t) => t.relativePath === relativePath)
    if (existing) {
      set({
        activeTab: relativePath,
        openTabs:  s.openTabs.map((t) =>
          t.relativePath === relativePath
            ? { ...t, gotoLine: gotoLine ?? null }
            : t,
        ),
      })
      bumpRecent(set, get, relativePath)
      void persist(get())
      return
    }

    // Optimistic insert → loading placeholder.
    const placeholder: OpenTab = {
      relativePath,
      content:  null,
      loading:  true,
      error:    null,
      gotoLine: gotoLine ?? null,
    }
    set({
      openTabs:  [...s.openTabs, placeholder],
      activeTab: relativePath,
    })

    try {
      // Add timeout to prevent indefinite loading (10 seconds)
      const timeoutMs = 10000
      const content = await Promise.race([
        invoke<FileContent>('read_file_content', {
          path:       relativePath,
          startLine:  null,
          endLine:    null,
        }),
        new Promise<never>((_, reject) =>
          setTimeout(() => reject(new Error(`Timeout loading file after ${timeoutMs}ms`)), timeoutMs)
        ),
      ])
      set((state) => ({
        openTabs: state.openTabs.map((t) =>
          t.relativePath === relativePath
            ? { ...t, content, loading: false }
            : t,
        ),
      }))
      bumpRecent(set, get, relativePath)
      void persist(get())
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e)
      console.error(`Failed to load file ${relativePath}:`, errorMsg)
      set((state) => ({
        openTabs: state.openTabs.map((t) =>
          t.relativePath === relativePath
            ? { ...t, loading: false, error: errorMsg }
            : t,
        ),
      }))
    }
  },

  revealFile: async (relativePath: string, gotoLine) => {
    const segments = relativePath.split('/').filter(Boolean)
    if (segments.length > 1) {
      if (!get().rootTree) await get().refreshTree()
      const expanded = new Set(get().expandedDirs)
      let directory = ''
      for (const segment of segments.slice(0, -1)) {
        directory = directory ? `${directory}/${segment}` : segment
        expanded.add(directory)
        await get().loadDirChildren(directory)
      }
      set({ expandedDirs: expanded })
      void persist(get())
    }
    await get().openFile(relativePath, gotoLine)
  },

  closeTab: (relativePath: string) => {
    const s = get()
    const remaining = s.openTabs.filter((t) => t.relativePath !== relativePath)
    let activeTab = s.activeTab
    if (activeTab === relativePath) {
      activeTab = remaining.length > 0 ? remaining[remaining.length - 1].relativePath : null
    }
    set({ openTabs: remaining, activeTab })
    void persist(get())
  },

  closeOtherTabs: (relativePath: string) => {
    set((state) => ({
      openTabs:  state.openTabs.filter((t) => t.relativePath === relativePath),
      activeTab: relativePath,
    }))
    void persist(get())
  },

  closeAllTabs: () => {
    set({ openTabs: [], activeTab: null })
    void persist(get())
  },

  switchTab: (relativePath: string) => {
    set({ activeTab: relativePath })
    void persist(get())
  },

  setSearchOpen: (open: boolean) => set({ searchOpen: open }),

  reorderTabs: (fromIdx: number, toIdx: number) => {
    set((state) => {
      const tabs = [...state.openTabs]
      const [moved] = tabs.splice(fromIdx, 1)
      tabs.splice(toIdx, 0, moved)
      return { openTabs: tabs }
    })
    void persist(get())
  },

  // ── File mutations ────────────────────────────────────────────────────────────

  createFile: async (relativePath: string, content?: string) => {
    await invoke('create_file', { path: relativePath, content: content ?? null })
    await get().refreshTree()
  },

  createDirectory: async (relativePath: string) => {
    await invoke('create_directory', { path: relativePath })
    await get().refreshTree()
  },

  deleteFile: async (relativePath: string) => {
    await invoke('delete_file', { path: relativePath })
    // Close the tab if it was open
    const s = get()
    if (s.openTabs.some(t => t.relativePath === relativePath)) {
      get().closeTab(relativePath)
    }
    await get().refreshTree()
  },

  renameFile: async (oldPath: string, newPath: string) => {
    await invoke('rename_file', { oldPath, newPath })
    const s = get()
    if (s.openTabs.some(t => t.relativePath === oldPath)) {
      get().closeTab(oldPath)
    }
    await get().refreshTree()
    // Re-open at new path if it was open before
    if (s.openTabs.some(t => t.relativePath === oldPath)) {
      await get().openFile(newPath)
    }
  },

  moveFile: async (fromPath: string, toDir: string) => {
    await invoke('move_file', { fromPath, toDir })
    const s = get()
    if (s.openTabs.some(t => t.relativePath === fromPath)) {
      get().closeTab(fromPath)
    }
    await get().refreshTree()
  },
}))

// ─── Helpers ──────────────────────────────────────────────────────────────────

function replaceNode(
  node:       FileTreeNode,
  targetPath: string,
  replacement: FileTreeNode,
): FileTreeNode {
  if (node.relative_path === targetPath) {
    return replacement
  }
  if (!node.is_directory || node.children.length === 0) {
    return node
  }
  return {
    ...node,
    children: node.children.map((c) => replaceNode(c, targetPath, replacement)),
  }
}

function bumpRecent(
  set: (updater: (s: IdeStore) => Partial<IdeStore>) => void,
  get: () => IdeStore,
  path: string,
) {
  const current = get().recentFiles.filter((p) => p !== path)
  set(() => ({ recentFiles: [path, ...current].slice(0, RECENT_LIMIT) }))
}

async function persist(s: IdeStore) {
  try {
    await invoke('set_ide_workspace_state', {
      state: {
        open_tabs:     s.openTabs.map((t) => t.relativePath),
        active_tab:    s.activeTab,
        expanded_dirs: Array.from(s.expandedDirs),
      } satisfies IdeWorkspaceState,
    })
  } catch (e) {
    console.error('persist ide state failed', e)
  }
}
