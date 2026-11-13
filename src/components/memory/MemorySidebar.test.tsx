import { describe, expect, it, vi } from 'vitest'
import { render, screen, waitFor } from '@testing-library/react'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { MemorySidebar } from './MemorySidebar'

describe('MemorySidebar', () => {
  it('shows the session timeline and the tier counts the backend returns', async () => {
    invoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_memory_stats') {
        return { episodic_count: 12, semantic_count: 4, procedural_count: 2, session_count: 3, last_updated: null }
      }
      if (cmd === 'get_session_timeline') {
        return [
          { id: 1, session_id: 's1', role: 'user', turn_index: 1, content: 'Add a retry to the provider call', tokens_in: 120, tokens_out: 0, cost_usd: 0, summary_type: null },
        ]
      }
      return []
    })

    render(<MemorySidebar sessionId="s1" />)

    await waitFor(() => expect(screen.getByText('Add a retry to the provider call')).toBeInTheDocument())
    expect(invoke).toHaveBeenCalledWith('get_session_timeline', expect.objectContaining({ sessionId: 's1' }))
    expect(screen.getByText('12')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Facts/ })).toBeInTheDocument()
  })
})
