// RunningToolsPill — small status indicator that shows up at the bottom of
// the chat message stream while the agent is mid-tick.
//
// Surfaces what was previously invisible: when the LLM has emitted tool_use
// blocks and the tools are queued / executing / waiting on permission, the
// user used to see only the assistant's last text and an empty window.
// This pill names the active tools so the wait feels intentional rather
// than broken.
//
// Hidden when the agent is idle. Always self-start aligned to match the
// assistant message side of the chat.

import { Loader2 } from 'lucide-react'
import { useAgentStore } from '../../stores/agentStore'

export default function RunningToolsPill() {
  const isStreaming        = useAgentStore(s => s.isStreaming)
  const pendingPermission  = useAgentStore(s => s.pendingPermission)
  const messages           = useAgentStore(s => s.messages)

  // Pending tool calls live on the most recent assistant message. We
  // surface the names of any still-pending ones so the user can see
  // which tools the agent is preparing to run.
  const pendingTools: string[] = (() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const m = messages[i]
      if (m.role === 'assistant') {
        return m.toolCalls.filter(t => t.pending).map(t => t.toolName)
      }
      if (m.role === 'user') break
    }
    return []
  })()

  // Decide what label to show. Priority: explicit permission wait → pending
  // tool names → generic "thinking".
  let label: string | null = null
  if (pendingPermission) {
    label = `awaiting your approval for ${pendingPermission.toolName}`
  } else if (pendingTools.length > 0) {
    label = pendingTools.length === 1
      ? `running ${pendingTools[0]}`
      : `running ${pendingTools.length} tools: ${pendingTools.slice(0, 3).join(', ')}${pendingTools.length > 3 ? '…' : ''}`
  } else if (isStreaming) {
    label = 'thinking'
  }

  if (!label) return null

  return (
    <div
      className="flex items-center gap-2 self-start rounded-sm font-mono my-1 px-2.5 py-1 anim-fade-in"
      style={{
        background: 'var(--bg-surface)',
        border:     '1px solid var(--border-1)',
        color:      'var(--text-3)',
        fontSize:   '11px',
      }}
    >
      {/* Spinner uses the existing global `anim-spin` keyframe via the
          Loader2 icon's animate-spin class wrapper. The dot below blinks
          to draw the eye to the status without being noisy. */}
      <Loader2 size={11} className="animate-spin" style={{ color: 'var(--accent)' }} />
      <span style={{ color: 'var(--text-2)' }}>{label}…</span>
    </div>
  )
}
