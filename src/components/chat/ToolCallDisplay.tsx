// ToolCallDisplay — collapsible block showing tool name, args, result.
// Design from autodevmockup.html .tool-block

import { useState } from 'react'
import { ChevronDown, ChevronRight, Terminal, Loader } from 'lucide-react'
import type { ToolCallDisplay as ToolCallData } from '../../stores/agentStore'
import { parseToolOutput } from '../../utils/toolOutputParser'
import { GrepResultsDisplay } from './GrepResultsDisplay'
import { GlobResultsDisplay } from './GlobResultsDisplay'
import { FileContentDisplay } from './FileContentDisplay'
import { BashOutputDisplay } from './BashOutputDisplay'
import { DirectoryListingDisplay } from './DirectoryListingDisplay'

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
            <div className="mt-2 pl-3">
              {renderToolOutput(toolCall)}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function renderToolOutput(toolCall: ToolCallData) {
  if (toolCall.isError) {
    return (
      <div
        className="whitespace-pre-wrap break-all p-2 rounded"
        style={{
          color: 'var(--danger)',
          borderLeft: `2px solid var(--danger)`,
          backgroundColor: '#FF006610',
          fontSize: '11px',
        }}
      >
        {toolCall.output}
      </div>
    )
  }

  const parsed = parseToolOutput(toolCall.toolName, toolCall.output || '')

  switch (parsed.type) {
    case 'grep':
      return <GrepResultsDisplay data={parsed.data} />
    
    case 'glob':
      return <GlobResultsDisplay data={parsed.data} />
    
    case 'file_content':
      return <FileContentDisplay data={parsed.data} />
    
    case 'bash':
      return <BashOutputDisplay data={parsed.data} />
    
    case 'directory_listing':
      return <DirectoryListingDisplay data={parsed.data.entries} />
    
    case 'text':
    default:
      return (
        <div
          className="whitespace-pre-wrap break-all p-2 rounded font-mono"
          style={{
            color: 'var(--accent)',
            borderLeft: `2px solid var(--accent)`,
            backgroundColor: 'var(--surface-bg)',
            fontSize: '11px',
            maxHeight: '300px',
            overflowY: 'auto',
          }}
        >
          {truncate(parsed.data, 2000)}
        </div>
      )
  }
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + '…' : s
}
