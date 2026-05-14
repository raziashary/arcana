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
  type WorkerRequest,
} from '../stores/taskStore'
import {
  useWorkerStore,
  workerStatusColor,
  WORKER_TYPE_LABELS,
} from '../stores/workerStore'
import { useSidecarStore } from '../stores/sidecarStore'
import { SubagentMonitor } from '../components/subagent/SubagentMonitor'
import { LiveMonitorSection } from '../components/autodev/LiveMonitorSection'
import { useAgentLiveStore }  from '../stores/agentLiveStore'
import { WorkerAffinityPanel } from '../components/dashboard/WorkerAffinityPanel'
import { OrphanAlert } from '../components/dashboard/OrphanAlert'
import { BudgetBar } from '../components/budget/BudgetBar'
import { DiffAnalysisCard } from '../components/dashboard/DiffAnalysisCard'
import { EpicsSection } from '../components/dashboard/EpicPhaseView'
import { ErrorBoundary } from '../components/ErrorBoundary'
import '../components/autodev/LiveMonitor.css'

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

  if (!agent || agent.status === 'idle' || agent.status === 'stopped') {
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
  const initAgents     = useAgentLiveStore(s => s.initAgents)
  const [, tick]       = useState(0)

  // Re-render every second so elapsed timers update live
  useEffect(() => {
    const id = setInterval(() => tick(n => n + 1), 1_000)
    return () => clearInterval(id)
  }, [])

  // Seed the live store with the current worker list whenever it loads
  useEffect(() => {
    const ids = workerStore.workers.map(w => w.slug)
    if (ids.length > 0) initAgents(ids)
  }, [workerStore.workers.length]) // eslint-disable-line react-hooks/exhaustive-deps

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

        // Per-worker mode badge.
        // Priority: current_tick_mode (mid-tick, live) > last_outcome (historical).
        // current_tick_mode is set on tick-start and cleared on tick-end.
        const lastOutcome    = worker.last_outcome
        const currentMode    = worker.current_tick_mode
        const modeBadge =
          currentMode === 'mode2'
            ? { label: 'LA', color: '#7FDBCA', title: 'Mode 2 — LivingAgent (tick in progress)', glow: true }
          : currentMode === 'mode1'
            ? { label: 'M1', color: '#FFCB8B', title: 'Mode 1 — Rust Engine (tick in progress)', glow: false }
          : lastOutcome === 'success'
            ? { label: 'LA', color: '#7FDBCA', title: 'Mode 2 — LivingAgent (last tick)', glow: false }
          : lastOutcome === 'mode1_forced'
            ? { label: 'M1', color: '#FFCB8B', title: 'Mode 1 — Rust Engine (MCP unreachable on pre-flight)', glow: false }
          : lastOutcome === 'fallback'
            ? { label: 'M1', color: '#FFCB8B', title: 'Mode 1 — Rust Engine (Mode 2 invoke failed)', glow: false }
          :   { label: '—',  color: '#3D5A6E', title: 'No tick data yet', glow: false }
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
  onResolve: (id: number, action: 'completed' | 'rejected', note?: string) => void
}) {
  const [expandedId, setExpandedId] = useState<number | null>(null)
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

  return (
    <div
      style={{
        borderBottom: '1px solid var(--border-1)',
        background:   'rgba(127,219,202,0.04)',
      }}
    >
      <div
        className="flex items-center justify-between px-4 py-2 cursor-pointer"
        onClick={() => setExpandedId(expandedId === -1 ? null : -1)}
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
        {expandedId === -1 ? <ChevronDown size={12} color="var(--text-4)" /> : <ChevronRight size={12} color="var(--text-4)" />}
      </div>

      {expandedId === -1 && (
        <div className="px-4 pb-3 flex flex-col gap-2">
          {requests.map(req => {
            const isOpen = expandedId === req.id
            let payloadPreview = ''
            try {
              const p = JSON.parse(req.payload)
              if (p.audit_summary) payloadPreview = p.audit_summary as string
              else if (p.issue_description) payloadPreview = p.issue_description as string
              else if (p.reason) payloadPreview = p.reason as string
              else payloadPreview = JSON.stringify(p).slice(0, 120)
            } catch { payloadPreview = req.payload.slice(0, 120) }

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
                  onClick={() => setExpandedId(isOpen ? -1 : req.id)}
                >
                  <div className="flex items-center gap-2">
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
                  </div>
                  {isOpen ? <ChevronDown size={10} color="var(--text-4)" /> : <ChevronRight size={10} color="var(--text-4)" />}
                </div>

                {isOpen && (
                  <div className="px-3 pb-3">
                    <div
                      style={{ color: 'var(--text-3)', fontSize: '10px', marginBottom: 8, whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}
                    >
                      {payloadPreview}
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={() => onResolve(req.id, 'completed')}
                        style={{
                          background: 'rgba(127,219,202,0.15)',
                          border: '1px solid rgba(127,219,202,0.3)',
                          borderRadius: 2,
                          padding: '2px 8px',
                          color: '#7FDBCA',
                          fontSize: '9px',
                          fontFamily: 'monospace',
                          cursor: 'pointer',
                        }}
                      >
                        Ack / Complete
                      </button>
                      <button
                        onClick={() => onResolve(req.id, 'rejected', 'Human override — declined')}
                        style={{
                          background: 'rgba(255,99,99,0.10)',
                          border: '1px solid rgba(255,99,99,0.25)',
                          borderRadius: 2,
                          padding: '2px 8px',
                          color: '#FF6363',
                          fontSize: '9px',
                          fontFamily: 'monospace',
                          cursor: 'pointer',
                        }}
                      >
                        Decline
                      </button>
                    </div>
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
          {/* core-orchestrator sidecar status badge */}
          <div
            className="flex items-center gap-1"
            title={`core-orchestrator: ${sidecarStatus}`}
            style={{ fontSize: '9px', color: sidecarConnected ? 'var(--accent)' : 'var(--text-2)', letterSpacing: '0.05em' }}
          >
            <span style={{
              width: 5, height: 5, borderRadius: '50%', display: 'inline-block',
              background: sidecarConnected ? 'var(--accent)'
                : sidecarStatus === 'starting' ? 'var(--warning)'
                : 'var(--text-4)',
              boxShadow: sidecarConnected ? '0 0 4px var(--accent)' : 'none',
            }} />
            <span>{sidecarConnected ? 'LivingAgent' : 'Mode 1'}</span>
          </div>
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

      {/* Agent roster table */}
      <ErrorBoundary label="Agent Roster" inline><AgentRoster tasks={tasks} /></ErrorBoundary>

      {/* Worker Affinity — historical routing performance per worker (Ruflo Feature 1) */}
      <ErrorBoundary label="Worker Affinity" inline><WorkerAffinityPanel /></ErrorBoundary>

      {/* Epics — Phase-based task decomposition view (Ruflo Feature 5) */}
      <ErrorBoundary label="Epics" inline><EpicsSection tasks={tasks} /></ErrorBoundary>

      {/* Worker Requests — Fleet→Supervisor escalation queue */}
      <ErrorBoundary label="Worker Requests" inline>
        <WorkerRequestsWidget
          requests={workerRequests}
          onResolve={(id, action, note) => resolveWorkerRequest(id, action, note)}
        />
      </ErrorBoundary>

      {/* Live Monitor — real-time per-agent state cards */}
      <ErrorBoundary label="Live Monitor" inline><LiveMonitorSection /></ErrorBoundary>

      {/* Task board label */}
      <div
        className="flex items-center justify-between px-4 py-2 shrink-0"
        style={{ borderBottom: '1px solid var(--border-1)', background: 'var(--bg-surface)' }}
      >
        <span style={{ fontSize: '11px', color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
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
            background:   clearConfirm ? 'rgba(255,68,68,0.12)' : 'var(--bg-card)',
            border:       clearConfirm ? '1px solid rgba(255,68,68,0.3)' : '1px solid var(--border-1)',
            borderRadius: 2,
            padding:      '3px 8px',
            color:        clearConfirm ? 'var(--danger)' : 'var(--text-2)',
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
