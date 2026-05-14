// UI store — cross-cutting UI state that needs to be read and mutated from
// anywhere in the app (not scoped to a single tab).
//
// Today this holds only the active top-level tab. Previously this lived in
// `App.tsx` local state; promoting it to a store lets chat-side components
// (tool-result renderers, etc.) jump into the IDE tab when the user clicks
// "Open in IDE".

import { create } from 'zustand'
import type { TabId } from '../App'

export interface UiStore {
  activeTab:    TabId
  setActiveTab: (tab: TabId) => void

  // Doc 06 Phase 3 — imperative "scroll the qc_review column into view and
  // pulse it". Incremented on every QC card chip click. The AutoDev task
  // board subscribes to the counter and runs a one-shot scrollIntoView + a
  // brief CSS pulse class. Keeping it as an integer counter rather than a
  // boolean lets the subscriber distinguish back-to-back clicks.
  taskColumnPulse: { colId: string; nonce: number }
  pulseTaskColumn: (colId: string) => void
}

export const useUiStore = create<UiStore>((set, get) => ({
  activeTab:    'chat',
  setActiveTab: (tab) => set({ activeTab: tab }),

  taskColumnPulse: { colId: '', nonce: 0 },
  pulseTaskColumn: (colId) => {
    set({ taskColumnPulse: { colId, nonce: get().taskColumnPulse.nonce + 1 } })
  },
}))
