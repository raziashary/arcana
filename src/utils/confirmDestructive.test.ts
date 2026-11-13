import { afterEach, describe, expect, it, vi } from 'vitest'
import { confirmDestructive } from './confirmDestructive'

describe('confirmDestructive', () => {
  afterEach(() => vi.restoreAllMocks())

  it('names the action and what is lost, and says it cannot be undone', () => {
    const ask = vi.spyOn(window, 'confirm').mockReturnValue(true)
    expect(confirmDestructive('Delete the hook "lint"', 'The hook stops running.')).toBe(true)
    const shown = String(ask.mock.calls[0][0])
    expect(shown).toContain('Delete the hook "lint"?')
    expect(shown).toContain('The hook stops running.')
    expect(shown).toContain('cannot be undone')
  })

  it('returns false when the user declines', () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false)
    expect(confirmDestructive('Delete', 'x')).toBe(false)
  })
})
