// Doc 07 Phase 3/4 — workspace startup banners.
//
// Two banners surfaced at the top of the app:
//
//   1. WorkspaceMismatchBanner — fires when `current_dir()` at startup
//      differs from the configured workspace. Lets the user click "Set
//      workspace" to reopen Settings without restarting.
//
//   2. StrayFilesBanner — fires when the startup stray-file scan detects
//      worker-generated files in `src-tauri/` or `src/`. Lists each file
//      and offers a "delete all" affordance.
//
// Both subscribe to one-shot Tauri events emitted by main.rs on startup
// and stay visible until the user dismisses them or fixes the underlying
// issue and restarts.

import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useWorkspaceStore } from '../stores/workspaceStore'

interface StrayFile {
  absolute_path:    string
  relative_to_repo: string
  size_bytes:       number
  reason:           string
}

interface StraysPayload {
  strays: StrayFile[]
}

interface MismatchPayload {
  current_dir:          string
  configured_workspace: string
  timestamp:            string
}

export function WorkspaceMismatchBanner() {
  const [mismatch, setMismatch] = useState<MismatchPayload | null>(null)
  const [dismissed, setDismissed] = useState(false)

  useEffect(() => {
    const unlisten = listen<MismatchPayload>('workspace:mismatch', e => {
      setMismatch(e.payload)
      setDismissed(false)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  if (!mismatch || dismissed) return null

  return (
    <div
      role="alert"
      style={{
        background: 'rgba(255, 203, 139, 0.10)',
        border:     '1px solid rgba(255, 203, 139, 0.35)',
        color:      'var(--warning)',
        padding:    '8px 14px',
        fontFamily: 'var(--font-mono)',
        fontSize:   '11px',
        display:    'flex',
        alignItems: 'center',
        gap:        12,
      }}
    >
      <span style={{ flexShrink: 0 }}>⚠</span>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ color: 'var(--text-1)', marginBottom: 2 }}>
          Workspace mismatch
        </div>
        <div style={{ color: 'var(--text-2)', wordBreak: 'break-all' }}>
          AutoDev is running with <code>cwd={mismatch.current_dir}</code> but
          the configured workspace is <code>{mismatch.configured_workspace}</code>.
          Agent file writes resolve against the configured workspace (the
          path jail), but any tool call that bypasses the jail still risks
          targeting the wrong tree.
        </div>
      </div>
      <button
        onClick={() => {
          // `open_workspace_picker` never existed as a command, so this
          // always fell through to the clipboard and the button appeared to
          // do nothing. The real flow is the workspace store's
          // `pickWorkspaceDir`, which calls `pick_workspace_dir`.
          // Found by the P0 post-phase audit (F-16).
          void useWorkspaceStore.getState().pickWorkspaceDir()
        }}
        style={{
          background:   'var(--bg-card)',
          border:       '1px solid var(--border-1)',
          color:        'var(--accent)',
          padding:      '3px 10px',
          borderRadius: 3,
          cursor:       'pointer',
          fontFamily:   'inherit',
          fontSize:     '10px',
        }}
      >
        Set workspace
      </button>
      <button
        onClick={() => setDismissed(true)}
        style={{
          background:   'transparent',
          border:       '1px solid var(--border-1)',
          color:        'var(--text-3)',
          padding:      '3px 8px',
          borderRadius: 3,
          cursor:       'pointer',
          fontFamily:   'inherit',
          fontSize:     '10px',
        }}
        aria-label="Dismiss"
      >
        ✕
      </button>
    </div>
  )
}

export function StrayFilesBanner() {
  const [strays, setStrays] = useState<StrayFile[]>([])
  const [dismissed, setDismissed] = useState(false)

  useEffect(() => {
    const unlisten = listen<StraysPayload>('workspace:strays-detected', e => {
      setStrays(e.payload.strays ?? [])
      setDismissed(false)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  if (strays.length === 0 || dismissed) return null

  return (
    <div
      role="alert"
      style={{
        background: 'rgba(255, 99, 99, 0.10)',
        border:     '1px solid rgba(255, 99, 99, 0.35)',
        color:      'var(--danger)',
        padding:    '8px 14px',
        fontFamily: 'var(--font-mono)',
        fontSize:   '11px',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
        <span style={{ flexShrink: 0 }}>⚠</span>
        <div style={{ flex: 1 }}>
          <div style={{ color: 'var(--text-1)', marginBottom: 2 }}>
            {strays.length} stray file{strays.length === 1 ? '' : 's'} detected in AutoDev repo
          </div>
          <div style={{ color: 'var(--text-2)' }}>
            These files were likely written by a previous worker run with a leaked
            workspace_dir. Review and delete before the next E2E run — they will
            otherwise trigger HMR restarts and confuse the Tauri host.
          </div>
        </div>
        <button
          onClick={() => setDismissed(true)}
          style={{
            background:   'transparent',
            border:       '1px solid var(--border-1)',
            color:        'var(--text-3)',
            padding:      '3px 8px',
            borderRadius: 3,
            cursor:       'pointer',
            fontSize:     '10px',
          }}
          aria-label="Dismiss"
        >
          ✕
        </button>
      </div>
      <ul style={{ margin: '6px 0 0 24px', padding: 0, listStyle: 'disc' }}>
        {strays.map(s => (
          <li key={s.absolute_path} style={{ color: 'var(--text-2)', marginBottom: 2 }}>
            <code>{s.relative_to_repo}</code>
            <span style={{ color: 'var(--text-3)' }}> · {s.size_bytes} bytes · {s.reason}</span>
          </li>
        ))}
      </ul>
    </div>
  )
}
