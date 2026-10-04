// Chat tab — PA-mode streaming chat with tool execution.
// Wires agent store events, manages session, renders message list.
// Phase 2.3: T4c — Context window meter in input meta row.

import { useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { Send, Square, Zap } from 'lucide-react'
import MessageBubble        from '../components/chat/MessageBubble'
import InlinePermissionCard from '../components/chat/PermissionDialog'
import RunningToolsPill     from '../components/chat/RunningToolsPill'
import { SessionBar }   from '../components/chat/SessionBar'
import AtFilePicker, { type FileEntry } from '../components/chat/AtFilePicker'
import { useAgentStore }    from '../stores/agentStore'
import { useSessionStore }  from '../stores/sessionStore'
import { useSettingsStore, PA_MODES, type PAMode } from '../stores/settingsStore'
import { useCostStore }     from '../stores/costStore'
import { useArcanaMdStore } from '../stores/arcanaMdStore'
import { useWorkspaceStore } from '../stores/workspaceStore'
import SkillsPalette from '../components/skills/SkillsPalette'
import SkillSlashPicker from '../components/skills/SkillSlashPicker'
import SkillParameterModal from '../components/skills/SkillParameterModal'
import { useSkillStore, type Skill, type SkillSummary } from '../stores/skillStore'
import { SubagentMonitor } from '../components/subagent/SubagentMonitor'

// ─── Context window sizes (tokens) per model ─────────────────────────────────
// Fallback to 200_000 for unknown models.
const MODEL_CONTEXT: Record<string, number> = {
  // Anthropic
  'claude-opus-4-5':      200_000,
  'claude-sonnet-4-5':    200_000,
  'claude-haiku-3-5':     200_000,
  // OpenAI
  'gpt-4o':               128_000,
  'gpt-4o-mini':          128_000,
  'gpt-4.1':            1_000_000,
  'gpt-4.1-mini':       1_000_000,
  'o4-mini':              200_000,
  'o3':                   200_000,
  // Google
  'gemini-3.8-flash':  1_048_576,
  'gemini-2.0-flash':   1_000_000,
  'gemini-2.5-pro':     2_000_000,
  'gemini-1.5-pro':     2_000_000,
  'gemini-1.5-flash':   1_000_000,
}

// Rough token estimate: total_chars / 4 (industry heuristic)
function estimateTokens(charCount: number): number {
  return Math.ceil(charCount / 4)
}

function ContextMeter({ modelId, messages }: { modelId: string; messages: { content?: unknown; text?: string }[] }) {
  const contextWindow = MODEL_CONTEXT[modelId] ?? 200_000

  const usedTokens = useMemo(() => {
    let chars = 0
    for (const m of messages) {
      if (typeof (m as { text?: string }).text === 'string') chars += (m as { text: string }).text.length
      if (Array.isArray((m as { content?: unknown[] }).content)) {
        for (const c of (m as { content: unknown[] }).content) {
          if (typeof c === 'object' && c !== null && 'text' in c) chars += String((c as { text: unknown }).text).length
        }
      }
    }
    return estimateTokens(chars)
  }, [messages])

  const pct     = Math.min(usedTokens / contextWindow, 1)
  const pctDisp = (pct * 100).toFixed(1)
  const barColor = pct > 0.75 ? 'var(--danger)' : pct > 0.5 ? 'var(--warning)' : 'var(--accent)'

  const fmt = (n: number) => n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(0)}k` : String(n)

  return (
    <span className="flex items-center gap-1.5" title={`~${fmt(usedTokens)} / ${fmt(contextWindow)} tokens used`}>
      <span style={{ color: 'var(--text-4)' }}>ctx</span>
      <span
        style={{
          display: 'inline-block', width: 48, height: 3,
          background: 'var(--border-1)', borderRadius: 99, overflow: 'hidden', verticalAlign: 'middle',
        }}
      >
        <span
          style={{
            display: 'block', height: '100%',
            width: `${pct * 100}%`, background: barColor,
            borderRadius: 99, transition: 'width 0.3s ease',
          }}
        />
      </span>
      <span style={{ color: barColor }}>{pctDisp}%</span>
    </span>
  )
}

// ─── Chat header — PA mode dropdown + live stats row ────────────────────────

interface ChatHeaderProps { localDir: string; setLocalDir: (d: string) => void }

function ChatHeader({ localDir, setLocalDir }: ChatHeaderProps) {
  const paMode        = useSettingsStore(s => s.paMode)
  const setPAMode     = useSettingsStore(s => s.setPAMode)
  const byCategory    = useSkillStore(s => s.byCategory)
  const globalContent = useArcanaMdStore(s => s.globalContent)
  const loadGlobal    = useArcanaMdStore(s => s.loadGlobal)

  const [dropdownOpen, setDropdownOpen] = useState(false)
  const dropRef = useRef<HTMLDivElement>(null)

  useEffect(() => { loadGlobal() }, [])

  useEffect(() => {
    if (!dropdownOpen) return
    const handler = (e: MouseEvent) => {
      if (dropRef.current && !dropRef.current.contains(e.target as Node)) setDropdownOpen(false)
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [dropdownOpen])

  const currentMode = PA_MODES.find(m => m.value === paMode) ?? PA_MODES[0]
  const allSkills   = Object.values(byCategory).flat()
  // P8-55 - the "N MCP" counter is gone. It counted the two PYTHON
  // sidecar processes, not MCP servers: 2 when the orchestrator was
  // running, 1 while starting. External MCP servers have their own panel.
  const arcanaOk   = globalContent.trim().length > 0

  return (
    <div
      className="flex items-center justify-between px-3 shrink-0"
      style={{ height: '40px', background: 'var(--bg-card)', borderBottom: '1px solid var(--border-1)' }}
    >
      {/* Left: PA label + mode dropdown */}
      <div className="flex items-center gap-2">
        <span className="font-mono" style={{ fontSize: '10px', color: 'var(--text-4)', letterSpacing: '0.08em' }}>PA</span>

        <div ref={dropRef} style={{ position: 'relative' }}>
          <button
            onClick={() => setDropdownOpen(o => !o)}
            className="flex items-center gap-1 font-mono cursor-pointer"
            style={{ background: 'var(--bg-surface)', border: '1px solid var(--border-2)', borderRadius: '2px', padding: '3px 8px', fontSize: '11px', color: 'var(--text-1)' }}
          >
            <span>{currentMode.icon}</span>
            <span>{currentMode.label}</span>
            <span style={{ color: 'var(--text-4)', marginLeft: '2px' }}>&#9662;</span>
          </button>

          {dropdownOpen && (
            <div
              style={{ position: 'absolute', top: 'calc(100% + 4px)', left: 0, zIndex: 200, background: 'var(--bg-card)', border: '1px solid var(--border-2)', borderRadius: '2px', minWidth: '240px', boxShadow: '0 4px 16px rgba(0,0,0,0.5)' }}
            >
              {PA_MODES.map((m, i) => (
                <button
                  key={m.value}
                  onClick={() => { setPAMode(m.value as PAMode); setDropdownOpen(false) }}
                  className="flex items-start gap-2 w-full font-mono cursor-pointer"
                  style={{ background: paMode === m.value ? 'var(--accent-muted)' : 'transparent', border: 'none', borderBottom: i < PA_MODES.length - 1 ? '1px solid var(--border-1)' : 'none', padding: '8px 12px', fontSize: '11px', color: paMode === m.value ? 'var(--accent)' : 'var(--text-1)', textAlign: 'left' }}
                >
                  <span style={{ fontSize: '14px', lineHeight: 1 }}>{m.icon}</span>
                  <div>
                    <div>{m.label}</div>
                    <div style={{ fontSize: '10px', color: 'var(--text-3)', marginTop: '2px' }}>{m.description}</div>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Right: live stats */}
      <div className="flex items-center gap-2 font-mono" style={{ fontSize: '10px', color: 'var(--text-3)' }}>
        {allSkills.length > 0 && (
          <>
            <span style={{ color: 'var(--text-4)' }}>&#183;</span>
            <span>{allSkills.length} skills</span>
          </>
        )}
        <span style={{ color: 'var(--text-4)' }}>&#183;</span>
        <span style={{ color: arcanaOk ? 'var(--accent)' : 'var(--danger)' }}>
          ARCANA.md {arcanaOk ? '\u2713' : '\u2717'}
        </span>
        {localDir && (
          <>
            <span style={{ color: 'var(--text-4)' }}>&#183;</span>
            <input
              value={localDir}
              onChange={e => setLocalDir(e.target.value)}
              className="font-mono"
              style={{ background: 'transparent', border: 'none', outline: 'none', color: 'var(--text-4)', fontSize: '10px', width: '120px' }}
              placeholder="cwd…"
            />
          </>
        )}
      </div>
    </div>
  )
}

export default function ChatTab() {
  const messages      = useAgentStore(s => s.messages)
  const isStreaming        = useAgentStore(s => s.isStreaming)
  const pendingPermission  = useAgentStore(s => s.pendingPermission)
  const sessionStore  = useSessionStore()
  const settingsStore = useSettingsStore()
  const skillStore    = useSkillStore()
  const workingDir    = useAgentStore(s => s.workingDir)
  const setWorkingDir = useAgentStore(s => s.setWorkingDir)
  const setSessionId  = useAgentStore(s => s.setSessionId)
  const workspaceDir  = useWorkspaceStore(s => s.workspaceDir)

  const [input,         setInput]         = useState('')
  // Session working dir: prefer workspace dir over agent store default
  const [localDir,      setLocalDir]      = useState(workingDir)
  const [isStarting,    setIsStarting]    = useState(false)
  // /skill-name slash picker state
  const [slashQuery,       setSlashQuery]       = useState<string | null>(null)
  const [slashResults,     setSlashResults]     = useState<SkillSummary[]>([])
  const [slashSelectedIdx, setSlashSelectedIdx] = useState(0)
  const [slashSkill,       setSlashSkill]       = useState<Skill | null>(null)
  // @-mention file picker state
  const [atQuery,       setAtQuery]       = useState<string | null>(null)
  const [atResults,     setAtResults]     = useState<FileEntry[]>([])
  const [atSelectedIdx, setAtSelectedIdx] = useState(0)
  // Subagent monitor visibility
  const [showSubagents, setShowSubagents] = useState(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const inputRef       = useRef<HTMLTextAreaElement>(null)

  // Load sessions from backend on mount so SessionBar populates
  useEffect(() => {
    sessionStore.loadSessions()
    useWorkspaceStore.getState().loadWorkspaceDir()
  }, [])

  // Auto-scroll to bottom on new messages, but also when the inline
  // permission card / running-tools pill flips into view — without these
  // extra dependencies, the pill and card render below the fold and the
  // user has to scroll manually to see why the chat seems frozen.
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, pendingPermission, isStreaming])

  // Ref to hold active unlisteners so they can be cleaned up on unmount
  const unlistenersRef = useRef<(() => void)[]>([])

  // Cleanup all listeners on unmount
  useEffect(() => {
    return () => { unlistenersRef.current.forEach(fn => fn()) }
  }, [])

  // Resolve @path tokens to absolute paths relative to workspace dir.
  // Matches @word/path/file, @folder/name, @filename.ext — anything with . or /
  const resolveAtMentions = (text: string): string => {
    const ws = workspaceDir || localDir
    if (!ws) return text
    const sep = ws.includes('\\') ? '\\' : '/'
    return text.replace(/@([/\\]?[\w\-.][\w\-.+/\\]*[./\\][\w\-.+/\\]*|[\w-]+\.\w+)/g, (_, p) => {
      if (p.startsWith('/') || p.startsWith('\\') || /^[A-Za-z]:/.test(p)) return `@${p}`
      return `${ws}${sep}${p}`
    })
  }

  const ensureSession = async (): Promise<string> => {
    if (sessionStore.activeSessionId) return sessionStore.activeSessionId

    const provider = settingsStore.activeProvider ?? 'google'
    const model    = settingsStore.activeModel    ?? 'gemini-3.8-flash'
    // Use workspace dir as the session working dir if set
    const dir = workspaceDir || localDir
    const sessionId  = await sessionStore.createSession(dir, provider, model)
    setSessionId(sessionId)
    return sessionId
  }

  const handleSend = async () => {
    if (!input.trim() || isStreaming) return
    const text = resolveAtMentions(input.trim())
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
        paMode: settingsStore.paMode,
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

  // Handle @query: detect @ followed by non-space chars at cursor → trigger file search
  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val    = e.target.value
    setInput(val)
    const cursor     = e.target.selectionStart ?? val.length
    const textBefore = val.slice(0, cursor)
    const atMatch    = textBefore.match(/@(\S*)$/)
    if (atMatch) {
      const q = atMatch[1]
      setAtQuery(q)
      setAtSelectedIdx(0)
      invoke<FileEntry[]>('search_workspace_files', { query: q })
        .then(r => setAtResults(r))
        .catch(() => setAtResults([]))
      setSlashQuery(null)
      setSlashResults([])
    } else {
      setAtQuery(null)
      setAtResults([])
      // /skill-name detection: only trigger when not an @-mention
      const slashMatch = textBefore.match(/\/([\w-]*)$/)
      if (slashMatch) {
        const q = slashMatch[1]
        setSlashQuery(q)
        setSlashSelectedIdx(0)
        skillStore.searchSkills(q || '')
          .then(r => setSlashResults(r.slice(0, 8)))
          .catch(() => setSlashResults([]))
      } else {
        setSlashQuery(null)
        setSlashResults([])
      }
    }
  }

  // Select skill from slash picker — clear /word and open SkillParameterModal
  const handleSlashSelect = async (summary: SkillSummary) => {
    const cursor     = inputRef.current?.selectionStart ?? input.length
    const textBefore = input.slice(0, cursor)
    const slashIdx   = textBefore.lastIndexOf('/')
    const newInput   = input.slice(0, slashIdx) + input.slice(cursor)
    setInput(newInput)
    setSlashQuery(null)
    setSlashResults([])
    setSlashSelectedIdx(0)
    const skill = await skillStore.getSkill(summary.name)
    setSlashSkill(skill)
    setTimeout(() => inputRef.current?.focus(), 0)
  }

  // Execute skill from slash flow — insert rendered template into input
  const handleSlashExecute = async (params: Record<string, unknown>) => {
    if (!slashSkill) return
    const result = await skillStore.executeSkill(slashSkill.name, params)
    setInput(i => i ? i + '\n\n' + result.rendered : result.rendered)
    setSlashSkill(null)
    setTimeout(() => inputRef.current?.focus(), 0)
  }

  // Insert selected file path at the @query position — AI sees raw absolute path
  const handleFileSelect = (entry: FileEntry) => {
    const cursor     = inputRef.current?.selectionStart ?? input.length
    const textBefore = input.slice(0, cursor)
    const atIdx      = textBefore.lastIndexOf('@')
    if (atIdx === -1) { setAtQuery(null); setAtResults([]); return }
    const newInput = input.slice(0, atIdx) + entry.absolute_path + input.slice(cursor)
    setInput(newInput)
    setAtQuery(null)
    setAtResults([])
    setAtSelectedIdx(0)
    // Restore focus after React re-render
    setTimeout(() => inputRef.current?.focus(), 0)
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Intercept navigation when slash skill picker is open
    if (slashResults.length > 0) {
      if (e.key === 'ArrowDown') { e.preventDefault(); setSlashSelectedIdx(i => Math.min(i + 1, slashResults.length - 1)); return }
      if (e.key === 'ArrowUp')   { e.preventDefault(); setSlashSelectedIdx(i => Math.max(i - 1, 0)); return }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault()
        if (slashResults[slashSelectedIdx]) handleSlashSelect(slashResults[slashSelectedIdx])
        return
      }
      if (e.key === 'Escape') { e.preventDefault(); setSlashQuery(null); setSlashResults([]); return }
    }
    // Intercept navigation keys when the file picker is open
    if (atResults.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setAtSelectedIdx(i => Math.min(i + 1, atResults.length - 1))
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setAtSelectedIdx(i => Math.max(i - 1, 0))
        return
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault()
        if (atResults[atSelectedIdx]) handleFileSelect(atResults[atSelectedIdx])
        return
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        setAtQuery(null)
        setAtResults([])
        return
      }
    }
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
            <h1 className="font-mono text-4xl mb-4" style={{ color: 'var(--accent)', letterSpacing: '0.05em' }}>
              Oh My AIRS
            </h1>
            <p className="text-sm" style={{ color: 'var(--text-3)' }}>
              Create a new session to get started
            </p>
          </div>
        </div>
      ) : (
      <div className="flex flex-col flex-1 overflow-hidden relative">

        {/* Chat header — PA mode dropdown + live stats */}
        <ChatHeader localDir={localDir} setLocalDir={d => { setLocalDir(d); setWorkingDir(d) }} />

      {/* Messages list */}
      <div className="flex-1 overflow-y-auto px-3.5 py-3 flex flex-col gap-2.5">
        {messages.length === 0 && (
          <div
            className="flex flex-col items-center justify-center h-full gap-3"
            style={{ color: 'var(--text-3)' }}
          >
            <div className="font-mono text-xs" style={{ letterSpacing: '0.08em' }}>
              ARCANA — PA MODE
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

        {/* Running-tools status pill — shows tool name + spinner when the
            agent is mid-tick. Hidden when idle. Sits inline with the
            messages so the user can see what's happening without an
            obscuring modal. */}
        <RunningToolsPill />

        {/* Inline permission card — replaces the old centered-modal
            PermissionDialog. Renders only when a permission request is
            pending, in the same stream as the messages, so the user can
            scroll context next to the request. */}
        <InlinePermissionCard />

        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <div
        className="shrink-0"
        style={{
          background:   'var(--bg-surface)',
          borderTop:    '1px solid var(--border-1)',
          padding:      '10px 14px',
        }}
      >
        {/* /skill-name picker — full-width block above the textarea row */}
        {slashResults.length > 0 && slashQuery !== null && (
          <SkillSlashPicker
            results={slashResults}
            selectedIndex={slashSelectedIdx}
            query={slashQuery}
            onSelect={handleSlashSelect}
          />
        )}

        {/* @-mention file picker — full-width block above the textarea row */}
        {atResults.length > 0 && atQuery !== null && (
          <AtFilePicker
            results={atResults}
            selectedIndex={atSelectedIdx}
            query={atQuery}
            onSelect={handleFileSelect}
          />
        )}

        <SkillsPalette onInsert={rendered => setInput(i => i ? i + '\n\n' + rendered : rendered)} />

        {/* Slash skill parameter modal */}
        {slashSkill && (
          <SkillParameterModal
            skill={slashSkill}
            onExecute={handleSlashExecute}
            onClose={() => setSlashSkill(null)}
          />
        )}

        <div className="flex items-end gap-2">
          <textarea
            ref={inputRef}
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            placeholder="Say something… (@ to reference a file, Enter to send)"
            rows={1}
            style={{
              flex:        1,
              background:  'var(--bg-card)',
              border:      '1px solid var(--border-1)',
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

          {/* Skills palette button */}
          <button
            title="Open skills palette (⌘⇧S)"
            onClick={() => skillStore.openPalette()}
            className="shrink-0 font-mono text-xs px-2 py-2 rounded-sm cursor-pointer"
            style={{ background: 'var(--bg-2)', border: '1px solid var(--border)', color: 'var(--text-3)' }}
          >
            <Zap size={13} />
          </button>

          {isStreaming ? (
            <button
              onClick={handleCancel}
              aria-label="Cancel"
              className="shrink-0 font-mono text-xs px-3 py-2 rounded-sm cursor-pointer"
              style={{
                background: 'var(--danger-muted)',
                border:     '1px solid rgba(255,68,68,0.25)',
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
                background: 'var(--accent)',
                border:     '1px solid var(--accent)',
                color:      '#000',
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
          <span className="flex items-center gap-3">
            {settingsStore.activeModel && (
              <ContextMeter modelId={settingsStore.activeModel} messages={messages as { content?: unknown; text?: string }[]} />
            )}
            <span>{settingsStore.activeModel ?? 'no model'} · ManualApproval</span>
          </span>
        </div>
      </div>
      </div>
      )}

      {/* Subagent Monitor Toggle Button */}
      <button
        onClick={() => setShowSubagents(!showSubagents)}
        style={{
          position: 'fixed',
          right: '12px',
          bottom: '20px',
          width: '48px',
          height: '48px',
          borderRadius: '50%',
          background: 'var(--accent)',
          border: 'none',
          color: '#011627',
          cursor: 'pointer',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '20px',
          zIndex: 999,
          boxShadow: '0 4px 12px rgba(0,0,0,0.3)',
        }}
        title="Toggle subagent monitor"
      >
        🤖
      </button>

      {/* Subagent Monitor Sidebar (Collapsible) */}
      {showSubagents && (
        <div style={{
          position: 'fixed',
          right: 0,
          top: 0,
          bottom: 0,
          width: '320px',
          borderLeft: '1px solid #0D3652',
          background: 'var(--bg-surface)',
          overflowY: 'auto',
          zIndex: 1000,
          boxShadow: '-2px 0 8px rgba(0,0,0,0.3)',
        }}>
          <div style={{ padding: '12px', borderBottom: '1px solid #0D3652', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ fontSize: '12px', fontWeight: 'bold', color: 'var(--text-1)' }}>Subagent Monitor</span>
            <button
              onClick={() => setShowSubagents(false)}
              style={{
                background: 'none',
                border: 'none',
                color: 'var(--text-3)',
                cursor: 'pointer',
                fontSize: '16px',
              }}
            >
              ✕
            </button>
          </div>
          <SubagentMonitor parentSessionId={sessionStore.activeSessionId ?? undefined} />
        </div>
      )}
    </div>
  )
}
