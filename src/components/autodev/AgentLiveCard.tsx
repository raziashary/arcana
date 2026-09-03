// AgentLiveCard — per-agent live state card in the Live Monitor grid
// Click to expand (shows 12 log lines, spans 2 columns). Click again to collapse.

import { useState, useMemo } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useAgentLiveStore } from '../../stores/agentLiveStore'
import { useTaskStore }      from '../../stores/taskStore'
import { useUiStore }        from '../../stores/uiStore'
import { useUiStore as _useUiStore } from '../../stores/uiStore'
import { useWorkerChatStore } from '../../stores/workerChatStore'
import { StagePipeline } from './StagePipeline'
import { MiniLogTail }   from './MiniLogTail'
import { TokenSparkline } from './TokenSparkline'

void _useUiStore  // tsc — keep import alive without flagged "unused"

interface Props {
  agentId: string
}

// Doc 01 Phase 3 — dot palette.
// `idle` has two flavours: blue (info) when the agent has been seen alive at
// some point, gray when it has never received an event (cold-boot / sidecar
// down). This is what tells the user "the card is ready but nothing happened"
// vs "the card has been working and is now waiting".
const STATUS_DOT_COLORS: Record<string, string> = {
  active:        'var(--accent)',
  idle:          'var(--info)',
  error:         'var(--danger)',
  paused:        '#555555',
  starting:      'var(--warning)',
  // no_provider: amber — the worker is not waiting for work, it is blocked
  //   until an LLM provider key is configured. Distinct from ordinary idle so
  //   the user can tell "this needs me" from "this is fine".
  no_provider:   'var(--warning)',
}
const UNKNOWN_DOT_COLOR = '#555555'      // gray — never seen alive

// Doc 01 Phase 3 — short, friendly model-name pill.
// Workers tend to be configured with the full registry key
// ("claude-haiku-4-5", "claude-sonnet-4-5-thinking", "deepseek-v4-pro" …) —
// renderable but verbose. Compress to a 2–8 char label for the card chip
// while leaving the full string in the `title` attribute on hover.
function shortModelName(model: string | null): string | null {
  if (!model) return null
  const m = model.toLowerCase()
  if (m.includes('haiku'))    return 'haiku'
  if (m.includes('sonnet') && m.includes('thinking')) return 'sonnet⊕'
  if (m.includes('sonnet'))   return 'sonnet'
  if (m.includes('opus'))     return 'opus'
  if (m.includes('deepseek')) return 'deepseek'
  if (m.includes('gpt-4'))    return 'gpt-4'
  if (m.includes('gpt-3'))    return 'gpt-3'
  // Fallback — strip vendor / version cruft, cap to 10 chars.
  return model.replace(/^[a-z]+-/, '').slice(0, 10)
}

