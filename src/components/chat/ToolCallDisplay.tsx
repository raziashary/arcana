// ToolCallDisplay — collapsible block showing tool name, args, result.
// Design from autodevmockup.html .tool-block

import { useState } from 'react'
import { ChevronDown, ChevronRight, Terminal, Loader } from 'lucide-react'
import type { ToolCallDisplay as ToolCallData } from '../../stores/agentStore'

interface Props {
  toolCall: ToolCallData
}

export default function ToolCallDisplay({ toolCall }: Props) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="rounded-sm font-mono text-xs my-1 self-start max-w-4xl"
      style={{
        background:  'var(--bg-surface)',
        border:      '1px solid #1d1d1d',
        padding:     '8px 10px',
      }}
    >
      {/* Header row */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full border-none cursor-pointer"
        style={{ background: 'transparent' }}
      >
        {expanded
          ? <ChevronDown size={12} color="var(--text-3)" />
          : <ChevronRight size={12} color="var(--text-3)" />
        }
        <Terminal size={11} color="var(--info)" />
        <span style={{ color: 'var(--info)' }}>{toolCall.toolName}</span>
        {toolCall.pending && (
          <Loader size={11} className="animate-spin ml-1" style={{ color: 'var(--text-3)' }} />
        )}
        {!toolCall.pending && toolCall.isError && (
          <span style={{ color: 'var(--danger)', fontSize: '10px', marginLeft: '4px' }}>
            ✗ error
          </span>
        )}
        {!toolCall.pending && !toolCall.isError && toolCall.output !== null && (
          <span style={{ color: 'var(--accent)', fontSize: '10px', marginLeft: '4px' }}>
            ✓ done
          </span>
        )}
      </button>

      {expanded && (
        <div className="mt-1.5 space-y-1">
          {/* Input args */}
          <div style={{ color: 'var(--text-2)', paddingLeft: '12px' }}>
            {Object.entries(toolCall.input).map(([k, v]) => (
              <div key={k} className="truncate">
                <span style={{ color: 'var(--text-3)' }}>{k}: </span>
                <span>{typeof v === 'string' ? truncate(v, 100) : JSON.stringify(v)}</span>
              </div>
            ))}
          </div>

          {/* Output */}
          {toolCall.output !== null && (
            <div
              className="mt-1.5 pl-3 whitespace-pre-wrap break-all"
              style={{
                color:      toolCall.isError ? 'var(--danger)' : 'var(--accent)',
                borderLeft: `2px solid ${toolCall.isError ? 'var(--danger)' : 'var(--accent)'}`,
                paddingLeft: '8px',
                maxHeight:   '200px',
                overflowY:   'auto',
                fontSize:    '11px',
              }}
            >
              {truncate(toolCall.output, 2000)}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + '…' : s
}
