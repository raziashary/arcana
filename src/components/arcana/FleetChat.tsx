// FleetChat — visualisation panel for the Arcana Communication Protocol.
//
// Slack-style transcript of every worker↔worker message. Two view modes:
//   * "All tasks" — chronological feed across the whole fleet (default).
//   * "By task"  — a single task's conversation thread, opened by clicking
//     a task chip in the header or by passing `taskId` via the store.
//
// The panel collapses to a header strip when there are zero messages, so it
// doesn't take board real estate on a fresh install.

import { useEffect, useMemo, useState } from 'react'
import {
  useWorkerChatStore, type WorkerMessage, type WorkerMessageKind,
} from '../../stores/workerChatStore'
import { useTaskStore } from '../../stores/taskStore'
import { parseDbTimestamp } from '../../utils/dbTime'

const KIND_PALETTE: Record<WorkerMessageKind, { fg: string; bg: string; label: string }> = {
  nudge:     { fg: '#7FDBCA', bg: 'rgba(127,219,202,0.08)', label: 'NUDGE' },
  question:  { fg: '#FFCB8B', bg: 'rgba(255,203,139,0.10)', label: 'QUESTION' },
  answer:    { fg: '#82AAFF', bg: 'rgba(130,170,255,0.08)', label: 'ANSWER' },
  fyi:       { fg: '#888888', bg: 'rgba(255,255,255,0.04)', label: 'FYI' },
  broadcast: { fg: '#C792EA', bg: 'rgba(199,146,234,0.10)', label: 'BROADCAST' },
}

function timeAgo(iso: string): string {
  const timestamp = parseDbTimestamp(iso)
  if (!timestamp) return iso
  const t = timestamp.getTime()
  const dt = Date.now() - t
  if (dt < 0)            return 'just now'
  if (dt < 60_000)       return `${Math.floor(dt / 1000)}s ago`
  if (dt < 3_600_000)    return `${Math.floor(dt / 60_000)}m ago`
  if (dt < 86_400_000)   return `${Math.floor(dt / 3_600_000)}h ago`
  return timestamp.toLocaleString()
}

interface Props {
  /** Optional: clamp the chat to a specific task. When null, shows all-tasks feed. */
  taskId?: number | null
}

