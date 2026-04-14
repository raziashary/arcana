// AutoDev tab — Internal Task Board (Kanban) + Agent Roster.
// Layout matches the autodevmockup.html spec exactly:
//   - Vertical roster table: status dot | agent name | current task | elapsed
//   - 4-column kanban: Ready | In Progress | QC Review | Done Today
//   - In Progress cards have progress bars
//   - Metrics footer: Cost Today | Tokens | Closed | QC Pass Rate

import { useEffect, useState, useCallback } from 'react'
import { RefreshCw, Trash2, ChevronDown, ChevronRight, Plus } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import {
  useTaskStore,
  priorityLabel,
  type Task,
} from '../stores/taskStore'

// ─── Helpers ─────────────────────────────────────────────────────────────────

function isToday(dateStr: string | null): boolean {
  if (!dateStr) return false
  const d = new Date(dateStr)
  const now = new Date()
  return (
    d.getFullYear() === now.getFullYear() &&
    d.getMonth()    === now.getMonth()    &&
    d.getDate()     === now.getDate()
  )
}

function elapsedSince(dateStr: string): string {
  const diffMs  = Date.now() - new Date(dateStr).getTime()
  const diffMin = Math.floor(diffMs / 60_000)
  const diffSec = Math.floor((diffMs % 60_000) / 1_000)
  return `${String(diffMin).padStart(2, '0')}:${String(diffSec).padStart(2, '0')}`
}

// ─── Agent Roster (vertical table) ───────────────────────────────────────────

const AGENT_DEFS = [
  { id: 'supervisor',       label: 'SUPERVISOR'       },
  { id: 'worker-backend',   label: 'WORKER-BACKEND'   },
  { id: 'worker-frontend',  label: 'WORKER-FRONTEND'  },
  { id: 'worker-platform',  label: 'WORKER-PLATFORM'  },
  { id: 'worker-mcp',       label: 'WORKER-MCP'       },
  { id: 'worker-cleanup',   label: 'WORKER-CLEANUP'   },
  { id: 'qc',               label: 'QC AGENT'         },
]

function AgentRoster({ tasks }: { tasks: Task[] }) {
  const [, tick] = useState(0)

  // Re-render every second so elapsed timers update live
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 1_000)
    return () => clearInterval(id)
  }, [])

  return (
    <div
      className="shrink-0 border-b font-mono"
      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-1)' }}
    >
      {AGENT_DEFS.map(agent => {
        const active = tasks.find(
          t => t.assigned_worker === agent.id && t.status === 'in_progress'
        )
        const isWorking  = !!active
        const taskLabel  = active
          ? `#${active.id} — ${active.title}`
          : agent.id === 'supervisor'
            ? 'Polling · 0 tasks queued'
            : 'Idle — awaiting tasks'
        const dotColor   = isWorking ? '#FFAA00' : '#4477FF'
        const elapsed    = active ? elapsedSince(active.updated_at) : '—'

        return (
          <div
            key={agent.id}
            className="flex items-center gap-2 px-3"
            style={{
              borderBottom: '1px solid #111',
              padding: '7px 10px',
              fontSize: '11px',
            }}
          >
            <div
              className="shrink-0 rounded-full"
              style={{ width: 7, height: 7, background: dotColor }}
            />
            <div className="shrink-0" style={{ color: '#888', width: 140 }}>
              {agent.label}
            </div>
            <div
              className="flex-1 overflow-hidden"
              style={{ color: '#444', textOverflow: 'ellipsis', whiteSpace: 'nowrap', overflow: 'hidden' }}
            >
              {taskLabel}
            </div>
            <div
              className="shrink-0 text-right"
              style={{ color: '#333', fontSize: '10px', width: 70 }}
            >
              {elapsed}
            </div>
          </div>
        )
      })}
    </div>
  )
}

// ─── Task Card ───────────────────────────────────────────────────────────────

