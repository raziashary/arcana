// McpHealthPanel — Ruflo Feature 6. Shows connection state badges for all
// registered external MCP servers. Displayed inside SidecarPanel beneath the
// managed services list. Users can manually trigger reconnects from here.
//
// State badges:
//   🟢 Connected (Xms avg)
//   🟡 Degraded (Xms avg)
//   🔴 Disconnected (N failures) [Reconnect]
//   🔄 Reconnecting…
//   ⬜ Not Monitored (stdio)

import { useEffect, useState } from 'react'
import { RefreshCw, RotateCcw } from 'lucide-react'
import { useMcpHealthStore, type McpServerHealth, type McpConnectionState } from '../../stores/mcpHealthStore'

// ─── Helpers ─────────────────────────────────────────────────────────────────

function stateColor(state: McpConnectionState): string {
  switch (state) {
    case 'connected':      return 'var(--aqua, #7fdbff)'
    case 'degraded':       return 'var(--warning, #FFCB8B)'
    case 'disconnected':   return 'var(--danger, #ff6b6b)'
    case 'reconnecting':   return 'var(--warning, #FFCB8B)'
    case 'not_monitored':  return 'var(--text-4)'
    default:               return 'var(--text-4)'
  }
}

function stateLabel(h: McpServerHealth): string {
  switch (h.state) {
    case 'connected':
      return h.avg_latency_ms > 0
        ? `Connected (${Math.round(h.avg_latency_ms)}ms avg)`
        : 'Connected'
    case 'degraded':
      return `Degraded (${Math.round(h.avg_latency_ms)}ms avg)`
    case 'disconnected':
      return `Disconnected (${h.consecutive_failures} failure${h.consecutive_failures !== 1 ? 's' : ''})`
    case 'reconnecting':
      return 'Reconnecting…'
    case 'not_monitored':
      return 'Stdio — not monitored'
    default:
      return h.state
  }
}

function StateDot({ state }: { state: McpConnectionState }) {
  return (
    <span
      style={{
        width: 7, height: 7, borderRadius: '50%',
        background: stateColor(state),
        display: 'inline-block', flexShrink: 0,
        animation: state === 'reconnecting' ? 'pulse 1.2s ease-in-out infinite' : 'none',
      }}
    />
  )
}

// ─── Single server row ────────────────────────────────────────────────────────

function ServerRow({ health, onReconnect }: {
  health:      McpServerHealth
  onReconnect: (name: string) => Promise<void>
}) {
  const [reconnecting, setReconnecting] = useState(false)

  const handleReconnect = async () => {
    setReconnecting(true)
    try {
      await onReconnect(health.server_name)
    } finally {
      setReconnecting(false)
    }
  }

  return (
    <div
      style={{
        display:     'flex',
        alignItems:  'center',
        gap:         8,
        padding:     '5px 8px',
        background:  'var(--bg-card)',
        borderRadius: 2,
        fontSize:    11,
      }}
    >
      <StateDot state={health.state} />

      {/* Server name */}
      <span
        style={{
          flex: 1, color: 'var(--text-2)',
          fontFamily: 'monospace', overflow: 'hidden',
          textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}
      >
        {health.server_name}
      </span>

      {/* State label */}
      <span
        style={{
          fontSize: 10, color: stateColor(health.state),
          fontFamily: 'monospace', flexShrink: 0,
        }}
      >
        {stateLabel(health)}
      </span>

      {/* Reconnect button — only for disconnected HTTP servers */}
      {(health.state === 'disconnected' || health.state === 'reconnecting') && health.transport === 'http' && (
        <button
          onClick={handleReconnect}
          disabled={reconnecting}
          title="Manually trigger reconnect"
          style={{
            display:    'flex', alignItems: 'center', gap: 3,
            padding:    '2px 6px',
            background: 'rgba(255,107,107,0.1)',
            border:     '1px solid rgba(255,107,107,0.3)',
            borderRadius: 2,
            color:      'var(--danger, #ff6b6b)',
            fontSize:   9,
            fontFamily: 'monospace',
            cursor:     reconnecting ? 'default' : 'pointer',
            flexShrink: 0,
          }}
        >
          <RotateCcw size={8} className={reconnecting ? 'anim-spin' : ''} />
          <span>Reconnect</span>
        </button>
      )}

      {/* Requires-restart badge for stdio servers */}
      {health.state === 'disconnected' && health.transport === 'stdio' && (
        <span
          style={{
            fontSize: 9, padding: '1px 5px', borderRadius: 2,
            background: 'rgba(255,203,139,0.12)', color: 'var(--warning)',
            border: '1px solid rgba(255,203,139,0.25)', fontFamily: 'monospace',
          }}
        >
          needs restart
        </span>
      )}
    </div>
  )
}

// ─── Main panel ──────────────────────────────────────────────────────────────

export function McpHealthPanel() {
  const { servers: mcpServerHealth, load: loadMcpHealth } = useMcpHealthStore()
  const [loading, setLoading] = useState(false)

  // Load on mount
  useEffect(() => {
    loadMcpHealth()
  }, [loadMcpHealth])

  const handleRefresh = async () => {
    setLoading(true)
    try { await loadMcpHealth() } finally { setLoading(false) }
  }

  const handleReconnect = async (serverName: string) => {
    await useMcpHealthStore.getState().reconnect(serverName)
    // Reload after a short delay to pick up the new state
    setTimeout(() => loadMcpHealth(), 1500)
  }

  if (mcpServerHealth.length === 0) return null

  return (
    <div
      style={{
        borderTop:  '1px solid var(--border-1)',
        padding:    '8px 10px',
      }}
    >
      {/* Section header */}
      <div
        style={{
          display:       'flex',
          alignItems:    'center',
          justifyContent:'space-between',
          marginBottom:  6,
        }}
      >
        <span
          style={{
            fontSize:      9,
            color:         'var(--text-4)',
            letterSpacing: '0.1em',
            textTransform: 'uppercase',
            fontFamily:    'monospace',
          }}
        >
          External MCP Servers
        </span>
        <button
          onClick={handleRefresh}
          disabled={loading}
          title="Refresh MCP health"
          style={{
            background: 'none', border: 'none', cursor: loading ? 'default' : 'pointer',
            color: 'var(--text-4)', padding: 2,
          }}
        >
          <RefreshCw size={9} className={loading ? 'anim-spin' : ''} />
        </button>
      </div>

      {/* Server rows */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
        {mcpServerHealth.map(h => (
          <ServerRow
            key={h.server_name}
            health={h}
            onReconnect={handleReconnect}
          />
        ))}
      </div>
    </div>
  )
}
