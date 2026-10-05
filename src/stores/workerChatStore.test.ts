import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const listen = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

import { useWorkerChatStore, type WorkerMessage } from './workerChatStore'

function message(id: number, created_at: string): WorkerMessage {
  return {
    id, created_at, from_slug: 'worker-backend', to_slug: 'supervisor',
    body: 'fixture', kind: 'fyi', task_id: 1, status: 'pending', read_at: null,
  }
}

describe('Fleet Chat timestamp hydration', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useWorkerChatStore.setState({ messages: [], unreadByTarget: {}, initialized: false })
    listen.mockResolvedValue(vi.fn())
  })

  it('renders legacy and canonical database rows by UTC instant, with stable ID ties', async () => {
    const recent = [
      message(3, '2026-10-05 12:00:00'),
      message(2, '2026-10-05T05:00:00-07:00'),
      message(1, '2026-10-05T09:00:00.000Z'),
    ]
    invoke.mockImplementation(async (command: string) => (
      command === 'list_recent_worker_messages' ? recent : 0
    ))

    await useWorkerChatStore.getState().hydrate()

    expect(useWorkerChatStore.getState().messages.map(row => row.id)).toEqual([1, 2, 3])
    expect(invoke).toHaveBeenCalledWith('list_recent_worker_messages', { limit: 200 })
    expect(listen).toHaveBeenCalledWith('worker:message', expect.any(Function))
  })
})
