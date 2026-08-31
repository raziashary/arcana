// Settings store — provider config, active model, theme, PA mode.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export type PAMode = 'fast_development' | 'review' | 'delegate'

export const PA_MODES: { value: PAMode; label: string; description: string; icon: string }[] = [
  {
    value:       'fast_development',
    label:       'Fast Development',
    description: 'Full file + execution tools. Direct coding.',
    icon:        '⚡',
  },
  {
    value:       'review',
    label:       'Review Mode',
    description: 'Same tools, explicit reasoning before each write.',
    icon:        '🔍',
  },
  {
    value:       'delegate',
    label:       'Delegate Mode',
    description: 'Only create_task. Workers are live.',
    icon:        '📋',
  },
]

export interface ProviderInfo {
  id:      string
  name:    string
  status:  'connected' | 'disconnected' | 'testing' | string
  models:  ModelInfo[]
  has_key: boolean
}

export interface ModelInfo {
  id:               string
  name:             string
  context_window:   number
  supports_vision:  boolean
  cost_in_per_1m:   number
  cost_out_per_1m:  number
}

interface SettingsState {
  providers:       ProviderInfo[]
  activeProvider:  string | null
  activeModel:     string | null
  isLoadingProviders: boolean
  systemPrompt:    string | null  // null = use default
  defaultSystemPrompt: string   // for reference display
  paMode:          PAMode         // PA mode toggle — persisted to localStorage

  loadProviders:         () => Promise<void>
  setActiveProvider:     (provider: string, model: string) => Promise<void>
  testProvider:          (provider: string) => Promise<string>
  storeProviderKey:      (provider: string, key: string) => Promise<void>
  deleteProviderKey:     (provider: string) => Promise<void>
  loadSystemPrompt:      () => Promise<void>
  setSystemPrompt:       (prompt: string | null) => Promise<void>
  resetSystemPrompt:     () => Promise<void>
  loadDefaultSystemPrompt: () => Promise<void>
  setPAMode:             (mode: PAMode) => void
}

const storedPAMode = (localStorage.getItem('autodev:pa_mode') ?? 'fast_development') as PAMode

export const useSettingsStore = create<SettingsState>((set, get) => ({
  providers:          [],
  activeProvider:     null,
  activeModel:        null,
  isLoadingProviders: false,
  systemPrompt:       null,
  defaultSystemPrompt: '',  // Loaded from Rust backend
  paMode:             storedPAMode,

  loadProviders: async () => {
    set({ isLoadingProviders: true })
    try {
      const providers = await invoke<ProviderInfo[]>('list_providers')
      set({ providers })

      const active = await invoke<[string, string] | null>('get_active_provider')
      if (active) {
        set({ activeProvider: active[0], activeModel: active[1] })
      }
    } catch (e) {
      console.error('Failed to load providers:', e)
    } finally {
      set({ isLoadingProviders: false })
    }
  },

  setActiveProvider: async (provider, model) => {
    await invoke('set_active_provider', { provider, model })
    set({ activeProvider: provider, activeModel: model })
  },

  testProvider: async (provider) => {
    const status = await invoke<string>('test_provider_connection', { provider })
    set(s => ({
      providers: s.providers.map(p =>
        p.id === provider ? { ...p, status: status.toLowerCase() } : p
      ),
    }))
    return status
  },

  storeProviderKey: async (provider, key) => {
    // Inbound only: set_provider_key returns a confirmation and a last-4 mask,
    // never the key. There is deliberately no command to read one back.
    await invoke<{ ok: boolean; masked_last4: string }>(
      'set_provider_key', { provider, apiKey: key },
    )
    await get().loadProviders()
  },

  deleteProviderKey: async (provider) => {
    await invoke('delete_provider_key', { provider })
    await get().loadProviders()
  },

  loadSystemPrompt: async () => {
    try {
      const prompt = await invoke<string | null>('get_system_prompt')
      set({ systemPrompt: prompt })
    } catch (e) {
      console.error('Failed to load system prompt:', e)
    }
  },

  setSystemPrompt: async (prompt) => {
    try {
      await invoke('set_system_prompt', { prompt })
      set({ systemPrompt: prompt })
    } catch (e) {
      console.error('Failed to set system prompt:', e)
    }
  },

  resetSystemPrompt: async () => {
    await get().setSystemPrompt(null)
  },

  setPAMode: (mode) => {
    // Phase 3.0 R3 — PA mode is now session-scoped. The localStorage value
    // survives as a first-launch fallback for brand-new sessions, but the
    // session row is the source of truth so reloads / engine rebuilds
    // never snap back to `fast_development`. The active session id is
    // resolved lazily from sessionStore to avoid a circular import.
    localStorage.setItem('autodev:pa_mode', mode)
    set({ paMode: mode })
    void (async () => {
      try {
        const { useSessionStore } = await import('./sessionStore')
        const activeId = useSessionStore.getState().activeSessionId
        if (activeId) {
          await invoke('set_session_pa_mode', { sessionId: activeId, paMode: mode })
        }
      } catch (e) {
        console.error('[settingsStore] set_session_pa_mode failed:', e)
      }
    })()
  },

  loadDefaultSystemPrompt: async () => {
    try {
      const defaultPrompt = await invoke<string>('get_default_system_prompt')
      set({ defaultSystemPrompt: defaultPrompt })
    } catch (e) {
      console.error('Failed to load default system prompt:', e)
    }
  },
}))
