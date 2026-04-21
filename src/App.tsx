// App root — tab router + layout shell.
// Renders TitleBar + TabBar + active tab content + Sidebar + StatusBar.
// Design matches autodevmockup.html exactly.

import { useEffect } from 'react'
import TitleBar    from './components/layout/TitleBar'
import TabBar      from './components/layout/TabBar'
import StatusBar   from './components/layout/StatusBar'
import Sidebar     from './components/layout/Sidebar'
import ChatTab     from './tabs/Chat'
import AutoDevTab  from './tabs/AutoDevPanel'
import IDETab      from './tabs/IDE'
import RecipesTab  from './tabs/Recipes'
import SettingsTab from './tabs/Settings'
import LogsTab     from './tabs/Logs'
import StartupOverlay from './components/startup/StartupOverlay'
import { useSettingsStore } from './stores/settingsStore'
import { useCostStore }     from './stores/costStore'
import { useUiStore }       from './stores/uiStore'

export type TabId = 'chat' | 'autodev' | 'ide' | 'recipes' | 'settings' | 'logs'

const TABS: { id: TabId; label: string }[] = [
  { id: 'chat',     label: 'Chat'     },
  { id: 'autodev',  label: 'AutoDev'  },
  { id: 'ide',      label: 'IDE'      },
  { id: 'recipes',  label: 'Recipes'  },
  { id: 'settings', label: 'Settings' },
  { id: 'logs',     label: 'Logs'     },
]

export default function App() {
  const activeTab    = useUiStore((s) => s.activeTab)
  const setActiveTab = useUiStore((s) => s.setActiveTab)
  const { loadProviders, loadDefaultSystemPrompt } = useSettingsStore()
  const { loadFromBackend } = useCostStore()

  useEffect(() => {
    loadProviders()
    loadDefaultSystemPrompt()
    loadFromBackend()
  }, [])

  return (
    <div className="app-shell">
      <StartupOverlay />
      <TitleBar />
      <TabBar
        tabs={TABS}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />

      <div className="app-body">
        <div className="app-main">
          {activeTab === 'chat'     && <ChatTab />}
          {activeTab === 'autodev'  && <AutoDevTab />}
          {activeTab === 'ide'      && <IDETab />}
          {activeTab === 'recipes'  && <RecipesTab />}
          {activeTab === 'settings' && <SettingsTab />}
          {activeTab === 'logs'     && <LogsTab />}
        </div>

        <Sidebar />
      </div>

      <StatusBar />
    </div>
  )
}
