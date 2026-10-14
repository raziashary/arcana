import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('../components/ide/FileTree', () => ({ default: () => <div /> }))
vi.mock('../components/ide/TabBar', () => ({ default: () => <div /> }))
vi.mock('../components/ide/BreadcrumbNav', () => ({ default: () => <div /> }))
vi.mock('../components/ide/CodeViewer', () => ({ default: () => <div /> }))
vi.mock('../components/ide/FileSearch', () => ({ default: () => <div /> }))
vi.mock('../components/ide/SymbolOutline', () => ({ default: () => <div /> }))
vi.mock('../components/ide/IdeStatusBar', () => ({ default: () => <div /> }))
vi.mock('../components/git/GitPanel', () => ({ default: () => <div /> }))

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useIdeStore } from '../stores/ideStore'
import { useWorkspaceStore } from '../stores/workspaceStore'
import IDETab from './IDE'

describe('IDE run action', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useWorkspaceStore.setState({ workspaceDir: 'C:/projects/sample' })
    useIdeStore.setState({
      activeTab: 'src/main.rs',
      openTabs: [{
        relativePath: 'src/main.rs',
        content: null,
        loading: false,
        error: null,
        gotoLine: null,
      }],
      init: vi.fn(async () => {}),
    })
  })

  afterEach(() => {
    useWorkspaceStore.setState({ workspaceDir: '' })
    useIdeStore.setState({ activeTab: null, openTabs: [], init: vi.fn(async () => {}) })
  })

  it('invokes the opened file with Auto and renders the host result', async () => {
    invoke.mockResolvedValue({
      toolchain: 'cargo',
      project_path: '.',
      command: 'cargo run',
      exit_code: 0,
      stdout: 'hello from host',
      stderr: '',
    })
    render(<IDETab />)

    fireEvent.click(screen.getByRole('button', { name: 'Run' }))

    expect(invoke).toHaveBeenCalledWith('run_ide_project', {
      path: 'src/main.rs',
      toolchain: null,
    })
    expect(await screen.findByRole('status', { name: 'Run output' })).toHaveTextContent('cargo · . · exit 0')
    expect(screen.getByText('Command: cargo run')).toBeInTheDocument()
    expect(screen.getByText('hello from host')).toBeInTheDocument()
  })

  it('passes a selected validated toolchain and bounds displayed output', async () => {
    invoke.mockResolvedValue({
      toolchain: 'cargo',
      project_path: '.',
      command: 'cargo run',
      exit_code: 1,
      stdout: `${'x'.repeat(8_100)} tail`,
      stderr: 'compiler failed',
    })
    render(<IDETab />)
    fireEvent.change(screen.getByRole('combobox', { name: 'Toolchain' }), { target: { value: 'cargo' } })
    fireEvent.click(screen.getByRole('button', { name: 'Run' }))

    expect(invoke).toHaveBeenCalledWith('run_ide_project', {
      path: 'src/main.rs',
      toolchain: 'cargo',
    })
    expect(await screen.findByText(/output truncated/)).toBeInTheDocument()
    expect(screen.getByText('compiler failed')).toBeInTheDocument()
    expect(screen.getByRole('status', { name: 'Run output' })).toHaveTextContent('exit 1')
  })

  it('disables the controls while the host command is pending', async () => {
    let finishRun: ((result: {
      toolchain: string
      project_path: string
      command: string
      exit_code: number
      stdout: string
      stderr: string
    }) => void) | undefined
    invoke.mockImplementation(() => new Promise((resolve) => { finishRun = resolve }))
    render(<IDETab />)

    fireEvent.click(screen.getByRole('button', { name: 'Run' }))

    expect(screen.getByRole('button', { name: 'Running…' })).toBeDisabled()
    expect(screen.getByRole('combobox', { name: 'Toolchain' })).toBeDisabled()
    finishRun?.({
      toolchain: 'cargo',
      project_path: '.',
      command: 'cargo run',
      exit_code: 0,
      stdout: '',
      stderr: '',
    })
    expect(await screen.findByText('No output.')).toBeInTheDocument()
  })

  it('disables Run without an active opened file and displays host errors accessibly', async () => {
    act(() => useIdeStore.setState({ activeTab: null }))
    render(<IDETab />)
    expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled()
    expect(screen.getByText('Open a file to run its project.')).toBeInTheDocument()

    act(() => useIdeStore.setState({
      activeTab: 'src/main.rs',
      openTabs: [{
        relativePath: 'src/main.rs', content: null, loading: true, error: null, gotoLine: null,
      }],
    }))
    expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled()
    expect(screen.getByText('Wait for the file to finish loading.')).toBeInTheDocument()

    act(() => useIdeStore.setState({
      openTabs: [{
        relativePath: 'src/main.rs', content: null, loading: false, error: 'Read failed', gotoLine: null,
      }],
    }))
    expect(screen.getByRole('button', { name: 'Run' })).toBeDisabled()
    expect(screen.getByText('The active file could not be loaded.')).toBeInTheDocument()

    act(() => useIdeStore.setState({
      openTabs: [{
        relativePath: 'src/main.rs', content: null, loading: false, error: null, gotoLine: null,
      }],
    }))
    invoke.mockRejectedValueOnce(new Error('Project manifest not found'))
    fireEvent.click(screen.getByRole('button', { name: 'Run' }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Project manifest not found')
    expect(screen.getByText('120-second limit')).toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Run' })).toBeEnabled())
  })
})
