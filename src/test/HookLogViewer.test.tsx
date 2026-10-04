import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'

import HookLogViewer from '../components/hooks/HookLogViewer'
import { useHookStore, type HookExecutionLog } from '../stores/hookStore'
import { parseDbTimestamp } from '../utils/dbTime'

const originalLoadLogs = useHookStore.getState().loadLogs

const log: HookExecutionLog = {
  id: 1,
  hook_id: 'hook-1',
  hook_name: 'Example hook',
  hook_type: 'pre_tool_use',
  outcome: 'executed',
  executed_at: '',
  duration_ms: 12,
}

beforeEach(() => {
  useHookStore.setState({ logs: [{ ...log }], loadLogs: vi.fn() })
})

afterEach(() => {
  useHookStore.setState({ logs: [], loadLogs: originalLoadLogs })
})

describe('HookLogViewer timestamp display', () => {
  it.each([
    ['legacy SQLite UTC', '2026-09-01 06:23:58'],
    ['canonical RFC3339-Z', '2026-09-01T06:23:58Z'],
  ])('formats %s timestamps from the parsed database instant', (_label, timestamp) => {
    useHookStore.setState({ logs: [{ ...log, executed_at: timestamp }] })

    render(<HookLogViewer />)

    const expected = parseDbTimestamp(timestamp)!.toLocaleTimeString()
    expect(screen.getByText(expected)).toBeInTheDocument()
  })
})
