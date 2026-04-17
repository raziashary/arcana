// SkillSlashPicker — autocomplete dropdown for /skill-name in chat input.
// Appears inline (above textarea) when the user types / followed by a word.
// Keyboard: ArrowUp/Down navigate · Enter/Tab select · Escape dismiss.
// Phase 3.1

import { useEffect, useRef } from 'react'
import type { SkillSummary } from '../../stores/skillStore'

interface Props {
  results:       SkillSummary[]
  selectedIndex: number
  query:         string
  onSelect:      (summary: SkillSummary) => void
}

export default function SkillSlashPicker({ results, selectedIndex, query, onSelect }: Props) {
  const listRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined
    el?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  if (results.length === 0) return null

  const highlight = (text: string) => {
    if (!query) return <span>{text}</span>
    const idx = text.toLowerCase().indexOf(query.toLowerCase())
    if (idx === -1) return <span>{text}</span>
    return (
      <span>
        {text.slice(0, idx)}
        <span style={{ color: 'var(--accent)', fontWeight: 700 }}>
          {text.slice(idx, idx + query.length)}
        </span>
        {text.slice(idx + query.length)}
      </span>
    )
  }

  return (
    <div
      style={{
        background:   'var(--bg-card)',
        border:       '1px solid var(--border-2)',
        borderRadius: '2px',
        overflow:     'hidden',
        boxShadow:    '0 -4px 16px rgba(0,0,0,0.5)',
        marginBottom: 6,
      }}
    >
      {/* Header */}
      <div
        style={{
          display:        'flex',
          justifyContent: 'space-between',
          alignItems:     'center',
          padding:        '4px 10px',
          borderBottom:   '1px solid var(--border-1)',
          background:     'var(--bg-surface)',
        }}
      >
        <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-3)', letterSpacing: '0.1em', textTransform: 'uppercase' }}>
          skills · /{query || '…'} · {results.length} match{results.length !== 1 ? 'es' : ''}
        </span>
        <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-4)' }}>
          ↑↓ · ↵ select · Esc
        </span>
      </div>

      {/* Results list */}
      <div ref={listRef} style={{ maxHeight: 220, overflowY: 'auto' }}>
        {results.map((skill, i) => (
          <div
            key={skill.name}
            onMouseDown={e => { e.preventDefault(); onSelect(skill) }}
            style={{
              display:      'flex',
              alignItems:   'center',
              gap:          10,
              padding:      '6px 10px',
              cursor:       'pointer',
              background:   i === selectedIndex ? 'var(--bg-selected)' : 'transparent',
              borderBottom: i < results.length - 1 ? '1px solid var(--border-1)' : undefined,
            }}
          >
            <span style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--accent)', flexShrink: 0 }}>
              /{highlight(skill.name)}
            </span>
            <span style={{
              fontFamily:   'monospace',
              fontSize:     10,
              color:        'var(--text-3)',
              overflow:     'hidden',
              textOverflow: 'ellipsis',
              whiteSpace:   'nowrap',
              flex:         1,
            }}>
              {skill.description}
            </span>
            {skill.param_count > 0 && (
              <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-3)', flexShrink: 0 }}>
                {skill.param_count}p
              </span>
            )}
            {skill.is_builtin && (
              <span style={{ fontFamily: 'monospace', fontSize: 8, color: 'var(--text-4)', flexShrink: 0, textTransform: 'uppercase' }}>
                builtin
              </span>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}
