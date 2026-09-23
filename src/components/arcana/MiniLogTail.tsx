// MiniLogTail — scrolling log of recent tool calls for one agent card
// Auto-scrolls to the newest line. Handles running (spinner), success (✓), error (✗).

import { useEffect, useRef } from 'react'
import type { LogLine } from '../../types/live'

interface Props {
  lines:    LogLine[]
  maxLines: number
}

function formatOffset(ms: number): string {
  const totalSec = Math.floor(ms / 1000)
  const m = Math.floor(totalSec / 60)
  const s = totalSec % 60
  return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
}

export function MiniLogTail({ lines, maxLines }: Props) {
  const containerRef = useRef<HTMLDivElement>(null)
  const visible      = lines.slice(-maxLines)
  const hasMore      = lines.length > maxLines

  // Auto-scroll to bottom on new lines
  useEffect(() => {
    if (containerRef.current) {
      containerRef.current.scrollTop = containerRef.current.scrollHeight
    }
  }, [lines.length])

  return (
    <div className={`mini-log-tail${hasMore ? ' mini-log-tail--overflow' : ''}`} ref={containerRef}>
      {visible.length === 0 ? (
        <div className="log-line log-line--empty">no activity yet</div>
      ) : (
        visible.map(line => (
          <div
            key={line.id}
            className={`log-line log-line--${line.status}`}
            title={line.full_argument}
          >
            {line.status === 'running' ? (
              <span className="log-spinner" aria-hidden />
            ) : (
              <span className="log-time">{formatOffset(line.timestamp_offset_ms)}</span>
            )}
            <span className="log-tool">{line.tool_name}</span>
            <span className="log-arg">{line.argument}</span>
            {line.status === 'success' && <span className="log-ok">✓</span>}
            {line.status === 'error'   && <span className="log-err">✗</span>}
          </div>
        ))
      )}
    </div>
  )
}
