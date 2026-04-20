// IDE tab — Phase 3.2 full layout.
//
// Layout:
//   ┌──────────────────────────────────────────────────────────────┐
//   │  File Tree  │ Tabs row (TabBar)                              │
//   │             │──────────────────────────────────────────────  │
//   │             │                                                │
//   │             │ CodeViewer (Monaco, read-only)                 │
//   │             │                                                │
//   │             │                                                │
//   │             │──────────────────────────────────────────────  │
//   │             │ IdeStatusBar                                   │
//   ├────────────┤                                                │
//   │  Outline    │                                                │
//   └──────────────────────────────────────────────────────────────┘
//
// Shortcuts:
//   Ctrl/Cmd + P → FileSearch overlay

import { useEffect } from 'react'
import { useIdeStore } from '../stores/ideStore'
import { useWorkspaceStore } from '../stores/workspaceStore'
import FileTree      from '../components/ide/FileTree'
import TabBar        from '../components/ide/TabBar'
import CodeViewer    from '../components/ide/CodeViewer'
import FileSearch    from '../components/ide/FileSearch'
import SymbolOutline from '../components/ide/SymbolOutline'
import IdeStatusBar  from '../components/ide/IdeStatusBar'

export default function IDETab() {
  const { workspaceDir, loadWorkspaceDir } = useWorkspaceStore()
  const init          = useIdeStore((s) => s.init)
  const activeTab     = useIdeStore((s) => s.activeTab)
  const openTabs      = useIdeStore((s) => s.openTabs)
  const setSearchOpen = useIdeStore((s) => s.setSearchOpen)

  // Boot: load workspace dir if not already, then init IDE state.
  useEffect(() => {
    if (!workspaceDir) {
      void loadWorkspaceDir()
    }
  }, [])

  useEffect(() => {
    if (workspaceDir) {
      void init()
    }
  }, [workspaceDir])

  // Global Ctrl/Cmd+P → quick open.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'p') {
        e.preventDefault()
        setSearchOpen(true)
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  if (!workspaceDir) {
    return (
      <div
        className="flex-1 flex flex-col items-center justify-center p-8"
        style={{ color: 'var(--text-3)' }}
      >
        <div className="max-w-md space-y-3 text-center">
          <span className="font-mono text-xs" style={{ color: 'var(--text-1)' }}>
            IDE
          </span>
          <p className="text-sm leading-relaxed" style={{ color: 'var(--text-2)' }}>
            Pick a workspace folder in <strong>Settings</strong> to unlock the file tree,
            Monaco viewer, and symbol outline.
          </p>
        </div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex overflow-hidden">
      {/* Left column — file tree on top, outline at bottom */}
      <div
        className="flex flex-col border-r"
        style={{ borderColor: 'var(--border-1)', width: 260, minWidth: 220 }}
      >
        <div className="flex-[2] min-h-0 flex flex-col">
          <FileTree />
        </div>
        <div
          className="flex-1 min-h-0 flex flex-col border-t"
          style={{ borderColor: 'var(--border-1)' }}
        >
          <SymbolOutline />
        </div>
      </div>

      {/* Main column — tabs, editor, status bar */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <TabBar />
        {activeTab && openTabs.some((t) => t.relativePath === activeTab) ? (
          <CodeViewer tabPath={activeTab} />
        ) : (
          <div
            className="flex-1 flex flex-col items-center justify-center"
            style={{ color: 'var(--text-3)' }}
          >
            <p className="text-xs font-mono mb-2">No file open</p>
            <p className="text-[11px]" style={{ color: 'var(--text-2)' }}>
              Select a file from the tree or press <kbd>Ctrl/⌘+P</kbd> to quick-open.
            </p>
          </div>
        )}
        <IdeStatusBar />
      </div>

      <FileSearch />
    </div>
  )
}
