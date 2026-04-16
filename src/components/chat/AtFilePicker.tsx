// AtFilePicker — VS Code-style file + folder autocomplete for @-mention in chat input.
// Appears inline (above textarea) when the user types @ followed by a query.
// Shows files with </>  icon and folders with □ icon + "(N files)" count.
// Keyboard: ArrowUp/Down navigate · Enter/Tab select · Escape dismiss.

import { useEffect, useRef } from 'react'

export interface FileEntry {
  name:           string
  absolute_path:  string
  relative_path:  string
  is_dir:         boolean
  children_count: number
}

interface Props {
  results:       FileEntry[]
  selectedIndex: number
  query:         string
  onSelect:      (entry: FileEntry) => void
}

export default function AtFilePicker({ results, selectedIndex, query, onSelect }: Props) {
  const listRef = useRef<HTMLDivElement>(null)

  // Scroll highlighted row into view on keyboard navigation
  useEffect(() => {
    const el = listRef.current?.children[selectedIndex] as HTMLElement | undefined
    el?.scrollIntoView({ block: 'nearest' })
  }, [selectedIndex])

  if (results.length === 0) return null

  // Highlight query match in a string (checks both name and path parts)
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

  const fileCount  = results.filter(r => !r.is_dir).length
  const folderCount = results.filter(r => r.is_dir).length

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
      {/* Header bar */}
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
          workspace · @{query || '…'} · {folderCount > 0 && `${folderCount} folders`}{folderCount > 0 && fileCount > 0 && ' + '}{fileCount > 0 && `${fileCount} files`}
        </span>
        <span style={{ fontFamily: 'monospace', fontSize: 9, color: 'var(--text-4)' }}>
          ↑↓ · ↵ select · Esc
        </span>
      </div>

      {/* Results list */}
      <div ref={listRef} style={{ maxHeight: 260, overflowY: 'auto' }}>
        {results.map((entry, i) => (
          <div
            key={entry.absolute_path}
            onMouseDown={e => { e.preventDefault(); onSelect(entry) }}
            style={{
              display:      'flex',
              alignItems:   'center',
              gap:          10,
              padding:      '5px 10px',
              cursor:       'pointer',
              background:   i === selectedIndex ? 'var(--bg-selected)' : 'transparent',
              borderBottom: i < results.length - 1 ? '1px solid var(--border-1)' : undefined,
            }}
          >
            {/* Type icon: folder □ or file </> */}
            <span style={{
              fontFamily: 'monospace',
              fontSize:   entry.is_dir ? 11 : 9,
              color:      entry.is_dir ? 'var(--gold)' : 'var(--accent)',
              flexShrink: 0,
              opacity:    0.85,
              width:      16,
              textAlign:  'center',
            }}>
              {entry.is_dir ? '□' : '</>'}
            </span>

            {/* Name (highlighted) + for dirs: file count badge */}
            <span style={{ fontFamily: 'monospace', fontSize: 11, color: 'var(--text-1)', flexShrink: 0 }}>
              {highlight(entry.name)}
              {entry.is_dir && (
                <span style={{ marginLeft: 6, fontSize: 9, color: 'var(--text-3)', fontWeight: 400 }}>
                  ({entry.children_count} files)
                </span>
              )}
            </span>

            {/* Relative path — truncated from right, dimmed */}
            <span
              style={{
                fontFamily:   'monospace',
                fontSize:     10,
                color:        'var(--text-3)',
                overflow:     'hidden',
                textOverflow: 'ellipsis',
                whiteSpace:   'nowrap',
                flex:         1,
              }}
            >
              {highlight(entry.relative_path.replace(/\\/g, '/'))}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}
