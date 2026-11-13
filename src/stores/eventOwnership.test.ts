import { describe, expect, it, vi } from 'vitest'
import { useAgentLiveStore } from './agentLiveStore'
import { useDashboardStore } from './dashboardStore'

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

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(async () => []) }))

function emit(name: string, payload: unknown) {
  bus.handlers.get(name)?.forEach(handler => handler({ payload }))
}

// P6-06: these stores accumulate state from events, so App subscribes them once
// for the app's lifetime. None of these tests mounts a tab.
describe('events that arrive while no tab is mounted', () => {
  it('reach the Live Monitor store through the subscription App owns', () => {
    const unsubscribe = useAgentLiveStore.getState().subscribeToEvents()

    emit('agent:status-change', { agent_id: 'worker-a', status: 'active' })

    expect(useAgentLiveStore.getState().agents['worker-a']?.status).toBe('active')
    unsubscribe()
  })

  it('release every listener when App unsubscribes', async () => {
    const before = bus.live
    const unsubscribe = useAgentLiveStore.getState().subscribeToEvents()
    await Promise.resolve()
    expect(bus.live).toBeGreaterThan(before)

    unsubscribe()
    await new Promise(resolve => setTimeout(resolve, 0))

    expect(bus.live).toBe(before)
  })

  it('register the dashboard listeners once even when started twice at the same time', async () => {
    const store = useDashboardStore.getState()
    const before = bus.live
    await store.startRealtimeUpdates()
    const oneSet = bus.live - before
    store.stopRealtimeUpdates()
    expect(bus.live).toBe(before)
    expect(oneSet).toBeGreaterThan(0)

    await Promise.all([store.startRealtimeUpdates(), store.startRealtimeUpdates()])

    expect(bus.live - before).toBe(oneSet)
    store.stopRealtimeUpdates()
  })
})
