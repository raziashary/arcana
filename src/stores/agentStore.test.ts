import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  hydrateActiveToolCalls,
  releaseSessionListeners,
  useAgentStore,
  type ChatMessage,
} from './agentStore'
import { useCostStore } from './costStore'

// A stand-in for Tauri's event bus: handlers by event name, and a count of the
// ones still registered.
const bus = vi.hoisted(() => ({
  handlers: new Map<string, Set<(event: { payload: unknown }) => void>>(),
  live: 0,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    const set = bus.handlers.get(name) ?? new Set()
    set.add(handler)
    bus.handlers.set(name, set)
    bus.live += 1
    return () => {
      if (set.delete(handler)) bus.live -= 1
    }
  }),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async () => []),
}))

function emit(name: string, payload: unknown) {
  bus.handlers.get(name)?.forEach(handler => handler({ payload }))
}

const assistantMessage = (): ChatMessage => ({
  id: 'assistant-1',
  sessionId: 'session-1',
  role: 'assistant',
  content: '',
  toolCalls: [],
  tokensIn: 0,
  tokensOut: 0,
  costUsd: 0,
  createdAt: '2026-09-24T00:00:00Z',
})

const activeTool = {
  session_id: 'session-1',
  agent_id: 'supervisor',
  call_id: 'call-1',
  tool_name: 'read_file',
  input: { path: 'package.json' },
}

describe('active tool execution hydration', () => {
  it('does not resurrect a tool that ended while the snapshot request was pending', () => {
    // The backend snapshot contains the in-flight call. Its end event arrives
    // before invoke resolves, so setupListeners records call-1 in this set.
    const hydrated = hydrateActiveToolCalls(
      [assistantMessage()],
      [activeTool],
      'session-1',
      new Set(['call-1']),
    )

    expect(hydrated[0].toolCalls).toEqual([])
  })

  it('hydrates a still-running call without duplicating an observed start event', () => {
    const message = assistantMessage()
    message.toolCalls.push({
      callId: 'call-1',
      toolName: 'read_file',
      input: activeTool.input,
      output: null,
      isError: false,
      pending: true,
    })

    const hydrated = hydrateActiveToolCalls([message], [activeTool], 'session-1', new Set())
    expect(hydrated[0].toolCalls).toHaveLength(1)
    expect(hydrated[0].toolCalls[0].pending).toBe(true)
  })
})

describe('chat listeners belong to the store, not the Chat tab', () => {
  beforeEach(() => {
    releaseSessionListeners()
    bus.handlers.clear()
    bus.live = 0
    useAgentStore.setState({ messages: [], isStreaming: false, streamingText: '' })
    useCostStore.setState({ costTodayUsd: 0, tokensInToday: 0, tokensOutToday: 0 })
  })

  it('keeps a reply streaming, ends it and records its cost with no Chat tab mounted', async () => {
    // Nothing here mounts the Chat component: the user has opened another tab.
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')

    emit('agent:token', { session_id: 'session-1', text: 'Hel' })
    emit('agent:token', { session_id: 'session-1', text: 'lo' })
    expect(useAgentStore.getState().messages.at(-1)?.content).toBe('Hello')
    expect(useAgentStore.getState().isStreaming).toBe(true)

    emit('agent:done', { session_id: 'session-1', tokens_in: 10, tokens_out: 5, cost_usd: 0.25 })
    expect(useAgentStore.getState().isStreaming).toBe(false)
    expect(useCostStore.getState().costTodayUsd).toBeCloseTo(0.25)
    expect(useCostStore.getState().tokensOutToday).toBe(5)
  })

  it('ignores another session and does not double-count after a second send', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    const registered = bus.live
    expect(registered).toBeGreaterThan(0)

    // The second send re-registers: the first set must be released, not stacked.
    await useAgentStore.getState().setupListeners('session-1')
    expect(bus.live).toBe(registered)

    emit('agent:token', { session_id: 'other', text: 'nope' })
    emit('agent:token', { session_id: 'session-1', text: 'once' })
    expect(useAgentStore.getState().messages.at(-1)?.content).toBe('once')

    emit('agent:done', { session_id: 'session-1', tokens_in: 1, tokens_out: 1, cost_usd: 0.1 })
    expect(useCostStore.getState().costTodayUsd).toBeCloseTo(0.1)
  })

  it('releases every listener on request', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    expect(bus.live).toBeGreaterThan(0)
    releaseSessionListeners()
    expect(bus.live).toBe(0)
  })
})

describe('a failed turn is always shown', () => {
  beforeEach(() => {
    releaseSessionListeners()
    bus.handlers.clear()
    bus.live = 0
    useAgentStore.setState({ messages: [], isStreaming: false, streamingText: '' })
  })

  it('fills the empty reply when the turn fails before any text', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')
    emit('agent:error', { session_id: 'session-1', message: 'API key rejected' })
    const messages = useAgentStore.getState().messages
    expect(messages.at(-1)?.content).toMatch(/^Error: API key rejected/)
    expect(useAgentStore.getState().isStreaming).toBe(false)
  })

  it('adds the error after partial text instead of dropping it', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')
    emit('agent:token', { session_id: 'session-1', text: 'Partial answer' })
    emit('agent:error', { session_id: 'session-1', message: 'The provider is overloaded' })
    const contents = useAgentStore.getState().messages.map(m => m.content)
    expect(contents).toContain('Partial answer')
    expect(contents.at(-1)).toMatch(/^Error: The provider is overloaded/)
  })

  it('adds the error when the last message is a tool result', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')
    emit('agent:tool-start', { session_id: 'session-1', call_id: 'c1', tool_name: 'read_file', input: {} })
    emit('agent:tool-end', { session_id: 'session-1', call_id: 'c1', tool_name: 'read_file', output: 'ok', is_error: false })
    expect(useAgentStore.getState().messages.at(-1)?.role).toBe('tool')
    emit('agent:error', { session_id: 'session-1', message: 'Rate limited' })
    expect(useAgentStore.getState().messages.at(-1)?.content).toMatch(/^Error: Rate limited/)
    expect(useAgentStore.getState().messages.at(-1)?.role).toBe('assistant')
  })

  it('shows a failure whose text mentions a cancel as a failure: the kind decides, not the words', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')
    emit('agent:error', { session_id: 'session-1', message: 'The run was cancelled.', kind: 'failed' })
    expect(useAgentStore.getState().messages.at(-1)?.content).toMatch(/^Error: The run was cancelled/)
  })

  it('shows a stopped turn as "Stopped.", not as an error', async () => {
    await useAgentStore.getState().setupListeners('session-1')
    useAgentStore.getState().startStreaming('session-1')
    emit('agent:token', { session_id: 'session-1', text: 'Half an answer' })
    emit('agent:error', { session_id: 'session-1', message: 'The run was cancelled.', kind: 'cancelled' })
    const contents = useAgentStore.getState().messages.map(m => m.content)
    expect(contents).toContain('Half an answer')
    expect(contents.at(-1)).toBe('Stopped.')
    expect(useAgentStore.getState().isStreaming).toBe(false)
  })
})
