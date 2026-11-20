import { beforeEach, describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'

const invoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))

import UpdateSettings from './UpdateSettings'
import { useUpdateStore } from '../../stores/updateStore'

describe('UpdateSettings', () => {
  beforeEach(() => {
    invoke.mockReset()
    invoke.mockImplementation(async (cmd: string) => (cmd === 'updates_enabled' ? false : []))
    useUpdateStore.setState({ updatesEnabled: null })
  })

  it('shows the real version, not the build placeholder', async () => {
    render(<UpdateSettings />)
    expect(await screen.findByText(/^\d+\.\d+\.\d+/)).toBeTruthy()
    expect(screen.queryByText('__APP_VERSION__')).toBeNull()
  })

  it('says updates are off, and offers no check, when the build cannot verify them', async () => {
    render(<UpdateSettings />)
    expect(await screen.findByText('Updates are turned off in this version')).toBeTruthy()
    expect(screen.queryByText(/check for updates/i)).toBeNull()
  })

  it('offers the check when updates are on', async () => {
    invoke.mockImplementation(async (cmd: string) => (cmd === 'updates_enabled' ? true : []))
    render(<UpdateSettings />)
    expect(await screen.findByText(/check for updates/i)).toBeTruthy()
    expect(screen.queryByText('Updates are turned off in this version')).toBeNull()
  })
})
