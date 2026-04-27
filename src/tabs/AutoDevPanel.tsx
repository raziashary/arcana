// AutoDev tab — Internal Task Board (Kanban) + Agent Roster.
// Layout matches the autodevmockup.html spec exactly:
//   - Vertical roster table: status dot | agent name | current task | elapsed
//   - 4-column kanban: Ready | In Progress | QC Review | Done Today
//   - In Progress cards have progress bars
//   - Metrics footer: Cost Today | Tokens | Closed | QC Pass Rate

import { useEffect, useState, useCallback } from 'react'
import { RefreshCw, Trash2, ChevronDown, ChevronRight, Plus, Eraser } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import {
  useTaskStore,
  priorityLabel,
  type Task,
} from '../stores/taskStore'
import {
  useWorkerStore,
  workerStatusColor,
  WORKER_TYPE_LABELS,
} from '../stores/workerStore'
import { useSidecarStore } from '../stores/sidecarStore'
import { SubagentMonitor } from '../components/subagent/SubagentMonitor'

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

// ─── Agent Roster (vertical table) — reads from workers.db ─────────────────────

function AgentRoster({ tasks }: { tasks: Task[] }) {
  const workerStore = useWorkerStore()
  const [, tick]    = useState(0)

  // Re-render every second so elapsed timers update live
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 1_000)
    return () => clearInterval(id)
  }, [])

  const workers = workerStore.workers

  if (workers.length === 0) {
    return (
      <div
        className="shrink-0 border-b font-mono"
        style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-1)', padding: '8px 12px' }}
      >
        <span style={{ fontSize: 11, color: '#333' }}>Loading agents…</span>
      </div>
    )
  }

  return (
    <div
      className="shrink-0 border-b font-mono"
      style={{ background: 'var(--bg-surface)', borderColor: 'var(--border-1)' }}
    >
      {workers.map(worker => {
        const active = tasks.find(
          t => t.assigned_worker === worker.slug && t.status === 'in_progress'
        )
        const isWorking  = worker.status === 'working' || !!active
        const dotColor   = workerStatusColor(isWorking ? 'working' : worker.status)
        const typeLabel  = (WORKER_TYPE_LABELS[worker.worker_type] ?? worker.worker_type).toUpperCase()
        const label      = worker.slug.toUpperCase()
        const taskLabel  = active
          ? `#${active.id} — ${active.title}`
          : worker.status === 'working'
            ? 'Working — polling…'
          : worker.status === 'active' || worker.status === 'idle'
            ? 'Idle — awaiting tasks'
            : worker.status === 'paused'
              ? 'Paused'
              : worker.status === 'created'
                ? 'Created — not started'
                : worker.status === 'error'
                  ? 'Error — see logs'
                  : worker.status
        const elapsed = active ? elapsedSince(active.updated_at) : '—'

        return (
          <div
            key={worker.id}
            className="flex items-center gap-2"
            style={{
              borderBottom: '1px solid #111',
              padding: '7px 10px',
              fontSize: '11px',
            }}
          >
            <div
              className="shrink-0 rounded-full"
              style={{
                width: 7, height: 7, background: dotColor,
                boxShadow: isWorking ? `0 0 4px ${dotColor}` : 'none',
              }}
            />
            <div className="shrink-0" style={{ color: '#888', width: 150 }}>
              {label}
            </div>
            <div
              className="flex-1 overflow-hidden"
              style={{ color: '#444', textOverflow: 'ellipsis', whiteSpace: 'nowrap', overflow: 'hidden' }}
            >
              {taskLabel}
            </div>
            <div className="shrink-0" style={{ color: '#555', fontSize: '10px', width: 55 }}>
              {typeLabel}
            </div>
            <div
              className="shrink-0 text-right"
              style={{ color: '#333', fontSize: '10px', width: 55 }}
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
  buildFailedStyle,
  onDelete,
}: {
  task:              Task
  showProgress:      boolean
  doneStyle:         boolean
  buildFailedStyle:  boolean
  onDelete:          (id: number) => void
}) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="font-mono text-xs cursor-pointer anim-fade-in"
      style={{
        background:   '#111',
        border:       '1px solid #1d1d1d',
        borderRadius: '2px',
        marginBottom: '6px',
        transition:   'border-color 0.15s ease, box-shadow 0.15s ease',
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
            <div style={{ fontSize: '10px', color: buildFailedStyle ? '#FFCB8B' : (doneStyle ? '#ADDB67' : '#7FDBCA') }}>
              {buildFailedStyle ? '⚠ build failed' : (doneStyle ? '✓ done' : task.assigned_worker)}
            </div>
          )}
          {showProgress && (
            <div style={{ height: 2, background: '#0D3652', borderRadius: 1, marginTop: 5, overflow: 'hidden', position: 'relative' }}>
              {/* Shimmer sweep — continuous animation indicating active work */}
              <div
                style={{
                  position: 'absolute', top: 0, left: 0, height: '100%',
                  width: '40%', background: '#7FDBCA', borderRadius: 1,
                  animation: 'shimmer 1.8s ease-in-out infinite',
                }}
              />
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
  in_progress: '#FFCB8B',
  qc_review:   '#C792EA',
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
  return (
    <div
      style={{
        flex:         1,
        background:   '#00233B',
        border:       '1px solid #0D3652',
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
              background:   COL_HEADER_COLORS[colId] ?? '#3D5A6E',
            }}
          />
          <span>{label}</span>
        </div>
        <span style={{ color: '#3D5A6E' }}>{tasks.length}</span>
      </div>

      {/* Tasks */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {tasks.length === 0 ? (
          <div style={{ textAlign: 'center', padding: '16px 0', color: '#3D5A6E', fontSize: '10px', fontFamily: 'monospace' }}>
            empty
          </div>
        ) : (
          tasks.map(task => {
            const showProgress = colId === 'in_progress'
            const doneStyle = colId === 'done_today' && task.status === 'done'
            const buildFailedStyle = colId === 'done_today' && task.status === 'build_failed'
            return (
              <TaskCard
                key={task.id}
                task={task}
                showProgress={showProgress}
                doneStyle={doneStyle}
                buildFailedStyle={buildFailedStyle}
                onDelete={onDelete}
              />
            )
          })
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
  const todayDone        = tasks.filter(t => t.status === 'done' && isToday(t.completed_at))
  const todayFailed      = tasks.filter(t => t.status === 'failed' && isToday(t.updated_at))
  const todayBuildFailed = tasks.filter(t => t.status === 'build_failed' && isToday(t.updated_at))
  const qcTotal          = todayDone.length + todayFailed.length + todayBuildFailed.length
  const qcPassRate       = qcTotal > 0
    ? Math.round((todayDone.length / qcTotal) * 100)
    : null
  const totalTokens = (tokensIn ?? 0) + (tokensOut ?? 0)

  const metrics = [
    {
      label: 'Cost Today',
      value: costUsd != null ? `$${costUsd.toFixed(2)}` : '—',
      color: '#7FDBCA',
    },
    {
      label: 'Tokens',
      value: totalTokens > 0 ? formatTokens(totalTokens) : '—',
      color: '#637777',
    },
    {
      label: 'Closed',
      value: String(todayDone.length),
      color: '#637777',
    },
    {
      label: 'Build Failed',
      value: String(todayBuildFailed.length),
      color: '#FF6363',
    },
    {
      label: 'QC Pass Rate',
      value: qcPassRate != null ? `${qcPassRate}%` : '—',
      color: '#FFCB8B',
    },
  ]

  return (
    <div
      className="shrink-0 flex gap-2 p-3"
      style={{ borderTop: '1px solid #0D3652', background: '#011627' }}
    >
      {metrics.map(m => (
        <div
          key={m.label}
          style={{
            flex:         1,
            background:   '#00233B',
            border:       '1px solid #0D3652',
            borderRadius: '2px',
            padding:      '8px 10px',
          }}
        >
          <div style={{ fontFamily: 'monospace', fontSize: '9px', color: '#3D5A6E', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
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

function formatLastRefreshed(d: Date): string {
  const diffMs  = Date.now() - d.getTime()
  const diffSec = Math.floor(diffMs / 1_000)
  if (diffSec < 10)  return 'just now'
  if (diffSec < 60)  return `${diffSec}s ago`
  const diffMin = Math.floor(diffSec / 60)
  return `${diffMin}m ago`
}

export default function AutoDevTab() {
  const { tasks, loadTasks, deleteTask } = useTaskStore()
  const workerStore = useWorkerStore()
  const { status: sidecarStatus, isConnected: sidecarConnected } = useSidecarStore()
  const [costUsd,      setCostUsd]      = useState<number | null>(null)
  const [tokensIn,     setTokensIn]     = useState<number | null>(null)
  const [tokensOut,    setTokensOut]    = useState<number | null>(null)
  const [clearConfirm, setClearConfirm] = useState(false)
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null)
  const [isRefreshing,  setIsRefreshing]  = useState(false)
  const [showSubagents, setShowSubagents] = useState(false)
  const [, tick] = useState(0)

  // Re-render every 10s so the "Xm ago" label stays fresh
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 10_000)
    return () => clearInterval(id)
  }, [])

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

  const doRefresh = useCallback(async () => {
    if (isRefreshing) return
    setIsRefreshing(true)
    try {
      await Promise.all([loadTasks(), loadCost(), workerStore.loadWorkers()])
      setLastRefreshed(new Date())
    } finally {
      setIsRefreshing(false)
    }
  }, [isRefreshing, loadTasks, loadCost, workerStore])

  useEffect(() => {
    doRefresh()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-refresh every 3 minutes — enough to stay current without stressing the app.
  useEffect(() => {
    const id = setInterval(doRefresh, 180_000)
    return () => clearInterval(id)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Column definitions — matches mockup's 4-column layout
  // "Ready" merges ready + assigned (both are pre-work states)
  // "Done Today" filters completed_at to today; failed tasks also show here with ✗ style
  const readyTasks     = tasks.filter(t => t.status === 'ready' || t.status === 'assigned')
  const inProgressTasks= tasks.filter(t => t.status === 'in_progress')
  const qcTasks        = tasks.filter(t => t.status === 'qc_review')
  const doneTodayTasks = tasks.filter(t =>
    (t.status === 'done' && isToday(t.completed_at)) ||
    (t.status === 'failed' && isToday(t.updated_at)) ||
    (t.status === 'build_failed' && isToday(t.updated_at))
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
      style={{ background: '#011627', fontFamily: 'IBM Plex Mono, monospace' }}
    >
      {/* Panel header */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid #0D3652', background: '#00233B' }}
      >
        <span style={{ fontSize: '11px', color: '#637777', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Agent Roster
        </span>
        <div className="flex items-center gap-3">
          {/* core-orchestrator sidecar status badge */}
          <div
            className="flex items-center gap-1"
            title={`core-orchestrator: ${sidecarStatus}`}
            style={{ fontSize: '9px', color: sidecarConnected ? '#7FDBCA' : '#637777', letterSpacing: '0.05em' }}
          >
            <span style={{
              width: 5, height: 5, borderRadius: '50%', display: 'inline-block',
              background: sidecarConnected ? '#7FDBCA'
                : sidecarStatus === 'starting' ? '#FFCB8B'
                : '#3D5A6E',
              boxShadow: sidecarConnected ? '0 0 4px #7FDBCA' : 'none',
            }} />
            <span>{sidecarConnected ? 'LivingAgent' : 'Mode 1'}</span>
          </div>
        <div className="flex gap-2">
          <div className="flex items-center gap-2">
            {lastRefreshed && (
              <span style={{ fontSize: '9px', color: '#3D5A6E', fontFamily: 'monospace' }}>
                ↺ {formatLastRefreshed(lastRefreshed)}
              </span>
            )}
            <button
              onClick={doRefresh}
              className="flex items-center gap-1"
              disabled={isRefreshing}
              style={{
                background: '#0B3A5C', border: '1px solid #1A527A', borderRadius: 2,
                padding: '4px 10px', color: isRefreshing ? '#82AAFF' : '#637777',
                fontSize: '10px', cursor: isRefreshing ? 'default' : 'pointer',
                transition: 'color 0.2s',
              }}
            >
              <RefreshCw size={10} className={isRefreshing ? 'anim-spin' : ''} />
              <span>{isRefreshing ? 'updating…' : 'Refresh'}</span>
            </button>
          </div>
          <button
            style={{
              background: 'var(--accent-muted)', border: '1px solid var(--accent-border)', borderRadius: 2,
              padding: '4px 10px', color: 'var(--accent)', fontSize: '10px', cursor: 'pointer',
              display: 'flex', alignItems: 'center', gap: 4,
            }}
            title="Create task (use PA in Delegate Mode)"
            disabled
          >
            <Plus size={10} />
            <span>New Task</span>
          </button>
        </div>
        </div> {/* closes flex items-center gap-3 */}
      </div>

      {/* Agent roster table */}
      <AgentRoster tasks={tasks} />

      {/* Task board label */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid #0D3652', background: '#00233B' }}
      >
        <span style={{ fontSize: '11px', color: '#637777', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Task Board
        </span>
        <button
          onClick={async () => {
            if (!clearConfirm) { setClearConfirm(true); setTimeout(() => setClearConfirm(false), 3000); return }
            await invoke('clear_all_tasks')
            setClearConfirm(false)
            loadTasks()
          }}
          className="flex items-center gap-1"
          style={{
            background:   clearConfirm ? '#FF636320' : '#0B3A5C',
            border:       clearConfirm ? '1px solid #FF636350' : '1px solid #1A527A',
            borderRadius: 2,
            padding:      '3px 8px',
            color:        clearConfirm ? '#FF6363' : '#637777',
            fontSize:     '10px',
            fontFamily:   'monospace',
            cursor:       'pointer',
          }}
          title={clearConfirm ? 'Click again to confirm — deletes all tasks' : 'Clear all tasks from board'}
        >
          <Eraser size={9} />
          <span>{clearConfirm ? 'Confirm clear?' : 'Clear Board'}</span>
        </button>
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

      {/* Subagent Monitor Toggle Button */}
      <button
        onClick={() => setShowSubagents(!showSubagents)}
        style={{
          position: 'fixed',
          right: '12px',
          bottom: '20px',
          width: '48px',
          height: '48px',
          borderRadius: '50%',
          background: 'var(--accent)',
          border: 'none',
          color: '#011627',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '20px',
          zIndex: 999,
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
        }}
        title="Toggle subagent monitor"
      >
        🤖
      </button>

      {/* Subagent Monitor Sidebar (Collapsible) */}
      {showSubagents && (
        <div style={{
          position: 'fixed',
          right: 0,
          top: 0,
          bottom: 0,
          width: '320px',
          borderLeft: '1px solid #0D3652',
          background: 'var(--bg-surface)',
          overflowY: 'auto',
          zIndex: 1000,
          boxShadow: '-2px 0 8px rgba(0,0,0,0.3)',
        }}>
          <div style={{ padding: '12px', borderBottom: '1px solid #0D3652', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ fontSize: '12px', fontWeight: 'bold', color: 'var(--text-1)' }}>Subagent Monitor</span>
            <button
              onClick={() => setShowSubagents(false)}
              style={{
                background: 'none',
                border: 'none',
                color: 'var(--text-3)',
                cursor: 'pointer',
                fontSize: '16px',
              }}
            >
              ✕
            </button>
          </div>
          <SubagentMonitor />
        </div>
      )}
    </div>
  )
}
