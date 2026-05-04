// Auto-updater store — Zustand store for the Phase 5.3 update system.
// Listens to Tauri events from the Rust updater backend and exposes
// state + actions for UpdateToast, UpdateDialog, and UpdateSettings.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

// ─── Types (mirror Rust structs) ──────────────────────────────────────────────

export interface PlatformAsset {
  full_url:    string
  delta_url:   string
  full_size:   number
  delta_size:  number
  signature:   string
  checksum:    string
}

export interface UpdateInfo {
  version:       string
  release_date:  string
  release_notes: string
  critical:      boolean
  channel:       string
  platforms:     Record<string, PlatformAsset>
}

export type DownloadStage = 'Pending' | 'Downloading' | 'Verifying' | 'Ready' | 'Failed'

export interface DownloadProgress {
  version:          string
  bytes_downloaded: number
  total_bytes:      number
  percent:          number
  speed_bps:        number
  stage:            DownloadStage
  path:             string | null
}

export interface InstallResult {
  success:          boolean
  version:          string
  message:          string
  requires_restart: boolean
}

export interface UpdateRecord {
  version:          string
  installed_at:     string
  previous_version: string
  channel:          string
}

export interface DataCheck {
  name:    string
  path:    string
  present: boolean
  size:    number
  message: string
}

export interface IntegrityReport {
  all_ok:           boolean
  checks:           DataCheck[]
  total_size_bytes: number
  message:          string
}

// ─── Store ────────────────────────────────────────────────────────────────────

interface UpdateState {
  // Update availability
  availableUpdate:  UpdateInfo | null
  isCritical:       boolean
  isToastVisible:   boolean
  isDialogOpen:     boolean

  // Download progress
  downloadProgress: DownloadProgress | null
  isDownloading:    boolean
  downloadError:    string | null

  // Install state
  isInstalling:     boolean
  installResult:    InstallResult | null
  installError:     string | null

  // Settings / history
  updateHistory:    UpdateRecord[]
  integrityReport:  IntegrityReport | null
  isChecking:       boolean
  lastChecked:      Date | null
  checkError:       string | null

  // Event listeners (internal)
  _unlisteners:     UnlistenFn[]
}

interface UpdateActions {
  // Initialise — call once on app mount
  init:               () => Promise<void>
  cleanup:            () => void

  // Update check
  checkForUpdate:     (channel?: string) => Promise<UpdateInfo | null>
  dismissToast:       () => void
  openDialog:         () => void
  closeDialog:        () => void

  // Download
  startDownload:      (version: string) => Promise<void>
  pollProgress:       () => Promise<void>

  // Install
  installUpdate:      (version: string) => Promise<void>
  skipVersion:        (version: string) => Promise<void>

  // History / integrity
  loadHistory:        () => Promise<void>
  verifyIntegrity:    () => Promise<void>
}

