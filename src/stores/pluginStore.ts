// Plugin store — Zustand store for the AIRS Plugin Marketplace.
// Phase 5.2: Plugin System & Worker Setup.
// See: DEVELOPMENT_PLAN.md Phase 5.2 — AIRS Plugin System

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

// ─── Types (mirror Rust plugin manifest types) ────────────────────────────────

export interface PluginInfo {
  name:        string
  version:     string
  description: string
  author:      string
  plugin_type: string
  os_support:  string[]
  tags:        string[]
  icon:        string
  installed:   boolean
  builtin:     boolean
}

export interface PluginManifest extends PluginInfo {
  dependencies: PluginDependency[]
  commands: {
    install?:   { linux?: string; macos?: string; windows?: string }
    verify?:    string
    uninstall?: string
    run?:       string
  }
  config?: { schema?: string; defaults?: string }
}

export interface PluginDependency {
  name:        string
  version:     string
  check_cmd?:  string
  install_cmd?: string
  optional:    boolean
}

export interface DependencyCheck {
  name:    string
  required: boolean
  present: boolean
  version?: string
  message: string
}

export interface VerifyResult {
  success:  boolean
  plugin:   string
  checks:   DependencyCheck[]
  message:  string
}

export interface InstallResult {
  success:  boolean
  plugin:   string
  version:  string
  message:  string
  warnings: string[]
}

export interface UpdateResult {
  success:      boolean
  plugin:       string
  from_version: string
  to_version:   string
  message:      string
}

export interface RunResult {
  success:   boolean
  stdout:    string
  stderr:    string
  exit_code: number
}

export interface SetupToken {
  token:       string
  worker_type: string
  expires_at:  string
  one_liner:   string
}

export interface TokenClaims {
  fleet_id:    string
  worker_type: string
  repos:       string[]
  expires_at:  string
  issued_at:   string
}

// ─── Store state ──────────────────────────────────────────────────────────────

export type PluginFilter = 'all' | 'installed' | 'builtin' | string

interface PluginState {
  plugins:       PluginInfo[]
  loadingPlugins: boolean
  pluginError:   string | null

  // Active plugin for detail / install flow
  activePlugin:  PluginInfo | null
  activeManifest: PluginManifest | null

  // Install / verify progress
  operationLoading: Record<string, boolean>
  lastInstallResult: InstallResult | null
  lastVerifyResult:  VerifyResult | null

  // Setup token UI
  setupToken:    SetupToken | null
  tokenClaims:   TokenClaims | null

  // Filter state
  filter: PluginFilter
  tagFilter: string
}

interface PluginActions {
  loadPlugins:          () => Promise<void>
  setActivePlugin:      (plugin: PluginInfo | null) => void
  loadActiveManifest:   (name: string) => Promise<void>

  installPlugin:        (name: string) => Promise<InstallResult>
  installFromYaml:      (yaml: string) => Promise<InstallResult>
  uninstallPlugin:      (name: string) => Promise<boolean>
  updatePlugin:         (name: string) => Promise<UpdateResult>
  verifyPlugin:         (name: string) => Promise<VerifyResult>
  runPlugin:            (name: string, args: string[]) => Promise<RunResult>

  generateSetupToken:   (fleetId: string, workerType: string, repos: string[], ttlHours?: number) => Promise<SetupToken>
  validateSetupToken:   (token: string) => Promise<TokenClaims>
  clearSetupToken:      () => void

  setFilter:            (filter: PluginFilter) => void
  setTagFilter:         (tag: string) => void
  clearError:           () => void
}

// ─── Store ───────────────────────────────────────────────────────────────────

