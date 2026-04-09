// Chat tab — PA-mode streaming chat with tool execution.
// Wires agent store events, manages session, renders message list.

import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Send, Square } from 'lucide-react'
import MessageBubble    from '../components/chat/MessageBubble'
import PermissionDialog from '../components/chat/PermissionDialog'
import { SessionBar }   from '../components/chat/SessionBar'
import { useAgentStore }    from '../stores/agentStore'
import { useSessionStore }  from '../stores/sessionStore'
import { useSettingsStore } from '../stores/settingsStore'
import { useCostStore }     from '../stores/costStore'

export default function ChatTab() {
  const messages      = useAgentStore(s => s.messages)
  const isStreaming   = useAgentStore(s => s.isStreaming)
  const sessionStore  = useSessionStore()
  const settingsStore = useSettingsStore()
  const workingDir    = useAgentStore(s => s.workingDir)
  const setWorkingDir = useAgentStore(s => s.setWorkingDir)
  const setSessionId  = useAgentStore(s => s.setSessionId)

  const [input,       setInput]       = useState('')
  const [localDir,    setLocalDir]    = useState(workingDir)
  const [isStarting,  setIsStarting]  = useState(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef       = useRef<HTMLTextAreaElement>(null)

  // Auto-scroll to bottom on new messages
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  // Ref to hold active unlisteners so they can be cleaned up on unmount
  const unlistenersRef = useRef<(() => void)[]>([])

  // Cleanup all listeners on unmount
  useEffect(() => {
    return () => { unlistenersRef.current.forEach(fn => fn()) }
  }, [])

  const ensureSession = async (): Promise<string> => {
    if (sessionStore.activeSessionId) return sessionStore.activeSessionId

    const provider = settingsStore.activeProvider ?? 'anthropic'
    const model    = settingsStore.activeModel    ?? 'claude-sonnet-4-5'
    const sessionId  = await sessionStore.createSession(localDir, provider, model)
    setSessionId(sessionId)
    return sessionId
  }

  const handleSend = async () => {
    if (!input.trim() || isStreaming) return
    const text = input.trim()
    setInput('')

    // Ensure provider + key
    if (!settingsStore.activeProvider || !settingsStore.activeModel) {
      alert('Please configure a provider in the Settings tab first.')
      return
    }

    try {
      setIsStarting(true)
      const sessionId = await ensureSession()

      // Tear down any previous listeners, then register fresh ones BEFORE invoking backend
      unlistenersRef.current.forEach(fn => fn())
      unlistenersRef.current = await useAgentStore.getState().setupListeners(sessionId)

      // Add cost store listener for agent:done event
      const costListener = await listen<{ session_id: string; tokens_in: number; tokens_out: number; cost_usd: number }>('agent:done', ({ payload }) => {
        if (payload.session_id === sessionId) {
          useCostStore.getState().addCost(payload.cost_usd, payload.tokens_in, payload.tokens_out)
        }
      })
      unlistenersRef.current.push(costListener)

      useAgentStore.getState().addUserMessage(text)
      useAgentStore.getState().startStreaming(sessionId)

      await invoke('send_message', {
        sessionId,
        userMessage: text,
      })
    } catch (e) {
      console.error('send_message failed:', e)
      useAgentStore.getState().stopStreaming()
    } finally {
      setIsStarting(false)
    }
  }

  const handleCancel = async () => {
    const sid = sessionStore.activeSession?.id
    if (!sid) return
    await invoke('cancel_agent', { sessionId: sid })
    useAgentStore.getState().stopStreaming()
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div className="flex flex-row flex-1 overflow-hidden">
      {/* Session sidebar */}
      <SessionBar />

      {/* Main chat area or placeholder */}
      {sessionStore.sessions.length === 0 ? (
        <div className="flex flex-col flex-1 items-center justify-center" style={{ color: 'var(--text-2)' }}>
          <div className="text-center">
            <h1 className="font-mono text-4xl mb-4" style={{ color: '#1a73e8', letterSpacing: '0.05em' }}>
              Oh My AIRS
            </h1>
            <p className="text-sm" style={{ color: 'var(--text-3)' }}>
              Create a new session to get started
            </p>
          </div>
        </div>
      ) : (
      <div className="flex flex-col flex-1 overflow-hidden relative">

        {/* Session header bar */}
        <div
          className="flex items-center gap-3 px-3.5 py-2 font-mono text-xs shrink-0"
          style={{
            background:   'var(--bg-surface)',
            borderBottom: '1px solid var(--border-1)',
          }}
        >
        <span style={{ color: 'var(--text-3)' }}>cwd:</span>
        <input
          className="flex-1 bg-transparent border-none outline-none font-mono text-xs"
          style={{ color: 'var(--text-2)' }}
          value={localDir}
          onChange={e => { setLocalDir(e.target.value); setWorkingDir(e.target.value) }}
          placeholder="Working directory path…"
        />
        <span style={{ color: 'var(--text-3)' }}>
          {sessionStore.activeSession
            ? `session: ${sessionStore.activeSession.id.slice(0, 8)}…`
            : 'no session'}
        </span>
      </div>

      {/* Messages list */}
      <div className="flex-1 overflow-y-auto px-3.5 py-3 flex flex-col gap-2.5">
        {messages.length === 0 && (
          <div
            className="flex flex-col items-center justify-center h-full gap-3"
            style={{ color: 'var(--text-3)' }}
          >
            <div className="font-mono text-xs" style={{ letterSpacing: '0.08em' }}>
              AUTODEV — PA MODE
            </div>
            <div className="text-sm" style={{ color: 'var(--text-4)' }}>
              Send a message to start a session
            </div>
          </div>
        )}

        {messages.map((msg, i: number) => (
          <MessageBubble
            key={msg.id}
            message={msg}
            isLast={i === messages.length - 1}
            streaming={isStreaming}
          />
        ))}

        <div ref={messagesEndRef} />
      </div>

      {/* Permission dialog overlay */}
      <PermissionDialog />

      {/* Input area */}
      <div
        className="shrink-0"
        style={{
          background:   'var(--bg-surface)',
          borderTop:    '1px solid var(--border-1)',
          padding:      '10px 14px',
        }}
      >
        <div className="flex items-end gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={e => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Send a message… (Enter to send, Shift+Enter for newline)"
            rows={1}
            style={{
              flex:        1,
              background:  'var(--bg-card)',
              border:      '1px solid #1d1d1d',
              borderRadius: '2px',
              padding:     '8px 12px',
              color:       'var(--text-1)',
              fontSize:    '13px',
              fontFamily:  'var(--font-sans)',
              resize:      'none',
              outline:     'none',
              minHeight:   '36px',
              maxHeight:   '120px',
            }}
          />

          {isStreaming ? (
            <button
              onClick={handleCancel}
              aria-label="Cancel"
              className="shrink-0 font-mono text-xs px-3 py-2 rounded-sm cursor-pointer"
              style={{
                background: '#FF444420',
                border:     '1px solid #FF444440',
                color:      'var(--danger)',
              }}
            >
              <Square size={14} />
            </button>
          ) : (
            <button
              onClick={handleSend}
              aria-label="Send"
              disabled={!input.trim() || isStarting}
              className="shrink-0 font-mono text-xs px-3 py-2 rounded-sm cursor-pointer disabled:opacity-40"
              style={{
                background: 'var(--accent-muted)',
                border:     '1px solid var(--accent-border)',
                color:      'var(--accent)',
              }}
            >
              <Send size={14} />
            </button>
          )}
        </div>

        {/* Input meta row */}
        <div
          className="flex justify-between mt-1.5 font-mono"
          style={{ fontSize: '10px', color: 'var(--text-4)' }}
        >
          <span>⏎ send · ⇧⏎ newline</span>
          <span>
            {settingsStore.activeModel ?? 'no model'} · ManualApproval
          </span>
        </div>
      </div>
      </div>
      )}
    </div>
  )
}
