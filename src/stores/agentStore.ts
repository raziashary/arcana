// Agent store — messages, streaming state, tool call tracking.
// Listens to Tauri events: agent:token, agent:tool-start, agent:tool-end,
// agent:permission-request, agent:done, agent:error

import { create } from 'zustand'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

export type MessageRole = 'user' | 'assistant'

export interface ToolCallDisplay {
  callId:   string
  toolName: string
  input:    Record<string, unknown>
  output:   string | null
  isError:  boolean
  pending:  boolean
}

export interface ChatMessage {
  id:        string
  sessionId: string
  role:      MessageRole
  content:   string
  toolCalls: ToolCallDisplay[]
  tokensIn:  number
  tokensOut: number
  costUsd:   number
  createdAt: string
}

export interface PermissionRequest {
  callId:      string
  toolName:    string
  input:       Record<string, unknown>
  description: string
}

interface AgentState {
  messages:          ChatMessage[]
  isStreaming:       boolean
  streamingText:     string     // current in-progress token buffer
  pendingPermission: PermissionRequest | null
  sessionId:         string | null
  workingDir:        string

  // Actions
  setSessionId:    (id: string)  => void
  setWorkingDir:   (dir: string) => void
  addUserMessage:  (text: string) => ChatMessage
  startStreaming:  (sessionId: string) => void
  stopStreaming:   () => void
  clearMessages:  () => void
  setupListeners: (sessionId: string) => Promise<UnlistenFn[]>
}

let _msgIdCounter = 0
const nextId = () => `msg-${Date.now()}-${_msgIdCounter++}`

export const useAgentStore = create<AgentState>((set, get) => ({
  messages:          [],
  isStreaming:       false,
  streamingText:     '',
  pendingPermission: null,
  sessionId:         null,
  workingDir:        '.',

  setSessionId:  (id)  => set({ sessionId: id }),
  setWorkingDir: (dir) => set({ workingDir: dir }),

  addUserMessage: (text) => {
    const msg: ChatMessage = {
      id:        nextId(),
      sessionId: get().sessionId ?? '',
      role:      'user',
      content:   text,
      toolCalls: [],
      tokensIn:  0,
      tokensOut: 0,
      costUsd:   0,
      createdAt: new Date().toISOString(),
    }
    set(s => ({ messages: [...s.messages, msg] }))
    return msg
  },

  startStreaming: (sessionId) => {
    const assistantMsg: ChatMessage = {
      id:        nextId(),
      sessionId,
      role:      'assistant',
      content:   '',
      toolCalls: [],
      tokensIn:  0,
      tokensOut: 0,
      costUsd:   0,
      createdAt: new Date().toISOString(),
    }
    set(s => ({
      messages:      [...s.messages, assistantMsg],
      isStreaming:   true,
      streamingText: '',
    }))
  },

  stopStreaming: () => set({ isStreaming: false, streamingText: '' }),

  clearMessages: () => set({ messages: [], isStreaming: false, streamingText: '' }),

  setupListeners: async (sessionId) => {
    const unlisteners: UnlistenFn[] = []

    // Token streaming
    unlisteners.push(await listen<{ session_id: string; text: string }>('agent:token', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set(s => {
        const msgs = [...s.messages]
        const last = msgs[msgs.length - 1]
        if (last?.role === 'assistant') {
          msgs[msgs.length - 1] = { ...last, content: last.content + payload.text }
        }
        return { messages: msgs, streamingText: s.streamingText + payload.text }
      })
    }))

    // Tool start
    unlisteners.push(await listen<{ session_id: string; call_id: string; tool_name: string; input: Record<string, unknown> }>('agent:tool-start', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      const toolCall: ToolCallDisplay = {
        callId:   payload.call_id,
        toolName: payload.tool_name,
        input:    payload.input,
        output:   null,
        isError:  false,
        pending:  true,
      }
      set(s => {
        const msgs = [...s.messages]
        const last = msgs[msgs.length - 1]
        if (last?.role === 'assistant') {
          msgs[msgs.length - 1] = { ...last, toolCalls: [...last.toolCalls, toolCall] }
        }
        return { messages: msgs }
      })
    }))

    // Tool end
    unlisteners.push(await listen<{ session_id: string; call_id: string; output: string; is_error: boolean }>('agent:tool-end', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set(s => {
        const msgs = s.messages.map(msg => ({
          ...msg,
          toolCalls: msg.toolCalls.map(tc =>
            tc.callId === payload.call_id
              ? { ...tc, output: payload.output, isError: payload.is_error, pending: false }
              : tc
          ),
        }))
        return { messages: msgs }
      })
    }))

    // Permission request
    unlisteners.push(await listen<{ session_id: string; call_id: string; tool_name: string; input: Record<string, unknown>; description: string }>('agent:permission-request', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set({
        pendingPermission: {
          callId:      payload.call_id,
          toolName:    payload.tool_name,
          input:       payload.input,
          description: payload.description,
        },
        isStreaming: false,
      })
    }))

    // Agent done
    unlisteners.push(await listen<{ session_id: string; tokens_in: number; tokens_out: number; cost_usd: number }>('agent:done', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set(s => {
        const msgs = [...s.messages]
        const last = msgs[msgs.length - 1]
        if (last?.role === 'assistant') {
          msgs[msgs.length - 1] = {
            ...last,
            tokensIn:  payload.tokens_in,
            tokensOut: payload.tokens_out,
            costUsd:   payload.cost_usd,
          }
        }
        return { messages: msgs, isStreaming: false, streamingText: '' }
      })
    }))

    // Agent error
    unlisteners.push(await listen<{ session_id: string; message: string }>('agent:error', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set(s => {
        const msgs = [...s.messages]
        const last = msgs[msgs.length - 1]
        if (last?.role === 'assistant' && !last.content) {
          msgs[msgs.length - 1] = { ...last, content: `Error: ${payload.message}` }
        }
        return { messages: msgs, isStreaming: false }
      })
    }))

    return unlisteners
  },
}))
