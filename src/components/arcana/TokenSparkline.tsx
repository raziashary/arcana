// TokenSparkline — token progress bar + 10-bar activity sparkline
// Used inside each AgentLiveCard.

import type { SparklineWindow } from '../../types/live'

interface Props {
  totalTokens:   number
  sparkline:     SparklineWindow[]
  budgetTokens?: number   // defaults to 100_000
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000)     return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

export function TokenSparkline({ totalTokens, sparkline, budgetTokens = 100_000 }: Props) {
  const fillPct  = Math.min((totalTokens / budgetTokens) * 100, 100)
  const barClass = fillPct > 90 ? 'token-bar-fg--danger'
                 : fillPct > 70 ? 'token-bar-fg--warning'
                 : 'token-bar-fg--normal'

  const maxWindow = Math.max(...sparkline.map(w => w.tokens), 1)

  return (
    <div className="token-sparkline">
      <div className="token-bar-row">
        <div className="token-bar-bg">
          <div className={`token-bar-fg ${barClass}`} style={{ width: `${fillPct}%` }} />
        </div>
        <span className="token-count">{formatTokens(totalTokens)}</span>
      </div>
      <div className="sparkline-bars" aria-hidden>
        {sparkline.map((w, i) => {
          const h      = Math.max(2, Math.round((w.tokens / maxWindow) * 16))
          const active = w.tokens > maxWindow * 0.5
          return (
            <div
              key={i}
              className={`spark-bar${active ? ' spark-bar--active' : ''}`}
              style={{ height: `${h}px` }}
            />
          )
        })}
      </div>
    </div>
  )
}
