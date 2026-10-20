// Logs tab — Real-time log viewer for ~/.arcana/logs/arcana.YYYY-MM-DD.log
// Auto-refreshes every 3 seconds. Lines colour-coded by level.
// All log events from Rust tracing (INFO/WARN/ERROR/DEBUG) appear here.
//
// Truncation: `read_log_file` returns only the last 3 000 lines by default.
// A banner appears at the top when a log file exceeds that limit, offering
// "Download full log" which calls `read_log_file_full` over a separate
// command. This keeps the IPC payload bounded and the UI responsive even
// when a single-day log grows large during heavy fleet activity.

import { useEffect, useRef, useState, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
// The WRITE is done in Rust (`export_log_file`), not with
// @tauri-apps/plugin-fs. That plugin's JS half was declared in package.json
// while its Rust half was never added to Cargo.toml, never registered, and
// never granted a capability — so writeTextFile threw at runtime while
// TypeScript compiled and CI stayed green, and this export silently did
// nothing. Keeping the write in Rust also avoids granting the frontend a broad
// filesystem scope.
import { RefreshCw, ChevronDown, Download, Copy, Check, Trash2, AlertTriangle } from 'lucide-react'
import { parseLogLine } from '../utils/logParser'

// ─── Log line presentation ────────────────────────────────────────────────────

type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE' | 'PANIC' | 'UNKNOWN'
type LogLine = ReturnType<typeof parseLogLine>

interface StructuredRecord {
  timestamp?: string
  level?: string
  target?: string
  fields?: Record<string, unknown>
  spans?: Array<Record<string, unknown>>
}

interface TraceNode {
  identity: string
  name: string
  children: Map<string, TraceNode>
  events: StructuredRecord[]
}

function buildTraceTree(records: StructuredRecord[], search = ''): TraceNode[] {
  const roots = new Map<string, TraceNode>()
  for (const [recordIndex, record] of records.entries()) {
    const spanFields = [...(record.spans ?? [])].reverse().flatMap(span => {
      // tracing_subscriber's production JSON formatter stores span fields
      // directly on each span object (for example { name, task_id, run_id }),
      // while older fixtures/readers may use an explicit `fields` object.
      const values = span.fields ?? span
      return values && typeof values === 'object' ? [values as Record<string, unknown>] : []
    })
    const fields = [...spanFields, record.fields ?? {}].reduce<Record<string, unknown>>(
      (all, values) => ({ ...all, ...values }),
      {},
    )
    // Keep a Fleet run as the root when present, and nest individual calls
    // beneath it. Without a Fleet ID, prefer the call so sibling calls from
    // one task remain separate roots.
    const fleetField = ['fleet_correlation_id', 'correlation_id']
      .find(field => fields[field] !== undefined && fields[field] !== null && String(fields[field]).trim() !== '')
    const correlationField = fleetField ?? ['call_id', 'session_id', 'task_id', 'run_id', 'trace_id']
      .find(field => fields[field] !== undefined && fields[field] !== null && String(fields[field]).trim() !== '')
    const correlationKind = correlationField === 'fleet_correlation_id' || correlationField === 'correlation_id'
      ? 'fleet'
      : correlationField?.replace(/_id$/, '')
    const correlation = correlationField
      ? `${correlationKind} ${String(fields[correlationField])}`
      : 'Unscoped operations'
    if (search && !JSON.stringify({ correlation, record }).toLowerCase().includes(search.toLowerCase())) continue

    let root = roots.get(correlation)
    if (!root) {
      root = { identity: correlation, name: correlation, children: new Map(), events: [] }
      roots.set(correlation, root)
    }
    const path = [
      ...(fleetField && fields.call_id !== undefined && fields.call_id !== null && String(fields.call_id).trim() !== ''
        ? [{ identity: `call:${String(fields.call_id)}`, name: `call ${String(fields.call_id)}` }]
        : []),
      ...(record.spans ?? []).flatMap((span, spanIndex) => {
        const name = String(span.name ?? '').trim()
        if (!name) return []
        const explicitIdentity = span.span_id ?? span.spanId ?? span.id
        // tracing_subscriber's JSON formatter does not include span IDs, so
        // use a record-local key when the producer has not supplied one.
        // This keeps repeated same-named operations as distinct visible nodes
        // instead of merging their event durations into one apparent span.
        const identity = explicitIdentity === undefined || explicitIdentity === null
          ? `record:${recordIndex}:span:${spanIndex}`
          : `span:${String(explicitIdentity)}`
        return [{ identity, name }]
      }),
    ]
    let siblings = root.children
    let leaf: TraceNode = root
    for (const span of path) {
      let child = siblings.get(span.identity)
      if (!child) {
        child = { ...span, children: new Map(), events: [] }
        siblings.set(span.identity, child)
      }
      leaf = child
      siblings = leaf.children
    }
    leaf.events.push({ ...record, fields })
  }
  return Array.from(roots.values())
}

const LEVEL_COLOR: Record<LogLevel, string> = {
  ERROR:   '#FF6363',
  WARN:    '#FFCB8B',
  INFO:    '#82AAFF',
  DEBUG:   '#637777',
  TRACE:   '#3D5A6E',
  PANIC:   '#FF6363',
  UNKNOWN: '#637777',
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function LogsTab() {
  const [dates,        setDates]        = useState<string[]>([])
  const [selectedDate, setSelectedDate] = useState<string>('')
  const [lines,        setLines]        = useState<LogLine[]>([])
  const [autoScroll,   setAutoScroll]   = useState(true)
  const [filter,       setFilter]       = useState<LogLevel | 'ALL'>('ALL')
  const [search,       setSearch]       = useState('')
  const [loading,      setLoading]      = useState(false)
  const [copied,       setCopied]        = useState(false)
  const [rawContent,   setRawContent]    = useState('')
  const [structuredRaw, setStructuredRaw] = useState('')
  const [structuredTotalLines, setStructuredTotalLines] = useState(0)
  const [structuredTruncated, setStructuredTruncated] = useState(false)
  const [structuredError, setStructuredError] = useState<string | null>(null)
  const [metricsError, setMetricsError] = useState<string | null>(null)
  const [metrics, setMetrics] = useState<Record<string, unknown> | null>(null)
  const [traceSearch, setTraceSearch] = useState('')
  const [correlationDraft, setCorrelationDraft] = useState('')
  const [correlationFilter, setCorrelationFilter] = useState('')
  const [totalLines,   setTotalLines]    = useState(0)
  const [truncated,    setTruncated]     = useState(false)
  const [fullSize,     setFullSize]      = useState(0)
  const [refreshMs,    setRefreshMs]     = useState<number | null>(null)
  // Export failures used to go to console.error only, so a user clicking
  // "Export" saw nothing happen and had no way to know why.
  const [exportError,  setExportError]   = useState<string | null>(null)
  const [loadError,    setLoadError]     = useState<string | null>(null)
  const [clearConfirm, setClearConfirm]  = useState(false)
  const [clearingLogs, setClearingLogs]  = useState(false)
  const bottomRef    = useRef<HTMLDivElement>(null)
  const intervalRef  = useRef<ReturnType<typeof setInterval> | null>(null)
  const loadInFlight = useRef(false)
  const loadPending  = useRef(false)
  const [queuedRefresh, setQueuedRefresh] = useState(0)

  // Host health is the only channel that can say "there is no file sink". It
  // must be read even when no log file exists yet, otherwise an unwritable log
  // directory renders as an empty, quiet day.
  const loadHealth = useCallback(async () => {
    try {
      const metricsResult = await invoke<Record<string, unknown>>('summarize_observability_events', { date: '' })
      setMetricsError(null)
      setMetrics(metricsResult)
    } catch (error) {
      setMetricsError(String(error))
      setMetrics(null)
    }
  }, [])

  // Load available log file dates
  const loadDates = useCallback(async () => {
    try {
      const d = await invoke<string[]>('list_log_files')
      setDates(d)
      if (d.length === 0) {
        await loadHealth()
      }
      if (d.length > 0 && (!selectedDate || selectedDate === dates[0])) {
        setSelectedDate(d[0]) // default to newest
      }
    } catch (error) {
      setLoadError(`Could not list log files: ${String(error)}`)
    }
  }, [dates, selectedDate, loadHealth])

  // Read the selected log file (tail-truncated) and parse lines
  interface LogFileResult {
    content:         string
    total_lines:     number
    truncated:       boolean
    full_size_bytes: number
  }
  const loadLog = useCallback(async () => {
    if (loadInFlight.current) {
      loadPending.current = true
      return
    }
    loadInFlight.current = true
    const startedAt = performance.now()
    setLoading(true)
    try {
      const [readableResult, structuredResult, metricsResult] = await Promise.allSettled([
        invoke<LogFileResult>('read_log_file', { date: selectedDate, tailLines: 3000 }),
        invoke<LogFileResult>('read_structured_events', {
          date: selectedDate,
          tailLines: 2000,
          ...(correlationFilter ? { correlationId: correlationFilter } : {}),
        }),
        invoke<Record<string, unknown>>('summarize_observability_events', {
          date: selectedDate,
          ...(correlationFilter ? { correlationId: correlationFilter } : {}),
        }),
      ])
      if (readableResult.status === 'fulfilled') {
        const result = readableResult.value
        setLoadError(null)
        setRawContent(result.content)
        setTotalLines(result.total_lines)
        setTruncated(result.truncated)
        setFullSize(result.full_size_bytes)
        setLines(result.content.split('\n').filter(line => line.trim().length > 0).map(parseLogLine))
      } else {
        setLoadError(`Could not read the selected log: ${String(readableResult.reason)}`)
        setRawContent('')
        setTotalLines(0)
        setTruncated(false)
        setFullSize(0)
        setLines([])
      }
      if (structuredResult.status === 'fulfilled') {
        setStructuredError(null)
        setStructuredRaw(structuredResult.value.content)
        setStructuredTotalLines(structuredResult.value.total_lines)
        setStructuredTruncated(structuredResult.value.truncated)
      } else {
        setStructuredError(String(structuredResult.reason))
        setStructuredRaw('')
        setStructuredTotalLines(0)
        setStructuredTruncated(false)
      }
      if (metricsResult.status === 'fulfilled') {
        setMetricsError(null)
        setMetrics(metricsResult.value)
      } else {
        setMetricsError(String(metricsResult.reason))
        setMetrics(null)
      }
      setRefreshMs(Math.round(performance.now() - startedAt))
    } finally {
      loadInFlight.current = false
      setLoading(false)
      if (loadPending.current) {
        loadPending.current = false
        setQueuedRefresh(refresh => refresh + 1)
      }
    }
  }, [selectedDate, correlationFilter])

  // Initial load
  useEffect(() => {
    loadDates()
    // eslint-disable-next-line react-hooks/exhaustive-deps -- initial discovery runs once; the refresh timer owns subsequent reads.
  }, [])

  // Load log when date changes
  useEffect(() => {
    if (selectedDate !== undefined) loadLog()
  }, [selectedDate, loadLog, queuedRefresh])

  // Auto-refresh every 3 seconds when viewing today's log
  useEffect(() => {
    if (intervalRef.current) clearInterval(intervalRef.current)
    intervalRef.current = setInterval(() => {
      if (!correlationFilter && dates.length > 0 && selectedDate === dates[0]) {
        loadLog()
        loadDates()
      }
    }, 3000)
    return () => { if (intervalRef.current) clearInterval(intervalRef.current) }
  }, [selectedDate, dates, loadLog, loadDates, correlationFilter])

  // Auto-scroll to bottom when new lines arrive
  useEffect(() => {
    if (autoScroll) bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [lines, autoScroll])

  // Apply level + search filter
  const visibleLines = lines.filter(l => {
    if (filter !== 'ALL' && l.level !== filter) return false
    if (search && !l.raw.toLowerCase().includes(search.toLowerCase())) return false
    if (correlationFilter && !l.raw.includes(correlationFilter)) return false
    return true
  })

  const errorCount = lines.filter(l => l.level === 'ERROR' || l.level === 'PANIC').length
  const warnCount  = lines.filter(l => l.level === 'WARN').length
  const structuredRecords = structuredRaw
    .split('\n')
    .filter(line => line.trim().length > 0)
    .flatMap((line): StructuredRecord[] => {
      try { return [JSON.parse(line) as StructuredRecord] } catch { return [] }
    })
    .filter(record => !search || JSON.stringify(record).toLowerCase().includes(search.toLowerCase()))
  const traces = structuredRecords.filter(record => record.spans?.length || record.fields?.duration_ns !== undefined)
  const traceTree = buildTraceTree(traces, traceSearch)
  const fullWindowMetrics = metrics ?? {}

  return (
    <div className="flex flex-col flex-1 overflow-hidden" style={{ background: 'var(--bg-surface)' }}>

      {/* ── Header ── */}
      <div
        style={{
          borderBottom: '1px solid var(--border-1)',
          background:   'var(--bg-panel)',
          padding:      '8px 14px',
          flexShrink:   0,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, flexWrap: 'wrap' }}>
          {/* Title */}
          <span style={{ fontFamily: 'monospace', fontSize: 10, color: 'var(--text-3)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
            System Logs
          </span>

          {/* Date selector */}
          <div style={{ position: 'relative', display: 'flex', alignItems: 'center' }}>
            <select
              title="Select log date"
              value={selectedDate}
              onChange={e => setSelectedDate(e.target.value)}
              style={{
                background:   'var(--bg-card)',
                border:       '1px solid var(--border-2)',
                borderRadius: 2,
                color:        'var(--text-1)',
                fontFamily:   'monospace',
                fontSize:     10,
                padding:      '3px 24px 3px 8px',
                appearance:   'none',
                cursor:       'pointer',
              }}
            >
              {dates.length === 0 && (
                <option value="">No log files yet</option>
              )}
              {dates.map(d => (
                <option key={d} value={d}>{d}</option>
              ))}
            </select>
            <ChevronDown size={10} style={{ position: 'absolute', right: 6, color: 'var(--text-3)', pointerEvents: 'none' }} />
          </div>

          <label style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--text-3)', fontFamily: 'monospace', fontSize: 9 }}>
            Run / correlation
            <input
              aria-label="Filter logs by run or correlation ID"
              value={correlationDraft}
              onChange={event => setCorrelationDraft(event.target.value)}
              onKeyDown={event => {
                if (event.key === 'Enter') setCorrelationFilter(correlationDraft.trim())
              }}
              placeholder="fleet ID, run ID, task ID..."
              style={{ width: 210, background: 'var(--bg-card)', border: '1px solid var(--border-2)', color: 'var(--text-1)', padding: '3px 7px', fontFamily: 'monospace', fontSize: 9 }}
            />
          </label>
          <button
            onClick={() => setCorrelationFilter(correlationDraft.trim())}
            style={{ background: 'var(--bg-card)', border: '1px solid var(--border-2)', borderRadius: 2, padding: '3px 7px', color: 'var(--text-2)', cursor: 'pointer', fontFamily: 'monospace', fontSize: 9 }}
          >
            {correlationFilter ? 'Apply run filter' : 'Filter run'}
          </button>
          {correlationFilter && (
            <button
              onClick={() => { setCorrelationDraft(''); setCorrelationFilter('') }}
              style={{ background: 'transparent', border: '1px solid var(--border-2)', borderRadius: 2, padding: '3px 7px', color: 'var(--text-3)', cursor: 'pointer', fontFamily: 'monospace', fontSize: 9 }}
            >Clear run</button>
          )}

          {/* Level filter buttons */}
          {(['ALL', 'ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE', 'PANIC'] as const).map(lvl => (
            <button
              key={lvl}
              onClick={() => setFilter(lvl)}
              style={{
                background:   filter === lvl ? 'var(--bg-selected)' : 'transparent',
                border:       `1px solid ${filter === lvl ? 'var(--border-2)' : 'transparent'}`,
                borderRadius: 2,
                padding:      '2px 7px',
                fontFamily:   'monospace',
                fontSize:     9,
                color:        lvl === 'ALL' ? 'var(--text-2)' : LEVEL_COLOR[lvl as LogLevel],
                cursor:       'pointer',
                letterSpacing: '0.05em',
              }}
            >
              {lvl}
              {lvl === 'ERROR' && errorCount > 0 && (
                <span style={{ marginLeft: 4, background: '#FF636330', borderRadius: 2, padding: '0 3px' }}>
                  {errorCount}
                </span>
              )}
              {lvl === 'WARN' && warnCount > 0 && (
                <span style={{ marginLeft: 4, background: '#FFCB8B20', borderRadius: 2, padding: '0 3px' }}>
                  {warnCount}
                </span>
              )}
            </button>
          ))}

          {/* Search */}
          <input
            type="text"
            placeholder="Search logs…"
            value={search}
            onChange={e => setSearch(e.target.value)}
            style={{
              background:   'var(--bg-card)',
              border:       '1px solid var(--border-2)',
              borderRadius: 2,
              color:        'var(--text-1)',
              fontFamily:   'monospace',
              fontSize:     10,
              padding:      '3px 8px',
              outline:      'none',
              flex:         1,
              minWidth:     120,
              maxWidth:     260,
            }}
          />

          {/* Spacer */}
          <div style={{ flex: 1 }} />

          {/* Auto-scroll toggle */}
          <button
            onClick={() => setAutoScroll(a => !a)}
            style={{
              background:   autoScroll ? 'var(--bg-selected)' : 'transparent',
              border:       '1px solid var(--border-2)',
              borderRadius: 2,
              padding:      '2px 7px',
              fontFamily:   'monospace',
              fontSize:     9,
              color:        autoScroll ? 'var(--aqua)' : 'var(--text-3)',
              cursor:       'pointer',
            }}
          >
            Auto-scroll
          </button>

          {/* Copy all */}
          <button
            onClick={async () => {
              if (!rawContent) return
              await navigator.clipboard.writeText(rawContent)
              setCopied(true)
              setTimeout(() => setCopied(false), 2000)
            }}
            disabled={!rawContent}
            title={truncated ? 'Copy the visible tail; use Full to export the complete log' : 'Copy all loaded log lines'}
            style={{
              background:   'var(--bg-card)',
              border:       '1px solid var(--border-2)',
              borderRadius: 2,
              padding:      '2px 7px',
              color:        copied ? 'var(--aqua)' : 'var(--text-2)',
              cursor:       rawContent ? 'pointer' : 'not-allowed',
              display:      'flex',
              alignItems:   'center',
              gap:          4,
            }}
          >
            {copied ? <Check size={9} /> : <Copy size={9} />}
            <span style={{ fontFamily: 'monospace', fontSize: 9 }}>{copied ? 'Copied!' : 'Copy'}</span>
          </button>

          {/* Export truncated (visible) */}
          <button
            onClick={async () => {
              if (!rawContent) return
              try {
                const defaultPath = `arcana.${selectedDate || 'log'}.log`
                const filePath = await save({
                  defaultPath,
                  filters: [{
                    name: 'Log Files',
                    extensions: ['log']
                  }]
                })
                if (filePath) {
                  await invoke<string>('export_log_file', {
                    date: selectedDate,
                    destPath: filePath,
                    full: false,
                    tailLines: null,
                  })
                }
              } catch (e) {
                console.error('Failed to export log:', e)
                setExportError(e instanceof Error ? e.message : String(e))
              }
            }}
            disabled={!rawContent}
            title="Download visible (truncated) log"
            style={{
              background:   'var(--bg-card)',
              border:       '1px solid var(--border-2)',
              borderRadius: 2,
              padding:      '2px 7px',
              color:        'var(--text-2)',
              cursor:       rawContent ? 'pointer' : 'not-allowed',
              display:      'flex',
              alignItems:   'center',
              gap:          4,
            }}
          >
            <Download size={9} />
            <span style={{ fontFamily: 'monospace', fontSize: 9 }}>Export</span>
          </button>

          {/* Download full log (only shown when truncated) */}
          {truncated && (
            <button
              onClick={async () => {
                try {
                  const defaultPath = `arcana.${selectedDate || 'log'}-FULL.log`
                  const filePath = await save({
                    defaultPath,
                    filters: [{
                      name: 'Log Files',
                      extensions: ['log']
                    }]
                  })
                  if (filePath) {
                    await invoke<string>('export_log_file', {
                      date: selectedDate,
                      destPath: filePath,
                      full: true,
                      tailLines: null,
                    })
                  }
                } catch (e) {
                  console.error('Failed to download full log:', e)
                  setExportError(e instanceof Error ? e.message : String(e))
                }
              }}
              title="Download full log file (not truncated)"
              style={{
                background:   '#FFCB8B18',
                border:       '1px solid #FFCB8B50',
                borderRadius: 2,
                padding:      '2px 7px',
                color:        '#FFCB8B',
                cursor:       'pointer',
                display:      'flex',
                alignItems:   'center',
                gap:          4,
              }}
            >
              <Download size={9} />
              <span style={{ fontFamily: 'monospace', fontSize: 9 }}>Full</span>
            </button>
          )}

          <button
            onClick={async () => {
              try {
                const filePath = await save({
                  defaultPath: `arcana-observability-${selectedDate || 'today'}.json`,
                  filters: [{ name: 'JSON Export', extensions: ['json'] }],
                })
                if (filePath) {
                  await invoke<string>('export_observability_bundle', {
                    date: selectedDate,
                    destPath: filePath,
                    ...(correlationFilter ? { correlationId: correlationFilter } : {}),
                  })
                }
              } catch (e) {
                setExportError(e instanceof Error ? e.message : String(e))
              }
            }}
            title="Export the complete readable log and structured event file for this date"
            style={{
              background: 'var(--bg-card)',
              border: '1px solid var(--border-2)',
              borderRadius: 2,
              padding: '2px 7px',
              color: 'var(--aqua)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              gap: 4,
            }}
          >
            <Download size={9} />
            <span style={{ fontFamily: 'monospace', fontSize: 9 }}>Export observability</span>
          </button>

          {/* Clear all log files */}
          <button
            onClick={async () => {
              if (!clearConfirm) {
                setClearConfirm(true)
                setTimeout(() => setClearConfirm(false), 3000)
                return
              }
              setClearingLogs(true)
              setClearConfirm(false)
              try {
                await invoke('clear_log_files')
                setDates([])
                setSelectedDate('')
                setLines([])
                setRawContent('')
                setStructuredRaw('')
              } catch (e) {
                console.error('clear_log_files failed:', e)
                setLoadError(`Could not clear logs: ${String(e)}`)
              } finally {
                setClearingLogs(false)
              }
            }}
            disabled={clearingLogs}
            title={clearConfirm ? 'Click again to permanently delete ALL log files' : 'Delete all log files from disk'}
            style={{
              background:   clearConfirm ? '#FF636318' : 'var(--bg-card)',
              border:       `1px solid ${clearConfirm ? '#FF636350' : 'var(--border-2)'}`,
              borderRadius: 2,
              padding:      '2px 7px',
              color:        clearConfirm ? '#FF6363' : 'var(--text-3)',
              cursor:       clearingLogs ? 'not-allowed' : 'pointer',
              display:      'flex',
              alignItems:   'center',
              gap:          4,
              transition:   'color 0.2s, background 0.2s',
            }}
          >
            <Trash2 size={9} />
            <span style={{ fontFamily: 'monospace', fontSize: 9 }}>
              {clearConfirm ? 'Confirm?' : clearingLogs ? 'Clearing…' : 'Clear All'}
            </span>
          </button>

          {/* Manual refresh */}
          <button
            onClick={() => { loadLog(); loadDates() }}
            disabled={loading}
            style={{
              background:   'var(--bg-card)',
              border:       '1px solid var(--border-2)',
              borderRadius: 2,
              padding:      '2px 7px',
              color:        'var(--text-2)',
              cursor:       loading ? 'wait' : 'pointer',
              display:      'flex',
              alignItems:   'center',
              gap:          4,
            }}
          >
            <RefreshCw size={9} style={{ animation: loading ? 'spin 1s linear infinite' : 'none' }} />
            <span style={{ fontFamily: 'monospace', fontSize: 9 }}>Refresh</span>
          </button>
        </div>

        {/* Stats row + truncation banner */}
        <div style={{ marginTop: 4, display: 'flex', gap: 12, alignItems: 'center', flexWrap: 'wrap' }}>
          <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-4)' }}>
            {visibleLines.length} / {lines.length} lines
            {refreshMs !== null && (
              <span style={{ marginLeft: 8 }}>refresh {refreshMs} ms · {fullSize.toLocaleString()} bytes</span>
            )}
            {truncated && (
              <span style={{ color: '#FFCB8B', marginLeft: 4 }}>
                (showing last {lines.length.toLocaleString()} of {totalLines.toLocaleString()} total)
              </span>
            )}
          </span>
          {truncated && (
            <span style={{
              fontFamily: 'monospace',
              fontSize: 9,
              color: '#FFCB8B',
              display: 'flex',
              alignItems: 'center',
              gap: 4,
            }}>
              <AlertTriangle size={9} />
              {(fullSize / 1024 / 1024).toFixed(1)} MB on disk
            </span>
          )}
          {exportError && (
            <span
              onClick={() => setExportError(null)}
              title="Click to dismiss"
              style={{
                fontFamily: 'monospace',
                fontSize: 9,
                color: 'var(--warning)',
                display: 'flex',
                alignItems: 'center',
                gap: 4,
                cursor: 'pointer',
              }}
            >
              <AlertTriangle size={9} />
              Export failed: {exportError}
            </span>
          )}
          {loadError && (
            <span role="alert" style={{ fontFamily: 'monospace', fontSize: 9, color: '#FF6363' }}>
              {loadError}
            </span>
          )}
          {dates.length > 0 && selectedDate === dates[0] && (
            <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--aqua)', opacity: 0.6 }}>
              ● live
            </span>
          )}
          {fullWindowMetrics.file_logging_unavailable === true ? (
            <span role="status" style={{ fontFamily: 'monospace', fontSize: 9, color: '#FF6363' }}>
              File logging unavailable in this process — console only. Empty sections below mean no file sink exists, not that nothing happened.
            </span>
          ) : dates.length === 0 && (
            <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--warning)' }}>
              No log files found — restart the app to generate today's log.
            </span>
          )}
        </div>
      </div>

      {/* ── Log lines ── */}
      <div
        style={{
          flex:       1,
          overflowY:  'auto',
          padding:    '4px 0',
          fontFamily: 'monospace',
          fontSize:   11,
        }}
        onScroll={e => {
          const el = e.currentTarget
          const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 32
          setAutoScroll(atBottom)
        }}
      >
        <details open style={{ borderBottom: '1px solid var(--border-1)' }}>
          <summary style={{ cursor: 'pointer', padding: '9px 14px', color: 'var(--text-1)', fontFamily: 'monospace', fontSize: 11 }}>
            Logs <span style={{ color: 'var(--text-4)' }}>({visibleLines.length}{truncated ? ` / ${totalLines}` : ''})</span>
          </summary>
          {visibleLines.length === 0 && !loading && (
          <div style={{ padding: '24px 20px', color: 'var(--text-4)', fontFamily: 'monospace', fontSize: 11, textAlign: 'center' }}>
            {lines.length === 0
              ? 'No log entries yet. The log file will appear after the first app startup with this version.'
              : 'No lines match the current filter.'
            }
          </div>
          )}

          {visibleLines.map((line, i) => (
          <div
            key={i}
            style={{
              display:    'flex',
              alignItems: 'flex-start',
              gap:        10,
              padding:    '2px 14px',
              background: line.level === 'ERROR' || line.level === 'PANIC'
                ? 'rgba(255, 99, 99, 0.04)'
                : line.level === 'WARN' ? 'rgba(255, 203, 139, 0.03)' : 'transparent',
              borderLeft: line.level === 'ERROR' || line.level === 'PANIC'
                ? '2px solid #FF636340'
                : line.level === 'WARN' ? '2px solid #FFCB8B40' : '2px solid transparent',
            }}
          >
            {/* Timestamp */}
            {line.timestamp && (
              <span style={{ color: 'var(--text-4)', flexShrink: 0, fontSize: 10 }}>
                {line.timestamp.split('T')[1]?.split('.')[0] ?? line.timestamp.slice(11, 19)}
              </span>
            )}

            {/* Level badge */}
            <span style={{
              color:      LEVEL_COLOR[line.level],
              flexShrink: 0,
              width:      36,
              fontSize:   9,
              fontWeight: 700,
              letterSpacing: '0.05em',
            }}>
              {line.level}
            </span>

            {/* Module (dimmed) */}
            {line.module && (
              <span style={{ color: 'var(--text-4)', flexShrink: 0, maxWidth: 200, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontSize: 10 }}>
                {line.module}
              </span>
            )}

            {/* Message */}
            <span style={{ color: 'var(--text-1)', flex: 1, wordBreak: 'break-word', whiteSpace: 'pre-wrap' }}>
              {line.message}
            </span>
          </div>
          ))}

          <div ref={bottomRef} />
        </details>

        <details style={{ borderBottom: '1px solid var(--border-1)' }}>
          <summary style={{ cursor: 'pointer', padding: '9px 14px', color: 'var(--text-1)', fontFamily: 'monospace', fontSize: 11 }}>
            Structured Events <span style={{ color: 'var(--text-4)' }}>({structuredRecords.length}{structuredTruncated ? ` of ${structuredTotalLines} · tail shown` : ''}{structuredError ? ' · read error' : ''})</span>
          </summary>
          {structuredError && <div role="alert" style={{ padding: '8px 14px', color: '#FF6363' }}>{structuredError}</div>}
          {structuredTruncated && <div role="status" style={{ padding: '8px 14px', color: '#FFCB8B' }}>
            Showing the newest 2,000 structured events of {structuredTotalLines.toLocaleString()}. Export the observability bundle for the complete selected date.
          </div>}
          {structuredRecords.length === 0 && !structuredError && <div style={{ padding: '12px 14px', color: 'var(--text-4)' }}>No structured telemetry has been recorded for this date.</div>}
          {structuredRecords.map((record, index) => (
            <details key={`${record.timestamp ?? 'event'}-${index}`} style={{ padding: '4px 14px', borderTop: '1px solid var(--border-1)' }}>
              <summary style={{ cursor: 'pointer', color: 'var(--text-2)', fontFamily: 'monospace', fontSize: 10 }}>
                {record.timestamp ?? 'time unavailable'} · {String(record.fields?.event_type ?? record.fields?.message ?? record.target ?? 'event')}
                {record.fields?.database ? ` · ${String(record.fields.database)}` : ''}
                {record.fields?.duration_ns !== undefined ? ` · ${(Number(record.fields.duration_ns) / 1_000_000).toFixed(2)} ms` : ''}
              </summary>
              <pre style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', margin: '8px 0', padding: 10, background: 'var(--bg-card)', color: 'var(--text-2)', fontSize: 10 }}>
                <code>{JSON.stringify(record, null, 2)}</code>
              </pre>
            </details>
          ))}
        </details>

        <details style={{ borderBottom: '1px solid var(--border-1)' }}>
          <summary style={{ cursor: 'pointer', padding: '9px 14px', color: 'var(--text-1)', fontFamily: 'monospace', fontSize: 11 }}>
            Traces <span style={{ color: 'var(--text-4)' }}>({traces.length})</span>
          </summary>
          <div style={{ padding: '8px 14px' }}>
            <input
              aria-label="Filter traces by correlation"
              type="search"
              placeholder="Filter by trace, run, task, session, or call ID"
              value={traceSearch}
              onChange={event => setTraceSearch(event.target.value)}
              style={{ width: '100%', maxWidth: 420, background: 'var(--bg-card)', border: '1px solid var(--border-2)', color: 'var(--text-1)', padding: '5px 8px', fontFamily: 'monospace', fontSize: 10 }}
            />
          </div>
          {traces.length === 0 && <div style={{ padding: '12px 14px', color: 'var(--text-4)' }}>Timed spans appear here as instrumented operations complete.</div>}
          {traces.length > 0 && traceTree.length === 0 && <div style={{ padding: '12px 14px', color: 'var(--text-4)' }}>No traces match this correlation filter.</div>}
          {traces.length === 0 && <div style={{ padding: '12px 14px', color: 'var(--text-4)' }}>No timed operation records for this date.</div>}
          {traceTree.map((node, index) => <TraceTreeNode key={`${node.name}-${index}`} node={node} />)}
        </details>

        <details style={{ borderBottom: '1px solid var(--border-1)' }}>
          <summary style={{ cursor: 'pointer', padding: '9px 14px', color: 'var(--text-1)', fontFamily: 'monospace', fontSize: 11 }}>
            Metrics <span style={{ color: 'var(--text-4)' }}>({correlationFilter ? 'selected correlation' : 'complete selected date'})</span>
          </summary>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(150px, 1fr))', gap: 8, padding: 12 }}>
            <MetricCard label="Provider calls" value={Number(fullWindowMetrics.provider_calls ?? 0).toLocaleString()} />
            <MetricCard label="Tool calls" value={Number(fullWindowMetrics.tool_calls ?? 0).toLocaleString()} />
            <MetricCard label="DB statements" value={Number(fullWindowMetrics.database_statements ?? 0).toLocaleString()} />
            <MetricCard label="Input tokens" value={Number(fullWindowMetrics.input_tokens ?? 0).toLocaleString()} />
            <MetricCard label="Output tokens" value={Number(fullWindowMetrics.output_tokens ?? 0).toLocaleString()} />
            <MetricCard label="Failed outcomes" value={Object.values((fullWindowMetrics.by_event ?? {}) as Record<string, { failures?: number }>)
              .reduce((total, aggregate) => total + Number(aggregate.failures ?? 0), 0).toLocaleString()} />
            <MetricCard label="Readable writer errors" value={Number(fullWindowMetrics.readable_writer_failures ?? 0).toLocaleString()} />
            <MetricCard label="Structured writer errors" value={Number(fullWindowMetrics.structured_writer_failures ?? 0).toLocaleString()} />
          </div>
          <div style={{ padding: '0 12px 12px', color: 'var(--text-4)', fontFamily: 'monospace', fontSize: 9 }}>
            {correlationFilter ? 'Selected-correlation totals' : 'Whole-day totals'} from {Number(fullWindowMetrics.total_events ?? 0).toLocaleString()} structured events
            {' · writer errors are process-lifetime totals'}
            {fullWindowMetrics.file_logging_unavailable === true && ' · file logging unavailable (console only)'}
            {Number(fullWindowMetrics.malformed_events ?? 0) > 0 && ` · ${Number(fullWindowMetrics.malformed_events).toLocaleString()} malformed`}
          {metricsError && <div role="alert" style={{ color: '#FF6363', marginTop: 6 }}>Metrics unavailable: {metricsError}</div>}
            <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap', marginTop: 8, color: 'var(--text-2)' }}>
              <span>{Number(fullWindowMetrics.provider_calls ?? 0)} provider calls</span>
              <span>{Number(fullWindowMetrics.provider_attempts ?? 0)} attempts/retries</span>
              <span>{Number(fullWindowMetrics.tool_calls ?? 0)} tool calls</span>
              <span>{Number(fullWindowMetrics.database_statements ?? 0)} DB statements</span>
              <span>{Number(fullWindowMetrics.input_tokens ?? 0).toLocaleString()} input tokens</span>
              <span>{Number(fullWindowMetrics.output_tokens ?? 0).toLocaleString()} output tokens</span>
              <span>{Number(fullWindowMetrics.cache_read_tokens ?? 0).toLocaleString()} cache reads</span>
              <span>{Number(fullWindowMetrics.cache_write_tokens ?? 0).toLocaleString()} cache writes</span>
              <span>{Number(fullWindowMetrics.reasoning_tokens ?? 0).toLocaleString()} reasoning tokens</span>
              <span>{Number(fullWindowMetrics.tool_tokens ?? 0).toLocaleString()} tool tokens</span>
            </div>
            <div style={{ marginTop: 8, maxHeight: 140, overflow: 'auto' }}>
              {Object.entries((fullWindowMetrics.by_event ?? {}) as Record<string, { count?: number; failures?: number; result_count_total?: number; result_bytes_total?: number; duration_count?: number; duration_zero_count?: number; duration_total_ns?: number; duration_max_ns?: number }> )
                .map(([kind, aggregate]) => (
                  <div key={kind} style={{ display: 'flex', gap: 12, padding: '2px 0' }}>
                    <span style={{ color: 'var(--aqua)', minWidth: 190 }}>{kind}</span>
                    <span>{Number(aggregate.count ?? 0)} calls</span>
                    <span>{Number(aggregate.failures ?? 0)} failures</span>
                    {Number(aggregate.result_count_total ?? 0) > 0 && <span>{Number(aggregate.result_count_total).toLocaleString()} results</span>}
                    {Number(aggregate.result_bytes_total ?? 0) > 0 && <span>{Number(aggregate.result_bytes_total).toLocaleString()} result bytes</span>}
                    {Number(aggregate.duration_count ?? 0) > 0 ? <>
                      <span>avg {(Number(aggregate.duration_total_ns ?? 0) / Number(aggregate.duration_count) / 1e6).toFixed(2)} ms</span>
                      <span>max {(Number(aggregate.duration_max_ns ?? 0) / 1e6).toFixed(2)} ms</span>
                    </> : Number(aggregate.duration_zero_count ?? 0) > 0
                      ? <span>below timer resolution ({Number(aggregate.duration_zero_count)} samples)</span>
                      : <span>duration unavailable</span>}
                  </div>
                ))}
            </div>
            <div style={{ marginTop: 10, maxHeight: 140, overflow: 'auto' }}>
              <div style={{ color: 'var(--text-3)', marginBottom: 4 }}>Provider / model</div>
              {Object.entries((fullWindowMetrics.by_provider_model ?? {}) as Record<string, { calls?: number; failures?: number; duration_count?: number; duration_zero_count?: number; duration_total_ns?: number; duration_max_ns?: number; input_tokens?: number; output_tokens?: number; result_blocks_total?: number }> )
                .map(([providerModel, aggregate]) => (
                  <div key={providerModel} style={{ display: 'flex', gap: 12, padding: '2px 0' }}>
                    <span style={{ color: 'var(--aqua)', minWidth: 190 }}>{providerModel}</span>
                    <span>{Number(aggregate.calls ?? 0)} calls</span>
                    <span>{Number(aggregate.failures ?? 0)} failed</span>
                    <span>{Number(aggregate.input_tokens ?? 0).toLocaleString()} in</span>
                    <span>{Number(aggregate.output_tokens ?? 0).toLocaleString()} out</span>
                    <span>{Number(aggregate.result_blocks_total ?? 0).toLocaleString()} result blocks</span>
                    {Number(aggregate.duration_count ?? 0) > 0 ? <>
                      <span>avg {(Number(aggregate.duration_total_ns ?? 0) / Number(aggregate.duration_count) / 1e6).toFixed(2)} ms</span>
                      <span>max {(Number(aggregate.duration_max_ns ?? 0) / 1e6).toFixed(2)} ms</span>
                    </> : Number(aggregate.duration_zero_count ?? 0) > 0
                      ? <span>below timer resolution ({Number(aggregate.duration_zero_count)} samples)</span>
                      : <span>duration unavailable</span>}
                  </div>
                ))}
            </div>
          </div>
        </details>
      </div>
    </div>
  )
}

