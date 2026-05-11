// AgentLiveCard — per-agent live state card in the Live Monitor grid
// Click to expand (shows 12 log lines, spans 2 columns). Click again to collapse.

import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAgentLiveStore } from '../../stores/agentLiveStore'
import { StagePipeline } from './StagePipeline'
import { MiniLogTail }   from './MiniLogTail'
import { TokenSparkline } from './TokenSparkline'

interface Props {
  agentId: string
}

const STATUS_DOT_COLORS: Record<string, string> = {
  active:   'var(--accent)',
  idle:     'var(--info)',
  error:    'var(--danger)',
  stopped:  '#555555',
  starting: 'var(--warning)',
}

export function AgentLiveCard({ agentId }: Props) {
  const agent    = useAgentLiveStore(s => s.agents[agentId])
  const [expanded, setExpanded] = useState(false)

  if (!agent) return null

  const dotColor = STATUS_DOT_COLORS[agent.status] ?? '#555555'

  const cardClass = [
    'agent-live-card',
    `agent-live-card--${agent.status}`,
    expanded ? 'agent-live-card--expanded' : '',
  ].filter(Boolean).join(' ')

  function openInLogs(e: React.MouseEvent) {
    e.stopPropagation()
    invoke('navigate_to_logs', { filterAgent: agentId }).catch(() => {})
  }

  return (
    <div
      className={cardClass}
      style={expanded ? { gridColumn: 'span 2' } : undefined}
      onClick={() => setExpanded(v => !v)}
    >
      {/* ── Card header ────────────────────────────────────────────── */}
      <div className="card-header">
        <span
          className="card-status-dot"
          style={{
            background: dotColor,
            boxShadow: agent.status === 'active' || agent.status === 'starting'
              ? `0 0 5px ${dotColor}` : 'none',
          }}
        />
        <span className="card-agent-name">{agent.display_name}</span>
        <span className="card-role-badge">{agent.agent_type.toUpperCase()}</span>
      </div>

      {/* ── Stage pipeline ─────────────────────────────────────────── */}
      <div className="card-divider" />
      <StagePipeline stages={agent.stages} />

      {/* ── Current tool + file ────────────────────────────────────── */}
      {(agent.current_tool || agent.current_file) && (
        <>
          <div className="card-divider" />
          <div className="card-current-action">
            {agent.current_tool && (
              <span className="current-tool-name">{agent.current_tool}</span>
            )}
            {agent.current_file && (
              <span className="current-file-name">
                {agent.current_file}
                {agent.current_file_diff && (
                  <span className="diff-badge">{agent.current_file_diff}</span>
                )}
              </span>
            )}
          </div>
        </>
      )}

      {/* ── Token bar + sparkline ──────────────────────────────────── */}
      <TokenSparkline totalTokens={agent.total_tokens} sparkline={agent.sparkline} />

      {/* ── Log tail ───────────────────────────────────────────────── */}
      <MiniLogTail lines={agent.log_lines} maxLines={expanded ? 12 : 3} />

      {/* ── Error message ──────────────────────────────────────────── */}
      {agent.error_message && (
        <div className="card-error-message">
          ✗ {agent.error_message}
        </div>
      )}

      {/* ── Expanded footer ────────────────────────────────────────── */}
      {expanded && (
        <div className="card-expanded-footer" onClick={e => e.stopPropagation()}>
          <button className="card-action-btn" onClick={openInLogs}>
            View full log ↗
          </button>
          {agent.task_id != null && (
            <span className="card-task-id">Task #{agent.task_id}</span>
          )}
        </div>
      )}
    </div>
  )
}
