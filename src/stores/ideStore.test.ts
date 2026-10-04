import { beforeEach, describe, expect, it, vi } from 'vitest'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

import { useIdeStore } from './ideStore'

describe('IDE file reveal', () => {
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
    invoke.mockImplementation(async (command: string, args?: Record<string, unknown>) => {
      if (command === 'read_file_tree') {
        const path = String(args?.path ?? '')
        const child = (name: string, relativePath: string, isDirectory: boolean) => ({
          name,
          relative_path: relativePath,
          absolute_path: `C:/project/${relativePath}`,
          is_directory: isDirectory,
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
          absolute_path: `C:/project/${path}`,
          is_directory: true,
          children,
          has_more: false,
        }
      }
      if (command === 'read_file_content') {
        return {
          relative_path: args?.path,
          content: 'fn main() {}',
          is_binary: false,
        }
      }
      return undefined
    })
  })

  it('loads ancestor folders, expands the file path, and opens the file at its requested line', async () => {
    await useIdeStore.getState().revealFile('src/nested/main.rs', 7)

    expect(invoke).toHaveBeenCalledWith('read_file_tree', { path: null, depth: 1 })
    expect(invoke).toHaveBeenCalledWith('read_file_tree', { path: 'src', depth: 1 })
    expect(invoke).toHaveBeenCalledWith('read_file_tree', { path: 'src/nested', depth: 1 })
    expect(invoke).toHaveBeenCalledWith('read_file_content', {
      path: 'src/nested/main.rs', startLine: null, endLine: null,
    })
    expect(useIdeStore.getState().expandedDirs).toEqual(new Set(['src', 'src/nested']))
    expect(useIdeStore.getState().activeTab).toBe('src/nested/main.rs')
    expect(useIdeStore.getState().openTabs[0]?.gotoLine).toBe(7)
    expect(useIdeStore.getState().rootTree?.children[0]?.children[0]?.children[0]?.relative_path)
      .toBe('src/nested/main.rs')
  })
})
