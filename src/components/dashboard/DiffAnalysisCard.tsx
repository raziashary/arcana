import { useState } from 'react'

interface DiffAnalysisResult {
  risk_score: number
  risk_level: 'low' | 'medium' | 'high' | 'critical'
  classification: string
  summary: string
  review_notes: string[]
  suggested_checks: string[]
  file_count: number
  lines_added: number
  lines_deleted: number
  critical_files: string[]
  analyzed_at: string
}

interface Props {
  diffAnalysisJson: string
}

const RISK_COLORS: Record<string, string> = {
  low:      '#4caf50',
  medium:   '#ff9800',
  high:     '#f44336',
  critical: '#9c27b0',
}

const RISK_BG: Record<string, string> = {
  low:      '#4caf5022',
  medium:   '#ff980022',
  high:     '#f4433622',
  critical: '#9c27b022',
}

export function DiffAnalysisCard({ diffAnalysisJson }: Props) {
  const [expanded, setExpanded] = useState(false)

  let result: DiffAnalysisResult | null = null
  try {
    result = JSON.parse(diffAnalysisJson)
  } catch {
    return null
  }

  if (!result) return null

  const riskColor = RISK_COLORS[result.risk_level] ?? '#888'
  const riskBg    = RISK_BG[result.risk_level]    ?? '#88888822'

  return (
    <div
      style={{
        border:       `1px solid ${riskColor}`,
        borderRadius: 6,
        background:   riskBg,
        padding:      '8px 10px',
        marginTop:    6,
        fontSize:     12,
      }}
    >
      {/* Header row */}
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
        <span
          style={{
            background:   riskColor,
            color:        '#fff',
            borderRadius: 4,
            padding:      '1px 7px',
            fontWeight:   700,
            fontSize:     11,
            textTransform: 'uppercase',
          }}
        >
          {result.risk_level}
        </span>

        <span
          style={{
            background:   'var(--bg-raised, #2a2a2a)',
            border:       '1px solid var(--border-1, #444)',
            borderRadius: 4,
            padding:      '1px 7px',
            fontSize:     11,
            color:        'var(--text-2, #aaa)',
          }}
        >
          {result.classification}
        </span>

        <span style={{ color: 'var(--text-3, #888)', fontSize: 11 }}>
          {result.file_count} file{result.file_count !== 1 ? 's' : ''}
          {' · +'}
          {result.lines_added}
          {' −'}
          {result.lines_deleted}
        </span>

        <span style={{ flex: 1 }} />

        <button
          onClick={() => setExpanded(v => !v)}
          aria-expanded={expanded}
          style={{
            fontSize:   10,
            padding:    '1px 6px',
            border:     '1px solid var(--border-1, #444)',
            borderRadius: 3,
            background: 'transparent',
            color:      'var(--text-3, #888)',
            cursor:     'pointer',
          }}
        >
          {expanded ? 'Hide' : 'Details'}
        </button>
      </div>

      {/* Summary */}
      <div style={{ marginTop: 5, color: 'var(--text-1, #eee)', lineHeight: 1.4 }}>
        {result.summary}
      </div>

      {/* Expanded details */}
      {expanded && (
        <div style={{ marginTop: 8, display: 'flex', flexDirection: 'column', gap: 6 }}>
          {result.review_notes.length > 0 && (
            <div>
              <div style={{ fontWeight: 600, color: 'var(--text-2, #aaa)', marginBottom: 3 }}>
                Review Notes
              </div>
              <ul style={{ margin: 0, paddingLeft: 16 }}>
                {result.review_notes.map((note, i) => (
                  <li key={i} style={{ color: 'var(--text-1, #eee)', marginBottom: 2 }}>
                    {note}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {result.suggested_checks.length > 0 && (
            <div>
              <div style={{ fontWeight: 600, color: 'var(--text-2, #aaa)', marginBottom: 3 }}>
                Suggested Checks
              </div>
              <ul style={{ margin: 0, paddingLeft: 16 }}>
                {result.suggested_checks.map((check, i) => (
                  <li key={i} style={{ color: 'var(--text-1, #eee)', marginBottom: 2 }}>
                    {check}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {result.critical_files.length > 0 && (
            <div>
              <div style={{ fontWeight: 600, color: riskColor, marginBottom: 3 }}>
                Critical Files
              </div>
              <ul style={{ margin: 0, paddingLeft: 16 }}>
                {result.critical_files.map((f, i) => (
                  <li key={i} style={{ fontFamily: 'monospace', color: riskColor, marginBottom: 2, fontSize: 11 }}>
                    {f}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <div style={{ color: 'var(--text-3, #888)', fontSize: 10 }}>
            Risk score: {result.risk_score.toFixed(2)} · Analyzed {new Date(result.analyzed_at).toLocaleString()}
          </div>
        </div>
      )}
    </div>
  )
}