function TaskCard({
  task,
  showProgress,
  doneStyle,
  onDelete,
}: {
  task:         Task
  showProgress: boolean
  doneStyle:    boolean
  onDelete:     (id: number) => void
}) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="font-mono text-xs cursor-pointer"
      style={{
        background:   '#111',
        border:       '1px solid #1d1d1d',
        borderRadius: '2px',
        marginBottom: '6px',
      }}
      onClick={() => setExpanded(e => !e)}
    >
      <div className="p-2" style={{ display: 'flex', alignItems: 'flex-start', gap: 6 }}>
        <button
          onClick={e => { e.stopPropagation(); setExpanded(v => !v) }}
          style={{ color: '#444', background: 'none', border: 'none', cursor: 'pointer', padding: 0, marginTop: 1 }}
        >
          {expanded ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
        </button>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ color: '#444', fontSize: '10px' }}>
            #{task.id} · {priorityLabel(task.priority)}
          </div>
          <div style={{ color: '#777', margin: '2px 0', lineHeight: 1.3 }}>
            {task.title}
          </div>
          {task.assigned_worker && (
            <div style={{ fontSize: '10px', color: doneStyle ? '#00FF88' : '#333' }}>
              {doneStyle ? '✓ done' : task.assigned_worker}
            </div>
          )}
          {showProgress && (
            <div style={{ height: 2, background: '#1a1a1a', borderRadius: 1, marginTop: 5, overflow: 'hidden' }}>
              <div style={{ height: '100%', background: '#00FF88', borderRadius: 1, width: '60%' }} />
            </div>
          )}
        </div>
        <button
          onClick={e => { e.stopPropagation(); onDelete(task.id) }}
          className="opacity-0 hover:opacity-100 transition-opacity shrink-0"
          style={{ color: 'var(--error)', background: 'none', border: 'none', cursor: 'pointer', padding: 2 }}
          title="Delete task"
        >
          <Trash2 size={10} />
        </button>
      </div>

      {expanded && (
        <div
          style={{
            borderTop: '1px solid var(--border-1)',
            padding: '6px 8px 8px',
          }}
        >
          {task.description && (
            <p style={{ color: 'var(--text-3)', fontSize: '10px', marginBottom: 6, lineHeight: 1.5 }}>
              {task.description}
            </p>
          )}
          <div style={{ color: 'var(--text-4)', fontSize: '9px', marginBottom: 3, letterSpacing: '0.08em' }}>
            ACCEPTANCE CRITERIA
          </div>
          <pre style={{ color: 'var(--text-2)', fontSize: '10px', whiteSpace: 'pre-wrap', fontFamily: 'inherit', lineHeight: 1.5 }}>
            {task.acceptance_criteria}
          </pre>
          <div style={{ color: 'var(--text-4)', fontSize: '9px', marginTop: 6 }}>
            created {new Date(task.created_at).toLocaleString()} by {task.created_by}
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Kanban Column ────────────────────────────────────────────────────────────

const COL_HEADER_COLORS: Partial<Record<string, string>> = {
  ready:       'var(--accent)',
  in_progress: '#FFAA00',
  qc_review:   '#a78bfa',
  done_today:  'var(--success)',
}

function KanbanColumn({
  colId, label, tasks, onDelete,
}: {
  colId:    string
  label:    string
  tasks:    Task[]
  onDelete: (id: number) => void
}) {
  const showProgress = colId === 'in_progress'
  const doneStyle    = colId === 'done_today'

  return (
    <div
      style={{
        flex:         1,
        background:   '#0D0D0D',
        border:       '1px solid #1a1a1a',
        borderRadius: '2px',
        padding:      '8px',
        minHeight:    120,
        minWidth:     160,
        display:      'flex',
        flexDirection:'column',
      }}
    >
      {/* Column header */}
      <div
        style={{
          fontFamily:    'IBM Plex Mono, monospace',
          fontSize:      '10px',
          color:         '#444',
          letterSpacing: '0.08em',
          textTransform: 'uppercase',
          marginBottom:  8,
          display:       'flex',
          justifyContent:'space-between',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <div
            style={{
              width:        6,
              height:       6,
              borderRadius: '50%',
              background:   COL_HEADER_COLORS[colId] ?? '#333',
            }}
          />
          <span>{label}</span>
        </div>
        <span style={{ color: '#333' }}>{tasks.length}</span>
      </div>

      {/* Tasks */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {tasks.length === 0 ? (
          <div style={{ textAlign: 'center', padding: '16px 0', color: '#2a2a2a', fontSize: '10px', fontFamily: 'monospace' }}>
            empty
          </div>
        ) : (
          tasks.map(task => (
            <TaskCard
              key={task.id}
              task={task}
              showProgress={showProgress}
              doneStyle={doneStyle}
              onDelete={onDelete}
            />
          ))
        )}
      </div>
    </div>
  )
}

// ─── Metrics Row ──────────────────────────────────────────────────────────────

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000)     return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function MetricsRow({
  tasks,
  costUsd,
  tokensIn,
  tokensOut,
}: {
  tasks:     Task[]
  costUsd:   number | null
  tokensIn:  number | null
  tokensOut: number | null
}) {
  const todayDone   = tasks.filter(t => t.status === 'done' && isToday(t.completed_at))
  const todayFailed = tasks.filter(t => t.status === 'failed' && isToday(t.updated_at))
  const qcPassRate  = todayDone.length + todayFailed.length > 0
    ? Math.round((todayDone.length / (todayDone.length + todayFailed.length)) * 100)
    : null
  const totalTokens = (tokensIn ?? 0) + (tokensOut ?? 0)

  const metrics = [
    {
      label: 'Cost Today',
      value: costUsd != null ? `$${costUsd.toFixed(2)}` : '—',
      color: '#00FF88',
    },
    {
      label: 'Tokens',
      value: totalTokens > 0 ? formatTokens(totalTokens) : '—',
      color: '#888',
    },
    {
      label: 'Closed',
      value: String(todayDone.length),
      color: '#888',
    },
    {
      label: 'QC Pass Rate',
      value: qcPassRate != null ? `${qcPassRate}%` : '—',
      color: '#FFAA00',
    },
  ]

  return (
    <div
      className="shrink-0 flex gap-2 p-3"
      style={{ borderTop: '1px solid #1a1a1a', background: '#0A0A0A' }}
    >
      {metrics.map(m => (
        <div
          key={m.label}
          style={{
            flex:         1,
            background:   '#0D0D0D',
            border:       '1px solid #1a1a1a',
            borderRadius: '2px',
            padding:      '8px 10px',
          }}
        >
          <div style={{ fontFamily: 'monospace', fontSize: '9px', color: '#333', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
            {m.label}
          </div>
          <div style={{ fontFamily: 'monospace', fontSize: '16px', color: m.color, marginTop: 2 }}>
            {m.value}
          </div>
        </div>
      ))}
    </div>
  )
}

// ─── Main Panel ───────────────────────────────────────────────────────────────

export default function AutoDevTab() {
  const { tasks, isLoading, loadTasks, deleteTask } = useTaskStore()
  const [costUsd,   setCostUsd]   = useState<number | null>(null)
  const [tokensIn,  setTokensIn]  = useState<number | null>(null)
  const [tokensOut, setTokensOut] = useState<number | null>(null)

  // get_cost_today returns [costUsd: f64, tokensIn: u64, tokensOut: u64]
  const loadCost = useCallback(async () => {
    try {
      const [cost, tin, tout] = await invoke<[number, number, number]>('get_cost_today')
      setCostUsd(cost)
      setTokensIn(tin)
      setTokensOut(tout)
    } catch (e) {
      console.error('Failed to load cost data:', e)
    }
  }, [])

  useEffect(() => {
    loadTasks()
    loadCost()
  }, [])

  // Column definitions — matches mockup's 4-column layout
  // "Ready" merges ready + assigned (both are pre-work states)
  // "Done Today" filters completed_at to today; failed tasks also show here with ✗ style
  const readyTasks     = tasks.filter(t => t.status === 'ready' || t.status === 'assigned')
  const inProgressTasks= tasks.filter(t => t.status === 'in_progress')
  const qcTasks        = tasks.filter(t => t.status === 'qc_review')
  const doneTodayTasks = tasks.filter(t =>
    (t.status === 'done' && isToday(t.completed_at)) ||
    (t.status === 'failed' && isToday(t.updated_at))
  )

  const COLS = [
    { colId: 'ready',       label: 'Ready',       tasks: readyTasks      },
    { colId: 'in_progress', label: 'In Progress', tasks: inProgressTasks },
    { colId: 'qc_review',   label: 'QC Review',   tasks: qcTasks         },
    { colId: 'done_today',  label: 'Done Today',  tasks: doneTodayTasks  },
  ]

  return (
    <div
      className="flex flex-col flex-1 overflow-hidden"
      style={{ background: '#0A0A0A', fontFamily: 'IBM Plex Mono, monospace' }}
    >
      {/* Panel header */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid #1a1a1a', background: '#0D0D0D' }}
      >
        <span style={{ fontSize: '11px', color: '#555', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Agent Roster
        </span>
        <div className="flex gap-2">
          <button
            onClick={() => { loadTasks(); loadCost() }}
            className="flex items-center gap-1"
            style={{
              background: '#111', border: '1px solid #222', borderRadius: 2,
              padding: '4px 10px', color: '#666', fontSize: '10px', cursor: 'pointer',
            }}
          >
            <RefreshCw size={10} />
            <span>{isLoading ? 'loading…' : 'Refresh'}</span>
          </button>
          <button
            style={{
              background: '#00FF8812', border: '1px solid #00FF8840', borderRadius: 2,
              padding: '4px 10px', color: '#00FF88', fontSize: '10px', cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
            title="Create task (use PA in Delegate Mode)"
            disabled
          >
            <Plus size={10} />
            <span>New Task</span>
          </button>
        </div>
      </div>

      {/* Agent roster table */}
      <AgentRoster tasks={tasks} />

      {/* Task board label */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid #1a1a1a', background: '#0D0D0D' }}
      >
        <span style={{ fontSize: '11px', color: '#555', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Task Board
        </span>
      </div>

      {/* Kanban board */}
      <div className="flex-1 overflow-auto">
        <div
          style={{
            display:  'grid',
            gridTemplateColumns: '1fr 1fr 1fr 1fr',
            gap:      8,
            padding:  12,
            minHeight:'100%',
          }}
        >
          {COLS.map(col => (
            <KanbanColumn
              key={col.colId}
              colId={col.colId}
              label={col.label}
              tasks={col.tasks}
              onDelete={deleteTask}
            />
          ))}
        </div>
      </div>

      {/* Metrics footer */}
      <MetricsRow tasks={tasks} costUsd={costUsd} tokensIn={tokensIn} tokensOut={tokensOut} />
    </div>
  )
}
