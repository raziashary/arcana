// EpicPhaseView — Ruflo Feature 5. Renders per-phase progress for a single Epic task.
// Shows phase name, milestone badge, task counts, completion state, and an unlock gating
// indicator. Listens to task:phase-complete and task:unblocked for live updates.

import { useState, useEffect, useCallback } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'
import { listen } from '@tauri-apps/api/event'
import { type Task, type PhaseSummary, useTaskStore } from '../../stores/taskStore'

// ─── Sub-components ──────────────────────────────────────────────────────────

function PhaseProgressBar({ done, total }: { done: number; total: number }) {
  const pct = total === 0 ? 0 : Math.round((done / total) * 100)
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 6, flex: 1 }}>
      <div
        style={{
          flex: 1,
          height: 4,
          background: 'var(--bg-card)',
          borderRadius: 2,
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            width: `${pct}%`,
            height: '100%',
            background: pct === 100 ? 'var(--success)' : 'var(--accent)',
            borderRadius: 2,
            transition: 'width 0.3s ease',
          }}
        />
      </div>
      <span style={{ fontSize: 9, color: 'var(--text-4)', fontFamily: 'monospace', minWidth: 28 }}>
        {done}/{total}
      </span>
    </div>
  )
}

function PhaseBadge({ label, color }: { label: string; color: string }) {
  return (
    <span
      style={{
        fontSize: 8,
        padding: '1px 5px',
        borderRadius: 3,
        background: `${color}22`,
        color,
        fontFamily: 'monospace',
        textTransform: 'uppercase',
        letterSpacing: '0.06em',
        border: `1px solid ${color}44`,
      }}
    >
      {label}
    </span>
  )
}