export function AgentLiveCard({ agentId }: Props) {
  const agent    = useAgentLiveStore(s => s.agents[agentId])
  const tasks    = useTaskStore(s => s.tasks)
  const pulseTaskColumn = useUiStore(s => s.pulseTaskColumn)
  // ACP unread badge — pending messages addressed to this worker.
  const unreadCount = useWorkerChatStore(s => s.unreadByTarget[agentId] ?? 0)
  const setSelectedTaskId = useWorkerChatStore(s => s.setSelectedTaskId)
  const [expanded, setExpanded] = useState(false)

  // Doc 06 Phase 3 — live count of qc_review tasks for the QC card chip.
  const qcReviewCount = useMemo(
    () => tasks.filter(t => t.status === 'qc_review').length,
    [tasks]
  )

  // Doc 01 Phase 3 — recent tools tail (last 3 distinct names) when there
  // isn't a current_tool to display. Avoids "no activity yet" emptiness on
  // an agent that finished a tick a few seconds ago.
  const recentTools = useMemo(() => {
    if (!agent) return [] as string[]
    const names: string[] = []
    for (let i = agent.log_lines.length - 1; i >= 0 && names.length < 3; i--) {
      const t = agent.log_lines[i].tool_name
      if (t && !names.includes(t)) names.push(t)
    }
    return names
  }, [agent?.log_lines])

  if (!agent) return null

  const isQc = agent.agent_type === 'qc'

  // Doc 01 Phase 3 — pick gray vs colored dot for idle based on
  // whether the agent has ever been seen alive in this session.
  const dotColor =
    agent.status === 'idle' && !agent.has_ever_been_active
      ? UNKNOWN_DOT_COLOR
      : (STATUS_DOT_COLORS[agent.status] ?? UNKNOWN_DOT_COLOR)

  const cardClass = [
    'agent-live-card',
    `agent-live-card--${agent.status}`,
    expanded ? 'agent-live-card--expanded' : '',
  ].filter(Boolean).join(' ')

  function openInLogs(e: React.MouseEvent) {
    e.stopPropagation()
    // `navigate_to_logs` is not a registered command — this click has always
    // done nothing, and the empty `.catch` hid that. Tab navigation is a
    // frontend concern and gets a real implementation with the UI work in P6;
    // until then the failure is at least visible rather than silent.
    // Found by the P0 post-phase audit (F-16).
    invoke('navigate_to_logs', { filterAgent: agentId }).catch((err) => {
      console.error(
        '[AgentLiveCard] "Open in Logs" is not wired up yet (no navigate_to_logs command):',
        err,
      )
    })
  }

  // Doc 06 Phase 3 — clicking the QC count chip scrolls + pulses the
  // qc_review column on the AutoDev task board.
  function jumpToQcReview(e: React.MouseEvent) {
    e.stopPropagation()
    pulseTaskColumn('qc_review')
  }

  const shortModel = shortModelName(agent.model)

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
        {/* ACP unread badge — clicking jumps to Fleet Chat with this agent's
            inbox isolated. Hidden when zero. */}
        {unreadCount > 0 && (
          <span
            onClick={(e) => {
              e.stopPropagation()
              // No task filter — show the all-tasks feed; the user filters
              // by clicking the specific task chip in Fleet Chat's header.
              setSelectedTaskId(null)
              // Switch the observability sub-tab; uses the same localStorage
              // key AutoDevPanel reads on mount.
              try { localStorage.setItem('autodev.obstab', 'fleetchat') } catch { /* noop */ }
              // Trigger a soft reload of the active obs tab by dispatching a
              // storage event — AutoDevPanel listens to its own setState, so
              // we instead just open Fleet Chat by reloading the tab.
              window.dispatchEvent(new Event('storage'))
            }}
            title={`${unreadCount} unread ACP message${unreadCount === 1 ? '' : 's'} for ${agent.display_name}. Click to open Fleet Chat.`}
            style={{
              fontSize:      9,
              padding:       '1px 5px',
              marginLeft:    4,
              background:    'var(--warning, #FFCB8B)',
              color:         '#000',
              borderRadius:  6,
              fontWeight:    700,
              cursor:        'pointer',
              letterSpacing: 0.4,
            }}
          >
            💬 {unreadCount}
          </span>
        )}
        {shortModel && (
          <span
            className="card-model-badge"
            title={agent.model ?? ''}
            style={{
              fontSize:   '9px',
              padding:    '1px 4px',
              marginLeft: '4px',
              border:     '1px solid var(--border-1)',
              borderRadius: '3px',
              color:      'var(--text-2)',
              letterSpacing: '0.04em',
              textTransform: 'uppercase',
            }}
          >
            {shortModel}
          </span>
        )}
        {/* Doc 06 Phase 3 — qc_review count chip on the QC card. Always
            visible; clicking jumps to the qc_review column. Even at zero,
            the chip explains why the card looks idle. */}
        {isQc && (
          <span
            className="card-qc-count-chip"
            onClick={jumpToQcReview}
            title={qcReviewCount === 0
              ? 'No tasks awaiting QC review (click to jump to column)'
              : `${qcReviewCount} task${qcReviewCount === 1 ? '' : 's'} awaiting QC review — click to jump`}
            style={{
              fontSize:   '9px',
              padding:    '1px 5px',
              marginLeft: 'auto',
              borderRadius: '3px',
              cursor:     'pointer',
              background: qcReviewCount > 0 ? 'var(--info)' : 'var(--bg-surface)',
              color:      qcReviewCount > 0 ? '#000' : 'var(--text-2)',
              border:     '1px solid var(--border-1)',
              letterSpacing: '0.05em',
            }}
          >
            QC: {qcReviewCount}
          </span>
        )}
      </div>

      {/* ── Stage pipeline ─────────────────────────────────────────── */}
      <div className="card-divider" />
      <StagePipeline stages={agent.stages} />

      {/* ── Currently reviewing task (QC card, Doc 06 Phase 3) ───── */}
      {isQc && agent.task_id != null && (
        <>
          <div className="card-divider" />
          <div
            className="card-qc-reviewing"
            onClick={(e) => { e.stopPropagation(); pulseTaskColumn('qc_review') }}
            style={{
              fontSize: '10px',
              color:    'var(--text-2)',
              padding:  '2px 6px',
              cursor:   'pointer',
            }}
            title="Click to jump to this task in the QC Review column"
          >
            reviewing <span style={{ color: 'var(--accent)' }}>#{agent.task_id}</span>
          </div>
        </>
      )}

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

      {/* ── Recent tools tail (Doc 01 Phase 3) ─────────────────────
           Shown only when no tool is in flight AND we have a non-empty
           history. Distinguishes "agent is between ticks" from "agent
           has never produced any activity at all". */}
      {!agent.current_tool && recentTools.length > 0 && (
        <div
          className="card-recent-tools"
          style={{
            fontSize:   '9px',
            color:      'var(--text-2)',
            padding:    '2px 6px',
            letterSpacing: '0.04em',
            display:    'flex',
            gap:        '4px',
            flexWrap:   'wrap',
          }}
        >
          <span style={{ opacity: 0.6 }}>last:</span>
          {recentTools.map((t, i) => (
            <span
              key={`${t}-${i}`}
              style={{
                padding: '0 4px',
                borderRadius: '2px',
                background: 'var(--bg-surface)',
                border: '1px solid var(--border-1)',
              }}
            >
              {t}
            </span>
          ))}
        </div>
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
