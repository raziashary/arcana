// PermissionDialog — modal overlay for tool execution approval.
// Appears when agent:permission-request event fires in ManualApproval mode.
// User can Allow, Deny, or Allow Always.

import { invoke } from '@tauri-apps/api/core'
import { Shield } from 'lucide-react'
import { useAgentStore } from '../../stores/agentStore'

export default function PermissionDialog() {
  const pendingPermission = useAgentStore(s => s.pendingPermission)

  if (!pendingPermission) return null

  const respond = async (response: 'allow' | 'deny' | 'allow_always') => {
    try {
      await invoke('respond_to_permission', {
        callId:   pendingPermission.callId,
        response,
      })
    } finally {
      // Clear dialog and resume streaming
      useAgentStore.setState({ pendingPermission: null, isStreaming: true })
    }
  }

  return (
    <div
      className="absolute inset-0 flex items-end justify-center pb-16 z-50"
      style={{ background: 'rgba(0,0,0,0.6)' }}
    >
      <div
        className="w-full max-w-lg rounded-sm"
        style={{
          background: 'var(--bg-surface)',
          border:     '1px solid var(--border-1)',
          padding:    '16px',
        }}
      >
        {/* Header */}
        <div className="flex items-center gap-2 mb-3">
          <Shield size={16} color="var(--warning)" />
          <span
            className="font-mono text-xs"
            style={{ color: 'var(--warning)', letterSpacing: '0.08em' }}
          >
            PERMISSION REQUIRED
          </span>
        </div>

        {/* Tool name */}
        <div className="mb-2">
          <span className="font-mono text-xs" style={{ color: 'var(--info)' }}>
            {pendingPermission.toolName}
          </span>
        </div>

        {/* Input preview */}
        <div
          className="font-mono text-xs mb-4 rounded-sm p-2 overflow-auto max-h-32"
          style={{
            background: 'var(--bg-app)',
            border:     '1px solid var(--border-1)',
            color:      'var(--text-2)',
          }}
        >
          {JSON.stringify(pendingPermission.input, null, 2)}
        </div>

        {/* Buttons */}
        <div className="flex gap-2 justify-end">
          <button
            onClick={() => respond('deny')}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer"
            style={{
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      'var(--danger)',
            }}
          >
            Deny
          </button>
          <button
            onClick={() => respond('allow_always')}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer"
            style={{
              background: 'var(--bg-card)',
              border:     '1px solid var(--border-2)',
              color:      'var(--text-2)',
            }}
          >
            Allow Always
          </button>
          <button
            onClick={() => respond('allow')}
            className="font-mono text-xs px-3 py-1.5 rounded-sm cursor-pointer"
            style={{
              background: 'var(--accent-muted)',
              border:     '1px solid var(--accent-border)',
              color:      'var(--accent)',
            }}
          >
            Allow
          </button>
        </div>
      </div>
    </div>
  )
}
