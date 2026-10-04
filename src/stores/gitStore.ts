// Git state store — Phase 3.3.
//
// Holds the opened repository, current status snapshot, branches, log,
// stashes, and checkpoints. Actions invoke Tauri commands from the
// `git_commands` module (see src-tauri/src/commands/git_commands.rs).
//
// Mirror types are intentionally structural matches of the Rust DTOs in
// src-tauri/src/git/types.rs; keep them in sync whenever the backend
// types change.

import { invoke } from '@tauri-apps/api/core'
import { create } from 'zustand'

// ─── Types (mirror src-tauri/src/git/types.rs) ────────────────────────────────

export interface RepoInfo {
  path:         string
  workdir:      string
  is_bare:      boolean
  is_empty:     boolean
  head_branch:  string | null
  head_oid:     string | null
  remote_url:   string | null
}

export type FileStatus =
  | 'index_new'
  | 'index_modified'
  | 'index_deleted'
  | 'index_renamed'
  | 'worktree_new'
  | 'worktree_modified'
  | 'worktree_deleted'
  | 'conflicted'
  | 'ignored'
  | 'clean'

export interface StatusEntry {
  path:                 string
  status:               FileStatus
  has_worktree_changes: boolean
  old_path:             string | null
}

export interface GitStatusReport {
  head_branch: string | null
  ahead:       number
  behind:      number
  staged:      StatusEntry[]
  unstaged:    StatusEntry[]
  untracked:   StatusEntry[]
  conflicted:  StatusEntry[]
}

export interface DiffResult {
  patch:         string
  files_changed: number
  insertions:    number
  deletions:     number
}

export interface CommitInfo {
  oid:          string
  short_oid:    string
  summary:      string
  body:         string
  author_name:  string
  author_email: string
  time:         string
  parents:      string[]
}

export interface BranchInfo {
  name:        string
  is_remote:   boolean
  is_head:     boolean
  upstream:    string | null
  ahead:       number
  behind:      number
  last_commit: string | null
}

export interface WorktreeInfo {
  name:    string
  path:    string
  branch:  string | null
  is_main: boolean
  locked:  boolean
}

export interface StashInfo {
  index:   number
  oid:     string
  message: string
  time:    string
}

export interface CheckpointInfo {
  id:            string
  session_id:    string
  file_path:     string
  created_ms:    number
  reason:        string
  size_bytes:    number
  snapshot_path: string
}

// ─── Store ────────────────────────────────────────────────────────────────────

interface SelectedDiff {
  path:     string
  staged:   boolean
  result:   DiffResult | null
  loading:  boolean
  error:    string | null
}

export interface GitStore {
  repo:        RepoInfo | null
  status:      GitStatusReport | null
  branches:    BranchInfo[]
  log:         CommitInfo[]
  stashes:     StashInfo[]
  checkpoints: CheckpointInfo[]
  worktrees:   WorktreeInfo[]

  loading:     boolean
  error:       string | null

  // Selected diff for the viewer pane.
  selectedDiff: SelectedDiff | null

  // Actions
  openRepo:           (path: string) => Promise<void>
  refreshAll:         () => Promise<void>
  refreshStatus:      () => Promise<void>
  refreshBranches:    () => Promise<void>
  refreshLog:         (maxCount?: number) => Promise<void>
  refreshStashes:     () => Promise<void>
  refreshCheckpoints: (filePath?: string) => Promise<void>
  refreshWorktrees:   () => Promise<void>

  selectDiff:         (path: string, staged: boolean) => Promise<void>
  clearDiff:          () => void

  stage:              (files: string[]) => Promise<void>
  unstage:            (files: string[]) => Promise<void>
  discard:            (files: string[]) => Promise<void>
  commit:             (message: string, trailer?: string) => Promise<CommitInfo>

  push:               (branch?: string) => Promise<void>
  pull:               (branch?: string) => Promise<void>
  fetch:              () => Promise<void>

  createBranch:       (name: string, checkout?: boolean) => Promise<void>
  deleteBranch:       (name: string, force?: boolean) => Promise<void>
  checkoutBranch:     (refname: string) => Promise<void>

  stashPush:          (message?: string, includeUntracked?: boolean) => Promise<void>
  stashApply:         (index: number, pop?: boolean) => Promise<void>
  stashDrop:          (index: number) => Promise<void>

  restoreCheckpoint:  (id: string) => Promise<void>
  deleteCheckpoint:   (id: string) => Promise<void>
}

function repoPath(state: GitStore): string {
  if (!state.repo) throw new Error('No repository opened')
  return state.repo.workdir
}

