// DirectoryListingDisplay — renders list_directory tool output.
// Shows name, type (📁/📄), size in a sortable table.
// Phase 1 extension — Rich Tool Display UI.

import { useState } from 'react'
import { formatFileSize } from '../../utils/toolOutputParser'
import type { DirectoryEntry } from '../../utils/toolOutputParser'

interface DirectoryListingDisplayProps {
  data: DirectoryEntry[]
}

type SortKey = 'name' | 'type' | 'size'

export function DirectoryListingDisplay({ data }: DirectoryListingDisplayProps) {
  const [sortKey,  setSortKey]  = useState<SortKey>('name')
  const [sortAsc,  setSortAsc]  = useState(true)

  const handleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortAsc(a => !a)
    } else {
      setSortKey(key)
      setSortAsc(true)
    }
  }

  const sorted = [...data].sort((a, b) => {
    let cmp = 0
    if (sortKey === 'name')  cmp = a.name.localeCompare(b.name)
    if (sortKey === 'type')  cmp = (a.is_directory ? 0 : 1) - (b.is_directory ? 0 : 1)
    if (sortKey === 'size')  cmp = (a.size_bytes ?? 0) - (b.size_bytes ?? 0)
    return sortAsc ? cmp : -cmp
  })

  const dirs  = data.filter(e => e.is_directory).length
  const files = data.length - dirs

  return (
    <div
      className="rounded font-mono text-xs"
      style={{ background: 'var(--surface-bg)', border: '1px solid var(--border-primary)' }}
    >
      {/* Summary */}
      <div
        className="px-3 py-2 border-b flex items-center gap-2"
        style={{ borderColor: 'var(--border-primary)', color: 'var(--text-3)' }}
      >
        <span>{dirs} dirs · {files} files</span>
      </div>

      {/* Table */}
      <table style={{ width: '100%', borderCollapse: 'collapse' }}>
        <thead>
          <tr style={{ borderBottom: '1px solid var(--border-primary)' }}>
            {(['name', 'type', 'size'] as SortKey[]).map(col => (
              <th
                key={col}
                onClick={() => handleSort(col)}
                style={{
                  padding: '4px 10px', textAlign: 'left', cursor: 'pointer',
                  color: sortKey === col ? 'var(--accent)' : 'var(--text-3)',
                  userSelect: 'none', fontSize: '10px', textTransform: 'uppercase', letterSpacing: 1,
                }}
              >
                {col} {sortKey === col ? (sortAsc ? '▲' : '▼') : ''}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.map((entry, i) => (
            <tr
              key={i}
              style={{
                borderBottom: '1px solid rgba(255,255,255,0.03)',
              }}
            >
              <td style={{ padding: '4px 10px', color: 'var(--text-1)' }}>
                {entry.is_directory ? '📁' : '📄'} {entry.name}
              </td>
              <td style={{ padding: '4px 10px', color: 'var(--text-3)' }}>
                {entry.is_directory ? 'dir' : 'file'}
              </td>
              <td style={{ padding: '4px 10px', color: 'var(--text-3)' }}>
                {entry.is_directory ? '—' : formatFileSize(entry.size_bytes ?? 0)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