export const useUpdateStore = create<UpdateState & UpdateActions>((set, get) => ({
  // ── Initial state ──────────────────────────────────────────────────────────
  availableUpdate:  null,
  isCritical:       false,
  isToastVisible:   false,
  isDialogOpen:     false,

  downloadProgress: null,
  isDownloading:    false,
  downloadError:    null,

  isInstalling:     false,
  installResult:    null,
  installError:     null,

  updateHistory:    [],
  integrityReport:  null,
  isChecking:       false,
  lastChecked:      null,
  checkError:       null,

  _unlisteners:     [],

  // ── Init — subscribe to Tauri events ──────────────────────────────────────
  init: async () => {
    const unlisteners: UnlistenFn[] = []

    // Update available (from background check)
    const ulAvailable = await listen<UpdateInfo>('updater:available', (event) => {
      set({
        availableUpdate: event.payload,
        isCritical:      false,
        isToastVisible:  true,
      })
    })
    unlisteners.push(ulAvailable)

    // Critical update
    const ulCritical = await listen<UpdateInfo>('updater:critical', (event) => {
      set({
        availableUpdate: event.payload,
        isCritical:      true,
        isToastVisible:  true,
        isDialogOpen:    true,   // critical updates open dialog automatically
      })
    })
    unlisteners.push(ulCritical)

    // Download progress
    const ulProgress = await listen<DownloadProgress>('updater:progress', (event) => {
      set({ downloadProgress: event.payload })
    })
    unlisteners.push(ulProgress)

    // Download ready
    const ulReady = await listen<string>('updater:ready', (_event) => {
      set({ isDownloading: false })
    })
    unlisteners.push(ulReady)

    // Download/verify error
    const ulError = await listen<string>('updater:error', (event) => {
      set({
        isDownloading: false,
        downloadError: event.payload,
      })
    })
    unlisteners.push(ulError)

    set({ _unlisteners: unlisteners })

    // Restore cached update from last background check
    try {
      const cached = await invoke<UpdateInfo | null>('get_cached_update')
      if (cached) {
        set({
          availableUpdate: cached,
          isCritical:      cached.critical,
          isToastVisible:  true,
        })
      }
    } catch {
      // Ignore — no cached update
    }
  },

  cleanup: () => {
    get()._unlisteners.forEach(fn => fn())
    set({ _unlisteners: [] })
  },

  // ── Check ─────────────────────────────────────────────────────────────────
  checkForUpdate: async (channel?: string) => {
    set({ isChecking: true, checkError: null })
    try {
      const info = await invoke<UpdateInfo | null>('check_for_update', { channel })
      set({
        availableUpdate: info,
        isCritical:      info?.critical ?? false,
        isToastVisible:  info !== null,
        lastChecked:     new Date(),
        isChecking:      false,
      })
      return info
    } catch (err) {
      set({ checkError: String(err), isChecking: false, lastChecked: new Date() })
      return null
    }
  },

  dismissToast: () => set({ isToastVisible: false }),
  openDialog:   () => set({ isDialogOpen: true }),
  closeDialog:  () => set({ isDialogOpen: false }),

  // ── Download ──────────────────────────────────────────────────────────────
  startDownload: async (version: string) => {
    set({ isDownloading: true, downloadError: null, downloadProgress: null })
    try {
      const initial = await invoke<DownloadProgress>('download_update', { version })
      set({ downloadProgress: initial })
    } catch (err) {
      set({ isDownloading: false, downloadError: String(err) })
    }
  },

  pollProgress: async () => {
    try {
      const progress = await invoke<DownloadProgress | null>('get_download_progress')
      if (progress) {
        set({ downloadProgress: progress })
        if (progress.stage === 'Ready' || progress.stage === 'Failed') {
          set({ isDownloading: false })
        }
      }
    } catch {
      // Ignore
    }
  },

  // ── Install ───────────────────────────────────────────────────────────────
  installUpdate: async (version: string) => {
    set({ isInstalling: true, installError: null })
    try {
      const result = await invoke<InstallResult>('install_update', { version })
      set({ isInstalling: false, installResult: result })
    } catch (err) {
      set({ isInstalling: false, installError: String(err) })
    }
  },

  skipVersion: async (version: string) => {
    try {
      await invoke<void>('skip_version', { version })
      set({ availableUpdate: null, isToastVisible: false, isDialogOpen: false })
    } catch (err) {
      console.error('[updateStore] skip_version failed:', err)
    }
  },

  // ── History / Integrity ───────────────────────────────────────────────────
  loadHistory: async () => {
    try {
      const history = await invoke<UpdateRecord[]>('get_update_history')
      set({ updateHistory: history })
    } catch (err) {
      console.error('[updateStore] get_update_history failed:', err)
    }
  },

  verifyIntegrity: async () => {
    try {
      const report = await invoke<IntegrityReport>('verify_data_integrity')
      set({ integrityReport: report })
    } catch (err) {
      console.error('[updateStore] verify_data_integrity failed:', err)
    }
  },
}))
