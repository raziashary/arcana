import { useState, useEffect, useCallback } from 'react'
import { invoke } from '@tauri-apps/api/core'

interface WorkerAffinityStats {
  worker_slug: string
  task_type: string
  total_tasks: number
  success_count: number
  failure_count: number
  success_rate: number
  avg_duration_ms: number | null
}

function formatDuration(ms: number | null): string {
  if (ms === null || ms === 0) return '—'
  if (ms < 60_000) return `${Math.round(ms / 1000)}s`
  return `${Math.round(ms / 60_000)}m`
}

function rateColor(rate: number): string {
  if (rate >= 0.85) return 'var(--green)'
  if (rate >= 0.60) return 'var(--yellow, #f5a623)'
  return 'var(--red, #e53e3e)'
}

export function WorkerAffinityPanel() {
  const [stats, setStats] = useState<WorkerAffinityStats[]>([])
  const [collapsed, setCollapsed] = useState(false)
  const [loading, setLoading] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const data = await invoke<WorkerAffinityStats[]>('get_worker_affinity_stats')
      setStats(data)
    } catch {
      // no routing history yet — empty table is fine
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { load() }, [load])

  // Group stats by worker_slug
  const byWorker = stats.reduce<Record<string, WorkerAffinityStats[]>>((acc, s) => {
    ;(acc[s.worker_slug] ??= []).push(s)
    return acc
  }, {})

  const workerSlugs = Object.keys(byWorker).sort()
  const taskTypes = [...new Set(stats.map(s => s.task_type))].sort()

  return (
    <div
      style={{
        margin: '8px 12px',
        border: '1px solid var(--border-1)',
        borderRadius: 6,
        overflow: 'hidden',
        background: 'var(--bg-surface)',
      }}
    >
      {/* Header */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 10px',
          borderBottom: collapsed ? 'none' : '1px solid var(--border-1)',
          cursor: 'pointer',
          userSelect: 'none',
        }}
        onClick={() => setCollapsed(c => !c)}
      >
        <span style={{ fontSize: 11, color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          Worker Affinity
        </span>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button
            onClick={e => { e.stopPropagation(); load() }}
            style={{
              fontSize: 10, padding: '1px 6px',
              border: '1px solid var(--border-1)', borderRadius: 3,
              background: 'transparent', color: 'var(--text-2)', cursor: 'pointer',
            }}
          >
            {loading ? '…' : 'Refresh'}
          </button>
          <span style={{ fontSize: 11, color: 'var(--text-3)' }}>{collapsed ? '▸' : '▾'}</span>
        </div>
      </div>

      {/* Table */}
      {!collapsed && (
        stats.length === 0 ? (
          <div style={{ padding: '12px 10px', fontSize: 12, color: 'var(--text-3)', textAlign: 'center' }}>
            No routing history yet. Assignments will appear here once workers complete tasks.
          </div>
        ) : (
          <div style={{ overflowX: 'auto' }}>
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 11 }}>
              <thead>
                <tr style={{ background: 'var(--bg-raised)' }}>
                  <th style={thStyle}>Worker</th>
                  {taskTypes.map(tt => (
                    <th key={tt} style={thStyle}>{tt}</th>
                  ))}
                  <th style={thStyle}>Overall Rate</th>
                </tr>
              </thead>
              <tbody>
                {workerSlugs.map(slug => {
                  const workerStats = byWorker[slug]
                  const totalSuccess = workerStats.reduce((n, s) => n + s.success_count, 0)
                  const totalDone    = workerStats.reduce((n, s) => n + s.success_count + s.failure_count, 0)
                  const overallRate  = totalDone > 0 ? totalSuccess / totalDone : null

                  return (
                    <tr key={slug} style={{ borderTop: '1px solid var(--border-0)' }}>
                      <td style={{ ...tdStyle, color: 'var(--text-1)', fontWeight: 500 }}>
                        {slug}
                      </td>
                      {taskTypes.map(tt => {
                        const s = workerStats.find(x => x.task_type === tt)
                        if (!s) return <td key={tt} style={{ ...tdStyle, color: 'var(--text-3)' }}>—</td>
                        const pct = s.success_count + s.failure_count > 0
                          ? Math.round(s.success_rate * 100)
                          : null
                        return (
                          <td key={tt} style={tdStyle}>
                            <span style={{ color: pct !== null ? rateColor(s.success_rate) : 'var(--text-3)' }}>
                              {s.total_tasks} {pct !== null ? `(${pct}%)` : ''}
                            </span>
                            {s.avg_duration_ms !== null && (
                              <span style={{ color: 'var(--text-3)', marginLeft: 4 }}>
                                {formatDuration(s.avg_duration_ms)}
                              </span>
                            )}
                          </td>
                        )
                      })}
                      <td style={tdStyle}>
                        {overallRate !== null ? (
                          <span style={{ color: rateColor(overallRate), fontWeight: 600 }}>
                            {Math.round(overallRate * 100)}%
                          </span>
                        ) : (
                          <span style={{ color: 'var(--text-3)' }}>—</span>
                        )}
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        )
      )}
    </div>
  )
}

const thStyle: React.CSSProperties = {
  padding: '4px 8px',
  textAlign: 'left',
  color: 'var(--text-3)',
  fontWeight: 500,
  borderBottom: '1px solid var(--border-1)',
}

const tdStyle: React.CSSProperties = {
  padding: '4px 8px',
  color: 'var(--text-2)',
  whiteSpace: 'nowrap',
}
