// MessageBubble — renders a single chat message with tool calls.
// User messages: right-aligned green tinted bubble.
// Assistant messages: left-aligned dark bubble + tool call blocks below.

import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import ToolCallDisplay from './ToolCallDisplay'
import type { ChatMessage } from '../../stores/agentStore'

interface Props {
  message:   ChatMessage
  isLast:    boolean
  streaming: boolean
}

export default function MessageBubble({ message, isLast, streaming }: Props) {
  const isUser      = message.role === 'user'
  const isAssistant = message.role === 'assistant'

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

      {/* Tool call blocks */}
      {isAssistant && message.toolCalls.map(tc => (
        <ToolCallDisplay key={tc.callId} toolCall={tc} />
      ))}

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
