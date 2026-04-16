// MessageBubble — renders a single chat message.
// User messages: right-aligned green tinted bubble.
// Assistant messages: left-aligned dark bubble.
// Tool messages: left-aligned info-tinted bubble (separate from assistant).

import { useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Terminal } from 'lucide-react'
import ToolCallDisplay from './ToolCallDisplay'
import { GrepResultsDisplay } from './GrepResultsDisplay'
import { GlobResultsDisplay } from './GlobResultsDisplay'
import { FileContentDisplay } from './FileContentDisplay'
import { BashOutputDisplay } from './BashOutputDisplay'
import { DirectoryListingDisplay } from './DirectoryListingDisplay'
import { parseToolOutput } from '../../utils/toolOutputParser'
import type { ChatMessage } from '../../stores/agentStore'

interface Props {
  message:   ChatMessage
  isLast:    boolean
  streaming: boolean
}

export default function MessageBubble({ message, isLast, streaming }: Props) {
  const isUser      = message.role === 'user'
  const isAssistant = message.role === 'assistant'
  const isTool      = message.role === 'tool'

  // Tool messages render as a tool bubble
  if (isTool) {
    return (
      <div className="flex flex-col items-start gap-0.5">
        <ToolMessageBubble message={message} />
      </div>
    )
  }

  return (
    <div className={`flex flex-col ${isUser ? 'items-end' : 'items-start'} gap-0.5`}>
      {/* Message text bubble */}
      {(message.content || (isAssistant && streaming && isLast)) && (
        <div
          className="max-w-3xl rounded-sm px-3 py-2 text-sm leading-relaxed prose prose-invert"
          style={isUser ? {
            background: 'var(--accent-muted)',
            border:     '1px solid var(--accent-border)',
            color:      '#aaa',
          } : {
            background: 'var(--bg-card)',
            border:     '1px solid var(--border-1)',
            color:      'var(--text-1)',
          }}
        >
          {message.content ? (
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              components={{
                code: ({ node, inline, children, className, ...props }: any) => (
                  inline ? (
                    <code className="bg-opacity-20 px-1 py-0.5 rounded" style={{ background: 'rgba(255,255,255,0.1)' }} {...props}>{children}</code>
                  ) : (
                    <pre className="bg-opacity-20 p-2 rounded overflow-x-auto" style={{ background: 'rgba(0,0,0,0.3)' }}>
                      <code {...props}>{children}</code>
                    </pre>
                  )
                ),
                a: ({ node, ...props }) => (
                  <a className="underline" style={{ color: 'var(--accent)' }} {...props} />
                ),
                h1: ({ node, ...props }) => <h1 className="text-lg font-bold mt-2 mb-1" {...props} />,
                h2: ({ node, ...props }) => <h2 className="text-base font-bold mt-2 mb-1" {...props} />,
                h3: ({ node, ...props }) => <h3 className="text-sm font-bold mt-1 mb-0.5" {...props} />,
                ul: ({ node, ...props }) => <ul className="list-disc list-inside my-1" {...props} />,
                ol: ({ node, ...props }) => <ol className="list-decimal list-inside my-1" {...props} />,
                li: ({ node, ...props }) => <li className="ml-2" {...props} />,
                blockquote: ({ node, ...props }) => (
                  <blockquote className="border-l-2 pl-2 italic my-1" style={{ borderColor: 'var(--accent)' }} {...props} />
                ),
              }}
            >
              {message.content}
            </ReactMarkdown>
          ) : null}
          {isAssistant && streaming && isLast && !message.content && (
            <span className="animate-pulse" style={{ color: 'var(--accent)' }}>▋</span>
          )}
          {isAssistant && streaming && isLast && message.content && (
            <span className="animate-pulse" style={{ color: 'var(--accent)' }}>▋</span>
          )}
        </div>
      )}

      {/* Tool call blocks (for streaming UI only) */}
      {isAssistant && message.toolCalls.map(tc => (
        <ToolCallDisplay key={tc.callId} toolCall={tc} />
      ))}

      {/* Tool message bubbles (for persisted tool messages) */}
      {isAssistant && message.toolCalls.length > 0 && message.content && (
        <div style={{ marginTop: '8px' }} />)}

      {/* Cost/token badge for completed assistant messages */}
      {isAssistant && !streaming && message.tokensOut > 0 && (
        <div
          className="font-mono"
          style={{ fontSize: '10px', color: 'var(--text-4)' }}
        >
          ↑{message.tokensIn.toLocaleString()} ↓{message.tokensOut.toLocaleString()} tokens
          {message.costUsd > 0 && ` · $${message.costUsd.toFixed(4)}`}
        </div>
      )}
    </div>
  )
}

// ToolMessageBubble — renders a tool execution as a separate chat bubble
// Shows tool name, input args (collapsed), and output with proper formatting
function ToolMessageBubble({ message }: { message: ChatMessage }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div
      className="rounded-sm font-mono text-xs self-start max-w-4xl w-full"
      style={{
        background:  'var(--bg-surface)',
        border:      '1px solid var(--border-1)',
        borderLeft:  `3px solid ${message.isError ? 'var(--danger)' : 'var(--info)'}`,
      }}
    >
      {/* Header row - always visible */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-1.5 w-full border-none cursor-pointer px-2 py-1.5"
        style={{ background: 'transparent' }}
      >
        <Terminal size={11} color={message.isError ? 'var(--danger)' : 'var(--info)'} />
        <span style={{ color: message.isError ? 'var(--danger)' : 'var(--info)' }}>
          {message.toolName}
        </span>
        {message.isError && (
          <span style={{ color: 'var(--danger)', fontSize: '10px', marginLeft: '4px' }}>
            ✗ error
          </span>
        )}
        {!message.isError && (
          <span style={{ color: 'var(--accent)', fontSize: '10px', marginLeft: '4px' }}>
            ✓ done
          </span>
        )}
        <span style={{ color: 'var(--text-3)', marginLeft: 'auto', fontSize: '10px' }}>
          {expanded ? '▼' : '▶'}
        </span>
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="px-2 pb-2 space-y-1">
          {/* Input args */}
          {message.toolInput && Object.keys(message.toolInput).length > 0 && (
            <div style={{ color: 'var(--text-2)', paddingLeft: '12px', fontSize: '10px' }}>
              {Object.entries(message.toolInput).map(([k, v]) => (
                <div key={k} className="truncate">
                  <span style={{ color: 'var(--text-3)' }}>{k}: </span>
                  <span>{typeof v === 'string' ? v.slice(0, 100) : JSON.stringify(v).slice(0, 100)}</span>
                </div>
              ))}
            </div>
          )}

          {/* Output — use rich display for known tool types */}
          <div className="mt-2 pl-3">
            {message.isError ? (
              <div
                className="whitespace-pre-wrap break-all p-2 rounded"
                style={{
                  color: 'var(--danger)',
                  borderLeft: `2px solid var(--danger)`,
                  backgroundColor: '#FF006610',
                  fontSize: '11px',
                }}
              >
                {message.content}
              </div>
            ) : renderPersistedToolOutput(message.toolName ?? '', message.content)}
          </div>
        </div>
      )}
    </div>
  )
}

function renderPersistedToolOutput(toolName: string, content: string) {
  const parsed = parseToolOutput(toolName, content)

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
            borderLeft: '2px solid var(--accent)',
            backgroundColor: 'var(--surface-bg)',
            fontSize: '11px',
            maxHeight: '300px',
            overflowY: 'auto',
          }}
        >
          {content.length > 2000 ? content.slice(0, 2000) + '…' : content}
        </div>
      )
  }
}
