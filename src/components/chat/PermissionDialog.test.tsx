import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }))

import PermissionDialog from './PermissionDialog'
import { useAgentStore } from '../../stores/agentStore'

describe('PermissionDialog', () => {
  afterEach(() => {
    cleanup()
    useAgentStore.setState({ pendingPermission: null })
  })

  it('hides standing approval for a one-call host installation', () => {
    useAgentStore.setState({
      pendingPermission: {
        callId: 'install-call',
        toolName: 'project_command',
        input: { intent: 'install' },
        description: 'install the selected project dependencies',
        allowAlways: false,
      },
    })

    render(<PermissionDialog />)

    expect(screen.getByText(/install the selected project dependencies/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Allow' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'Allow Always' })).not.toBeInTheDocument()
  })

  it('preserves standing approval for effects that support it', () => {
    useAgentStore.setState({
      pendingPermission: {
        callId: 'build-call',
        toolName: 'project_command',
        input: { intent: 'build' },
        description: 'run a command on your machine',
        allowAlways: true,
      },
    })

    render(<PermissionDialog />)

    expect(screen.getByRole('button', { name: 'Allow Always' })).toBeInTheDocument()
  })
})
