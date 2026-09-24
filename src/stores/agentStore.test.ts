import { describe, expect, it } from 'vitest'
import { hydrateActiveToolCalls, type ChatMessage } from './agentStore'

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
