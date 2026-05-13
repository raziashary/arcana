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
import GraphTab    from './tabs/Graph'
import SettingsTab from './tabs/Settings'
import LogsTab     from './tabs/Logs'
import DocsTab     from './tabs/Docs'
import Dashboard   from './components/dashboard/Dashboard'
import StartupOverlay from './components/startup/StartupOverlay'
import ToastStack from './components/ToastStack'
import { useSettingsStore } from './stores/settingsStore'
import { useCostStore }     from './stores/costStore'
import { useUiStore }       from './stores/uiStore'
import { useWorkerStore }   from './stores/workerStore'
import { useTaskStore }     from './stores/taskStore'
import { useSidecarStore }  from './stores/sidecarStore'
import { useUpdateStore }   from './stores/updateStore'
import { UpdateToast }      from './components/update/UpdateToast'
import { UpdateDialog }     from './components/update/UpdateDialog'
import { ErrorBoundary }    from './components/ErrorBoundary'

export type TabId = 'chat' | 'autodev' | 'ide' | 'recipes' | 'graph' | 'dashboard' | 'settings' | 'logs' | 'docs'

const TABS: { id: TabId; label: string }[] = [
  { id: 'chat',      label: 'Chat'      },
  { id: 'autodev',   label: 'AutoDev'   },
  { id: 'ide',       label: 'IDE'       },
  { id: 'recipes',   label: 'Recipes'   },
  { id: 'logs',      label: 'Logs'      },
  { id: 'settings',  label: 'Settings'  },
  { id: 'graph',     label: 'Graph'     },
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'docs',      label: 'Docs'      },
]

export default function App() {
  const activeTab    = useUiStore((s) => s.activeTab)
  const setActiveTab = useUiStore((s) => s.setActiveTab)
  const { loadProviders, loadDefaultSystemPrompt } = useSettingsStore()
  const { loadFromBackend } = useCostStore()
  const initWorkerEvents  = useWorkerStore(s => s.initEventListeners)
  const initTaskEvents    = useTaskStore(s => s.initEventListeners)
  const initSidecarEvents = useSidecarStore(s => s.initEventListeners)
  const initUpdater       = useUpdateStore(s => s.init)
  const cleanupUpdater    = useUpdateStore(s => s.cleanup)

  useEffect(() => {
    loadProviders()
    loadDefaultSystemPrompt()
    loadFromBackend()
    // Phase 3.0 R1/R2 — install the Tauri event-channel subscriptions once
    // per app lifetime. All calls are idempotent (guarded by module-level
    // flags) so React StrictMode's double-mount is safe.
    void initWorkerEvents()
    void initTaskEvents()
    void initSidecarEvents()
    // Phase 5.3 — subscribe to auto-updater events
    void initUpdater()
    return () => { cleanupUpdater() }
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
          {activeTab === 'chat'      && <ErrorBoundary label="Chat"><ChatTab /></ErrorBoundary>}
          {activeTab === 'autodev'   && <ErrorBoundary label="AutoDev"><AutoDevTab /></ErrorBoundary>}
          {activeTab === 'ide'       && <ErrorBoundary label="IDE"><IDETab /></ErrorBoundary>}
          {activeTab === 'recipes'   && <ErrorBoundary label="Recipes"><RecipesTab /></ErrorBoundary>}
          {activeTab === 'graph'     && <ErrorBoundary label="Graph"><GraphTab /></ErrorBoundary>}
          {activeTab === 'dashboard' && <ErrorBoundary label="Dashboard"><Dashboard /></ErrorBoundary>}
          {activeTab === 'settings'  && <ErrorBoundary label="Settings"><SettingsTab /></ErrorBoundary>}
          {activeTab === 'logs'      && <ErrorBoundary label="Logs"><LogsTab /></ErrorBoundary>}
          {activeTab === 'docs'      && <ErrorBoundary label="Docs"><DocsTab /></ErrorBoundary>}
        </div>

        <Sidebar />
      </div>

      <StatusBar />
      <ToastStack />
      {/* Phase 5.3 — auto-updater overlay components */}
      <UpdateToast />
      <UpdateDialog />
    </div>
  )
}
