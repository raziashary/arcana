import { afterEach, describe, expect, it } from 'vitest'
import { useAgentLiveStore } from './agentLiveStore'

afterEach(() => {
  useAgentLiveStore.getState().initAgents([])
})

describe('worker removal in the live monitor', () => {
  it('removes a soft-deleted worker instead of leaving an idle card', () => {
    useAgentLiveStore.getState().initAgents(['worker-custom', 'worker-other'])

    useAgentLiveStore.getState().removeAgent('worker-custom')

    expect(Object.keys(useAgentLiveStore.getState().agents)).toEqual(['worker-other'])
  })
})
