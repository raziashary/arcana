// Settings store — provider config, active model, theme.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

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

  loadProviders:         () => Promise<void>
  setActiveProvider:     (provider: string, model: string) => Promise<void>
  testProvider:          (provider: string) => Promise<string>
  storeProviderKey:      (provider: string, key: string) => Promise<void>
  deleteProviderKey:     (provider: string) => Promise<void>
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  providers:          [],
  activeProvider:     null,
  activeModel:        null,
  isLoadingProviders: false,

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
    await invoke('store_provider_key', { provider, apiKey: key })
    await get().loadProviders()
  },

  deleteProviderKey: async (provider) => {
    await invoke('delete_provider_key', { provider })
    await get().loadProviders()
  },
}))
