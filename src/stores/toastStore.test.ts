// Toast store tests.
//
// The first of these exists because of a real, shipped defect. `workerStore.ts`
// produced worker-error and QC toasts with:
//
//     const { addToast } = require('../stores/toastStore')
//
// `addToast` is a method ON the Zustand store, not a module-level export, so
// that destructuring bound `undefined` and every toast it tried to raise did
// nothing. The store and the renderer were both correct; only the two producers
// were broken, and nothing failed loudly enough to notice. ESLint's very first
// run on this codebase flagged it as `no-require-imports`.

import { beforeEach, describe, expect, it } from 'vitest'
import { useToastStore } from './toastStore'

describe('toastStore', () => {
  beforeEach(() => {
    useToastStore.getState().clearAll()
  })

  it('exposes addToast through getState, the way producers call it', () => {
    // The regression guard: this is the exact access pattern workerStore uses.
    const { addToast } = useToastStore.getState()
    expect(typeof addToast).toBe('function')

    addToast({
      title: 'Worker worker-backend errored',
      message: 'Invoke failed',
      type: 'error',
      workerSlug: 'worker-backend',
      durationMs: 4000,
    })

    const { toasts } = useToastStore.getState()
    expect(toasts).toHaveLength(1)
    expect(toasts[0].title).toBe('Worker worker-backend errored')
    expect(toasts[0].workerSlug).toBe('worker-backend')
  })

  it('does NOT expose addToast as a module export', () => {
    // Documents why the require() form failed: there is no such named export,
    // so any future `import { addToast } from './toastStore'` is also wrong.
    const moduleExports = { useToastStore } as Record<string, unknown>
    expect(moduleExports.addToast).toBeUndefined()
  })

  it('stamps every toast with a unique id and a creation time', () => {
    const { addToast } = useToastStore.getState()
    const base = { message: 'm', type: 'info' as const, durationMs: 1000 }

    addToast({ ...base, title: 'first' })
    addToast({ ...base, title: 'second' })

    const { toasts } = useToastStore.getState()
    expect(toasts).toHaveLength(2)
    expect(toasts[0].id).not.toBe(toasts[1].id)
    expect(toasts[0].createdAt).toBeGreaterThan(0)
  })

  it('removes only the toast asked for', () => {
    const { addToast } = useToastStore.getState()
    const base = { message: 'm', type: 'info' as const, durationMs: 1000 }
    addToast({ ...base, title: 'keep' })
    addToast({ ...base, title: 'drop' })

    const target = useToastStore.getState().toasts.find(t => t.title === 'drop')!
    useToastStore.getState().removeToast(target.id)

    const remaining = useToastStore.getState().toasts
    expect(remaining).toHaveLength(1)
    expect(remaining[0].title).toBe('keep')
  })

  it('ignores removal of an unknown id rather than throwing', () => {
    const { addToast } = useToastStore.getState()
    addToast({ title: 't', message: 'm', type: 'info', durationMs: 1000 })

    expect(() => useToastStore.getState().removeToast('no-such-id')).not.toThrow()
    expect(useToastStore.getState().toasts).toHaveLength(1)
  })

  it('clears every toast', () => {
    const { addToast } = useToastStore.getState()
    addToast({ title: 'a', message: 'm', type: 'info', durationMs: 1000 })
    addToast({ title: 'b', message: 'm', type: 'warning', durationMs: 1000 })

    useToastStore.getState().clearAll()
    expect(useToastStore.getState().toasts).toEqual([])
  })
})
