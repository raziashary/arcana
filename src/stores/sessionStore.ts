// Session store — list of sessions, active session management.

import { create } from 'zustand'
import { invoke } from '@tauri-apps/api/core'

export interface Session {
  id:          string
  title:       string
  working_dir: string
  provider:    string
  model:       string
  created_at:  string
  updated_at:  string
}

interface SessionState {
  sessions:      Session[]
  activeSession: Session | null
  isLoading:     boolean

  loadSessions:    () => Promise<void>
  createSession:   (workingDir: string, provider: string, model: string) => Promise<Session>
  setActive:       (session: Session) => void
  deleteSession:   (id: string) => Promise<void>
}

export const useSessionStore = create<SessionState>((set) => ({
  sessions:      [],
  activeSession: null,
  isLoading:     false,

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

  createSession: async (workingDir, provider, model) => {
    const session = await invoke<Session>('create_session', {
      workingDir,
      provider,
      model,
    })
    set(s => ({ sessions: [session, ...s.sessions], activeSession: session }))
    return session
  },

  setActive: (session) => set({ activeSession: session }),

  deleteSession: async (id) => {
    await invoke('delete_session', { sessionId: id })
    set(s => ({
      sessions:      s.sessions.filter(sess => sess.id !== id),
      activeSession: s.activeSession?.id === id ? null : s.activeSession,
    }))
  },
}))
