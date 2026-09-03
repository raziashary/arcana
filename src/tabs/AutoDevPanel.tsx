// AutoDev tab — Internal Task Board (Kanban) + Agent Roster.
// Layout matches the autodevmockup.html spec exactly:
//   - Vertical roster table: status dot | agent name | current task | elapsed
//   - 4-column kanban: Ready | In Progress | QC Review | Done Today
//   - In Progress cards have progress bars
//   - Metrics footer: Cost Today | Tokens | Closed | QC Pass Rate

import { useEffect, useState, useCallback, type ReactNode } from 'react'
import { RefreshCw, Trash2, ChevronDown, ChevronRight, Plus, Eraser } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import {
  useTaskStore,
  priorityLabel,
  type Task,
  type WorkerRequest,
} from '../stores/taskStore'
import {
  useWorkerStore,
  workerStatusColor,
  WORKER_TYPE_LABELS,
} from '../stores/workerStore'
import { SubagentMonitor } from '../components/subagent/SubagentMonitor'
import { LiveMonitorSection } from '../components/autodev/LiveMonitorSection'
import { FleetChat }          from '../components/autodev/FleetChat'
import { useAgentLiveStore }  from '../stores/agentLiveStore'
import { useUiStore }         from '../stores/uiStore'
import { WorkerAffinityPanel } from '../components/dashboard/WorkerAffinityPanel'
import { OrphanAlert } from '../components/dashboard/OrphanAlert'
import { QcRoiPanel }  from '../components/dashboard/QcRoiPanel'
import { BudgetBar } from '../components/budget/BudgetBar'
import { DiffAnalysisCard } from '../components/dashboard/DiffAnalysisCard'
import { EpicsSection } from '../components/dashboard/EpicPhaseView'
import { ErrorBoundary } from '../components/ErrorBoundary'
import '../components/autodev/LiveMonitor.css'
import { parseDbTimestamp } from "../utils/dbTime"

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

// ─── Enhanced status cell — pulls live tool/file info from agentLiveStore ───────

function AgentStatusCell({ agentId, fallbackLabel }: { agentId: string; fallbackLabel: string }) {
  const agent     = useAgentLiveStore(s => s.agents[agentId])
  const elapsedMs = agent?.elapsed_since_last_tool_ms ?? 0
  const isSlowTool = elapsedMs > 120_000

  if (!agent || agent.status === 'idle' || agent.status === 'paused') {
    return <span className="status-text status-text--idle">{fallbackLabel}</span>
  }
  if (agent.status === 'error') {
    return (
      <span className="status-text status-text--error">
        ⚠ {agent.error_message ?? 'error'}
      </span>
    )
  }
  if (agent.current_tool) {
    return (
      <span className="status-text status-text--active">
        <span className="status-tool">{agent.current_tool}</span>
        {agent.current_file && (
          <>
            <span className="status-sep"> · </span>
            <span className="status-file">{agent.current_file}</span>
            {agent.current_file_diff && (
              <span className="status-diff"> {agent.current_file_diff}</span>
            )}
          </>
        )}
        {isSlowTool && <span className="status-slow-warn" title="Tool running > 2 minutes"> ⚠</span>}
      </span>
    )
  }
  return <span className="status-text">{fallbackLabel}</span>
}

// ─── Agent Roster (vertical table) — reads from workers.db ─────────────────────

