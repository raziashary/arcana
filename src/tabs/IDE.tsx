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

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { GitBranch, Play } from 'lucide-react'
import { useIdeStore } from '../stores/ideStore'
import { useWorkspaceStore } from '../stores/workspaceStore'
import FileTree      from '../components/ide/FileTree'
import TabBar        from '../components/ide/TabBar'
import BreadcrumbNav from '../components/ide/BreadcrumbNav'
import CodeViewer    from '../components/ide/CodeViewer'
import FileSearch    from '../components/ide/FileSearch'
import SymbolOutline from '../components/ide/SymbolOutline'
import IdeStatusBar  from '../components/ide/IdeStatusBar'
import GitPanel      from '../components/git/GitPanel'

const TOOLCHAINS = ['cargo', 'go', 'npm', 'pnpm', 'yarn', 'bun', 'python', 'make'] as const
type Toolchain = typeof TOOLCHAINS[number]

interface IdeRunResult {
  toolchain: string
  project_path: string
  command: string
  exit_code: number
  stdout: string
  stderr: string
}

const MAX_OUTPUT_CHARS = 8_000

function boundedOutput(value: string): string {
  if (value.length <= MAX_OUTPUT_CHARS) return value
  return `${value.slice(0, MAX_OUTPUT_CHARS)}\n… output truncated`
}

function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error)
  return boundedOutput(message)
}

