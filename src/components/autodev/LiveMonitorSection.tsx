// LiveMonitorSection — collapsible fleet monitor between Agent Roster and Task Board
// Manages Tauri event subscriptions for the entire Live Monitor feature.

import { useEffect, useState, useRef } from 'react'
import { useAgentLiveStore } from '../../stores/agentLiveStore'
import { AgentLiveCard }     from './AgentLiveCard'

export function LiveMonitorSection() {
  const {
    agents,
    isMonitorCollapsed,
    setMonitorCollapsed,
    subscribeToEvents,
    lastEventAt,
  } = useAgentLiveStore()

  // Tick every second so "Xs ago" stays live
  const [, tick] = useState(0)
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 1_000)
    return () => clearInterval(id)
  }, [])

  // Subscribe to Tauri events once on mount
  const cleanupRef = useRef<(() => void) | null>(null)
  useEffect(() => {
    cleanupRef.current = subscribeToEvents()
    return () => { cleanupRef.current?.() }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const agentList   = Object.values(agents)
  const secondsAgo  = Math.floor((Date.now() - lastEventAt) / 1000)
  const hasActive   = agentList.some(a => a.status === 'active')
  const isLive      = hasActive && secondsAgo < 30
  const isStale     = !isLive && secondsAgo < 120
  const isOffline   = !isLive && !isStale

  function timeAgo(): string {
    if (secondsAgo < 60) return `${secondsAgo}s ago`
    const mins = Math.floor(secondsAgo / 60)
    return `${mins}m ago`
  }

  return (
    <div className="live-monitor-section">
      {/* ── Section header ───────────────────────────────────────── */}
      <div className="lm-header">
        <span className="lm-section-label">LIVE MONITOR</span>
        <div className="lm-header-right">
          {isLive    && <span className="live-badge live-badge--live">◉ LIVE</span>}
          {isStale   && <span className="live-badge live-badge--stale">◉ STALE</span>}
          {isOffline && <span className="live-badge live-badge--offline">◉ OFFLINE</span>}
          {!isLive && (
            <span className="lm-last-event">{timeAgo()}</span>
          )}
          <button
            className="lm-collapse-btn"
            onClick={() => setMonitorCollapsed(!isMonitorCollapsed)}
            aria-label={isMonitorCollapsed ? 'Expand live monitor' : 'Collapse live monitor'}
          >
            {isMonitorCollapsed ? '▲' : '▼'}
          </button>
        </div>
      </div>

      {/* ── Card grid ────────────────────────────────────────────── */}
      {!isMonitorCollapsed && (
        <div className="live-monitor-grid">
          {agentList.length === 0 ? (
            <div className="lm-empty">No agents initialised — start a worker to see live data</div>
          ) : (
            agentList.map(agent => (
              <AgentLiveCard key={agent.agent_id} agentId={agent.agent_id} />
            ))
          )}
        </div>
      )}
    </div>
  )
}
