// Session store — list of sessions, active session management.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'
import { useToastStore } from './toastStore'

export interface Session {
  id:            string
  title:         string
  working_dir:   string
  provider:      string
  model:         string
  // Phase 3.0 R3 — persisted PA mode. Mirrors the Rust `Session.pa_mode`
  // column. When switching / creating sessions we copy this into
  // `settingsStore.paMode` so the mode toggle always reflects the loaded
  // session's stored setting.
  pa_mode:       string
  created_at:    string
  updated_at:    string
  message_count: number
}

interface SessionState {
  sessions:        Session[]
  activeSession:   Session | null
  activeSessionId: string | null
  isLoading:       boolean

  loadSessions:    () => Promise<void>
  createSession:   (workingDir?: string, provider?: string, model?: string) => Promise<string>
  switchSession:   (id: string) => Promise<void>
  renameSession:   (id: string, newTitle: string) => Promise<void>
  setActive:       (session: Session) => void
  deleteSession:   (id: string) => Promise<void>
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions:        [],
  activeSession:   null,
  activeSessionId: null,
  isLoading:       false,

  loadSessions: async () => {
    set({ isLoading: true })
    try {
      const sessions = await invoke<Session[]>('list_sessions')
      set({ sessions })
    } catch (e) {
      console.error('Failed to load sessions:', e)
    } finally {
      set({ isLoading: false })
    }
  },

  createSession: async (workingDir = '', provider = 'google', model = 'gemini-3.8-flash') => {
    let session: Session
    try {
      session = await invoke<Session>('create_session', {
        workingDir,
        provider,
        model,
      })
    } catch (error) {
      useToastStore.getState().addToast({
        title: 'Workspace unavailable',
        message: String(error),
        type: 'error',
        durationMs: 8000,
      })
      throw error
    }

    // Phase 3.0 R3 — carry the user's current PA-mode preference onto the
    // new session row so Delegate sticks across New Session clicks. If the
    // preference equals the default, the backend already has it.
    try {
      const { useSettingsStore } = await import('./settingsStore')
      const currentMode = useSettingsStore.getState().paMode
      if (currentMode && currentMode !== 'fast_development' && currentMode !== session.pa_mode) {
        await invoke('set_session_pa_mode', { sessionId: session.id, paMode: currentMode })
        session.pa_mode = currentMode
      }
    } catch (e) {
      console.error('Failed to seed pa_mode for new session:', e)
    }

    set(s => ({ 
      sessions: [session, ...s.sessions], 
      activeSession: session,
      activeSessionId: session.id
    }))
    return session.id
  },

  switchSession: async (id: string) => {
    const session = get().sessions.find(s => s.id === id)
    if (!session) return
    
    set({ activeSession: session, activeSessionId: id })

    // Phase 3.0 R3 — hydrate PA mode from the session row so the toggle
    // reflects what the user last chose for *this* chat (not a global
    // localStorage default). Lazy import avoids a circular dependency.
    if (session.pa_mode) {
      try {
        const { useSettingsStore } = await import('./settingsStore')
        const current = useSettingsStore.getState().paMode
        if (current !== session.pa_mode) {
          // Update directly (not via setPAMode) so we don't write back the
          // same value we just read — avoids a redundant round-trip.
          useSettingsStore.setState({ paMode: session.pa_mode as any })
          localStorage.setItem('arcana:pa_mode', session.pa_mode)
        }
      } catch (e) {
        console.error('Failed to hydrate paMode from session:', e)
      }
    }
    
    // Load messages for this session
    try {
      const messages = await invoke<any[]>('get_session_messages', { sessionId: id })
      
      // Transform backend Message format to frontend ChatMessage format
      // Tool messages are now separate rows in the database
      const chatMessages = messages.map(msg => {
        // Check if this is a tool message
        if (msg.role === 'tool') {
          return {
            id: msg.id,
            sessionId: msg.session_id,
            role: 'tool' as const,
            content: msg.content,
            toolCalls: [],  // Tool messages don't have nested tool calls
            callId: msg.tool_call_id,
            toolName: msg.tool_name,
            toolInput: msg.tool_input || {},
            isError: msg.is_error,
            tokensIn: 0,
            tokensOut: 0,
            costUsd: 0,
            createdAt: msg.created_at,
          }
        }
        
        // For user and assistant messages
        return {
          id: msg.id,
          sessionId: msg.session_id,
          role: msg.role as 'user' | 'assistant',
          content: msg.content,
          toolCalls: [],  // Tool calls are now separate messages
          tokensIn: msg.tokens_in || 0,
          tokensOut: msg.tokens_out || 0,
          costUsd: msg.cost_usd || 0,
          createdAt: msg.created_at,
        }
      })
      
      // Import useAgentStore dynamically to avoid circular dependency
      const { useAgentStore } = await import('./agentStore')
      useAgentStore.getState().setMessages(chatMessages)
    } catch (error) {
      console.error('Failed to load session messages:', error)
    }
  },

  renameSession: async (id: string, newTitle: string) => {
    try {
      await invoke('update_session_title', { sessionId: id, title: newTitle })
      const state = get()
      const updatedSessions = state.sessions.map(s => 
        s.id === id ? { ...s, title: newTitle } : s
      )
      set({ 
        sessions: updatedSessions,
        activeSession: state.activeSessionId === id 
          ? { ...state.activeSession!, title: newTitle }
          : state.activeSession
      })
    } catch (error) {
      console.error('Failed to rename session:', error)
    }
  },

  setActive: (session) => set({ activeSession: session, activeSessionId: session.id }),

  deleteSession: async (id) => {
    await invoke('delete_session', { sessionId: id })
    const state = get()
    const remainingSessions = state.sessions.filter(sess => sess.id !== id)
    
    set({
      sessions: remainingSessions,
      activeSession: state.activeSession?.id === id 
        ? (remainingSessions[0] || null)
        : state.activeSession,
      activeSessionId: state.activeSessionId === id
        ? (remainingSessions[0]?.id || null)
        : state.activeSessionId,
    })
  },
}))
