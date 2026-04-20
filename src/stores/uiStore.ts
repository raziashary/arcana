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
}

export const useUiStore = create<UiStore>((set) => ({
  activeTab:    'chat',
  setActiveTab: (tab) => set({ activeTab: tab }),
}))
