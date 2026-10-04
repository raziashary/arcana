import { beforeEach, describe, expect, it, vi } from 'vitest'

const { listenMock, revealFile, setActiveTab, workspaceState, workspacePath } = vi.hoisted(() => {
  const workspacePath = String.fromCharCode(67, 58, 47) + 'work/project'
  return {
    listenMock: vi.fn(),
    revealFile: vi.fn(),
    setActiveTab: vi.fn(),
    workspaceState: { workspaceDir: workspacePath, loadWorkspaceDir: vi.fn() },
    workspacePath,
  }
})

vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }))
vi.mock('../stores/ideStore', () => ({ useIdeStore: { getState: () => ({ revealFile }) } }))
vi.mock('../stores/uiStore', () => ({ useUiStore: { getState: () => ({ setActiveTab }) } }))
vi.mock('../stores/workspaceStore', () => ({ useWorkspaceStore: { getState: () => workspaceState } }))

import { registerIdeNavigationListener, workspaceRelativeFile } from './ideNavigation'

describe('IDE navigation event consumer', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('opens a workspace file, reveals it in the tree and selects its line from the real event listener', async () => {
    let handler: ((event: { payload: unknown }) => Promise<void>) | undefined
    listenMock.mockImplementation(async (_event: string, callback: typeof handler) => {
      handler = callback
      return vi.fn()
    })

    await registerIdeNavigationListener()
    expect(listenMock).toHaveBeenCalledWith('ide:navigate', expect.any(Function))
    await handler?.({ payload: { file: 'C:\\work\\project\\src\\main.rs', line: 19 } })

    expect(revealFile).toHaveBeenCalledWith('src/main.rs', 19)
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

    expect(revealFile).not.toHaveBeenCalled()
    expect(setActiveTab).not.toHaveBeenCalled()
  })

  it('maps absolute paths case-insensitively on Windows and rejects traversal', () => {
    expect(workspaceRelativeFile('c:\\WORK\\PROJECT\\src\\lib.rs', workspacePath))
      .toBe('src/lib.rs')
    expect(workspaceRelativeFile('src/../outside.rs', workspacePath)).toBeNull()
  })
})
