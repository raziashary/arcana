// Agent store — messages, streaming state, tool call tracking.
// Listens to Tauri events: agent:token, agent:tool-start, agent:tool-end,
// agent:permission-request, agent:done, agent:error

import { create } from 'zustand'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

export type MessageRole = 'user' | 'assistant' | 'tool'

export interface ToolCallDisplay {
  callId:   string
  toolName: string
  input:    Record<string, unknown>
  output:   string | null
  isError:  boolean
  pending:  boolean
}

// Tool message as a separate chat message (for persistence)
export interface ToolMessage {
  id:        string
  sessionId: string
  role:      'tool'
  content:   string  // Tool output
  callId:    string
  toolName:  string
  toolInput: Record<string, unknown>
  isError:   boolean
  tokensIn:  number  // Always 0 for tool messages
  tokensOut: number  // Always 0 for tool messages
  costUsd:   number  // Always 0 for tool messages
  createdAt: string
}

// Unified chat message type (user, assistant, or tool)
export interface ChatMessage {
  id:        string
  sessionId: string
  role:      MessageRole
  content:   string
  // For assistant messages: embedded tool calls during streaming
  toolCalls: ToolCallDisplay[]
  // For tool messages: metadata
  callId?:   string
  toolName?: string
  toolInput?: Record<string, unknown>
  isError?:  boolean
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
  setMessages:    (messages: ChatMessage[]) => void
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

  setMessages: (messages) => set({ messages }),

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
          // Append to existing assistant message
          msgs[msgs.length - 1] = { ...last, content: last.content + payload.text }
        } else {
          // Create new assistant message after tool/user message
          msgs.push({
            id: crypto.randomUUID(),
            sessionId,
            role: 'assistant',
            content: payload.text,
            toolCalls: [],
            tokensIn: 0,
            tokensOut: 0,
            costUsd: 0,
            createdAt: new Date().toISOString(),
          })
        }
        return { messages: msgs, streamingText: s.streamingText + payload.text }
      })
    }))

    // Tool start - add tool call to the current assistant message (for streaming UI)
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

    // Tool end - update the pending tool call AND add a separate tool message for persistence
    unlisteners.push(await listen<{ session_id: string; call_id: string; tool_name: string; output: string; is_error: boolean }>('agent:tool-end', ({ payload }) => {
      if (payload.session_id !== sessionId) return
      set(s => {
        // First update the pending tool call in the assistant message
        const msgs = s.messages.map(msg => ({
          ...msg,
          toolCalls: msg.toolCalls.map(tc =>
            tc.callId === payload.call_id
              ? { ...tc, output: payload.output, isError: payload.is_error, pending: false }
              : tc
          ),
        }))
        // Add a separate tool message for proper conversation flow
        const toolMsg: ChatMessage = {
          id:        payload.call_id,
          sessionId: sessionId,
          role:      'tool',
          content:   payload.output,
          toolCalls: [],
          callId:    payload.call_id,
          toolName:  payload.tool_name,
          toolInput: {},  // Will be filled from the original tool call
          isError:   payload.is_error,
          tokensIn:  0,
          tokensOut: 0,
          costUsd:   0,
          createdAt: new Date().toISOString(),
        }
        return { messages: [...msgs, toolMsg] }
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
