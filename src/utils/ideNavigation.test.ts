import { beforeEach, describe, expect, it, vi } from 'vitest'

const { listenMock, invoke, setActiveTab, workspaceState, workspacePath } = vi.hoisted(() => {
  const workspacePath = String.fromCharCode(67, 58, 47) + 'work/project'
  return {
    listenMock: vi.fn(),
    invoke: vi.fn(),
    setActiveTab: vi.fn(),
    workspaceState: { workspaceDir: workspacePath, loadWorkspaceDir: vi.fn() },
    workspacePath,
  }
})

vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }))
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('../stores/uiStore', () => ({ useUiStore: { getState: () => ({ setActiveTab }) } }))
vi.mock('../stores/workspaceStore', () => ({ useWorkspaceStore: { getState: () => workspaceState } }))

import { registerIdeNavigationListener, workspaceRelativeFile } from './ideNavigation'
import { useIdeStore } from '../stores/ideStore'

describe('IDE navigation event consumer', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    useIdeStore.setState({
      rootTree: null,
      expandedDirs: new Set(),
      treeLoading: false,
      treeError: null,
      openTabs: [],
      activeTab: null,
      searchOpen: false,
      recentFiles: [],
    })
  })

  it('routes the event through workspace validation and the real IDE store to load a file', async () => {
    let handler: ((event: { payload: unknown }) => Promise<void>) | undefined
    listenMock.mockImplementation(async (_event: string, callback: typeof handler) => {
      handler = callback
      return vi.fn()
    })
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'read_file_tree') {
        const path = String(args?.path ?? '')
        const child = (name: string, relativePath: string, isDirectory: boolean) => ({
          name,
          relative_path: relativePath,
          absolute_path: `${workspacePath}/${relativePath}`,
          is_directory: isDirectory,
          size_bytes: null,
          modified_ms: null,
          git_status: null,
          children: [],
          has_more: false,
        })
        const children = path === ''
          ? [child('src', 'src', true)]
          : path === 'src'
            ? [child('nested', 'src/nested', true)]
            : [child('main.rs', 'src/nested/main.rs', false)]
        return {
          name: path.split('/').at(-1) ?? 'root',
          relative_path: path,
          absolute_path: `${workspacePath}/${path}`,
          is_directory: true,
          size_bytes: null,
          modified_ms: null,
          git_status: null,
          children,
          has_more: false,
        }
      }
      if (command === 'read_file_content') {
        return {
          absolute_path: `${workspacePath}/${String(args?.path)}`,
          relative_path: args?.path,
          content: 'fn main() {}',
          encoding: 'utf-8',
          is_binary: false,
          line_count: 1,
          size_bytes: 12,
          start_line: null,
          end_line: null,
          language: 'rust',
        }
      }
      return undefined
    })

    await registerIdeNavigationListener()
    expect(listenMock).toHaveBeenCalledWith('ide:navigate', expect.any(Function))
    await handler?.({ payload: { file: 'C:\\work\\project\\src\\nested\\main.rs', line: 19 } })

    expect(invoke).toHaveBeenCalledWith('read_file_content', {
      path: 'src/nested/main.rs', startLine: null, endLine: null,
    })
    expect(useIdeStore.getState().expandedDirs).toEqual(new Set(['src', 'src/nested']))
    expect(useIdeStore.getState().activeTab).toBe('src/nested/main.rs')
    expect(useIdeStore.getState().openTabs[0]).toMatchObject({
      relativePath: 'src/nested/main.rs',
      content: { content: 'fn main() {}' },
      gotoLine: 19,
      loading: false,
    })
    expect(setActiveTab).toHaveBeenCalledWith('ide')
  })

  it('rejects paths outside the workspace and malformed navigation events', async () => {
    let handler: ((event: { payload: unknown }) => Promise<void>) | undefined
    listenMock.mockImplementation(async (_event: string, callback: typeof handler) => {
      handler = callback
      return vi.fn()
    })

    await registerIdeNavigationListener()
    await handler?.({ payload: { file: 'C:/work/other/secrets.txt', line: 1 } })
    await handler?.({ payload: { file: '../outside.txt', line: 1 } })
    await handler?.({ payload: { file: 'src/main.rs', line: 0 } })

    expect(invoke).not.toHaveBeenCalled()
    expect(setActiveTab).not.toHaveBeenCalled()
  })

  it('maps absolute paths case-insensitively on Windows and rejects traversal', () => {
    expect(workspaceRelativeFile('c:\\WORK\\PROJECT\\src\\lib.rs', workspacePath))
      .toBe('src/lib.rs')
    expect(workspaceRelativeFile('src/../outside.rs', workspacePath)).toBeNull()
  })
})
