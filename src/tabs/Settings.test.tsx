import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
const openDialog = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openDialog }))

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useSettingsStore } from '../stores/settingsStore'
import { useWorkspaceStore } from '../stores/workspaceStore'
import { PythonFallbackCard, WorkspaceEgressConsentCard } from './Settings'

describe('WorkspaceEgressConsentCard', () => {
  const grants = new Set<string>()
  let hooks: Array<{ id: string; name: string; action: { type: string } }> = []

  const grantKey = (providerId: string) =>
    `${useWorkspaceStore.getState().workspaceDir}|${providerId}`

  beforeEach(() => {
    vi.clearAllMocks()
    grants.clear()
    hooks = []
    useWorkspaceStore.setState({ workspaceDir: 'C:/projects/arcana' })
    useSettingsStore.setState({ providers: [] })
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'list_providers') {
        return [{ id: 'google', name: 'Google', status: 'connected', models: [], has_key: true }]
      }
      if (command === 'get_active_provider') return ['google', 'gemini-3.8-flash']
      if (command === 'list_hooks') return hooks
      if (command === 'get_workspace_egress_consent') {
        return grants.has(grantKey(String(args?.providerId)))
      }
      if (command === 'grant_workspace_egress_consent') {
        grants.add(grantKey(String(args?.providerId)))
        return undefined
      }
      if (command === 'revoke_workspace_egress_consent') {
        return grants.delete(grantKey(String(args?.providerId)))
      }
      if (command === 'get_python_user_fallback') return null
      if (command === 'set_python_user_fallback') return args?.path ?? null
      throw new Error(`Unexpected Tauri command: ${command}`)
    })
  })

  afterEach(() => {
    cleanup()
    vi.unstubAllGlobals()
    useWorkspaceStore.setState({ workspaceDir: '' })
    useSettingsStore.setState({ providers: [] })
  })

  it('reads, grants, refreshes, and revokes provider consent through Tauri commands', async () => {
    render(<WorkspaceEgressConsentCard />)

    const allow = await screen.findByRole('button', { name: 'Allow egress for Google' })
    expect(invoke).toHaveBeenCalledWith('get_workspace_egress_consent', { providerId: 'google' })

    fireEvent.click(allow)
    await screen.findByRole('button', { name: 'Revoke consent for Google' })
    expect(invoke).toHaveBeenCalledWith('grant_workspace_egress_consent', { providerId: 'google' })

    act(() => useWorkspaceStore.setState({ workspaceDir: 'C:/projects/other' }))
    await screen.findByRole('button', { name: 'Allow egress for Google' })

    act(() => useWorkspaceStore.setState({ workspaceDir: 'C:/projects/arcana' }))
    await screen.findByRole('button', { name: 'Revoke consent for Google' })

    fireEvent.click(screen.getByRole('button', { name: 'Revoke consent for Google' }))
    await screen.findByRole('button', { name: 'Allow egress for Google' })
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('revoke_workspace_egress_consent', { providerId: 'google' })
      expect(grants.has('C:/projects/arcana|google')).toBe(false)
    })
  })

  // An HTTP hook is an egress destination: the card must offer it, name it by
  // hook id, and grant/revoke it through the same consent commands.
  it('grants egress for an HTTP hook by hook destination and ignores non-network hooks', async () => {
    hooks = [
      { id: 'hook-net', name: 'Notify build', action: { type: 'http' } },
      { id: 'hook-log', name: 'Write log', action: { type: 'log' } },
    ]
    render(<WorkspaceEgressConsentCard />)

    const allow = await screen.findByRole('button', {
      name: 'Allow egress for HTTP hook Notify build',
    })
    expect(invoke).toHaveBeenCalledWith('get_workspace_egress_consent', {
      providerId: 'hook:hook-net',
    })
    expect(screen.queryByRole('button', { name: /Write log/ })).toBeNull()

    fireEvent.click(allow)
    await screen.findByRole('button', { name: 'Revoke consent for HTTP hook Notify build' })
    expect(invoke).toHaveBeenCalledWith('grant_workspace_egress_consent', {
      providerId: 'hook:hook-net',
    })
  })

  it('lets the user select, persist, and clear a Windows Python fallback', async () => {
    vi.stubGlobal('navigator', { userAgent: 'Windows NT 10.0' })
    openDialog.mockResolvedValue('C:/Users/test/AppData/Local/Programs/Python/Python314/python.exe')
    render(<PythonFallbackCard />)

    fireEvent.click(await screen.findByRole('button', { name: 'Select Python' }))
    await screen.findByRole('button', { name: 'Change' })
    expect(invoke).toHaveBeenCalledWith('set_python_user_fallback', {
      path: 'C:/Users/test/AppData/Local/Programs/Python/Python314/python.exe',
    })
    expect(openDialog).toHaveBeenCalledWith({
      multiple: false,
      directory: false,
      filters: [{ name: 'Python interpreter', extensions: ['exe'] }],
    })

    fireEvent.click(screen.getByRole('button', { name: 'Clear Python fallback' }))
    await screen.findByText('No fallback selected')
    expect(invoke).toHaveBeenCalledWith('set_python_user_fallback', { path: null })
  })
})
