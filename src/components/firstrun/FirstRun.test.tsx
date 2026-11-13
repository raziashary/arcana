import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import FirstRun, { isSetupComplete, keyFailureMessage } from './FirstRun'
import { useSettingsStore } from '../../stores/settingsStore'
import { useWorkspaceStore } from '../../stores/workspaceStore'
import { useSessionStore } from '../../stores/sessionStore'

const SECRET = 'sk-test-0123456789abcdef'

const model = { id: 'm1', name: 'M1', context_window: 1, supports_vision: false, cost_in_per_1m: 0, cost_out_per_1m: 0 }
let hasKey = false
let setKeyResult: unknown = { ok: true, masked_last4: 'cdef', validated: true }

function backend() {
  invoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case 'list_providers':
        return [{ id: 'google', name: 'Google', status: 'disconnected', has_key: hasKey, models: [model, model] }]
      case 'get_active_provider': return ['google', 'm1']
      case 'get_workspace_dir': return ''
      case 'set_provider_key':
        if (setKeyResult instanceof Error) throw setKeyResult
        hasKey = true
        return setKeyResult
      case 'pick_workspace_dir': return 'C:/projects/demo'
      case 'create_session':
        return { id: 's1', title: 'New', working_dir: args?.workingDir, pa_mode: 'fast_development' }
      default: return null
    }
  })
}

beforeEach(() => {
  hasKey = false
  setKeyResult = { ok: true, masked_last4: 'cdef', validated: true }
  invoke.mockReset()
  backend()
  useSettingsStore.setState({
    providers: [{ id: 'google', name: 'Google', status: 'disconnected', has_key: false, models: [model, model] }],
  })
  useWorkspaceStore.setState({ workspaceDir: '', error: null })
  useSessionStore.setState({ sessions: [], activeSessionId: null, activeSession: null })
})

afterEach(() => vi.restoreAllMocks())

async function enterKey() {
  fireEvent.change(screen.getByLabelText('Google API key'), { target: { value: SECRET } })
  fireEvent.click(screen.getByRole('button', { name: 'Verify and save' }))
}

describe('FirstRun', () => {
  it('takes a key, confirms the provider accepted it, and never shows it again', async () => {
    render(<FirstRun />)
    await enterKey()

    await waitFor(() => expect(screen.getByText('Google accepted the key.')).toBeInTheDocument())
    expect(invoke).toHaveBeenCalledWith('set_provider_key', { provider: 'google', apiKey: SECRET })
    expect(invoke).toHaveBeenCalledWith('set_active_provider', { provider: 'google', model: 'm1' })
    expect(document.body.textContent).not.toContain(SECRET)
    expect(screen.queryByDisplayValue(SECRET)).toBeNull()
  })

  it('says so when the key was stored without a check, instead of claiming it was verified', async () => {
    setKeyResult = { ok: true, masked_last4: 'cdef', validated: false }
    render(<FirstRun />)
    await enterKey()

    await waitFor(() => expect(screen.getByText(/could not be reached to check it/)).toBeInTheDocument())
    expect(screen.queryByText(/accepted the key/)).toBeNull()
  })

  it('names the next action when the provider rejects the key, without echoing it', async () => {
    setKeyResult = new Error(`That key was not accepted by google, so it has NOT been saved: ${SECRET} is invalid`)
    render(<FirstRun />)
    await enterKey()

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toMatch(/Copy the whole key again from Google/)
    expect(document.body.textContent).not.toContain(SECRET)
    expect(hasKey).toBe(false)
  })

  it('offers the $50 daily limit, then creates the first session in the chosen folder', async () => {
    render(<FirstRun />)
    expect(screen.getByRole('button', { name: 'Start' })).toBeDisabled()
    await enterKey()
    await screen.findByText('Google accepted the key.')
    fireEvent.click(screen.getByRole('button', { name: 'Select folder' }))
    await waitFor(() => expect(useWorkspaceStore.getState().workspaceDir).toBe('C:/projects/demo'))

    expect((screen.getByLabelText('Daily spend limit in US dollars') as HTMLInputElement).value).toBe('50')
    fireEvent.click(screen.getByRole('button', { name: 'Start' }))

    await waitFor(() => expect(useSessionStore.getState().activeSessionId).toBe('s1'))
    expect(invoke).toHaveBeenCalledWith('set_daily_cap', { enabled: true, limitUsd: 50 })
    expect(invoke).toHaveBeenCalledWith('create_session', { workingDir: 'C:/projects/demo', provider: 'google', model: 'm1' })
  })
})

describe('setup helpers', () => {
  it('is complete only with a stored key and a workspace folder', () => {
    const p = (has_key: boolean) => [{ id: 'a', name: 'A', status: '', has_key, models: [] }]
    expect(isSetupComplete(p(true), 'C:/x')).toBe(true)
    expect(isSetupComplete(p(false), 'C:/x')).toBe(false)
    expect(isSetupComplete(p(true), '  ')).toBe(false)
  })

  it('scrubs the key from a failure message', () => {
    expect(keyFailureMessage('Google', `bad ${SECRET}`, SECRET)).not.toContain(SECRET)
  })
})
