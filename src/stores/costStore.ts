// Cost store — today's running totals, updated via agent:done events.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

interface CostState {
  costTodayUsd:   number
  tokensInToday:  number
  tokensOutToday: number

  loadFromBackend: () => Promise<void>
  addCost:         (costUsd: number, tokensIn: number, tokensOut: number) => void
}

export const useCostStore = create<CostState>((set) => ({
  costTodayUsd:   0,
  tokensInToday:  0,
  tokensOutToday: 0,

  loadFromBackend: async () => {
    try {
      const [cost, tIn, tOut] = await invoke<[number, number, number]>('get_cost_today')
      set({ costTodayUsd: cost, tokensInToday: tIn, tokensOutToday: tOut })
    } catch (e) {
      console.error('Failed to load cost data:', e)
    }
  },

  addCost: (costUsd, tokensIn, tokensOut) => set(s => ({
    costTodayUsd:   s.costTodayUsd   + costUsd,
    tokensInToday:  s.tokensInToday  + tokensIn,
    tokensOutToday: s.tokensOutToday + tokensOut,
  })),
}))
