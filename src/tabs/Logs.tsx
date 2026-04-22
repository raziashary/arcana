// Logs tab — Real-time log viewer for ~/.autodev/logs/autodev-YYYY-MM-DD.log
// Auto-refreshes every 3 seconds. Lines colour-coded by level.
// All log events from Rust tracing (INFO/WARN/ERROR/DEBUG) appear here.
//
// Truncation: `read_log_file` returns only the last 10 000 lines by default.
// A banner appears at the top when a log file exceeds that limit, offering
// "Download full log" which calls `read_log_file_full` over a separate
// command. This keeps the IPC payload bounded and the UI responsive even
// when a single-day log grows large during heavy fleet activity.

import { useEffect, useRef, useState, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { RefreshCw, ChevronDown, Download, Copy, Check, Trash2, AlertTriangle } from 'lucide-react'

// ─── Log line parser ──────────────────────────────────────────────────────────

type LogLevel = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE' | 'PANIC' | 'UNKNOWN'

interface LogLine {
  raw:       string
  timestamp: string
  level:     LogLevel
  module:    string
  message:   string
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

// Parse a tracing compact log line: "2026-04-15T12:58:00.000000Z  INFO crate::module: message"
function parseLine(raw: string): LogLine {
  // Match: timestamp  LEVEL crate::module: message
  const m = raw.match(/^(\S+)\s+(ERROR|WARN|INFO|DEBUG|TRACE)\s+([^:]+):\s*(.*)$/)
  if (m) {
    return {
      raw,
      timestamp: m[1].replace('T', ' ').replace('Z', ''),
      level:     m[2] as LogLevel,
      module:    m[3].trim(),
      message:   m[4].trim(),
    }
  }
  // Panic lines start with [PANIC]
  if (raw.includes('[PANIC]')) {
    return { raw, timestamp: '', level: 'PANIC', module: 'runtime', message: raw }
  }
  return { raw, timestamp: '', level: 'UNKNOWN', module: '', message: raw }
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
  const [totalLines,   setTotalLines]    = useState(0)
  const [truncated,    setTruncated]     = useState(false)
  const [fullSize,     setFullSize]      = useState(0)
  const [clearConfirm, setClearConfirm]  = useState(false)
  const [clearingLogs, setClearingLogs]  = useState(false)
  const bottomRef    = useRef<HTMLDivElement>(null)
  const intervalRef  = useRef<ReturnType<typeof setInterval> | null>(null)

  // Load available log file dates
  const loadDates = useCallback(async () => {
    try {
      const d = await invoke<string[]>('list_log_files')
      setDates(d)
      if (d.length > 0 && !selectedDate) {
        setSelectedDate(d[0]) // default to newest
      }
    } catch (_) { /* no log dir yet */ }
  }, [selectedDate])

  // Read the selected log file (tail-truncated) and parse lines
  interface LogFileResult {
    content:         string
    total_lines:     number
    truncated:       boolean
    full_size_bytes: number
  }
  const loadLog = useCallback(async () => {
    setLoading(true)
    try {
      const result = await invoke<LogFileResult>('read_log_file', {
        date: selectedDate,
        tailLines: 10000,
      })
      setRawContent(result.content)
      setTotalLines(result.total_lines)
      setTruncated(result.truncated)
      setFullSize(result.full_size_bytes)
      const parsed = result.content
        .split('\n')
        .filter(l => l.trim().length > 0)
        .map(parseLine)
      setLines(parsed)
    } catch (_) {
      setRawContent('')
      setTotalLines(0)
      setTruncated(false)
      setFullSize(0)
      setLines([])
    } finally {
      setLoading(false)
    }
  }, [selectedDate])

  // Initial load
  useEffect(() => {
    loadDates()
  }, [])

  // Load log when date changes
  useEffect(() => {
    if (selectedDate !== undefined) loadLog()
  }, [selectedDate, loadLog])

  // Auto-refresh every 3 seconds when viewing today's log
  useEffect(() => {
    if (intervalRef.current) clearInterval(intervalRef.current)
    intervalRef.current = setInterval(() => {
      if (dates.length > 0 && selectedDate === dates[0]) {
        loadLog()
        loadDates()
      }
    }, 3000)
    return () => { if (intervalRef.current) clearInterval(intervalRef.current) }
  }, [selectedDate, dates, loadLog, loadDates])

  // Auto-scroll to bottom when new lines arrive
  useEffect(() => {
    if (autoScroll) bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [lines, autoScroll])

  // Apply level + search filter
  const visibleLines = lines.filter(l => {
    if (filter !== 'ALL' && l.level !== filter) return false
    if (search && !l.raw.toLowerCase().includes(search.toLowerCase())) return false
    return true
  })

  const errorCount = lines.filter(l => l.level === 'ERROR' || l.level === 'PANIC').length
  const warnCount  = lines.filter(l => l.level === 'WARN').length

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

          {/* Level filter buttons */}
          {(['ALL', 'ERROR', 'WARN', 'INFO', 'DEBUG'] as const).map(lvl => (
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
            title="Copy entire log to clipboard"
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
            onClick={() => {
              if (!rawContent) return
              const blob = new Blob([rawContent], { type: 'text/plain' })
              const url  = URL.createObjectURL(blob)
              const a    = document.createElement('a')
              a.href     = url
              a.download = `autodev-${selectedDate || 'log'}.log`
              document.body.appendChild(a)
              a.click()
              document.body.removeChild(a)
              URL.revokeObjectURL(url)
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
                  const full = await invoke<string>('read_log_file_full', { date: selectedDate })
                  const blob = new Blob([full], { type: 'text/plain' })
                  const url  = URL.createObjectURL(blob)
                  const a    = document.createElement('a')
                  a.href     = url
                  a.download = `autodev-${selectedDate || 'log'}-FULL.log`
                  document.body.appendChild(a)
                  a.click()
                  document.body.removeChild(a)
                  URL.revokeObjectURL(url)
                } catch (e) {
                  console.error('Failed to download full log:', e)
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
              } catch (e) {
                console.error('clear_log_files failed:', e)
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
          {dates.length > 0 && selectedDate === dates[0] && (
            <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--aqua)', opacity: 0.6 }}>
              ● live
            </span>
          )}
          {dates.length === 0 && (
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
      </div>
    </div>
  )
}
