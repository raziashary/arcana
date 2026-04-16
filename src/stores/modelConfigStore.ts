// Model config store — per-model temperature, top_p, reasoning_effort, thinking settings.
// Phase 2.3: T4a/T4b — Reasoning Effort + Temperature/Top-P per-model configuration.
// Persisted in ~/.autodev/config.db via Tauri backend.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ModelConfig {
  model_id:           string
  temperature:        number | null   // 0.0 – 2.0; null = provider default
  top_p:              number | null   // 0.0 – 1.0; null = provider default
  reasoning_effort:   'low' | 'medium' | 'high' | null  // for o1/o3/o4-mini / Gemini thinking
  thinking_enabled:   boolean         // enable extended thinking / reasoning blocks
  thinking_budget:    number | null   // token budget (Anthropic: 1024–100000; Google: similar)
}

// Models that support extended thinking / reasoning modes
export const THINKING_MODELS = new Set([
  'claude-opus-4-5',
  'claude-sonnet-4-5',
  'gemini-2.0-flash-thinking-exp-01-21',
  'gemini-2.5-pro',
  'o4-mini',
])

// Models that support reasoning_effort parameter (OpenAI o-series)
export const REASONING_EFFORT_MODELS = new Set([
  'o4-mini',
  'o3',
  'o3-mini',
  'o1',
  'o1-mini',
])

// ─── Store ────────────────────────────────────────────────────────────────────

interface ModelConfigState {
  configs:         Record<string, ModelConfig>
  loadingModelId:  string | null
  savingModelId:   string | null

  loadConfig:   (modelId: string) => Promise<ModelConfig>
  saveConfig:   (cfg: ModelConfig) => Promise<void>
  resetConfig:  (modelId: string) => Promise<void>
  getConfig:    (modelId: string) => ModelConfig
}

function defaultConfig(modelId: string): ModelConfig {
  return {
    model_id:         modelId,
    temperature:      null,
    top_p:            null,
    reasoning_effort: null,
    thinking_enabled: false,
    thinking_budget:  null,
  }
}

export const useModelConfigStore = create<ModelConfigState>((set, get) => ({
  configs:        {},
  loadingModelId: null,
  savingModelId:  null,

  loadConfig: async (modelId) => {
    set({ loadingModelId: modelId })
    try {
      const cfg = await invoke<ModelConfig | null>('get_model_config', { modelId })
      const resolved = cfg ?? defaultConfig(modelId)
      set(s => ({ configs: { ...s.configs, [modelId]: resolved } }))
      return resolved
    } catch (e) {
      console.error(`Failed to load model config for ${modelId}:`, e)
      const fallback = defaultConfig(modelId)
      set(s => ({ configs: { ...s.configs, [modelId]: fallback } }))
      return fallback
    } finally {
      set({ loadingModelId: null })
    }
  },

  saveConfig: async (cfg) => {
    set({ savingModelId: cfg.model_id })
    try {
      await invoke('set_model_config', {
        modelId:         cfg.model_id,
        temperature:     cfg.temperature,
        topP:            cfg.top_p,
        reasoningEffort: cfg.reasoning_effort,
        thinkingEnabled: cfg.thinking_enabled,
        thinkingBudget:  cfg.thinking_budget,
      })
      set(s => ({ configs: { ...s.configs, [cfg.model_id]: cfg } }))
    } catch (e) {
      console.error(`Failed to save model config for ${cfg.model_id}:`, e)
      throw e
    } finally {
      set({ savingModelId: null })
    }
  },

  resetConfig: async (modelId) => {
    try {
      await invoke('delete_model_config', { modelId })
      set(s => {
        const next = { ...s.configs }
        delete next[modelId]
        return { configs: next }
      })
    } catch (e) {
      console.error(`Failed to reset model config for ${modelId}:`, e)
      throw e
    }
  },

  getConfig: (modelId) => {
    return get().configs[modelId] ?? defaultConfig(modelId)
  },
}))
