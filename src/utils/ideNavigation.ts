import { listen } from '@tauri-apps/api/event'
import { useIdeStore } from '../stores/ideStore'
import { useUiStore } from '../stores/uiStore'
import { useWorkspaceStore } from '../stores/workspaceStore'

interface IdeNavigatePayload {
  file: string
  line: number
}

export function workspaceRelativeFile(file: string, workspaceDir: string): string | null {
  const normalizedFile = file.trim().replace(/\\/g, '/')
  const normalizedWorkspace = workspaceDir.trim().replace(/\\/g, '/').replace(/\/$/, '')
  if (!normalizedFile || !normalizedWorkspace) return null

  const isAbsolute = normalizedFile.startsWith('/') || /^[a-zA-Z]:\//.test(normalizedFile)
  let relative = normalizedFile
  if (isAbsolute) {
    const root = normalizedWorkspace.toLowerCase()
    const candidate = normalizedFile.toLowerCase()
    if (!candidate.startsWith(`${root}/`)) return null
    relative = normalizedFile.slice(normalizedWorkspace.length + 1)
  }

  const segments = relative.split('/').filter(Boolean)
  if (segments.length === 0 || segments.some((segment) => segment === '.' || segment === '..')) return null
  return segments.join('/')
}

function parsePayload(payload: unknown): IdeNavigatePayload | null {
  if (typeof payload !== 'object' || payload === null) return null
  const value = payload as Record<string, unknown>
  if (typeof value.file !== 'string' || typeof value.line !== 'number') return null
  if (!Number.isInteger(value.line) || value.line < 1) return null
  return { file: value.file, line: value.line }
}

export async function registerIdeNavigationListener(): Promise<() => void> {
  return listen<unknown>('ide:navigate', async ({ payload }) => {
    const navigation = parsePayload(payload)
    if (!navigation) return

    let { workspaceDir } = useWorkspaceStore.getState()
    if (!workspaceDir) {
      await useWorkspaceStore.getState().loadWorkspaceDir()
      workspaceDir = useWorkspaceStore.getState().workspaceDir
    }
    const path = workspaceRelativeFile(navigation.file, workspaceDir)
    if (!path) return

    await useIdeStore.getState().revealFile(path, navigation.line)
    useUiStore.getState().setActiveTab('ide')
  })
}