export const usePluginStore = create<PluginState & PluginActions>((set, get) => ({
  plugins:            [],
  loadingPlugins:     false,
  pluginError:        null,
  activePlugin:       null,
  activeManifest:     null,
  operationLoading:   {},
  lastInstallResult:  null,
  lastVerifyResult:   null,
  setupToken:         null,
  tokenClaims:        null,
  filter:             'all',
  tagFilter:          '',

  // ── Load marketplace ──────────────────────────────────────────────────────

  loadPlugins: async () => {
    set({ loadingPlugins: true, pluginError: null })
    try {
      const plugins = await invoke<PluginInfo[]>('list_plugins')
      set({ plugins, loadingPlugins: false })
    } catch (e: any) {
      set({ pluginError: String(e), loadingPlugins: false })
    }
  },

  setActivePlugin: (plugin) => set({ activePlugin: plugin, lastInstallResult: null, lastVerifyResult: null }),

  loadActiveManifest: async (name) => {
    try {
      const manifest = await invoke<PluginManifest | null>('get_plugin', { name })
      set({ activeManifest: manifest })
    } catch {
      set({ activeManifest: null })
    }
  },

  // ── Install ───────────────────────────────────────────────────────────────

  installPlugin: async (name) => {
    set(s => ({ operationLoading: { ...s.operationLoading, [name]: true } }))
    try {
      const result = await invoke<InstallResult>('install_plugin', { name })
      set(s => ({
        lastInstallResult: result,
        operationLoading: { ...s.operationLoading, [name]: false },
        // Refresh installed flag optimistically
        plugins: s.plugins.map(p => p.name === name ? { ...p, installed: result.success } : p),
      }))
      return result
    } catch (e: any) {
      set(s => ({ operationLoading: { ...s.operationLoading, [name]: false }, pluginError: String(e) }))
      return { success: false, plugin: name, version: '?', message: String(e), warnings: [] }
    }
  },

  installFromYaml: async (yaml) => {
    try {
      const result = await invoke<InstallResult>('install_plugin_from_yaml', { yaml })
      if (result.success) await get().loadPlugins()
      set({ lastInstallResult: result })
      return result
    } catch (e: any) {
      const result: InstallResult = { success: false, plugin: 'unknown', version: '?', message: String(e), warnings: [] }
      set({ lastInstallResult: result })
      return result
    }
  },

  uninstallPlugin: async (name) => {
    set(s => ({ operationLoading: { ...s.operationLoading, [name]: true } }))
    try {
      const ok = await invoke<boolean>('uninstall_plugin', { name })
      set(s => ({
        operationLoading: { ...s.operationLoading, [name]: false },
        plugins: s.plugins.map(p => p.name === name ? { ...p, installed: false } : p),
      }))
      return ok
    } catch (e: any) {
      set(s => ({ operationLoading: { ...s.operationLoading, [name]: false }, pluginError: String(e) }))
      return false
    }
  },

  updatePlugin: async (name) => {
    set(s => ({ operationLoading: { ...s.operationLoading, [name]: true } }))
    try {
      const result = await invoke<UpdateResult>('update_plugin', { name })
      set(s => ({ operationLoading: { ...s.operationLoading, [name]: false } }))
      if (result.success) await get().loadPlugins()
      return result
    } catch (e: any) {
      set(s => ({ operationLoading: { ...s.operationLoading, [name]: false } }))
      return { success: false, plugin: name, from_version: '?', to_version: '?', message: String(e) }
    }
  },

  verifyPlugin: async (name) => {
    set(s => ({ operationLoading: { ...s.operationLoading, [`verify_${name}`]: true } }))
    try {
      const result = await invoke<VerifyResult>('verify_plugin', { name })
      set(s => ({
        lastVerifyResult: result,
        operationLoading: { ...s.operationLoading, [`verify_${name}`]: false },
      }))
      return result
    } catch (e: any) {
      set(s => ({ operationLoading: { ...s.operationLoading, [`verify_${name}`]: false } }))
      return { success: false, plugin: name, checks: [], message: String(e) }
    }
  },

  runPlugin: async (name, args) => {
    const result = await invoke<RunResult>('run_plugin', { name, args })
    return result
  },

  // ── Setup tokens ──────────────────────────────────────────────────────────

  generateSetupToken: async (fleetId, workerType, repos, ttlHours) => {
    const token = await invoke<SetupToken>('generate_setup_token', {
      fleetId, workerType, repos, ttlHours: ttlHours ?? 24,
    })
    set({ setupToken: token })
    return token
  },

  validateSetupToken: async (token) => {
    const claims = await invoke<TokenClaims>('validate_setup_token', { token })
    set({ tokenClaims: claims })
    return claims
  },

  clearSetupToken: () => set({ setupToken: null, tokenClaims: null }),

  // ── Filters ───────────────────────────────────────────────────────────────

  setFilter:    (filter) => set({ filter }),
  setTagFilter: (tagFilter) => set({ tagFilter }),
  clearError:   () => set({ pluginError: null }),
}))

// ─── Selectors ────────────────────────────────────────────────────────────────

export const selectFilteredPlugins = (state: PluginState): PluginInfo[] => {
  return state.plugins.filter(p => {
    if (state.filter === 'installed' && !p.installed) return false
    if (state.filter === 'builtin' && !p.builtin) return false
    if (state.tagFilter && !p.tags.some(t => t.includes(state.tagFilter))) return false
    return true
  })
}
