// MessageBubble — renders a single chat message with tool calls.
// User messages: right-aligned green tinted bubble.
// Assistant messages: left-aligned dark bubble + tool call blocks below.

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
          className="max-w-3xl rounded-sm px-3 py-2 text-md leading-relaxed"
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
          {message.content}
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