export function FleetChat({ taskId = null }: Props) {
  const messages       = useWorkerChatStore((s) => s.messages)
  const selectedTaskId = useWorkerChatStore((s) => s.selectedTaskId)
  const setSelectedTaskId = useWorkerChatStore((s) => s.setSelectedTaskId)
  const hydrate = useWorkerChatStore((s) => s.hydrate)
  const initialized = useWorkerChatStore((s) => s.initialized)
  const tasks = useTaskStore((s) => s.tasks)

  const [showCollapsed, setShowCollapsed] = useState(false)

  // Override-priority: prop > store selection > all tasks.
  const effectiveTaskId = taskId ?? selectedTaskId

  useEffect(() => {
    if (!initialized) {
      void hydrate()
    }
  }, [initialized, hydrate])

  const filtered = useMemo<WorkerMessage[]>(() => {
    if (effectiveTaskId === null) return messages
    return messages.filter((m) => m.task_id === effectiveTaskId)
  }, [messages, effectiveTaskId])

  const distinctTaskIds = useMemo(() => {
    const ids = new Set<number>()
    for (const m of messages) {
      if (m.task_id !== null) ids.add(m.task_id)
    }
    return [...ids].sort((a, b) => a - b)
  }, [messages])

  if (messages.length === 0 && showCollapsed) {
    return (
      <div
        style={{
          padding:       '8px 12px',
          background:    'var(--bg-2, #111)',
          border:        '1px solid var(--border, #222)',
          borderRadius:  2,
          color:         'var(--text-secondary, #666)',
          fontSize:      11,
          fontFamily:    'var(--font-mono, monospace)',
        }}
      >
        <span style={{ marginRight: 8 }}>📡 Fleet Chat — no messages yet</span>
        <button
          onClick={() => setShowCollapsed(false)}
          style={{
            background: 'transparent',
            border:     'none',
            color:      'var(--accent, #00FF88)',
            cursor:     'pointer',
            fontSize:   10,
          }}
        >
          [expand]
        </button>
      </div>
    )
  }

  return (
    <div
      style={{
        background:    'var(--bg-1, #0A0A0A)',
        border:        '1px solid var(--border, #222)',
        borderRadius:  2,
        padding:       12,
        fontFamily:    'var(--font-mono, monospace)',
        fontSize:      11,
        color:         'var(--text-primary, #F0F0F0)',
      }}
    >
      {/* Header: task chips + collapse */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 10, flexWrap: 'wrap' }}>
        <span style={{ fontSize: 10, textTransform: 'uppercase', color: 'var(--text-secondary, #666)' }}>
          📡 Fleet Chat
        </span>
        <button
          onClick={() => setSelectedTaskId(null)}
          style={{
            background:    effectiveTaskId === null ? 'var(--accent, #00FF88)' : 'transparent',
            color:         effectiveTaskId === null ? '#000' : 'var(--text-secondary, #888)',
            border:        '1px solid var(--border, #333)',
            borderRadius:  2,
            padding:       '2px 6px',
            cursor:        'pointer',
            fontSize:      10,
          }}
        >
          All ({messages.length})
        </button>
        {distinctTaskIds.map((id) => {
          const task = tasks.find((t) => t.id === id)
          const count = messages.filter((m) => m.task_id === id).length
          const active = effectiveTaskId === id
          return (
            <button
              key={id}
              onClick={() => setSelectedTaskId(id)}
              title={task?.title ?? `Task #${id}`}
              style={{
                background:   active ? 'var(--accent, #00FF88)' : 'transparent',
                color:        active ? '#000' : 'var(--text-secondary, #888)',
                border:       '1px solid var(--border, #333)',
                borderRadius: 2,
                padding:      '2px 6px',
                cursor:       'pointer',
                fontSize:     10,
              }}
            >
              #{id} ({count})
            </button>
          )
        })}
        <div style={{ marginLeft: 'auto' }}>
          {messages.length === 0 && (
            <button
              onClick={() => setShowCollapsed(true)}
              style={{
                background: 'transparent', border: 'none',
                color: 'var(--text-secondary, #666)', cursor: 'pointer', fontSize: 10,
              }}
            >
              [collapse]
            </button>
          )}
        </div>
      </div>

      {/* Transcript */}
      <div
        style={{
          maxHeight:  240,
          overflowY:  'auto',
          display:    'flex',
          flexDirection: 'column',
          gap:        6,
        }}
      >
        {filtered.length === 0 && (
          <div style={{ color: 'var(--text-secondary, #666)', fontStyle: 'italic' }}>
            {effectiveTaskId === null
              ? 'No messages yet. Workers will start posting here when they need to coordinate.'
              : `No messages on task #${effectiveTaskId}.`}
          </div>
        )}
        {filtered.map((m) => {
          const palette = KIND_PALETTE[m.kind] ?? KIND_PALETTE.nudge
          return (
            <div
              key={m.id}
              style={{
                display:       'grid',
                gridTemplateColumns: 'auto 1fr auto',
                gap:           8,
                alignItems:    'baseline',
                padding:       '6px 8px',
                background:    palette.bg,
                borderLeft:    `2px solid ${palette.fg}`,
                borderRadius:  2,
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: 6, minWidth: 0 }}>
                <span style={{ color: palette.fg, fontWeight: 600 }}>{m.from_slug}</span>
                <span style={{ color: 'var(--text-secondary, #666)' }}>→</span>
                <span style={{ color: 'var(--text-primary, #F0F0F0)' }}>{m.to_slug}</span>
                <span
                  style={{
                    fontSize:      9,
                    letterSpacing: 0.5,
                    color:         palette.fg,
                    padding:       '1px 4px',
                    border:        `1px solid ${palette.fg}`,
                    borderRadius:  2,
                    marginLeft:    4,
                  }}
                >
                  {palette.label}
                </span>
                {m.task_id !== null && (
                  <span
                    style={{ fontSize: 9, color: 'var(--text-secondary, #888)', marginLeft: 4 }}
                  >
                    #{m.task_id}
                  </span>
                )}
              </div>
              <div style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{m.body}</div>
              <div
                title={m.created_at}
                style={{ fontSize: 10, color: 'var(--text-secondary, #666)', whiteSpace: 'nowrap' }}
              >
                {timeAgo(m.created_at)}
                {m.status !== 'pending' && (
                  <span style={{ marginLeft: 4 }}>
                    {m.status === 'read' ? '· read' : '· ✓'}
                  </span>
                )}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

export default FleetChat
