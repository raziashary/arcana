import path from 'node:path'

async function nativeInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return await browser.execute(async (cmd, input) => {
    const runtime = window as unknown as {
      __TAURI_INTERNALS__?: { invoke: (name: string, payload: Record<string, unknown>) => Promise<unknown> }
    }
    if (!runtime.__TAURI_INTERNALS__) throw new Error('Native Tauri IPC is unavailable.')
    return runtime.__TAURI_INTERNALS__.invoke(cmd, input)
  }, command, args) as T
}

async function remountSettings() {
  await $('button=Chat').click()
  await $('button=Settings').click()
}

async function selectFixtureWorkspace(workspace: string): Promise<void> {
  await nativeInvoke('set_workspace_dir', { path: workspace })
  const canonical = await nativeInvoke<string>('get_workspace_dir')
  await remountSettings()
  await browser.waitUntil(async () => browser.execute((expected) =>
    Array.from(document.querySelectorAll('[title]')).some(element => element.getAttribute('title') === expected),
    canonical), { timeoutMsg: 'Settings did not consume the selected native workspace.' })
}

const allowSelector = 'button[aria-label="Allow egress for GitHub MCP connector"]'
const revokeSelector = 'button[aria-label="Revoke consent for GitHub MCP connector"]'
const destination = { providerId: 'mcp:github-mcp' }
const fixtureDiff = 'diff --git a/src/example.ts b/src/example.ts\n--- a/src/example.ts\n+++ b/src/example.ts\n@@ -1 +1 @@\n-old\n+new\n'

async function expectLocalDiffReview() {
  const result = await nativeInvoke<{ analysis_mode: string; file_count: number; summary: string }>(
    'analyze_diff', { diffText: fixtureDiff },
  )
  await expect(result.analysis_mode).toBe('deterministic_only')
  await expect(result.file_count).toBe(1)
  await expect(result.summary).toContain('were not sent')
}

// This scenario sends no provider/MCP request. It changes consent only in the
// harness-owned profile and proves the UI consumes native persisted state.
describe('Workspace consent through the built desktop', () => {
  it('persists grants, isolates workspaces and reads revocation back in Settings', async () => {
    const root = process.env.ARCANA_E2E_FIXTURE_ROOT
    const workspaceA = process.env.ARCANA_E2E_WORKSPACE_A
    const workspaceB = process.env.ARCANA_E2E_WORKSPACE_B
    if (!root || !workspaceA || !workspaceB || [workspaceA, workspaceB].some(workspace =>
      path.dirname(path.resolve(workspace)) !== path.resolve(root))) {
      throw new Error('Refusing consent mutations without harness-owned disposable workspaces.')
    }
    await $('#root').waitForExist({ timeout: 30_000 })
    await $('button=Chat').waitForExist()
    try {
      await selectFixtureWorkspace(workspaceA)
      // Selecting a model persists only configuration; it sends no request.
      // Google has no grant in this disposable profile, so native diff review
      // must take the structural path before any credential lookup or egress.
      await nativeInvoke('set_active_provider', { provider: 'google', model: 'gemini-3.8-flash' })
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', { providerId: 'google' })).toBe(false)
      await expectLocalDiffReview()
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', destination)).toBe(false)
      await $(allowSelector).waitForExist()
      await $(allowSelector).click()
      await $(revokeSelector).waitForExist()
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', destination)).toBe(true)
      await expectLocalDiffReview()

      // Reopen the native application, not just React, to prove durable consent.
      await browser.reloadSession()
      await $('button=Chat').waitForExist({ timeout: 30_000 })
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', destination)).toBe(true)
      await remountSettings()
      await $(revokeSelector).waitForExist()
      await selectFixtureWorkspace(workspaceB)
      await $(allowSelector).waitForExist()
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', destination)).toBe(false)

      await selectFixtureWorkspace(workspaceA)
      await $(revokeSelector).waitForExist()
      await $(revokeSelector).click()
      await $(allowSelector).waitForExist()
      await expect(await nativeInvoke<boolean>('get_workspace_egress_consent', destination)).toBe(false)
      await remountSettings()
      await $(allowSelector).waitForExist()
      await expectLocalDiffReview()
    } finally {
      // Only the two validated fixture paths are ever touched by cleanup.
      for (const workspace of [workspaceA, workspaceB]) {
        await nativeInvoke('set_workspace_dir', { path: workspace })
        await nativeInvoke('revoke_workspace_egress_consent', destination)
      }
      await nativeInvoke('set_workspace_dir', { path: '' })
    }
  })
})
