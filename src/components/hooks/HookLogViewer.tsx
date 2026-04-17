// HookLogViewer — shows recent hook execution history with outcome badges.
// Supports: outcome filter tabs, expandable rows with full context, JSON export.
// Phase 3.1

import { useEffect, useState } from 'react'
import { CheckCircle, XCircle, MinusCircle, Ban, RefreshCw, ChevronDown, ChevronUp, Download } from 'lucide-react'
import { useHookStore, type HookExecutionLog, type HookOutcome } from '../../stores/hookStore'

type FilterMode = 'all' | HookOutcome

interface Props {
  hookId?: string
  limit?:  number
}

export default function HookLogViewer({ hookId, limit = 50 }: Props) {
  const store  = useHookStore()
  const [filter, setFilter] = useState<FilterMode>('all')

  useEffect(() => {
    store.loadLogs(hookId, limit)
  }, [hookId, limit])

  const filtered = filter === 'all'
    ? store.logs
    : store.logs.filter(l => l.outcome === filter)

  function handleExport() {
    const blob = new Blob([JSON.stringify(filtered, null, 2)], { type: 'application/json' })
    const url  = URL.createObjectURL(blob)
    const a    = document.createElement('a')
    a.href     = url
    a.download = `hook-logs-${Date.now()}.json`
    a.click()
    URL.revokeObjectURL(url)
  }

  const FILTERS: FilterMode[] = ['all', 'executed', 'failed', 'blocked', 'skipped']

  return (
    <div className="space-y-2">
      {/* Header row */}
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-xs font-mono font-semibold" style={{ color: 'var(--text-2)' }}>
          Execution Log
        </span>

        {/* Filter tabs */}
        <div className="flex items-center gap-1 flex-1 flex-wrap">
          {FILTERS.map(f => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className="text-xs px-2 py-0.5 rounded font-mono"
              style={{
                background: filter === f ? 'var(--bg-selected)' : 'transparent',
                color:      filter === f ? 'var(--accent)' : 'var(--text-3)',
                border:     `1px solid ${filter === f ? 'var(--accent)' : 'transparent'}`,
              }}
            >
              {f}
              {f !== 'all' && (
                <span className="ml-1" style={{ fontSize: '10px' }}>
                  ({store.logs.filter(l => l.outcome === f).length})
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-1">
          {filtered.length > 0 && (
            <button
              title="Export logs as JSON"
              onClick={handleExport}
              className="p-1 rounded hover:opacity-70"
            >
              <Download size={12} style={{ color: 'var(--text-3)' }} />
            </button>
          )}
          <button
            title="Refresh logs"
            onClick={() => store.loadLogs(hookId, limit)}
            className="p-1 rounded hover:opacity-70"
          >
            <RefreshCw size={12} style={{ color: 'var(--text-3)' }} />
          </button>
        </div>
      </div>

      {/* Log rows */}
      {filtered.length === 0 ? (
        <div className="text-xs py-4 text-center" style={{ color: 'var(--text-3)' }}>
          {store.logs.length === 0 ? 'No hook executions yet' : `No ${filter} executions`}
        </div>
      ) : (
        <div className="space-y-1">
          {filtered.map(log => (
            <LogRow key={log.id} log={log} />
          ))}
        </div>
      )}
    </div>
  )
}

function outcomeIcon(outcome: HookOutcome) {
  switch (outcome) {
    case 'executed': return <CheckCircle size={13} style={{ color: 'var(--success)' }} />
    case 'blocked':  return <Ban         size={13} style={{ color: 'var(--danger)' }} />
    case 'skipped':  return <MinusCircle size={13} style={{ color: 'var(--text-3)' }} />
    case 'failed':   return <XCircle     size={13} style={{ color: 'var(--danger)' }} />
  }
}

function outcomeColor(outcome: HookOutcome): string {
  switch (outcome) {
    case 'executed': return 'var(--success)'
    case 'blocked':  return 'var(--danger)'
    case 'failed':   return 'var(--danger)'
    default:         return 'var(--text-3)'
  }
}

function formatTime(ts: string): string {
  try {
    return new Date(ts).toLocaleTimeString()
  } catch {
    return ts
  }
}

function LogRow({ log }: { log: HookExecutionLog }) {
  const [expanded, setExpanded] = useState(false)
  const hasDetail = !!(log.output || log.error)

  return (
    <div
      className="rounded text-xs font-mono"
      style={{ background: 'var(--bg-2)', border: '1px solid var(--border)' }}
    >
      {/* Summary row */}
      <div className="flex items-center gap-2 p-2">
        {outcomeIcon(log.outcome)}
        <span style={{ color: 'var(--text-1)', fontWeight: 500 }}>{log.hook_name}</span>
        <span style={{ color: 'var(--text-3)' }}>·</span>
        <span style={{ color: 'var(--text-3)' }}>{log.hook_type}</span>
        <span className="ml-auto" style={{ color: outcomeColor(log.outcome), textTransform: 'uppercase', fontSize: '10px' }}>
          {log.outcome}
        </span>
        <span style={{ color: 'var(--text-3)' }}>{log.duration_ms}ms</span>
        <span style={{ color: 'var(--text-3)' }}>{formatTime(log.executed_at)}</span>
        {hasDetail && (
          <button
            title={expanded ? 'Collapse' : 'Expand details'}
            onClick={() => setExpanded(e => !e)}
            className="p-0.5 hover:opacity-70"
          >
            {expanded
              ? <ChevronUp   size={11} style={{ color: 'var(--text-3)' }} />
              : <ChevronDown size={11} style={{ color: 'var(--text-3)' }} />
            }
          </button>
        )}
      </div>

      {/* Expanded detail */}
      {expanded && (
        <div
          className="px-3 pb-2 space-y-1"
          style={{ borderTop: '1px solid var(--border)' }}
        >
          {log.output && (
            <div className="pt-2">
              <span style={{ color: 'var(--text-3)', fontSize: '10px' }}>output: </span>
              <pre className="whitespace-pre-wrap mt-0.5" style={{ color: 'var(--text-1)', fontSize: '10px' }}>
                {log.output}
              </pre>
            </div>
          )}
          {log.error && (
            <div className="pt-1">
              <span style={{ color: 'var(--danger)', fontSize: '10px' }}>error: </span>
              <pre className="whitespace-pre-wrap mt-0.5" style={{ color: 'var(--danger)', fontSize: '10px' }}>
                {log.error}
              </pre>
            </div>
          )}
          <div style={{ color: 'var(--text-4)', fontSize: '10px' }}>
            id: {log.id} · hook: {log.hook_id}
          </div>
        </div>
      )}
    </div>
  )
}
