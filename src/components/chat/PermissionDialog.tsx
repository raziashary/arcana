// InlinePermissionCard — tool-execution approval prompt rendered inline in
// the chat stream (NOT as a centered modal overlay).
//
// Earlier versions of this component rendered as a full-screen modal that
// obscured the assistant's last message and felt jarring. Inline rendering
// matches the way most agentic IDEs surface tool-use decisions: as a card
// in the conversation flow, immediately after the assistant's response,
// so the user can read the context and the requested tool side-by-side.
//
// Returns null when no permission is pending — the caller can drop this
// component into the message list unconditionally.

import { invoke } from '@tauri-apps/api/core'
import { Shield, Terminal } from 'lucide-react'
import { useAgentStore } from '../../stores/agentStore'

export default function InlinePermissionCard() {
  const pendingPermission = useAgentStore(s => s.pendingPermission)

  if (!pendingPermission) return null

  const respond = async (response: 'allow' | 'deny' | 'allow_always') => {
    try {
      await invoke('respond_to_permission', {
        callId:   pendingPermission.callId,
        response,
      })
    } finally {
      // Clear card and resume streaming
      useAgentStore.setState({ pendingPermission: null, isStreaming: true })
    }
  }

  // Pretty-print the input. Cap each value at ~200 chars so long file
  // contents don't blow up the card height.
  const inputPreview = JSON.stringify(pendingPermission.input, null, 2)

  return (
    <div
      className="rounded-sm font-mono self-start max-w-4xl w-full my-1"
      style={{
        background:  'var(--bg-surface)',
        border:      '1px solid var(--warning)',
        borderLeft:  '3px solid var(--warning)',
        fontSize:    '12px',
      }}
    >
      {/* Header */}
      <div
        className="flex items-center gap-1.5 px-3 py-2"
        style={{ borderBottom: '1px solid var(--border-1)' }}
      >
        <Shield size={12} color="var(--warning)" />
        <span style={{
          color:          'var(--warning)',
          letterSpacing:  '0.08em',
          fontSize:       '10px',
          textTransform:  'uppercase',
          fontWeight:     600,
        }}>
          Permission required
        </span>
        <span style={{ color: 'var(--text-4)' }}>·</span>
        <Terminal size={11} color="var(--info)" />
        <span style={{ color: 'var(--info)' }}>{pendingPermission.toolName}</span>
      </div>

      {/* Input preview */}
      <div
        className="px-3 py-2 overflow-auto whitespace-pre-wrap break-all"
        style={{
          background: 'var(--bg-app)',
          color:      'var(--text-2)',
          fontSize:   '11px',
          maxHeight:  '180px',
          lineHeight: 1.5,
        }}
      >
        {inputPreview}
      </div>

      {/* Description (optional) */}
      {pendingPermission.description && (
        <div
          className="px-3 py-1.5"
          style={{
            color:        'var(--text-3)',
            fontSize:     '11px',
            borderTop:    '1px solid var(--border-1)',
          }}
        >
          {pendingPermission.description}
        </div>
      )}

      {/* Actions */}
      <div
        className="flex gap-2 justify-end px-3 py-2"
        style={{ borderTop: '1px solid var(--border-1)' }}
      >
        <button
          onClick={() => respond('deny')}
          className="font-mono px-3 py-1 rounded-sm cursor-pointer"
          style={{
            background: 'var(--bg-card)',
            border:     '1px solid var(--border-2)',
            color:      'var(--danger)',
            fontSize:   '11px',
          }}
        >
          Deny
        </button>
        {pendingPermission.allowAlways && (
          <button
            onClick={() => respond('allow_always')}
            className="font-mono px-3 py-1 rounded-sm cursor-pointer"
            style={{
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-2)',
              fontSize:   '11px',
            }}
          >
            Allow Always
          </button>
        )}
        <button
          onClick={() => respond('allow')}
          className="font-mono px-3 py-1 rounded-sm cursor-pointer"
          style={{
            background: 'var(--accent-muted)',
            border:     '1px solid var(--accent-border)',
            color:      'var(--accent)',
            fontSize:   '11px',
          }}
        >
          Allow
        </button>
      </div>
    </div>
  )
}
