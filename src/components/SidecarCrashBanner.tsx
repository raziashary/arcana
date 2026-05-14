// Doc 05 Phase 1 — sidecar crash banner.
//
// Subscribes to the `sidecar:crash` Tauri event emitted from
// `sidecar/mod.rs` when the circuit breaker opens (5 crashes in 60s).
// Renders a dismissible banner with the captured stderr tail folded into
// a <details> block + a "Copy crash report" button that bundles the
// stderr with the surrounding Rust log window for issue triage.

import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'

interface CrashPayload {
  sidecar_id:    string
  restart_count: number
  window_secs:   number
  stderr_tail:   string[]
  timestamp:     string
}

export function SidecarCrashBanner() {
  const [crash, setCrash] = useState<CrashPayload | null>(null)
  const [dismissed, setDismissed] = useState(false)
  const [copyOk, setCopyOk] = useState(false)

  useEffect(() => {
    const unlisten = listen<CrashPayload>('sidecar:crash', e => {
      setCrash(e.payload)
      setDismissed(false)
    })
    return () => { unlisten.then(fn => fn()) }
  }, [])

  if (!crash || dismissed) return null

  // Doc 05 Phase 1.3 — "Copy crash report" combines the stderr tail with
  // the surrounding host metadata so the user can paste it directly into
  // a GitHub issue without opening the log file.
  function copyReport() {
    if (!crash) return
    const report = [
      `# Sidecar crash — ${crash.sidecar_id}`,
      '',
      `- Restart count: ${crash.restart_count} in ${crash.window_secs}s window`,
      `- Detected at: ${crash.timestamp}`,
      `- AutoDev host: ${navigator.userAgent}`,
      '',
      '## Captured stderr tail',
      '```',
      ...crash.stderr_tail,
      '```',
    ].join('\n')
    navigator.clipboard.writeText(report)
      .then(() => { setCopyOk(true); setTimeout(() => setCopyOk(false), 2_000) })
      .catch(() => { /* clipboard refused — user can still expand the details */ })
  }

  return (
    <div
      role="alert"
      style={{
        background: 'rgba(255, 99, 99, 0.10)',
        border:     '1px solid rgba(255, 99, 99, 0.45)',
        color:      'var(--text-1)',
        padding:    '10px 14px',
        fontFamily: 'var(--font-mono)',
        fontSize:   '11px',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
        <span style={{ color: 'var(--danger)' }}>✗</span>
        <div style={{ flex: 1 }}>
          <div style={{ color: 'var(--text-1)', marginBottom: 2 }}>
            Sidecar crash — circuit breaker open
          </div>
          <div style={{ color: 'var(--text-2)' }}>
            <code>{crash.sidecar_id}</code> crashed {crash.restart_count} times in {' '}
            {crash.window_secs}s. The watchdog has stopped attempting restarts —
            Mode 2 (LivingAgent) will stay unavailable until you fix the
            underlying issue and restart AutoDev.
          </div>
        </div>
        <button
          onClick={copyReport}
          style={{
            background:   'var(--bg-card)',
            border:       '1px solid var(--border-1)',
            color:        copyOk ? 'var(--accent)' : 'var(--text-1)',
            padding:      '3px 10px',
            borderRadius: 3,
            cursor:       'pointer',
            fontFamily:   'inherit',
            fontSize:     '10px',
          }}
        >
          {copyOk ? '✓ Copied' : 'Copy crash report'}
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
      <details style={{ marginTop: 6, color: 'var(--text-3)' }}>
        <summary style={{ cursor: 'pointer', userSelect: 'none' }}>
          Captured stderr tail ({crash.stderr_tail.length} line{crash.stderr_tail.length === 1 ? '' : 's'})
        </summary>
        <pre
          style={{
            marginTop:    4,
            padding:      '6px 8px',
            background:   'var(--bg-app)',
            border:       '1px solid var(--border-1)',
            borderRadius: 3,
            maxHeight:    220,
            overflow:     'auto',
            fontSize:     '10px',
            lineHeight:   '14px',
            color:        'var(--text-2)',
            whiteSpace:   'pre-wrap',
            wordBreak:    'break-word',
          }}
        >
          {crash.stderr_tail.length > 0 ? crash.stderr_tail.join('\n') : '(no stderr captured)'}
        </pre>
      </details>
    </div>
  )
}
