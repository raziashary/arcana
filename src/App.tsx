// App root — tab router + layout shell.
// Renders TitleBar + TabBar + active tab content + Sidebar + StatusBar.
// Design matches arcanamockup.html exactly.

import { useEffect } from 'react'
import TitleBar    from './components/layout/TitleBar'
import TabBar      from './components/layout/TabBar'
import StatusBar   from './components/layout/StatusBar'
import Sidebar     from './components/layout/Sidebar'
import ChatTab     from './tabs/Chat'
import ArcanaTab  from './tabs/ArcanaPanel'
import IDETab      from './tabs/IDE'
import RecipesTab  from './tabs/Recipes'
import GraphTab    from './tabs/Graph'
import SettingsTab from './tabs/Settings'
import LogsTab     from './tabs/Logs'
import DocsTab     from './tabs/Docs'
import Dashboard   from './components/dashboard/Dashboard'
import ToastStack from './components/ToastStack'
import { useSettingsStore } from './stores/settingsStore'
import { useCostStore }     from './stores/costStore'
import { useUiStore }       from './stores/uiStore'
import { useWorkerStore }   from './stores/workerStore'
import { useTaskStore }     from './stores/taskStore'
import { useUpdateStore }   from './stores/updateStore'
import { useWorkerChatStore } from './stores/workerChatStore'
import { UpdateToast }      from './components/update/UpdateToast'
import { UpdateDialog }     from './components/update/UpdateDialog'
import { ErrorBoundary }    from './components/ErrorBoundary'
// Doc 07 Phase 3/4 — startup banners for stray files + workspace mismatch.
import { WorkspaceMismatchBanner, StrayFilesBanner } from './components/WorkspaceMismatchBanner'
// Doc 05 Phase 1 — sidecar crash banner with stderr tail + copy report.

export type TabId = 'chat' | 'arcana' | 'ide' | 'recipes' | 'graph' | 'dashboard' | 'settings' | 'logs' | 'docs'

const TABS: { id: TabId; label: string }[] = [
  { id: 'chat',      label: 'Chat'      },
  { id: 'arcana',   label: 'Arcana'   },
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
  const initUpdater       = useUpdateStore(s => s.init)
  const cleanupUpdater    = useUpdateStore(s => s.cleanup)
  const hydrateWorkerChat = useWorkerChatStore(s => s.hydrate)

  useEffect(() => {
    loadProviders()
    loadDefaultSystemPrompt()
    loadFromBackend()
    // Phase 3.0 R1/R2 — install the Tauri event-channel subscriptions once
    // per app lifetime. All calls are idempotent (guarded by module-level
    // flags) so React StrictMode's double-mount is safe.
    void initWorkerEvents()
    void initTaskEvents()
    // Arcana Communication Protocol — seed Fleet Chat from SQLite + subscribe
    // to the live worker:message stream. Idempotent.
    void hydrateWorkerChat()
    // Phase 5.3 — subscribe to auto-updater events
    void initUpdater()
    return () => { cleanupUpdater() }
  }, [])

  return (
    <div className="app-shell">
      {/* P8-52 finished in P8-60. `StartupOverlay` was mounted here
          unconditionally and hid itself only once a `startup:progress` event
          reached the "ready" phase. P8-52 deleted the FSM that emitted those
          events and the `get_startup_phase` command the overlay fell back to,
          but left the component mounted -- so the app rendered a full-screen
          blocking modal that nothing could ever dismiss. */}
      <TitleBar />
      <TabBar
        tabs={TABS}
        activeTab={activeTab}
        onTabChange={setActiveTab}
      />

      {/* Doc 07 Phase 3/4 — startup-time workspace banners. Both are
          dismissible and only render on the corresponding one-shot
          Tauri event from main.rs setup. */}
      <ErrorBoundary label="Workspace Mismatch Banner" inline>
        <WorkspaceMismatchBanner />
      </ErrorBoundary>
      <ErrorBoundary label="Stray Files Banner" inline>
        <StrayFilesBanner />
      </ErrorBoundary>

      <div className="app-body">
        <div className="app-main">
          {activeTab === 'chat'      && <ErrorBoundary label="Chat"><ChatTab /></ErrorBoundary>}
          {activeTab === 'arcana'   && <ErrorBoundary label="Arcana"><ArcanaTab /></ErrorBoundary>}
          {activeTab === 'ide'       && <ErrorBoundary label="IDE"><IDETab /></ErrorBoundary>}
          {activeTab === 'recipes'   && <ErrorBoundary label="Recipes"><RecipesTab /></ErrorBoundary>}
          {activeTab === 'graph'     && <ErrorBoundary label="Graph"><GraphTab /></ErrorBoundary>}
          {activeTab === 'dashboard' && <ErrorBoundary label="Dashboard"><Dashboard /></ErrorBoundary>}
          {activeTab === 'settings'  && <ErrorBoundary label="Settings"><SettingsTab /></ErrorBoundary>}
          {activeTab === 'logs'      && <ErrorBoundary label="Logs"><LogsTab /></ErrorBoundary>}
          {activeTab === 'docs'      && <ErrorBoundary label="Docs"><DocsTab /></ErrorBoundary>}
        </div>

        <ErrorBoundary label="Sidebar" inline><Sidebar /></ErrorBoundary>
      </div>

      <ErrorBoundary label="StatusBar" inline><StatusBar /></ErrorBoundary>
      <ToastStack />
      {/* Phase 5.3 — auto-updater overlay components */}
      <UpdateToast />
      <UpdateDialog />
    </div>
  )
}
