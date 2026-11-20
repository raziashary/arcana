import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))

import { useUpdateStore } from './updateStore'

describe('updatesEnabled', () => {
  beforeEach(() => {
    invoke.mockReset()
    useUpdateStore.setState({ updatesEnabled: null })
  })

  it('is false when the build cannot verify updates', async () => {
    invoke.mockResolvedValue(false)
    await useUpdateStore.getState().loadUpdatesEnabled()
    expect(invoke).toHaveBeenCalledWith('updates_enabled')
    expect(useUpdateStore.getState().updatesEnabled).toBe(false)
  })

  it('is true when it can', async () => {
    invoke.mockResolvedValue(true)
    await useUpdateStore.getState().loadUpdatesEnabled()
    expect(useUpdateStore.getState().updatesEnabled).toBe(true)
  })

  it('stays unknown, not off, when the command fails', async () => {
    invoke.mockRejectedValue(new Error('ipc down'))
    await useUpdateStore.getState().loadUpdatesEnabled()
    expect(useUpdateStore.getState().updatesEnabled).toBeNull()
  })
})
