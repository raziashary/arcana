// StatusBar — spec Section 2.3.
// Left: PA mode dot + label. Middle: sidecar/MCP status dots. Right: session token count.

import { useCostStore }     from '../../stores/costStore'
import { useSettingsStore } from '../../stores/settingsStore'
import { useSidecarStore }  from '../../stores/sidecarStore'

const PA_MODE_LABELS: Record<string, string> = {
  fast:     'Fast Dev',
  review:   'Review Mode',
  delegate: 'Delegate',
}

const PA_MODE_COLORS: Record<string, string> = {
  fast:     'var(--accent)',
  review:   'var(--warning)',
  delegate: 'var(--info)',
}

function fmtTokens(n: number): string {
  if (n < 1_000)     return String(n)
  if (n < 1_000_000) return `${(n / 1_000).toFixed(1)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}

export default function StatusBar() {
  const paMode          = useSettingsStore(s => s.paMode)
  const sidecarStatus   = useSidecarStore(s => s.status)
  const mcpDisconnected = useSidecarStore(s => s.mcpDisconnected)
  const tokensIn        = useCostStore(s => s.tokensInToday)
  const tokensOut       = useCostStore(s => s.tokensOutToday)
  const totalTokens     = tokensIn + tokensOut

  const modeColor = PA_MODE_COLORS[paMode] ?? 'var(--text-3)'
  const modeLabel = PA_MODE_LABELS[paMode]  ?? paMode

  const orchColor = sidecarStatus === 'running'
    ? 'var(--accent)'
    : sidecarStatus === 'starting'
      ? 'var(--warning)'
      : sidecarStatus.startsWith('error')
        ? 'var(--danger)'
        : '#1A1A1A'

  return (
    <div
      className="flex items-center justify-between shrink-0"
      style={{
        height:     '22px',
        background: 'var(--bg-surface)',
        borderTop:  '1px solid var(--border-1)',
        fontSize:   '10px',
        fontFamily: 'var(--font-mono)',
        padding:    '0 12px',
      }}
    >
      {/* Left: PA mode indicator */}
      <div className="flex items-center gap-1.5">
        <span
          style={{
            width: 6, height: 6, borderRadius: '50%',
            background: modeColor, display: 'inline-block', flexShrink: 0,
          }}
        />
        <span style={{ color: 'var(--text-4)' }}>PA</span>
        <span style={{ color: 'var(--text-4)' }}>·</span>
        <span style={{ color: 'var(--text-3)' }}>{modeLabel}</span>
      </div>

      {/* Middle: sidecar / MCP status */}
      <div className="flex items-center gap-2">
        <span
          style={{
            width: 5, height: 5, borderRadius: '50%',
            background: orchColor, display: 'inline-block',
          }}
        />
        <span style={{ color: 'var(--text-3)' }}>core-orch</span>
        <span style={{ color: 'var(--text-4)' }}>·</span>
        <span style={{ color: 'var(--text-4)' }}>autodev</span>

        {/* Ruflo Feature 6 — MCP disconnected warning */}
        {mcpDisconnected.length > 0 && (
          <>
            <span style={{ color: 'var(--text-4)' }}>·</span>
            <span
              title={`MCP disconnected: ${mcpDisconnected.join(', ')}`}
              style={{
                color:      'var(--danger, #ff6b6b)',
                fontWeight: 600,
                letterSpacing: '0.03em',
              }}
            >
              ⚠ MCP
            </span>
          </>
        )}
      </div>

      {/* Right: token count */}
      <div style={{ color: 'var(--text-3)' }}>
        {totalTokens > 0 ? `${fmtTokens(totalTokens)} tk` : 'autodev v1.0'}
      </div>
    </div>
  )
}