function MetricCard({ label, value }: { label: string; value: string }) {
  return <div style={{ padding: 10, background: 'var(--bg-card)', border: '1px solid var(--border-1)' }}>
    <div style={{ color: 'var(--text-4)', fontFamily: 'monospace', fontSize: 9, textTransform: 'uppercase' }}>{label}</div>
    <div style={{ color: 'var(--text-1)', fontFamily: 'monospace', fontSize: 15, marginTop: 5 }}>{value}</div>
  </div>
}

function TraceTreeNode({ node }: { node: TraceNode }) {
  return <details open style={{ marginLeft: 12, borderLeft: '1px solid var(--border-2)' }}>
    <summary style={{ cursor: 'pointer', padding: '6px 10px', color: 'var(--aqua)', fontFamily: 'monospace', fontSize: 10 }}>
      {node.name}
    </summary>
    {node.events.map((record, index) => {
      const fields = record.fields ?? {}
      const operation = String(fields.operation ?? fields.event_type ?? fields.message ?? 'operation')
      const stage = fields.phase ?? fields.stage
      const label = stage === undefined
        ? operation
        : `${operation} · ${String(stage)}`
      const correlationContext = [
        ['task', fields.task_id],
        ['session', fields.session_id],
        ['call', fields.call_id],
        ['worker', fields.worker],
      ]
        .filter(([, value]) => value !== undefined && value !== null && String(value).trim() !== '')
        .map(([kind, value]) => `${kind} ${String(value)}`)
        .join(' · ')
      return <div key={`${record.timestamp ?? 'trace'}-${index}`} style={{ display: 'flex', gap: 10, padding: '4px 10px', borderTop: '1px solid var(--border-1)', fontFamily: 'monospace', fontSize: 10 }}>
        <span style={{ color: 'var(--text-4)', minWidth: 150 }}>{record.timestamp ?? ''}</span>
        <span style={{ color: 'var(--text-1)', flex: 1 }}>{label}{fields.outcome !== undefined ? ` · ${String(fields.outcome)}` : ''}{correlationContext ? ` · ${correlationContext}` : ''}</span>
        <span style={{ color: 'var(--aqua)' }}>{fields.duration_ns !== undefined ? `${(Number(fields.duration_ns) / 1_000_000).toFixed(2)} ms` : ''}</span>
      </div>
    })}
    {Array.from(node.children.values()).map(child => <TraceTreeNode key={child.identity} node={child} />)}
  </details>
}