export default function IDETab() {
  const workspaceDir     = useWorkspaceStore((s) => s.workspaceDir)
  const loadWorkspaceDir = useWorkspaceStore((s) => s.loadWorkspaceDir)
  const init          = useIdeStore((s) => s.init)
  const activeTab     = useIdeStore((s) => s.activeTab)
  const openTabs      = useIdeStore((s) => s.openTabs)
  const setSearchOpen = useIdeStore((s) => s.setSearchOpen)
  const [toolchain, setToolchain] = useState<Toolchain | ''>('')
  const [runPending, setRunPending] = useState(false)
  const [runTarget, setRunTarget] = useState<string | null>(null)
  const [runResult, setRunResult] = useState<IdeRunResult | null>(null)
  const [runError, setRunError] = useState<string | null>(null)
  const activeFile = activeTab ? openTabs.find((tab) => tab.relativePath === activeTab) : undefined
  const hasActiveFile = Boolean(activeFile)
  const canRunActiveFile = Boolean(activeFile && !activeFile.loading && !activeFile.error)

  const runActiveFile = async () => {
    if (!canRunActiveFile || !activeTab || runPending) return
    setRunPending(true)
    setRunTarget(activeTab)
    setRunResult(null)
    setRunError(null)
    try {
      const result = await invoke<IdeRunResult>('run_ide_project', {
        path: activeTab,
        toolchain: toolchain || null,
      })
      setRunResult(result)
    } catch (error) {
      setRunError(errorMessage(error))
    } finally {
      setRunPending(false)
    }
  }

  // Phase 3.3 — togglable Git sub-panel on the right.
  const [gitOpen, setGitOpen] = useState(false)

  // Boot: load workspace dir if not already, then init IDE state.
  useEffect(() => {
    if (!workspaceDir) {
      void loadWorkspaceDir()
    }
  }, [workspaceDir, loadWorkspaceDir])

  useEffect(() => {
    if (workspaceDir) {
      void init()
    }
  }, [workspaceDir, init])

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
  }, [setSearchOpen])

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
        <div className="flex items-center border-b" style={{ borderColor: 'var(--border-1)' }}>
          <div className="flex-1 min-w-0"><TabBar /></div>
          <button
            onClick={() => setGitOpen((v) => !v)}
            className="flex items-center gap-1 px-2 py-1 text-[10px] font-mono uppercase tracking-wider hover:bg-[var(--bg-hover)]"
            style={{
              color:       gitOpen ? 'var(--accent)' : 'var(--text-2)',
              borderLeft:  '1px solid var(--border-1)',
            }}
            title="Toggle Git panel"
            aria-label="Toggle Git panel"
          >
            <GitBranch size={11} />
            Git
          </button>
        </div>
        <div
          className="flex flex-wrap items-center gap-2 px-2 py-1.5 border-b"
          style={{ borderColor: 'var(--border-1)', background: 'var(--bg-surface)' }}
        >
          <label htmlFor="ide-run-toolchain" className="font-mono text-[10px]" style={{ color: 'var(--text-3)' }}>
            Toolchain
          </label>
          <select
            id="ide-run-toolchain"
            value={toolchain}
            onChange={(event) => setToolchain(event.target.value as Toolchain | '')}
            disabled={runPending}
            className="min-w-0 max-w-full px-1.5 py-1 font-mono text-[10px] rounded-sm"
            style={{ color: 'var(--text-1)', background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
          >
            <option value="">Auto</option>
            {TOOLCHAINS.map((choice) => <option key={choice} value={choice}>{choice}</option>)}
          </select>
          <button
            type="button"
            onClick={() => void runActiveFile()}
            disabled={!canRunActiveFile || runPending}
            className="inline-flex items-center gap-1 px-2 py-1 rounded-sm font-mono text-[10px] disabled:opacity-40"
            style={{ color: 'var(--accent)', background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}
          >
            <Play size={10} aria-hidden="true" />
            {runPending ? 'Running…' : 'Run'}
          </button>
          {!hasActiveFile && (
            <span className="font-mono text-[10px]" style={{ color: 'var(--text-3)' }}>
              Open a file to run its project.
            </span>
          )}
          {activeFile?.loading && (
            <span className="font-mono text-[10px]" style={{ color: 'var(--text-3)' }}>
              Wait for the file to finish loading.
            </span>
          )}
          {activeFile?.error && (
            <span className="font-mono text-[10px]" style={{ color: 'var(--danger)' }}>
              The active file could not be loaded.
            </span>
          )}
          <span className="font-mono text-[10px]" style={{ color: 'var(--text-3)' }}>
            120-second limit
          </span>
        </div>
        {(runPending || runResult || runError) && (
          <section
            aria-label="IDE run result"
            aria-live="polite"
            className="max-h-40 overflow-auto border-b px-2 py-1.5 font-mono text-[10px]"
            style={{ borderColor: 'var(--border-1)', background: 'var(--bg-card)', color: 'var(--text-2)' }}
          >
            {runTarget && <div style={{ overflowWrap: 'anywhere' }}>File: {runTarget}</div>}
            {runPending && <div role="status">Running project…</div>}
            {runError && <div role="alert" style={{ color: 'var(--danger)', whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>{runError}</div>}
            {runResult && (
              <div role="status" aria-label="Run output" className="space-y-1">
                <div style={{ overflowWrap: 'anywhere' }}>
                  {runResult.toolchain} · {runResult.project_path} · exit {runResult.exit_code}
                </div>
                <div style={{ overflowWrap: 'anywhere' }}>Command: {runResult.command}</div>
                {runResult.stdout && (
                  <pre className="whitespace-pre-wrap" style={{ overflowWrap: 'anywhere' }}>
                    {boundedOutput(runResult.stdout)}
                  </pre>
                )}
                {runResult.stderr && (
                  <pre className="whitespace-pre-wrap" style={{ color: 'var(--danger)', overflowWrap: 'anywhere' }}>
                    {boundedOutput(runResult.stderr)}
                  </pre>
                )}
                {!runResult.stdout && !runResult.stderr && <div>No output.</div>}
              </div>
            )}
          </section>
        )}
        {activeFile && activeTab ? (
          <>
            <BreadcrumbNav path={activeTab} />
            <CodeViewer tabPath={activeTab} />
          </>
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

      {/* Right column — Git panel (togglable) */}
      {gitOpen && <GitPanel onClose={() => setGitOpen(false)} />}

      <FileSearch />
    </div>
  )
}