function PhaseRow({ summary }: { summary: PhaseSummary }) {
  const pct = summary.total_tasks === 0
    ? 0
    : Math.round((summary.done_tasks / summary.total_tasks) * 100)

  const statusColor = summary.is_complete
    ? 'var(--success)'
    : !summary.is_unlocked
      ? 'var(--text-4)'
      : 'var(--accent)'

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: '120px 1fr 80px 60px',
        alignItems: 'center',
        gap: 8,
        padding: '5px 8px',
        background: summary.is_complete
          ? 'rgba(127,219,202,0.04)'
          : !summary.is_unlocked
            ? 'rgba(0,0,0,0.15)'
            : 'var(--bg-card)',
        borderRadius: 2,
        opacity: !summary.is_unlocked && !summary.is_complete ? 0.55 : 1,
      }}
    >
      {/* Phase name + milestone */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <span
          style={{
            width: 6,
            height: 6,
            borderRadius: '50%',
            background: statusColor,
            flexShrink: 0,
          }}
        />
        <span
          style={{
            fontSize: 10,
            color: summary.is_unlocked ? 'var(--text-1)' : 'var(--text-3)',
            fontFamily: 'monospace',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {summary.phase}
        </span>
        {summary.milestone && (
          <PhaseBadge label={summary.milestone} color="#7FDBCA" />
        )}
      </div>

      {/* Progress bar */}
      <PhaseProgressBar done={summary.done_tasks} total={summary.total_tasks} />

      {/* Counts */}
      <div
        style={{
          display: 'flex',
          gap: 4,
          fontSize: 9,
          fontFamily: 'monospace',
          color: 'var(--text-4)',
          justifyContent: 'flex-end',
        }}
      >
        {summary.in_progress_tasks > 0 && (
          <span style={{ color: 'var(--accent)' }}>{summary.in_progress_tasks}▶</span>
        )}
        {summary.blocked_tasks > 0 && (
          <span style={{ color: '#F59E0B' }}>{summary.blocked_tasks}⊘</span>
        )}
      </div>

      {/* Status chip */}
      <div style={{ display: 'flex', justifyContent: 'flex-end' }}>
        {summary.is_complete ? (
          <PhaseBadge label="done" color="var(--success)" />
        ) : !summary.is_unlocked ? (
          <PhaseBadge label="locked" color="var(--text-4)" />
        ) : pct > 0 ? (
          <PhaseBadge label={`${pct}%`} color="var(--accent)" />
        ) : (
          <PhaseBadge label="ready" color="#82AAFF" />
        )}
      </div>
    </div>
  )
}

// ─── Main Component ───────────────────────────────────────────────────────────

interface EpicPhaseViewProps {
  epic: Task
}

export function EpicPhaseView({ epic }: EpicPhaseViewProps) {
  const [expanded, setExpanded] = useState(false)
  const [summaries, setSummaries] = useState<PhaseSummary[]>([])
  const [loading,   setLoading]   = useState(false)

  const { getEpicPhaseSummary } = useTaskStore()

  const loadSummaries = useCallback(async () => {
    setLoading(true)
    try {
      const data = await getEpicPhaseSummary(String(epic.id))
      setSummaries(data)
    } finally {
      setLoading(false)
    }
  }, [epic.id, getEpicPhaseSummary])

  // Load when expanded
  useEffect(() => {
    if (expanded) loadSummaries()
  }, [expanded, loadSummaries])

  // Live refresh on phase-complete or task-unblocked events
  useEffect(() => {
    const unlistenPhase = listen<{ epic_id: string }>('task:phase-complete', e => {
      if (e.payload.epic_id === String(epic.id) && expanded) loadSummaries()
    })
    const unlistenUnblocked = listen<{ task_id: number }>('task:unblocked', () => {
      if (expanded) loadSummaries()
    })
    return () => {
      unlistenPhase.then(f => f())
      unlistenUnblocked.then(f => f())
    }
  }, [epic.id, expanded, loadSummaries])

  const totalTasks = summaries.reduce((a, s) => a + s.total_tasks, 0)
  const doneTasks  = summaries.reduce((a, s) => a + s.done_tasks,  0)
  const pct        = totalTasks === 0 ? 0 : Math.round((doneTasks / totalTasks) * 100)

  return (
    <div
      style={{
        border:       '1px solid var(--border-1)',
        borderRadius: 3,
        marginBottom: 6,
        background:   'var(--bg-surface)',
        overflow:     'hidden',
      }}
    >
      {/* Epic header row */}
      <div
        onClick={() => setExpanded(e => !e)}
        style={{
          display:    'flex',
          alignItems: 'center',
          gap:        8,
          padding:    '6px 10px',
          cursor:     'pointer',
          userSelect: 'none',
        }}
      >
        {expanded
          ? <ChevronDown size={10} color="var(--text-4)" />
          : <ChevronRight size={10} color="var(--text-4)" />
        }

        {/* Epic badge */}
        <PhaseBadge label="epic" color="#C3A6FF" />

        {/* Title */}
        <span
          style={{
            flex:         1,
            fontSize:     11,
            color:        'var(--text-1)',
            fontFamily:   'monospace',
            overflow:     'hidden',
            textOverflow: 'ellipsis',
            whiteSpace:   'nowrap',
          }}
        >
          #{epic.id} — {epic.title}
        </span>

        {/* Mini progress */}
        {totalTasks > 0 && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
            <div
              style={{
                width: 40, height: 3,
                background: 'var(--bg-card)',
                borderRadius: 2,
                overflow: 'hidden',
              }}
            >
              <div
                style={{
                  width: `${pct}%`, height: '100%',
                  background: pct === 100 ? 'var(--success)' : 'var(--accent)',
                  borderRadius: 2,
                }}
              />
            </div>
            <span style={{ fontSize: 9, color: 'var(--text-4)', fontFamily: 'monospace' }}>
              {pct}%
            </span>
          </div>
        )}

        {/* Status badge */}
        {epic.status === 'done' && <PhaseBadge label="done" color="var(--success)" />}
      </div>

      {/* Phase rows */}
      {expanded && (
        <div style={{ borderTop: '1px solid var(--border-1)', padding: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
          {loading ? (
            <div style={{ textAlign: 'center', padding: '8px 0', color: 'var(--text-4)', fontSize: 10 }}>
              loading phases…
            </div>
          ) : summaries.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '8px 0', color: 'var(--text-4)', fontSize: 10 }}>
              No phases yet — add subtasks via create_phased_task
            </div>
          ) : (
            summaries.map(s => (
              <PhaseRow key={`${s.phase_order}-${s.phase}`} summary={s} />
            ))
          )}
        </div>
      )}
    </div>
  )
}

// ─── Epic Section (list of all epics) ────────────────────────────────────────

export function EpicsSection({ tasks }: { tasks: Task[] }) {
  const [expanded, setExpanded] = useState(false)
  const epics = tasks.filter(t => t.task_type === 'epic')

  if (epics.length === 0) return null

  return (
    <div
      style={{
        borderBottom: '1px solid var(--border-1)',
        background:   'rgba(195,166,255,0.03)',
      }}
    >
      <div
        className="flex items-center justify-between px-4 py-2 cursor-pointer"
        onClick={() => setExpanded(e => !e)}
      >
        <div className="flex items-center gap-2">
          <span style={{ width: 6, height: 6, borderRadius: '50%', background: '#C3A6FF', display: 'inline-block' }} />
          <span style={{ fontSize: '11px', color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
            Epics
          </span>
          <span
            style={{
              fontSize: '9px',
              background: 'rgba(195,166,255,0.15)',
              color: '#C3A6FF',
              padding: '1px 6px',
              borderRadius: 4,
              fontFamily: 'monospace',
            }}
          >
            {epics.length}
          </span>
        </div>
        {expanded ? <ChevronDown size={12} color="var(--text-4)" /> : <ChevronRight size={12} color="var(--text-4)" />}
      </div>

      {expanded && (
        <div className="px-4 pb-3">
          {epics.map(epic => (
            <EpicPhaseView key={epic.id} epic={epic} />
          ))}
        </div>
      )}
    </div>
  )
}