function AgentRoster({ tasks }: { tasks: Task[] }) {
  const workerStore    = useWorkerStore()
  const [, tick]       = useState(0)

  // Re-render every second so elapsed timers update live
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 1_000)
    return () => clearInterval(id)
  }, [])

  // NOTE: `initAgents()` used to live in this component, which raced with
  // `LiveMonitorSection.subscribeToEvents()` — STARTUP `worker:tick-start`
  // events fired before this component had a chance to seed the slug list,
  // and `handleStatusChange` silently dropped them. The init call has been
  // promoted to the top-level `AutoDevTab` mount (search for `initAgents`
  // there), and `handleStatusChange` now auto-creates unknown agents as a
  // belt-and-suspenders fallback. See docs/endtoendrun/14may2026/01.

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

        // 🔴 `P8-43`. This was a per-worker ENGINE badge, and with one
        // engine left it had become a liar rather than merely redundant.
        // `current_tick_mode` comes from the tick-start payload, which
        // `runner.rs` now sets to `"native"`, so both live branches were
        // unreachable and every successful tick fell through to
        // `lastOutcome === 'success'` — painting a green badge naming the
        // deleted second engine for work it could not have done.
        //
        // It is now a tick-activity badge, which is the one thing here that is
        // still true and still useful: is this worker mid-tick, and how did its
        // last one end.
        const lastOutcome    = worker.last_outcome
        const currentMode    = worker.current_tick_mode
        const modeBadge =
          currentMode
            ? { label: 'RUN', color: 'var(--accent)',  title: 'Tick in progress', glow: true }
          : lastOutcome === 'success'
            ? { label: 'OK',  color: 'var(--success)', title: 'Last tick completed', glow: false }
          : lastOutcome?.startsWith('error')
            ? { label: 'ERR', color: 'var(--danger)',  title: `Last tick failed: ${lastOutcome}`, glow: false }
          :   { label: '—',   color: '#3D5A6E',        title: 'No tick data yet', glow: false }
        const hasModeData = !!currentMode || !!lastOutcome

        return (
          <div
            key={worker.id}
            className="flex items-center gap-2"
            style={{
              borderBottom: '1px solid var(--border-1)',
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
            {/* Agent name + inline mode indicator (dot + label) right after name */}
            <div className="shrink-0 flex items-center gap-1" style={{ width: 150 }}>
              <span style={{ color: '#888' }}>{label}</span>
              <span
                title={modeBadge.title}
                style={{
                  width: 5, height: 5, borderRadius: '50%', flexShrink: 0,
                  background:  modeBadge.color,
                  boxShadow:   modeBadge.glow ? `0 0 4px ${modeBadge.color}` : 'none',
                  opacity:     hasModeData ? 1 : 0.25,
                  display:     'inline-block',
                }}
              />
              {hasModeData && (
                <span style={{
                  fontSize: '8px', color: modeBadge.color,
                  fontWeight: 700, fontFamily: 'monospace',
                  letterSpacing: '0.04em', opacity: 0.9,
                }}>
                  {modeBadge.label}
                </span>
              )}
            </div>
            <div className="flex-1 overflow-hidden">
              <AgentStatusCell agentId={worker.slug} fallbackLabel={taskLabel} />
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
  rejectedStyle,
  needsInfoStyle,
  onDelete,
}: {
  task:              Task
  showProgress:      boolean
  doneStyle:         boolean
  buildFailedStyle:  boolean
  rejectedStyle:     boolean
  needsInfoStyle:    boolean
  onDelete:          (id: number) => void
}) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="font-mono text-xs cursor-pointer anim-fade-in"
      style={{
        background:   'var(--bg-card)',
        border:       '1px solid var(--border-1)',
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
            <div style={{ fontSize: '10px', color: buildFailedStyle ? 'var(--warning)' : rejectedStyle ? '#FF6B35' : needsInfoStyle ? '#F59E0B' : (doneStyle ? 'var(--accent)' : 'var(--accent)') }}>
              {buildFailedStyle ? '⚠ build failed' : rejectedStyle ? '✗ rejected' : needsInfoStyle ? '⏸ needs info' : (doneStyle ? '✓ done' : task.assigned_worker)}
            </div>
          )}
          {rejectedStyle && task.rejection_reason && (
            <div style={{ fontSize: '9px', color: '#FF6B35', marginTop: 2, fontStyle: 'italic', lineHeight: 1.4 }}>
              {task.rejection_reason}
            </div>
          )}
          {/* Ruflo Feature 5 — Phase badges for tasks that belong to an epic */}
          {(task.epic_id || task.phase) && (
            <div style={{ display: 'flex', gap: 3, marginTop: 3, flexWrap: 'wrap' }}>
              {task.epic_id && (
                <span style={{
                  fontSize: 8, padding: '1px 4px', borderRadius: 2,
                  background: 'rgba(195,166,255,0.12)', color: '#C3A6FF',
                  border: '1px solid rgba(195,166,255,0.25)', fontFamily: 'monospace',
                }}>
                  epic #{task.epic_id}
                </span>
              )}
              {task.phase && (
                <span style={{
                  fontSize: 8, padding: '1px 4px', borderRadius: 2,
                  background: 'rgba(130,170,255,0.12)', color: '#82AAFF',
                  border: '1px solid rgba(130,170,255,0.25)', fontFamily: 'monospace',
                }}>
                  {task.phase}
                </span>
              )}
              {task.blocked_by && task.blocked_by.length > 0 && (
                <span style={{
                  fontSize: 8, padding: '1px 4px', borderRadius: 2,
                  background: 'rgba(245,158,11,0.12)', color: '#F59E0B',
                  border: '1px solid rgba(245,158,11,0.25)', fontFamily: 'monospace',
                }}>
                  blocked
                </span>
              )}
            </div>
          )}
          {showProgress && (
            <div style={{ height: 2, background: 'var(--border-1)', borderRadius: 1, marginTop: 5, overflow: 'hidden', position: 'relative' }}>
              {/* Shimmer sweep — continuous animation indicating active work */}
              <div
                style={{
                  position: 'absolute', top: 0, left: 0, height: '100%',
                  width: '40%', background: 'var(--accent)', borderRadius: 1,
                  animation: 'shimmer 1.8s ease-in-out infinite',
                }}
              />
            </div>
          )}
          {/* Ruflo Feature 4 — Budget Circuit Breaker: show budget bar when task has a budget */}
          {(task.budget_max_tokens != null || task.budget_max_usd != null) && (
            <div style={{ marginTop: 6 }}>
              <BudgetBar
                taskId={task.id}
                maxTokens={task.budget_max_tokens ?? undefined}
                maxUsd={task.budget_max_usd ?? undefined}
                maxTurns={task.budget_max_turns ?? undefined}
                compact
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
          {/* Ruflo Feature 3 — QC diff risk card. Renders when analyze_diff_tool
              persisted a result on this task (typically during QC review). */}
          {task.diff_analysis && (
            <div style={{ marginTop: 8 }}>
              <DiffAnalysisCard diffAnalysisJson={task.diff_analysis} />
            </div>
          )}
          <div style={{ color: 'var(--text-4)', fontSize: '9px', marginTop: 6 }}>
            created {parseDbTimestamp(task.created_at)?.toLocaleString() ?? "unknown"} by {task.created_by}
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Kanban Column ────────────────────────────────────────────────────────────

const COL_HEADER_COLORS: Partial<Record<string, string>> = {
  ready:       'var(--accent)',
  in_progress: 'var(--warning)',
  qc_review:   'var(--info)',
  rejected:    '#FF6B35',
  done_today:  'var(--success)',
  needs_info:  '#F59E0B',
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
        background:   'var(--bg-surface)',
        border:       '1px solid var(--border-1)',
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
              background:   COL_HEADER_COLORS[colId] ?? 'var(--text-4)',
            }}
          />
          <span>{label}</span>
        </div>
        <span style={{ color: 'var(--text-4)' }}>{tasks.length}</span>
      </div>

      {/* Tasks */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {tasks.length === 0 ? (
          <div style={{ textAlign: 'center', padding: '16px 0', color: 'var(--text-4)', fontSize: '10px', fontFamily: 'monospace' }}>
            empty
          </div>
        ) : (
          tasks.map(task => {
            const showProgress    = colId === 'in_progress'
            const doneStyle       = colId === 'done_today' && task.status === 'done'
            const buildFailedStyle = colId === 'done_today' && task.status === 'build_failed'
            const rejectedStyle   = colId === 'rejected'
            const needsInfoStyle  = colId === 'needs_info' || task.status === 'needs_info'
            return (
              <TaskCard
                key={task.id}
                task={task}
                showProgress={showProgress}
                doneStyle={doneStyle}
                buildFailedStyle={buildFailedStyle}
                rejectedStyle={rejectedStyle}
                needsInfoStyle={needsInfoStyle}
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
      color: 'var(--accent)',
    },
    {
      label: 'Tokens',
      value: totalTokens > 0 ? formatTokens(totalTokens) : '—',
      color: 'var(--text-2)',
    },
    {
      label: 'Closed',
      value: String(todayDone.length),
      color: 'var(--text-2)',
    },
    {
      label: 'Build Failed',
      value: String(todayBuildFailed.length),
      color: 'var(--danger)',
    },
    {
      label: 'QC Pass Rate',
      value: qcPassRate != null ? `${qcPassRate}%` : '—',
      color: 'var(--warning)',
    },
  ]

  return (
    <div
      className="shrink-0 flex gap-2 p-3"
      style={{ borderTop: '1px solid var(--border-1)', background: 'var(--bg-app)' }}
    >
      {metrics.map(m => (
        <div
          key={m.label}
          style={{
            flex:         1,
            background:   'var(--bg-surface)',
            border:       '1px solid var(--border-1)',
            borderRadius: '2px',
            padding:      '8px 10px',
          }}
        >
          <div style={{ fontFamily: 'monospace', fontSize: '9px', color: 'var(--text-4)', letterSpacing: '0.08em', textTransform: 'uppercase' }}>
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

// ─── Worker Requests Widget — Fleet→Supervisor escalation queue ───────────────

function WorkerRequestsWidget({
  requests,
  onResolve,
}: {
  requests: WorkerRequest[]
  onResolve: (id: number, action: 'completed' | 'rejected', note?: string) => void | Promise<void>
}) {
  // Doc 03 Phase 1: state split. Previously a single `expandedId` carried
  // two orthogonal interactions — `-1` meant "panel open" and any other
  // number meant "row N open". Clicking a row set it to req.id, which made
  // the panel-open guard false and unmounted the entire list. That's the
  // bug screenshot users reported: clicking the chevron collapses the panel.
  const [panelOpen,     setPanelOpen]     = useState(false)
  const [openRequestId, setOpenRequestId] = useState<number | null>(null)
  // Doc 03 Phase 3: per-row inline error string for resolve failures.
  const [rowError,      setRowError]      = useState<{ id: number; message: string } | null>(null)
  const [pendingId,     setPendingId]     = useState<number | null>(null)

  if (requests.length === 0) return null

  const typeLabel = (t: string) =>
    t === 'decompose_audit' ? 'Decompose Audit'
    : t === 'reassign' ? 'Reassign'
    : t === 'needs_info' ? 'Needs Info'
    : t === 'report_issue' ? 'Report Issue'
    : t === 'request_review' ? 'Request Review'
    : t

  const typeColor = (t: string) =>
    t === 'decompose_audit' ? '#7FDBCA'
    : t === 'reassign' ? '#FFCB8B'
    : t === 'needs_info' ? '#82AAFF'
    : t === 'report_issue' ? '#FF6363'
    : t === 'request_review' ? '#C3A6FF'
    : 'var(--text-4)'

  const handleResolve = async (
    id: number,
    action: 'completed' | 'rejected',
    note?: string,
  ) => {
    setPendingId(id)
    setRowError(null)
    try {
      await onResolve(id, action, note)
    } catch (e) {
      setRowError({ id, message: String(e instanceof Error ? e.message : e) })
    } finally {
      setPendingId(null)
    }
  }

  return (
    <div
      style={{
        borderBottom: '1px solid var(--border-1)',
        background:   'rgba(127,219,202,0.04)',
      }}
    >
      <div
        className="flex items-center justify-between px-4 py-2 cursor-pointer"
        onClick={() => setPanelOpen(o => !o)}
      >
        <div className="flex items-center gap-2">
          <span style={{ width: 6, height: 6, borderRadius: '50%', background: '#FF6363', display: 'inline-block' }} />
          <span style={{ fontSize: '11px', color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
            Worker Requests
          </span>
          <span
            style={{
              fontSize: '9px',
              background: 'rgba(255,99,99,0.15)',
              color: '#FF6363',
              padding: '1px 6px',
              borderRadius: 4,
              fontFamily: 'monospace',
            }}
          >
            {requests.length}
          </span>
        </div>
        {panelOpen ? <ChevronDown size={12} color="var(--text-4)" /> : <ChevronRight size={12} color="var(--text-4)" />}
      </div>

      {panelOpen && (
        <div className="px-4 pb-3 flex flex-col gap-2">
          {requests.map(req => {
            const isOpen = openRequestId === req.id

            // Compute a short single-line preview from the JSON payload.
            // Shown ONLY when the row is collapsed — when expanded we show
            // the fully parsed JSON below to satisfy doc 03 Phase 2.
            let payloadPreview = ''
            let parsedPayload: unknown = null
            let parseError = false
            try {
              parsedPayload = JSON.parse(req.payload)
              const p = parsedPayload as Record<string, unknown>
              if (p && typeof p === 'object') {
                if (typeof p.audit_summary === 'string') payloadPreview = p.audit_summary
                else if (typeof p.issue_description === 'string') payloadPreview = p.issue_description
                else if (typeof p.reason === 'string') payloadPreview = p.reason
                else payloadPreview = JSON.stringify(p).slice(0, 120)
              } else {
                payloadPreview = String(parsedPayload).slice(0, 120)
              }
            } catch {
              parseError = true
              payloadPreview = req.payload.slice(0, 120)
            }

            return (
              <div
                key={req.id}
                style={{
                  border: '1px solid var(--border-1)',
                  borderRadius: 2,
                  background: 'var(--bg-card)',
                  fontSize: '11px',
                }}
              >
                <div
                  className="flex items-center justify-between px-3 py-2 cursor-pointer"
                  onClick={() => setOpenRequestId(isOpen ? null : req.id)}
                >
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <span style={{ color: typeColor(req.request_type), fontWeight: 700, fontFamily: 'monospace', fontSize: '9px' }}>
                      {typeLabel(req.request_type).toUpperCase()}
                    </span>
                    <span style={{ color: 'var(--text-2)' }}>
                      {req.worker_slug}
                    </span>
                    {req.task_id != null && (
                      <span style={{ color: 'var(--text-4)', fontFamily: 'monospace', fontSize: '9px' }}>
                        #{req.task_id}
                      </span>
                    )}
                    {/* Doc 03 Phase 2: show the short preview ONLY when the
                        row is collapsed. When expanded, the full parsed JSON
                        renders below — the preview would be redundant. */}
                    {!isOpen && payloadPreview && (
                      <span
                        className="truncate"
                        style={{
                          color:    'var(--text-3)',
                          fontSize: '10px',
                          flex:     1,
                        }}
                        title={payloadPreview}
                      >
                        — {payloadPreview}
                      </span>
                    )}
                  </div>
                  {isOpen ? <ChevronDown size={10} color="var(--text-4)" /> : <ChevronRight size={10} color="var(--text-4)" />}
                </div>

                {isOpen && (
                  <div className="px-3 pb-3">
                    {/* Doc 03 Phase 2: full parsed JSON payload in a <pre>
                        when the row is open. Falls back to the raw string
                        when parsing fails so we never hide data from the user. */}
                    <div
                      className="font-mono"
                      style={{
                        color:        'var(--text-3)',
                        fontSize:     '10px',
                        background:   'var(--bg-deep, #0f0f0f)',
                        border:       '1px solid var(--border-1)',
                        borderRadius: 2,
                        padding:      '6px 8px',
                        marginBottom: 8,
                        maxHeight:    '240px',
                        overflow:     'auto',
                        whiteSpace:   'pre-wrap',
                        wordBreak:    'break-word',
                      }}
                    >
                      {parseError
                        ? req.payload
                        : JSON.stringify(parsedPayload, null, 2)}
                    </div>

                    {/* Doc 03 Phase 3: Acknowledge / Reject buttons. */}
                    <div className="flex items-center gap-2">
                      <button
                        disabled={pendingId === req.id}
                        onClick={() => handleResolve(req.id, 'completed')}
                        style={{
                          background: 'rgba(127,219,202,0.15)',
                          border: '1px solid rgba(127,219,202,0.3)',
                          borderRadius: 2,
                          padding: '2px 8px',
                          color: '#7FDBCA',
                          fontSize: '9px',
                          fontFamily: 'monospace',
                          cursor: pendingId === req.id ? 'wait' : 'pointer',
                          opacity: pendingId === req.id ? 0.6 : 1,
                        }}
                      >
                        {pendingId === req.id ? 'Acknowledging…' : 'Acknowledge'}
                      </button>
                      <button
                        disabled={pendingId === req.id}
                        onClick={() => handleResolve(req.id, 'rejected', 'Human override — declined')}
                        style={{
                          background: 'rgba(255,99,99,0.10)',
                          border: '1px solid rgba(255,99,99,0.25)',
                          borderRadius: 2,
                          padding: '2px 8px',
                          color: '#FF6363',
                          fontSize: '9px',
                          fontFamily: 'monospace',
                          cursor: pendingId === req.id ? 'wait' : 'pointer',
                          opacity: pendingId === req.id ? 0.6 : 1,
                        }}
                      >
                        Reject
                      </button>
                    </div>

                    {/* Doc 03 Phase 3: inline error string on resolve failure.
                        Does not collapse the row — the user keeps the
                        context they need to retry. */}
                    {rowError?.id === req.id && (
                      <div
                        className="font-mono"
                        style={{
                          color:      'var(--danger, #FF6363)',
                          fontSize:   '10px',
                          marginTop:  6,
                        }}
                      >
                        Failed: {rowError.message}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}

// ─── ObservabilityTabs ───────────────────────────────────────────────────────
//
// Tab strip that surfaces one of the AutoDev panel's five secondary panels
// at a time. Replaces the previous always-on vertical stack which caused the
// page to feel crowded (5 + Agent Roster + Task Board = 7 stacked sections
// before the user's screen ran out). Each tab includes a small live badge
// where useful — e.g. Worker Requests shows the pending count, Live Monitor
// shows the active reviewer count via QC ROI's source.
//
// Persistence: the active tab is stored under `autodev.obstab` so the user's
// preferred view sticks. Default = "monitor" because that's what is most
// often needed during a live run.

type ObsTabId = 'monitor' | 'requests' | 'affinity' | 'epics' | 'qcroi' | 'fleetchat'

interface ObsTab {
  id:    ObsTabId
  label: string
  badge?: (ctx: { workerRequests: WorkerRequest[] }) => string | null
  render: (ctx: {
    tasks:           Task[]
    workerRequests:  WorkerRequest[]
    onResolveRequest: (id: number, action: 'completed' | 'rejected', note?: string) => Promise<void>
  }) => ReactNode
}

const OBS_TABS: ObsTab[] = [
  {
    id:    'monitor',
    label: 'Live Monitor',
    render: () => <LiveMonitorSection />,
  },
  {
    id:    'requests',
    label: 'Worker Requests',
    badge: ({ workerRequests }) => {
      const n = workerRequests.filter(r => r.status === 'pending').length
      return n > 0 ? String(n) : null
    },
    render: ({ workerRequests, onResolveRequest }) => (
      <WorkerRequestsWidget
        requests={workerRequests}
        onResolve={onResolveRequest}
      />
    ),
  },
  {
    id:    'affinity',
    label: 'Worker Affinity',
    render: () => <WorkerAffinityPanel />,
  },
  {
    id:    'epics',
    label: 'Epics',
    render: ({ tasks }) => <EpicsSection tasks={tasks} />,
  },
  {
    id:    'qcroi',
    label: 'QC ROI',
    render: () => (
      <div style={{ padding: '0 12px 8px 12px' }}>
        <QcRoiPanel />
      </div>
    ),
  },
  {
    id:    'fleetchat',
    label: 'Fleet Chat',
    render: () => (
      <div style={{ padding: '0 12px 8px 12px' }}>
        <FleetChat />
      </div>
    ),
  },
]

function ObservabilityTabs(props: {
  tasks:            Task[]
  workerRequests:   WorkerRequest[]
  onResolveRequest: (id: number, action: 'completed' | 'rejected', note?: string) => Promise<void>
}) {
  const [active, setActive] = useState<ObsTabId>(() => {
    const stored = localStorage.getItem('autodev.obstab') as ObsTabId | null
    if (stored && OBS_TABS.some(t => t.id === stored)) return stored
    return 'monitor'
  })

  function selectTab(id: ObsTabId) {
    setActive(id)
    localStorage.setItem('autodev.obstab', id)
  }

  const activeTab = OBS_TABS.find(t => t.id === active) ?? OBS_TABS[0]

  return (
    <div
      className="autodev-obs-tabs"
      style={{
        borderTop:    '1px solid var(--border-1)',
        borderBottom: '1px solid var(--border-1)',
        background:   'var(--bg-app)',
      }}
    >
      {/* Tab strip */}
      <div
        role="tablist"
        style={{
          display:    'flex',
          alignItems: 'center',
          gap:        4,
          padding:    '6px 12px',
          background: 'var(--bg-surface)',
          borderBottom: '1px solid var(--border-1)',
          fontFamily: 'var(--font-mono)',
          fontSize:   '10px',
          overflowX:  'auto',
        }}
      >
        {OBS_TABS.map(tab => {
          const isActive = tab.id === active
          const badge = tab.badge?.({ workerRequests: props.workerRequests }) ?? null
          return (
            <button
              key={tab.id}
              role="tab"
              aria-selected={isActive}
              onClick={() => selectTab(tab.id)}
              style={{
                background:    isActive ? 'var(--bg-card)' : 'transparent',
                border:        `1px solid ${isActive ? 'var(--accent-border)' : 'var(--border-1)'}`,
                color:         isActive ? 'var(--accent)' : 'var(--text-2)',
                padding:       '4px 10px',
                borderRadius:  3,
                cursor:        'pointer',
                fontFamily:    'inherit',
                fontSize:      'inherit',
                letterSpacing: '0.05em',
                textTransform: 'uppercase',
                display:       'inline-flex',
                alignItems:    'center',
                gap:           5,
                whiteSpace:    'nowrap',
              }}
            >
              <span>{tab.label}</span>
              {badge && (
                <span
                  style={{
                    background:   isActive ? 'var(--accent)' : 'var(--info)',
                    color:        '#000',
                    fontSize:     '9px',
                    padding:      '0 5px',
                    borderRadius: 9,
                    fontWeight:   600,
                  }}
                >
                  {badge}
                </span>
              )}
            </button>
          )
        })}
      </div>

      {/* Active panel content */}
      <div className="autodev-obs-tabs-content">
        {activeTab.render({
          tasks:           props.tasks,
          workerRequests:  props.workerRequests,
          onResolveRequest: props.onResolveRequest,
        })}
      </div>
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
  const {
    tasks, loadTasks, deleteTask,
    workerRequests, loadWorkerRequests, resolveWorkerRequest,
  } = useTaskStore()
  const workerStore = useWorkerStore()
  const [costUsd,      setCostUsd]      = useState<number | null>(null)
  const [tokensIn,     setTokensIn]     = useState<number | null>(null)
  const [tokensOut,    setTokensOut]    = useState<number | null>(null)
  const [clearConfirm, setClearConfirm] = useState(false)
  const [clearError,   setClearError]   = useState<string | null>(null)
  const [clearBusy,    setClearBusy]    = useState(false)
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
      await Promise.all([loadTasks(), loadCost(), workerStore.loadWorkers(), loadWorkerRequests('pending')])
      setLastRefreshed(new Date())
    } finally {
      setIsRefreshing(false)
    }
  }, [isRefreshing, loadTasks, loadCost, workerStore])

  useEffect(() => {
    doRefresh()
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-refresh every 1 minute — enough to stay current without stressing the app.
  useEffect(() => {
    const id = setInterval(doRefresh, 60_000)
    return () => clearInterval(id)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Seed the live-monitor agent registry as early as possible — at the
  // top-level `AutoDevTab` mount, NOT inside the deep `AgentRoster` child.
  // Previously `initAgents` lived in AgentRoster and raced with
  // `LiveMonitorSection.subscribeToEvents()`: STARTUP `worker:tick-start`
  // events fired ~1ms after Rust boot and were silently dropped because
  // `state.agents` was still empty. `agentLiveStore.handleStatusChange`
  // now auto-creates unknown agents as a belt-and-suspenders fallback,
  // but seeding here keeps the slug-set authoritative.
  //
  // Dep array uses a stable hash of the slug set rather than `.length`
  // so a worker rename or replacement re-seeds correctly.
  const initAgents       = useAgentLiveStore(s => s.initAgents)
  const setAgentModel    = useAgentLiveStore(s => s.setAgentModel)
  const workerSlugKey    = workerStore.workers.map(w => w.slug).sort().join(',')
  // Doc 01 Phase 3 — also push each worker's `model` field into the live
  // store so AgentLiveCard can render a model badge without re-querying
  // workerStore on every render.
  const workerModelKey   = workerStore.workers.map(w => `${w.slug}:${w.model}`).sort().join(',')
  useEffect(() => {
    const ids = workerStore.workers.map(w => w.slug)
    if (ids.length > 0) initAgents(ids)
    workerStore.workers.forEach(w => setAgentModel(w.slug, w.model ?? null))
  }, [workerSlugKey, workerModelKey, initAgents, setAgentModel])

  // Doc 06 Phase 3 — react to "jump to qc_review column" pulses from the
  // QC card. The card increments `taskColumnPulse.nonce`; this effect
  // scrolls the matching column into view and adds a brief pulse class.
  const taskColumnPulse = useUiStore(s => s.taskColumnPulse)
  useEffect(() => {
    if (!taskColumnPulse.colId || taskColumnPulse.nonce === 0) return
    const el = document.querySelector<HTMLElement>(
      `[data-kanban-col="${taskColumnPulse.colId}"]`
    )
    if (!el) return
    el.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' })
    el.classList.add('kanban-col-pulse')
    const t = setTimeout(() => el.classList.remove('kanban-col-pulse'), 1500)
    return () => clearTimeout(t)
  }, [taskColumnPulse.nonce, taskColumnPulse.colId])

  // Column definitions — matches mockup's 4-column layout
  // "Ready" merges ready + assigned (both are pre-work states)
  // "Done Today" filters completed_at to today; failed tasks also show here with ✗ style
  const readyTasks     = tasks.filter(t => t.status === 'ready' || t.status === 'assigned')
  const inProgressTasks= tasks.filter(t => t.status === 'in_progress')
  const qcTasks        = tasks.filter(t => t.status === 'qc_review')
  const rejectedTasks  = tasks.filter(t => t.status === 'rejected')
  // Tasks that the Supervisor or a worker has escalated for human / budget input —
  // surfacing them keeps stuck work visible instead of invisibly buried beneath Done.
  const needsInfoTasks = tasks.filter(t => t.status === 'needs_info')
  const doneTodayTasks = tasks.filter(t =>
    (t.status === 'done' && isToday(t.completed_at)) ||
    (t.status === 'failed' && isToday(t.updated_at)) ||
    (t.status === 'build_failed' && isToday(t.updated_at))
  )

  const COLS = [
    { colId: 'ready',       label: 'Ready',       tasks: readyTasks      },
    { colId: 'in_progress', label: 'In Progress', tasks: inProgressTasks },
    { colId: 'qc_review',   label: 'QC Review',   tasks: qcTasks         },
    { colId: 'needs_info',  label: 'Needs Info',  tasks: needsInfoTasks  },
    { colId: 'rejected',    label: 'Rejected',    tasks: rejectedTasks   },
    { colId: 'done_today',  label: 'Done Today',  tasks: doneTodayTasks  },
  ]

  return (
    <div
      className="flex flex-col flex-1 overflow-hidden"
      style={{ background: 'var(--bg-app)', fontFamily: 'IBM Plex Mono, monospace' }}
    >
      {/* Panel header */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid var(--border-1)', background: 'var(--bg-surface)' }}
      >
        <span style={{ fontSize: '11px', color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Agent Roster
        </span>
        <div className="flex items-center gap-3">
<div className="flex gap-2">
          <div className="flex items-center gap-2">
            {lastRefreshed && (
              <span style={{ fontSize: '9px', color: 'var(--text-4)', fontFamily: 'monospace' }}>
                ↺ {formatLastRefreshed(lastRefreshed)}
              </span>
            )}
            <button
              onClick={doRefresh}
              className="flex items-center gap-1"
              disabled={isRefreshing}
              style={{
                background: 'var(--bg-card)', border: '1px solid var(--border-1)', borderRadius: 2,
                padding: '4px 10px', color: isRefreshing ? 'var(--accent)' : 'var(--text-2)',
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

      {/* Orphan Alert — shows toast when worker processes die (Ruflo Feature 2) */}
      <ErrorBoundary label="Orphan Alert" inline><OrphanAlert /></ErrorBoundary>

      {/* Agent roster table — always visible (the primary live state) */}
      <ErrorBoundary label="Agent Roster" inline><AgentRoster tasks={tasks} /></ErrorBoundary>

      {/*
        ───────────────────────────────────────────────────────────────────
        Observability tab strip — replaces the previous stack of five
        always-expanded panels (Live Monitor / Worker Requests / Worker
        Affinity / Epics / QC ROI) that made the AutoDev tab feel crowded.
        Only one section is open at a time; user's choice is persisted in
        localStorage so the preference sticks across sessions. The default
        is "Live Monitor" because that's what the user most often wants to
        see during a run.
        ───────────────────────────────────────────────────────────────────
      */}
      <ErrorBoundary label="Observability Tabs" inline>
        <ObservabilityTabs
          tasks={tasks}
          workerRequests={workerRequests}
          onResolveRequest={(id, action, note) => resolveWorkerRequest(id, action, note)}
        />
      </ErrorBoundary>

      {/* Task board label */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid var(--border-1)', background: 'var(--bg-surface)' }}
      >
        <span style={{ fontSize: '11px', color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Task Board
        </span>
        <div className="flex items-center gap-2">
          {clearError && (
            <span
              style={{
                fontSize:   '10px',
                color:      'var(--danger)',
                fontFamily: 'monospace',
                maxWidth:   320,
                overflow:   'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
              title={clearError}
            >
              ✗ {clearError}
            </span>
          )}
          <button
            onClick={async () => {
              if (!clearConfirm) {
                setClearError(null)
                setClearConfirm(true)
                setTimeout(() => setClearConfirm(false), 3000)
                return
              }
              setClearBusy(true)
              try {
                // The Rust command deletes worker_requests first, then
                // tasks, inside a transaction. If it still fails (e.g.
                // disk-full / db locked), surface the message instead of
                // letting the unhandled rejection vanish.
                await invoke('clear_all_tasks')
                setClearError(null)
                await loadTasks()
              } catch (e) {
                const msg = typeof e === 'string' ? e : (e as Error).message ?? 'clear failed'
                setClearError(msg)
                // Auto-dismiss the error after 6s so the toolbar doesn't
                // permanently advertise stale errors.
                setTimeout(() => setClearError(null), 6_000)
              } finally {
                setClearConfirm(false)
                setClearBusy(false)
              }
            }}
            disabled={clearBusy}
            className="flex items-center gap-1"
            style={{
              background:   clearConfirm ? 'rgba(255,68,68,0.12)' : 'var(--bg-card)',
              border:       clearConfirm ? '1px solid rgba(255,68,68,0.3)' : '1px solid var(--border-1)',
              borderRadius: 2,
              padding:      '3px 8px',
              color:        clearConfirm ? 'var(--danger)' : 'var(--text-2)',
              fontSize:     '10px',
              fontFamily:   'monospace',
              cursor:       clearBusy ? 'wait' : 'pointer',
              opacity:      clearBusy ? 0.6 : 1,
            }}
            title={clearConfirm ? 'Click again to confirm — deletes all tasks' : 'Clear all tasks from board'}
          >
            <Eraser size={9} />
            <span>{clearBusy ? 'Clearing…' : clearConfirm ? 'Confirm clear?' : 'Clear Board'}</span>
          </button>
        </div>
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
            // Doc 06 Phase 3 — wrapper carries `data-kanban-col` so the
            // QC card's "jump to qc_review" click handler can locate this
            // column without prop-drilling refs through KanbanColumn.
            <div key={col.colId} data-kanban-col={col.colId}>
              <KanbanColumn
                colId={col.colId}
                label={col.label}
                tasks={col.tasks}
                onDelete={deleteTask}
              />
            </div>
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
          color: 'var(--bg-app)',
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
          <ErrorBoundary label="Subagent Monitor" inline><SubagentMonitor /></ErrorBoundary>
        </div>
      )}
    </div>
  )
}
