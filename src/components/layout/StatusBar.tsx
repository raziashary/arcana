// StatusBar — bottom bar showing active model + today's cost.
// Design from autodevmockup.html .status-chip + .model-badge + .cost-badge

import { useCostStore }     from '../../stores/costStore'
import { useSettingsStore } from '../../stores/settingsStore'

function formatCost(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.001) return `$${usd.toFixed(6)}`
  if (usd < 1)    return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(2)}`
}

export default function StatusBar() {
  const costToday    = useCostStore(s => s.costTodayUsd)
  const tokensIn     = useCostStore(s => s.tokensInToday)
  const tokensOut    = useCostStore(s => s.tokensOutToday)
  const activeModel  = useSettingsStore(s => s.activeModel)
  const activeProvider = useSettingsStore(s => s.activeProvider)

  return (
    <div
      className="flex items-center justify-between px-3.5 shrink-0"
      style={{
        background:   'var(--bg-surface)',
        borderTop:    '1px solid var(--border-1)',
        height:       '28px',
        fontSize:     '10px',
        fontFamily:   'var(--font-mono)',
      }}
    >
      {/* Left: working directory / status */}
      <div style={{ color: 'var(--text-4)' }}>
        autodev
      </div>

      {/* Right: model + cost */}
      <div className="flex items-center gap-2" style={{ color: 'var(--text-3)' }}>
        {activeModel && (
          <span
            className="px-2 py-0.5"
            style={{
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              borderRadius: '2px',
              color:      'var(--text-2)',
            }}
          >
            {activeProvider && `${activeProvider}/`}{activeModel}
          </span>
        )}

        <span style={{ color: 'var(--accent)', fontSize: '10px' }}>
          {formatCost(costToday)} today
        </span>

        {(tokensIn > 0 || tokensOut > 0) && (
          <span style={{ color: 'var(--text-4)' }}>
            ↑{formatTokens(tokensIn)} ↓{formatTokens(tokensOut)}
          </span>
        )}
      </div>
    </div>
  )
}

function formatTokens(n: number): string {
  if (n < 1000) return String(n)
  if (n < 1_000_000) return `${(n / 1000).toFixed(1)}K`
  return `${(n / 1_000_000).toFixed(1)}M`
}
