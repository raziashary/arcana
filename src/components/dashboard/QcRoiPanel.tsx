// QcRoiPanel — Doc 04 Phase 4.3.
//
// Surfaces per-task QC cost vs. worker cost so the user can spot tasks where
// the QC reviewer spent more than the implementer. Also shows the live
// reviewer count from `get_qc_reviewer_stats` (Phase 3.3).
//
// Data sources:
//   • taskStore.tasks  — `task_qc_cost_usd` + `budget_cost_usd` per task
//   • invoke('get_qc_reviewer_stats') — { in_flight, max_concurrent }
//
// Rendering: top 8 tasks by qc_cost descending. Each row shows
//   #id  title  qc:$0.04  worker:$0.18  ratio:22%
// with the ratio coloured red when qc > worker (suspicious).
//
// Refresh policy:
//   • Tasks come from the store, which already fans out on `task:changed`.
//   • The reviewer stats poll runs every 5 s and on `subagent:event`.

import { useEffect, useMemo, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTaskStore } from '../../stores/taskStore'

interface QcStats {
  in_flight:       number
  max_concurrent:  number
}

interface TaskRoi {
  id:        number
  title:     string
  qcCost:    number
  workerCost: number
  ratio:     number   // qcCost / max(workerCost, 0.0001)
}

const ZERO_STATS: QcStats = { in_flight: 0, max_concurrent: 6 }

export function QcRoiPanel() {
  const tasks = useTaskStore(s => s.tasks)
  const [stats, setStats] = useState<QcStats>(ZERO_STATS)
  const [err,  setErr]    = useState<string | null>(null)

  // Live reviewer count
  useEffect(() => {
    let cancelled = false
    async function fetchStats() {
      try {
        const v = await invoke<QcStats>('get_qc_reviewer_stats')
        if (!cancelled) {
          setStats(v)
          setErr(null)
        }
      } catch (e) {
        if (!cancelled) {
          setErr(typeof e === 'string' ? e : (e as Error).message ?? 'failed')
        }
      }
    }
    fetchStats()
    const id = setInterval(fetchStats, 5_000)
    // Also refresh on every subagent event so the count is near-instant.
    const unlisten = listen('subagent:event', () => fetchStats())
    return () => {
      cancelled = true
      clearInterval(id)
      unlisten.then(fn => fn())
    }
  }, [])

  // Top-N tasks by QC cost (descending), with at least 1 cent reviewer spend
  // so the panel never renders 30 empty rows.
  const rows: TaskRoi[] = useMemo(() => {
    const eligible = tasks
      .filter(t => (t.task_qc_cost_usd ?? 0) > 0.001)
      .map(t => {
        const qc = t.task_qc_cost_usd ?? 0
        const wc = t.budget_cost_usd  ?? 0
        const ratio = qc / Math.max(wc, 0.0001)
        return { id: t.id, title: t.title, qcCost: qc, workerCost: wc, ratio }
      })
      .sort((a, b) => b.qcCost - a.qcCost)
      .slice(0, 8)
    return eligible
  }, [tasks])

  const totalQcCost     = rows.reduce((s, r) => s + r.qcCost,     0)
  const totalWorkerCost = rows.reduce((s, r) => s + r.workerCost, 0)
  const totalRatio      = totalQcCost / Math.max(totalWorkerCost, 0.0001)

  return (
    <div
      className="qc-roi-panel"
      style={{
        background:    'var(--bg-surface)',
        border:        '1px solid var(--border-1)',
        borderRadius:  3,
        padding:       '12px 14px',
        fontFamily:    'var(--font-mono)',
        fontSize:      '11px',
        color:         'var(--text-1)',
      }}
    >
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: 8,
      }}>
        <span style={{
          color: 'var(--text-2)', letterSpacing: '0.1em', textTransform: 'uppercase',
        }}>
          QC ROI
        </span>
        <span style={{
          fontSize: '10px',
          padding: '1px 6px',
          borderRadius: 3,
          background: stats.in_flight > 0 ? 'var(--info)' : 'var(--bg-app)',
          color: stats.in_flight > 0 ? '#000' : 'var(--text-2)',
          border: '1px solid var(--border-1)',
        }}>
          Reviewers: {stats.in_flight} / {stats.max_concurrent}
        </span>
      </div>

      {err && (
        <div style={{ color: 'var(--danger)', fontSize: 10, marginBottom: 6 }}>
          stats: {err}
        </div>
      )}

      {rows.length === 0 ? (
        <div style={{ color: 'var(--text-3)', fontSize: 10, padding: '8px 0' }}>
          No QC activity yet. Cost rows appear when the qc-dispatcher fans out
          per-task reviewers.
        </div>
      ) : (
        <>
          <div style={{
            display: 'grid',
            gridTemplateColumns: '40px 1fr 70px 70px 60px',
            gap: 6,
            color: 'var(--text-3)',
            fontSize: 9,
            paddingBottom: 4,
            borderBottom: '1px solid var(--border-1)',
            marginBottom: 4,
          }}>
            <span>#</span><span>TASK</span>
            <span style={{ textAlign: 'right' }}>QC$</span>
            <span style={{ textAlign: 'right' }}>WORKER$</span>
            <span style={{ textAlign: 'right' }}>QC/W</span>
          </div>
          {rows.map(r => {
            const suspicious = r.ratio > 1
            return (
              <div key={r.id} style={{
                display: 'grid',
                gridTemplateColumns: '40px 1fr 70px 70px 60px',
                gap: 6,
                fontSize: 10,
                padding: '2px 0',
              }}>
                <span style={{ color: 'var(--text-3)' }}>#{r.id}</span>
                <span style={{
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }} title={r.title}>{r.title}</span>
                <span style={{ textAlign: 'right', color: 'var(--info)' }}>
                  ${r.qcCost.toFixed(3)}
                </span>
                <span style={{ textAlign: 'right', color: 'var(--text-2)' }}>
                  ${r.workerCost.toFixed(3)}
                </span>
                <span style={{
                  textAlign: 'right',
                  color: suspicious ? 'var(--danger)' : 'var(--accent)',
                }}>
                  {(r.ratio * 100).toFixed(0)}%
                </span>
              </div>
            )
          })}
          <div style={{
            display: 'grid',
            gridTemplateColumns: '40px 1fr 70px 70px 60px',
            gap: 6,
            fontSize: 10,
            paddingTop: 4,
            marginTop: 4,
            borderTop: '1px solid var(--border-1)',
            color: 'var(--text-2)',
          }}>
            <span />
            <span>Total (top {rows.length})</span>
            <span style={{ textAlign: 'right' }}>${totalQcCost.toFixed(3)}</span>
            <span style={{ textAlign: 'right' }}>${totalWorkerCost.toFixed(3)}</span>
            <span style={{
              textAlign: 'right',
              color: totalRatio > 1 ? 'var(--danger)' : 'var(--accent)',
            }}>
              {(totalRatio * 100).toFixed(0)}%
            </span>
          </div>
        </>
      )}
    </div>
  )
}
