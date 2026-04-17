// Skill store — Phase 3.1
// Manages loaded skills, search, execution history, and the palette open state.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

// ── Types ─────────────────────────────────────────────────────────────────────

export type SkillCategory = 'coding' | 'debugging' | 'devops' | 'fleet' | 'custom'

export interface SkillParameter {
  name:        string
  param_type:  'string' | 'boolean' | 'integer' | 'enum'
  required:    boolean
  description: string
  default?:    unknown
  values?:     string[]
}

export interface SkillSummary {
  name:        string
  description: string
  category:    SkillCategory
  tags:        string[]
  version:     string
  is_builtin:  boolean
  param_count: number
}

export interface Skill extends SkillSummary {
  parameters: SkillParameter[]
  template:   string
  file_path:  string
  author:     string
}

export interface SkillExecutionResult {
  skill_name:  string
  rendered:    string
  executed_at: string
  parameters:  Record<string, unknown>
}

export interface ValidationError {
  field:   string
  message: string
}

export interface ValidationResult {
  valid:  boolean
  errors: ValidationError[]
}

// ── Store ──────────────────────────────────────────────────────────────────────

interface SkillState {
  byCategory:       Record<string, SkillSummary[]>
  recentSkills:     string[]          // skill names, most-recent first
  executionHistory: SkillExecutionResult[]
  isLoading:        boolean
  error:            string | null
  paletteOpen:      boolean

  loadSkills:              () => Promise<void>
  searchSkills:            (query: string) => Promise<SkillSummary[]>
  getSkill:                (name: string) => Promise<Skill>
  executeSkill:            (name: string, params: Record<string, unknown>) => Promise<SkillExecutionResult>
  validateParams:          (name: string, params: Record<string, unknown>) => Promise<ValidationResult>
  reloadFromDisk:          () => Promise<void>
  openPalette:             () => void
  closePalette:            () => void
  addToRecent:             (name: string) => void
}

const MAX_RECENT = 8

export const useSkillStore = create<SkillState>((set, get) => ({
  byCategory:       {},
  recentSkills:     [],
  executionHistory: [],
  isLoading:        false,
  error:            null,
  paletteOpen:      false,

  loadSkills: async () => {
    set({ isLoading: true, error: null })
    try {
      const byCategory = await invoke<Record<string, SkillSummary[]>>('skills_by_category')
      set({ byCategory, isLoading: false })
    } catch (e) {
      set({ error: String(e), isLoading: false })
    }
  },

  searchSkills: async (query: string) => {
    return invoke<SkillSummary[]>('search_skills', { query })
  },

  getSkill: async (name: string) => {
    return invoke<Skill>('get_skill', { name })
  },

  executeSkill: async (name: string, params: Record<string, unknown>) => {
    const result = await invoke<SkillExecutionResult>('execute_skill', {
      name,
      parameters: params,
    })
    get().addToRecent(name)
    set(s => ({ executionHistory: [result, ...s.executionHistory].slice(0, 50) }))
    return result
  },

  validateParams: async (name: string, params: Record<string, unknown>) => {
    return invoke<ValidationResult>('validate_skill_parameters', { name, parameters: params })
  },

  reloadFromDisk: async () => {
    await invoke('reload_skills')
    await get().loadSkills()
  },

  openPalette:  () => set({ paletteOpen: true }),
  closePalette: () => set({ paletteOpen: false }),

  addToRecent: (name: string) => {
    set(s => {
      const filtered = s.recentSkills.filter(n => n !== name)
      return { recentSkills: [name, ...filtered].slice(0, MAX_RECENT) }
    })
  },
}))
