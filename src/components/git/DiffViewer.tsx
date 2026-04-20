// Diff viewer — renders a unified-diff patch string.
//
// We could use Monaco's side-by-side DiffEditor, but we already have a
// unified diff string from the backend and it is cheaper + simpler to
// render directly with our own coloured line renderer.

import { useMemo } from 'react'

interface Props {
  patch: string
  filesChanged: number
  insertions: number
  deletions: number
}

export default function DiffViewer({ patch, filesChanged, insertions, deletions }: Props) {
  const lines = useMemo(() => patch.split('\n'), [patch])

  if (!patch.trim()) {
    return (
      <div
        className="flex-1 flex items-center justify-center p-6 text-xs font-mono"
        style={{ color: 'var(--text-3)' }}
      >
        No diff
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ background: 'var(--bg-card)' }}>
      <div
        className="px-3 py-1.5 text-[10px] font-mono flex gap-3 border-b"
        style={{ borderColor: 'var(--border-1)', color: 'var(--text-2)' }}
      >
        <span>{filesChanged} file{filesChanged === 1 ? '' : 's'}</span>
        <span style={{ color: 'var(--success)' }}>+{insertions}</span>
        <span style={{ color: 'var(--danger)' }}>-{deletions}</span>
      </div>
      <div className="flex-1 overflow-auto font-mono text-[11px]" style={{ lineHeight: '1.4' }}>
        {lines.map((ln, i) => (
          <DiffLine key={i} line={ln} />
        ))}
      </div>
    </div>
  )
}

function DiffLine({ line }: { line: string }) {
  let color: string | undefined
  let background: string | undefined

  if (line.startsWith('+++') || line.startsWith('---')) {
    color = 'var(--text-1)'
    background = 'var(--bg-2)'
  } else if (line.startsWith('@@')) {
    color = 'var(--info)'
    background = 'var(--bg-2)'
  } else if (line.startsWith('+')) {
    color = 'var(--success)'
    background = 'rgba(173, 219, 103, 0.08)'
  } else if (line.startsWith('-')) {
    color = 'var(--danger)'
    background = 'rgba(255, 99, 99, 0.08)'
  } else {
    color = 'var(--text-2)'
  }

  return (
    <div
      className="px-3 whitespace-pre"
      style={{ color, background }}
    >
      {line || '\u00a0'}
    </div>
  )
}
