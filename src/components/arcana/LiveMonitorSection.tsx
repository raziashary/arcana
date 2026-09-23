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

  const agentList    = Object.values(agents)
  const secondsAgo   = Math.floor((Date.now() - lastEventAt) / 1000)
  const hasActive    = agentList.some(a => a.status === 'active')
  // "online" = at least one agent is registered and not stopped (sidecars are up)
  // ⚠ This was `!== 'stopped'`, and nothing ever produced that status — so the
  // condition reduced to `agentList.length > 0` and could not distinguish an
  // online fleet from a fully paused one (`P8-43`).
  const hasAnyOnline = agentList.length > 0 && agentList.some(a => a.status !== 'paused')

  const isLive    = hasActive && secondsAgo < 30
  // STALE only when there WAS recent activity that just went quiet (not blank-slate idle)
  const isStale   = !isLive && hasActive && secondsAgo < 120
  // IDLE = agents are up and registered but no tasks are running right now
  const isIdle    = !isLive && !isStale && hasAnyOnline
  // OFFLINE = sidecars are down / no agents initialised / all workers stopped
  const isOffline = !isLive && !isStale && !hasAnyOnline

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
          {isIdle    && <span className="live-badge live-badge--idle">◉ IDLE</span>}
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