export const useGitStore = create<GitStore>((set, get) => ({
  repo:        null,
  status:      null,
  branches:    [],
  log:         [],
  stashes:     [],
  checkpoints: [],
  worktrees:   [],
  loading:     false,
  error:       null,
  selectedDiff: null,

  openRepo: async (path) => {
    set({ loading: true, error: null })
    try {
      const repo = await invoke<RepoInfo>('git_open_repo', { path })
      set({ repo, loading: false })
      await get().refreshAll()
    } catch (e) {
      set({ repo: null, loading: false, error: String(e) })
    }
  },

  refreshAll: async () => {
    const state = get()
    if (!state.repo) return
    await Promise.all([
      state.refreshStatus(),
      state.refreshBranches(),
      state.refreshLog(50),
      state.refreshStashes(),
      state.refreshCheckpoints(),
      state.refreshWorktrees(),
    ])
  },

  refreshStatus: async () => {
    try {
      const report = await invoke<GitStatusReport>('git_status', { repoPath: repoPath(get()) })
      set({ status: report })
    } catch (e) { set({ error: String(e) }) }
  },

  refreshBranches: async () => {
    try {
      const branches = await invoke<BranchInfo[]>('git_branch_list', { repoPath: repoPath(get()) })
      set({ branches })
    } catch (e) { set({ error: String(e) }) }
  },

  refreshLog: async (maxCount = 100) => {
    try {
      const log = await invoke<CommitInfo[]>('git_log', { repoPath: repoPath(get()), maxCount, file: null })
      set({ log })
    } catch (e) { set({ error: String(e) }) }
  },

  refreshStashes: async () => {
    try {
      const stashes = await invoke<StashInfo[]>('git_stash_list', { repoPath: repoPath(get()) })
      set({ stashes })
    } catch (e) { set({ error: String(e) }) }
  },

  refreshCheckpoints: async (filePath) => {
    try {
      const checkpoints = await invoke<CheckpointInfo[]>('list_checkpoints', {
        filePath:  filePath ?? null,
        sessionId: null,
        taskId:    null,
      })
      set({ checkpoints })
    } catch (e) { set({ error: String(e) }) }
  },

  refreshWorktrees: async () => {
    try {
      const worktrees = await invoke<WorktreeInfo[]>('git_worktree_list', { repoPath: repoPath(get()) })
      set({ worktrees })
    } catch (e) { set({ error: String(e) }) }
  },

  selectDiff: async (path, staged) => {
    set({ selectedDiff: { path, staged, result: null, loading: true, error: null } })
    try {
      const result = await invoke<DiffResult>('git_diff', {
        repoPath: repoPath(get()),
        file:     path,
        staged,
      })
      set({ selectedDiff: { path, staged, result, loading: false, error: null } })
    } catch (e) {
      set({ selectedDiff: { path, staged, result: null, loading: false, error: String(e) } })
    }
  },

  clearDiff: () => set({ selectedDiff: null }),

  stage: async (files) => {
    await invoke('git_stage', { repoPath: repoPath(get()), files })
    await get().refreshStatus()
  },

  unstage: async (files) => {
    await invoke('git_unstage', { repoPath: repoPath(get()), files })
    await get().refreshStatus()
  },

  discard: async (files) => {
    await invoke('git_discard', { repoPath: repoPath(get()), files })
    await get().refreshStatus()
  },

  commit: async (message, trailer) => {
    const info = await invoke<CommitInfo>('git_commit', {
      repoPath:    repoPath(get()),
      message,
      authorName:  null,
      authorEmail: null,
      trailer:     trailer ?? null,
    })
    await Promise.all([get().refreshStatus(), get().refreshLog(50)])
    return info
  },

  push: async (branch) => {
    await invoke('git_push', { repoPath: repoPath(get()), remoteName: null, branch: branch ?? null })
    await get().refreshStatus()
  },

  pull: async (branch) => {
    await invoke('git_pull', { repoPath: repoPath(get()), remoteName: null, branch: branch ?? null })
    await get().refreshAll()
  },

  fetch: async () => {
    await invoke('git_fetch', { repoPath: repoPath(get()), remoteName: null })
    await get().refreshStatus()
  },

  createBranch: async (name, checkout) => {
    await invoke('git_branch_create', { repoPath: repoPath(get()), name, checkout: checkout ?? false })
    await get().refreshBranches()
    if (checkout) await get().refreshStatus()
  },

  deleteBranch: async (name, force) => {
    await invoke('git_branch_delete', { repoPath: repoPath(get()), name, force: force ?? false })
    await get().refreshBranches()
  },

  checkoutBranch: async (refname) => {
    await invoke('git_checkout', { repoPath: repoPath(get()), refname })
    await get().refreshAll()
  },

  stashPush: async (message, includeUntracked) => {
    await invoke('git_stash_push', {
      repoPath:          repoPath(get()),
      message:           message ?? null,
      includeUntracked:  includeUntracked ?? false,
    })
    await Promise.all([get().refreshStatus(), get().refreshStashes()])
  },

  stashApply: async (index, pop) => {
    await invoke('git_stash_apply', { repoPath: repoPath(get()), index, pop: pop ?? false })
    await Promise.all([get().refreshStatus(), get().refreshStashes()])
  },

  stashDrop: async (index) => {
    await invoke('git_stash_drop', { repoPath: repoPath(get()), index })
    await get().refreshStashes()
  },

  restoreCheckpoint: async (id) => {
    await invoke('restore_checkpoint', { checkpointId: id })
    await Promise.all([get().refreshStatus(), get().refreshCheckpoints()])
  },

  deleteCheckpoint: async (id) => {
    await invoke('delete_checkpoint', { checkpointId: id })
    await get().refreshCheckpoints()
  },
}))
