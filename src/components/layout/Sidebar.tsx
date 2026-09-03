// Sidebar — spec Section 10.
// Live-data sections: CONTEXT, MCP ACTIVE, SKILLS, HOOKS, SESSION, + AUTODEV.md editor.
// Width: 176px. All data from real stores — no static stubs.

import { useEffect }           from 'react'
import { useAutodevMdStore }   from '../../stores/autodevMdStore'
import { useSkillStore }       from '../../stores/skillStore'
import { useHookStore }        from '../../stores/hookStore'
import { useCostStore }        from '../../stores/costStore'
import AutodevMdCard           from '../autodev/AutodevMdCard'

function fmtN(n: number): string {
  if (n < 1_000)     return String(n)
  if (n < 1_000_000) return `${(n / 1_000).toFixed(1)}k`
  return `${(n / 1_000_000).toFixed(1)}M`
}

export default function Sidebar() {
  const globalContent = useAutodevMdStore(s => s.globalContent)
  const loadGlobal    = useAutodevMdStore(s => s.loadGlobal)
  const byCategory    = useSkillStore(s => s.byCategory)
  const hooks         = useHookStore(s => s.hooks)
  const loadHooks     = useHookStore(s => s.loadHooks)
  const tokensIn      = useCostStore(s => s.tokensInToday)
  const tokensOut     = useCostStore(s => s.tokensOutToday)
  const costToday     = useCostStore(s => s.costTodayUsd)

  useEffect(() => {
    loadGlobal()
    loadHooks()
  }, [])

  // P8-55 — the orchestrator indicator is gone with the orchestrator.
  const autodevLoaded = globalContent.trim().length > 0
  const allSkills     = Object.values(byCategory).flat()
  const enabledHooks  = hooks.filter(h => h.enabled)
  const totalTokens   = tokensIn + tokensOut

  return (
    <div
      className="shrink-0 overflow-y-auto"
      style={{
        width:          '176px',
        borderLeft:     '1px solid var(--border-1)',
        background:     'var(--bg-surface)',
        padding:        '10px 8px',
        display:        'flex',
        flexDirection:  'column',
        gap:            '14px',
      }}
    >
      {/* ── CONTEXT ──────────────────────────────────────── */}
      <Section title="CONTEXT">
        <Dot label="AUTODEV.md" active={autodevLoaded} />
        <Dot label=".autodevhints" active={false} />
      </Section>

      {/* P8-55 — the "MCP ACTIVE" section is removed. All three dots reported
          the Python sidecar: core-orch and memory-mcp were its two processes,
          and github was hardcoded inactive, a light that could never turn on.
          External MCP health has its own panel, backed by real data. */}

      {/* ── SKILLS — only rendered when skills are loaded ── */}
      {allSkills.length > 0 && (
        <Section title="SKILLS">
          {allSkills.slice(0, 5).map(s => (
            <Dot key={s.name} label={`/${s.name}`} active />
          ))}
          {allSkills.length > 5 && (
            <div
              className="font-mono"
              style={{ fontSize: '10px', color: 'var(--text-4)', paddingLeft: '10px', marginTop: '1px' }}
            >
              +{allSkills.length - 5} more
            </div>
          )}
        </Section>
      )}

      {/* ── HOOKS — only rendered when hooks are configured ── */}
      {enabledHooks.length > 0 && (
        <Section title="HOOKS">
          {enabledHooks.slice(0, 4).map(h => (
            <Dot key={h.id} label={h.name} active={h.enabled} />
          ))}
        </Section>
      )}

      {/* ── SESSION ──────────────────────────────────────── */}
      <Section title="SESSION">
        <div
          className="font-mono"
          style={{ fontSize: '11px', lineHeight: '1.7', color: 'var(--text-3)' }}
        >
          {totalTokens > 0 ? (
            <div>{fmtN(totalTokens)} tokens</div>
          ) : (
            <div style={{ color: 'var(--text-4)' }}>no session</div>
          )}
          {costToday > 0 && (
            <div style={{ color: 'var(--accent)' }}>
              {costToday < 0.001
                ? `$${costToday.toFixed(6)}`
                : costToday < 1
                  ? `$${costToday.toFixed(4)}`
                  : `$${costToday.toFixed(2)}`} today
            </div>
          )}
        </div>
      </Section>

      {/* ── AUTODEV.md editor card ────────────────────────── */}
      <AutodevMdCard />
    </div>
  )
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div>
      <div
        className="font-mono mb-1"
        style={{
          fontSize:      '9px',
          color:         'var(--text-4)',
          letterSpacing: '0.10em',
          textTransform: 'uppercase',
        }}
      >
        {title}
      </div>
      {children}
    </div>
  )
}

function Dot({ label, active, error }: { label: string; active: boolean; error?: boolean }) {
  const dotColor = error ? 'var(--danger)' : active ? 'var(--accent)' : '#1A1A1A'
  return (
    <div
      className="flex items-center gap-1.5 font-mono"
      style={{
        fontSize: '11px',
        color:    active ? 'var(--text-2)' : 'var(--text-4)',
        padding:  '1px 0',
      }}
    >
      <span
        style={{
          width:        '5px',
          height:       '5px',
          borderRadius: '50%',
          background:   dotColor,
          flexShrink:   0,
          display:      'inline-block',
        }}
      />
      <span className="truncate">{label}</span>
    </div>
  )
}
