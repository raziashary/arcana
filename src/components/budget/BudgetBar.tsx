// BudgetBar — Ruflo Feature 4 (Budget Circuit Breaker, section 4.7).
//
// Renders per-task token and cost usage as a compact progress bar with
// colour-coded fill and an EXCEEDED badge. Consumed by task cards in the
// Arcana Panel when a task is in_progress or needs_info with a budget set.
//
// Colour thresholds (mirroring ADR-097):
//   green  < 50 %
//   yellow  50–80 %
//   orange  80–95 %
//   red    > 95 %  (or exceeded)

import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

// ── Types ──────────────────────────────────────────────────────────────────────

interface BudgetStatus {
  tokens_used:      number
  cost_usd:         number
  turns_used:       number
  exceeded:         boolean
  exceeded_reason:  string | null
}

interface BudgetBarProps {
  taskId:      number
  maxTokens?:  number   // None displayed as 'unlimited'
  maxUsd?:     number
  maxTurns?:   number
  compact?:    boolean  // single-line mode for narrow cards
}

// ── Helpers ────────────────────────────────────────────────────────────────────

function pct(used: number, max: number | undefined): number {
  if (!max || max === 0) return 0
  return Math.min(100, (used / max) * 100)
}

function fillClass(p: number, exceeded: boolean): string {
  if (exceeded || p > 95) return 'bg-red-500'
  if (p > 80)             return 'bg-orange-500'
  if (p > 50)             return 'bg-yellow-400'
  return 'bg-emerald-500'
}

function trackClass(p: number, exceeded: boolean): string {
  if (exceeded || p > 95) return 'bg-red-950/40'
  if (p > 80)             return 'bg-orange-950/40'
  if (p > 50)             return 'bg-yellow-950/40'
  return 'bg-emerald-950/40'
}

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000)     return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

// ── Component ──────────────────────────────────────────────────────────────────

export function BudgetBar({ taskId, maxTokens, maxUsd, maxTurns, compact = false }: BudgetBarProps) {
  const [status, setStatus] = useState<BudgetStatus | null>(null)

  const load = async () => {
    try {
      const s = await invoke<BudgetStatus>('get_task_budget_status', { taskId })
      setStatus(s)
    } catch {
      // Task may not have budget tracking — silently no-op
    }
  }

  useEffect(() => {
    void load()

    // Refresh on budget-exceeded events and task-board changes
    const unlisten = Promise.all([
      listen<{ task_id: number | null }>('agent:budget-exceeded', (e) => {
        if (e.payload.task_id === taskId) void load()
      }),
      listen<{ task_id: number | null }>('task:changed', (e) => {
        if (e.payload.task_id === taskId) void load()
      }),
    ])

    return () => { void unlisten.then(([a, b]) => { a(); b() }) }
  }, [taskId])

  if (!status) return null

  const tokenPct = pct(status.tokens_used, maxTokens)
  const costPct  = pct(status.cost_usd,    maxUsd)
  const exceeded = status.exceeded

  // Use the higher of the two percentages for colour coding
  const dominant = Math.max(tokenPct, costPct)

  if (compact) {
    return (
      <div className="flex items-center gap-2 text-[10px] text-zinc-400">
        {exceeded && (
          <span className="px-1 py-0.5 rounded bg-red-600 text-white font-bold text-[9px] tracking-wide">
            EXCEEDED
          </span>
        )}
        <div className={`flex-1 h-1.5 rounded-full overflow-hidden ${trackClass(dominant, exceeded)}`}>
          <div
            className={`h-full rounded-full transition-all duration-300 ${fillClass(dominant, exceeded)}`}
            style={{ width: `${Math.max(tokenPct, costPct)}%` }}
          />
        </div>
        <span className="shrink-0 tabular-nums">
          {fmt(status.tokens_used)}{maxTokens ? `/${fmt(maxTokens)}` : ''} tok
        </span>
      </div>
    )
  }

  return (
    <div className="space-y-1.5 text-[11px]">
      {exceeded && (
        <div className="flex items-center gap-1.5">
          <span className="px-1.5 py-0.5 rounded bg-red-600 text-white font-bold text-[10px] tracking-widest">
            EXCEEDED
          </span>
          {status.exceeded_reason && (
            <span className="text-red-400">{status.exceeded_reason.replace(/_/g, ' ')}</span>
          )}
        </div>
      )}

      {/* Token bar */}
      <div>
        <div className="flex justify-between text-zinc-400 mb-0.5">
          <span>Tokens</span>
          <span className="tabular-nums">
            {fmt(status.tokens_used)}{maxTokens ? ` / ${fmt(maxTokens)}` : ''}
          </span>
        </div>
        <div className={`h-1.5 rounded-full overflow-hidden ${trackClass(tokenPct, exceeded)}`}>
          <div
            className={`h-full rounded-full transition-all duration-300 ${fillClass(tokenPct, exceeded)}`}
            style={{ width: `${tokenPct}%` }}
          />
        </div>
      </div>

      {/* Cost bar — only render if a USD budget was set */}
      {maxUsd != null && (
        <div>
          <div className="flex justify-between text-zinc-400 mb-0.5">
            <span>Cost</span>
            <span className="tabular-nums">
              ${status.cost_usd.toFixed(4)} / ${maxUsd.toFixed(2)}
            </span>
          </div>
          <div className={`h-1.5 rounded-full overflow-hidden ${trackClass(costPct, exceeded)}`}>
            <div
              className={`h-full rounded-full transition-all duration-300 ${fillClass(costPct, exceeded)}`}
              style={{ width: `${costPct}%` }}
            />
          </div>
        </div>
      )}

      {/* Turns indicator */}
      {maxTurns != null && (
        <div className="text-zinc-500 tabular-nums">
          Turns: {status.turns_used} / {maxTurns}
        </div>
      )}
    </div>
  )
}
